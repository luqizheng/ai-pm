//! 知识空间 V2 提案流：质量门 → 自动提交 / 红线事前确认 / 事后否决回滚。
//!
//! 状态机（设计稿 §八）：`quality_gates_passed → commit_pending → committed`；
//! 红线走 `redline_pending → confirmed → committed`；失败 `rejected`；否决
//! `rolled_back`（并触发 Git revert）。正文权威仍是 Git，提案正文只作为候选。

#![allow(unused_imports)]

use serde::Serialize;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::knowledge::{self, KnowledgeError, KnowledgeRoot};
use crate::knowledge_quality::{self, GateFailure, GateInput, QualityGateResult};

/// 否决窗口默认 24 小时（可用 `AI_PM_KNOWLEDGE_VETO_WINDOW_SECS` 覆盖）。
pub const DEFAULT_VETO_WINDOW_SECS: i64 = 86_400;

mod query;
mod submit;
mod support;
mod veto;

pub use query::*;
pub use submit::*;
pub use support::*;
pub use veto::*;

#[derive(Debug, Clone)]
pub struct ProposalRequest {
    pub project_id: Uuid,
    pub doc_key: String,
    pub doc_type: String,
    pub markdown: String,
    pub change_summary: Option<String>,
    pub source_event_id: Option<Uuid>,
    pub analysis_run_id: Option<Uuid>,
    /// 显式来源标记（例如需求迁移 `requirement_revision:<id>`）。
    pub source_ref: Option<String>,
    pub redline_kind: Option<String>,
    pub base_content_hash: Option<String>,
    pub created_by: Uuid,
}


#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ProposalOutcome {
    /// 同一文档、同一内容：幂等无操作。
    Duplicate {
        document_id: Uuid,
        revision_id: Uuid,
        revision_no: i32,
    },
    /// 红线类：已落库，等待人类事前确认，未写 Git。
    RedlinePending { proposal_id: Uuid },
    /// 质量门拒绝：未写 Git。
    Rejected {
        proposal_id: Uuid,
        failures: Vec<GateFailure>,
    },
    /// 已提交 Git 并落修订。
    Committed {
        proposal_id: Uuid,
        document_id: Uuid,
        revision_id: Uuid,
        revision_no: i32,
        git_commit_sha: String,
        content_hash: String,
        canonical_path: String,
    },
}


#[derive(Debug, Clone, Serialize)]
pub struct ProposalRow {
    pub id: Uuid,
    pub doc_key: String,
    pub doc_type: String,
    pub status: String,
    pub quality_gate_result: serde_json::Value,
    pub redline_kind: Option<String>,
    pub change_summary: Option<String>,
    pub resulting_revision_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}


#[derive(Debug, Clone, Serialize)]
pub struct RevisionVetoOutcome {
    pub new_commit_sha: String,
    pub rolled_back_revision_id: Uuid,
    pub restored_revision_id: Option<Uuid>,
}


#[derive(Debug, Clone)]
struct CurrentDoc {
    document_id: Option<Uuid>,
    revision_id: Option<Uuid>,
    revision_no: i32,
    content_hash: Option<String>,
}

