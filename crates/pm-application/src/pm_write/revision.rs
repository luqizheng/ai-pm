use super::*;

/// 新建草稿修订：version 自增、content_hash = sha256("{rid}:{version}:{markdown}")。
pub async fn create_revision(
    pool: &PgPool,
    requirement_id: Uuid,
    req: CreateRevisionRequest,
    actor: &AuditActor,
) -> Result<RequirementRevisionRow, PmWriteError> {
    let markdown = req
        .content
        .and_then(|c| c.markdown)
        .map(|m| m.trim().to_string())
        .unwrap_or_default();
    if markdown.is_empty() {
        return Err(PmWriteError::Validation("content.markdown is required"));
    }
    let (project_id,): (Uuid,) =
        sqlx::query_as("SELECT project_id FROM delivery.requirement WHERE id = $1")
            .bind(requirement_id)
            .fetch_optional(pool)
            .await?
            .ok_or(PmWriteError::NotFound("not found"))?;

    let mut tx = pool.begin().await?;
    let version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(max(version), 0) + 1 FROM delivery.requirement_revision \
         WHERE requirement_id = $1",
    )
    .bind(requirement_id)
    .fetch_one(&mut *tx)
    .await?;

    let hash_input = format!("{requirement_id}:{version}:{markdown}");
    let content_hash = hex_sha256(hash_input.as_bytes());
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.requirement_revision \
         (id, requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, 'draft', $6, $7, \
                 (SELECT tenant_id FROM core.project WHERE id = $3))",
    )
    .bind(revision_id)
    .bind(requirement_id)
    .bind(project_id)
    .bind(version)
    .bind(json!({ "markdown": markdown }))
    .bind(&content_hash)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.propose_revision",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;

    let row = sqlx::query_as::<_, RequirementRevisionRow>(
        "SELECT id, requirement_id, project_id, version, content, state, content_hash, \
                created_by, approved_by, created_at \
         FROM delivery.requirement_revision WHERE id = $1",
    )
    .bind(revision_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}


/// 定稿：draft → approved；同需求其余 approved → superseded；requirement 指向
/// 新定稿、status='approved'、row_revision+1。非 draft（含不存在）一律 409。
pub async fn approve_revision(
    pool: &PgPool,
    requirement_id: Uuid,
    revision_id: Uuid,
    actor: &AuditActor,
) -> Result<RequirementRevisionRow, PmWriteError> {
    let mut tx = pool.begin().await?;
    let (project_id, state): (Uuid, String) = sqlx::query_as(
        "SELECT project_id, state FROM delivery.requirement_revision \
         WHERE id = $1 AND requirement_id = $2",
    )
    .bind(revision_id)
    .bind(requirement_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(PmWriteError::Conflict("not a draft revision"))?;
    if state != "draft" {
        return Err(PmWriteError::Conflict("not a draft revision"));
    }

    sqlx::query(
        "UPDATE delivery.requirement_revision SET state = 'approved', approved_by = $2 WHERE id = $1",
    )
    .bind(revision_id)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE delivery.requirement_revision SET state = 'superseded' \
         WHERE requirement_id = $1 AND state = 'approved' AND id <> $2",
    )
    .bind(requirement_id)
    .bind(revision_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE delivery.requirement \
         SET current_revision_id = $2, status = 'approved', row_revision = row_revision + 1 \
         WHERE id = $1",
    )
    .bind(requirement_id)
    .bind(revision_id)
    .execute(&mut *tx)
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.approve_revision",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;

    let row = sqlx::query_as::<_, RequirementRevisionRow>(
        "SELECT id, requirement_id, project_id, version, content, state, content_hash, \
                created_by, approved_by, created_at \
         FROM delivery.requirement_revision WHERE id = $1",
    )
    .bind(revision_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

// --------------------------------------------------------------------- tasks


