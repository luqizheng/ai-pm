//! 专表审批（design.design_approval）：请求 / 裁决 / 收件箱与聚合读。

use super::*;

/// 请求批准设计修订（AI/人类皆可发起；每目标至多一条 pending）。
pub async fn request_design_revision_approval(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
    actor: &AuditActor,
) -> Result<ApprovalView, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    let (_, state, content_hash, _) =
        lock_design_revision(&mut tx, tenant_id, design_id, revision_id).await?;
    if state != DesignRevisionState::PendingConfirmation.as_str() {
        return Err(PmWriteError::Conflict(
            "only pending_confirmation revisions can be requested for approval",
        ));
    }
    let approval_id = insert_design_approval(
        &mut tx,
        tenant_id,
        project_id,
        "design_revision",
        "design_id",
        design_id,
        revision_id,
        &content_hash,
        actor.principal_id,
        "an approval is already pending for this revision",
    )
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "design.approval.requested",
        "design_approval",
        Some(approval_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_approval_view(pool, tenant_id, approval_id).await
}

/// 请求接受 ADR（AI/人类皆可发起）。
pub async fn request_adr_approval(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    actor: &AuditActor,
) -> Result<ApprovalView, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, status, current_revision_id, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    if status != AdrStatus::UnderReview.as_str() {
        return Err(PmWriteError::Conflict(
            "only under_review adrs can be requested for acceptance",
        ));
    }
    let content_hash: String = sqlx::query_scalar(
        "SELECT content_hash FROM design.adr_revision \
         WHERE id = $1 AND tenant_id = $2 AND adr_id = $3",
    )
    .bind(current_revision_id)
    .bind(tenant_id)
    .bind(adr_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::Conflict(
        "adr has no submitted content revision",
    ))?;
    let approval_id = insert_design_approval(
        &mut tx,
        tenant_id,
        project_id,
        "adr_acceptance",
        "adr_id",
        adr_id,
        adr_id,
        &content_hash,
        actor.principal_id,
        "an approval is already pending for this adr",
    )
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "adr.approval.requested",
        "design_approval",
        Some(approval_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_approval_view(pool, tenant_id, approval_id).await
}

/// 项目级审批收件箱（S03-05 评审与批准；S03 工作空间「待办审批」）。
///
/// 覆盖三种审批 kind（design_revision / adr_acceptance / baseline_freeze），
/// 并带出目标的展示标识（设计 code / ADR code / 基线 code）——审批行本身
/// 只存 target_id，UI 需要一个列表能直接读的标题。
pub async fn list_approvals(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<ApprovalInboxRow>, PmWriteError> {
    sqlx::query_as(
        "SELECT a.id, a.kind, a.design_id, a.adr_id, a.baseline_id, a.target_id, \
                a.target_hash, a.status, a.requested_by, a.decided_by, \
                a.decision_reason, a.decided_at, a.created_at, \
                COALESCE(d.code, ad.code, b.code) AS target_code, \
                COALESCE(d.title, ad.title, b.code) AS target_title, \
                rb.display_name AS requested_by_name, db.display_name AS decided_by_name \
         FROM design.design_approval a \
         LEFT JOIN design.design d ON d.id = a.design_id \
         LEFT JOIN design.adr ad ON ad.id = a.adr_id \
         LEFT JOIN design.implementation_baseline b ON b.id = a.baseline_id \
         LEFT JOIN core.principal rb ON rb.id = a.requested_by \
         LEFT JOIN core.principal db ON db.id = a.decided_by \
         WHERE a.tenant_id = $1 AND a.project_id = $2 \
         ORDER BY (a.status = 'pending') DESC, a.created_at DESC",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// S03 工作空间聚合（S03 四页共用的项目级上下文 + 计数）。
///
/// 一次读出：设计/ADR/基线计数 + 待办审批数 + 待处理评审问题数 + 当前
/// 正式版本与草稿版本清单——S03 四页的页头与空态判据都从这里取，避免
/// 前端为每个计数各打一次请求。
pub async fn design_workspace(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<DesignWorkspace, PmWriteError> {
    let designs = list_designs(pool, tenant_id, project_id).await?;
    let adrs = list_adrs(pool, tenant_id, project_id).await?;
    let baselines = list_baselines(pool, tenant_id, project_id).await?;
    let approvals = list_approvals(pool, tenant_id, project_id).await?;

    let open_findings: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM design.review_finding \
         WHERE tenant_id = $1 AND project_id = $2 AND status = 'open'",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;

    // 需求覆盖：设计 ↔ 需求绑定按「当前正式版本」计数（被新版本取代的绑定
    // 不计入覆盖，避免把过期依据当成已覆盖）。
    let (linked_requirements, stale_requirements): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE r.current_revision_id = dr.requirement_revision_id), \
                count(*) FILTER (WHERE r.current_revision_id IS DISTINCT FROM dr.requirement_revision_id) \
         FROM design.design_requirement dr \
         LEFT JOIN delivery.requirement r ON r.id = dr.requirement_id \
         WHERE dr.tenant_id = $1 AND dr.project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let mut design_states: BTreeMap<String, i64> = BTreeMap::new();
    for design in &designs {
        let state = design.current_state.as_deref().unwrap_or("none");
        *design_states.entry(state.to_string()).or_insert(0) += 1;
    }
    let mut adr_states: BTreeMap<String, i64> = BTreeMap::new();
    for adr in &adrs {
        *adr_states.entry(adr.status.clone()).or_insert(0) += 1;
    }

    Ok(DesignWorkspace {
        design_count: designs.len() as i64,
        adr_count: adrs.len() as i64,
        baseline_count: baselines.len() as i64,
        pending_approval_count: approvals.iter().filter(|a| a.status == "pending").count() as i64,
        open_finding_count: open_findings,
        linked_requirement_count: linked_requirements,
        stale_requirement_count: stale_requirements,
        design_states,
        adr_states,
    })
}

/// 基线 → 任务交接事实（S03-06 交接 S04）。
///
/// 冻结基线时钉定的任务引用（`delivery.task.implementation_baseline_id`），
/// 供「交接 S04」区展示：哪些任务以本基线为实施依据。
pub async fn baseline_tasks(
    pool: &PgPool,
    tenant_id: Uuid,
    baseline_id: Uuid,
) -> Result<Vec<BaselineTaskRow>, PmWriteError> {
    sqlx::query_as(
        "SELECT t.id, t.title, t.status, t.priority, t.created_at \
         FROM delivery.task t \
         WHERE t.tenant_id = $1 AND t.implementation_baseline_id = $2 \
         ORDER BY t.created_at",
    )
    .bind(tenant_id)
    .bind(baseline_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// S03-01/02 分析页读模型：每个设计的**需求覆盖**与**影响关系**。
///
/// - 需求覆盖：设计 ↔ 需求绑定按修订锚定（§3.4 第一条）；`is_current=false`
///   表示需求已被新版本取代而设计仍引用旧版——这正是 §6.2「不允许的行为」
///   之一，也是影响分析页要标红的那一类。
/// - 影响关系：本设计被哪些 ADR 引用（`design.adr_design_revision`），以及
///   本设计涉及的需求（供「改一处影响哪些需求」的反向视图）。
pub async fn design_analysis(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<DesignAnalysisRow>, PmWriteError> {
    let designs = list_designs(pool, tenant_id, project_id).await?;

    let requirement_links: Vec<AnalysisRequirementLink> = sqlx::query_as(
        "SELECT dr.design_id, dr.design_revision_id, dr.requirement_id, \
                dr.requirement_revision_id, r.code AS requirement_code, \
                r.title AS requirement_title, rv.version AS requirement_version, \
                rv.state AS requirement_state, \
                (r.current_revision_id = dr.requirement_revision_id) AS is_current \
         FROM design.design_requirement dr \
         LEFT JOIN delivery.requirement r ON r.id = dr.requirement_id \
         LEFT JOIN delivery.requirement_revision rv ON rv.id = dr.requirement_revision_id \
         WHERE dr.tenant_id = $1 AND dr.project_id = $2 \
         ORDER BY r.code",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let adr_links: Vec<AnalysisAdrLink> = sqlx::query_as(
        "SELECT l.design_id, l.adr_revision_id, a.id AS adr_id, a.code AS adr_code, \
                a.title AS adr_title, a.status AS adr_status, rv.version AS adr_version \
         FROM design.adr_design_revision l \
         LEFT JOIN design.adr a ON a.id = l.adr_id \
         LEFT JOIN design.adr_revision rv ON rv.id = l.adr_revision_id \
         WHERE l.tenant_id = $1 AND l.project_id = $2 \
         ORDER BY a.code",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let finding_rows: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT design_id, count(*) FROM design.review_finding \
         WHERE tenant_id = $1 AND project_id = $2 AND status = 'open' \
         GROUP BY design_id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let mut rows = Vec::with_capacity(designs.len());
    for design in designs {
        let requirements: Vec<AnalysisRequirementLink> = requirement_links
            .iter()
            .filter(|link| link.design_id == design.id)
            .cloned()
            .collect();
        let adrs: Vec<AnalysisAdrLink> = adr_links
            .iter()
            .filter(|link| link.design_id == design.id)
            .cloned()
            .collect();
        let open_findings = finding_rows
            .iter()
            .find(|(design_id, _)| *design_id == design.id)
            .map(|(_, count)| *count)
            .unwrap_or(0);
        let stale_requirement_count = requirements
            .iter()
            .filter(|l| l.is_current != Some(true))
            .count() as i64;
        rows.push(DesignAnalysisRow {
            design_id: design.id,
            code: design.code,
            title: design.title,
            lifecycle_status: design.lifecycle_status,
            current_revision_id: design.current_revision_id,
            current_version: design.current_version,
            current_state: design.current_state,
            requirement_count: requirements.len() as i64,
            stale_requirement_count,
            open_finding_count: open_findings,
            requirements,
            adrs,
        });
    }
    Ok(rows)
}

/// 专表审批插入 + pending 唯一冲突映射（批 A4 单源：design_revision /
/// adr_acceptance / task_plan 三域同构 INSERT 收敛于此）。
/// `entity_col` ∈ {design_id, adr_id, plan_id}（与 kind 一一对应）。
#[allow(clippy::too_many_arguments)] // 审批插入行，列清单与 design_approval 对应
pub(crate) async fn insert_design_approval(
    tx: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    project_id: Uuid,
    kind: &str,
    entity_col: &str,
    entity_id: Uuid,
    target_id: Uuid,
    target_hash: &str,
    requested_by: Uuid,
    pending_conflict: &'static str,
) -> Result<Uuid, PmWriteError> {
    let approval_id = Uuid::new_v4();
    sqlx::query(&format!(
        "INSERT INTO design.design_approval \
         (id, tenant_id, project_id, kind, {entity_col}, target_id, target_hash, requested_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
    ))
    .bind(approval_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(kind)
    .bind(entity_id)
    .bind(target_id)
    .bind(target_hash)
    .bind(requested_by)
    .execute(tx)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            PmWriteError::Conflict(pending_conflict)
        } else {
            PmWriteError::Db(e)
        }
    })?;
    Ok(approval_id)
}

pub async fn fetch_approval_view(
    db: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
    approval_id: Uuid,
) -> Result<ApprovalView, PmWriteError> {
    sqlx::query_as(
        "SELECT id, kind, design_id, adr_id, baseline_id, target_id, target_hash, status, \
                requested_by, decided_by, decision_reason, decided_at, created_at \
         FROM design.design_approval WHERE id = $1 AND tenant_id = $2",
    )
    .bind(approval_id)
    .bind(tenant_id)
    .fetch_optional(db)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("approval not found"))
}

/// 审批裁决（**唯一批准入口**；真人主体 + REST 真人门双闸）。
///
/// - `design_revision` approve：修订 approved + 旧批准版本 superseded +
///   design.current 指针切换（SC-01/SC-02/SC-06）；
/// - `adr_acceptance` approve：ADR under_review → accepted（Q3）；
/// - AI 主体调用即 `Forbidden`（SC-05；命令通道红线之外的服务面闸）。
pub async fn decide_approval(
    pool: &PgPool,
    tenant_id: Uuid,
    approval_id: Uuid,
    req: DecideApprovalRequest,
    actor: &AuditActor,
) -> Result<ApprovalView, PmWriteError> {
    if !["approve", "reject"].contains(&req.decision.as_str()) {
        return Err(PmWriteError::Validation(
            "decision must be approve or reject",
        ));
    }
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    type ApprovalDecisionRow = (
        Uuid,
        String,
        Option<Uuid>,
        Option<Uuid>,
        Option<Uuid>,
        Option<Uuid>,
        Uuid,
        String,
    );
    let (project_id, kind, design_id, adr_id, _baseline_id, plan_id, target_id, target_hash): ApprovalDecisionRow =
        sqlx::query_as(
        "SELECT project_id, kind, design_id, adr_id, baseline_id, plan_id, target_id, target_hash \
         FROM design.design_approval WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(approval_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("approval not found"))?;
    let status: String = sqlx::query_scalar(
        "SELECT status FROM design.design_approval WHERE id = $1 AND tenant_id = $2",
    )
    .bind(approval_id)
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if status != "pending" {
        return Err(PmWriteError::Conflict("approval is already decided"));
    }

    match kind.as_str() {
        "design_revision" => {
            let design_id = design_id.ok_or(PmWriteError::Validation("approval missing design"))?;
            let (project_id, lifecycle, current, row_revision) =
                lock_design(&mut tx, tenant_id, design_id).await?;
            if lifecycle != "active" {
                return Err(PmWriteError::Conflict("design is not active"));
            }
            // SC-06：审批人核准时的当前版本已漂移 → 显式 409。
            if let Some(expected) = req.expected_current_revision_id {
                if Some(expected) != current {
                    return Err(PmWriteError::Conflict(
                        "design current revision changed since the approval was reviewed",
                    ));
                }
            }
            if let Some(expected) = req.expected_row_revision {
                if expected != row_revision {
                    return Err(PmWriteError::Conflict(
                        "design row_revision changed since the approval was reviewed",
                    ));
                }
            }
            let (version, state, content_hash, _) =
                lock_design_revision(&mut tx, tenant_id, design_id, target_id).await?;
            if content_hash != target_hash {
                return Err(PmWriteError::Conflict(
                    "content hash mismatch: the reviewed content is not the current content",
                ));
            }
            let from = parse_state(&state)?;
            let to = if req.decision == "approve" {
                DesignRevisionState::Approved
            } else {
                DesignRevisionState::Rejected
            };
            transition_or_conflict(|f, t| f.can_transition_to(t), from, to, "illegal design revision transition")?;
            if req.decision == "approve" {
                // SC-05 前置：存在未关闭的 blocker 评审问题 → 拒绝批准。
                let open_blockers: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM design.review_finding \
                     WHERE tenant_id = $1 AND design_revision_id = $2 \
                       AND severity = 'blocker' AND status <> 'resolved'",
                )
                .bind(tenant_id)
                .bind(target_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(PmWriteError::Db)?;
                if open_blockers > 0 {
                    return Err(PmWriteError::Conflict(
                        "open blocker review findings must be resolved before approval",
                    ));
                }
                // SC-02：新版本批准，旧批准版本退位（历史保留、可查询）。
                sqlx::query(
                    "UPDATE design.design_revision SET state = 'superseded' \
                     WHERE design_id = $1 AND tenant_id = $2 AND state = 'approved' AND id <> $3",
                )
                .bind(design_id)
                .bind(tenant_id)
                .bind(target_id)
                .execute(&mut *tx)
                .await
                .map_err(PmWriteError::Db)?;
                sqlx::query(
                    "UPDATE design.design_revision \
                     SET state = 'approved', approved_by = $1, approved_at = now() \
                     WHERE id = $2 AND design_id = $3 AND tenant_id = $4 AND state = 'pending_confirmation'",
                )
                .bind(actor.principal_id)
                .bind(target_id)
                .bind(design_id)
                .bind(tenant_id)
                .execute(&mut *tx)
                .await
                .map_err(PmWriteError::Db)?;
                sqlx::query(
                    "UPDATE design.design \
                     SET current_revision_id = $1, row_revision = row_revision + 1, updated_at = now() \
                     WHERE id = $2 AND tenant_id = $3",
                )
                .bind(target_id)
                .bind(design_id)
                .bind(tenant_id)
                .execute(&mut *tx)
                .await
                .map_err(PmWriteError::Db)?;
            } else {
                sqlx::query(
                    "UPDATE design.design_revision \
                     SET state = 'rejected' \
                     WHERE id = $1 AND design_id = $2 AND tenant_id = $3 AND state = 'pending_confirmation'",
                )
                .bind(target_id)
                .bind(design_id)
                .bind(tenant_id)
                .execute(&mut *tx)
                .await
                .map_err(PmWriteError::Db)?;
            }
            finalize_approval(
                &mut tx,
                project_id,
                approval_id,
                &req,
                actor,
                json!({ "version": version }),
            )
            .await?;
        }
        "adr_acceptance" => {
            let adr_id = adr_id.ok_or(PmWriteError::Validation("approval missing adr"))?;
            let (project_id, status_now, current_revision_id, _) =
                lock_adr(&mut tx, tenant_id, adr_id).await?;
            let from = AdrStatus::parse(&status_now)
                .ok_or_else(|| PmWriteError::Validation("unknown adr status"))?;
            let to = if req.decision == "approve" {
                AdrStatus::Accepted
            } else {
                AdrStatus::Rejected
            };
            transition_or_conflict(|f, t| f.can_transition_to(t), from, to, "illegal adr transition")?;
            // 内容锚定：当前内容版本哈希必须与审批人核准的哈希一致。
            let content_hash: String = sqlx::query_scalar(
                "SELECT content_hash FROM design.adr_revision \
                 WHERE id = $1 AND tenant_id = $2 AND adr_id = $3",
            )
            .bind(current_revision_id)
            .bind(tenant_id)
            .bind(adr_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(PmWriteError::Db)?
            .ok_or(PmWriteError::Conflict("adr has no content revision"))?;
            if content_hash != target_hash {
                return Err(PmWriteError::Conflict(
                    "content hash mismatch: the reviewed content is not the current content",
                ));
            }
            let to_str = to.as_str();
            sqlx::query(
                "UPDATE design.adr \
                 SET status = $1, row_revision = row_revision + 1, updated_at = now() \
                 WHERE id = $2 AND tenant_id = $3",
            )
            .bind(to_str)
            .bind(adr_id)
            .bind(tenant_id)
            .execute(&mut *tx)
            .await
            .map_err(PmWriteError::Db)?;
            finalize_approval(
                &mut tx,
                project_id,
                approval_id,
                &req,
                actor,
                json!({ "status": to_str }),
            )
            .await?;
        }
        "task_plan" => {
            // 计划裁决走独立事务（apply_plan_decision 内含 require_human +
            // 状态机校验）；随后在本事务内终局化审批行。
            let target_plan_id =
                plan_id.ok_or(PmWriteError::Validation("approval missing plan"))?;
            crate::task_plan::apply_plan_decision(
                pool,
                tenant_id,
                target_plan_id,
                req.decision == "approve",
                actor,
            )
            .await?;
            finalize_approval(
                &mut tx,
                project_id,
                approval_id,
                &req,
                actor,
                json!({ "kind": "task_plan" }),
            )
            .await?;
        }
        _ => {
            return Err(PmWriteError::Validation("unknown approval kind"));
        }
    }
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_approval_view(pool, tenant_id, approval_id).await
}

async fn finalize_approval(
    tx: &mut sqlx::PgConnection,
    project_id: Uuid,
    approval_id: Uuid,
    req: &DecideApprovalRequest,
    actor: &AuditActor,
    after: Value,
) -> Result<(), PmWriteError> {
    let decision = if req.decision == "approve" {
        "approved"
    } else {
        "rejected"
    };
    sqlx::query(
        "UPDATE design.design_approval \
         SET status = $1, decided_by = $2, decision_reason = $3, decided_at = now() \
         WHERE id = $4 AND status = 'pending'",
    )
    .bind(decision)
    .bind(actor.principal_id)
    .bind(req.reason.clone())
    .bind(approval_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        tx,
        project_id,
        actor,
        "approval.decide",
        "design_approval",
        Some(approval_id),
        after,
    )
    .await?;
    Ok(())
}
