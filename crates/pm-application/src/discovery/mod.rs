//! Discovery 认知域（T-083）：`discovery_session` 及其子表（message / finding /
//! question / answer / proposal）的读写与状态机。
//!
//! Discovery 是独立于 Repository Import 的业务域：Import 负责把仓库接入系统，
//! Discovery 负责理解项目（设计稿 `docs/ai-pm-project-discovery-v1.md` §一/§四）。
//! 本模块只做采集层写入；正式知识入库仍走既有 `propose_document` 提案链
//! （§4.2 知识污染红线），`discovery_proposal` 仅记录提案事实与 `base_revision_id`。

pub(crate) use chrono::{DateTime, Utc};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use sqlx::PgPool;
pub(crate) use uuid::Uuid;

mod finding;
mod message;
mod proposal;
mod question;
mod session;
mod web;

pub use finding::*;
pub use message::*;
pub use proposal::*;
pub use question::*;
pub use session::*;
pub use web::{
    FindingVerificationOutcome, StartWebSession, record_finding_verification,
    start_session_from_import, start_session_from_import_as,
};

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("discovery session not found")]
    SessionNotFound,
    #[error("discovery finding not found")]
    FindingNotFound,
    #[error("discovery question not found")]
    QuestionNotFound,
    #[error("discovery proposal not found")]
    ProposalNotFound,
    #[error("repo import not found")]
    ImportNotFound,
    #[error("project not found")]
    ProjectNotFound,
    #[error("invalid session status transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },
    #[error("{0}")]
    Validation(String),
    #[error("database error: {0}")]
    Db(sqlx::Error),
}

impl From<sqlx::Error> for DiscoveryError {
    fn from(e: sqlx::Error) -> Self {
        DiscoveryError::Db(e)
    }
}

pub const SESSION_SOURCES: [&str; 3] = ["WEB", "CODEX", "SYSTEM"];
pub const SESSION_KINDS: [&str; 3] = ["INITIAL_ONBOARDING", "UPDATE", "TASK_DISCOVERY"];
pub const SESSION_STATUS_IN_PROGRESS: &str = "IN_PROGRESS";
pub const SESSION_STATUS_NEEDS_USER_INPUT: &str = "NEEDS_USER_INPUT";
pub const SESSION_STATUS_REVIEW_REQUIRED: &str = "REVIEW_REQUIRED";
pub const SESSION_STATUS_COMPLETED: &str = "COMPLETED";
pub const SESSION_STATUS_PAUSED: &str = "PAUSED";
pub const SESSION_STATUSES: [&str; 5] = [
    "IN_PROGRESS",
    "NEEDS_USER_INPUT",
    "REVIEW_REQUIRED",
    "COMPLETED",
    "PAUSED",
];

pub const FINDING_KINDS: [&str; 4] =
    ["observed", "user_confirmed", "speculation", "contradicted"];
pub const VERIFICATION_STATUSES: [&str; 4] = ["PENDING", "CONFIRMED", "REJECTED", "CONFLICT"];

pub const QUESTION_STATUS_OPEN: &str = "open";
pub const QUESTION_STATUS_ANSWERED: &str = "answered";
pub const QUESTION_STATUS_DEFERRED: &str = "deferred";

pub const PROPOSAL_ACTION_ADD: &str = "add";
pub const PROPOSAL_ACTION_MODIFY: &str = "modify";
pub const PROPOSAL_STATUS_PENDING: &str = "pending";
pub const PROPOSAL_STATUS_APPROVED: &str = "approved";
pub const PROPOSAL_STATUS_REJECTED: &str = "rejected";
pub const PROPOSAL_STATUS_CONFLICT: &str = "conflict";

pub(crate) fn validate_one_of(value: &str, allowed: &[&str], label: &str) -> Result<(), DiscoveryError> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(DiscoveryError::Validation(format!(
            "{label} must be one of {allowed:?}, got {value:?}"
        )))
    }
}

pub(crate) fn validate_confidence(confidence: Option<f64>) -> Result<(), DiscoveryError> {
    match confidence {
        Some(c) if (0.0..=1.0).contains(&c) => Ok(()),
        Some(_) => Err(DiscoveryError::Validation(
            "confidence must be within [0,1]".to_string(),
        )),
        None => Ok(()),
    }
}

pub(crate) fn empty_array_if_null(value: serde_json::Value) -> serde_json::Value {
    if value.is_null() {
        serde_json::json!([])
    } else {
        value
    }
}