//! 审计操作者（T-021a）：业务执行主体 + 认证身份（三元组身份行）。
//!
//! `governance.audit_log` 的 `actor_id` 始终是执行主体；`actor_identity_id` /
//! `actor_client` / `actor_auth_mode` 描述「谁的身份、用什么形态的 token」。
//! 后台/系统动作没有认证身份，三列为 NULL。

use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AuditActor {
    pub principal_id: Uuid,
    pub identity_id: Option<Uuid>,
    pub client: Option<String>,
    pub auth_mode: Option<String>,
}

impl AuditActor {
    /// 没有认证身份的调用者（系统后台、测试种子、off 模式）。
    pub fn principal(principal_id: Uuid) -> Self {
        Self {
            principal_id,
            identity_id: None,
            client: None,
            auth_mode: None,
        }
    }

    /// 带认证身份的真人/Key 调用者（HTTP 请求路径）。
    pub fn identified(
        principal_id: Uuid,
        identity_id: Uuid,
        client: Option<String>,
        auth_mode: Option<String>,
    ) -> Self {
        Self {
            principal_id,
            identity_id: Some(identity_id),
            client,
            auth_mode,
        }
    }

    /// 平台后台动作（worker / 调度）：`ai-pm-dispatch` service principal。
    pub fn dispatch() -> Self {
        Self::principal(crate::run_dispatch::DISPATCH_SERVICE_PRINCIPAL)
    }
}
