//! `/mcp` 设备声明层（T-088）：`X-AI-PM-Device-Id` 的**唯一校验点**。
//!
//! 设计稿 §4.3 / §6.2 / 评审 P0-3：
//! - **仅 `tools/call` 请求**做设备查询与忽略审计（M3）——`initialize` /
//!   `tools/list` / notification 不查库、不写归因审计（C184）；
//! - 每个符合处理条件的请求**最多一次**设备查询（C179 的服务端半边）；
//! - 校验顺序：存在性 → owner → status；任一不过 → `DeviceClaim::ignored(..)`，
//!   **不拒绝请求**——设备声明仅影响归因，绝不影响 Principal 的授权结果；
//! - 忽略事件写租户级 `governance.tenant_audit_log`（action =
//!   `device_attribution_ignored`）；归因审计写失败**不升级为拒绝**（M4），只记日志。

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use pm_application::caller::{DeviceClaim, DeviceClaimIgnoredReason};

/// 设备声明头（客户端自报，不可信）。
pub const DEVICE_HEADER: &str = "x-ai-pm-device-id";

/// 只对 `tools/call` 做设备校验（M3）。
fn is_tool_call(body: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.get("method")
                .and_then(|m| m.as_str())
                .map(|m| m == "tools/call")
        })
        .unwrap_or(false)
}

/// 设备声明校验 + 注入 `Extension<DeviceClaim>`。
pub(crate) async fn attach_device_claim(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    // 非工具请求：不查库、不审计，注入 Absent 占位（pm-mcp 侧归因自然为 NULL）。
    let (parts, body) = request.into_parts();
    // V-05-1：cpk 凭据通道已在鉴权层注入 DeviceClaim::valid（凭据即声明，
    // 约束 A）——跳过 X-header 校验路径，原样放行。
    if parts.extensions.get::<pm_application::caller::ClientChannel>().is_some() {
        let request = axum::http::Request::from_parts(parts, body);
        return next.run(request).await;
    }
    let bytes = match axum::body::to_bytes(body, 256 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return (
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                Json(json!({
                    "code": "IAM_REQUEST_TOO_LARGE",
                    "message": "mcp request body exceeds the 256 KiB limit"
                })),
            )
                .into_response();
        }
    };
    if !is_tool_call(&bytes) {
        let mut request = axum::http::Request::from_parts(parts, axum::body::Body::from(bytes));
        request.extensions_mut().insert(DeviceClaim::absent());
        return next.run(request).await;
    }

    let Some(caller) = parts.extensions.get::<AuthedPrincipal>().cloned() else {
        // 鉴权链在位时不可达；fail-closed 为占位声明（不影响鉴权层已有的拒绝）。
        let mut request = axum::http::Request::from_parts(parts, axum::body::Body::from(bytes));
        request.extensions_mut().insert(DeviceClaim::absent());
        return next.run(request).await;
    };

    let header: Option<String> = parts
        .headers
        .get(DEVICE_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let (claim, ignore_reason) = match header.as_deref() {
        None => (DeviceClaim::absent(), None),
        Some(raw) => {
            let reason = match raw.trim() {
                "" => Some(DeviceClaimIgnoredReason::Absent),
                s => match Uuid::parse_str(s) {
                    Err(_) => Some(DeviceClaimIgnoredReason::Malformed),
                    Ok(device_id) => match device_status(&state, device_id).await {
                        None => Some(DeviceClaimIgnoredReason::NotFound),
                        Some((tenant_id, owner, _)) if tenant_id != caller.tenant_id || owner != caller.principal_id => {
                            Some(DeviceClaimIgnoredReason::NotOwner)
                        }
                        Some((_, _, status)) if status == "revoked" => {
                            Some(DeviceClaimIgnoredReason::Revoked)
                        }
                        Some(_) => None, // 有效声明
                    },
                },
            };
            match reason {
                None => {
                    let id = Uuid::parse_str(raw.trim()).expect("validated above");
                    (DeviceClaim::valid(id), None)
                }
                Some(r) => (DeviceClaim::ignored(r), Some(r)),
            }
        }
    };

    // 忽略留痕（仅 tools/call；写失败不升级为拒绝——M4）。
    if let Some(reason) = ignore_reason {
        if let Err(e) = record_ignored(&state, &caller, reason).await {
            tracing::error!(error = %e, "device attribution ignored-audit write failed");
        }
    }

    let mut request = axum::http::Request::from_parts(parts, axum::body::Body::from(bytes));
    request.extensions_mut().insert(claim);
    next.run(request).await
}

/// 一次设备查询：返回 `(tenant_id, owner, status)`；不存在 → None。
async fn device_status(
    state: &AppState,
    device_id: Uuid,
) -> Option<(Uuid, Uuid, String)> {
    sqlx::query_as(
        "SELECT tenant_id, owner_principal_id, status FROM devenv.device WHERE id = $1",
    )
    .bind(device_id)
    .fetch_optional(&state.pool)
    .await
    .ok()?
}

/// 「设备声明被忽略」事件（租户级审计；`claimed_device_id` 为 NULL）。
async fn record_ignored(
    state: &AppState,
    caller: &AuthedPrincipal,
    reason: DeviceClaimIgnoredReason,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.tenant_audit_log \
         (tenant_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_principal_id, before_data, after_data) \
         VALUES ($1, $2, $3, $4, $5, NULL, 'device_attribution_ignored', $2, NULL, $6)",
    )
    .bind(caller.tenant_id)
    .bind(caller.principal_id)
    .bind(caller.identity_id)
    .bind(&caller.client)
    .bind(&caller.auth_mode)
    .bind(json!({ "reason": reason.as_str() }))
    .execute(&state.pool)
    .await
    .map(|_| ())
}
