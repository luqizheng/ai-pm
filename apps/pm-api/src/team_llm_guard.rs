//! 租户级守卫与错误构造（从 `tenant_members.rs` 抽出共享）：
//! `guard_tenant` 跨租户一律 404（不泄露目标租户是否存在），写面统一
//! `err_forbidden("tenant admin required")`——供 tenant_members 与 team_llm
//! 两个管理面复用同一口径。

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;

pub(crate) fn guard_tenant(caller: &AuthedPrincipal, tenant_id: Uuid) -> Result<(), Response> {
    // 跨租户一律 404：不泄露目标租户是否存在。
    if caller.tenant_id != tenant_id {
        return Err(err_not_found("tenant not found"));
    }
    Ok(())
}

pub(crate) fn err_not_found(message: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "message": message })),
    )
        .into_response()
}

pub(crate) fn err_forbidden(message: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({ "message": message })),
    )
        .into_response()
}
