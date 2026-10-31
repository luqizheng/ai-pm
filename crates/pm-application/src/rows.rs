//! `/v1/pm/*` API 行结构（Slice A 契约，字段源：`frontend/src/api/types.ts`，
//! 与 `migrations/0001-0004` 表列一一对应）。读端点与写端点返回共用。

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PrincipalRow {
    pub id: Uuid,
    pub kind: String,
    pub display_name: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ProjectRow {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RequirementRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub code: String,
    pub title: String,
    pub status: String,
    pub current_revision_id: Option<Uuid>,
    pub row_revision: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MilestoneRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub planned_at: Option<DateTime<Utc>>,
    pub status: String,
    pub revision: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DecisionRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub status: String,
    pub required_role: String,
    pub decided_by: Option<Uuid>,
    pub decision_reason: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub proposal_id: Option<Uuid>,
    pub proposal_status: Option<String>,
    pub target_type: Option<String>,
    pub command_type: Option<String>,
    pub rationale: Option<String>,
    pub risk_level: Option<String>,
    pub evidence: Option<serde_json::Value>,
    pub proposed_data: Option<serde_json::Value>,
    pub analysis_run_id: Option<Uuid>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TaskRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub milestone_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub assignee_id: Option<Uuid>,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ApprovalRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub proposal_id: Uuid,
    pub status: String,
    pub required_role: String,
    pub decided_by: Option<Uuid>,
    pub decision_reason: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AuditLogRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub actor_id: Uuid,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RequirementRevisionRow {
    pub id: Uuid,
    pub requirement_id: Uuid,
    pub project_id: Uuid,
    pub version: i32,
    pub content: serde_json::Value,
    pub state: String,
    pub content_hash: String,
    pub created_by: Uuid,
    pub approved_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
