//! 任务执行 Run 读模型 port（任务书批 B 域 6，自 pm-api `pm_runs.rs` 收口）。
//!
//! 域语义（spec §19.5/§20、T-070 §5.2、S05 0013，见 pm_runs 模块头）：Run
//! 列表/详情/事件时间线/项目汇总全部只读；可见性 = 调用者在
//! `core.project_member`；ACP-07 重授权守卫（租户 admin 或项目 owner）的
//! 两个读原语也在此（角色判定裁决留 HTTP 层）。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

/// Run 列表行。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct TaskRunRow {
    pub id: Uuid,
    pub task_id: Uuid,
    pub task_title: String,
    pub attempt_no: i32,
    pub run_kind: String,
    pub status: String,
    pub outcome: Option<String>,
    pub agent_id: Option<Uuid>,
    pub executor_kind: Option<String>,
    pub executor_session: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    /// 评审裁决（三值：approved / changes_requested / inconclusive；未评审为 NULL）。
    pub review_verdict: Option<String>,
    pub failure_category: Option<String>,
    pub failure_reason: Option<String>,
}

/// 单 Run 详情头：列表字段 + 租约 / 执行器 / 阶段时间线 / 集成与审查结论。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct TaskRunDetailRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub task_title: String,
    pub attempt_no: i32,
    pub run_kind: String,
    pub status: String,
    pub outcome: Option<String>,
    pub stage: Option<String>,
    pub agent_id: Option<Uuid>,
    pub executor_kind: Option<String>,
    pub executor_session: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub last_agent_heartbeat_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub deadline_at: Option<DateTime<Utc>>,
    pub release_reason: Option<String>,
    pub integration_status: String,
    pub review_verdict: Option<String>,
    pub created_at: DateTime<Utc>,
    pub failure_category: Option<String>,
    pub failure_reason: Option<String>,
}

/// 本 Run 锁定的一仓基线（`task_run_repository`）；`workspace_path` 即「日志定位」文本。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct RunRepositoryRow {
    pub repository_id: Uuid,
    pub repository_name: String,
    pub base_ref: String,
    pub base_commit: String,
    pub write_access: bool,
    pub workspace_path: String,
}

/// 详情响应：详情头 + 本 Run 锁定的各仓基线。
#[derive(Debug, serde::Serialize)]
pub struct TaskRunDetail {
    #[serde(flatten)]
    pub run: TaskRunDetailRow,
    pub repositories: Vec<RunRepositoryRow>,
}

/// 单条 run 事件（`delivery.task_run_event`；表追加式禁 UPDATE）。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct RunEventRow {
    pub id: Uuid,
    pub event_type: String,
    pub lease_generation: i32,
    pub dedupe_tag: String,
    pub client_id: Option<Uuid>,
    pub occurred_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub detail: Option<Value>,
}

/// 项目内 Run 汇总（状态/结果分布 + 活跃数）。
#[derive(Debug)]
pub struct RunSummary {
    pub total: i64,
    pub by_status: Value,
    pub by_outcome: Value,
    pub active: i64,
}

/// 项目内 Run 事件汇总（近 24h + 最近事件时刻）。
#[derive(Debug)]
pub struct RunEventSummary {
    pub events_24h: i64,
    pub last_event_at: Option<DateTime<Utc>>,
}

/// ACP-07 守卫读原语：run 归属 + 调用者租户角色（None = 非成员）。
#[derive(Debug, Clone, Copy)]
pub struct RunAuthorizationCtx {
    pub project_id: Uuid,
    /// 调用者在 run 租户内的角色（owner/admin/member；无行 = None）。
    pub tenant_role: Option<&'static str>,
}

/// 分页参数（size/offset）。
#[derive(Debug, Clone, Copy)]
pub struct Page {
    pub size: i64,
    pub offset: i64,
}

#[async_trait]
pub trait RunRepo: Send + Sync {
    /// ACP-07 守卫①：run 存在于调用者租户 + 调用者租户角色。
    async fn run_authorization_ctx(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<RunAuthorizationCtx>, sqlx::Error>;

    /// ACP-07 守卫②：调用者是否为项目 owner。
    async fn is_project_owner(
        &self,
        project_id: Uuid,
        principal_id: Uuid,
    ) -> Result<bool, sqlx::Error>;

    async fn list_runs(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        task_id: Option<Uuid>,
        run_kind: Option<&str>,
        page: Page,
    ) -> Result<(Vec<TaskRunRow>, i64), sqlx::Error>;

    /// 详情头（不存在 / 跨租户 / 非成员 → None）。
    async fn find_run_detail(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<TaskRunDetailRow>, sqlx::Error>;

    async fn run_repositories(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<RunRepositoryRow>, sqlx::Error>;

    /// 事件时间线可见性检查（同详情口径 → None）。
    async fn run_visible(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error>;

    async fn list_run_events(
        &self,
        run_id: Uuid,
        event_type: Option<&str>,
        page: Page,
    ) -> Result<(Vec<RunEventRow>, i64), sqlx::Error>;

    async fn run_summary(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<RunSummary, sqlx::Error>;

    async fn run_event_summary(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<RunEventSummary, sqlx::Error>;
}
