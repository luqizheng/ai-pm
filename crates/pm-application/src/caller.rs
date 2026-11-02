//! 调用人身份的三层结构（T-088 / Codex Bootstrap V1.3 §5.1.1、评审 P0-1/P0-4）。
//!
//! 三层各司其职，**生产点唯一、消费点只读**：
//!
//! ```text
//! HTTP Authentication
//!        │
//!        ▼
//!    HumanActor          请求级：经过认证的真人（pm-api 鉴权层生产）
//!        │
//!        ▼
//!    DeviceClaim         请求级：已处理过的设备声明结果（pm-api 设备层生产）
//!        │
//!        ▼
//!   CallerContext        MCP 请求级：human + device（pm-mcp 组装，不访问数据库）
//!        │
//!        ▼
//! require_project_member （用真人 principal_id）
//!        │
//!        ▼
//! resolve_project_executor → AuditActor （项目级执行审计身份）
//! ```
//!
//! **红线**：设备声明仅用于归因，**绝不**写进真人身份列，也**绝不**影响授权结果；
//! **授权主体永远是真人**——`dev_agent` 本身就是 `core.project_member` 的一行，
//! 用它做成员校验恒真。

use uuid::Uuid;

/// 请求级：**经过认证的真人**身份。生产点唯一——pm-api 鉴权层。
#[derive(Debug, Clone)]
pub struct HumanActor {
    /// 真人 principal（授权主体）。
    pub principal_id: Uuid,
    /// 真人所属租户。
    pub tenant_id: Uuid,
    /// `core.principal_identity.id`（`actor_identity_id` 的来源）。
    pub identity_id: Option<Uuid>,
    /// 执行端名（如 `codex` / `trae`），来自 key 的 `client_id` claim。
    pub client: Option<String>,
    /// 认证形态：`api_key` / `interactive` / `unknown`。
    pub auth_mode: String,
}

/// 设备声明被忽略的原因（`DeviceClaim` 的忽略侧）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClaimIgnoredReason {
    /// 未提供该头。
    Absent,
    /// 不是合法 UUID。
    Malformed,
    /// 设备不存在。
    NotFound,
    /// 设备 owner ≠ 当前调用人。
    NotOwner,
    /// 设备已被撤销。
    Revoked,
}

impl DeviceClaimIgnoredReason {
    /// 审计用的稳定动作名（不进 `after_data` 的自由文本）。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Malformed => "malformed",
            Self::NotFound => "not_found",
            Self::NotOwner => "not_owner",
            Self::Revoked => "revoked",
        }
    }
}

/// 请求级：**已经处理过的**设备声明结果（不是待验证的请求）。
///
/// 生产点唯一——pm-api 的 `attach_device_claim` 层；**每请求最多一次**设备查询。
#[derive(Debug, Clone)]
pub struct DeviceClaim {
    /// 已校验的设备 id；声明非法 / 缺席 → `None`。
    pub claimed_device_id: Option<Uuid>,
    /// 忽略原因（供审计留痕）。
    pub ignored_reason: Option<DeviceClaimIgnoredReason>,
}

impl DeviceClaim {
    /// 未提供设备声明。
    pub fn absent() -> Self {
        Self {
            claimed_device_id: None,
            ignored_reason: Some(DeviceClaimIgnoredReason::Absent),
        }
    }

    /// 声明被忽略（附原因）。
    pub fn ignored(reason: DeviceClaimIgnoredReason) -> Self {
        Self {
            claimed_device_id: None,
            ignored_reason: Some(reason),
        }
    }

    /// 声明有效。
    pub fn valid(device_id: Uuid) -> Self {
        Self {
            claimed_device_id: Some(device_id),
            ignored_reason: None,
        }
    }
}

/// MCP 请求级：真人 + 设备声明。由 pm-mcp 的 `caller_from_ctx` 组装——**只读
/// extensions，不访问数据库**。
#[derive(Debug, Clone)]
pub struct CallerContext {
    pub human: HumanActor,
    pub device: DeviceClaim,
}

impl CallerContext {
    pub fn new(human: HumanActor, device: DeviceClaim) -> Self {
        Self { human, device }
    }

    /// 已校验的设备 id（若有）。
    pub fn claimed_device_id(&self) -> Option<Uuid> {
        self.device.claimed_device_id
    }
}
