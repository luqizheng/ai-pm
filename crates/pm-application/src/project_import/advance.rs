use super::*;

/// 已生成提案的批次（审批页整批批准用）。
pub async fn import_proposals(
    pool: &PgPool,
    project_id: Uuid,
    import_id: Uuid,
) -> Result<ProposedImport, ImportError> {
    let row: Option<(String, Option<Uuid>)> = sqlx::query_as(
        "SELECT status, analysis_run_id FROM integration.repo_import \
         WHERE id = $1 AND project_id = $2",
    )
    .bind(import_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let (status, analysis_run_id) = row.ok_or(ImportError::NotFound)?;
    if status != "proposed" && status != "applied" {
        return Err(ImportError::InvalidState("import has no proposals yet"));
    }
    let run_id = analysis_run_id.ok_or(ImportError::InvalidState("import has no analysis run"))?;
    existing_proposals(pool, project_id, run_id).await
}


/// 审批决策后推进导入状态：
/// - 任一提案被驳回 → import `rejected`（已批准部分不回滚）；
/// - 全部 executed → import `applied` + 项目 `planning` + 审计；
/// - 存在 failed → import 保持 `proposed` 并标 `partial`。
pub async fn advance_import_after_decision(
    pool: &PgPool,
    analysis_run_id: Uuid,
    actor: &AuditActor,
) -> Result<(), ImportError> {
    let import: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, project_id FROM integration.repo_import WHERE analysis_run_id = $1",
    )
    .bind(analysis_run_id)
    .fetch_optional(pool)
    .await?;
    let Some((import_id, project_id)) = import else {
        return Ok(());
    };
    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM ai.change_proposal WHERE analysis_run_id = $1")
            .bind(analysis_run_id)
            .fetch_all(pool)
            .await?;
    if statuses.is_empty() {
        return Ok(());
    }

    if statuses.iter().any(|status| status == "vetoed") {
        sqlx::query(
            "UPDATE integration.repo_import SET status = 'rejected', updated_at = now() \
             WHERE id = $1",
        )
        .bind(import_id)
        .execute(pool)
        .await?;
        return Ok(());
    }
    if statuses.iter().all(|status| status == "executed") {
        let mut tx = pool.begin().await?;
        sqlx::query(
            "UPDATE integration.repo_import SET status = 'applied', applied_at = now(), \
             error_code = NULL, error_detail = NULL, updated_at = now() WHERE id = $1",
        )
        .bind(import_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE core.project SET status = 'planning', updated_at = now() WHERE id = $1",
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO governance.audit_log \
             (project_id, actor_id, action, target_type, target_id, after_data, analysis_run_id, tenant_id) \
             VALUES ($1, $2, 'project_import_applied', 'project', $1, $3, $4, \
                     (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(project_id)
        .bind(actor.principal_id)
        .bind(serde_json::json!({ "import_id": import_id }))
        .bind(analysis_run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(());
    }
    if statuses.iter().any(|status| status == "failed") {
        sqlx::query(
            "UPDATE integration.repo_import SET status = 'proposed', error_code = 'partial', \
             error_detail = 'some import proposals failed to execute', updated_at = now() \
             WHERE id = $1",
        )
        .bind(import_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}


pub(crate) async fn existing_proposals(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
) -> Result<ProposedImport, ImportError> {
    let rows: Vec<(String, Uuid, Uuid)> = sqlx::query_as(
        "SELECT p.command_type, p.id, a.id \
         FROM ai.change_proposal p \
         JOIN governance.approval_request a ON a.proposal_id = p.id \
         WHERE p.project_id = $1 AND p.analysis_run_id = $2 ORDER BY p.proposal_seq",
    )
    .bind(project_id)
    .bind(run_id)
    .fetch_all(pool)
    .await?;
    let proposals = rows
        .into_iter()
        .map(|(kind, proposal_id, approval_id)| ProposalRef {
            kind,
            proposal_id,
            approval_id,
            item_count: 0,
        })
        .collect();
    Ok(ProposedImport {
        outcome: "proposed".to_string(),
        proposals,
        analysis_run_id: run_id,
    })
}

