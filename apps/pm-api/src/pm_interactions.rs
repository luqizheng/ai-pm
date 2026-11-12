//! ACP-03：人工回复入口（设计稿 docs/ui/s03-acp/smark-pm-s03-acp-slice-design-acp03-v1.md）。
//!
//! 薄适配层：① 可见性门（interaction → project → 成员，404 不泄露）+
//! auth_mode 门（api_key 403，`ensure_trusted_context` 先例）；§8.3 全部六项
//! 检查在 `pm-application::interaction::submit_interaction_response` 内
//! fail-closed 二次断言（V-6：正式写操作统一走应用服务）。
//! UI（WP-5 Inbox 投影）调用同一端点。

use axum::extract::{Path, State};
use axum::Extension;
use serde::Deserialize;
use serde_json::Value;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::guard_query_repo;
use pm_application::repo::GuardQueryRepo;
use crate::support::{ensure_trusted_context, require_project_visible, ApiError, ApiResult};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route(
        "/v1/pm/interactions/{interaction_id}/responses",
        axum::routing::post(submit_response),
    )
}

use axum::Router;

#[derive(Debug, Deserialize)]
pub(crate) struct SubmitInteractionResponseRequest {
    pub response_payload: Value,
    pub idempotency_key: String,
}

/// interaction → project 解析（租户内；不存在 404 不泄露存在性）。
async fn interaction_project(
    pool: &sqlx::PgPool,
    interaction_id: uuid::Uuid,
    caller: &AuthedPrincipal,
) -> ApiResult<uuid::Uuid> {
    guard_query_repo(pool)
        .interaction_project_id(caller.tenant_id, interaction_id)
        .await?
        .ok_or(ApiError::NotFound("interaction not found"))
}

async fn submit_response(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(interaction_id): Path<uuid::Uuid>,
    axum::Json(body): axum::Json<SubmitInteractionResponseRequest>,
) -> ApiResult<axum::Json<Value>> {
    // 检查①（可见性）+ 检查②（auth_mode）：ensure_trusted_context 先例
    // （pm_requirements confirm 同门；A*：机器凭据无人类确认语义）。
    let project_id = interaction_project(&state.pool, interaction_id, &caller).await?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    ensure_trusted_context(&caller)?;

    let input = pm_application::interaction::SubmitResponseInput {
        project_id,
        interaction_id,
        responder_principal_id: caller.principal_id,
        idempotency_key: body.idempotency_key,
        response_payload: body.response_payload,
    };
    let out = pm_application::interaction::submit_interaction_response(
        &state.pool,
        &input,
        &caller.audit_actor(),
    )
    .await?;
    let value = serde_json::to_value(&out)
        .map_err(|_| ApiError::BadRequest("response serialization failed"))?;
    Ok(axum::Json(value))
}
