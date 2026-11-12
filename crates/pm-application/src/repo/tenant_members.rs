//! 租户成员名册读写 port（任务书批 B 第五域，自 pm-api `tenant_members.rs`
//! 收口）。
//!
//! 域语义（T-021b / ADR-0001 D5，见 tenant_members 模块头）：授权事实源是
//! `core.tenant_member.role`；撤权即时生效（每请求查库）。写路径单事务：
//! 锁租户行串行化 → 锁调用者行验权 → 锁目标行 → 规则判定（HTTP 层纯函数）
//! → DML + 级联撤权 + 租户审计。
//!
//! 规则判定失败要回 HTTP Response，无法经 port 回调表达——所以事务编排分两
//! 段：[`MemberRepo::change_snapshot`] 在锁内取判定输入（三个 role + owner
//! 计数 + 项目 owner 依赖），HTTP 层纯函数裁决；[`MemberRepo::apply_change`]
//! 落裁决产出的 DML 与审计。两段之间锁由实现侧持有（实现内部一个事务里先
//! 取快照、经回调裁决、再落库——见 PgMemberRepo）。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 名册行（list/detail 共用）。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct TenantMemberRow {
    pub principal_id: Uuid,
    pub display_name: String,
    pub role: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// 域级语义错误：NotFound（租户/成员不存在）、规则拒绝（回滚通道）与
/// S01-W13 项目 owner 依赖。
#[derive(Debug)]
pub enum MemberRepoError {
    Db(sqlx::Error),
    NotFound(&'static str),
    /// `plan` 闭包裁决拒绝（能力矩阵/最后 owner 保护等）；事务整体回滚，
    /// (错误码, 消息) 由 HTTP 层原样回前端。
    RuleRejected(String, String),
    /// 目标仍是某项目的 owner（先转移项目所有权）。
    ProjectOwnerDependency,
}

impl From<sqlx::Error> for MemberRepoError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

/// 锁内判定快照（change 事务第一步的读出）。
#[derive(Debug, Clone, Copy)]
pub struct ChangeSnapshot {
    /// 调用者当前角色（无行 = 非成员）。
    pub actor_role: Option<&'static str>,
    /// 目标当前角色（无行 = 非成员）。
    pub target_role: Option<&'static str>,
    /// 本租户 owner 数。
    pub owner_count: i64,
}

/// 成员变更 DML（规则校验后传入 [`MemberRepo::apply_change`]）。
#[derive(Debug, Clone, Copy)]
pub enum MemberWrite {
    Add {
        principal_id: Uuid,
        role: &'static str,
        granted_by: Uuid,
    },
    SetRole {
        principal_id: Uuid,
        role: &'static str,
    },
    Remove {
        principal_id: Uuid,
    },
    TransferOwner {
        old_owner: Uuid,
        new_owner: Uuid,
    },
}

/// 写事务执行结果：级联撤销的项目成员关系（Remove 时非空）。
#[derive(Debug, Default)]
pub struct MemberWriteOutcome {
    pub project_memberships_revoked: usize,
    pub revoked_project_ids: Vec<Uuid>,
}

/// 审计行（governance.tenant_audit_log，target_principal_id 形态——与
/// pm_write::insert_tenant_audit 的 target_type 形态不同，独立保留）。
#[derive(Debug, Clone)]
pub struct MemberAuditLine {
    pub action: &'static str,
    pub target: Uuid,
    pub before_role: Option<&'static str>,
    pub after_role: Option<&'static str>,
    /// 合并进 after_data 的附加字段（如级联撤销计数）。
    pub after_extra: Option<serde_json::Value>,
}

/// 审计身份四列（C174）。
#[derive(Debug, Clone)]
pub struct MemberAuditActor {
    pub principal_id: Uuid,
    pub identity_id: Option<Uuid>,
    pub client: Option<String>,
    pub auth_mode: String,
}

/// 锁内快照 → 裁决 的同步回调（规则判定是纯函数，无 await）。
/// Err = (错误码, 消息)——HTTP 错误码契约（OWNER_LAST / USE_OWNER_TRANSFER 等）
/// 由规则层携带，实现侧只负责回滚。
pub type PlanFn<'a> =
    &'a (dyn Fn(ChangeSnapshot) -> Result<(Vec<MemberWrite>, Vec<MemberAuditLine>), (String, String)>
         + Send
         + Sync);

#[async_trait]
pub trait MemberRepo: Send + Sync {
    async fn list_members(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<TenantMemberRow>, MemberRepoError>;

    async fn find_member(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
    ) -> Result<Option<TenantMemberRow>, MemberRepoError>;

    /// 成员变更单事务：锁租户行 → 锁调用者行 → 锁目标行 → owner 计数 →
    /// `plan` 裁决（锁内回调）→ Remove 先做项目 owner 依赖检查（S01-W13）→
    /// DML + 级联撤权（真人项目成员关系）+ 审计行。
    ///
    /// `plan` 返回 Err(消息) 时整体回滚；快照非法（租户/调用者不存在）由
    /// 实现侧在回调前返回 NotFound。
    async fn apply_change(
        &self,
        tenant_id: Uuid,
        actor: &MemberAuditActor,
        target_id: Uuid,
        plan: PlanFn<'_>,
    ) -> Result<MemberWriteOutcome, MemberRepoError>;
}
