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

use pm_domain::{AuditId, CommandError, PrincipalId, ProjectCommand, ProjectId, ProposalId};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::command_executor::{
    ActorKind, CommandSource, ExecutionContext, ExecutionReceipt, PgCommandExecutor,
};

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

/// Vetoes one proposal: rolls an executed command back, or closes a
/// pre-execution (`redline_pending` / `escalated`) proposal. See the module
/// docs for the full branch table.
pub async fn veto(
    pool: &PgPool,
    executor: &PgCommandExecutor,
    req: VetoRequest,
) -> Result<VetoOutcome, SupervisionError> {
    let (status, executed_audit_id, command_type): (String, Option<Uuid>, String) =
        sqlx::query_as(
            "SELECT status, executed_audit_id, command_type \
             FROM ai.change_proposal WHERE id = $1 AND project_id = $2",
        )
        .bind(req.proposal_id)
        .bind(req.project_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| {
            SupervisionError::NotFound(format!(
                "proposal {} not found in project {}",
                req.proposal_id, req.project_id
            ))
        })?;

    match status.as_str() {
        // Post-hoc veto of an already applied change: compensate the command
        // behind the proposal's audit linkage. The veto window is not
        // consulted — 「随时否决」.
        "executed" => {
            let executed_audit_id = executed_audit_id.ok_or_else(|| {
                SupervisionError::NotFound(format!(
                    "proposal {} is executed but carries no executed_audit_id",
                    req.proposal_id
                ))
            })?;
            // The intervention id exists before the compensation so the
            // rollback audit row can carry it as its source identity.
            let intervention_id = Uuid::new_v4();
            let ctx = ExecutionContext {
                project_id: req.project_id,
                actor_id: req.actor_id,
                actor_kind: ActorKind::Human,
                source: CommandSource::HumanIntervention { intervention_id },
            };
            match executor.compensate(executed_audit_id, &req.reason, ctx).await {
                Ok(receipt) => {
                    let mut tx = pool.begin().await?;
                    record_intervention(
                        &mut tx,
                        intervention_id,
                        req.project_id,
                        "veto",
                        Some(req.proposal_id),
                        Some(executed_audit_id),
                        req.actor_id,
                        &req.reason,
                        Some(chrono::Utc::now()),
                    )
                    .await?;
                    notify(
                        &mut tx,
                        req.project_id,
                        "veto_applied",
                        json!({
                            "proposal_id": req.proposal_id,
                            "command_type": command_type,
                            "rollback_audit_id": receipt.audit_id,
                            "reason": req.reason,
                        }),
                    )
                    .await?;
                    tx.commit().await?;
                    Ok(VetoOutcome::RolledBack { new_audit_id: receipt.audit_id })
                }
                // Already rolled back (e.g. by an earlier veto): the veto is
                // still recorded, and the outcome is a success.
                Err(CommandError::AlreadyCompensated { .. }) => {
                    let mut tx = pool.begin().await?;
                    record_intervention(
                        &mut tx,
                        intervention_id,
                        req.project_id,
                        "veto",
                        Some(req.proposal_id),
                        Some(executed_audit_id),
                        req.actor_id,
                        &req.reason,
                        Some(chrono::Utc::now()),
                    )
                    .await?;
                    tx.commit().await?;
                    Ok(VetoOutcome::AlreadyCompensated)
                }
                // The target moved on since the proposal executed: the
                // rollback is refused (never overwriting newer state) and the
                // failure lands as intervention('rollback_failed',
                // executed_at NULL — nothing was executed) plus its feed
                // notification: the veto attempt must stay on record.
                Err(err @ CommandError::RevisionConflict { .. }) => {
                    let mut tx = pool.begin().await?;
                    record_intervention(
                        &mut tx,
                        intervention_id,
                        req.project_id,
                        "rollback_failed",
                        Some(req.proposal_id),
                        Some(executed_audit_id),
                        req.actor_id,
                        &req.reason,
                        None,
                    )
                    .await?;
                    notify(
                        &mut tx,
                        req.project_id,
                        "rollback_failed",
                        json!({
                            "proposal_id": req.proposal_id,
                            "command_type": command_type,
                            "reason": req.reason,
                        }),
                    )
                    .await?;
                    tx.commit().await?;
                    Err(SupervisionError::Command(err))
                }
                // An irreversible command (e.g. an approved `sign_acceptance`
                // carries no compensation spec) can be vetoed but never rolled
                // back: the refusal is recorded exactly like a revision
                // conflict — intervention('rollback_failed', executed_at NULL)
                // + feed notification — so the human veto never vanishes
                // without a trace, and surfaces as a 409, not a bare 500.
                Err(err @ CommandError::Validation(_)) => {
                    let mut tx = pool.begin().await?;
                    record_intervention(
                        &mut tx,
                        intervention_id,
                        req.project_id,
                        "rollback_failed",
                        Some(req.proposal_id),
                        Some(executed_audit_id),
                        req.actor_id,
                        &req.reason,
                        None,
                    )
                    .await?;
                    notify(
                        &mut tx,
                        req.project_id,
                        "rollback_failed",
                        json!({
                            "proposal_id": req.proposal_id,
                            "command_type": command_type,
                            "reason": req.reason,
                        }),
                    )
                    .await?;
                    tx.commit().await?;
                    Err(SupervisionError::Command(err))
                }
                Err(err) => Err(SupervisionError::Command(err)),
            }
        }
        // Pre-execution veto: close the proposal (and, for a red line, its
        // still-pending approval request, so a vetoed proposal can never be
        // approved afterwards).
        "redline_pending" | "escalated" => {
            let mut tx = pool.begin().await?;
            sqlx::query("UPDATE ai.change_proposal SET status = 'vetoed' WHERE id = $1")
                .bind(req.proposal_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "UPDATE governance.approval_request \
                 SET status = 'rejected', decided_by = $2, decision_reason = $3, decided_at = now() \
                 WHERE proposal_id = $1 AND status = 'pending'",
            )
            .bind(req.proposal_id)
            .bind(req.actor_id)
            .bind(&req.reason)
            .execute(&mut *tx)
            .await?;
            record_intervention(
                &mut tx,
                Uuid::new_v4(),
                req.project_id,
                "veto",
                Some(req.proposal_id),
                None,
                req.actor_id,
                &req.reason,
                Some(chrono::Utc::now()),
            )
            .await?;
            tx.commit().await?;
            Ok(VetoOutcome::ProposalClosed)
        }
        other => Err(SupervisionError::NotFound(format!(
            "proposal {} is not vetoable (status '{other}')",
            req.proposal_id
        ))),
    }
}

/// Decides one red-line approval request. The pending → decided transition is
/// a single guarded UPDATE, so a repeat decision loses the race and surfaces
/// `AlreadyDecided`. `Approve` executes the stored command as the one red-line
/// exception channel (`Human` + `RedlineApproval`); `Reject` vetoes the
/// proposal and tells the feed.
pub async fn decide_approval(
    pool: &PgPool,
    executor: &PgCommandExecutor,
    req: DecideRequest,
) -> Result<DecisionOutcome, SupervisionError> {
    let (proposal_id, project_id, analysis_run_id, command_type, proposed_data): (
        ProposalId,
        ProjectId,
        Uuid,
        String,
        serde_json::Value,
    ) = sqlx::query_as(
        "SELECT p.id, p.project_id, p.analysis_run_id, p.command_type, p.proposed_data \
         FROM governance.approval_request a \
         JOIN ai.change_proposal p ON p.id = a.proposal_id \
         WHERE a.id = $1",
    )
    .bind(req.approval_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        SupervisionError::NotFound(format!("approval request {} not found", req.approval_id))
    })?;

    // Atomic pending → decided: the loser of a race (or any repeat) hits
    // rows_affected == 0 and gets AlreadyDecided instead of a double execution.
    let decided = sqlx::query(
        "UPDATE governance.approval_request \
         SET status = $2, decided_by = $3, decision_reason = $4, decided_at = now() \
         WHERE id = $1 AND status = 'pending'",
    )
    .bind(req.approval_id)
    .bind(match req.decision {
        Decision::Approve => "approved",
        Decision::Reject => "rejected",
    })
    .bind(req.actor_id)
    .bind(&req.reason)
    .execute(pool)
    .await?
    .rows_affected();
    if decided == 0 {
        return Err(CommandError::AlreadyDecided { approval_id: req.approval_id }.into());
    }

    match req.decision {
        Decision::Reject => {
            sqlx::query("UPDATE ai.change_proposal SET status = 'vetoed' WHERE id = $1")
                .bind(proposal_id)
                .execute(pool)
                .await?;
            let mut tx = pool.begin().await?;
            notify(
                &mut tx,
                project_id,
                "veto_applied",
                json!({
                    "proposal_id": proposal_id,
                    "command_type": command_type,
                    "decision": "rejected",
                    "reason": req.reason,
                }),
            )
            .await?;
            tx.commit().await?;
            let _ = crate::project_import::advance_import_after_decision(
                pool,
                analysis_run_id,
                &crate::actor::AuditActor::principal(req.actor_id),
            )
            .await;
            Ok(DecisionOutcome::Rejected)
        }
        Decision::Approve => {
            // The executor's red-line exception: a human actor coming through
            // the RedlineApproval channel may execute the red line.
            let command: ProjectCommand = serde_json::from_value(proposed_data).map_err(|e| {
                CommandError::Validation(format!("stored proposal command is unreadable: {e}"))
            })?;
            let receipt = executor
                .execute(
                    command,
                    ExecutionContext {
                        project_id,
                        actor_id: req.actor_id,
                        actor_kind: ActorKind::Human,
                        source: CommandSource::RedlineApproval { approval_id: req.approval_id },
                    },
                )
                .await;
            match receipt {
                Ok(receipt) => {
                    sqlx::query(
                        "UPDATE ai.change_proposal \
                         SET status = 'executed', executed_at = now(), executed_audit_id = $2 \
                         WHERE id = $1",
                    )
                    .bind(proposal_id)
                    .bind(receipt.audit_id)
                    .execute(pool)
                    .await?;
                    let _ = crate::project_import::advance_import_after_decision(
                        pool,
                        analysis_run_id,
                        &crate::actor::AuditActor::principal(req.actor_id),
                    )
                    .await;
                    Ok(DecisionOutcome::Executed { receipt })
                }
                // The human approved, but execution failed (target gone or
                // moved on): the proposal fails, the error surfaces.
                Err(err) => {
                    sqlx::query("UPDATE ai.change_proposal SET status = 'failed' WHERE id = $1")
                        .bind(proposal_id)
                        .execute(pool)
                        .await?;
                    let _ = crate::project_import::advance_import_after_decision(
                        pool,
                        analysis_run_id,
                        &crate::actor::AuditActor::principal(req.actor_id),
                    )
                    .await;
                    Err(SupervisionError::Command(err))
                }
            }
        }
    }
}

/// The brake: stops the AI from claiming new jobs by flipping the project to
/// `controlled` (the Task 10 claim filters on the mode), recording the
/// intervention and a `project_frozen` notification.
pub async fn freeze(
    pool: &PgPool,
    project_id: ProjectId,
    actor_id: PrincipalId,
    reason: &str,
) -> Result<(), SupervisionError> {
    set_automation_mode(pool, project_id, actor_id, reason, "controlled", "freeze", "project_frozen")
        .await
}

/// Lifts the brake: back to `ai_lead`, with intervention('unfreeze') and a
/// `project_unfrozen` notification.
pub async fn unfreeze(
    pool: &PgPool,
    project_id: ProjectId,
    actor_id: PrincipalId,
    reason: &str,
) -> Result<(), SupervisionError> {
    set_automation_mode(pool, project_id, actor_id, reason, "ai_lead", "unfreeze", "project_unfrozen")
        .await
}

/// One mode flip inside one transaction: config UPDATE, intervention row,
/// notification row — or nothing.
async fn set_automation_mode(
    pool: &PgPool,
    project_id: ProjectId,
    actor_id: PrincipalId,
    reason: &str,
    automation_mode: &str,
    intervention_type: &str,
    notification_kind: &str,
) -> Result<(), SupervisionError> {
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE ai.project_ai_config SET automation_mode = $2, updated_at = now() \
         WHERE project_id = $1",
    )
    .bind(project_id)
    .bind(automation_mode)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(SupervisionError::NotFound(format!(
            "project {project_id} has no AI config"
        )));
    }
    record_intervention(
        &mut tx,
        Uuid::new_v4(),
        project_id,
        intervention_type,
        None,
        None,
        actor_id,
        reason,
        Some(chrono::Utc::now()),
    )
    .await?;
    notify(
        &mut tx,
        project_id,
        notification_kind,
        json!({ "reason": reason, "automation_mode": automation_mode }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// The supervision desk's notification feed for one project, newest first,
/// with `since` as an exclusive cursor (`created_at > since`) and a bounded
/// limit.
pub async fn notifications_feed(
    pool: &PgPool,
    project_id: ProjectId,
    since: Option<chrono::DateTime<chrono::Utc>>,
    limit: i64,
) -> Result<Vec<NotificationRow>, sqlx::Error> {
    let limit = limit.clamp(1, 500);
    sqlx::query_as(
        "SELECT id, kind, payload, veto_window_until, created_at \
         FROM integration.notification \
         WHERE project_id = $1 AND ($2 IS NULL OR created_at > $2) \
         ORDER BY created_at DESC \
         LIMIT $3",
    )
    .bind(project_id)
    .bind(since)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// One `governance.human_intervention` row — the durable record of a human
/// power being exercised. `executed_at` is `Some(now)` for interventions that
/// took effect (veto applied, freeze flipped) and `None` for recorded-but-not-
/// executed outcomes (`rollback_failed`: the refusal is the record; nothing
/// was executed).
#[allow(clippy::too_many_arguments)]
async fn record_intervention(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    intervention_id: Uuid,
    project_id: ProjectId,
    intervention_type: &str,
    target_proposal_id: Option<ProposalId>,
    target_command_id: Option<AuditId>,
    actor_id: PrincipalId,
    reason: &str,
    executed_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.human_intervention \
         (id, project_id, intervention_type, target_proposal_id, target_command_id, actor_id, reason, executed_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(intervention_id)
    .bind(project_id)
    .bind(intervention_type)
    .bind(target_proposal_id)
    .bind(target_command_id)
    .bind(actor_id)
    .bind(reason)
    .bind(executed_at)
    .execute(&mut **tx)
    .await
    .map(|_| ())
}

/// One `integration.notification` for the feed. `source_proposal_id` stays
/// NULL: the column is UNIQUE per proposal (Task 14 already claims it with the
/// proposal's own change_applied / redline_pending row), so the proposal
/// identity rides in the payload instead.
async fn notify(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: ProjectId,
    kind: &str,
    payload: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO integration.notification (project_id, source_proposal_id, kind, payload) \
         VALUES ($1, NULL, $2, $3)",
    )
    .bind(project_id)
    .bind(kind)
    .bind(payload)
    .execute(&mut **tx)
    .await
    .map(|_| ())
}
