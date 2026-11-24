//! 验收/工件读模型 port（任务书批 B 域 13，自 pm-api `pm_acceptance.rs` /
//! `pm_artifacts.rs` / `pm_reviews.rs` 收口）。
//!
//! 全部只读投影；可见性前置（require_task_project / require_project_visible）
//! 留 HTTP 层。行结构是前端契约，随 SQL 固化在实现侧。
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

/// 评审发现行（review_finding ⋈ task_run；blocker 置顶由 SQL 排序保证）。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct ReviewFindingRow {
    pub id: Uuid,
    pub review_run_id: Uuid,
    pub review_attempt_no: i32,
    pub review_run_status: String,
    pub criterion_id: Option<Uuid>,
    pub kind: String,
    pub severity: String,
    pub code_location: Option<String>,
    pub message: String,
    pub evidence_ref: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// 验收标准裁决行（标准 LEFT JOIN LATERAL 最新裁决）。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct CriterionVerdictRow {
    pub criterion_id: Uuid,
    pub ordinal: i32,
    pub text: String,
    pub verification_kind: String,
    pub evaluated: bool,
    pub review_run_id: Option<Uuid>,
    pub review_attempt_no: Option<i32>,
    pub verdict: Option<String>,
    pub evidence: Option<Value>,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    pub verdict_created_at: Option<DateTime<Utc>>,
}

/// 评审/交付审批行（review_approval ⋈ task）。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct ApprovalRow {
    pub id: Uuid,
    pub task_id: Uuid,
    pub review_run_id: Uuid,
    pub delivery_run_id: Uuid,
    pub kind: String,
    pub status: String,
    pub decided_by: Option<Uuid>,
    pub decision_reason: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub task_title: String,
}

/// 需求修订工件行（requirement_revision ⋈ requirement）。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct ArtifactRow {
    pub id: Uuid,
    pub requirement_id: Uuid,
    pub requirement_code: String,
    pub requirement_title: String,
    pub version: i32,
    pub state: String,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
    pub approved_by: Option<Uuid>,
}

/// 分页参数（size/offset）。
#[derive(Debug, Clone, Copy)]
pub struct Page {
    pub size: i64,
    pub offset: i64,
}

#[async_trait]
pub trait AcceptanceReadRepo: Send + Sync {
    async fn review_findings(
        &self,
        task_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<ReviewFindingRow>, sqlx::Error>;

    async fn criterion_verdicts(
        &self,
        task_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<CriterionVerdictRow>, sqlx::Error>;

    async fn task_approvals(
        &self,
        task_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<ApprovalRow>, sqlx::Error>;

    async fn project_approvals(&self, project_id: Uuid) -> Result<Vec<ApprovalRow>, sqlx::Error>;

    async fn list_artifacts(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: Page,
    ) -> Result<(Vec<ArtifactRow>, i64), sqlx::Error>;
}
