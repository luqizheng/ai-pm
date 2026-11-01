use super::*;

pub(crate) async fn project_code(pool: &PgPool, project_id: Uuid) -> Result<String, RepositoryError> {
    sqlx::query_scalar("SELECT code FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await?
        .ok_or(RepositoryError::ProjectNotFound)
}


pub(crate) async fn append_audit(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (id, project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, \
          action, target_type, target_id, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}


pub(crate) fn require_text(
    value: Option<String>,
    message: &'static str,
    max: usize,
) -> Result<String, RepositoryError> {
    let text = value.unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Err(RepositoryError::Validation(message));
    }
    if text.chars().count() > max {
        return Err(RepositoryError::Validation("value is too long"));
    }
    Ok(text)
}


pub(crate) fn optional_text(
    value: Option<String>,
    max: usize,
) -> Result<Option<String>, RepositoryError> {
    let text = value.unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Ok(None);
    }
    if text.chars().count() > max {
        return Err(RepositoryError::Validation("value is too long"));
    }
    Ok(Some(text))
}


pub(crate) fn normalize_role(raw: &str) -> Result<String, RepositoryError> {
    let role = raw.trim();
    if REPOSITORY_ROLES.contains(&role) {
        Ok(role.to_string())
    } else {
        Err(RepositoryError::InvalidRole)
    }
}


