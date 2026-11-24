//! Team 成员名册读写 port（P4-ORG Phase 2，原 tenant_members 收口重命名）。
//!
//! 域语义（DR-ORG-02，基线 V0.1 §3）：授权事实源是 `core.team_member.role`
//! （唯一值域 owner|member）；撤权即时生效（每请求查库）。写路径单事务：
//! 锁 core.team 行串行化本 Team 的成员变更 → 锁调用者行验权 → 锁目标行 →
//! 规则判定（HTTP 层纯函数）→ DML + 级联撤权 + 租户审计。
//!
//! 规则判定失败要回 HTTP Response，无法经 port 回调表达——所以事务编排分两
//! 段：回调闭包在锁内取判定输入（两个 role + owner 计数），HTTP 层纯函数
//! 裁决；实现侧落裁决产出的 DML 与审计。锁由实现侧持有（实现内部一个事务
//! 里先取快照、经回调裁决、再落库——见 PgMemberRepo）。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 名册行（list/detail 共用）。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct TeamMemberRow {
    pub principal_id: Uuid,
    pub display_name: String,
    pub role: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// 域级语义错误：NotFound（团队/成员不存在）、规则拒绝（回滚通道）与
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
    /// 本 Team 的 owner 数。
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
/// Err = (错误码, 消息)——HTTP 错误码契约（TEAM_OWNER_REQUIRED / OWNER_LAST 等）
/// 由规则层携带，实现侧只负责回滚。
pub type PlanFn<'a> = &'a (dyn Fn(ChangeSnapshot) -> Result<(Vec<MemberWrite>, Vec<MemberAuditLine>), (String, String)>
         + Send
         + Sync);

#[async_trait]
pub trait MemberRepo: Send + Sync {
    async fn list_members(&self, team_id: Uuid) -> Result<Vec<TeamMemberRow>, MemberRepoError>;

    async fn find_member(
        &self,
        team_id: Uuid,
        principal_id: Uuid,
    ) -> Result<Option<TeamMemberRow>, MemberRepoError>;

    /// 成员变更单事务：锁 core.team 行（同 Team 变更串行化）→ 锁调用者行 →
    /// 锁目标行 → owner 计数 → `plan` 裁决（锁内回调）→ Remove 先做项目
    /// owner 依赖检查（S01-W13）→ DML + 级联撤权（真人项目成员关系）+ 审计行。
    ///
    /// `plan` 返回 Err(消息) 时整体回滚；快照非法（团队/调用者不存在）由
    /// 实现侧在回调前返回 NotFound。
    async fn apply_change(
        &self,
        team_id: Uuid,
        actor: &MemberAuditActor,
        target_id: Uuid,
        plan: PlanFn<'_>,
    ) -> Result<MemberWriteOutcome, MemberRepoError>;
}
