//! `/v1/pm/*` 业务写端点（Slice A）：审批决策、需求修订起草/定稿、任务创建/编辑。
//! 校验顺序与错误体对齐 `frontend/src/api/mock/fixtures.ts`（404 → 409 →
//! 400 decision → 400 reason；`{message}` 裸体）；决策复用
//! `supervision::decide_approval`（approve 执行提案命令、reject veto）。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{ApiError, ApiResult};
use pm_application::pm_write::{self, CreateTaskRequest, UpdateTaskRequest};
use pm_application::supervision::{DecideRequest, Decision, SupervisionError};

fn conflict(message: &'static str) -> Response {
    (StatusCode::CONFLICT, Json(json!({ "message": message }))).into_response()
}

// ------------------------------------------------------- tenant pre-checks
//
// 写路径的租户边界在**对外入口**校验：目标行不属于调用者租户时一律 404
// （不泄露存在性）。内部应用层函数继续只看 `project_id`（组合外键已保证同租户）。

async fn project_in_tenant(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
) -> ApiResult<()> {
    let found: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM core.project WHERE id = $1 AND tenant_id = $2")
            .bind(project_id)
            .bind(tenant_id)
            .fetch_optional(pool)
            .await?;
    found
        .map(|_| ())
        .ok_or(ApiError::NotFound("unknown project"))
}

async fn task_in_tenant(pool: &sqlx::PgPool, task_id: Uuid, tenant_id: Uuid) -> ApiResult<()> {
    let found: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM delivery.task WHERE id = $1 AND tenant_id = $2")
            .bind(task_id)
            .bind(tenant_id)
            .fetch_optional(pool)
            .await?;
    found.map(|_| ()).ok_or(ApiError::NotFound("not found"))
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
        "SELECT project_id, status FROM governance.approval_request \
          WHERE id = $1 AND tenant_id = $2",
    )
    .bind(approval_id)
    .bind(principal.tenant_id)
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
            actor: Some(actor.clone()),
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
        Err(SupervisionError::Command(pm_application::CommandError::AlreadyDecided { .. })) => {
            conflict("already decided")
        }
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
    if let Some(project_id) = body.project_id {
        project_in_tenant(&state.pool, project_id, principal.tenant_id).await?;
    }
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
    task_in_tenant(&state.pool, task_id, principal.tenant_id).await?;
    let actor = principal.audit_actor();
    let task = pm_write::update_task(&state.pool, task_id, body, &actor).await?;
    Ok((StatusCode::OK, Json(task)).into_response())
}
