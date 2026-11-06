//! `/v1/pm/me` 与 `/v1/pm/principals/{id}`（T-021a）：主体自查 + 管理端激活/停用。
//!
//! `GET /v1/pm/me` 是未激活主体唯一的自查入口（`require_active` 对它放行）；
//! `PATCH` 仅 `is_admin`（临时口径：token `roles` 含 `tenant_admin`，T-021b 改读
//! `core.tenant_member.role`）。停用即停用，不删行 —— 审计仍引用该 principal。

use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch},
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;

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
    // onboarding_required = principal 未激活（provision 后未认领/未受邀）——
    // 首登必须建团或入团（产品裁决 Q1）。memberships 按 principal 查全量
    // （V1 单团队现实下为 0/1 行；契约按多团队 shape 设计，上游支持后免改）。
    let team = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>)>(
        "SELECT id, display_name, slug, description FROM core.tenant WHERE id = $1",
    )
    .bind(principal.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    let memberships = sqlx::query_as::<_, (Uuid, String, String)>(
        // 只统计 ACTIVE 成员关系：未激活 principal 的 provision member 行
        // 不算「已加入」（onboarding_required 语义与之配套）。
        "SELECT t.id, t.display_name, tm.role \
           FROM core.tenant_member tm \
           JOIN core.tenant t ON t.id = tm.tenant_id \
           JOIN core.principal p ON p.id = tm.principal_id \
          WHERE tm.principal_id = $1 AND p.is_active \
          ORDER BY t.created_at",
    )
    .bind(principal.principal_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let team_json = team
        .map(|(id, name, slug, description)| {
            json!({ "id": id, "name": name, "slug": slug, "description": description })
        })
        .unwrap_or(serde_json::Value::Null);
    let memberships_json: Vec<serde_json::Value> = memberships
        .into_iter()
        .map(|(team_id, team_name, role)| {
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
        "onboarding_required": !principal.is_active,
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
    let affected = sqlx::query(
        "UPDATE core.principal SET is_active = $2 \
          WHERE id = $1 AND kind <> 'service' \
            AND EXISTS (SELECT 1 FROM core.tenant_member m \
                         WHERE m.principal_id = core.principal.id AND m.tenant_id = $3)",
    )
    .bind(principal_id)
    .bind(body.is_active)
    .bind(caller.tenant_id)
    .execute(&state.pool)
    .await;
    match affected {
        Ok(result) if result.rows_affected() == 1 => (
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
