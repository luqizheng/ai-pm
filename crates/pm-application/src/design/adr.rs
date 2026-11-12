//! 架构决策记录（ADR，Q3）：起草 / 评审 / 关联 / 取代 / 废弃全生命周期。

use super::*;

pub async fn list_adrs(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<AdrEntry>, PmWriteError> {
    sqlx::query_as(
        "SELECT a.id, a.project_id, a.code, a.title, a.status, a.blockers_implementation, \
                a.current_revision_id, rv.version AS current_version, \
                a.superseded_by_adr_id, a.row_revision, a.created_at, a.updated_at \
         FROM design.adr a \
         LEFT JOIN design.adr_revision rv ON rv.id = a.current_revision_id \
         WHERE a.tenant_id = $1 AND a.project_id = $2 \
         ORDER BY a.created_at DESC",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

#[derive(Debug, Serialize)]
pub struct AdrDetail {
    pub adr: AdrEntry,
    pub revisions: Vec<AdrRevisionView>,
    pub design_links: Vec<Value>,
    pub requirement_links: Vec<Value>,
    pub approvals: Vec<ApprovalView>,
}

pub async fn adr_detail(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
) -> Result<AdrDetail, PmWriteError> {
    let adr: AdrEntry = sqlx::query_as(
        "SELECT a.id, a.project_id, a.code, a.title, a.status, a.blockers_implementation, \
                a.current_revision_id, rv.version AS current_version, \
                a.superseded_by_adr_id, a.row_revision, a.created_at, a.updated_at \
         FROM design.adr a \
         LEFT JOIN design.adr_revision rv ON rv.id = a.current_revision_id \
         WHERE a.tenant_id = $1 AND a.id = $2",
    )
    .bind(tenant_id)
    .bind(adr_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("adr not found"))?;
    let revisions: Vec<AdrRevisionView> = sqlx::query_as(
        "SELECT id, adr_id, version, content, state, content_hash, created_by, created_at \
         FROM design.adr_revision WHERE tenant_id = $1 AND adr_id = $2 ORDER BY version DESC",
    )
    .bind(tenant_id)
    .bind(adr_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let design_links: Vec<Value> = sqlx::query(
        "SELECT l.adr_revision_id, l.design_id, l.design_revision_id, d.code, rv.version \
         FROM design.adr_design_revision l \
         LEFT JOIN design.design d ON d.id = l.design_id \
         LEFT JOIN design.design_revision rv ON rv.id = l.design_revision_id \
         WHERE l.tenant_id = $1 AND l.adr_id = $2 ORDER BY l.created_at",
    )
    .bind(tenant_id)
    .bind(adr_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?
    .into_iter()
    .map(|row| {
        use sqlx::Row;
        json!({
            "adr_revision_id": row.get::<Uuid, _>("adr_revision_id"),
            "design_id": row.get::<Uuid, _>("design_id"),
            "design_revision_id": row.get::<Uuid, _>("design_revision_id"),
            "design_code": row.get::<Option<String>, _>("code"),
            "design_revision_version": row.get::<Option<i32>, _>("version"),
        })
    })
    .collect();
    let requirement_links: Vec<Value> = sqlx::query(
        "SELECT l.adr_revision_id, l.requirement_id, l.requirement_revision_id, \
                r.code, rv.version \
         FROM design.adr_requirement l \
         LEFT JOIN delivery.requirement r ON r.id = l.requirement_id \
         LEFT JOIN delivery.requirement_revision rv ON rv.id = l.requirement_revision_id \
         WHERE l.tenant_id = $1 AND l.adr_id = $2 ORDER BY l.created_at",
    )
    .bind(tenant_id)
    .bind(adr_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?
    .into_iter()
    .map(|row| {
        use sqlx::Row;
        json!({
            "adr_revision_id": row.get::<Uuid, _>("adr_revision_id"),
            "requirement_id": row.get::<Uuid, _>("requirement_id"),
            "requirement_revision_id": row.get::<Uuid, _>("requirement_revision_id"),
            "requirement_code": row.get::<Option<String>, _>("code"),
            "requirement_revision_version": row.get::<Option<i32>, _>("version"),
        })
    })
    .collect();
    let approvals: Vec<ApprovalView> = sqlx::query_as(
        "SELECT id, kind, design_id, adr_id, baseline_id, target_id, target_hash, status, \
                requested_by, decided_by, decision_reason, decided_at, created_at \
         FROM design.design_approval \
         WHERE tenant_id = $1 AND adr_id = $2 ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .bind(adr_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    Ok(AdrDetail {
        adr,
        revisions,
        design_links,
        requirement_links,
        approvals,
    })
}

fn build_adr_content(req: &DraftAdrRevisionRequest) -> Value {
    let mut content = json!({ "decision_question": req.decision_question });
    let obj = content.as_object_mut().expect("object");
    if let Some(s) = req.context.as_deref().filter(|s| !s.trim().is_empty()) {
        obj.insert("context".into(), json!(s));
    }
    if !req.options.is_empty() {
        obj.insert("options".into(), json!(req.options));
    }
    if let Some(s) = req
        .chosen_option
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        obj.insert("chosen_option".into(), json!(s));
    }
    if let Some(s) = req.rationale.as_deref().filter(|s| !s.trim().is_empty()) {
        obj.insert("rationale".into(), json!(s));
    }
    if let Some(s) = req.consequences.as_deref().filter(|s| !s.trim().is_empty()) {
        obj.insert("consequences".into(), json!(s));
    }
    content
}

pub async fn create_adr(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    req: CreateAdrRequest,
    actor: &AuditActor,
) -> Result<AdrDetail, PmWriteError> {
    let title = req.title.trim().to_string();
    if title.is_empty() {
        return Err(PmWriteError::Validation("title is required"));
    }
    if req.decision_question.trim().is_empty() {
        return Err(PmWriteError::Validation("decision_question is required"));
    }
    let content = build_adr_content(&DraftAdrRevisionRequest {
        decision_question: req.decision_question.clone(),
        context: req.context.clone(),
        options: req.options.clone(),
        chosen_option: req.chosen_option.clone(),
        rationale: req.rationale.clone(),
        consequences: req.consequences.clone(),
    });
    let content_hash = content_hash_of(&content);

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let code = match req.code.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(c) => c.to_string(),
        None => next_code(&mut tx, tenant_id, project_id, "design.adr", "ADR").await?,
    };
    let adr_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.adr \
         (id, tenant_id, project_id, code, title, blockers_implementation, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(adr_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(&code)
    .bind(&title)
    .bind(req.blockers_implementation)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            PmWriteError::Conflict("adr code already exists in this project")
        } else {
            PmWriteError::Db(e)
        }
    })?;
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.adr_revision \
         (id, tenant_id, project_id, adr_id, version, content, state, content_hash, created_by) \
         VALUES ($1, $2, $3, $4, 1, $5, 'draft', $6, $7)",
    )
    .bind(revision_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(adr_id)
    .bind(&content)
    .bind(&content_hash)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "adr.created",
        "adr",
        Some(adr_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    adr_detail(pool, tenant_id, adr_id).await
}

/// 追加 ADR 内容修订（仅 proposed/under_review；已接受的决策不可改，走新 ADR）。
pub async fn create_adr_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    req: DraftAdrRevisionRequest,
    actor: &AuditActor,
) -> Result<AdrRevisionView, PmWriteError> {
    if req.decision_question.trim().is_empty() {
        return Err(PmWriteError::Validation("decision_question is required"));
    }
    let content = build_adr_content(&req);
    let content_hash = content_hash_of(&content);

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, status, _, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    if !matches!(status.as_str(), "proposed" | "under_review") {
        return Err(PmWriteError::Conflict(
            "accepted/rejected/superseded/deprecated adrs are immutable; create a new adr",
        ));
    }
    let version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM design.adr_revision \
         WHERE adr_id = $1 AND tenant_id = $2",
    )
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.adr_revision \
         (id, tenant_id, project_id, adr_id, version, content, state, content_hash, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, 'draft', $7, $8)",
    )
    .bind(revision_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(adr_id)
    .bind(version)
    .bind(&content)
    .bind(&content_hash)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "adr_revision.created",
        "adr_revision",
        Some(revision_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    sqlx::query_as(
        "SELECT id, adr_id, version, content, state, content_hash, created_by, created_at \
         FROM design.adr_revision WHERE id = $1 AND tenant_id = $2",
    )
    .bind(revision_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 提交 ADR 内容（revision draft→submitted + adr proposed→under_review）。
pub async fn submit_adr_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    revision_id: Uuid,
    actor: &AuditActor,
) -> Result<AdrDetail, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, status, _, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    let (version, state): (i32, String) = sqlx::query_as(
        "SELECT version, state FROM design.adr_revision \
         WHERE id = $1 AND adr_id = $2 AND tenant_id = $3 FOR UPDATE",
    )
    .bind(revision_id)
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("adr revision not found"))?;
    let from = AdrRevisionState::parse(&state)
        .ok_or_else(|| PmWriteError::Validation("unknown adr revision state"))?;
    if !from.can_transition_to(AdrRevisionState::Submitted) {
        return Err(PmWriteError::Conflict("adr revision already submitted"));
    }
    sqlx::query(
        "UPDATE design.adr_revision SET state = 'submitted' \
         WHERE id = $1 AND adr_id = $2 AND tenant_id = $3 AND state = 'draft'",
    )
    .bind(revision_id)
    .bind(adr_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    let adr_from =
        AdrStatus::parse(&status).ok_or_else(|| PmWriteError::Validation("unknown adr status"))?;
    if adr_from == AdrStatus::Proposed {
        transition_or_conflict(
            |f, t| f.can_transition_to(t),
            adr_from,
            AdrStatus::UnderReview,
            "illegal adr transition",
        )?;
        sqlx::query(
            "UPDATE design.adr SET status = 'under_review', updated_at = now() \
             WHERE id = $1 AND tenant_id = $2",
        )
        .bind(adr_id)
        .bind(tenant_id)
        .execute(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
    }
    // 提交即成为当前内容版本（D-03：acceptance 锚定最新提交内容）。
    sqlx::query(
        "UPDATE design.adr SET current_revision_id = $1, updated_at = now() \
         WHERE id = $2 AND tenant_id = $3",
    )
    .bind(revision_id)
    .bind(adr_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "adr_revision.submitted",
        "adr_revision",
        Some(revision_id),
        json!({ "version": version }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    adr_detail(pool, tenant_id, adr_id).await
}

/// ADR 关联设计版本（§3.4 关键约束二：引用具体 Design Revision；仅 draft 修订可改）。
pub async fn link_adr_design_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    adr_revision_id: Uuid,
    req: LinkAdrDesignRequest,
    actor: &AuditActor,
) -> Result<(), PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, current, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    let (state,): (String,) = sqlx::query_as(
        "SELECT state FROM design.adr_revision \
         WHERE id = $1 AND adr_id = $2 AND tenant_id = $3",
    )
    .bind(adr_revision_id)
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("adr revision not found"))?;
    if !AdrRevisionState::parse(&state)
        .map(|s| s.content_editable())
        .unwrap_or(false)
    {
        return Err(PmWriteError::Conflict(
            "design links are only editable on draft adr revisions",
        ));
    }
    // 设计修订必须属于本项目（跨项目越权拦截）。
    let in_project: Option<Uuid> = sqlx::query_scalar(
        "SELECT rv.id FROM design.design_revision rv \
         WHERE rv.id = $1 AND rv.design_id = $2 AND rv.tenant_id = $3 AND rv.project_id = $4",
    )
    .bind(req.design_revision_id)
    .bind(req.design_id)
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if in_project.is_none() {
        return Err(PmWriteError::Validation(
            "design revision not found in this project",
        ));
    }
    let _ = current;
    sqlx::query(
        "INSERT INTO design.adr_design_revision \
         (adr_id, adr_revision_id, design_id, design_revision_id, tenant_id, project_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (adr_revision_id, design_revision_id) DO NOTHING",
    )
    .bind(adr_id)
    .bind(adr_revision_id)
    .bind(req.design_id)
    .bind(req.design_revision_id)
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
        "adr_design_revision.linked",
        "adr_revision",
        Some(adr_revision_id),
        json!({
            "design_id": req.design_id,
            "design_revision_id": req.design_revision_id,
        }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    Ok(())
}

/// ADR 关联需求修订（与设计关联同规则：必须 approved；仅 draft 修订可改）。
pub async fn link_adr_requirement(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    adr_revision_id: Uuid,
    req: LinkAdrRequirementRequest,
    actor: &AuditActor,
) -> Result<(), PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    let (state,): (String,) = sqlx::query_as(
        "SELECT state FROM design.adr_revision \
         WHERE id = $1 AND adr_id = $2 AND tenant_id = $3",
    )
    .bind(adr_revision_id)
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("adr revision not found"))?;
    if !AdrRevisionState::parse(&state)
        .map(|s| s.content_editable())
        .unwrap_or(false)
    {
        return Err(PmWriteError::Conflict(
            "requirement links are only editable on draft adr revisions",
        ));
    }
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
        return Err(PmWriteError::Validation(
            "only approved requirement revisions can be linked",
        ));
    }
    sqlx::query(
        "INSERT INTO design.adr_requirement \
         (adr_id, adr_revision_id, requirement_id, requirement_revision_id, \
          tenant_id, project_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (adr_revision_id, requirement_id) DO NOTHING",
    )
    .bind(adr_id)
    .bind(adr_revision_id)
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
        "adr_requirement.linked",
        "adr_revision",
        Some(adr_revision_id),
        json!({
            "requirement_id": req.requirement_id,
            "requirement_revision_id": revision_id_target,
        }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    Ok(())
}

/// 决策被新 ADR 取代（accepted → superseded；后继必须已接受）。
pub async fn supersede_adr(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    req: SupersedeAdrRequest,
    actor: &AuditActor,
) -> Result<AdrEntry, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    let (project_id, status, _, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    transition_or_conflict(
        |f, t| f.can_transition_to(t),
        AdrStatus::parse(&status).ok_or_else(|| PmWriteError::Validation("unknown adr status"))?,
        AdrStatus::Superseded,
        "illegal adr transition",
    )?;
    let (successor_status, successor_project): (String, Uuid) = sqlx::query_as(
        "SELECT status, project_id FROM design.adr \
         WHERE id = $1 AND tenant_id = $2",
    )
    .bind(req.superseded_by_adr_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::Validation("successor adr not found"))?;
    if successor_project != project_id {
        return Err(PmWriteError::Validation(
            "successor adr is in another project",
        ));
    }
    if successor_status != AdrStatus::Accepted.as_str() {
        return Err(PmWriteError::Validation(
            "successor adr must be accepted before it can supersede",
        ));
    }
    if req.superseded_by_adr_id == adr_id {
        return Err(PmWriteError::Validation("an adr cannot supersede itself"));
    }
    sqlx::query(
        "UPDATE design.adr \
         SET status = 'superseded', superseded_by_adr_id = $1, row_revision = row_revision + 1, updated_at = now() \
         WHERE id = $2 AND tenant_id = $3",
    )
    .bind(req.superseded_by_adr_id)
    .bind(adr_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "adr.superseded",
        "adr",
        Some(adr_id),
        json!({ "superseded_by_adr_id": req.superseded_by_adr_id, "reason": req.reason }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    sqlx::query_as(
        "SELECT a.id, a.project_id, a.code, a.title, a.status, a.blockers_implementation, \
                a.current_revision_id, rv.version AS current_version, \
                a.superseded_by_adr_id, a.row_revision, a.created_at, a.updated_at \
         FROM design.adr a \
         LEFT JOIN design.adr_revision rv ON rv.id = a.current_revision_id \
         WHERE a.id = $1 AND a.tenant_id = $2",
    )
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 废弃 ADR（proposed/under_review/accepted → deprecated；真人命令）。
pub async fn deprecate_adr(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    actor: &AuditActor,
) -> Result<AdrEntry, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    let (project_id, status, _, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    transition_or_conflict(
        |f, t| f.can_transition_to(t),
        AdrStatus::parse(&status).ok_or_else(|| PmWriteError::Validation("unknown adr status"))?,
        AdrStatus::Deprecated,
        "illegal adr transition",
    )?;
    sqlx::query(
        "UPDATE design.adr SET status = 'deprecated', row_revision = row_revision + 1, updated_at = now() \
         WHERE id = $1 AND tenant_id = $2",
    )
    .bind(adr_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "adr.deprecated",
        "adr",
        Some(adr_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    sqlx::query_as(
        "SELECT a.id, a.project_id, a.code, a.title, a.status, a.blockers_implementation, \
                a.current_revision_id, rv.version AS current_version, \
                a.superseded_by_adr_id, a.row_revision, a.created_at, a.updated_at \
         FROM design.adr a \
         LEFT JOIN design.adr_revision rv ON rv.id = a.current_revision_id \
         WHERE a.id = $1 AND a.tenant_id = $2",
    )
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 设置实施阻塞标记（§6.2；真人命令——直接决定 Readiness Gate 判定输入）。
pub async fn set_adr_implementation_blocking(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    req: SetBlockingRequest,
    actor: &AuditActor,
) -> Result<AdrEntry, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    let (project_id, _, _, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    sqlx::query(
        "UPDATE design.adr SET blockers_implementation = $1, updated_at = now() \
         WHERE id = $2 AND tenant_id = $3",
    )
    .bind(req.blockers_implementation)
    .bind(adr_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "adr.blocking_updated",
        "adr",
        Some(adr_id),
        json!({ "blockers_implementation": req.blockers_implementation }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    sqlx::query_as(
        "SELECT a.id, a.project_id, a.code, a.title, a.status, a.blockers_implementation, \
                a.current_revision_id, rv.version AS current_version, \
                a.superseded_by_adr_id, a.row_revision, a.created_at, a.updated_at \
         FROM design.adr a \
         LEFT JOIN design.adr_revision rv ON rv.id = a.current_revision_id \
         WHERE a.id = $1 AND a.tenant_id = $2",
    )
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}
