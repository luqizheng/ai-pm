use super::*;

/// Vetoes one proposal: rolls an executed command back, or closes a
/// pre-execution (`redline_pending` / `escalated`) proposal. See the module
/// docs for the full branch table.
pub async fn veto(
    pool: &PgPool,
    executor: &PgCommandExecutor,
    req: VetoRequest,
) -> Result<VetoOutcome, SupervisionError> {
    // T-088：优先用认证调用人的完整审计身份；缺省回退为仅执行主体（真人三列 NULL，不猜）。
    let actor = req
        .actor
        .clone()
        .unwrap_or_else(|| crate::actor::AuditActor::principal(req.actor_id));
    let (status, executed_audit_id, command_type): (String, Option<Uuid>, String) = sqlx::query_as(
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
                actor: actor.clone(),
                actor_kind: ActorKind::Human,
                source: CommandSource::HumanIntervention { intervention_id },
            };
            match executor
                .compensate(executed_audit_id, &req.reason, ctx)
                .await
            {
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
                    Ok(VetoOutcome::RolledBack {
                        new_audit_id: receipt.audit_id,
                    })
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
