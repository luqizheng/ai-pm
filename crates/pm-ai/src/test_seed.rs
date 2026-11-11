//! 测试共享 seed（#[cfg(test)]）：Phase 3B Role 解析链只认目录面
//! （team_llm_model + authorization + policy），provider
//! `configuration->>'model'` 不再参与解析——依赖旧口径的测试统一走本模块
//! 补齐目录 seed。

/// 建目录模型（enabled）+ team_wide 授权 + Team general 策略。
/// 返回目录模型行 id（llm_call_record.model_ref 归因面）。
pub(crate) async fn seed_model_catalog(
    pool: &sqlx::PgPool,
    tenant_id: uuid::Uuid,
    provider_id: uuid::Uuid,
    model_id: &str,
    principal_id: uuid::Uuid,
) -> uuid::Uuid {
    sqlx::query(
        "INSERT INTO ai.team_llm_model \
         (tenant_id, provider_id, model_id, display_name, enabled, created_by) \
         VALUES ($1, $2, $3, $3, true, $4)",
    )
    .bind(tenant_id)
    .bind(provider_id)
    .bind(model_id)
    .bind(principal_id)
    .execute(pool)
    .await
    .unwrap();
    let model_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM ai.team_llm_model \
         WHERE tenant_id = $1 AND provider_id = $2 AND model_id = $3",
    )
    .bind(tenant_id)
    .bind(provider_id)
    .bind(model_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai.team_llm_model_authorization \
         (tenant_id, model_id, state, granted_by) \
         VALUES ($1, $2, 'team_wide', $3)",
    )
    .bind(tenant_id)
    .bind(model_ref)
    .bind(principal_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai.team_llm_model_policy \
         (tenant_id, role_key, model_id, updated_by) \
         VALUES ($1, 'general', $2, $3)",
    )
    .bind(tenant_id)
    .bind(model_ref)
    .bind(principal_id)
    .execute(pool)
    .await
    .unwrap();
    model_ref
}