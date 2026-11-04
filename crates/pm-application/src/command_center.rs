//! 项目总控台读模型（S01 · I5-C）。
//!
//! 契约：`docs/s01-i5-read-model-contract.md`（I5-B 定稿）
//! 事实：`docs/s01-i5-code-facts.md`（I5-A 核验）
//!
//! 原则（契约 §3）：
//! - **纯读**：不写任何领域业务状态；写操作仍走原领域端点；
//! - **每条 SQL 带 tenant 谓词**——不复用「只按 project_id 过滤」的既有函数而不补租户条件
//!   （I5-A §12.3/§12.5 的缺口 I5-G-13）；
//! - **每块独立降级**：一个数据源失败只让该块 `unavailable`，其余照常；
//! - **原始状态保留**：跨域展示保留来源领域原始状态字符串，展示层严重度只是附加映射。
//!
//! **不产生百分比**：初始化进度只给事项状态与计数（设计稿 §4.8）。

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// 活动白名单 + action → 归一化活动类型（设计稿 §5.3 → 契约 §4.7）。
///
/// **唯一真源**：SQL 的 `CASE` 映射与 Rust 侧 `activity_type_for` 都由本表生成，
/// 避免两处漂移。非白名单 action 一律不展示。
pub const ACTIVITY_TYPE_BY_ACTION: &[(&str, &str)] = &[
    ("project.archived", "project_archived"),
    ("commit_document_revision", "goal_committed"),
    ("finding.decided", "finding_decided"),
    (
        "discovery_finding_revision_requested",
        "finding_revision_requested",
    ),
    ("repository.created", "repository_created"),
    ("repository.verified", "repository_verified"),
    ("repo_import.draft_succeeded", "analysis_succeeded"),
    ("repo_import.attempt_succeeded", "analysis_succeeded"),
    ("repo_import.failed", "analysis_failed"),
    ("repo_import.attempt_failed", "analysis_failed"),
    ("repo_import.draft_failed", "analysis_failed"),
    ("run_started", "run_started"),
    ("claim_task_run", "run_started"),
    ("run_finished", "run_finished"),
    ("run_start_failed", "run_failed"),
    ("terminate_task_run", "run_terminated"),
    ("delivery.submitted", "delivery_submitted"),
    ("delivery.rejected", "delivery_rejected"),
    ("review.completed", "review_completed"),
    ("review.changes_requested", "review_changes_requested"),
    ("project_member.granted", "member_changed"),
    ("project_member.revoked", "member_changed"),
    ("project_member.owner_transferred", "member_changed"),
    ("device.project_bound", "device_bound"),
];

/// 白名单 action 列表（供 SQL `= ANY($n)` 过滤）。
pub fn activity_audit_actions() -> Vec<String> {
    ACTIVITY_TYPE_BY_ACTION
        .iter()
        .map(|(action, _)| (*action).to_string())
        .collect()
}

/// 知识提案的「活跃」状态集（待人工处理口径；复用 `review::knowledge` 的同一常量）。
pub const ACTIVE_PROPOSAL_STATUSES: [&str; 5] = [
    "draft",
    "quality_gates_passed",
    "redline_pending",
    "confirmed",
    "commit_pending",
];

/// 初始化事项的展示层状态（设计稿 §4.4；**不是**领域数据库状态）。
pub mod init_status {
    pub const NOT_STARTED: &str = "not_started";
    pub const IN_PROGRESS: &str = "in_progress";
    /// 系统已产生需要人类处理的事项（系统交出球，等待人类行动）。
    /// **展示层状态**：不进入任何领域实体的权威状态体系。
    pub const WAITING_FOR_HUMAN: &str = "waiting_for_human";
    pub const COMPLETED: &str = "completed";
    pub const BLOCKED: &str = "blocked";
    pub const NOT_APPLICABLE: &str = "not_applicable";
    pub const UNDETERMINED: &str = "undetermined";
}

/// 初始化事项的展示状态判定（**纯函数**，便于单测覆盖七态与跨域组合）。
///
/// S01-01 · G-01 七态模型：
///   - `in_progress` —— 系统正在工作；
///   - `waiting_for_human` —— 系统已交出球，等待人类行动；
///   - `blocked` —— 外部条件阻止继续推进，当前**没有**可执行的人类确认动作。
///
/// 三者不得混为一谈。
///
/// 判定纪律：**不得**用「存在 PENDING 记录」直接推出 `waiting_for_human`——
/// 必须依据该事项与人的**实际操作责任**：只有当该事项的下一步动作确实由人
/// 承担（人工确认 / 人工批准 / 人工提交）时才判为等待人类。
pub mod init_judgement {
    use super::init_status;

    /// 项目目标：已发布 → 完成；存在待人工红线确认的提案 → 等人；其余活跃提案由系统推进。
    ///
    /// `human_pending` 只统计 `redline_pending`（唯一可达的「待人工确认」提案状态，
    /// 见 knowledge_proposal/submit.rs 的状态派生）；`draft` / `commit_pending` 属系统处理中。
    pub fn goal_status(present: bool, human_pending: bool, pending: bool) -> &'static str {
        if present {
            return init_status::COMPLETED;
        }
        if human_pending {
            return init_status::WAITING_FOR_HUMAN;
        }
        if pending {
            return init_status::IN_PROGRESS;
        }
        init_status::NOT_STARTED
    }

    /// 仓库接入：零仓库合法（不判失败）；不可达 → 受阻；待核验 → 系统进行中。
    pub fn repository_access_status(
        total: i64,
        unreachable: i64,
        pending: i64,
    ) -> &'static str {
        if total == 0 {
            // 零仓库是合法状态：不判失败（设计稿 §4.7）。
            return init_status::NOT_STARTED;
        }
        if unreachable > 0 {
            return init_status::BLOCKED;
        }
        if pending > 0 {
            return init_status::IN_PROGRESS;
        }
        init_status::COMPLETED
    }

    /// 代码分析：作业状态 → 展示状态。
    ///
    /// `draft_ready` / `proposed` 的下一步动作由人承担（草案正文自述「需人工确认」，
    /// 提交/批准入口是人类触发的治理动作），故判为等待人类；
    /// `applied` 才算完成——分析跑完不等于结论已成为正式事实。
    pub fn code_analysis_status(repo_total: i64, import_status: Option<&str>) -> &'static str {
        if repo_total == 0 {
            return init_status::NOT_APPLICABLE;
        }
        match import_status {
            Some("queued" | "cloning" | "reading" | "analyzing") => init_status::IN_PROGRESS,
            Some("draft_ready" | "proposed") => init_status::WAITING_FOR_HUMAN,
            Some("applied") => init_status::COMPLETED,
            Some("failed" | "rejected" | "cancelled") => init_status::BLOCKED,
            _ => init_status::NOT_STARTED,
        }
    }

    /// 项目认知：**以领域会话状态为权威信号**。
    ///
    /// 优先级（G-01 验收：人工待办不得被普通进行中覆盖）：
    ///   1. 会话自声明的人审态（`NEEDS_USER_INPUT` / `REVIEW_REQUIRED`）→ 等人；
    ///   2. 待人工确认发现（核实决定者只能是人类）→ 等人——即使会话仍在分析，
    ///      已产出的待确认事项也是当下真实的人类动作；
    ///   3. 其余未完成会话 → 系统进行中；
    ///   4. 会话全部完成且无待确认发现 → 完成（G-06：完成 ≠ 自动全确认，
    ///      但无任何待办时才允许判完成）。
    pub fn cognition_status(
        sessions: &[(String, i64)],
        pending_findings: i64,
    ) -> &'static str {
        let total: i64 = sessions.iter().map(|(_, n)| n).sum();
        if total == 0 {
            return init_status::NOT_STARTED;
        }
        let human_waiting: i64 = sessions
            .iter()
            .filter(|(status, _)| status == "NEEDS_USER_INPUT" || status == "REVIEW_REQUIRED")
            .map(|(_, n)| n)
            .sum();
        if human_waiting > 0 {
            return init_status::WAITING_FOR_HUMAN;
        }
        if pending_findings > 0 {
            return init_status::WAITING_FOR_HUMAN;
        }
        let open: i64 = sessions
            .iter()
            .filter(|(status, _)| status != "COMPLETED")
            .map(|(_, n)| n)
            .sum();
        if open > 0 {
            return init_status::IN_PROGRESS;
        }
        init_status::COMPLETED
    }

    /// 开发环境：从未自检 → 证据不足（待判定），不臆断为等人类或完成。
    pub fn environment_status(
        total: i64,
        ready: i64,
        degraded: i64,
        unchecked: i64,
    ) -> &'static str {
        if total == 0 {
            return init_status::NOT_STARTED;
        }
        if degraded > 0 {
            return init_status::BLOCKED;
        }
        if unchecked > 0 {
            return init_status::UNDETERMINED;
        }
        if ready > 0 {
            return init_status::COMPLETED;
        }
        init_status::IN_PROGRESS
    }

    /// 人工处理：存在待人工处理事项 → 系统已交出球（等待人类）。
    pub fn human_actions_status(action_total: i64) -> &'static str {
        if action_total == 0 {
            return init_status::COMPLETED;
        }
        init_status::WAITING_FOR_HUMAN
    }
}

/// 数据块状态位（契约 §2）。
pub const BLOCK_OK: &str = "ok";
pub const BLOCK_UNAVAILABLE: &str = "unavailable";

/// 块级错误（不泄露 SQL/内部细节）。
#[derive(Debug, Clone, Serialize)]
pub struct BlockError {
    pub code: String,
    pub message: String,
}

/// 一个数据块：状态 + 新鲜度 + 数据。
#[derive(Debug, Clone, Serialize)]
pub struct Block<T> {
    /// `ok` | `unavailable`
    pub status: String,
    /// 本块数据的新鲜度（查询时刻）。
    pub as_of: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<BlockError>,
    pub data: Option<T>,
}

fn ok_block<T>(data: T) -> Block<T> {
    Block {
        status: BLOCK_OK.to_string(),
        as_of: Utc::now(),
        error: None,
        data: Some(data),
    }
}

fn unavailable_block<T>(code: &str, err: impl std::fmt::Display) -> Block<T> {
    tracing::warn!(code, error = %err, "command center block unavailable");
    Block {
        status: BLOCK_UNAVAILABLE.to_string(),
        as_of: Utc::now(),
        error: Some(BlockError {
            code: code.to_string(),
            message: "该数据源暂时不可用，请稍后重试".to_string(),
        }),
        data: None,
    }
}

/// 总控台聚合响应（块顺序 = 首屏优先级，设计稿 §3.1）。
#[derive(Debug, Clone, Serialize)]
pub struct CommandCenter {
    pub project_id: Uuid,
    pub as_of: DateTime<Utc>,
    pub project_summary: Block<ProjectSummary>,
    pub initialization_progress: Block<InitializationProgress>,
    pub work_summary: Block<WorkSummary>,
    pub human_action_summary: Block<HumanActionSummary>,
    pub risk_summary: Block<RiskSummary>,
    pub recent_activity: Block<RecentActivity>,
}

// ---------------------------------------------------------------------------
// §4.1 project_summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub project_id: Uuid,
    pub code: String,
    pub name: String,
    /// `core.project.status` 原样透传（`planning`/`initializing`/`archived`）。
    pub lifecycle: String,
    pub archived: bool,
    pub customer_name: Option<String>,
    pub goal: GoalSummary,
    pub repository_count: i64,
    pub verified_repository_count: i64,
    pub member_count: i64,
    pub last_activity_at: Option<DateTime<Utc>>,
}

/// Goal 摘要：**只表达两个事实**，不下「是否确认」的结论
/// （当前领域无 Goal 确认状态字段，I5-A §6.5 / 契约 §4.1）。
#[derive(Debug, Clone, Serialize)]
pub struct GoalSummary {
    /// 是否存在正式（published）修订。
    pub present: bool,
    pub revision_no: Option<i32>,
    pub published_at: Option<DateTime<Utc>>,
    pub change_summary: Option<String>,
    /// 是否存在待人工确认的 Goal 修改提案。
    pub pending_proposal: bool,
}

// ---------------------------------------------------------------------------
// §4.2 initialization_progress
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct InitializationProgress {
    pub items: Vec<InitItem>,
    /// 必需事项键集合（当前只有三项，见契约 §4.2 裁定）。
    pub required_items: Vec<String>,
    /// 当前**无法判定**初始化是否完成（无项目类型/初始化要求配置）→ 设计稿 §4.5。
    pub completion: String,
    /// 是否展开详细进度（契约 §4.2 收缩规则）。
    pub expanded: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InitItem {
    pub key: String,
    pub title: String,
    /// 展示层状态（`init_status::*`）。
    pub status: String,
    pub required: bool,
    /// 领域原始状态（保留，不被展示层状态覆盖）。
    pub source_state: Option<String>,
    pub detail: Option<String>,
}

// ---------------------------------------------------------------------------
// §4.3 work_summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct WorkSummary {
    pub task_total: i64,
    pub task_by_status: BTreeMap<String, i64>,
    pub active_run_count: i64,
    pub active_runs: Vec<ActiveRun>,
    pub recent_runs: Vec<RunBrief>,
    pub delivery: DeliverySummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeliverySummary {
    pub delivered: i64,
    pub partial: i64,
    pub missing: i64,
    pub no_contract: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActiveRun {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub task_title: String,
    pub run_kind: String,
    pub attempt_no: i32,
    pub status: String,
    pub stage: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    /// 执行主体：**权威来源是 `task.assignee_id`**（`task_run.agent_id` 恒 NULL，禁用）。
    pub agent: Option<AgentRef>,
    pub repositories: Vec<RunRepository>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentRef {
    pub principal_id: Uuid,
    pub display_name: String,
    /// `human` | `dev_agent` | `ai_pm` | `service` | `unknown`
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunRepository {
    pub repository_id: Uuid,
    pub base_ref: String,
    pub base_commit: String,
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunBrief {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub task_title: String,
    pub run_kind: String,
    pub attempt_no: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// §4.4 human_action_summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct HumanActionSummary {
    pub total: i64,
    pub items: Vec<ActionItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionItem {
    /// `finding_pending` | `finding_revision_requested` | `proposal_pending` | `delivery_pending`
    pub kind: String,
    /// 来源领域：`discovery` | `knowledge` | `delivery`
    pub domain: String,
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub detail: Option<String>,
    /// 来源领域原始状态（保留）。
    pub source_state: String,
    pub updated_at: Option<DateTime<Utc>>,
    pub action_hint: String,
    pub link: ActionLink,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionLink {
    pub kind: String,
    pub params: BTreeMap<String, String>,
}

// ---------------------------------------------------------------------------
// §4.5 risk_summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct RiskSummary {
    pub total: i64,
    pub items: Vec<RiskItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RiskItem {
    /// `run_failed` | `task_blocked` | `repository_unreachable` | `analysis_failed` | `archive_cleanup`
    pub kind: String,
    pub domain: String,
    pub source_id: String,
    pub project_id: Uuid,
    pub title: String,
    /// 来源领域原始状态。
    pub source_state: String,
    /// 展示层严重度：`high` | `medium` | `low`
    pub severity: String,
    pub occurred_at: Option<DateTime<Utc>>,
    pub suggestion: String,
    pub link: ActionLink,
}

// ---------------------------------------------------------------------------
// §4.6 recent_activity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct RecentActivity {
    pub total: i64,
    pub items: Vec<ActivityItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityItem {
    pub id: Uuid,
    pub activity_type: String,
    pub source_type: Option<String>,
    pub source_id: Option<Uuid>,
    pub occurred_at: DateTime<Utc>,
    pub actor: ActivityActor,
    pub summary: String,
    /// 去重键（`{activity_type}:{source_type}:{source_id}`）。
    pub dedup_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityActor {
    /// `human` | `agent` | `system`
    pub kind: String,
    pub id: Option<Uuid>,
    pub name: Option<String>,
}

// ---------------------------------------------------------------------------
// 入口
// ---------------------------------------------------------------------------

/// 载入项目总控台聚合（六块独立降级）。
pub async fn load_command_center(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    activity_limit: i64,
    activity_offset: i64,
) -> CommandCenter {
    let project_summary = match load_project_summary(pool, tenant_id, project_id).await {
        Ok(data) => ok_block(data),
        Err(err) => unavailable_block("project_summary", err),
    };
    let initialization_progress = match load_initialization_progress(pool, tenant_id, project_id).await
    {
        Ok(data) => ok_block(data),
        Err(err) => unavailable_block("initialization_progress", err),
    };
    let work_summary = match load_work_summary(pool, tenant_id, project_id).await {
        Ok(data) => ok_block(data),
        Err(err) => unavailable_block("work_summary", err),
    };
    let human_action_summary = match load_human_action_summary(pool, tenant_id, project_id).await {
        Ok(data) => ok_block(data),
        Err(err) => unavailable_block("human_action_summary", err),
    };
    let risk_summary = match load_risk_summary(pool, tenant_id, project_id).await {
        Ok(data) => ok_block(data),
        Err(err) => unavailable_block("risk_summary", err),
    };
    let recent_activity = match load_recent_activity(
        pool,
        tenant_id,
        project_id,
        activity_limit,
        activity_offset,
    )
    .await
    {
        Ok(data) => ok_block(data),
        Err(err) => unavailable_block("recent_activity", err),
    };

    CommandCenter {
        project_id,
        as_of: Utc::now(),
        project_summary,
        initialization_progress,
        work_summary,
        human_action_summary,
        risk_summary,
        recent_activity,
    }
}

// ---------------------------------------------------------------------------
// §4.1 实现
// ---------------------------------------------------------------------------

async fn load_project_summary(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<ProjectSummary, sqlx::Error> {
    let (code, name, lifecycle, customer_name): (String, String, String, Option<String>) =
        sqlx::query_as(
            "SELECT p.code, p.name, p.status, c.name \
             FROM core.project p LEFT JOIN core.customer c ON c.id = p.customer_id \
             WHERE p.id = $1 AND p.tenant_id = $2",
        )
        .bind(project_id)
        .bind(tenant_id)
        .fetch_one(pool)
        .await?;

    let goal_row: Option<(i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT r.revision_no, r.change_summary, r.created_at \
         FROM ai.project_document d \
         JOIN LATERAL ( \
              SELECT revision_no, change_summary, created_at \
                FROM ai.document_revision \
               WHERE document_id = d.id AND status = 'published' \
               ORDER BY revision_no DESC LIMIT 1 \
         ) r ON true \
         WHERE d.project_id = $1 AND d.tenant_id = $2 AND d.doc_key = 'project-goal'",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?;

    let goal_pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.document_proposal \
         WHERE project_id = $1 AND tenant_id = $2 AND doc_key = 'project-goal' \
           AND status = ANY($3)",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(&ACTIVE_PROPOSAL_STATUSES)
    .fetch_one(pool)
    .await?;

    let (repository_count, verified_repository_count): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE status = 'verified') \
         FROM integration.project_repository \
         WHERE project_id = $1 AND tenant_id = $2",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;

    let member_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.project_member WHERE project_id = $1 AND tenant_id = $2",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;

    let last_activity_at: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT max(created_at) FROM governance.audit_log \
         WHERE tenant_id = $1 AND project_id = $2 AND action = ANY($3)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(activity_audit_actions())
    .fetch_one(pool)
    .await?;

    Ok(ProjectSummary {
        project_id,
        code,
        name,
        archived: lifecycle == "archived",
        lifecycle,
        customer_name,
        goal: GoalSummary {
            present: goal_row.is_some(),
            revision_no: goal_row.as_ref().map(|r| r.0),
            published_at: goal_row.as_ref().map(|r| r.2),
            change_summary: goal_row.as_ref().and_then(|r| r.1.clone()),
            pending_proposal: goal_pending > 0,
        },
        repository_count,
        verified_repository_count,
        member_count,
        last_activity_at,
    })
}

// ---------------------------------------------------------------------------
// §4.2 实现
// ---------------------------------------------------------------------------

async fn load_initialization_progress(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<InitializationProgress, sqlx::Error> {
    // 仓库接入
    let repo_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, count(*) FROM integration.project_repository \
         WHERE project_id = $1 AND tenant_id = $2 GROUP BY status",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let repo_total: i64 = repo_rows.iter().map(|(_, n)| n).sum();
    let repo_verified: i64 = repo_rows
        .iter()
        .find(|(s, _)| s == "verified")
        .map(|(_, n)| *n)
        .unwrap_or(0);
    let repo_unreachable: i64 = repo_rows
        .iter()
        .find(|(s, _)| s == "unreachable")
        .map(|(_, n)| *n)
        .unwrap_or(0);
    let repo_pending: i64 = repo_rows
        .iter()
        .find(|(s, _)| s == "pending")
        .map(|(_, n)| *n)
        .unwrap_or(0);

    // 代码分析（导入作业）
    let import_rows: Vec<(String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT status, error_code, created_at FROM integration.repo_import \
         WHERE project_id = $1 AND tenant_id = $2 ORDER BY created_at DESC, id DESC LIMIT 20",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let latest_import = import_rows.first();

    // 项目认知（Discovery 会话）
    let session_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, count(*) FROM discovery.discovery_session \
         WHERE project_id = $1 AND tenant_id = $2 GROUP BY status",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let session_total: i64 = session_rows.iter().map(|(_, n)| n).sum();
    // 开/待人审会话的口径收敛在 init_judgement::cognition_status（G-01 七态）。

    // 开发环境（绑定设备的就绪快照）
    let device_rows: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT d.readiness, count(*) \
         FROM devenv.device_project_binding b \
         JOIN devenv.device d ON d.id = b.device_id \
         WHERE b.project_id = $1 AND b.tenant_id = $2 AND b.unbound_at IS NULL \
         GROUP BY d.readiness",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let device_total: i64 = device_rows.iter().map(|(_, n)| n).sum();
    let device_ready: i64 = device_rows
        .iter()
        .find(|(r, _)| r.as_deref() == Some("ready"))
        .map(|(_, n)| *n)
        .unwrap_or(0);
    let device_degraded: i64 = device_rows
        .iter()
        .filter(|(r, _)| matches!(r.as_deref(), Some("degraded") | Some("failed")))
        .map(|(_, n)| n)
        .sum();
    let device_unchecked: i64 = device_rows
        .iter()
        .find(|(r, _)| r.is_none())
        .map(|(_, n)| *n)
        .unwrap_or(0);

    // 人工处理待办数（用于「人工处理」事项与展开判定）
    let action_total = pending_action_total(pool, tenant_id, project_id).await?;

    // Goal
    let goal_present: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.project_document d \
         WHERE d.project_id = $1 AND d.tenant_id = $2 AND d.doc_key = 'project-goal' \
           AND EXISTS (SELECT 1 FROM ai.document_revision r \
                        WHERE r.document_id = d.id AND r.status = 'published')",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    // G-01 七态：goal 的活跃提案按 status 拆分——只有 `redline_pending`
    // 是「待人工红线确认」（系统已交出球）；draft / commit_pending 等由系统推进。
    // （`quality_gates_passed` / `confirmed` 现无写入路径，不计。）
    let goal_proposal_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, count(*) FROM ai.document_proposal \
         WHERE project_id = $1 AND tenant_id = $2 AND doc_key = 'project-goal' \
           AND status = ANY($3) GROUP BY status",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(&ACTIVE_PROPOSAL_STATUSES)
    .fetch_all(pool)
    .await?;
    let goal_human_pending: i64 = goal_proposal_rows
        .iter()
        .filter(|(status, _)| status == "redline_pending")
        .map(|(_, n)| n)
        .sum();
    let goal_pending: i64 = goal_proposal_rows.iter().map(|(_, n)| n).sum();

    // 认知：待人工确认的发现数——finding 的核实决定者只能是人类
    // （set_finding_verification 强制可信人类身份），故 PENDING 即人类待办。
    let finding_pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM discovery.discovery_finding \
         WHERE project_id = $1 AND tenant_id = $2 AND verification_status = 'PENDING'",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;

    let mut items: Vec<InitItem> = Vec::new();
    items.push(InitItem {
        key: "project_basics".to_string(),
        title: "项目基本信息".to_string(),
        status: init_status::COMPLETED.to_string(),
        required: true,
        source_state: Some("planning".to_string()),
        detail: None,
    });
    items.push(InitItem {
        key: "project_goal".to_string(),
        title: "项目目标".to_string(),
        status: init_judgement::goal_status(
            goal_present > 0,
            goal_human_pending > 0,
            goal_pending > 0,
        )
        .to_string(),
        required: true,
        source_state: None,
        detail: None,
    });
    items.push(InitItem {
        key: "repository_access".to_string(),
        title: "仓库接入".to_string(),
        status: init_judgement::repository_access_status(
            repo_total,
            repo_unreachable,
            repo_pending,
        )
        .to_string(),
        required: false,
        source_state: None,
        detail: Some(format!(
            "已关联 {repo_total} 个仓库，其中 {repo_verified} 个已验证"
        )),
    });
    items.push(InitItem {
        key: "code_analysis".to_string(),
        title: "代码分析".to_string(),
        status: init_judgement::code_analysis_status(
            repo_total,
            latest_import.map(|(s, _, _)| s.as_str()),
        )
        .to_string(),
        required: false,
        source_state: latest_import.map(|(s, _, _)| s.clone()),
        detail: latest_import.and_then(|(_, code, _)| code.clone()),
    });
    items.push(InitItem {
        key: "project_cognition".to_string(),
        title: "项目认知".to_string(),
        status: init_judgement::cognition_status(&session_rows, finding_pending).to_string(),
        required: false,
        source_state: None,
        detail: Some(format!("共 {session_total} 个发现会话")),
    });
    items.push(InitItem {
        key: "dev_environment".to_string(),
        title: "开发环境".to_string(),
        status: init_judgement::environment_status(
            device_total,
            device_ready,
            device_degraded,
            device_unchecked,
        )
        .to_string(),
        required: false,
        source_state: None,
        detail: Some(format!("已绑定 {device_total} 台设备")),
    });
    items.push(InitItem {
        key: "human_actions".to_string(),
        title: "人工处理".to_string(),
        status: init_judgement::human_actions_status(action_total).to_string(),
        required: true,
        source_state: None,
        detail: Some(format!("{action_total} 项待人工处理")),
    });

    let required_items = items
        .iter()
        .filter(|item| item.required)
        .map(|item| item.key.clone())
        .collect::<Vec<_>>();
    // 收缩规则（设计稿 §4.6）：进行中/未开始的必需事项或任一阻塞 → 展开。
    // G-01 七态：waiting_for_human 走**自有展开分支**（无条件展开）——
    // 球在人手里，收缩不得隐藏人工待办（I5 §4.6「收缩不能隐藏新的阻塞或
    // 人工处理事项」）；刻意不并入 in_progress 的 required 限定分支，
    // 两态展示语义不同（系统在工作 vs 等人行动）。
    let expanded = items.iter().any(|item| {
        item.status == init_status::BLOCKED
            || item.status == init_status::WAITING_FOR_HUMAN
            || (item.required
                && matches!(
                    item.status.as_str(),
                    init_status::IN_PROGRESS | init_status::NOT_STARTED
                ))
    });

    Ok(InitializationProgress {
        items,
        required_items,
        // 当前没有项目类型/初始化要求配置 → 不得宣称「初始化已完成」。
        completion: "not_determinable".to_string(),
        expanded,
    })
}

// ---------------------------------------------------------------------------
// §4.3 实现
// ---------------------------------------------------------------------------

async fn load_work_summary(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<WorkSummary, sqlx::Error> {
    let task_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, count(*) FROM delivery.task \
         WHERE project_id = $1 AND tenant_id = $2 GROUP BY status",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let task_by_status: BTreeMap<String, i64> = task_rows.into_iter().collect();
    let task_total: i64 = task_by_status.values().sum();

    let active_runs = load_runs(pool, tenant_id, project_id, true, 20).await?;
    let active_run_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run \
         WHERE project_id = $1 AND tenant_id = $2 \
           AND status IN ('queued','claimed','running')",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    let recent_runs = load_recent_run_briefs(pool, tenant_id, project_id, 5).await?;

    let delivery_rows = crate::delivery::list_project_delivery(pool, tenant_id, project_id).await?;
    let mut delivery = DeliverySummary {
        delivered: 0,
        partial: 0,
        missing: 0,
        no_contract: 0,
    };
    for item in &delivery_rows {
        match item.delivery_state.as_str() {
            "delivered" => delivery.delivered += 1,
            "partial" => delivery.partial += 1,
            "missing" => delivery.missing += 1,
            _ => delivery.no_contract += 1,
        }
    }

    Ok(WorkSummary {
        task_total,
        task_by_status,
        active_run_count,
        active_runs,
        recent_runs,
        delivery,
    })
}

async fn load_runs(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    only_active: bool,
    limit: i64,
) -> Result<Vec<ActiveRun>, sqlx::Error> {
    let rows: Vec<(
        Uuid,
        Uuid,
        String,
        String,
        i32,
        String,
        Option<String>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<Uuid>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT r.id, r.task_id, t.title, r.run_kind, r.attempt_no, r.status, r.stage, \
                r.claimed_at, r.started_at, r.heartbeat_at, r.lease_expires_at, \
                t.assignee_id, p.display_name, p.kind \
         FROM delivery.task_run r \
         JOIN delivery.task t ON t.id = r.task_id \
         LEFT JOIN core.principal p ON p.id = t.assignee_id \
         WHERE r.project_id = $1 AND r.tenant_id = $2 \
           AND ($3 = false OR r.status IN ('queued','claimed','running')) \
         ORDER BY r.created_at DESC, r.id DESC \
         LIMIT $4",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(only_active)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let run_ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
    let mut repos: BTreeMap<Uuid, Vec<RunRepository>> = BTreeMap::new();
    if !run_ids.is_empty() {
        let repo_rows: Vec<(Uuid, Uuid, String, String, String)> = sqlx::query_as(
            "SELECT run_id, repository_id, base_ref, base_commit, workspace_path \
             FROM delivery.task_run_repository \
             WHERE run_id = ANY($1) ORDER BY run_id, repository_id",
        )
        .bind(&run_ids)
        .fetch_all(pool)
        .await?;
        for (run_id, repository_id, base_ref, base_commit, workspace_path) in repo_rows {
            repos.entry(run_id).or_default().push(RunRepository {
                repository_id,
                base_ref,
                base_commit,
                workspace_path,
            });
        }
    }

    Ok(rows
        .into_iter()
        .map(
            |(
                run_id,
                task_id,
                task_title,
                run_kind,
                attempt_no,
                status,
                stage,
                claimed_at,
                started_at,
                heartbeat_at,
                lease_expires_at,
                assignee_id,
                display_name,
                kind,
            )| ActiveRun {
                run_id,
                task_id,
                task_title,
                run_kind,
                attempt_no,
                status,
                stage,
                claimed_at,
                started_at,
                heartbeat_at,
                lease_expires_at,
                agent: assignee_id.map(|principal_id| AgentRef {
                    principal_id,
                    display_name: display_name.unwrap_or_else(|| "未知执行主体".to_string()),
                    kind: kind.unwrap_or_else(|| "unknown".to_string()),
                }),
                repositories: repos.get(&run_id).cloned().unwrap_or_default(),
            },
        )
        .collect())
}

async fn load_recent_run_briefs(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    limit: i64,
) -> Result<Vec<RunBrief>, sqlx::Error> {
    let rows: Vec<(
        Uuid,
        Uuid,
        String,
        String,
        i32,
        String,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        "SELECT r.id, r.task_id, t.title, r.run_kind, r.attempt_no, r.status, r.created_at, r.finished_at \
         FROM delivery.task_run r \
         JOIN delivery.task t ON t.id = r.task_id \
         WHERE r.project_id = $1 AND r.tenant_id = $2 \
         ORDER BY r.created_at DESC, r.id DESC LIMIT $3",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(run_id, task_id, task_title, run_kind, attempt_no, status, created_at, finished_at)| {
                RunBrief {
                    run_id,
                    task_id,
                    task_title,
                    run_kind,
                    attempt_no,
                    status,
                    created_at,
                    finished_at,
                }
            },
        )
        .collect())
}

// ---------------------------------------------------------------------------
// §4.4 实现
// ---------------------------------------------------------------------------

async fn pending_action_total(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let findings: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM discovery.discovery_finding \
         WHERE project_id = $1 AND tenant_id = $2 AND verification_status = 'PENDING'",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    let proposals: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.document_proposal \
         WHERE project_id = $1 AND tenant_id = $2 AND status = ANY($3)",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(&ACTIVE_PROPOSAL_STATUSES)
    .fetch_one(pool)
    .await?;
    let deliveries: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task \
         WHERE project_id = $1 AND tenant_id = $2 AND status IN ('submitted','reviewing')",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    Ok(findings + proposals + deliveries)
}

async fn load_human_action_summary(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<HumanActionSummary, sqlx::Error> {
    let mut items: Vec<ActionItem> = Vec::new();

    // 待确认 / 待补充 Finding（I2 权威推导：display_state）
    let finding_rows: Vec<(Uuid, String, String, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, kind, content, verification_status, created_at \
         FROM discovery.discovery_finding \
         WHERE project_id = $1 AND tenant_id = $2 AND verification_status = 'PENDING' \
         ORDER BY created_at DESC, id DESC LIMIT 50",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let finding_ids: Vec<Uuid> = finding_rows.iter().map(|r| r.0).collect();
    let decisions =
        crate::discovery::latest_decision_actions(pool, project_id, &finding_ids).await?;
    for (id, kind, content, status, created_at) in finding_rows {
        let display_state =
            crate::discovery::derive_display_state(&status, decisions.get(&id).map(|s| s.as_str()));
        let (action_kind, hint) = if display_state == "REVISION_REQUESTED" {
            ("finding_revision_requested", "补充信息后重新提交")
        } else {
            ("finding_pending", "确认或拒绝该发现")
        };
        items.push(ActionItem {
            kind: action_kind.to_string(),
            domain: "discovery".to_string(),
            id,
            project_id,
            title: format!("发现待处理（{kind}）"),
            detail: Some(truncate(&content, 160)),
            source_state: display_state,
            updated_at: Some(created_at),
            action_hint: hint.to_string(),
            link: ActionLink {
                kind: "discovery_finding".to_string(),
                params: BTreeMap::from([("finding_id".to_string(), id.to_string())]),
            },
        });
    }

    // 待审批知识提案
    let proposal_rows: Vec<(Uuid, String, Option<String>, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, doc_key, change_summary, status, created_at \
         FROM ai.document_proposal \
         WHERE project_id = $1 AND tenant_id = $2 AND status = ANY($3) \
         ORDER BY created_at DESC, id DESC LIMIT 50",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(&ACTIVE_PROPOSAL_STATUSES)
    .fetch_all(pool)
    .await?;
    for (id, doc_key, change_summary, status, created_at) in proposal_rows {
        items.push(ActionItem {
            kind: "proposal_pending".to_string(),
            domain: "knowledge".to_string(),
            id,
            project_id,
            title: format!("文档提案待确认（{doc_key}）"),
            detail: change_summary.map(|s| truncate(&s, 160)),
            source_state: status,
            updated_at: Some(created_at),
            action_hint: "确认或否决该提案".to_string(),
            link: ActionLink {
                kind: "document_proposal".to_string(),
                params: BTreeMap::from([("proposal_id".to_string(), id.to_string())]),
            },
        });
    }

    // 待验收 Delivery
    let delivery_rows: Vec<(Uuid, String, String, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT id, title, status, updated_at FROM delivery.task \
         WHERE project_id = $1 AND tenant_id = $2 AND status IN ('submitted','reviewing') \
         ORDER BY updated_at DESC NULLS LAST, id DESC LIMIT 50",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    for (id, title, status, updated_at) in delivery_rows {
        items.push(ActionItem {
            kind: "delivery_pending".to_string(),
            domain: "delivery".to_string(),
            id,
            project_id,
            title: format!("交付待验收（{title}）"),
            detail: None,
            source_state: status,
            updated_at,
            action_hint: "验收交付证据".to_string(),
            link: ActionLink {
                kind: "task_delivery".to_string(),
                params: BTreeMap::from([("task_id".to_string(), id.to_string())]),
            },
        });
    }

    // 排序：kind 优先级 → 更新时间 → id（唯一 tiebreaker，保证稳定）。
    items.sort_by(|a, b| {
        action_kind_rank(&a.kind)
            .cmp(&action_kind_rank(&b.kind))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    let total = items.len() as i64;
    Ok(HumanActionSummary { total, items })
}

fn action_kind_rank(kind: &str) -> u8 {
    match kind {
        "finding_pending" => 0,
        "finding_revision_requested" => 1,
        "proposal_pending" => 2,
        "delivery_pending" => 3,
        _ => 9,
    }
}

// ---------------------------------------------------------------------------
// §4.5 实现
// ---------------------------------------------------------------------------

async fn load_risk_summary(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<RiskSummary, sqlx::Error> {
    let mut items: Vec<RiskItem> = Vec::new();

    // 失败 / 超时的 Run
    let failed_runs: Vec<(Uuid, Uuid, String, String, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT r.id, r.task_id, t.title, r.status, r.finished_at \
         FROM delivery.task_run r JOIN delivery.task t ON t.id = r.task_id \
         WHERE r.project_id = $1 AND r.tenant_id = $2 AND r.status IN ('failed','timed_out') \
         ORDER BY r.created_at DESC, r.id DESC LIMIT 50",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    for (run_id, _task_id, title, status, finished_at) in failed_runs {
        items.push(RiskItem {
            kind: "run_failed".to_string(),
            domain: "run".to_string(),
            source_id: run_id.to_string(),
            project_id,
            title: format!("执行失败（{title}）"),
            source_state: status,
            severity: "high".to_string(),
            occurred_at: finished_at,
            suggestion: "查看 Run 日志并决定重跑".to_string(),
            link: ActionLink {
                kind: "task_run".to_string(),
                params: BTreeMap::from([("run_id".to_string(), run_id.to_string())]),
            },
        });
    }

    // 阻塞中的 Task
    let blocked_tasks: Vec<(Uuid, String, String, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT id, title, status, updated_at FROM delivery.task \
         WHERE project_id = $1 AND tenant_id = $2 AND status = 'blocked' \
         ORDER BY updated_at DESC NULLS LAST, id DESC LIMIT 50",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    for (id, title, status, updated_at) in blocked_tasks {
        items.push(RiskItem {
            kind: "task_blocked".to_string(),
            domain: "task".to_string(),
            source_id: id.to_string(),
            project_id,
            title: format!("任务阻塞（{title}）"),
            source_state: status,
            severity: "medium".to_string(),
            occurred_at: updated_at,
            suggestion: "查看阻塞原因并解除".to_string(),
            link: ActionLink {
                kind: "task".to_string(),
                params: BTreeMap::from([("task_id".to_string(), id.to_string())]),
            },
        });
    }

    // 不可达仓库
    let repos: Vec<(Uuid, String, Option<String>, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT id, name, last_error_code, last_verified_at \
         FROM integration.project_repository \
         WHERE project_id = $1 AND tenant_id = $2 AND status = 'unreachable' \
         ORDER BY name, id LIMIT 50",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    for (id, name, error_code, verified_at) in repos {
        let severity = if error_code.as_deref() == Some("auth_failed") {
            "high"
        } else {
            "medium"
        };
        items.push(RiskItem {
            kind: "repository_unreachable".to_string(),
            domain: "repository".to_string(),
            source_id: id.to_string(),
            project_id,
            title: format!("仓库不可达（{name}）"),
            source_state: error_code.unwrap_or_else(|| "unreachable".to_string()),
            severity: severity.to_string(),
            occurred_at: verified_at,
            suggestion: "检查 SSH 密钥与仓库地址后重新校验".to_string(),
            link: ActionLink {
                kind: "repository".to_string(),
                params: BTreeMap::from([("repository_id".to_string(), id.to_string())]),
            },
        });
    }

    // 分析失败（以最新导入作业为准：失败且无更新的成功）
    let latest_import: Option<(Uuid, String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, status, error_code, created_at FROM integration.repo_import \
         WHERE project_id = $1 AND tenant_id = $2 ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?;
    if let Some((import_id, status, error_code, created_at)) = latest_import {
        if status == "failed" || status == "rejected" {
            items.push(RiskItem {
                kind: "analysis_failed".to_string(),
                domain: "import".to_string(),
                source_id: import_id.to_string(),
                project_id,
                title: "代码分析失败".to_string(),
                source_state: status,
                severity: "high".to_string(),
                occurred_at: Some(created_at),
                suggestion: error_code
                    .unwrap_or_else(|| "查看导入作业详情".to_string()),
                link: ActionLink {
                    kind: "repo_import".to_string(),
                    params: BTreeMap::from([("import_id".to_string(), import_id.to_string())]),
                },
            });
        }
    }

    // 归档后残留活跃 Run（I1 P3 事实）
    let archived: bool = sqlx::query_scalar(
        "SELECT status = 'archived' FROM core.project WHERE id = $1 AND tenant_id = $2",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    if archived {
        let residual: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM delivery.task_run \
             WHERE project_id = $1 AND tenant_id = $2 AND status IN ('claimed','running')",
        )
        .bind(project_id)
        .bind(tenant_id)
        .fetch_one(pool)
        .await?;
        if residual > 0 {
            items.push(RiskItem {
                kind: "archive_cleanup".to_string(),
                domain: "project".to_string(),
                source_id: project_id.to_string(),
                project_id,
                title: "归档后仍有执行中的 Run".to_string(),
                source_state: "archived".to_string(),
                severity: "medium".to_string(),
                occurred_at: None,
                suggestion: "终止残留进程并确认清理完成".to_string(),
                link: ActionLink {
                    kind: "project".to_string(),
                    params: BTreeMap::from([("project_id".to_string(), project_id.to_string())]),
                },
            });
        }
    }

    let total = items.len() as i64;
    Ok(RiskSummary { total, items })
}

// ---------------------------------------------------------------------------
// §4.6 实现
// ---------------------------------------------------------------------------

async fn load_recent_activity(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<RecentActivity, sqlx::Error> {
    // 去重键 = `{activity_type}:{target_type}:{target_id}`——**必须在 LIMIT 之前**完成，
    // 否则一页里可能出现重复项（少行），且同一条目会跨页重复出现（I5-030/I5-031）。
    // 因此去重下推到 SQL（`DISTINCT ON`），再排序分页；排序键 `created_at DESC, id DESC`
    // 保证并列时间下的稳定次序。
    let case_sql = ACTIVITY_TYPE_BY_ACTION
        .iter()
        .map(|(action, activity_type)| format!("WHEN '{action}' THEN '{activity_type}'"))
        .collect::<Vec<_>>()
        .join(" ");
    let deduped = format!(
        "SELECT DISTINCT ON (activity_type, target_type, target_id) \
                id, activity_type, target_type, target_id, actor_id, created_at \
           FROM ( \
                SELECT a.id, a.action, a.target_type, a.target_id, a.actor_id, a.created_at, \
                       CASE a.action {case_sql} END AS activity_type \
                  FROM governance.audit_log a \
                 WHERE a.tenant_id = $1 AND a.project_id = $2 AND a.action = ANY($3) \
           ) s \
          ORDER BY activity_type, target_type, target_id, created_at DESC, id DESC"
    );

    let total: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM ({deduped}) d"))
        .bind(tenant_id)
        .bind(project_id)
        .bind(activity_audit_actions())
        .fetch_one(pool)
        .await?;

    let rows: Vec<(
        Uuid,
        String,
        Option<String>,
        Option<Uuid>,
        Option<Uuid>,
        DateTime<Utc>,
    )> = sqlx::query_as(&format!(
        "SELECT d.id, d.activity_type, d.target_type, d.target_id, d.actor_id, d.created_at \
           FROM ({deduped}) d \
          ORDER BY d.created_at DESC, d.id DESC \
          LIMIT $4 OFFSET $5"
    ))
    .bind(tenant_id)
    .bind(project_id)
    .bind(activity_audit_actions())
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    // 执行主体名称（actor）单独批量取，避免与去重子查询耦合。
    let actor_ids: Vec<Uuid> = rows.iter().filter_map(|row| row.4).collect();
    let mut actors: std::collections::HashMap<Uuid, (Option<String>, Option<String>)> =
        std::collections::HashMap::new();
    if !actor_ids.is_empty() {
        let actor_rows: Vec<(Uuid, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT id, display_name, kind FROM core.principal WHERE id = ANY($1)",
        )
        .bind(&actor_ids)
        .fetch_all(pool)
        .await?;
        for (id, name, kind) in actor_rows {
            actors.insert(id, (name, kind));
        }
    }

    let items = rows
        .into_iter()
        .map(|(id, activity_type, target_type, target_id, actor_id, created_at)| {
            let (actor_name, actor_kind) = actor_id
                .and_then(|actor| actors.get(&actor).cloned())
                .unwrap_or((None, None));
            let dedup_key = format!(
                "{activity_type}:{}:{}",
                target_type.clone().unwrap_or_default(),
                target_id.map(|v| v.to_string()).unwrap_or_default()
            );
            ActivityItem {
                id,
                summary: activity_summary(&activity_type),
                activity_type,
                source_type: target_type,
                source_id: target_id,
                occurred_at: created_at,
                actor: ActivityActor {
                    kind: match actor_kind.as_deref() {
                        Some("human") => "human".to_string(),
                        Some("dev_agent") | Some("ai_pm") | Some("service") => "agent".to_string(),
                        _ => "system".to_string(),
                    },
                    id: actor_id,
                    name: actor_name,
                },
                dedup_key,
            }
        })
        .collect();

    Ok(RecentActivity { total, items })
}

/// 审计 action → 归一化活动类型（白名单映射，契约 §4.7；与 SQL `CASE` 同源）。
pub fn activity_type_for(action: &str) -> Option<&'static str> {
    ACTIVITY_TYPE_BY_ACTION
        .iter()
        .find(|(candidate, _)| *candidate == action)
        .map(|(_, activity_type)| *activity_type)
}

fn activity_summary(activity_type: &str) -> String {
    match activity_type {
        "project_archived" => "项目已归档".to_string(),
        "goal_committed" => "项目目标已发布新版本".to_string(),
        "finding_decided" => "发现已由人工裁决".to_string(),
        "finding_revision_requested" => "发现被要求补充信息".to_string(),
        "repository_created" => "关联了新的仓库".to_string(),
        "repository_verified" => "仓库连通性已校验".to_string(),
        "analysis_succeeded" => "代码分析完成".to_string(),
        "analysis_failed" => "代码分析失败".to_string(),
        "run_started" => "执行已启动".to_string(),
        "run_finished" => "执行已结束".to_string(),
        "run_failed" => "执行启动失败".to_string(),
        "run_terminated" => "执行已终止".to_string(),
        "delivery_submitted" => "交付证据已提交".to_string(),
        "delivery_rejected" => "交付被驳回".to_string(),
        "review_completed" => "评审已完成".to_string(),
        "review_changes_requested" => "评审要求修改".to_string(),
        "member_changed" => "项目成员变更".to_string(),
        "device_bound" => "设备已绑定项目".to_string(),
        _ => "项目活动".to_string(),
    }
}

fn truncate(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    let mut out: String = trimmed.chars().take(max).collect();
    out.push('…');
    out
}
