//! 守卫与错误构造（从 `tenant_members.rs` 抽出共享）：跨 Team 一律 404
//! （不泄露目标是否存在），写面统一 `err_forbidden(...)`——供 tenant_members
//! 与 team_llm 两个管理面复用同一口径。

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::support::repos::platform_ops_repo;
use pm_application::repo::platform_ops::PlatformOpsRepo;

/// Team 面守卫：跨 Team 一律 404（不泄露目标 Team 是否存在）。Team 隶属
/// 租户——caller 租户与 Team 租户不一致即按不存在处理（回库比对，不信任锚）。
pub(crate) async fn guard_team(
    caller: &AuthedPrincipal,
    pool: &sqlx::PgPool,
    team_id: Uuid,
) -> Result<(), Response> {
    let tenant_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT tenant_id FROM core.team WHERE id = $1",
    )
    .bind(team_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "team tenant lookup failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "message": "internal error" })),
        )
            .into_response()
    })?;
    match tenant_id {
        Some(t) if t == caller.tenant_id => Ok(()),
        _ => Err(err_not_found("team not found")),
    }
}

/// 读面守卫（设计稿 §6.3 Verify Active Membership，TEAM-QA-B05-02）：
/// 被 remove 的成员 `principal.tenant_id` 锚不变，仅比对锚会被绕过——
/// 读名册等成员面接口必须回库验证「当前仍是 active 成员」。
/// 写面无需此守卫：`apply_change` 已从库读 actor 角色并要求 Owner。
pub(crate) async fn verify_active_team_member(
    pool: &sqlx::PgPool,
    caller: &AuthedPrincipal,
    team_id: Uuid,
) -> Result<(), Response> {
    guard_team(caller, pool, team_id).await?;
    let active = match platform_ops_repo(pool)
        .active_team_membership(team_id, caller.principal_id)
        .await
    {
        Ok(active) => active,
        Err(e) => {
            tracing::error!(error = %e, "active membership check failed");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "message": "internal error" })),
            )
                .into_response());
        }
    };
    if !active {
        return Err(err_forbidden("not an active member"));
    }
    Ok(())
}

pub(crate) fn err_not_found(message: &str) -> Response {
    (StatusCode::NOT_FOUND, Json(json!({ "message": message }))).into_response()
}

pub(crate) fn err_forbidden(message: &str) -> Response {
    (StatusCode::FORBIDDEN, Json(json!({ "message": message }))).into_response()
}
