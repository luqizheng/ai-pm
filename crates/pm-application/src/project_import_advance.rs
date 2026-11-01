/// 审批决策后推进导入状态：
/// - 任一提案被驳回 → import `rejected`（已批准部分不回滚）；
/// - 全部 executed → import `applied` + 项目 `planning` + 审计；
/// - 存在 failed → import 保持 `proposed` 并标 `partial`。
pub async fn advance_import_after_decision(
    pool: &PgPool,
    analysis_run_id: Uuid,
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
        let actor = owner_actor(pool, project_id).await?;
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
        .bind(actor)
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