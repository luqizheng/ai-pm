//! S02-I6-A · Approver 显式授权 REST 适配层（`core.approver_grant`，迁移 0040）。
//!
//! 授权矩阵（O-6 裁决）：grant/revoke/list = **Lead 专属**（被授人须为在职成员
//! 且非现任 Lead）；confirm/reject/治理命令的消费方闸在
//! `pm-application::approver::ensure_governance_authorized`（requirement 服务内）。
//! 成员闸：列表成员可见；授予/撤销先过 Lead 判定（服务层）+ 项目可见性。

use axum::{
    extract::{Extension, Path, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, ApiResult};
use pm_application::approver;

#[derive(Deserialize)]
pub struct GrantApproverBody {
    pub principal_id: Uuid,
}

async fn list_approvers(
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
    let items = approver::list_approvers(&state.pool, project_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn grant_approver(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<GrantApproverBody>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let view = approver::grant_approver(
        &state.pool,
        caller.tenant_id,
        project_id,
        body.principal_id,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "grant": view })),
    ))
}

async fn revoke_approver(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((project_id, principal_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    approver::revoke_approver(&state.pool, project_id, principal_id, &caller.audit_actor()).await?;
    Ok(Json(serde_json::json!({
        "principal_id": principal_id,
        "revoked": true
    })))
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/approvers",
            get(list_approvers).post(grant_approver),
        )
        .route(
            "/v1/pm/projects/{project_id}/approvers/{principal_id}",
            axum::routing::delete(revoke_approver),
        )
}
