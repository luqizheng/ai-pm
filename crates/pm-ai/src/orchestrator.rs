//! End-to-end main-chain orchestrator (Task 14, 主稿 §14.2).
//!
//! [`Orchestrator::handle_ai_job`] drives one claimed `ai_analysis` job
//! through the whole vertical slice, in the fixed order mandated by the
//! plan:
//!
//! 1. [`context::start_run`] — `AlreadyDone` means an idempotent
//!    redelivery: return `Ok(Completed)` (the job completes, it does not
//!    fail).
//! 2. [`context::build`] — `NotFound` (a dangling or foreign ref) fails
//!    the run with `context_not_found` and fails the job itself (queued
//!    again with backoff), returning `Ok(RetryScheduled)` so the caller
//!    knows NOT to complete the job — a `complete` here would erase the
//!    scheduled retry. When the job is re-claimed after its backoff, the
//!    retry opens a *fresh* run row per the Task 12 rulings.
//! 3. [`context::save_snapshot`].
//! 4. [`skill_router::route`] — no skills for this event type → the run
//!    completes with an empty self-review and the event becomes
//!    `completed_no_change`.
//! 5. [`AnalysisModel::analyze`] — a [`ModelError`] fails the run with
//!    the coarse code (`model_timeout` / `model_unavailable` /
//!    `model_bad_output`) and surfaces as
//!    [`OrchestratorError::Model`] so the *worker* fails the job with
//!    backoff.
//! 6. Per-proposal validation (handler exists + target object exists in
//!    this project; a create has no target): every proposal becomes an
//!    `ai.change_proposal` row, `proposed` when valid, `failed` when not.
//! 7. [`self_review::review_proposals`] routes every proposal (outcome
//!    index ↔ `proposal_seq`):
//!     * `Auto` and `PolicyEngine::evaluate` says `AutoApply` → real
//!       execution through [`PgCommandExecutor`] as the AI principal,
//!       `status='executed'` with audit linkage, plus a
//!       `change_applied` notification carrying the policy's veto
//!       window; `RedLineBlocked` from the executor is the double-guard
//!       branch (`status='failed'` + `redline_violation_blocked`
//!       notification); any other [`CommandError`] fails the proposal
//!       and the chain continues.
//!     * `RequirePreConfirmation` → `governance.approval_request`
//!       (pending, with the required role), proposal
//!       `redline_pending`, `redline_pending` notification.
//!     * `Escalate` → proposal `escalated`, `escalated` notification.
//!     * A validation-failed proposal skips routing entirely and stays
//!       `failed` (an unhandleable or dangling command must never reach
//!       the executor or collect an approval request).
//! 8. Finish: [`context::finish_run`] with the model's result and the
//!    serialized self-review, the event becomes `applied` (at least one
//!    execution) or `needs_clarification` (proposals but none executed;
//!    a proposal-free analysis means nothing changed), and one
//!    `decision` `ai.agent_feedback` row is addressed to the AI
//!    principal.

use std::sync::Arc;

use pm_application::{
    ActorKind, CommandSource, ExecutionContext, PgCommandExecutor, PolicyContext, PolicyEngine,
};
use pm_domain::{
    CommandError, PolicyDecision, PrincipalId, ProjectCommand, ProjectId, Proposal, ProposalId,
    RunId,
};
use pm_infrastructure::job_queue::{self, ClaimedJob};
use serde::Serialize;

use crate::model::{AnalysisModel, ModelError};
use crate::{context, self_review, skill_router};

/// Attempts ceiling handed to [`job_queue::fail`]; matches the schema
/// default of `integration.job.max_attempts`.
const MAX_JOB_ATTEMPTS: i32 = 5;
/// Backoff base (seconds) for job-level retries; the queue multiplies it
/// by 2^attempts.
const JOB_BACKOFF_BASE_SECS: i64 = 5;

/// Orchestrates the AI analysis main chain for one claimed job.
pub struct Orchestrator {
    pub pool: sqlx::PgPool,
    pub model: Arc<dyn AnalysisModel>,
    pub policy: Arc<PolicyEngine>,
    pub executor: Arc<PgCommandExecutor>,
}

/// Errors surfaced by [`Orchestrator::handle_ai_job`].
///
/// `RunAlreadyDone` is part of the interface for callers that want to
/// distinguish a redelivery; `handle_ai_job` itself maps an
/// `AlreadyDone` start onto `Ok(HandleOutcome::Completed)` — an
/// idempotent redelivery is a normal completion, not a job failure.
#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("analysis run already done for this event")]
    RunAlreadyDone,
    #[error("analysis model failure: {0}")]
    Model(#[from] ModelError),
    #[error("orchestrator database error: {0}")]
    Db(#[from] sqlx::Error),
}

/// What the caller (the worker loop) must do with the claimed job after
/// [`Orchestrator::handle_ai_job`] returned `Ok`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleOutcome {
    /// The chain finished normally (or the event was an idempotent
    /// redelivery): the caller marks the job completed.
    Completed,
    /// The orchestrator already failed the job itself (`job_queue::fail`:
    /// back to `queued` with backoff, attempts incremented). The caller
    /// must leave the job alone — completing it here would erase the
    /// scheduled retry.
    RetryScheduled,
}

impl Orchestrator {
    /// Runs the full main chain for `job`; see the module docs for the
    /// fixed step order and each branch's side effects. The returned
    /// [`HandleOutcome`] tells the caller whether to complete the job
    /// (`Completed`) or hands-off because the orchestrator already
    /// scheduled its retry (`RetryScheduled`).
    pub async fn handle_ai_job(
        &self,
        job: &ClaimedJob,
    ) -> Result<HandleOutcome, OrchestratorError> {
        // Step 1: run lifecycle. AlreadyDone = idempotent redelivery.
        let run_id = match context::start_run(&self.pool, job.event_id).await {
            Ok(run_id) => run_id,
            Err(context::ContextError::AlreadyDone(_)) => {
                tracing::info!(
                    event_id = %job.event_id,
                    "analysis already completed; redelivery is a no-op"
                );
                return Ok(HandleOutcome::Completed);
            }
            Err(context::ContextError::EventNotFound(id)) => {
                // Unreachable after a claim (the job row FKs the event);
                // fail the job defensively instead of wedging it running.
                tracing::error!(event_id = %id, "claimed job references a missing event");
                job_queue::fail(
                    &self.pool,
                    job.job_id,
                    "event_not_found",
                    MAX_JOB_ATTEMPTS,
                    JOB_BACKOFF_BASE_SECS,
                )
                .await?;
                return Ok(HandleOutcome::RetryScheduled);
            }
            Err(context::ContextError::Db(e)) => return Err(OrchestratorError::Db(e)),
            // start_run never returns NotFound; the arm keeps the match
            // exhaustive without a wildcard.
            Err(context::ContextError::NotFound { .. }) => unreachable!("start_run has no refs"),
        };

        // Step 2: context. NotFound fails the run and the job (queued
        // again with backoff); `RetryScheduled` tells the caller the job
        // was already handled and must not be completed. The eventual
        // retry — a fresh claim after backoff — opens a fresh run row
        // (Task 12 ruling 1).
        let ctx = match context::build(&self.pool, job.event_id).await {
            Ok(ctx) => ctx,
            Err(
                err @ (context::ContextError::NotFound { .. }
                | context::ContextError::EventNotFound(_)),
            ) => {
                tracing::warn!(event_id = %job.event_id, error = %err, "context build failed");
                context::fail_run(&self.pool, run_id, "context_not_found").await?;
                job_queue::fail(
                    &self.pool,
                    job.job_id,
                    "context_not_found",
                    MAX_JOB_ATTEMPTS,
                    JOB_BACKOFF_BASE_SECS,
                )
                .await?;
                return Ok(HandleOutcome::RetryScheduled);
            }
            // build never returns AlreadyDone; same defensive treatment.
            Err(err @ context::ContextError::AlreadyDone(_)) => {
                tracing::error!(event_id = %job.event_id, error = %err, "unexpected build error");
                context::fail_run(&self.pool, run_id, "context_not_found").await?;
                job_queue::fail(
                    &self.pool,
                    job.job_id,
                    "context_not_found",
                    MAX_JOB_ATTEMPTS,
                    JOB_BACKOFF_BASE_SECS,
                )
                .await?;
                return Ok(HandleOutcome::RetryScheduled);
            }
            Err(context::ContextError::Db(e)) => return Err(OrchestratorError::Db(e)),
        };

        // Step 3: freeze the assembled context on the run row.
        context::save_snapshot(&self.pool, run_id, &ctx).await?;

        // Step 4: routing. No skills → nothing can change.
        let skills = skill_router::route(&ctx.event_type);
        if skills.is_empty() {
            tracing::info!(event_id = %job.event_id, event_type = %ctx.event_type, "no skills routed");
            let result = serde_json::json!({ "summary": "no skills routed" });
            context::finish_run(
                &self.pool,
                run_id,
                &result,
                &self_review::self_review_json(&[]),
            )
            .await?;
            sqlx::query(
                "UPDATE integration.inbound_event SET status = 'completed_no_change' WHERE id = $1",
            )
            .bind(job.event_id)
            .execute(&self.pool)
            .await?;
            return Ok(HandleOutcome::Completed);
        }

        // Step 5: analysis. A model failure is terminal for this run row
        // and surfaces to the worker (which fails the job with backoff).
        let result = match self.model.analyze(&ctx, &skills).await {
            Ok(result) => result,
            Err(err) => {
                let code = model_failure_code(&err);
                tracing::warn!(event_id = %job.event_id, code, error = %err, "model analysis failed");
                context::fail_run(&self.pool, run_id, code).await?;
                return Err(OrchestratorError::Model(err));
            }
        };

        // The AI principal acts for every execution (V2 trust model).
        let ai_principal_id: PrincipalId = sqlx::query_scalar(
            "SELECT ai_principal_id FROM ai.project_ai_config WHERE project_id = $1",
        )
        .bind(ctx.project_id)
        .fetch_one(&self.pool)
        .await?;

        // Step 6: validate and persist every proposal.
        // Blast radius is the total number of affected work items, not the
        // number of proposal envelopes: a single CreateTasks may contain up
        // to MAX_BATCH_ITEMS tasks. impact_count() reads only collection
        // cardinality, never model-controlled payload content.
        let proposal_count = result.proposals.len();
        let blast_radius = result
            .proposals
            .iter()
            .map(|proposal| proposal.command.impact_count())
            .fold(0usize, |total, count| total.saturating_add(count));
        let mut proposal_ids: Vec<ProposalId> = Vec::with_capacity(proposal_count);
        let mut routable: Vec<bool> = Vec::with_capacity(proposal_count);
        for (seq, proposal) in result.proposals.iter().enumerate() {
            // `rollback_command` is registered in the handler map, but its
            // entry point is `compensate`, never `execute` — a rollback is
            // a human-intervention command, not an executable AI proposal.
            let executable = self.executor.has_handler(proposal.command.kind())
                && proposal.command.kind() != "rollback_command";
            let passes = executable
                && self
                    .target_exists(ctx.project_id, &proposal.command)
                    .await?;
            let proposal_id = self
                .insert_proposal(ctx.project_id, run_id, seq, proposal, passes)
                .await?;
            proposal_ids.push(proposal_id);
            routable.push(passes);
        }

        // Step 7: self-review gate, then per-proposal routing.
        let outcomes = self_review::review_proposals(&self.policy, &result.proposals, blast_radius);
        let mut executed_count = 0usize;
        for (seq, (proposal, outcome)) in result.proposals.iter().zip(&outcomes).enumerate() {
            if !routable[seq] {
                // Already `failed` at validation: never executed, never
                // approved, never notified.
                continue;
            }
            let proposal_id = proposal_ids[seq];
            match outcome {
                self_review::ProposalOutcome::Auto => {
                    let decision = self.policy.evaluate(
                        &proposal.command,
                        &PolicyContext {
                            confidence: proposal.confidence,
                            blast_radius,
                            unresolved_conflict: false,
                        },
                    );
                    match decision {
                        PolicyDecision::AutoApply => {
                            executed_count += self
                                .execute_proposal(
                                    ctx.project_id,
                                    run_id,
                                    proposal_id,
                                    ai_principal_id,
                                    &proposal.command,
                                )
                                .await?;
                        }
                        // The gate said Auto but the policy is stricter
                        // (defense in depth: same engine, second opinion).
                        PolicyDecision::Escalate { reason } => {
                            self.route_escalated(
                                ctx.project_id,
                                proposal_id,
                                &proposal.command,
                                &reason,
                            )
                            .await?;
                        }
                        PolicyDecision::RequirePreConfirmation { role } => {
                            self.route_redline(
                                ctx.project_id,
                                proposal_id,
                                &proposal.command,
                                &role,
                            )
                            .await?;
                        }
                        PolicyDecision::Deny { .. } => {
                            self.mark_proposal_failed(proposal_id).await?;
                        }
                    }
                }
                self_review::ProposalOutcome::RequirePreConfirmation { role } => {
                    self.route_redline(ctx.project_id, proposal_id, &proposal.command, role)
                        .await?;
                }
                self_review::ProposalOutcome::Escalate { reason } => {
                    self.route_escalated(ctx.project_id, proposal_id, &proposal.command, reason)
                        .await?;
                }
            }
        }

        // Step 8: close the run, the event and the feedback loop.
        let result_json = serde_json::to_value(&result).expect("AnalysisResult serializes");
        context::finish_run(
            &self.pool,
            run_id,
            &result_json,
            &self_review::self_review_json(&outcomes),
        )
        .await?;

        let event_status = if executed_count > 0 {
            "applied"
        } else if !result.proposals.is_empty() {
            "needs_clarification"
        } else {
            // Skills ran but the model proposed nothing: no change.
            "completed_no_change"
        };
        sqlx::query("UPDATE integration.inbound_event SET status = $2 WHERE id = $1")
            .bind(job.event_id)
            .bind(event_status)
            .execute(&self.pool)
            .await?;

        // The decision feedback is addressed to the principal that raised
        // the event (architecture §14.2 sequence: the dev agent reads the
        // authorized feedback addressed to it), not back to the AI itself.
        let event_actor_id: PrincipalId =
            sqlx::query_scalar("SELECT actor_id FROM integration.inbound_event WHERE id = $1")
                .bind(job.event_id)
                .fetch_one(&self.pool)
                .await?;

        sqlx::query(
            "INSERT INTO ai.agent_feedback \
             (project_id, recipient_id, related_event_id, feedback_type, content, tenant_id) \
             VALUES ($1, $2, $3, 'decision', $4, \
                     (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(ctx.project_id)
        .bind(event_actor_id)
        .bind(job.event_id)
        .bind(serde_json::json!({
            "summary": result.summary,
            "proposal_count": blast_radius,
            "executed_count": executed_count,
        }))
        .execute(&self.pool)
        .await?;

        tracing::info!(
            event_id = %job.event_id,
            run_id = %run_id,
            proposals = blast_radius,
            executed = executed_count,
            event_status,
            "analysis chain complete"
        );
        Ok(HandleOutcome::Completed)
    }

    /// Step 6 existence check: the command's target row must exist and
    /// belong to this project. A create has no target object (the
    /// executor checks the command's own project instead); a
    /// handler-less command never gets here with `true` because the
    /// caller also requires `has_handler`.
    async fn target_exists(
        &self,
        project_id: ProjectId,
        command: &ProjectCommand,
    ) -> Result<bool, sqlx::Error> {
        let exists = match command {
            ProjectCommand::CreateTask { .. } | ProjectCommand::ChangePolicy { .. } => true,
            ProjectCommand::CreateMilestones { .. }
            | ProjectCommand::CreateRequirements { .. }
            | ProjectCommand::CreateTasks { .. }
            | ProjectCommand::WriteDocuments { .. } => true,
            ProjectCommand::UpdateTaskStatus { task_id, .. } => {
                sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM delivery.task \
                     WHERE id = $1 AND project_id = $2)",
                )
                .bind(task_id)
                .bind(project_id)
                .fetch_one(&self.pool)
                .await?
            }
            ProjectCommand::AdjustMilestonePlan { milestone_id, .. }
            | ProjectCommand::SignAcceptance { milestone_id, .. } => {
                sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM delivery.milestone \
                     WHERE id = $1 AND project_id = $2)",
                )
                .bind(milestone_id)
                .bind(project_id)
                .fetch_one(&self.pool)
                .await?
            }
            ProjectCommand::ApproveRequirementRevision { requirement_id, .. } => {
                sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM delivery.requirement \
                     WHERE id = $1 AND project_id = $2)",
                )
                .bind(requirement_id)
                .bind(project_id)
                .fetch_one(&self.pool)
                .await?
            }
            ProjectCommand::RollbackCommand {
                target_audit_id, ..
            } => {
                sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM governance.audit_log \
                     WHERE id = $1 AND project_id = $2)",
                )
                .bind(target_audit_id)
                .bind(project_id)
                .fetch_one(&self.pool)
                .await?
            }
        };
        Ok(exists)
    }

    /// Step 6 persistence: one `ai.change_proposal` row per proposal,
    /// `proposed` when it passed validation, `failed` when it did not.
    /// Returns the new proposal id.
    async fn insert_proposal(
        &self,
        project_id: ProjectId,
        run_id: RunId,
        seq: usize,
        proposal: &Proposal,
        passes: bool,
    ) -> Result<ProposalId, sqlx::Error> {
        let (target_type, target_id, expected_revision) = command_target(&proposal.command);
        let proposed_data =
            serde_json::to_value(&proposal.command).expect("ProjectCommand serializes");
        let evidence = serde_json::Value::Array(
            proposal
                .evidence
                .iter()
                .map(|r| serde_json::Value::String(r.clone()))
                .collect(),
        );
        let proposal_id = ProposalId::new_v4();
        sqlx::query(
            "INSERT INTO ai.change_proposal \
             (id, project_id, analysis_run_id, proposal_seq, target_type, target_id, \
              expected_revision, command_type, proposed_data, rationale, evidence, \
              risk_level, confidence, status, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, \
                     (SELECT tenant_id FROM core.project WHERE id = $2))",
        )
        .bind(proposal_id)
        .bind(project_id)
        .bind(run_id)
        .bind(seq as i32)
        .bind(target_type)
        .bind(target_id)
        .bind(expected_revision)
        .bind(proposal.command.kind())
        .bind(proposed_data)
        .bind(&proposal.rationale)
        .bind(evidence)
        .bind(enum_as_str(&proposal.risk_level))
        .bind(enum_as_str(&proposal.confidence))
        .bind(if passes { "proposed" } else { "failed" })
        .execute(&self.pool)
        .await?;
        if !passes {
            tracing::warn!(
                command_type = proposal.command.kind(),
                "proposal failed pre-execution validation (no handler or dangling target)"
            );
        }
        Ok(proposal_id)
    }

    /// Step 7 `Auto` + `AutoApply`: real execution as the AI principal.
    /// Returns 1 when the command executed, 0 otherwise.
    async fn execute_proposal(
        &self,
        project_id: ProjectId,
        run_id: RunId,
        proposal_id: ProposalId,
        actor_id: PrincipalId,
        command: &ProjectCommand,
    ) -> Result<usize, sqlx::Error> {
        let receipt = self
            .executor
            .execute(
                command.clone(),
                ExecutionContext {
                    project_id,
                    actor: pm_application::actor::AuditActor::principal(actor_id),
                    actor_kind: ActorKind::AiPm,
                    source: CommandSource::AiProposal {
                        run_id,
                        proposal_id,
                    },
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
                .execute(&self.pool)
                .await?;
                let veto_until = chrono::Utc::now() + self.policy.veto_window(command);
                sqlx::query(
                    "INSERT INTO integration.notification \
                     (project_id, source_proposal_id, kind, payload, veto_window_until, tenant_id) \
                     VALUES ($1, $2, 'change_applied', $3, $4, \
                             (SELECT tenant_id FROM core.project WHERE id = $1))",
                )
                .bind(project_id)
                .bind(proposal_id)
                .bind(serde_json::json!({
                    "proposal_id": proposal_id,
                    "command_type": command.kind(),
                    "audit_id": receipt.audit_id,
                }))
                .bind(veto_until)
                .execute(&self.pool)
                .await?;
                Ok(1)
            }
            // Double guard: the gate already routed red lines to
            // RequirePreConfirmation, so the executor's own red-line
            // rejection means something bypassed the gate.
            Err(CommandError::RedLineBlocked { command: kind }) => {
                tracing::error!(
                    command_type = kind,
                    "executor red-line double guard fired for an AiPm actor"
                );
                self.mark_proposal_failed(proposal_id).await?;
                self.notify(
                    project_id,
                    proposal_id,
                    "redline_violation_blocked",
                    serde_json::json!({
                        "proposal_id": proposal_id,
                        "command_type": command.kind(),
                    }),
                    None,
                )
                .await?;
                Ok(0)
            }
            // Any other execution error (revision conflict, target gone,
            // …) fails this proposal only; the chain continues.
            Err(err) => {
                tracing::warn!(
                    command_type = command.kind(),
                    error = %err,
                    "proposal execution failed"
                );
                self.mark_proposal_failed(proposal_id).await?;
                Ok(0)
            }
        }
    }

    /// Step 7 `RequirePreConfirmation`: park the proposal behind a human
    /// decision and tell the supervision desk.
    async fn route_redline(
        &self,
        project_id: ProjectId,
        proposal_id: ProposalId,
        command: &ProjectCommand,
        role: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO governance.approval_request \
             (project_id, proposal_id, required_role, status, tenant_id) \
             VALUES ($1, $2, $3, 'pending', \
                     (SELECT tenant_id FROM ai.change_proposal WHERE id = $2))",
        )
        .bind(project_id)
        .bind(proposal_id)
        .bind(role)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE ai.change_proposal SET status = 'redline_pending' WHERE id = $1")
            .bind(proposal_id)
            .execute(&self.pool)
            .await?;
        self.notify(
            project_id,
            proposal_id,
            "redline_pending",
            serde_json::json!({
                "proposal_id": proposal_id,
                "command_type": command.kind(),
                "required_role": role,
            }),
            None,
        )
        .await?;
        tracing::info!(
            command_type = command.kind(),
            required_role = role,
            "red line awaiting pre-confirmation"
        );
        Ok(())
    }

    /// Step 7 `Escalate`: mark the proposal for human review and notify.
    async fn route_escalated(
        &self,
        project_id: ProjectId,
        proposal_id: ProposalId,
        command: &ProjectCommand,
        reason: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE ai.change_proposal SET status = 'escalated' WHERE id = $1")
            .bind(proposal_id)
            .execute(&self.pool)
            .await?;
        self.notify(
            project_id,
            proposal_id,
            "escalated",
            serde_json::json!({
                "proposal_id": proposal_id,
                "command_type": command.kind(),
                "reason": reason,
            }),
            None,
        )
        .await?;
        tracing::info!(command_type = command.kind(), reason, "proposal escalated");
        Ok(())
    }

    async fn mark_proposal_failed(&self, proposal_id: ProposalId) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE ai.change_proposal SET status = 'failed' WHERE id = $1")
            .bind(proposal_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
    }

    /// One supervision-desk notification; `veto_window_until` only exists
    /// for applied changes.
    async fn notify(
        &self,
        project_id: ProjectId,
        proposal_id: ProposalId,
        kind: &str,
        payload: serde_json::Value,
        veto_window_until: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO integration.notification \
             (project_id, source_proposal_id, kind, payload, veto_window_until, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, \
                     (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(project_id)
        .bind(proposal_id)
        .bind(kind)
        .bind(payload)
        .bind(veto_window_until)
        .execute(&self.pool)
        .await
        .map(|_| ())
    }
}

/// Coarse failure code for a [`ModelError`], as stored on
/// `ai.analysis_run.failure_code`.
fn model_failure_code(err: &ModelError) -> &'static str {
    match err {
        ModelError::Timeout => "model_timeout",
        ModelError::Unavailable(_) => "model_unavailable",
        ModelError::BadOutput(_) => "model_bad_output",
        ModelError::Configuration(_) => "model_configuration",
    }
}

/// `(target_type, target_id, expected_revision)` derived from the
/// command: the audit-facing target of the command, `None` where the
/// command has none of that dimension (creates have no target object,
/// approvals/replaces carry no optimistic lock of their own here).
fn command_target(command: &ProjectCommand) -> (&'static str, Option<uuid::Uuid>, Option<i64>) {
    match command {
        ProjectCommand::CreateTask { .. } => ("task", None, None),
        ProjectCommand::UpdateTaskStatus {
            task_id,
            expected_revision,
            ..
        } => ("task", Some(*task_id), Some(*expected_revision)),
        ProjectCommand::ApproveRequirementRevision { requirement_id, .. } => {
            ("requirement", Some(*requirement_id), None)
        }
        ProjectCommand::AdjustMilestonePlan {
            milestone_id,
            expected_revision,
            ..
        } => ("milestone", Some(*milestone_id), Some(*expected_revision)),
        ProjectCommand::SignAcceptance {
            milestone_id,
            expected_revision,
            ..
        } => ("milestone", Some(*milestone_id), Some(*expected_revision)),
        ProjectCommand::ChangePolicy { project_id, .. } => ("project", Some(*project_id), None),
        ProjectCommand::RollbackCommand {
            target_audit_id, ..
        } => ("audit_log", Some(*target_audit_id), None),
        ProjectCommand::CreateMilestones { .. }
        | ProjectCommand::CreateRequirements { .. }
        | ProjectCommand::CreateTasks { .. }
        | ProjectCommand::WriteDocuments { .. } => ("project", None, None),
    }
}

/// The snake_case storage form of a `RiskLevel` / `Confidence` (both
/// derive `Serialize` with `rename_all = "snake_case"`).
fn enum_as_str(value: &impl Serialize) -> String {
    serde_json::to_value(value)
        .expect("plain fieldless enums serialize")
        .as_str()
        .expect("a fieldless enum serializes to a string")
        .to_string()
}
