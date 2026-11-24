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
    ("project.closed", "project_closed"),
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
    pub fn repository_access_status(total: i64, unreachable: i64, pending: i64) -> &'static str {
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
    pub fn cognition_status(sessions: &[(String, i64)], pending_findings: i64) -> &'static str {
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

    /// 项目基线：已确认版本 → 完成；存在待确认提案（pending，人工确认门）
    /// → 等人；无提案且未确认 → 未开始。基线确认只能由人类执行
    /// （ai.project_baseline.confirmed_by NOT NULL，结构强制）。
    pub fn baseline_status(confirmed_version: bool, pending_proposal: bool) -> &'static str {
        if confirmed_version {
            return init_status::COMPLETED;
        }
        if pending_proposal {
            return init_status::WAITING_FOR_HUMAN;
        }
        init_status::NOT_STARTED
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
    /// `core.project.status` 原样透传（`active`/`closed`）。
    pub lifecycle: String,
    pub closed: bool,
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
    /// 需求进度（§4.3）：active 需求总数与其中当前修订已 approved（真人定稿）数。
    pub requirement_total: i64,
    pub requirement_approved: i64,
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
    /// `finding_pending` | `finding_revision_requested` | `report_pending` |
    /// `proposal_pending` | `delivery_pending`
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
    let initialization_progress =
        match load_initialization_progress(pool, tenant_id, project_id).await {
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
    let recent_activity =
        match load_recent_activity(pool, tenant_id, project_id, activity_limit, activity_offset)
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

fn truncate(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    let mut out: String = trimmed.chars().take(max).collect();
    out.push('…');
    out
}

mod action;
mod activity;
mod project;
mod risk;
mod work;

use action::{load_human_action_summary, pending_action_total};
use activity::load_recent_activity;

pub use activity::activity_type_for;
use project::{load_initialization_progress, load_project_summary};
use risk::load_risk_summary;
use work::load_work_summary;
