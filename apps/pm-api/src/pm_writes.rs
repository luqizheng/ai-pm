//! `/v1/pm/*` 业务写端点（Slice A）：审批决策、需求修订起草/定稿、任务创建/编辑。
//! 校验顺序与错误体对齐 `frontend/src/api/mock/fixtures.ts`（404 → 409 →
//! 400 decision → 400 reason；`{message}` 裸体）；决策复用
//! `supervision::decide_approval`（approve 执行提案命令、reject veto）。

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{ApiError, ApiResult};
use pm_application::pm_write::{
    self, CreateRevisionRequest, CreateTaskRequest, UpdateTaskRequest,
};
use pm_application::supervision::{DecideRequest, Decision, SupervisionError};

fn conflict(message: &'static str) -> Response {
    (StatusCode::CONFLICT, Json(json!({ "message": message }))).into_response()
}

// ------------------------------------------------------------------ decide

#[derive(Debug, Deserialize)]
pub struct DecideBody {
    #[serde(default)]
    pub decision: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

pub(crate) async fn decide_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(approval_id): Path<Uuid>,
    Json(body): Json<DecideBody>,
) -> Response {
    // mock 校验顺序：存在性 → pending → decision → reason。
    let (project_id, status): (Uuid, String) = match sqlx::query_as(
        "SELECT project_id, status FROM governance.approval_request WHERE id = $1",
    )
    .bind(approval_id)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "message": "not found" })),
            )
                .into_response();
        }
        Err(err) => return ApiError::Internal(err).into_response(),
    };
    if status != "pending" {
        return conflict("already decided");
    }
    let decision = match body.decision.as_deref() {
        Some("approved") => Decision::Approve,
        Some("rejected") => Decision::Reject,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "message": "invalid decision" })),
            )
                .into_response();
        }
    };
    let reason = match body.reason.as_deref().map(str::trim) {
        Some(reason) if !reason.is_empty() => reason.to_string(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "message": "reason is required" })),
            )
                .into_response();
        }
    };

    // T-021a：操作者 = 真实调用者的 principal（不再按项目 owner 猜测）。
    let actor = principal.audit_actor();
    match pm_application::decide_approval(
        &state.pool,
        &state.executor,
        DecideRequest {
            approval_id,
            actor_id: actor.principal_id,
            decision,
            reason,
        },
    )
    .await
    {
        Ok(_) => {
            if let Err(err) =
                pm_write::record_decision_audit(&state.pool, project_id, &actor, approval_id).await
            {
                return ApiError::from(err).into_response();
            }
            match sqlx::query_as::<_, (String, Option<DateTime<Utc>>)>(
                "SELECT status, decided_at FROM governance.approval_request WHERE id = $1",
            )
            .bind(approval_id)
            .fetch_one(&state.pool)
            .await
            {
                Ok((status, decided_at)) => (
                    StatusCode::OK,
                    Json(json!({
                        "id": approval_id,
                        "status": status,
                        "decided_at": decided_at,
                    })),
                )
                    .into_response(),
                Err(err) => ApiError::Internal(err).into_response(),
            }
        }
        // decide_approval 内部的原子 UPDATE 兜并发（预检与执行之间被决策）。
        Err(SupervisionError::Command(
            pm_application::CommandError::AlreadyDecided { .. },
        )) => conflict("already decided"),
        Err(SupervisionError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "message": "not found" })),
        )
            .into_response(),
        // approve 执行提案命令失败（如目标 revision 已漂移）。
        Err(SupervisionError::Command(err)) => {
            tracing::error!(error = %err, "approval command execution failed");
            conflict("command execution failed")
        }
        Err(err) => {
            tracing::error!(error = %err, "decide failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "message": "internal error" })),
            )
                .into_response()
        }
    }
}

// ---------------------------------------------------- tasks create / update

pub(crate) async fn create_task_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Json(body): Json<CreateTaskRequest>,
) -> ApiResult<Response> {
    let actor = principal.audit_actor();
    let task = pm_write::create_task(&state.pool, body, &actor).await?;
    Ok((StatusCode::CREATED, Json(task)).into_response())
}

pub(crate) async fn update_task_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
    Json(body): Json<UpdateTaskRequest>,
) -> ApiResult<Response> {
    let actor = principal.audit_actor();
    let task = pm_write::update_task(&state.pool, task_id, body, &actor).await?;
    Ok((StatusCode::OK, Json(task)).into_response())
}

// ------------------------------------------------- revisions create / approve

pub(crate) async fn create_revision_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(requirement_id): Path<Uuid>,
    Json(body): Json<CreateRevisionRequest>,
) -> ApiResult<Response> {
    let actor = principal.audit_actor();
    let revision = pm_write::create_revision(&state.pool, requirement_id, body, &actor).await?;
    Ok((StatusCode::CREATED, Json(revision)).into_response())
}

pub(crate) async fn approve_revision_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((requirement_id, revision_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Response> {
    let actor = principal.audit_actor();
    let revision =
        pm_write::approve_revision(&state.pool, requirement_id, revision_id, &actor).await?;
    Ok((StatusCode::OK, Json(revision)).into_response())
}
