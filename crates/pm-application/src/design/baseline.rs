//! Implementation Baseline（Q5 §6.3）：冻结 / 取代 / 任务视图。

use super::*;

pub async fn list_baselines(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<BaselineEntry>, PmWriteError> {
    sqlx::query_as(
        "SELECT id, project_id, code, status, content, content_hash, readiness, \
                frozen_by, frozen_at, superseded_by_baseline_id, created_at \
         FROM design.implementation_baseline \
         WHERE tenant_id = $1 AND project_id = $2 ORDER BY frozen_at DESC",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

pub async fn baseline_detail(
    pool: &PgPool,
    tenant_id: Uuid,
    baseline_id: Uuid,
) -> Result<BaselineEntry, PmWriteError> {
    sqlx::query_as(
        "SELECT id, project_id, code, status, content, content_hash, readiness, \
                frozen_by, frozen_at, superseded_by_baseline_id, created_at \
         FROM design.implementation_baseline \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(baseline_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("baseline not found"))
}

/// 冻结实施基线（§6.3）：Readiness Gate 五条件全过 → 不可变快照 + 冻结审批行。
///
/// 冻结后设计/需求/ADR 的任何变化都不会改变本基线（SC-08）；基线变更 =
/// 重新冻结新基线 + 显式 supersede。
pub async fn freeze_baseline(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    req: FreezeBaselineRequest,
    actor: &AuditActor,
) -> Result<BaselineEntry, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    let gate = evaluate_readiness(pool, tenant_id, project_id, &req.design_revision_ids).await?;
    if !gate.ready {
        return Err(PmWriteError::Conflict(
            "readiness gate failed: resolve the failed conditions before freezing a baseline",
        ));
    }
    let revision_ids = req.design_revision_ids.clone();

    // 设计与修订快照。
    let design_rows: Vec<(Uuid, String, String, Uuid, i32, String)> = sqlx::query_as(
        "SELECT d.id, d.code, d.title, rv.id, rv.version, rv.content_hash \
         FROM design.design_revision rv \
         JOIN design.design d ON d.id = rv.design_id \
         WHERE rv.tenant_id = $1 AND rv.project_id = $2 AND rv.id = ANY($3) \
         ORDER BY d.code, rv.version",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&revision_ids)
    .fetch_all(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;

    // 需求依据快照（去重）。
    let requirement_rows: Vec<(Uuid, Uuid, i32)> = sqlx::query_as(
        "SELECT DISTINCT dr.requirement_id, dr.requirement_revision_id, rv.version \
         FROM design.design_requirement dr \
         JOIN delivery.requirement_revision rv ON rv.id = dr.requirement_revision_id \
         WHERE dr.tenant_id = $1 AND dr.project_id = $2 \
           AND dr.design_revision_id = ANY($3) \
         ORDER BY dr.requirement_id, dr.requirement_revision_id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&revision_ids)
    .fetch_all(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;

    // ADR 快照（经设计关联收集；去重）。
    let adr_rows: Vec<(Uuid, String, Uuid, String)> = sqlx::query_as(
        "SELECT DISTINCT l.adr_id, a.code, l.adr_revision_id, a.status \
         FROM design.adr_design_revision l \
         JOIN design.adr a ON a.id = l.adr_id \
         WHERE l.tenant_id = $1 AND l.project_id = $2 \
           AND l.design_revision_id = ANY($3) \
         ORDER BY l.adr_id, l.adr_revision_id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&revision_ids)
    .fetch_all(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;

    // 范围/验收合并（逐修订 content 结构化字段并集）。
    let mut included: Vec<Value> = Vec::new();
    let mut excluded: Vec<Value> = Vec::new();
    let mut modules: Vec<Value> = Vec::new();
    let mut acceptance: Vec<Value> = Vec::new();
    for rid in &revision_ids {
        let content: Option<Value> = sqlx::query_scalar(
            "SELECT content FROM design.design_revision \
             WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(rid)
        .fetch_optional(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
        let Some(content) = content else { continue };
        if let Some(scope) = content.get("implementation_scope") {
            for key in ["included", "excluded", "modules"] {
                if let Some(arr) = scope.get(key).and_then(Value::as_array) {
                    for item in arr {
                        let target = match key {
                            "included" => &mut included,
                            "excluded" => &mut excluded,
                            _ => &mut modules,
                        };
                        if !target.contains(item) {
                            target.push(item.clone());
                        }
                    }
                }
            }
        }
        if let Some(arr) = content.get("acceptance_criteria").and_then(Value::as_array) {
            for item in arr {
                if !acceptance.contains(item) {
                    acceptance.push(item.clone());
                }
            }
        }
    }

    // 仓库引用校验（§6.3 Repository Context 固定依据：必须属于本项目）。
    let unique_repo_ids: Vec<Uuid> = {
        let mut seen = std::collections::HashSet::new();
        req.repository_refs
            .iter()
            .filter(|r| seen.insert(r.repository_id))
            .map(|r| r.repository_id)
            .collect()
    };
    if !unique_repo_ids.is_empty() {
        let known: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM integration.project_repository \
             WHERE id = ANY($1) AND project_id = $2",
        )
        .bind(&unique_repo_ids)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
        if known != unique_repo_ids.len() as i64 {
            return Err(PmWriteError::Validation(
                "repository_refs contains repositories outside this project",
            ));
        }
    }

    let content = json!({
        "design_revision_ids": revision_ids,
        "designs": design_rows.iter().map(|(did, code, title, rid, ver, hash)| json!({
            "design_id": did, "code": code, "title": title,
            "design_revision_id": rid, "version": ver, "content_hash": hash,
        })).collect::<Vec<_>>(),
        "requirements": requirement_rows.iter().map(|(rid, rvid, ver)| json!({
            "requirement_id": rid, "requirement_revision_id": rvid, "version": ver,
        })).collect::<Vec<_>>(),
        "adrs": adr_rows.iter().map(|(aid, code, arid, status)| json!({
            "adr_id": aid, "code": code, "adr_revision_id": arid, "status": status,
        })).collect::<Vec<_>>(),
        "scope": { "included": included, "excluded": excluded, "modules": modules },
        "acceptance_criteria": acceptance,
        "repository_refs": req.repository_refs.iter().map(|r| json!({
            "repository_id": r.repository_id,
            "base_commit": r.base_commit,
        })).collect::<Vec<_>>(),
    });
    let content_hash = content_hash_of(&content);
    let gate_json = serde_json::to_value(&gate)
        .map_err(|e| PmWriteError::Db(sqlx::Error::Protocol(format!("gate serialization: {e}"))))?;

    let code = match req.code.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(c) => c.to_string(),
        None => {
            next_code(
                &mut tx,
                tenant_id,
                project_id,
                "design.implementation_baseline",
                "IB",
            )
            .await?
        }
    };
    let baseline_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.implementation_baseline \
         (id, tenant_id, project_id, code, content, content_hash, readiness, frozen_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(baseline_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(&code)
    .bind(&content)
    .bind(&content_hash)
    .bind(&gate_json)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            PmWriteError::Conflict("baseline code already exists in this project")
        } else {
            PmWriteError::Db(e)
        }
    })?;
    // R6：基线冻结的独立审批语义——专表审批行（created+approved 同事务，冻结人即批准人）。
    sqlx::query(
        "INSERT INTO design.design_approval \
         (id, tenant_id, project_id, kind, baseline_id, target_id, target_hash, \
          status, requested_by, decided_by, decision_reason, decided_at) \
         VALUES ($1, $2, $3, 'baseline_freeze', $4, $4, $5, 'approved', $6, $6, $7, now())",
    )
    .bind(Uuid::new_v4())
    .bind(tenant_id)
    .bind(project_id)
    .bind(baseline_id)
    .bind(&content_hash)
    .bind(actor.principal_id)
    .bind(req_reason_of_freeze(&gate))
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "baseline.frozen",
        "implementation_baseline",
        Some(baseline_id),
        json!({ "code": code, "content_hash": content_hash }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    baseline_detail(pool, tenant_id, baseline_id).await
}

fn req_reason_of_freeze(gate: &GateReport) -> Option<String> {
    let passed = gate
        .conditions
        .iter()
        .filter(|c| c.passed)
        .map(|c| c.condition)
        .collect::<Vec<_>>()
        .join(",");
    Some(format!("readiness gate passed: {passed}"))
}

/// 显式替代基线（SC-08 的另一半：新基线冻结后，旧基线由人显式退位）。
pub async fn supersede_baseline(
    pool: &PgPool,
    tenant_id: Uuid,
    baseline_id: Uuid,
    req: SupersedeBaselineRequest,
    actor: &AuditActor,
) -> Result<BaselineEntry, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    let (project_id, status): (Uuid, String) = sqlx::query_as(
        "SELECT project_id, status FROM design.implementation_baseline \
         WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(baseline_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("baseline not found"))?;
    if status != "frozen" {
        return Err(PmWriteError::Conflict("baseline is already superseded"));
    }
    if let Some(successor) = req.successor_baseline_id {
        let (s_project, s_status): (Uuid, String) = sqlx::query_as(
            "SELECT project_id, status FROM design.implementation_baseline \
             WHERE id = $1 AND tenant_id = $2",
        )
        .bind(successor)
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?
        .ok_or(PmWriteError::Validation("successor baseline not found"))?;
        if s_project != project_id {
            return Err(PmWriteError::Validation(
                "successor baseline is in another project",
            ));
        }
        if s_status != "frozen" {
            return Err(PmWriteError::Validation(
                "successor baseline must be frozen",
            ));
        }
        sqlx::query(
            "UPDATE design.implementation_baseline \
             SET status = 'superseded', superseded_by_baseline_id = $1 \
             WHERE id = $2 AND tenant_id = $3",
        )
        .bind(successor)
        .bind(baseline_id)
        .bind(tenant_id)
        .execute(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
    } else {
        sqlx::query(
            "UPDATE design.implementation_baseline SET status = 'superseded' \
             WHERE id = $1 AND tenant_id = $2",
        )
        .bind(baseline_id)
        .bind(tenant_id)
        .execute(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
    }
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "baseline.superseded",
        "implementation_baseline",
        Some(baseline_id),
        json!({ "superseded_by_baseline_id": req.successor_baseline_id, "reason": req.reason }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    baseline_detail(pool, tenant_id, baseline_id).await
}
