//! Project 成员角色（S01-W13，`docs/ai-pm-s01-project-member-authorization-v1.md`）。
//!
//! `core.project_member.role` 一列同时承载两类语义，必须区分：
//!
//! - **真人访问角色**（本模块）：`owner` / `member`——谁能管这个项目。
//! - **Agent 职能定位**（不在本模块）：`ai_pm` / `architect` / `developer` / `tester`
//!   ——任务承接定位，由 `resolve_assignee` 按 role 取 principal，与访问权限无关。
//!
//! 判定入口一律用 [`ProjectMemberRole::parse`]：认不出就是职能行，
//! **不得**当作访问角色使用（`core.project_member` 的既有 5 种取值全部合法）。

/// 真人项目成员角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectMemberRole {
    /// 项目负责人：可管理项目成员、可转移所有权。
    Owner,
    /// 普通成员：可读写项目内业务对象（任务、仓库、交付、知识）。
    Member,
}

impl ProjectMemberRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Member => "member",
        }
    }

    /// 只有 `owner` / `member` 是真人访问角色；其余一律是 Agent 职能值。
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "owner" => Some(Self::Owner),
            "member" => Some(Self::Member),
            _ => None,
        }
    }

    /// 能否管理项目成员（授予 / 撤销 / 转移所有权）。
    pub const fn is_manager(self) -> bool {
        matches!(self, Self::Owner)
    }
}

/// `core.project_member.role` 的全部合法值——**人类两值**（P4-ORG Phase 4 /
/// DR-PA-04③：Agent 职能（ai_pm/architect/developer/tester）已迁出本列，
/// 项目级承载见 `ai.project_agent_member`，租户级能力见
/// `ai.ai_executor_capability`）。
///
/// 与迁移 `0041` 的 `ck_project_member_role` CHECK 保持同一份口径。
pub const PROJECT_MEMBER_ROLES: [&str; 2] = ["owner", "member"];
