//! `/v1/pm/tenants/{tenant_id}/llm-models*` + `/llm-model-policies` +
//! `/llm-model-authorizations`：Team 模型目录/授权/Role 策略管理面
//! （Phase 3A；设计基线 docs/smark-pm-team-llm-provider-management-phase3-design-v1.md
//! §5；权限矩阵 = Team Owner/Admin，同 team_llm::router 口径）。
//!
//! `/projects/{project_id}/ai-model-policies`（语义扩展：Provider 约束 +
//! Role 覆盖 + Team 默认参照）留在 team_llm::router——守卫
//! `guard_project_manager` 是项目域。

use axum::{
    extract::{rejection::JsonRejection, Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use pm_application::team_llm::TeamLlmError;
use pm_application::team_llm_catalog::{
    create_model, get_team_policies, list_models, set_team_policies, update_model,
    upsert_authorizations, AuthorizationItem, CreateModelRequest, PolicyItem, UpdateModelRequest,
};

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::team_llm::audit_actor;
use crate::team_llm::error_response;
use crate::team_llm_guard::{err_forbidden, guard_tenant};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/tenants/{tenant_id}/llm-models",
            get(list_models_handler).post(create_model_handler),
        )
        .route(
            "/v1/pm/tenants/{tenant_id}/llm-models/{model_row_id}",
            axum::routing::patch(update_model_handler),
        )
        .route(
            "/v1/pm/tenants/{tenant_id}/llm-model-authorizations",
            axum::routing::put(authorizations_handler),
        )
        .route(
            "/v1/pm/tenants/{tenant_id}/llm-model-policies",
            get(team_policies_get_handler).put(team_policies_put_handler),
        )
}

async fn require_admin(
    caller: &AuthedPrincipal,
    tenant_id: Uuid,
) -> Result<(), Response> {
    if let Err(resp) = guard_tenant(caller, tenant_id) {
        return Err(resp);
    }
    if !caller.is_admin {
        return Err(err_forbidden("tenant admin required"));
    }
    Ok(())
}

#[derive(Deserialize)]
struct ListModelsQuery {
    #[serde(default)]
    provider_id: Option<Uuid>,
    #[serde(default)]
    q: Option<String>,
}

async fn list_models_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
    Query(query): Query<ListModelsQuery>,
) -> Response {
    if let Err(resp) = require_admin(&caller, tenant_id).await {
        return resp;
    }
    match list_models(&state.pool, tenant_id, query.provider_id).await {
        Ok(mut items) => {
            // q 精确过滤（display_name/model_id 前缀/子串；目录规模小，内存过滤）
            if let Some(q) = query.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                let needle = q.to_lowercase();
                items.retain(|row| {
                    row.display_name.to_lowercase().contains(&needle)
                        || row.model_id.to_lowercase().contains(&needle)
                });
            }
            let total = items.len();
            (StatusCode::OK, Json(json!({ "items": items, "total": total }))).into_response()
        }
        Err(err) => error_response(err),
    }
}

async fn create_model_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
    body: Result<Json<CreateModelRequest>, JsonRejection>,
) -> Response {
    // 守卫先于 body 提取：权限拒绝（403）优先于媒体类型/JSON 错误（415/400）
    if let Err(resp) = require_admin(&caller, tenant_id).await {
        return resp;
    }
    let Ok(Json(body)) = body else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "invalid JSON body" })),
        )
            .into_response();
    };
    match create_model(&state.pool, &audit_actor(&caller), tenant_id, body).await {
        Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
        Err(err) => error_response(err),
    }
}

async fn update_model_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((tenant_id, model_row_id)): Path<(Uuid, Uuid)>,
    body: Result<Json<UpdateModelRequest>, JsonRejection>,
) -> Response {
    // 守卫先于 body 提取（同上）
    if let Err(resp) = require_admin(&caller, tenant_id).await {
        return resp;
    }
    let Ok(Json(body)) = body else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "invalid JSON body" })),
        )
            .into_response();
    };
    match update_model(&state.pool, &audit_actor(&caller), tenant_id, model_row_id, body).await {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
        Err(err) => error_response(err),
    }
}

#[derive(Deserialize)]
struct AuthorizationsBody {
    #[serde(default)]
    items: Vec<AuthorizationItem>,
}

async fn authorizations_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
    body: Result<Json<AuthorizationsBody>, JsonRejection>,
) -> Response {
    // 守卫先于 body 提取（同上）
    if let Err(resp) = require_admin(&caller, tenant_id).await {
        return resp;
    }
    let Ok(Json(body)) = body else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "invalid JSON body" })),
        )
            .into_response();
    };
    match upsert_authorizations(&state.pool, &audit_actor(&caller), tenant_id, body.items).await {
        Ok(rows) => (StatusCode::OK, Json(json!({ "items": rows }))).into_response(),
        Err(err) => error_response(err),
    }
}

async fn team_policies_get_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = require_admin(&caller, tenant_id).await {
        return resp;
    }
    match get_team_policies(&state.pool, tenant_id).await {
        Ok(rows) => (StatusCode::OK, Json(json!({ "items": rows }))).into_response(),
        Err(err) => error_response(err),
    }
}

#[derive(Deserialize)]
struct TeamPoliciesBody {
    #[serde(default)]
    items: Vec<PolicyItem>,
}

async fn team_policies_put_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
    body: Result<Json<TeamPoliciesBody>, JsonRejection>,
) -> Response {
    // 守卫先于 body 提取（同上）
    if let Err(resp) = require_admin(&caller, tenant_id).await {
        return resp;
    }
    let Ok(Json(body)) = body else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "invalid JSON body" })),
        )
            .into_response();
    };
    match set_team_policies(&state.pool, &audit_actor(&caller), tenant_id, body.items).await {
        Ok(rows) => (StatusCode::OK, Json(json!({ "items": rows }))).into_response(),
        Err(err) => error_response(err),
    }
}

// 保持 TeamLlmError 引用（错误面与 Provider 面一致；handler 经 error_response 映射）
#[allow(dead_code)]
fn _error_type_marker(err: TeamLlmError) -> TeamLlmError {
    err
}