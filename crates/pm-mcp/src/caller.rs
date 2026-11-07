//! MCP 工具层的调用人上下文（T-088）。
//!
//! 职责边界（设计稿 §4.3~§4.5、冻结约束）：
//!
//! - [`caller_from_ctx`] **只读** rmcp `RequestContext.extensions` 里由 pm-api 鉴权层
//!   注入的 `HumanActor` 与 `DeviceClaim`，组装 [`CallerContext`]——**不访问数据库、
//!   不重新判断设备 owner / status**（设备声明的唯一校验点在 pm-api `attach_device_claim`）。
//! - [`authorize`] 用**真人** principal 做项目成员校验（`dev_agent` 本身就是
//!   `project_member` 一行，用它校验恒真），再解析当前 Project 的执行主体
//!   （dev_agent / ai_pm，**维持各工具现状口径**），构造项目级 [`AuditActor`]。
//!   找不到执行主体即报错，**不回退真人**。
//! - 非成员 → `ToolError::NotFound`（与 T-022「不可见即不存在」一致，防 ID 探测）。

use pm_application::actor::AuditActor;
use pm_application::authz::{AuthzError, ExecutorKind, require_project_member, resolve_project_executor};
use pm_application::caller::{CallerContext, ClientChannel, DeviceClaim, HumanActor};
use rmcp::service::RequestContext;
use rmcp::RoleServer;
use sqlx::PgPool;
use uuid::Uuid;

use crate::tools::ToolError;

/// 从 rmcp 请求上下文读取调用人（**无 IO**）。
///
/// 读取链：`ctx.extensions` → `http::request::Parts` → `parts.extensions`
/// → `HumanActor` / `DeviceClaim`。rmcp 3.8 在 `tower.rs` 把请求 parts 注入消息
/// extensions，再于 `service.rs:1579` 换进 `RequestContext.extensions`——已 spike 实证。
pub fn caller_from_ctx(ctx: &RequestContext<RoleServer>) -> Result<CallerContext, ToolError> {
    let parts = ctx
        .extensions
        .get::<http::request::Parts>()
        .ok_or_else(|| ToolError::Forbidden("caller identity missing (no http parts)".into()))?;
    let human = parts
        .extensions
        .get::<HumanActor>()
        .cloned()
        .ok_or_else(|| ToolError::Forbidden("caller identity missing".into()))?;
    // 设备声明层缺席（理论上不可达）→ 视为未声明，不影响授权。
    let device = parts
        .extensions
        .get::<DeviceClaim>()
        .cloned()
        .unwrap_or_else(DeviceClaim::absent);
    // V-05-1 凭据通道（cpk_）：鉴权层解析成功时注入——claim 据此写稳定
    // Client 身份（lease_client_id / lease_owner=client:{id}）。
    let client_channel = parts.extensions.get::<ClientChannel>().copied();
    let ctx = CallerContext::new(human, device);
    Ok(match client_channel {
        Some(channel) => ctx.with_client_channel(channel),
        None => ctx,
    })
}

/// 仅做真人成员校验（不取执行主体）——供只读与 discovery 类工具使用。
pub async fn ensure_member(
    pool: &PgPool,
    caller: &CallerContext,
    project_id: Uuid,
) -> Result<(), ToolError> {
    require_project_member(
        pool,
        caller.human.principal_id,
        caller.human.tenant_id,
        project_id,
    )
    .await
    .map_err(|e| match e {
        AuthzError::NotMember => ToolError::NotFound(format!(
            "project {project_id} not found (or not visible)"
        )),
        AuthzError::ExecutorMissing => unreachable!("member check never yields ExecutorMissing"),
        AuthzError::Db(m) => ToolError::InvalidRequest(format!("authorization failed: {m}")),
    })
}

/// 成员校验 + 执行主体解析 → 项目级审计身份。
///
/// 顺序固定：先用**真人**校验成员资格（非成员 404），再解析执行主体。
pub async fn authorize(
    pool: &PgPool,
    caller: &CallerContext,
    project_id: Uuid,
    kind: ExecutorKind,
) -> Result<AuditActor, ToolError> {
    require_project_member(
        pool,
        caller.human.principal_id,
        caller.human.tenant_id,
        project_id,
    )
    .await
    .map_err(|e| match e {
        AuthzError::NotMember => ToolError::NotFound(format!(
            "project {project_id} not found (or not visible)"
        )),
        AuthzError::ExecutorMissing => unreachable!("member check never yields ExecutorMissing"),
        AuthzError::Db(m) => ToolError::InvalidRequest(format!("authorization failed: {m}")),
    })?;

    let executor = resolve_project_executor(pool, project_id, kind)
        .await
        .map_err(|e| match e {
            AuthzError::ExecutorMissing => ToolError::InvalidRequest(format!(
                "project {project_id} has no {} principal",
                kind.principal_kind()
            )),
            AuthzError::NotMember => unreachable!("executor lookup never yields NotMember"),
            AuthzError::Db(m) => ToolError::InvalidRequest(format!("authorization failed: {m}")),
        })?;

    Ok(AuditActor {
        principal_id: executor,
        identity_id: caller.human.identity_id,
        client: caller.human.client.clone(),
        auth_mode: Some(caller.human.auth_mode.clone()),
        claimed_device_id: caller.device.claimed_device_id,
    })
}
