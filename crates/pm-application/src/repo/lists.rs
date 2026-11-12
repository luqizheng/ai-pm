//! `/v1/pm/*` 经典列表页的读模型 port（任务书批 B 首域，自 pm-api
//! `pm_lists.rs` 收口）。
//!
//! 统一契约：`{items,total}` 分页对、`project_id?`/`customer_id?` 过滤、
//! 可见性过滤（调用者在 `core.project_member`）在 SQL 内完成；排序与列
//! 选择是前端契约的一部分，随 SQL 固化在实现侧（协议见 pm_lists 模块头）。

use async_trait::async_trait;
use uuid::Uuid;

use crate::rows::{
    ApprovalRow, AuditLogRow, DecisionRow, MilestoneRow, PrincipalRow, ProjectRow,
    RequirementRevisionRow, RequirementRow, TaskRow,
};

/// 单页读参数（HTTP 层的 Paging 在 pm-api；仓储只认 size/offset）。
#[derive(Debug, Clone, Copy)]
pub struct ListPage {
    pub size: i64,
    pub offset: i64,
}

/// 经典列表页仓储。
#[async_trait]
pub trait PmListRepo: Send + Sync {
    /// 平台人员/代理列表（`ai-pm-dispatch` service principal 不暴露）。
    async fn list_principals(
        &self,
        is_active: Option<bool>,
        page: ListPage,
    ) -> Result<(Vec<PrincipalRow>, i64), sqlx::Error>;

    /// 项目列表（调用者可见；customer 过滤可缺省）。
    async fn list_projects(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        customer_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<ProjectRow>, i64), sqlx::Error>;

    async fn list_requirements(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<RequirementRow>, i64), sqlx::Error>;

    async fn list_tasks(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<TaskRow>, i64), sqlx::Error>;

    async fn list_milestones(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<MilestoneRow>, i64), sqlx::Error>;

    async fn list_approvals(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<ApprovalRow>, i64), sqlx::Error>;

    /// 决策中心读模型（审批请求 + 关联变更提案左连接）。
    async fn list_decisions(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<DecisionRow>, i64), sqlx::Error>;

    async fn list_audit_logs(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<AuditLogRow>, i64), sqlx::Error>;

    /// 修订历史（version DESC；未知 requirement 返回空列表而非 404）。
    async fn list_requirement_revisions(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        requirement_id: Uuid,
    ) -> Result<Vec<RequirementRevisionRow>, sqlx::Error>;

    /// 建项目后的描述回填（bootstrap 产物描述字段缺省，建项请求携带时补写；
    /// 失败由调用方吞掉——与原 handler 语义一致，描述丢失不阻断建项响应）。
    async fn set_project_description(
        &self,
        project_id: Uuid,
        description: &str,
    ) -> Result<(), sqlx::Error>;

    /// 按 id 回读项目行（建项响应体；租户边界）。
    async fn find_project_row(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<ProjectRow, sqlx::Error>;
}
