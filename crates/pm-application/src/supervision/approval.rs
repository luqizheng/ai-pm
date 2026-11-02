use super::*;

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
    // T-088：优先用认证调用人的完整审计身份；缺省回退为仅执行主体（真人三列 NULL，不猜）。
    let actor = req
        .actor
        .clone()
        .unwrap_or_else(|| crate::actor::AuditActor::principal(req.actor_id));
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
                &actor,
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
                        actor: actor.clone(),
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
                        &actor,
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
                        &actor,
                    )
                    .await;
                    Err(SupervisionError::Command(err))
                }
            }
        }
    }
}


