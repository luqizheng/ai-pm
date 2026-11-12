//! 租户级守卫与错误构造（从 `tenant_members.rs` 抽出共享）：
//! `guard_tenant` 跨租户一律 404（不泄露目标租户是否存在），写面统一
//! `err_forbidden("tenant admin required")`——供 tenant_members 与 team_llm
//! 两个管理面复用同一口径。

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

pub(crate) fn guard_tenant(caller: &AuthedPrincipal, tenant_id: Uuid) -> Result<(), Response> {
    // 跨租户一律 404：不泄露目标租户是否存在。
    if caller.tenant_id != tenant_id {
        return Err(err_not_found("tenant not found"));
    }
    Ok(())
}

/// 读面守卫（设计稿 §6.3 Verify Active Membership，TEAM-QA-B05-02）：
/// 被 remove 的成员 `principal.tenant_id` 锚不变，仅比对锚会被绕过——
/// 读名册等成员面接口必须回库验证「当前仍是 active 成员」。
/// 写面无需此守卫：`apply_member_change` 已从库读 actor 角色并要求 admin。
pub(crate) async fn verify_active_member(
    pool: &sqlx::PgPool,
    caller: &AuthedPrincipal,
    tenant_id: Uuid,
) -> Result<(), Response> {
    guard_tenant(caller, tenant_id)?;
    let active = match platform_ops_repo(pool)
        .active_tenant_membership(tenant_id, caller.principal_id)
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
