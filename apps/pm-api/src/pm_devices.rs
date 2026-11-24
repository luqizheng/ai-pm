//! `/v1/pm/devices*`：开发环境目录域（T-088）。
//!
//! 边界（Codex Bootstrap V1.3 §5.4.1、评审 P0-1/P0-2）：
//! - Device 是**目录实体**，不持有执行凭据；登记 / 绑定**不产生**项目或仓库权限。
//! - 可见范围沿 `core.tenant_member.role` 唯一事实源：member 见自己的设备并可撤销
//!   自己的；tenant owner / admin 见全租户设备并可撤销任意设备；他人设备一律 404。
//! - `verified_repositories` 只表示服务端确认的仓库身份与项目归属。
//! - Ready 只描述设备自身；`GET /{id}` 的 `bindings[].project_accessible` 按**调用人**
//!   实时派生，不落库。
//!
//! 错误体沿用 `support` 的 `{message}` 约定。身份沿用 `AuthedPrincipal`（T-021a）。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::guard_query_repo;
use pm_application::devenv::{
    bind_project, get_device, list_devices, register_device, report_checks, revoke_device,
    unbind_binding, BindingRow, DeviceChecks, DeviceDetail, DeviceError, DeviceRow,
    RegisterDeviceRequest,
};
use pm_application::repo::GuardQueryRepo;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/devices", get(list_handler).post(register_handler))
        .route("/v1/pm/devices/{device_id}", get(detail_handler))
        .route("/v1/pm/devices/{device_id}/bindings", post(bind_handler))
        .route(
            "/v1/pm/devices/{device_id}/bindings/{binding_id}",
            delete(unbind_handler),
        )
        .route("/v1/pm/devices/{device_id}/checks", post(checks_handler))
        .route("/v1/pm/devices/{device_id}/revoke", post(revoke_handler))
        // V-05-1 Client Registry（0049）：凭据签发/撤销/权限裁剪。
        .route(
            "/v1/pm/devices/{device_id}/credential",
            post(issue_credential_handler).delete(revoke_credential_handler),
        )
        .route(
            "/v1/pm/devices/{device_id}/credential/permissions",
            axum::routing::put(set_permissions_handler),
        )
}

// ───────────────────────────── 请求体 ─────────────────────────────

#[derive(Debug, Deserialize)]
struct RegisterBody {
    /// 客户端持久化的设备 id（`device.json`）；缺省 = 首次登记。
    device_id: Option<Uuid>,
    name: String,
    platform: String,
    os_version: Option<String>,
    executor_kind: Option<String>,
    executor_version: Option<String>,
    /// D-2=c：领取型 Runner 载体声明（缺省 false）。
    #[serde(default)]
    is_runner: bool,
}

#[derive(Debug, Deserialize)]
struct BindBody {
    project_id: Uuid,
    #[serde(default)]
    declared_repository_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
struct ChecksBody {
    #[serde(default)]
    installed: bool,
    #[serde(default)]
    connected: bool,
    #[serde(default)]
    tool: bool,
    #[serde(default)]
    accessible_repositories: Vec<Uuid>,
    evidence: Option<serde_json::Value>,
}

// ───────────────────────────── handlers ─────────────────────────────

async fn register_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Json(body): Json<RegisterBody>,
) -> Response {
    let req = RegisterDeviceRequest {
        device_id: body.device_id,
        name: body.name,
        platform: body.platform,
        os_version: body.os_version,
        executor_kind: body.executor_kind,
        executor_version: body.executor_version,
        is_runner: Some(body.is_runner),
    };
    match register_device(&state.pool, &principal.human_actor(), &req).await {
        Ok(outcome) => {
            let status = if outcome.reused {
                StatusCode::OK
            } else {
                StatusCode::CREATED
            };
            let mut v = device_json(&outcome.device);
            v["reused"] = json!(outcome.reused);
            (status, Json(v)).into_response()
        }
        Err(e) => map_err(e),
    }
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
) -> Response {
    match list_devices(&state.pool, &principal.human_actor()).await {
        Ok(rows) => Json(json!({
            "items": rows.iter().map(device_json).collect::<Vec<_>>(),
            "total": rows.len(),
        }))
        .into_response(),
        Err(e) => map_err(e),
    }
}

async fn detail_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(device_id): Path<Uuid>,
) -> Response {
    match get_device(&state.pool, &principal.human_actor(), device_id).await {
        Ok(detail) => Json(detail_json(&detail)).into_response(),
        Err(e) => map_err(e),
    }
}

async fn bind_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(device_id): Path<Uuid>,
    Json(body): Json<BindBody>,
) -> Response {
    match bind_project(
        &state.pool,
        &principal.human_actor(),
        device_id,
        body.project_id,
        &body.declared_repository_ids,
    )
    .await
    {
        Ok(b) => (StatusCode::OK, Json(binding_json(&b, None))).into_response(),
        Err(e) => map_err(e),
    }
}

async fn unbind_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((device_id, binding_id)): Path<(Uuid, Uuid)>,
) -> Response {
    match unbind_binding(&state.pool, &principal.human_actor(), device_id, binding_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => map_err(e),
    }
}

async fn checks_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(device_id): Path<Uuid>,
    Json(body): Json<ChecksBody>,
) -> Response {
    let checks = DeviceChecks {
        installed: body.installed,
        connected: body.connected,
        tool: body.tool,
        accessible_repositories: body.accessible_repositories,
        evidence: body.evidence,
    };
    match report_checks(&state.pool, &principal.human_actor(), device_id, &checks).await {
        Ok(d) => Json(device_json(&d)).into_response(),
        Err(e) => map_err(e),
    }
}

/// V-05-1 守卫：租户 admin 或 device owner（§5 管理面口径；其余一律 404
/// 不泄露存在性——与 revoke_device 服务内守卫同规格）。
async fn ensure_credential_manager(
    state: &AppState,
    principal: &AuthedPrincipal,
    device_id: Uuid,
) -> Result<(), Response> {
    let owner: Option<Uuid> = guard_query_repo(&state.pool)
        .device_owner(principal.tenant_id, device_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "code": "internal", "message": e.to_string() })),
            )
                .into_response()
        })?;
    let Some(owner) = owner else {
        return Err(not_found_response());
    };
    if !principal.is_admin && owner != principal.principal_id {
        return Err(not_found_response());
    }
    Ok(())
}

fn not_found_response() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "code": "device_not_found", "message": "device not found" })),
    )
        .into_response()
}

/// V-05-1（约束 A）：签发凭据——明文仅本响应返回一次；已有未撤销凭据 → 409。
async fn issue_credential_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(device_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = ensure_credential_manager(&state, &principal, device_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    match pm_application::device_credential::issue_device_credential(
        &state.pool,
        principal.tenant_id,
        device_id,
        &actor,
    )
    .await
    {
        Ok(issued) => (
            StatusCode::CREATED,
            Json(json!({
                "device_id": issued.device_id,
                "key": issued.plaintext,
                "issued_at": issued.issued_at,
                "warning": "store this key now; it is not retrievable later",
            })),
        )
            .into_response(),
        Err(pm_application::device_credential::DeviceCredentialError::ActiveCredentialExists) => (
            StatusCode::CONFLICT,
            Json(json!({ "code": "CREDENTIAL_ACTIVE", "message": "revoke the active credential first" })),
        )
            .into_response(),
        Err(pm_application::device_credential::DeviceCredentialError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "code": "device_not_found", "message": "device not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "code": "internal", "message": e.to_string() })),
        )
            .into_response(),
    }
}

/// V-05-1（约束 C）：撤销——下一请求起全通道 401（每请求直查库）。幂等。
async fn revoke_credential_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(device_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = ensure_credential_manager(&state, &principal, device_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    match pm_application::device_credential::revoke_device_credential(
        &state.pool,
        principal.tenant_id,
        device_id,
        &actor,
    )
    .await
    {
        Ok(revoked) => Json(json!({ "device_id": device_id, "revoked": revoked })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "code": "internal", "message": e.to_string() })),
        )
            .into_response(),
    }
}

/// Q-C：granted_permissions 裁剪（签发后可收紧；值域=已知三码）。
#[derive(serde::Deserialize)]
struct SetPermissionsBody {
    granted_permissions: Vec<String>,
}

async fn set_permissions_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(device_id): Path<Uuid>,
    axum::Json(body): axum::Json<SetPermissionsBody>,
) -> Response {
    if let Err(resp) = ensure_credential_manager(&state, &principal, device_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    match pm_application::device_credential::set_granted_permissions(
        &state.pool,
        principal.tenant_id,
        device_id,
        &body.granted_permissions,
        &actor,
    )
    .await
    {
        Ok(perms) => {
            Json(json!({ "device_id": device_id, "granted_permissions": perms })).into_response()
        }
        Err(pm_application::device_credential::DeviceCredentialError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "code": "device_not_found", "message": "device not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "code": "invalid_permissions", "message": e.to_string() })),
        )
            .into_response(),
    }
}

async fn revoke_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(device_id): Path<Uuid>,
) -> Response {
    match revoke_device(&state.pool, &principal.human_actor(), device_id).await {
        Ok(d) => Json(device_json(&d)).into_response(),
        Err(e) => map_err(e),
    }
}

// ───────────────────────────── 序列化 ─────────────────────────────

fn device_json(d: &DeviceRow) -> serde_json::Value {
    json!({
        "id": d.id,
        "tenant_id": d.tenant_id,
        "owner_principal_id": d.owner_principal_id,
        "name": d.name,
        "platform": d.platform,
        "os_version": d.os_version,
        "executor_kind": d.executor_kind,
        "executor_version": d.executor_version,
        "status": d.status.as_str(),
        "readiness": d.readiness.map(|r| r.as_str()),
        "last_check_at": d.last_check_at,
        "last_check": d.last_check,
        "revoked_at": d.revoked_at,
        "created_at": d.created_at,
        "updated_at": d.updated_at,
        "is_runner": d.is_runner,
        "last_heartbeat_at": d.last_heartbeat_at,
    })
}

fn binding_json(b: &BindingRow, project_accessible: Option<bool>) -> serde_json::Value {
    let mut v = json!({
        "id": b.id,
        "device_id": b.device_id,
        "project_id": b.project_id,
        "declared_repository_ids": b.declared_repository_ids,
        "verified_repositories": b.verified_repositories,
        "bound_by": b.bound_by,
        "bound_at": b.bound_at,
        "unbound_at": b.unbound_at,
    });
    if let Some(a) = project_accessible {
        v["project_accessible"] = json!(a);
    }
    v
}

fn detail_json(detail: &DeviceDetail) -> serde_json::Value {
    let mut v = device_json(&detail.device);
    v["bindings"] = json!(detail
        .bindings
        .iter()
        .map(|bv| binding_json(&bv.binding, Some(bv.project_accessible)))
        .collect::<Vec<_>>());
    v
}

fn map_err(e: DeviceError) -> Response {
    let (status, message) = match e {
        DeviceError::NotFound => (StatusCode::NOT_FOUND, "device not found".to_string()),
        DeviceError::BindingNotFound => (StatusCode::NOT_FOUND, "binding not found".to_string()),
        DeviceError::Validation(m) => (StatusCode::BAD_REQUEST, m),
        DeviceError::Db(m) => {
            tracing::error!(error = %m, "devenv internal error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_string(),
            )
        }
    };
    (status, Json(json!({ "message": message }))).into_response()
}
