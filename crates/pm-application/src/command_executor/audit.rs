use super::*;

/// Writes one `governance.audit_log` row and returns its id. `source` is the
/// `(source_kind, source_ref)` identity of the write (see [`audit_source`] for
/// command executions; compensations pass `("rollback", original audit id)`).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_audit(
    tx: &mut Tx,
    ctx: &ExecutionContext,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    before_data: &Value,
    after_data: &Value,
    compensation: Option<&CompensationSpec>,
    source: (&'static str, Option<Uuid>),
) -> Result<AuditId, CommandError> {
    let analysis_run_id = match &ctx.source {
        CommandSource::AiProposal { run_id, .. } => Some(*run_id),
        _ => None,
    };
    let compensation = compensation.map(|spec| serde_json::to_value(spec).expect("CompensationSpec serializes"));
    sqlx::query_scalar(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, action, target_type, target_id, before_data, after_data, compensation_command, analysis_run_id, source_kind, source_ref, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(ctx.project_id)
    .bind(ctx.actor_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(before_data)
    .bind(after_data)
    .bind(compensation)
    .bind(analysis_run_id)
    .bind(source.0)
    .bind(source.1)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_err)
}


/// The audit identity of a command execution: `(source_kind, source_ref)` —
/// the join anchor Task 15's approval flow reads (`analysis_run_id` keeps
/// carrying the AiProposal run id as before).
pub(crate) fn audit_source(ctx: &ExecutionContext) -> (&'static str, Option<Uuid>) {
    match &ctx.source {
        CommandSource::AiProposal { proposal_id, .. } => ("ai_proposal", Some(*proposal_id)),
        CommandSource::HumanIntervention { intervention_id } => {
            ("human_intervention", Some(*intervention_id))
        }
        CommandSource::RedlineApproval { approval_id } => ("redline_approval", Some(*approval_id)),
        CommandSource::Bootstrap => ("bootstrap", None),
        CommandSource::System => ("system", None),
    }
}


/// Binds a snapshot field value with the right Postgres representation. SQLx
/// sends `serde_json::Value` as JSONB, so a plain JSON string bound into a
/// TEXT column would store its quoted JSON text (`"backlog"`); JSON scalars
/// therefore bind as their SQL scalar types, only objects/arrays as JSONB.
pub(crate) fn bind_field<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    value: &'q Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match value {
        Value::String(text) => query.bind(text.clone()),
        Value::Bool(flag) => query.bind(*flag),
        Value::Number(number) => {
            if let Some(int) = number.as_i64() {
                query.bind(int)
            } else {
                query.bind(number.as_f64().unwrap_or(0.0))
            }
        }
        Value::Null => query.bind(Option::<String>::None),
        object_or_array => query.bind(object_or_array.clone()),
    }
}


