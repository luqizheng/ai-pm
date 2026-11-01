use super::*;

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
pub(crate) async fn record_intervention(
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
         (id, project_id, intervention_type, target_proposal_id, target_command_id, actor_id, reason, executed_at, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
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
pub(crate) async fn notify(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: ProjectId,
    kind: &str,
    payload: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO integration.notification \
         (project_id, source_proposal_id, kind, payload, tenant_id) \
         VALUES ($1, NULL, $2, $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(kind)
    .bind(payload)
    .execute(&mut **tx)
    .await
    .map(|_| ())
}

