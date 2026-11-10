//! S04 · 任务计划 REST 适配层。
//!
//! 统一服务（`pm_application::task_plan` + `pm_ai::plan_decomposition`）的
//! 薄适配：业务裁决零实现，只做（a）`require_project_visible` 成员闸、
//! （b）确认类命令的可信交互上下文判定（`ensure_trusted_context`：api_key
//! 拒绝——计划批准是真人命令，AI 无批准路径）、（c）错误映射。
//!
//! 权限矩阵（S4 提案 §5）：
//! - AI 主体：创建计划/修订、发起拆解、改稿、提交审核、请求批准——全部允许
//!   （产物一律 draft，来源留痕）；
//! - 真人：批准/退回（decide）、正式化置 ready（复用 `pm_writes` mark-ready）、
//!   计划替代；decide 端点走 `pm_reviews` 同款真人门。

use axum::{
    extract::{Extension, Path, State},
    routing::{get, patch, post},
    Json, Router,
};
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{ensure_trusted_context, require_project_visible, ApiError, ApiResult};
use pm_application::task_plan::{
    self, CreatePlanRequest, DraftPlanRevisionRequest, MaterializeRequest, SupersedePlanRequest,
};

/// 计划级端点公共前置：租户边界（越权/不存在一律 404）+ 项目成员可见性。
async fn require_plan_visible(
    state: &AppState,
    plan_id: Uuid,
    principal: &AuthedPrincipal,
) -> ApiResult<Uuid> {
    let project_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT project_id FROM plan.task_plan WHERE id = $1 AND tenant_id = $2",
    )
    .bind(plan_id)
    .bind(principal.tenant_id)
    .fetch_optional(&state.pool)
    .await?;
    let project_id = project_id.ok_or(ApiError::NotFound("task plan not found"))?;
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    Ok(project_id)
}

// ────────────────────────────────────────────────── 计划 CRUD

async fn list_plans(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let items = task_plan::list_plans(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn create_plan(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreatePlanRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let detail = task_plan::create_plan(
        &state.pool,
        caller.tenant_id,
        project_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!(detail)),
    ))
}

async fn get_plan(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(plan_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_plan_visible(&state, plan_id, &caller).await?;
    let detail = task_plan::plan_detail(&state.pool, caller.tenant_id, plan_id).await?;
    Ok(Json(serde_json::json!(detail)))
}

async fn create_revision(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(plan_id): Path<Uuid>,
    Json(body): Json<DraftPlanRevisionRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_plan_visible(&state, plan_id, &caller).await?;
    let revision = task_plan::create_plan_revision(
        &state.pool,
        caller.tenant_id,
        plan_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "revision": revision })),
    ))
}

async fn update_revision_draft(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((plan_id, revision_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<DraftPlanRevisionRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    require_plan_visible(&state, plan_id, &caller).await?;
    let revision = task_plan::update_plan_revision_draft(
        &state.pool,
        caller.tenant_id,
        plan_id,
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
    Path((plan_id, revision_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_plan_visible(&state, plan_id, &caller).await?;
    let revision = task_plan::submit_plan_revision(
        &state.pool,
        caller.tenant_id,
        plan_id,
        revision_id,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "revision": revision })))
}

// ─────────────────────────────────────────────────────── AI 拆解 / 重拆

/// 发起 AI 拆解/重拆（AI/人类皆可；产物 draft，不直接写正式任务）。
async fn decompose(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<pm_ai::plan_decomposition::DecomposeRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let outcome = pm_ai::plan_decomposition::decompose(
        &state.pool,
        caller.tenant_id,
        project_id,
        body,
        &caller.audit_actor(),
    )
    .await
    .map_err(ApiError::from)?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!(outcome)),
    ))
}

/// 拆解历史（本计划的 AI 修订清单）。
async fn list_decompositions(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(plan_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_plan_visible(&state, plan_id, &caller).await?;
    let items =
        pm_ai::plan_decomposition::list_decompositions(&state.pool, caller.tenant_id, plan_id)
            .await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

// ────────────────────────────────────────────────── 审批（真人门）

async fn request_plan_approval(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(plan_id): Path<Uuid>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_plan_visible(&state, plan_id, &caller).await?;
    let approval = task_plan::request_plan_approval(
        &state.pool,
        caller.tenant_id,
        plan_id,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "approval": approval })),
    ))
}

// 裁决走统一端点 POST /v1/pm/design-approvals/{id}/decide（pm_designs），
// 该端点按 kind 分派到 task_plan::apply_plan_decision（真人门双闸同款）。

// ──────────────────────────────────────────────────── 正式化 / 交接 / 进度

/// 正式化：批准修订 → 正式任务（draft；幂等；AI 可触发机械转写）。
async fn materialize(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(plan_id): Path<Uuid>,
    Json(body): Json<MaterializeRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    require_plan_visible(&state, plan_id, &caller).await?;
    let outcome = task_plan::materialize_plan(
        &state.pool,
        caller.tenant_id,
        plan_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(outcome))
}

/// 执行资格检查 + 上下文载荷（S05 交接）。
async fn handoff(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(plan_id): Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    require_plan_visible(&state, plan_id, &caller).await?;
    let task_id = query
        .get("task_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or(ApiError::BadRequest("task_id query parameter is required"))?;
    let payload = task_plan::prepare_handoff(&state.pool, caller.tenant_id, task_id).await?;
    Ok(Json(payload))
}

/// 计划整体进度 + 阻塞清单。
async fn progress(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(plan_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_plan_visible(&state, plan_id, &caller).await?;
    let report = task_plan::plan_progress(&state.pool, caller.tenant_id, plan_id).await?;
    Ok(Json(serde_json::json!(report)))
}

/// 计划替代（真人门）。
async fn supersede_plan(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(plan_id): Path<Uuid>,
    Json(body): Json<SupersedePlanRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    require_plan_visible(&state, plan_id, &caller).await?;
    ensure_trusted_context(&caller)?;
    let entry = task_plan::supersede_plan(
        &state.pool,
        caller.tenant_id,
        plan_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "plan": entry })))
}

impl From<pm_ai::plan_decomposition::DecomposeError> for ApiError {
    fn from(e: pm_ai::plan_decomposition::DecomposeError) -> Self {
        ApiError::from(pm_application::pm_write::PmWriteError::from(e))
    }
}

// ─────────────────────────────────────────────────────────────── 路由

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/task-plans",
            get(list_plans).post(create_plan),
        )
        .route("/v1/pm/task-plans/{plan_id}", get(get_plan))
        .route(
            "/v1/pm/task-plans/{plan_id}/revisions",
            post(create_revision),
        )
        .route(
            "/v1/pm/task-plans/{plan_id}/revisions/{revision_id}",
            patch(update_revision_draft),
        )
        .route(
            "/v1/pm/task-plans/{plan_id}/revisions/{revision_id}/submit",
            post(submit_revision),
        )
        .route(
            "/v1/pm/task-plans/{plan_id}/approvals",
            post(request_plan_approval),
        )
        .route("/v1/pm/task-plans/{plan_id}/materialize", post(materialize))
        .route("/v1/pm/task-plans/{plan_id}/handoff", get(handoff))
        .route("/v1/pm/task-plans/{plan_id}/progress", get(progress))
        .route(
            "/v1/pm/task-plans/{plan_id}/supersede",
            post(supersede_plan),
        )
        .route(
            "/v1/pm/projects/{project_id}/task-plans/decompose",
            post(decompose),
        )
        .route(
            "/v1/pm/task-plans/{plan_id}/decompositions",
            get(list_decompositions),
        )
}
