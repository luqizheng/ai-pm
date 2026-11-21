//! `/v1/pm/teams/{team_id}/products` 与 `/v1/pm/products/{product_id}/*`
//! （P4-ORG Phase 3；PA 基线 §2/§3/§4/§5；DR-PA-01/02/03）：
//! Product Membership 内部管理 + Owner 保护 + {active, abandoned} 生命周期
//! + Product↔Project Link。
//!
//! 授权事实源 = `core.product_member.role`（owner|member）与 `core.team_member`
//! （Team Owner 恢复性任命 / Owner 变更 OPEN-3 域）。跨 Team 一律 404。
//!
//! 稳定错误 code（`{code, message}` 或 `{message}` 契约）：
//! team_owner_required / product_owner_required / not_a_product_member /
//! last_owner / already_product_member / already_owner / already_member /
//! product_member_not_found / product_abandoned / link_already_exists /
//! product_lifecycle_noop。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use pm_application::product::{self, ProductError};

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::team_llm_guard::{guard_team, err_not_found};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        // Team 面：创建（Team Owner 指定首位 Owner）与列表（Owner 全量 / Member 过滤）。
        .route(
            "/v1/pm/teams/{team_id}/products",
            get(list_products_handler).post(create_product_handler),
        )
        .route("/v1/pm/products/{product_id}", get(product_profile_handler))
        // 成员（Product Member 平权添加/移除普通成员；Owner 变更走专用端点）。
        .route(
            "/v1/pm/products/{product_id}/members",
            get(list_members_handler).post(add_member_handler),
        )
        .route(
            "/v1/pm/products/{product_id}/members/{principal_id}",
            axum::routing::delete(remove_member_handler),
        )
        .route(
            "/v1/pm/products/{product_id}/members/{principal_id}/appoint-owner",
            post(appoint_owner_handler),
        )
        .route(
            "/v1/pm/products/{product_id}/members/{principal_id}/demote-owner",
            post(demote_owner_handler),
        )
        // 生命周期（任意有效 Product Owner）。
        .route(
            "/v1/pm/products/{product_id}/lifecycle",
            post(lifecycle_handler),
        )
        // Link（abandoned 拦截第一消费点）。
        .route(
            "/v1/pm/products/{product_id}/links",
            get(list_links_handler).post(create_link_handler),
        )
        .route("/v1/pm/product-links/{link_id}/end", post(end_link_handler))
        // PA-07 项目级产品读面（Figma 179:3603 E 系列）。
        .route(
            "/v1/pm/projects/{project_id}/products",
            get(project_products_handler),
        )
        .route("/v1/pm/products/{product_id}/tasks", get(product_tasks_handler))
}

// ─────────────────────────────────────────────────────────────── 错误映射

fn api_err(err: ProductError) -> Response {
    match err {
        ProductError::NotFound => err_not_found("product not found"),
        ProductError::Validation(m) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": m })),
        )
            .into_response(),
        ProductError::Conflict(m) => (
            StatusCode::CONFLICT,
            Json(json!({ "message": m })),
        )
            .into_response(),
        ProductError::Forbidden(m) => err_forbidden(&m),
        ProductError::RuleRejected(code, message) => match code.as_str() {
            "not_a_product_member"
            | "team_owner_required"
            | "product_owner_required" => err_forbidden(&message),
            "product_member_not_found" => err_not_found(&message),
            // last_owner / already_* / 其他 → 409。
            _ => (
                StatusCode::CONFLICT,
                Json(json!({ "code": code, "message": message })),
            )
                .into_response(),
        },
        ProductError::Db(e) => {
            tracing::error!(error = %e, "product operation failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "message": "internal error" })),
            )
                .into_response()
        }
    }
}

fn err_forbidden(message: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({ "message": message })),
    )
        .into_response()
}

fn db_ok<T>(result: Result<T, ProductError>, render: impl Fn(T) -> Response) -> Response {
    match result {
        Ok(v) => render(v),
        Err(e) => api_err(e),
    }
}

// ─────────────────────────────────────────────────────────────── Team 面

#[derive(Debug, Deserialize)]
struct CreateProductBody {
    name: String,
    #[serde(default)]
    description: Option<String>,
    first_owner_principal_id: Uuid,
}

async fn create_product_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(team_id): Path<Uuid>,
    Json(body): Json<CreateProductBody>,
) -> Response {
    if let Err(resp) = guard_team(&caller, &state.pool, team_id).await {
        return resp;
    }
    let actor = caller.audit_actor();
    db_ok(
        product::create_product(
            &state.pool,
            &actor,
            caller.principal_id,
            team_id,
            &body.name,
            body.description.as_deref(),
            body.first_owner_principal_id,
        )
        .await,
    |p| (StatusCode::CREATED, Json(p)).into_response(),
    )
}

async fn list_products_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(team_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = guard_team(&caller, &state.pool, team_id).await {
        return resp;
    }
    db_ok(
        product::list_products(&state.pool, team_id, caller.principal_id).await,
        |rows| Json(json!({ "items": rows })).into_response(),
    )
}

// ─────────────────────────────────────────────────────────────── Product 面

async fn product_profile_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(product_id): Path<Uuid>,
) -> Response {
    db_ok(
        product::product_profile(&state.pool, product_id, caller.principal_id).await,
        |p| Json(p).into_response(),
    )
}

/// `GET /v1/pm/projects/{project_id}/products`：项目关联产品（link 有效行 +
/// 产品 profile + 本项目内关联任务计数）。项目成员可见。
async fn project_products_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(resp) =
        crate::support::require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id)
            .await
    {
        return resp.into_response();
    }
    db_ok(
        product::list_project_products(
            &state.pool,
            caller.tenant_id,
            project_id,
            caller.principal_id,
        )
        .await,
        |rows| Json(json!({ "items": rows })).into_response(),
    )
}

/// `GET /v1/pm/products/{product_id}/tasks`：产品关联任务反查（E 页 Tasks tab /
/// Impact Summary；行含 PA-07 调度原料 deps_total/deps_met）。Product Member 可见。
async fn product_tasks_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(product_id): Path<Uuid>,
) -> Response {
    db_ok(
        product::list_product_tasks(&state.pool, product_id, caller.principal_id).await,
        |rows| Json(json!({ "items": rows })).into_response(),
    )
}

async fn list_members_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(product_id): Path<Uuid>,
) -> Response {
    db_ok(
        product::list_product_members(&state.pool, product_id, caller.principal_id).await,
        |rows| Json(json!({ "items": rows })).into_response(),
    )
}

#[derive(Debug, Deserialize)]
struct AddMemberBody {
    principal_id: Uuid,
}

async fn add_member_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(product_id): Path<Uuid>,
    Json(body): Json<AddMemberBody>,
) -> Response {
    let actor = caller.audit_actor();
    db_ok(
        product::add_product_member(
            &state.pool,
            &actor,
            caller.principal_id,
            product_id,
            body.principal_id,
        )
        .await,
        |_| (StatusCode::OK, Json(json!({ "added": true }))).into_response(),
    )
}

async fn remove_member_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((product_id, principal_id)): Path<(Uuid, Uuid)>,
) -> Response {
    let actor = caller.audit_actor();
    db_ok(
        product::remove_product_member(
            &state.pool,
            &actor,
            caller.principal_id,
            product_id,
            principal_id,
        )
        .await,
        |_| (StatusCode::OK, Json(json!({ "removed": true }))).into_response(),
    )
}

async fn appoint_owner_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((product_id, principal_id)): Path<(Uuid, Uuid)>,
) -> Response {
    let actor = caller.audit_actor();
    db_ok(
        product::appoint_product_owner(
            &state.pool,
            &actor,
            caller.principal_id,
            product_id,
            principal_id,
        )
        .await,
        |_| (StatusCode::OK, Json(json!({ "appointed": true }))).into_response(),
    )
}

async fn demote_owner_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((product_id, principal_id)): Path<(Uuid, Uuid)>,
) -> Response {
    let actor = caller.audit_actor();
    db_ok(
        product::demote_product_owner(
            &state.pool,
            &actor,
            caller.principal_id,
            product_id,
            principal_id,
        )
        .await,
        |_| (StatusCode::OK, Json(json!({ "demoted": true }))).into_response(),
    )
}

// ─────────────────────────────────────────────────────────────── 生命周期

#[derive(Debug, Deserialize)]
struct LifecycleBody {
    action: String,
}

async fn lifecycle_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(product_id): Path<Uuid>,
    Json(body): Json<LifecycleBody>,
) -> Response {
    let actor = caller.audit_actor();
    db_ok(
        product::set_product_lifecycle(
            &state.pool,
            &actor,
            caller.principal_id,
            product_id,
            &body.action,
        )
        .await,
        |p| Json(p).into_response(),
    )
}

// ─────────────────────────────────────────────────────────────── Link

async fn list_links_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(product_id): Path<Uuid>,
) -> Response {
    db_ok(
        product::list_product_links(&state.pool, product_id, caller.principal_id).await,
        |rows| Json(json!({ "items": rows })).into_response(),
    )
}

#[derive(Debug, Deserialize)]
struct CreateLinkBody {
    project_id: Uuid,
}

async fn create_link_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(product_id): Path<Uuid>,
    Json(body): Json<CreateLinkBody>,
) -> Response {
    let actor = caller.audit_actor();
    db_ok(
        product::create_product_link(
            &state.pool,
            &actor,
            caller.principal_id,
            product_id,
            body.project_id,
        )
        .await,
        |id| (StatusCode::CREATED, Json(json!({ "id": id }))).into_response(),
    )
}

async fn end_link_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(link_id): Path<Uuid>,
) -> Response {
    let actor = caller.audit_actor();
    db_ok(
        product::end_product_link(&state.pool, &actor, caller.principal_id, link_id).await,
        |_| (StatusCode::OK, Json(json!({ "ended": true }))).into_response(),
    )
}
