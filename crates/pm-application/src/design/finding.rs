//! 设计评审问题（review_finding）登记与关闭。

use super::*;

pub async fn list_review_findings(
    pool: &PgPool,
    tenant_id: Uuid,
    revision_id: Uuid,
) -> Result<Vec<ReviewFindingView>, PmWriteError> {
    sqlx::query_as(
        "SELECT id, design_revision_id, finding_kind, severity, body, status, \
                resolution_note, raised_by, resolved_by, created_at, resolved_at \
         FROM design.review_finding \
         WHERE tenant_id = $1 AND design_revision_id = $2 \
         ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .bind(revision_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

pub async fn add_review_finding(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
    req: AddFindingRequest,
    actor: &AuditActor,
) -> Result<ReviewFindingView, PmWriteError> {
    let kind = req.finding_kind.as_deref().unwrap_or("issue");
    if !["issue", "question", "suggestion"].contains(&kind) {
        return Err(PmWriteError::Validation("invalid finding_kind"));
    }
    let severity = req.severity.as_deref().unwrap_or("warning");
    if !["info", "warning", "blocker"].contains(&severity) {
        return Err(PmWriteError::Validation("invalid severity"));
    }
    let body = req.body.trim().to_string();
    if body.is_empty() {
        return Err(PmWriteError::Validation("body is required"));
    }
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    let (_, state, _, _) = lock_design_revision(&mut tx, tenant_id, design_id, revision_id).await?;
    let parsed = parse_state(&state)?;
    if !matches!(
        parsed,
        DesignRevisionState::Draft | DesignRevisionState::PendingConfirmation
    ) {
        return Err(PmWriteError::Conflict(
            "findings can only be raised on draft or pending_confirmation revisions",
        ));
    }
    let finding_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.review_finding \
         (id, tenant_id, project_id, design_id, design_revision_id, finding_kind, severity, \
          body, raised_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(finding_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(design_id)
    .bind(revision_id)
    .bind(kind)
    .bind(severity)
    .bind(&body)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "review_finding.raised",
        "review_finding",
        Some(finding_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    sqlx::query_as(
        "SELECT id, design_revision_id, finding_kind, severity, body, status, \
                resolution_note, raised_by, resolved_by, created_at, resolved_at \
         FROM design.review_finding WHERE id = $1 AND tenant_id = $2",
    )
    .bind(finding_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 问题关闭：open → acknowledged → resolved（SC-05 审批前置：blocker 未关不许批准）。
pub async fn resolve_review_finding(
    pool: &PgPool,
    tenant_id: Uuid,
    finding_id: Uuid,
    req: ResolveFindingRequest,
    actor: &AuditActor,
) -> Result<ReviewFindingView, PmWriteError> {
    if !["acknowledged", "resolved"].contains(&req.status.as_str()) {
        return Err(PmWriteError::Validation(
            "status must be acknowledged or resolved",
        ));
    }
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, status, severity): (Uuid, String, String) = sqlx::query_as(
        "SELECT project_id, status, severity FROM design.review_finding \
         WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(finding_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("finding not found"))?;
    let legal = matches!(
        (status.as_str(), req.status.as_str()),
        ("open", "acknowledged") | ("open", "resolved") | ("acknowledged", "resolved")
    );
    if !legal {
        return Err(PmWriteError::Conflict("illegal finding status transition"));
    }
    if req.status == "resolved" && severity == "blocker" && req.resolution_note.is_none() {
        return Err(PmWriteError::Validation(
            "resolving a blocker finding requires a resolution note",
        ));
    }
    sqlx::query(
        "UPDATE design.review_finding \
         SET status = $1, resolution_note = COALESCE($2, resolution_note), \
             resolved_by = $3, resolved_at = CASE WHEN $1 = 'resolved' THEN now() ELSE resolved_at END \
         WHERE id = $4 AND tenant_id = $5",
    )
    .bind(&req.status)
    .bind(req.resolution_note.clone())
    .bind(actor.principal_id)
    .bind(finding_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "review_finding.closed",
        "review_finding",
        Some(finding_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    sqlx::query_as(
        "SELECT id, design_revision_id, finding_kind, severity, body, status, \
                resolution_note, raised_by, resolved_by, created_at, resolved_at \
         FROM design.review_finding WHERE id = $1 AND tenant_id = $2",
    )
    .bind(finding_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}
