//! Readiness Gate（总体设计 §6.1 门禁五条件）——设计的就绪判定。

use super::*;

/// 门禁五条件之一（§6.1）。
#[derive(Debug, Serialize)]
pub struct GateCondition {
    pub condition: &'static str,
    pub passed: bool,
    pub details: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GateReport {
    pub ready: bool,
    pub design_revision_ids: Vec<Uuid>,
    pub conditions: Vec<GateCondition>,
}

/// Readiness Gate 五条件判定（§6.1；freeze 前置 + dry-run 端点共用）。
pub async fn evaluate_readiness(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    design_revision_ids: &[Uuid],
) -> Result<GateReport, PmWriteError> {
    if design_revision_ids.is_empty() {
        return Err(PmWriteError::Validation(
            "design_revision_ids is required for readiness evaluation",
        ));
    }
    let mut conditions = Vec::with_capacity(5);

    // 条件 2：设计已批准（人类正式批准，AI 草稿不合格）。
    let rows: Vec<(Uuid, i32, String, Uuid)> = sqlx::query_as(
        "SELECT rv.id, rv.version, rv.state, rv.design_id \
         FROM design.design_revision rv \
         WHERE rv.tenant_id = $1 AND rv.project_id = $2 AND rv.id = ANY($3) \
         ORDER BY rv.design_id, rv.version",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(design_revision_ids)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let mut c2_details = Vec::new();
    for missing in design_revision_ids
        .iter()
        .filter(|id| !rows.iter().any(|(rid, _, _, _)| rid == *id))
    {
        c2_details.push(format!("revision {missing} not found in this project"));
    }
    for (rid, version, state, _) in &rows {
        if state != DesignRevisionState::Approved.as_str() {
            c2_details.push(format!(
                "revision {rid} (v{version}) is `{state}`, not approved"
            ));
        }
    }
    conditions.push(GateCondition {
        condition: "design_approved",
        passed: c2_details.is_empty(),
        details: c2_details,
    });

    // 条件 1：需求依据已确认（绑定的需求修订必须 approved 且为当前正式版本）。
    let links: Vec<(Uuid, Uuid, Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT dr.design_revision_id, dr.requirement_id, dr.requirement_revision_id, \
                r.current_revision_id \
         FROM design.design_requirement dr \
         LEFT JOIN delivery.requirement r ON r.id = dr.requirement_id \
         WHERE dr.tenant_id = $1 AND dr.project_id = $2 \
           AND dr.design_revision_id = ANY($3)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(design_revision_ids)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let mut c1_details = Vec::new();
    if links.is_empty() {
        c1_details.push("no requirement basis linked to the selected revisions".to_string());
    }
    for (_rid, requirement_id, revision_id, current) in &links {
        let state: Option<String> = sqlx::query_scalar(
            "SELECT state FROM delivery.requirement_revision \
             WHERE tenant_id = $1 AND id = $2 AND requirement_id = $3",
        )
        .bind(tenant_id)
        .bind(revision_id)
        .bind(requirement_id)
        .fetch_optional(pool)
        .await
        .map_err(PmWriteError::Db)?;
        match state.as_deref() {
            None => c1_details.push(format!(
                "requirement revision {revision_id} not found for requirement {requirement_id}"
            )),
            Some(s) if s != "approved" => c1_details.push(format!(
                "linked requirement revision {revision_id} is `{s}`, not approved"
            )),
            _ => {}
        }
        if state.is_some() && current.is_some() && current.as_ref() != Some(revision_id) {
            // §6.2：需求已被新版本取代而设计仍引用旧版本 → 阻止进入 S04。
            c1_details.push(format!(
                "requirement {requirement_id} moved past linked revision {revision_id}; \
                 impact assessment required"
            ));
        }
    }
    conditions.push(GateCondition {
        condition: "requirements_confirmed",
        passed: c1_details.is_empty(),
        details: c1_details,
    });

    // 条件 3：必要 ADR 已解决（阻塞项必须 accepted；经设计与需求双链收集）。
    let blocking: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT DISTINCT a.id, a.code, a.status \
         FROM design.adr a \
         WHERE a.tenant_id = $1 AND a.project_id = $2 AND a.blockers_implementation \
           AND a.id IN ( \
               SELECT adr_id FROM design.adr_design_revision \
                WHERE tenant_id = $1 AND project_id = $2 \
                  AND design_revision_id = ANY($3) \
               UNION \
               SELECT adr_id FROM design.adr_requirement \
                WHERE tenant_id = $1 AND project_id = $2 \
                  AND requirement_revision_id IN ( \
                      SELECT requirement_revision_id FROM design.design_requirement \
                       WHERE tenant_id = $1 AND project_id = $2 \
                         AND design_revision_id = ANY($3)))",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(design_revision_ids)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let c3_details: Vec<String> = blocking
        .iter()
        .filter(|(_, _, status)| {
            AdrStatus::parse(status)
                .map(|s| !s.resolved_for_implementation())
                .unwrap_or(true)
        })
        .map(|(id, code, status)| format!("blocking ADR {code} ({id}) is `{status}`"))
        .collect();
    conditions.push(GateCondition {
        condition: "adrs_resolved",
        passed: c3_details.is_empty(),
        details: c3_details,
    });

    // 条件 4/5：实施范围与验收标准齐备（按 revision content 结构化字段判定）。
    let mut c4_details = Vec::new();
    let mut c5_details = Vec::new();
    for (rid, _, _, _) in &rows {
        let content: Option<Value> = sqlx::query_scalar(
            "SELECT content FROM design.design_revision \
             WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(rid)
        .fetch_optional(pool)
        .await
        .map_err(PmWriteError::Db)?;
        let Some(content) = content else { continue };
        match scope_included_len(&content) {
            Ok(n) if n > 0 => {}
            Ok(_) => c4_details.push(format!("revision {rid} has no implementation scope")),
            Err(e) => c4_details.push(format!("revision {rid}: {e}")),
        }
        match acceptance_ready(&content) {
            Ok(()) => {}
            Err(e) => c5_details.push(format!("revision {rid}: {e}")),
        }
    }
    conditions.push(GateCondition {
        condition: "scope_defined",
        passed: c4_details.is_empty(),
        details: c4_details,
    });
    conditions.push(GateCondition {
        condition: "acceptance_ready",
        passed: c5_details.is_empty(),
        details: c5_details,
    });

    let ready = conditions.iter().all(|c| c.passed);
    Ok(GateReport {
        ready,
        design_revision_ids: design_revision_ids.to_vec(),
        conditions,
    })
}

fn scope_included_len(content: &Value) -> Result<usize, String> {
    let scope = content
        .get("implementation_scope")
        .ok_or_else(|| "content.implementation_scope missing".to_string())?;
    let included = scope
        .get("included")
        .ok_or_else(|| "implementation_scope.included missing".to_string())?;
    included
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|v| v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false))
                .count()
        })
        .ok_or_else(|| "implementation_scope.included must be an array".to_string())
}

fn acceptance_ready(content: &Value) -> Result<(), String> {
    let criteria = content
        .get("acceptance_criteria")
        .ok_or_else(|| "content.acceptance_criteria missing".to_string())?;
    let items = criteria
        .as_array()
        .ok_or_else(|| "acceptance_criteria must be an array".to_string())?;
    if items.is_empty() {
        return Err("acceptance_criteria is empty".to_string());
    }
    for (i, item) in items.iter().enumerate() {
        let statement = item
            .get("statement")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if statement.is_empty() {
            return Err(format!("acceptance_criteria[{i}] statement is empty"));
        }
        let kind = item
            .get("verification_kind")
            .and_then(Value::as_str)
            .unwrap_or("manual");
        if !VERIFICATION_KINDS.contains(&kind) {
            return Err(format!(
                "acceptance_criteria[{i}] verification_kind `{kind}` is invalid"
            ));
        }
    }
    Ok(())
}
