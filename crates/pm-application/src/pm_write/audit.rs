use super::*;

/// 测试夹具：解析项目的操作者（三级回退）。生产路径不再调用——actor 必须由
/// 调用方显式传入（T-021a / `docs/governance/ai-pm-principal-identity-v1.md` Q3）。
#[cfg(test)]
pub async fn owner_actor(pool: &PgPool, project_id: Uuid) -> Result<Uuid, PmWriteError> {
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT p.id FROM core.project_member m \
         JOIN core.principal p ON p.id = m.principal_id \
         WHERE m.project_id = $1 AND m.role = 'owner' AND p.kind = 'human' AND p.is_active \
         LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    if let Some(actor) = owner {
        return Ok(actor);
    }
    let member: Option<Uuid> = sqlx::query_scalar(
        "SELECT p.id FROM core.project_member m \
         JOIN core.principal p ON p.id = m.principal_id \
         WHERE m.project_id = $1 AND p.kind = 'human' AND p.is_active \
         LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    if let Some(actor) = member {
        return Ok(actor);
    }
    sqlx::query_scalar(
        "SELECT id FROM core.principal WHERE kind = 'human' AND is_active ORDER BY created_at LIMIT 1",
    )
    .fetch_optional(pool)
    .await?
    .ok_or(PmWriteError::Validation("no human principal available as actor"))
}

pub(crate) async fn append_audit(
    tx: &mut sqlx::PgConnection,
    project_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (id, project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .execute(tx)
    .await?;
    Ok(())
}

/// 人工写路径的审计行（D2 契约变更需要 `after_data` 快照）。
pub(crate) async fn append_audit_with_after(
    tx: &mut sqlx::PgConnection,
    project_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    after: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (id, project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, after_data, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(after)
    .execute(tx)
    .await?;
    Ok(())
}

// ------------------------------------------------------- requirement revisions

/// 审批决策的审计行（action='approval.decide'，approve/reject 都落一条；
/// approve 的红线执行审计由 command_executor 另行落 redline 行）。
pub async fn record_decision_audit(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    approval_id: Uuid,
) -> Result<(), PmWriteError> {
    let mut tx = pool.begin().await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "approval.decide",
        "approval_request",
        Some(approval_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
