use super::*;

/// The brake: stops the AI from claiming new jobs by flipping the project to
/// `controlled` (the Task 10 claim filters on the mode), recording the
/// intervention and a `project_frozen` notification.
pub async fn freeze(
    pool: &PgPool,
    project_id: ProjectId,
    actor_id: PrincipalId,
    reason: &str,
) -> Result<(), SupervisionError> {
    set_automation_mode(
        pool,
        project_id,
        actor_id,
        reason,
        "controlled",
        "freeze",
        "project_frozen",
    )
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
    set_automation_mode(
        pool,
        project_id,
        actor_id,
        reason,
        "ai_lead",
        "unfreeze",
        "project_unfrozen",
    )
    .await
}

/// One mode flip inside one transaction: config UPDATE, intervention row,
/// notification row — or nothing.
pub(crate) async fn set_automation_mode(
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
