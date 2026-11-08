//! Command channel core (Task 6): the single write path into the database for
//! project commands — execute + audit + snapshot compensation.
//!
//! Every command runs inside one sqlx transaction: `SELECT ... FOR UPDATE` on
//! the target row → revision guard → apply the change and bump `revision` →
//! `INSERT governance.audit_log` (before/after JSONB, compensation spec) →
//! commit and return an [`ExecutionReceipt`].
//!
//! Compensation is status-recovery style: the audit row carries a
//! [`CompensationSpec`] snapshotted at execution time. `compensate` replays it
//! under a fresh revision guard — the snapshot's revision column holds the
//! executed command's `resulting_revision`, so if the target row has moved on
//! since, the rollback fails with [`CommandError::RevisionConflict`] instead of
//! silently overwriting newer state. Idempotency of compensation itself is
//! guarded by `compensated_command_id` ([`CommandError::AlreadyCompensated`]).
//!
//! Red lines: the red-line set comes from the shared [`PolicyEngine`] (the
//! single source of truth; never a local list). A red-line command executes
//! only for an [`ActorKind::Human`] actor through the
//! [`CommandSource::RedlineApproval`] flow; in M1 the executor rejects every
//! non-human actor outright (`RedLineBlocked`) before any transaction opens.

#![allow(unused_imports)]

pub(crate) use std::collections::HashMap;
pub(crate) use std::sync::Arc;

pub(crate) use pm_domain::import::{DocumentDraft, MilestoneDraft, RequirementDraft, TaskDraft};
pub(crate) use pm_domain::{
    validate_transition, AuditId, CommandError, MilestoneId, PrincipalId, ProjectCommand,
    ProjectId, RequirementId, RevisionId, TaskId, TaskStatus, TransitionInput,
};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use serde_json::{json, Value};
pub(crate) use sqlx::Row;
pub(crate) use uuid::Uuid;

pub(crate) use crate::bootstrap::resolve_assignee;
pub(crate) use crate::knowledge::{self, CreateDocumentRequest, KnowledgeRoot};
pub(crate) use crate::outbox::{OutboxAppend, OutboxEvent, PgOutboxAppend};
pub(crate) use crate::policy_engine::PolicyEngine;

mod audit;
mod compensation;
mod executor;
mod guards;
mod transition;

pub use audit::*;
pub use compensation::*;
pub use executor::*;
pub use guards::*;
pub use transition::*;

/// Who issues a command. The executor only branches on [`ActorKind::Human`]
/// (red-line gate); the other variants mirror `core.principal.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    Human,
    AiPm,
    DevAgent,
    Service,
}

/// Where a command came from. `AiProposal` additionally lands the run id in
/// `governance.audit_log.analysis_run_id`.
#[derive(Debug, Clone)]
pub enum CommandSource {
    AiProposal {
        run_id: pm_domain::RunId,
        proposal_id: pm_domain::ProposalId,
    },
    HumanIntervention {
        intervention_id: Uuid,
    },
    RedlineApproval {
        approval_id: Uuid,
    },
    Bootstrap,
    /// 平台内部动作（如 Run 调度领取、租约回收）——无人类/提案身份。
    System,
}

/// Execution identity: which project, which principal, on whose behalf, from
/// which channel.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub project_id: ProjectId,
    /// 执行主体 + 认证身份（T-021a 三列 + T-088 设备归因列）。`actor.principal_id`
    /// 是执行主体（human / ai_pm / dev_agent / service）；真人身份列来自认证上下文，
    /// 后台动作为 NULL；`claimed_device_id` 仅归因。
    pub actor: crate::actor::AuditActor,
    pub actor_kind: ActorKind,
    pub source: CommandSource,
}

impl ExecutionContext {
    /// 执行主体 principal（兼容旧读法）。
    pub fn actor_id(&self) -> pm_domain::PrincipalId {
        self.actor.principal_id
    }
}

/// A compensable table, mapped to `(schema.table, primary-key column,
/// revision column)` by [`CompensableTable::mapping`] (shared with the Task 7
/// handlers — do not inline).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompensableTable {
    Task,
    Milestone,
}

impl CompensableTable {
    /// Fully-qualified table name, primary-key column, optimistic-lock column.
    pub(crate) fn mapping(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Task => ("delivery.task", "id", "revision"),
            Self::Milestone => ("delivery.milestone", "id", "revision"),
        }
    }
}

/// One row snapshot inside a [`CompensationSpec::Restore`].
///
/// `fields` maps column names to the values to write back; the entry's
/// revision column holds the revision the row must currently be at when this
/// entry is restored (the executing command's `resulting_revision`). Restore
/// bumps the revision by one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub table: CompensableTable,
    pub target_id: Uuid,
    pub fields: Value,
}

/// How to undo an executed command, stored in
/// `governance.audit_log.compensation_command`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CompensationSpec {
    /// Restore previously snapshotted field values (status-recovery style).
    Restore(Vec<SnapshotEntry>),
    /// Delete a row created by the command (create-style handlers, Task 7).
    Delete {
        table: CompensableTable,
        target_id: Uuid,
    },
}

/// What one successful execution (or compensation) produced.
#[derive(Debug, Clone)]
pub struct ExecutionReceipt {
    pub audit_id: AuditId,
    pub command_kind: &'static str,
    pub target_id: Uuid,
    pub resulting_revision: i64,
}

/// Postgres-backed command executor: the write-safety base for V2. All project
/// commands go through [`PgCommandExecutor::execute`]; rollbacks go through
/// [`PgCommandExecutor::compensate`].
pub struct PgCommandExecutor {
    pool: sqlx::PgPool,
    policy: Arc<PolicyEngine>,
    /// §20.6.1：outbox 事件与业务状态变更写在同一事务里（`append` 收 `&mut tx`）。
    outbox: Arc<dyn OutboxAppend>,
}

pub(crate) type Tx = sqlx::Transaction<'static, sqlx::Postgres>;
