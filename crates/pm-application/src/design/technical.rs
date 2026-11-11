//! Technical Design（TD）与「设计 ↔ 需求」多对多关联。

use super::*;

pub async fn list_designs(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<DesignEntry>, PmWriteError> {
    sqlx::query_as(&format!(
        "{DESIGN_BY_ID} WHERE d.tenant_id = $1 AND d.project_id = $2 ORDER BY d.created_at DESC"
    ))
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

#[derive(Debug, Serialize)]
pub struct DesignDetail {
    pub design: DesignEntry,
    pub revisions: Vec<DesignRevisionView>,
    pub approvals: Vec<ApprovalView>,
}

pub async fn design_detail(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
) -> Result<DesignDetail, PmWriteError> {
    let design: DesignEntry = sqlx::query_as(&format!(
        "{DESIGN_BY_ID} WHERE d.tenant_id = $1 AND d.id = $2"
    ))
    .bind(tenant_id)
    .bind(design_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("design not found"))?;
    let revisions: Vec<DesignRevisionView> = sqlx::query_as(&format!(
        "SELECT {REVISION_VIEW_COLS} {REVISION_VIEW_FROM} \
         WHERE rv.tenant_id = $1 AND rv.design_id = $2 ORDER BY rv.version DESC"
    ))
    .bind(tenant_id)
    .bind(design_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let approvals: Vec<ApprovalView> = sqlx::query_as(
        "SELECT id, kind, design_id, adr_id, baseline_id, target_id, target_hash, status, \
                requested_by, decided_by, decision_reason, decided_at, created_at \
         FROM design.design_approval \
         WHERE tenant_id = $1 AND design_id = $2 ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .bind(design_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    Ok(DesignDetail {
        design,
        revisions,
        approvals,
    })
}

pub async fn create_design(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    req: CreateDesignRequest,
    actor: &AuditActor,
) -> Result<DesignDetail, PmWriteError> {
    let title = req.title.trim().to_string();
    if title.is_empty() {
        return Err(PmWriteError::Validation("title is required"));
    }
    let markdown = req
        .markdown
        .clone()
        .unwrap_or_else(|| format!("# {title}\n"));
    let draft = DraftDesignRevisionRequest {
        markdown,
        summary: req.summary.clone(),
        technical_analysis: req.technical_analysis.clone(),
        implementation_scope: req.implementation_scope.clone(),
        acceptance_criteria: req.acceptance_criteria.clone(),
    };
    let content = build_revision_content(&draft);
    let content_hash = content_hash_of(&content);

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let source = if principal_is_human(&mut tx, actor.principal_id).await? {
        "human"
    } else {
        "ai"
    };
    let code = match req.code.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(c) => c.to_string(),
        None => next_code(&mut tx, tenant_id, project_id, "design.design", "TD").await?,
    };
    let design_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.design (id, tenant_id, project_id, code, title, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(design_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(&code)
    .bind(&title)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            PmWriteError::Conflict("design code already exists in this project")
        } else {
            PmWriteError::Db(e)
        }
    })?;
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.design_revision \
         (id, tenant_id, project_id, design_id, version, content, state, content_hash, \
          source, created_by) \
         VALUES ($1, $2, $3, $4, 1, $5, 'draft', $6, $7, $8)",
    )
    .bind(revision_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(design_id)
    .bind(&content)
    .bind(&content_hash)
    .bind(source)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "design.created",
        "design",
        Some(design_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    design_detail(pool, tenant_id, design_id).await
}

/// 新修订草稿（版本 = max+1；基线指针 = 创建时的当前批准版本，§4.1）。
pub async fn create_design_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    req: DraftDesignRevisionRequest,
    actor: &AuditActor,
) -> Result<DesignRevisionView, PmWriteError> {
    if req.markdown.trim().is_empty() {
        return Err(PmWriteError::Validation("markdown is required"));
    }
    let content = build_revision_content(&req);
    let content_hash = content_hash_of(&content);

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, lifecycle, current, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    if lifecycle != "active" {
        return Err(PmWriteError::Conflict("design is not active"));
    }
    let source = if principal_is_human(&mut tx, actor.principal_id).await? {
        "human"
    } else {
        "ai"
    };
    let version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM design.design_revision \
         WHERE design_id = $1 AND tenant_id = $2",
    )
    .bind(design_id)
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.design_revision \
         (id, tenant_id, project_id, design_id, version, content, state, content_hash, \
          source, base_current_revision_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, 'draft', $7, $8, $9, $10)",
    )
    .bind(revision_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(design_id)
    .bind(version)
    .bind(&content)
    .bind(&content_hash)
    .bind(source)
    .bind(current)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "design_revision.created",
        "design_revision",
        Some(revision_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_revision_view(pool, tenant_id, design_id, revision_id).await
}

/// 改稿（仅 draft；正文由库级触发器双重保障，§4.2）。
pub async fn update_design_revision_draft(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
    req: DraftDesignRevisionRequest,
    actor: &AuditActor,
) -> Result<DesignRevisionView, PmWriteError> {
    if req.markdown.trim().is_empty() {
        return Err(PmWriteError::Validation("markdown is required"));
    }
    let content = build_revision_content(&req);
    let content_hash = content_hash_of(&content);

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    let (version, state, _, _) =
        lock_design_revision(&mut tx, tenant_id, design_id, revision_id).await?;
    let from = parse_state(&state)?;
    if !from.content_editable() {
        return Err(PmWriteError::Conflict("only draft revisions can be edited"));
    }
    sqlx::query(
        "UPDATE design.design_revision SET content = $1, content_hash = $2 \
         WHERE id = $3 AND design_id = $4 AND tenant_id = $5 AND state = 'draft'",
    )
    .bind(&content)
    .bind(&content_hash)
    .bind(revision_id)
    .bind(design_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "design_revision.updated",
        "design_revision",
        Some(revision_id),
        json!({ "version": version, "content_hash": content_hash }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_revision_view(pool, tenant_id, design_id, revision_id).await
}

/// 提交评审：draft → pending_confirmation（正文自此冻结；AI/人类皆可提交）。
pub async fn submit_design_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
    actor: &AuditActor,
) -> Result<DesignRevisionView, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    let (version, state, content_hash, content) =
        lock_design_revision(&mut tx, tenant_id, design_id, revision_id).await?;
    let from = parse_state(&state)?;
    transition_or_conflict(
        |f, t| f.can_transition_to(t),
        from,
        DesignRevisionState::PendingConfirmation,
        "illegal design revision transition",
    )?;
    let markdown = content
        .get("markdown")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if markdown.is_empty() {
        return Err(PmWriteError::Validation(
            "markdown is required before submit",
        ));
    }
    sqlx::query(
        "UPDATE design.design_revision \
         SET state = 'pending_confirmation', submitted_at = now() \
         WHERE id = $1 AND design_id = $2 AND tenant_id = $3 AND state = 'draft'",
    )
    .bind(revision_id)
    .bind(design_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "design_revision.submitted",
        "design_revision",
        Some(revision_id),
        json!({ "version": version, "content_hash": content_hash }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_revision_view(pool, tenant_id, design_id, revision_id).await
}

// ─────────────────────────────────────────── Design ↔ Requirement（Q1 多对多）

pub async fn list_design_requirements(
    pool: &PgPool,
    tenant_id: Uuid,
    _design_id: Uuid,
    revision_id: Uuid,
) -> Result<Vec<DesignRequirementLink>, PmWriteError> {
    sqlx::query_as(
        "SELECT dr.design_revision_id, dr.requirement_id, dr.requirement_revision_id, \
                r.code AS requirement_code, rv.version AS requirement_version, \
                rv.state AS requirement_revision_state, \
                (r.current_revision_id = dr.requirement_revision_id) AS is_current, \
                dr.created_at \
         FROM design.design_requirement dr \
         LEFT JOIN delivery.requirement r ON r.id = dr.requirement_id \
         LEFT JOIN delivery.requirement_revision rv ON rv.id = dr.requirement_revision_id \
         WHERE dr.tenant_id = $1 AND dr.design_revision_id = $2 \
         ORDER BY dr.created_at",
    )
    .bind(tenant_id)
    .bind(revision_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 绑定需求修订（§2.2：必须绑定具体且已批准的需求修订；仅 draft 可改关联）。
pub async fn link_design_requirement(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
    req: LinkDesignRequirementRequest,
    actor: &AuditActor,
) -> Result<DesignRequirementLink, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    let (_, state, _, _) = lock_design_revision(&mut tx, tenant_id, design_id, revision_id).await?;
    if !parse_state(&state)?.content_editable() {
        return Err(PmWriteError::Conflict(
            "requirement links are only editable on draft revisions",
        ));
    }
    // 需求必须属于本项目 + 本租户（跨项目越权拦截）。
    let (req_current,): (Option<Uuid>,) = sqlx::query_as(
        "SELECT current_revision_id FROM delivery.requirement \
         WHERE id = $1 AND tenant_id = $2 AND project_id = $3",
    )
    .bind(req.requirement_id)
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("requirement not found"))?;
    let revision_id_target =
        req.requirement_revision_id
            .unwrap_or(req_current.ok_or(PmWriteError::Validation(
                "requirement has no current revision to link",
            ))?);
    let (rev_state,): (String,) = sqlx::query_as(
        "SELECT state FROM delivery.requirement_revision \
         WHERE id = $1 AND requirement_id = $2 AND tenant_id = $3",
    )
    .bind(revision_id_target)
    .bind(req.requirement_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::Validation("requirement revision not found"))?;
    if rev_state != "approved" {
        // §2.2：设计只引用已经正式确认的需求修订。
        return Err(PmWriteError::Validation(
            "only approved requirement revisions can be linked",
        ));
    }
    sqlx::query(
        "INSERT INTO design.design_requirement \
         (design_revision_id, design_id, requirement_id, requirement_revision_id, \
          tenant_id, project_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (design_revision_id, requirement_id) DO NOTHING",
    )
    .bind(revision_id)
    .bind(design_id)
    .bind(req.requirement_id)
    .bind(revision_id_target)
    .bind(tenant_id)
    .bind(project_id)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "design_requirement.linked",
        "design_revision",
        Some(revision_id),
        json!({
            "requirement_id": req.requirement_id,
            "requirement_revision_id": revision_id_target,
        }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    let links = list_design_requirements(pool, tenant_id, design_id, revision_id).await?;
    links
        .into_iter()
        .find(|l| l.requirement_id == req.requirement_id)
        .ok_or(PmWriteError::Db(sqlx::Error::RowNotFound))
}

/// 解绑（仅 draft；关联变化全部留审计，§3.4 关键约束三）。
pub async fn unlink_design_requirement(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
    requirement_id: Uuid,
    actor: &AuditActor,
) -> Result<(), PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    let (_, state, _, _) = lock_design_revision(&mut tx, tenant_id, design_id, revision_id).await?;
    if !parse_state(&state)?.content_editable() {
        return Err(PmWriteError::Conflict(
            "requirement links are only editable on draft revisions",
        ));
    }
    let result = sqlx::query(
        "DELETE FROM design.design_requirement \
         WHERE tenant_id = $1 AND design_revision_id = $2 AND requirement_id = $3",
    )
    .bind(tenant_id)
    .bind(revision_id)
    .bind(requirement_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if result.rows_affected() == 0 {
        return Err(PmWriteError::NotFound("requirement link not found"));
    }
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "design_requirement.unlinked",
        "design_revision",
        Some(revision_id),
        json!({ "requirement_id": requirement_id }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    Ok(())
}
