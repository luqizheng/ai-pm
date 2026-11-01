//! 冷启动导入状态机：`queued → cloning → reading → analyzing → draft_ready`，
//! `apply` 之后转 `proposed`，审批执行完成转 `applied` 或 `rejected`。
//!
//! 导入产物**不直写业务表**：草案只落 `integration.repo_import.draft`，由
//! `propose_import` 转成 `ai.change_proposal`，走既有红线审批与执行链。

#![allow(unused_imports)]

pub(crate) use chrono::{DateTime, Utc};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use sqlx::PgPool;
pub(crate) use uuid::Uuid;

pub(crate) use crate::git_access;
pub(crate) use crate::actor::AuditActor;
pub(crate) use crate::pm_write::PmWriteError;
pub(crate) use crate::repo_facts::{self, RepoAnalysisModel, RepoMaterial, pm_ai_port::ModelError};
pub(crate) use pm_domain::import::{Evidence, EvidenceKind, ProjectDraft};

/// `list_imports` 的结果行（14 列）。
type ImportListRow = (
    Uuid,
    String,
    i32,
    String,
    i64,
    Option<Uuid>,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
);
/// `get_import` 的结果行（14 列，含草案与阶段详情）。
type ImportDetailRow = (
    Uuid,
    String,
    i32,
    Option<String>,
    String,
    Option<serde_json::Value>,
    i64,
    i32,
    Option<String>,
    Option<String>,
    Option<Uuid>,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
);
/// `get_import` 的来源快照行（8 列）。
type ImportSourceRow = (Uuid, String, Option<String>, i32, i32, i64, bool, Option<String>);

mod advance;
mod grounding;
mod lifecycle;
mod proposal;
mod support;

pub use advance::*;
pub use grounding::*;
pub use lifecycle::*;
pub use proposal::*;
pub use support::*;

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("project not found")]
    ProjectNotFound,
    #[error("project already has requirements")]
    ProjectNotEmpty,
    #[error("project has no verified repository")]
    NoVerifiedRepository,
    #[error("an import is already in progress")]
    ImportInProgress,
    #[error("import not found")]
    NotFound,
    #[error("import can no longer be cancelled")]
    NotCancellable,
    #[error("draft revision is stale")]
    StaleDraft,
    #[error("no draft items selected")]
    NoSelection,
    #[error("project code is already taken")]
    ProjectCodeTaken,
    #[error("import already applied")]
    AlreadyApplied,
    #[error("import has no grounded output")]
    NoGroundedOutput,
    #[error("{0}")]
    InvalidState(&'static str),
    #[error("model failed: {0}")]
    Model(String),
    #[error("git failed: {0}")]
    Git(String),
    #[error("database error: {0}")]
    Db(sqlx::Error),
}


impl From<sqlx::Error> for ImportError {
    fn from(e: sqlx::Error) -> Self {
        ImportError::Db(e)
    }
}


impl From<PmWriteError> for ImportError {
    fn from(e: PmWriteError) -> Self {
        match e {
            PmWriteError::NotFound(_) => ImportError::NotFound,
            PmWriteError::Conflict(message) | PmWriteError::Validation(message) => {
                ImportError::Model(message.to_string())
            }
            PmWriteError::Db(err) => ImportError::Db(err),
        }
    }
}

pub const IN_PROGRESS: [&str; 4] = ["queued", "cloning", "reading", "analyzing"];


#[derive(Debug, Clone, Serialize)]
pub struct StartImport {
    pub import_id: Uuid,
    pub status: String,
}


#[derive(Debug, Clone, Serialize)]
pub struct ImportSourceView {
    pub repository_id: Uuid,
    pub label: String,
    pub head_commit_sha: Option<String>,
    pub commit_count: i32,
    pub doc_count: i32,
    pub size_bytes: i64,
    pub truncated: bool,
    pub error_code: Option<String>,
}


#[derive(Debug, Clone, Serialize)]
pub struct ImportDetail {
    pub id: Uuid,
    pub project_id: Uuid,
    pub status: String,
    pub progress: i32,
    pub stage_detail: Option<String>,
    pub model_name: String,
    pub draft: Option<ProjectDraft>,
    pub draft_revision: i64,
    pub dropped_items: i32,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
    pub analysis_run_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub proposed_at: Option<DateTime<Utc>>,
    pub applied_at: Option<DateTime<Utc>>,
    pub sources: Vec<ImportSourceView>,
}


/// 项目下的导入批次（审批页分组用）。
#[derive(Debug, Clone, Serialize)]
pub struct ImportListItem {
    pub id: Uuid,
    pub status: String,
    pub progress: i32,
    pub model_name: String,
    pub draft_revision: i64,
    pub analysis_run_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub proposed_at: Option<DateTime<Utc>>,
    pub applied_at: Option<DateTime<Utc>>,
}

