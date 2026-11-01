//! Supervision service (Task 15): the three human powers over an AI-led
//! project — 否决 (veto / post-hoc rollback), 刹车 (freeze / unfreeze), and
//! 立法确认 (red-line confirmation) — plus the supervision desk's read-only
//! notification feed.
//!
//! Veto ([`veto`]) branches on the proposal state:
//!
//! * `executed` → the command behind `executed_audit_id` is compensated
//!   through [`PgCommandExecutor::compensate`] as a `Human` actor with a
//!   `HumanIntervention` source: `Ok` → intervention('veto') +
//!   notification('veto_applied') → [`VetoOutcome::RolledBack`];
//!   `AlreadyCompensated` → intervention('veto') (idempotent, one row per
//!   veto attempt) → [`VetoOutcome::AlreadyCompensated`] (HTTP 200);
//!   `RevisionConflict` / `Validation` (an irreversible command carries no
//!   compensation spec) → intervention('rollback_failed', executed_at NULL)
//!   with notification('rollback_failed') → [`SupervisionError::Command`]
//!   (HTTP 409) — a rollback never overwrites newer state, and a veto of
//!   something un-undoable is still on record. The veto window is
//!   deliberately *not* checked here: the 「随时否决」 semantics keep the
//!   veto available after the window closes.
//! * `redline_pending` / `escalated` → the proposal closes as `vetoed` (and
//!   its still-pending approval request is rejected with the veto reason, so
//!   a closed proposal can never be approved afterwards) →
//!   [`VetoOutcome::ProposalClosed`].
//! * anything else → [`SupervisionError::NotFound`] (HTTP 404).
//!
//! Red-line confirmation ([`decide_approval`]) moves a pending
//! `governance.approval_request` to `approved` / `rejected` in one atomic
//! `UPDATE ... WHERE status = 'pending'`; a repeat decision surfaces the
//! executor's `AlreadyDecided` (the route maps it to HTTP 200 with
//! `already: true`). `Approve` executes the stored command through the one
//! channel the executor accepts for red lines: `ActorKind::Human` +
//! [`CommandSource::RedlineApproval`] (the audit row then carries
//! `source_kind = 'redline_approval'`, the Task 6 join anchor).
//!
//! The brake ([`freeze`] / [`unfreeze`]) flips `ai.project_ai_config.
//! automation_mode` between `controlled` and `ai_lead` and records an
//! intervention plus a `project_frozen` / `project_unfrozen` notification.
//! The claim layer (Task 10) filters on the mode, so freezing stops the AI
//! from picking up new jobs; the human paths in this module never consult it.

#![allow(unused_imports)]

pub(crate) use pm_domain::{AuditId, CommandError, PrincipalId, ProjectCommand, ProjectId, ProposalId};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use serde_json::json;
pub(crate) use sqlx::PgPool;
pub(crate) use uuid::Uuid;

pub(crate) use crate::command_executor::{
    ActorKind, CommandSource, ExecutionContext, ExecutionReceipt, PgCommandExecutor,
};

mod approval;
mod feed;
mod freeze;
mod veto;

pub use approval::*;
pub use feed::*;
pub use freeze::*;
pub use veto::*;

/// Everything the veto endpoint needs: which proposal, in which project, by
/// whom, and why (the reason lands in the intervention row and notification).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VetoRequest {
    pub project_id: ProjectId,
    pub proposal_id: ProposalId,
    pub actor_id: PrincipalId,
    pub reason: String,
}


/// What one veto did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VetoOutcome {
    /// The executed command was compensated; `new_audit_id` is the
    /// `rollback_command` audit row the compensation wrote.
    RolledBack { new_audit_id: AuditId },
    /// A pre-execution proposal (`redline_pending` / `escalated`) was closed
    /// as `vetoed`.
    ProposalClosed,
    /// The executed command had already been compensated; idempotent success
    /// (HTTP 200 at the route), recorded as one more intervention('veto').
    AlreadyCompensated,
}


/// The human ruling on a red-line approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Approve,
    Reject,
}


/// Everything the decision endpoint needs. `approval_id` is normally taken
/// from the path, not the body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecideRequest {
    pub approval_id: Uuid,
    pub actor_id: PrincipalId,
    pub decision: Decision,
    pub reason: String,
}


/// What one decision did.
#[derive(Debug, Clone)]
pub enum DecisionOutcome {
    /// The red-line command executed through the human channel; the audit row
    /// carries `source_kind = 'redline_approval'` with the approval id.
    Executed { receipt: ExecutionReceipt },
    /// The proposal was rejected and closed as `vetoed`.
    Rejected,
}


/// One row of the supervision desk's notification feed.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct NotificationRow {
    pub id: Uuid,
    pub kind: String,
    pub payload: serde_json::Value,
    pub veto_window_until: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}


/// Errors surfaced by the supervision service.
///
/// `NotFound` covers the 404 cases: a veto target that does not exist or sits
/// in a non-vetoable state, an unknown approval id, a project without AI
/// config. `Command(CommandError::RevisionConflict)` is the HTTP 409
/// `rollback_failed` case (the intervention row is already written when it
/// surfaces); `Command(CommandError::AlreadyDecided)` is the HTTP 200
/// idempotent-repeat case.
#[derive(Debug, thiserror::Error)]
pub enum SupervisionError {
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

