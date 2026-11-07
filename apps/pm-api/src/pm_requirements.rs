//! S02-I1 · Requirement 需求核心 REST 适配层。
//!
//! 统一服务（`pm_application::requirement`，S02D-01）的薄适配：业务裁决零实现，
//! 只做（a）`require_project_visible` 成员闸（DEF-01 六点的 REST 面）、（b）确认
//! 类命令的可信交互上下文判定（`api_key` 鉴权拒绝，§7.2 A*）、（c）错误映射。
//! 旧 REST 直改面（`POST .../revisions` 的 pm_writes 版与 `.../approve`）已随本
//! 模块退役——draft→approved 直跳废除（S02D-04），必须经 submit → confirm。

use axum::{
    extract::{Extension, Path, State},
    routing::{get, patch, post},
    Json, Router,
};
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{ensure_trusted_context, require_project_visible, ApiError, ApiResult};
use pm_application::requirement::{
    self, ConfirmRevisionRequest, CreateRequirementRequest, DraftRevisionRequest,
    LifecycleAction, RejectRevisionRequest,
};
use pm_application::requirement_traceability::{
    self, LinkFindingRequest, LinkSourceInputRequest, RegisterSourceInputRequest,
};

/// 需求级端点公共前置：租户边界（越权/不存在一律 404）+ 项目成员可见性。
async fn require_requirement_visible(
    state: &AppState,
    requirement_id: Uuid,
    principal: &AuthedPrincipal,
) -> ApiResult<Uuid> {
    let project_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT project_id FROM delivery.requirement WHERE id = $1 AND tenant_id = $2",
    )
    .bind(requirement_id)
    .bind(principal.tenant_id)
    .fetch_optional(&state.pool)
    .await?;
    let project_id = project_id.ok_or(ApiError::NotFound("requirement not found"))?;
    require_project_visible(&state.pool, project_id, principal.tenant_id, principal.principal_id)
        .await?;
    Ok(project_id)
}

// ────────────────────────────────────────────────── 项目作用域（列表/创建）

async fn list_requirements(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id)
        .await?;
    let items = requirement::list_requirements(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn create_requirement(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateRequirementRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id)
        .await?;
    let entry = requirement::create_requirement(
        &state.pool,
        caller.tenant_id,
        project_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "requirement": entry })),
    ))
}

// ────────────────────────────────────────────────────── 需求作用域（详情）

async fn get_requirement(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(requirement_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_requirement_visible(&state, requirement_id, &caller).await?;
    let detail =
        requirement::requirement_detail(&state.pool, caller.tenant_id, requirement_id).await?;
    Ok(Json(serde_json::json!({
        "requirement": detail.requirement,
        "revisions": detail.revisions,
        "confirmations": detail.confirmations,
    })))
}

// ────────────────────────────────────────────────────────── 修订命令

async fn create_draft(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(requirement_id): Path<Uuid>,
    Json(body): Json<DraftRevisionRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_requirement_visible(&state, requirement_id, &caller).await?;
    let revision =
        requirement::create_draft(&state.pool, caller.tenant_id, requirement_id, body, &caller.audit_actor())
            .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "revision": revision })),
    ))
}

async fn update_draft(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((requirement_id, revision_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<DraftRevisionRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    require_requirement_visible(&state, requirement_id, &caller).await?;
    let revision = requirement::update_draft(
        &state.pool,
        caller.tenant_id,
        requirement_id,
        revision_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "revision": revision })))
}

async fn submit_revision(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((requirement_id, revision_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_requirement_visible(&state, requirement_id, &caller).await?;
    let revision = requirement::submit_revision(
        &state.pool,
        caller.tenant_id,
        requirement_id,
        revision_id,
        &caller.audit_actor(),
        &state.knowledge,
    )
    .await?;
    Ok(Json(serde_json::json!({ "revision": revision })))
}

async fn confirm_revision(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((requirement_id, revision_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<ConfirmRevisionRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    require_requirement_visible(&state, requirement_id, &caller).await?;
    ensure_trusted_context(&caller)?;
    let entry = requirement::confirm_revision(
        &state.pool,
        caller.tenant_id,
        requirement_id,
        revision_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "requirement": entry })))
}

async fn reject_revision(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((requirement_id, revision_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<RejectRevisionRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    require_requirement_visible(&state, requirement_id, &caller).await?;
    ensure_trusted_context(&caller)?;
    let revision = requirement::reject_revision(
        &state.pool,
        caller.tenant_id,
        requirement_id,
        revision_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "revision": revision })))
}

// ─────────────────────────────────────────────────── 生命周期治理命令

async fn withdraw_requirement(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(requirement_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    lifecycle(state, caller, requirement_id, LifecycleAction::Withdraw).await
}

async fn restore_requirement(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(requirement_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    lifecycle(state, caller, requirement_id, LifecycleAction::Restore).await
}

async fn archive_requirement(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(requirement_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    lifecycle(state, caller, requirement_id, LifecycleAction::Archive).await
}

async fn lifecycle(
    state: AppState,
    caller: AuthedPrincipal,
    requirement_id: Uuid,
    action: LifecycleAction,
) -> ApiResult<Json<serde_json::Value>> {
    require_requirement_visible(&state, requirement_id, &caller).await?;
    ensure_trusted_context(&caller)?;
    let entry = requirement::transition_lifecycle(
        &state.pool,
        caller.tenant_id,
        requirement_id,
        action,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "requirement": entry })))
}

// ────────────────────────────────────────── Source & Traceability（S02-I3）

async fn register_source_input(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<RegisterSourceInputRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id)
        .await?;
    let entry = requirement_traceability::register_source_input(
        &state.pool,
        caller.tenant_id,
        project_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "source_input": entry })),
    ))
}

async fn list_requirement_sources(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(requirement_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_requirement_visible(&state, requirement_id, &caller).await?;
    let items = requirement_traceability::list_requirement_sources(
        &state.pool,
        caller.tenant_id,
        requirement_id,
    )
    .await?;
    Ok(Json(items))
}

async fn link_source_input(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(requirement_id): Path<Uuid>,
    Json(body): Json<LinkSourceInputRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_requirement_visible(&state, requirement_id, &caller).await?;
    let entry = requirement_traceability::link_source_input(
        &state.pool,
        caller.tenant_id,
        requirement_id,
        body.input_id,
        &caller.audit_actor(),
    )
    .await?;
    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!(entry))))
}

async fn unlink_source_input(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((requirement_id, input_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<axum::http::StatusCode> {
    require_requirement_visible(&state, requirement_id, &caller).await?;
    requirement_traceability::unlink_source_input(
        &state.pool,
        caller.tenant_id,
        requirement_id,
        input_id,
        &caller.audit_actor(),
    )
    .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn list_requirement_findings(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(requirement_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_requirement_visible(&state, requirement_id, &caller).await?;
    let items = requirement_traceability::list_requirement_findings(
        &state.pool,
        caller.tenant_id,
        requirement_id,
    )
    .await?;
    Ok(Json(items))
}

async fn link_finding(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(requirement_id): Path<Uuid>,
    Json(body): Json<LinkFindingRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_requirement_visible(&state, requirement_id, &caller).await?;
    let entry = requirement_traceability::link_finding(
        &state.pool,
        caller.tenant_id,
        requirement_id,
        body.finding_id,
        &caller.audit_actor(),
    )
    .await?;
    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!(entry))))
}

async fn unlink_finding(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((requirement_id, finding_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<axum::http::StatusCode> {
    require_requirement_visible(&state, requirement_id, &caller).await?;
    requirement_traceability::unlink_finding(
        &state.pool,
        caller.tenant_id,
        requirement_id,
        finding_id,
        &caller.audit_actor(),
    )
    .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─────────────────────────────────────────────────────────────── 路由

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/requirements",
            get(list_requirements).post(create_requirement),
        )
        .route("/v1/pm/requirements/{requirement_id}", get(get_requirement))
        .route(
            "/v1/pm/requirements/{requirement_id}/revisions",
            post(create_draft),
        )
        .route(
            "/v1/pm/requirements/{requirement_id}/revisions/{revision_id}",
            patch(update_draft),
        )
        .route(
            "/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/submit",
            post(submit_revision),
        )
        .route(
            "/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/confirm",
            post(confirm_revision),
        )
        .route(
            "/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/reject",
            post(reject_revision),
        )
        .route(
            "/v1/pm/requirements/{requirement_id}/withdraw",
            post(withdraw_requirement),
        )
        .route(
            "/v1/pm/requirements/{requirement_id}/restore",
            post(restore_requirement),
        )
        .route(
            "/v1/pm/requirements/{requirement_id}/archive",
            post(archive_requirement),
        )
        .route(
            "/v1/pm/projects/{project_id}/source-inputs",
            post(register_source_input),
        )
        .route(
            "/v1/pm/requirements/{requirement_id}/source-inputs",
            get(list_requirement_sources).post(link_source_input),
        )
        .route(
            "/v1/pm/requirements/{requirement_id}/source-inputs/{input_id}",
            axum::routing::delete(unlink_source_input),
        )
        .route(
            "/v1/pm/requirements/{requirement_id}/findings",
            get(list_requirement_findings).post(link_finding),
        )
        .route(
            "/v1/pm/requirements/{requirement_id}/findings/{finding_id}",
            axum::routing::delete(unlink_finding),
        )
}
