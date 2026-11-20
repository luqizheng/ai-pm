//! 租户成员角色（ADR-0001 D5 + T-021b 用户补充要求 §3）。
//!
//! `core.tenant_member.role` 是租户角色的**唯一事实源**：`is_tenant_admin`
//! 每请求查库得出，token 里的 `roles` 只作签发时的提示，不参与授权。
//! 租户角色与项目角色分离——项目工作角色（developer / tester / reviewer /
//! ai_pm）只在 `core.project_member.role` 出现，不得混入本枚举。

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 系统租户：由 `0019_tenant.sql` 幂等种子，`dev-off` 模式与测试夹具共用。
/// 生产路径的租户一律来自调用者的 `AuthedPrincipal.tenant_id`。
pub const SYSTEM_TENANT: Uuid = Uuid::from_u128(0x10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TenantRole {
    Owner,
    Admin,
    Member,
}

impl TenantRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "owner" => Some(Self::Owner),
            "admin" => Some(Self::Admin),
            "member" => Some(Self::Member),
            _ => None,
        }
    }

    /// `owner` / `admin` 可进入租户管理面（成员列表、成员增删改）。
    pub fn is_tenant_admin(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    /// 只有 `owner` 能转让所有权；`admin` 即使能管理成员也不行。
    pub fn can_transfer_ownership(self) -> bool {
        matches!(self, Self::Owner)
    }

    /// 最小权限兜底：库里没有成员行（或角色值无法识别）时按 `Member` 处理，
    /// 使「token 声称 tenant_admin 但库内无行」自然落到 403（验收 RV-10）。
    pub fn min_privilege() -> Self {
        Self::Member
    }
}

impl fmt::Display for TenantRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
