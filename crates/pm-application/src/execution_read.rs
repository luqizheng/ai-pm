//! S05 · 执行域读模型（需要人工处理 / Run 详情附属事实 / 协作图 / 名册）。
//!
//! S05 后台原有的是**写路径与单 Run 读面**（task-runs 列表、run 详情、
//! 事件账），缺的是 S05 四页要用的**聚合与附属事实**：
//!
//! - `project_waiting`：**需要人工处理**队列——活跃等待（waiting reason
//!   五值）+ 待答交互（question 原文 / 选项 / 截止）+ 待裁决审批。
//! - `project_execution_overview`：Execution Center 页头聚合（活跃 Run /
//!   待人工 / 待评审 / 失败 / 设备就绪 / 成功率 / 平均时长）。
//! - `project_run_events`：项目级执行活动流（`delivery.task_run_event`）。
//! - `list_run_authorizations`：ACP-07 Deadline 重授权记录（Run 级）。
//! - `list_run_commands`：Worker 命令台账（issued→delivered→acked/failed）。
//! - `list_run_continuations`：Continuation 链（open/started/consumed/…）。
//! - `list_run_commits`：Run 级逐仓交付清单（commit / 变更文件数 / 测试证据）。
//! - `task_collaboration`：多 Agent 协作图（子任务节点 + 依赖边 + 阻塞链）。
//! - `project_agents`：Agent 名册（执行者 + 设备就绪链）。
//!
//! 领域纪律：
//! - **Task ≠ Run ≠ Agent ≠ Delivery ≠ Review**：本模块按各自实体读，不
//!   把「Run 成功」当「交付已验收」。
//! - **任务级一次性授权 ≠ ACP-07 Run 级 Deadline 重授权**：后者是 Run 级
//!   事实，只从 `task_run_execution_authorization` 读。
//! - **waiting reason 五值**：needs_info / needs_decision / needs_approval /
//!   external_dependency / no_executor_available（库级 CHECK）。

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

// ───────────────────────────────────────────── 需要人工处理（S05-01 / S05-04）

/// 等待行（delivery.task_waiting，活跃）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct WaitingRow {
    pub id: Uuid,
    pub task_id: Uuid,
    pub task_title: Option<String>,
    pub task_status: Option<String>,
    pub task_priority: Option<String>,
    /// 等待原因（五值之一）。
    pub reason: String,
    /// 待澄清问题（jsonb 数组）。
    pub questions: Value,
    pub requested_from: Option<Uuid>,
    pub since: DateTime<Utc>,
    pub deadline: Option<DateTime<Utc>>,
}

/// 待答交互行（delivery.task_interaction，pending）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct InteractionRow {
    pub id: Uuid,
    pub task_id: Uuid,
    pub task_title: Option<String>,
    pub run_id: Uuid,
    pub reason: String,
    pub title: String,
    /// 问题原文（人工答复的对象）。
    pub question: String,
    pub options: Value,
    pub required_response: bool,
    pub deadline_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// 待裁决审批行（governance.review_approval + design.design_approval）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct PendingApprovalRow {
    pub id: Uuid,
    /// `review` / `design`。
    pub source: String,
    pub kind: String,
    pub task_id: Option<Uuid>,
    pub task_title: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// 需要人工处理队列。
#[derive(Debug, Serialize)]
pub struct HumanAttentionQueue {
    pub waiting: Vec<WaitingRow>,
    pub interactions: Vec<InteractionRow>,
    pub approvals: Vec<PendingApprovalRow>,
}

/// 项目级「需要人工处理」队列。
pub async fn project_waiting(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<HumanAttentionQueue, crate::pm_write::PmWriteError> {
    use crate::pm_write::PmWriteError;

    let waiting: Vec<WaitingRow> = sqlx::query_as(
        "SELECT w.id, w.task_id, t.title AS task_title, t.status AS task_status, \
                t.priority AS task_priority, w.reason, w.questions, w.requested_from, \
                w.since, w.deadline \
         FROM delivery.task_waiting w \
         LEFT JOIN delivery.task t ON t.id = w.task_id \
         WHERE w.tenant_id = $1 AND w.project_id = $2 AND w.ended_at IS NULL \
         ORDER BY w.since",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let interactions: Vec<InteractionRow> = sqlx::query_as(
        "SELECT i.id, i.task_id, t.title AS task_title, i.run_id, i.reason, i.title, \
                i.question, i.options, i.required_response, i.deadline_at, i.created_at \
         FROM delivery.task_interaction i \
         LEFT JOIN delivery.task t ON t.id = i.task_id \
         WHERE i.tenant_id = $1 AND i.project_id = $2 AND i.status = 'pending' \
         ORDER BY i.created_at",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let approvals: Vec<PendingApprovalRow> = sqlx::query_as(
        "SELECT a.id, 'review' AS source, a.kind, a.task_id, t.title AS task_title, \
                a.status, a.created_at \
         FROM governance.review_approval a \
         LEFT JOIN delivery.task t ON t.id = a.task_id \
         WHERE a.tenant_id = $1 AND a.project_id = $2 AND a.status = 'pending' \
         UNION ALL \
         SELECT a.id, 'design' AS source, a.kind, NULL::uuid AS task_id, \
                NULL::text AS task_title, a.status, a.created_at \
         FROM design.design_approval a \
         WHERE a.tenant_id = $1 AND a.project_id = $2 AND a.status = 'pending' \
         ORDER BY created_at",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    Ok(HumanAttentionQueue {
        waiting,
        interactions,
        approvals,
    })
}

// ───────────────────────────────────────────── Execution Center 页头聚合（S05-01）

/// 执行态势概览。
#[derive(Debug, Serialize)]
pub struct ExecutionOverview {
    pub active_run_count: i64,
    pub waiting_count: i64,
    pub pending_interaction_count: i64,
    pub pending_approval_count: i64,
    pub failed_run_count: i64,
    pub awaiting_review_count: i64,
    pub total_run_count: i64,
    pub succeeded_run_count: i64,
    /// 成功率（0–1；分母为 0 时为 null）。
    pub success_rate: Option<f64>,
    /// 平均执行时长（秒；分母为 0 时为 null）。
    pub average_duration_seconds: Option<f64>,
    pub device_ready_count: i64,
    pub device_total_count: i64,
    /// 按等待原因分桶（五值）。
    pub waiting_by_reason: Vec<(String, i64)>,
}

/// 项目级执行态势概览。
pub async fn project_execution_overview(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<ExecutionOverview, crate::pm_write::PmWriteError> {
    use crate::pm_write::PmWriteError;

    let run_counts: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE status IN ('claimed','running')), \
                count(*) FILTER (WHERE status = 'failed'), \
                count(*) FILTER (WHERE status = 'succeeded'), \
                count(*) \
         FROM delivery.task_run WHERE tenant_id = $1 AND project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let waiting_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_waiting \
         WHERE tenant_id = $1 AND project_id = $2 AND ended_at IS NULL",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let pending_interaction_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_interaction \
         WHERE tenant_id = $1 AND project_id = $2 AND status = 'pending'",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let pending_approval_count: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM governance.review_approval \
                 WHERE tenant_id = $1 AND project_id = $2 AND status = 'pending') \
              + (SELECT count(*) FROM design.design_approval \
                 WHERE tenant_id = $1 AND project_id = $2 AND status = 'pending')",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;

    // 待评审 = 已提交但尚未裁决的交付 Run。
    let awaiting_review_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run \
         WHERE tenant_id = $1 AND project_id = $2 \
           AND status = 'succeeded' AND review_verdict IS NULL",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let (active_run_count, failed_run_count, succeeded, total) = run_counts;
    let success_rate = if total > 0 {
        Some(succeeded as f64 / total as f64)
    } else {
        None
    };

    let average_duration_seconds: Option<f64> = sqlx::query_scalar(
        "SELECT avg(EXTRACT(EPOCH FROM (finished_at - started_at))) \
         FROM delivery.task_run \
         WHERE tenant_id = $1 AND project_id = $2 \
           AND status = 'succeeded' AND started_at IS NOT NULL AND finished_at IS NOT NULL",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let (device_ready_count, device_total_count): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE readiness = 'ready'), count(*) \
         FROM devenv.device WHERE tenant_id = $1 AND revoked_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let waiting_by_reason: Vec<(String, i64)> = sqlx::query_as(
        "SELECT reason, count(*) FROM delivery.task_waiting \
         WHERE tenant_id = $1 AND project_id = $2 AND ended_at IS NULL \
         GROUP BY reason ORDER BY reason",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    Ok(ExecutionOverview {
        active_run_count,
        waiting_count,
        pending_interaction_count,
        pending_approval_count,
        failed_run_count,
        awaiting_review_count,
        total_run_count: total,
        succeeded_run_count: succeeded,
        success_rate,
        average_duration_seconds,
        device_ready_count,
        device_total_count,
        waiting_by_reason,
    })
}

// ───────────────────────────────────────────── 项目级活动流（S05-01 §E）

/// 项目级执行事件行（delivery.task_run_event）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct RunEventRow {
    pub id: Uuid,
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub event_type: String,
    pub lease_generation: i32,
    pub client_id: Option<Uuid>,
    pub occurred_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub detail: Option<Value>,
}

/// 项目级执行活动流（occurred_at 降序）。
pub async fn project_run_events(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    limit: i64,
) -> Result<Vec<RunEventRow>, crate::pm_write::PmWriteError> {
    use crate::pm_write::PmWriteError;
    sqlx::query_as(
        "SELECT id, run_id, task_id, event_type, lease_generation, client_id, \
                occurred_at, received_at, detail \
         FROM delivery.task_run_event \
         WHERE tenant_id = $1 AND project_id = $2 \
         ORDER BY occurred_at DESC LIMIT $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

// ───────────────────────────────────────────── Run 附属事实（S05-02 / S05-07）

/// ACP-07 Deadline 重授权行（Run 级）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct RunAuthorizationRow {
    pub id: Uuid,
    pub run_id: Uuid,
    pub interaction_id: Option<Uuid>,
    pub authorized_by: Uuid,
    pub authorized_by_name: Option<String>,
    pub reason: String,
    pub new_deadline_at: DateTime<Utc>,
    pub granted_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Run 的重授权台账（按授予时间倒序；含已撤销与已过期）。
pub async fn list_run_authorizations(
    pool: &PgPool,
    tenant_id: Uuid,
    run_id: Uuid,
) -> Result<Vec<RunAuthorizationRow>, crate::pm_write::PmWriteError> {
    use crate::pm_write::PmWriteError;
    sqlx::query_as(
        "SELECT a.id, a.run_id, a.interaction_id, a.authorized_by, \
                p.display_name AS authorized_by_name, a.reason, a.new_deadline_at, \
                a.granted_at, a.expires_at, a.revoked_at \
         FROM delivery.task_run_execution_authorization a \
         LEFT JOIN core.principal p ON p.id = a.authorized_by \
         WHERE a.tenant_id = $1 AND a.run_id = $2 \
         ORDER BY a.granted_at DESC",
    )
    .bind(tenant_id)
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// Worker 命令行。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct WorkerCommandRow {
    pub id: Uuid,
    pub command_type: String,
    pub worker_id: String,
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub payload: Value,
    pub status: String,
    pub claim_count: i32,
    pub issued_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub acked_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

/// Run 的 Worker 命令台账（issued → delivered → acked / failed）。
pub async fn list_run_commands(
    pool: &PgPool,
    tenant_id: Uuid,
    run_id: Uuid,
) -> Result<Vec<WorkerCommandRow>, crate::pm_write::PmWriteError> {
    use crate::pm_write::PmWriteError;
    sqlx::query_as(
        "SELECT id, command_type, worker_id, task_id, run_id, payload, status, \
                claim_count, issued_at, delivered_at, acked_at, failed_at, last_error \
         FROM delivery.worker_command \
         WHERE tenant_id = $1 AND run_id = $2 \
         ORDER BY issued_at DESC",
    )
    .bind(tenant_id)
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// Continuation 行。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct ContinuationRow {
    pub id: Uuid,
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub origin_interaction_id: Uuid,
    pub generation: i32,
    pub status: String,
    pub frozen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
}

/// Run 的 Continuation 链（generation 倒序）。
pub async fn list_run_continuations(
    pool: &PgPool,
    tenant_id: Uuid,
    run_id: Uuid,
) -> Result<Vec<ContinuationRow>, crate::pm_write::PmWriteError> {
    use crate::pm_write::PmWriteError;
    sqlx::query_as(
        "SELECT id, task_id, run_id, origin_interaction_id, generation, status, \
                frozen_at, created_at, started_at, closed_at \
         FROM delivery.continuation \
         WHERE tenant_id = $1 AND run_id = $2 \
         ORDER BY generation DESC",
    )
    .bind(tenant_id)
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// Run 级逐仓交付行（delivery.task_repository_commit）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct RunCommitRow {
    pub id: Uuid,
    pub repository_id: Uuid,
    pub repository_name: Option<String>,
    pub commit_sha: String,
    pub branch: Option<String>,
    pub changed_files: i32,
    pub test_evidence: Value,
    pub commit_author: Option<String>,
    pub commit_authored_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Run 的逐仓交付清单。
pub async fn list_run_commits(
    pool: &PgPool,
    tenant_id: Uuid,
    run_id: Uuid,
) -> Result<Vec<RunCommitRow>, crate::pm_write::PmWriteError> {
    use crate::pm_write::PmWriteError;
    sqlx::query_as(
        "SELECT c.id, c.repository_id, pr.name AS repository_name, c.commit_sha, \
                c.branch, c.changed_files, c.test_evidence, c.commit_author, \
                c.commit_authored_at, c.created_at \
         FROM delivery.task_repository_commit c \
         LEFT JOIN integration.project_repository pr ON pr.id = c.repository_id \
         WHERE c.tenant_id = $1 AND c.task_run_id = $2 \
         ORDER BY c.created_at",
    )
    .bind(tenant_id)
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

// ───────────────────────────────────────────── 多 Agent 协作图（S05-03）

/// 协作图节点（子任务 / 执行单元）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct CollaborationNode {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub priority: String,
    /// 该节点上的最新 Run（attempt_no / 状态 / agent）。
    pub run_id: Option<Uuid>,
    pub attempt_no: Option<i32>,
    pub run_status: Option<String>,
    pub run_kind: Option<String>,
    pub agent_id: Option<Uuid>,
    pub agent_name: Option<String>,
    /// 未完成的上游依赖数（阻塞判定）。
    pub open_upstream_count: i64,
    pub planned_start_at: Option<DateTime<Utc>>,
    pub planned_end_at: Option<DateTime<Utc>>,
    pub estimate_hours: Option<f64>,
}

/// 协作图边（依赖）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct CollaborationEdge {
    pub from_task_id: Uuid,
    pub to_task_id: Uuid,
}

/// 协作图读模型。
#[derive(Debug, Serialize)]
pub struct CollaborationGraph {
    pub nodes: Vec<CollaborationNode>,
    pub edges: Vec<CollaborationEdge>,
    /// 活跃 Agent（执行过 Run 的 agent 名册）。
    pub agents: Vec<CollaborationAgent>,
}

/// 协作图里的 Agent 行。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct CollaborationAgent {
    pub agent_id: Uuid,
    pub agent_name: Option<String>,
    pub run_count: i64,
    pub active_run_count: i64,
}

/// 任务的协作图（子任务节点 + 依赖边 + Agent 名册）。
pub async fn task_collaboration(
    pool: &PgPool,
    tenant_id: Uuid,
    task_id: Uuid,
) -> Result<CollaborationGraph, crate::pm_write::PmWriteError> {
    use crate::pm_write::PmWriteError;

    let nodes: Vec<CollaborationNode> = sqlx::query_as(
        "SELECT t.id, t.title, t.status, t.priority, \
                r.id AS run_id, r.attempt_no, r.status AS run_status, r.run_kind, \
                r.agent_id, p.display_name AS agent_name, \
                (SELECT count(*) FROM delivery.task_dependency d \
                   JOIN delivery.task up ON up.id = d.depends_on_task_id \
                  WHERE d.task_id = t.id AND up.status <> 'completed') AS open_upstream_count, \
                t.planned_start_at, t.planned_end_at, t.estimate_hours \
         FROM delivery.task t \
         LEFT JOIN LATERAL ( \
             SELECT tr.* FROM delivery.task_run tr \
              WHERE tr.task_id = t.id AND tr.tenant_id = t.tenant_id \
              ORDER BY tr.attempt_no DESC LIMIT 1 \
         ) r ON TRUE \
         LEFT JOIN core.principal p ON p.id = r.agent_id \
         WHERE t.tenant_id = $1 AND (t.id = $2 OR t.id IN ( \
                 SELECT d.depends_on_task_id FROM delivery.task_dependency d \
                  WHERE d.task_id = $2 AND d.tenant_id = $1)) \
         ORDER BY t.created_at",
    )
    .bind(tenant_id)
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let edges: Vec<CollaborationEdge> = sqlx::query_as(
        "SELECT d.depends_on_task_id AS from_task_id, d.task_id AS to_task_id \
         FROM delivery.task_dependency d \
         JOIN delivery.task t ON t.id = d.task_id \
         WHERE d.tenant_id = $1 AND (d.task_id = $2 OR d.depends_on_task_id = $2) \
         ORDER BY d.created_at",
    )
    .bind(tenant_id)
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let agents: Vec<CollaborationAgent> = sqlx::query_as(
        "SELECT r.agent_id, p.display_name AS agent_name, count(*) AS run_count, \
                count(*) FILTER (WHERE r.status IN ('claimed','running')) AS active_run_count \
         FROM delivery.task_run r \
         LEFT JOIN core.principal p ON p.id = r.agent_id \
         WHERE r.tenant_id = $1 AND r.agent_id IS NOT NULL \
           AND r.task_id IN (SELECT id FROM delivery.task WHERE tenant_id = $1 \
                              AND (id = $2 OR id IN ( \
                                  SELECT d.depends_on_task_id FROM delivery.task_dependency d \
                                   WHERE d.task_id = $2 AND d.tenant_id = $1))) \
         GROUP BY r.agent_id, p.display_name \
         ORDER BY run_count DESC",
    )
    .bind(tenant_id)
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    Ok(CollaborationGraph {
        nodes,
        edges,
        agents,
    })
}

// ───────────────────────────────────────────── Agent 名册（S05-01 / S05-05）

/// Agent / 设备名册行。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct AgentRosterRow {
    /// `principal`（AI 执行者）或 `device`（设备载体）。
    pub kind: String,
    pub id: Uuid,
    pub name: String,
    /// 设备就绪链（installed/connected/authenticated/ready/degraded/failed）。
    pub readiness: Option<String>,
    pub executor_kind: Option<String>,
    pub platform: Option<String>,
    pub active_run_count: i64,
    pub total_run_count: i64,
}

/// 项目内 Agent 名册（执行过 Run 的 AI 主体 + 已绑定设备）。
pub async fn project_agents(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<AgentRosterRow>, crate::pm_write::PmWriteError> {
    use crate::pm_write::PmWriteError;
    sqlx::query_as(
        "SELECT 'principal' AS kind, r.agent_id AS id, p.display_name AS name, \
                NULL::text AS readiness, NULL::text AS executor_kind, \
                NULL::text AS platform, \
                count(*) FILTER (WHERE r.status IN ('claimed','running')) AS active_run_count, \
                count(*) AS total_run_count \
         FROM delivery.task_run r \
         LEFT JOIN core.principal p ON p.id = r.agent_id \
         WHERE r.tenant_id = $1 AND r.project_id = $2 AND r.agent_id IS NOT NULL \
         GROUP BY r.agent_id, p.display_name \
         UNION ALL \
         SELECT 'device' AS kind, d.id, d.name, d.readiness, d.executor_kind, d.platform, \
                (SELECT count(*) FROM delivery.task_run r \
                  WHERE r.tenant_id = $1 AND r.project_id = $2 AND r.agent_id = d.owner_principal_id \
                    AND r.status IN ('claimed','running')) AS active_run_count, \
                (SELECT count(*) FROM delivery.task_run r \
                  WHERE r.tenant_id = $1 AND r.project_id = $2 AND r.agent_id = d.owner_principal_id) AS total_run_count \
         FROM devenv.device d \
         WHERE d.tenant_id = $1 AND d.revoked_at IS NULL \
         ORDER BY 2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}
