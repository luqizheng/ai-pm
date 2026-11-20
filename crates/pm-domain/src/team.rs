//! Team 成员角色（P4-ORG 基线 V0.1 §3；DR-ORG-02，2026-11-19）。
//!
//! `core.team_member.role` 是 Team 角色的**唯一事实源**：`is_admin`（管理面
//! 通行证）每请求查库得出，token 里的 `roles` 只作签发时的提示，不参与授权。
//! Team 角色只有 Owner / Member 两值（多 Owner 并存；最后 Owner 不可退出/
//! 降级/移除）。Tenant Admin 不参与 Team 面（D-1）；项目工作角色（developer /
//! tester / ai_pm 等 Agent 职能）只在 Agent Registry / core.project_member
//! 历史列出现，不得混入本枚举。

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 系统租户：由 `0019_tenant.sql` 幂等种子，`dev-off` 模式与测试夹具共用。
/// 生产路径的租户一律来自调用者的 `AuthedPrincipal.tenant_id`。
pub const SYSTEM_TENANT: Uuid = Uuid::from_u128(0x10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TeamRole {
    Owner,
    Member,
}

impl TeamRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Member => "member",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "owner" => Some(Self::Owner),
            "member" => Some(Self::Member),
            _ => None,
        }
    }

    /// `owner` 可进入 Team 管理面（成员增删改、邀请管理、团队档案）；
    /// Member 只有名册读权限。admin 角色已随 P4-ORG DR-ORG-02 取消。
    pub fn is_owner(self) -> bool {
        matches!(self, Self::Owner)
    }

    /// 最小权限兜底：库里没有成员行（或角色值无法识别）时按 `Member` 处理，
    /// 使「token 声称管理角色但库内无行」自然落到 403（验收 RV-10）。
    pub fn min_privilege() -> Self {
        Self::Member
    }
}

impl fmt::Display for TeamRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
