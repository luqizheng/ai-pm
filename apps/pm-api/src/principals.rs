//! `/v1/pm/me` 与 `/v1/pm/principals/{id}`（T-021a）：主体自查 + 管理端激活/停用。
//!
//! `GET /v1/pm/me` 是未激活主体唯一的自查入口（`require_active` 对它放行）；
//! `PATCH` 仅 `is_admin`（临时口径：token `roles` 含 `tenant_admin`，T-021b 改读
//! `core.tenant_member.role`）。停用即停用，不删行 —— 审计仍引用该 principal。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::platform_ops_repo;
use pm_application::repo::platform_ops::PlatformOpsRepo;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/me", get(me_handler))
        .route("/v1/pm/principals/{principal_id}", patch(update_handler))
}

async fn me_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
) -> Json<serde_json::Value> {
    // Team Onboarding（0044）扩展面：团队档案 + onboarding 门 + memberships。
    // onboarding_required = principal 未激活（provision 后未认领/未受邀）
    // **或无 active 成员关系**（被 remove / 团队休眠，TEAM-QA-B05-03）——
    // 两种态都进 onboarding 流（产品裁决 Q1：必须建团或入团）。
    // 注意不能以 principal.tenant_id 锚解析 tenant 行判定 has_team：remove
    // 不改锚，锚解析恒真会吞掉「已移除」语义；team 档案同理从 membership
    // 派生（被移除者 team=null，与 memberships=[] 自洽）。memberships 按
    // principal 查全量（V1 单团队现实下为 0/1 行；契约按多团队 shape 设计，
    // 上游支持后免改）。
    let memberships = platform_ops_repo(&state.pool)
        .principal_memberships(principal.principal_id)
        .await
        .unwrap_or_default();

    let has_team = !memberships.is_empty();
    let team_json = memberships
        .first()
        .map(|(id, name, _, slug, description)| {
            json!({ "id": id, "name": name, "slug": slug, "description": description })
        })
        .unwrap_or(serde_json::Value::Null);
    let memberships_json: Vec<serde_json::Value> = memberships
        .into_iter()
        .map(|(team_id, team_name, role, _, _)| {
            json!({ "team_id": team_id, "team_name": team_name, "role": role, "status": "active" })
        })
        .collect();

    Json(json!({
        "principal_id": principal.principal_id,
        "display_name": principal.display_name,
        "is_active": principal.is_active,
        "is_admin": principal.is_admin,
        "tenant_id": principal.tenant_id,
        "tenant_role": principal.tenant_role.as_str(),
        "provider": principal.provider,
        "claims_tenant_key": principal.claims_tenant_key,
        "subject": principal.subject,
        "auth_mode": principal.auth_mode,
        "team": team_json,
        "onboarding_required": !principal.is_active || !has_team,
        "memberships": memberships_json,
    }))
}

#[derive(Debug, Deserialize)]
struct UpdatePrincipalBody {
    is_active: bool,
}

async fn update_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(principal_id): Path<Uuid>,
    Json(body): Json<UpdatePrincipalBody>,
) -> Response {
    if !caller.is_admin {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "tenant admin required" })),
        )
            .into_response();
    }
    // 平台 service principal（`ai-pm-dispatch`）不参与人员激活开关；
    // 且只能操作**本租户**的成员（跨租户 id 一律按不存在处理）。
    let affected = platform_ops_repo(&state.pool)
        .set_principal_active(principal_id, body.is_active, caller.tenant_id)
        .await;
    match affected {
        Ok(1) => (
            StatusCode::OK,
            Json(json!({ "id": principal_id, "is_active": body.is_active })),
        )
            .into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "message": "principal not found" })),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "principal update failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "message": "internal error" })),
            )
                .into_response()
        }
    }
}
