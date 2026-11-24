//! `/v1/pm/review-approvals/*`：D5 审批裁决端点（T-043 / `docs/slice-d/ai-pm-slice-d5-review-v1.md` §5.2）。
//!
//! 审批裁决是**单事务**：行锁 pending 行 → 锚定回读校验 → E2 前置复核 →
//! UPDATE 审批行 → 任务状态迁移 → `task_waiting` 结行 → 审计 + 事件。
//! 任一步失败全回滚（审批行回到 pending、任务状态不变）。
//!
//! 错误映射：
//!
//! | 错误 | HTTP | code |
//! |---|---|---|
//! | `NotFound` | 404 | `approval_not_found` |
//! | `AlreadyDecided` | 409 | `already_decided` |
//! | `AnchorDrift` | 409 | `anchor_drift` |
//! | `GateRefused` | 409 | `completion_gate_refused` |
//! | `ReviewNotSucceeded` | 409 | `review_not_succeeded` |
//! | 其他 | 500 | `internal` |

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

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::acceptance_read_repo;
use crate::support::{require_human_principal, require_project_visible, ApiResult};
use pm_application::repo::acceptance_read::AcceptanceReadRepo;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/review-approvals", get(list_handler))
        .route(
            "/v1/pm/review-approvals/{approval_id}/decision",
            post(decision_handler),
        )
}

#[derive(Debug, Deserialize)]
pub(crate) struct DecisionBody {
    pub project_id: Uuid,
    /// `approved` / `rejected`。
    pub decision: String,
    #[serde(default)]
    pub reason: Option<String>,
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = params
        .get("project_id")
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(crate::support::ApiError::BadRequest(
            "project_id is required",
        ))?;
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let rows = acceptance_read_repo(&state.pool)
        .project_approvals(project_id)
        .await?;
    Ok(Json(json!({ "items": rows, "total": rows.len() })))
}

async fn decision_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(approval_id): Path<Uuid>,
    Json(body): Json<DecisionBody>,
) -> Response {
    // DR-PA-06 / PA06-IMP-001 G-04（2026-11-21）：审批裁决=验收动作，归人类
    // Product Member——显式人类 principal 门 + 项目可见性（原实现把可见性
    // 检查 `.ok()` 吞掉，本批次修复为真守卫）。
    if let Err(err) = require_project_visible(
        &state.pool,
        body.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return err.into_response();
    }
    if let Err(err) = require_human_principal(&state.pool, &principal).await {
        return err.into_response();
    }

    let request = pm_application::review::ReviewDecisionRequest {
        project_id: body.project_id,
        approval_id,
        decision: body.decision,
        decided_by: principal.principal_id,
        decision_reason: body.reason,
    };
    let outbox = pm_application::outbox::PgOutboxAppend;
    match pm_application::review::decide_review_approval(&state.pool, &outbox, &request).await {
        Ok(outcome) => (StatusCode::OK, Json(outcome)).into_response(),
        Err(error) => {
            let (status, code) = match &error {
                pm_application::review::ApprovalError::NotFound => {
                    (StatusCode::NOT_FOUND, "approval_not_found")
                }
                pm_application::review::ApprovalError::AlreadyDecided(_) => {
                    (StatusCode::CONFLICT, "already_decided")
                }
                pm_application::review::ApprovalError::AnchorDrift(_) => {
                    (StatusCode::CONFLICT, "anchor_drift")
                }
                pm_application::review::ApprovalError::GateRefused(_) => {
                    (StatusCode::CONFLICT, "completion_gate_refused")
                }
                pm_application::review::ApprovalError::ReviewNotSucceeded => {
                    (StatusCode::CONFLICT, "review_not_succeeded")
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
            };
            if status == StatusCode::INTERNAL_SERVER_ERROR {
                tracing::error!(error = %error, "review approval decision failed");
            }
            (
                status,
                Json(json!({ "error": code, "message": error.to_string() })),
            )
                .into_response()
        }
    }
}
