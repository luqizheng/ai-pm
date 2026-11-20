//! [`PmListRepo`] 的 Postgres 实现（任务书批 B 首域：pm-api `pm_lists.rs`
//! 的 SQL 收口）。SQL 与列选择原样迁移，行为不变。

use async_trait::async_trait;
use uuid::Uuid;

use pm_application::repo::{ListPage, PmListRepo};
use pm_application::rows::{
    ApprovalRow, AuditLogRow, DecisionRow, MilestoneRow, PrincipalRow, ProjectRow,
    RequirementRevisionRow, RequirementRow, TaskRow,
};

/// 无状态连接句柄（`PgPool` 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub struct PgPmListRepo {
    pool: sqlx::PgPool,
}

impl PgPmListRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PmListRepo for PgPmListRepo {
    async fn list_principals(
        &self,
        is_active: Option<bool>,
        page: ListPage,
    ) -> Result<(Vec<PrincipalRow>, i64), sqlx::Error> {
        // 平台专用 service principal（`ai-pm-dispatch`，0015 幂等种子）是审计兜底
        // 身份，不属于可指派的人员/代理列表，故不对外暴露。
        let items: Vec<PrincipalRow> = sqlx::query_as(
            "SELECT id, kind, display_name, is_active, created_at FROM core.principal \
              WHERE kind <> 'service' AND ($3::bool IS NULL OR is_active = $3) \
              ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(page.size)
        .bind(page.offset)
        .bind(is_active)
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM core.principal \
              WHERE kind <> 'service' AND ($1::bool IS NULL OR is_active = $1)",
        )
        .bind(is_active)
        .fetch_one(&self.pool)
        .await?;
        Ok((items, total))
    }

    async fn list_projects(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        customer_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<ProjectRow>, i64), sqlx::Error> {
        // customer_id 是 NOT NULL 外键，INNER JOIN 不会丢行；COALESCE 只为让行结构
        // 保持 String（前端不需要区分「有客户但没名字」）。可见性（T-022）：只列
        // 调用者在 `core.project_member` 里的项目。
        let items: Vec<ProjectRow> = sqlx::query_as(
            "SELECT p.id, p.customer_id, COALESCE(c.name, '') AS customer_name, \
                    COALESCE(c.status, 'active') AS customer_status, \
                    p.code, p.name, p.description, p.status, p.revision, p.created_at, p.updated_at \
             FROM core.project p JOIN core.customer c ON c.id = p.customer_id \
             WHERE p.tenant_id = $5 \
               AND EXISTS (SELECT 1 FROM core.project_member m \
                            WHERE m.project_id = p.id AND m.principal_id = $4) \
               AND ($1::uuid IS NULL OR p.customer_id = $1) \
             ORDER BY p.created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(customer_id)
        .bind(page.size)
        .bind(page.offset)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM core.project p \
              WHERE p.tenant_id = $3 \
                AND EXISTS (SELECT 1 FROM core.project_member m \
                             WHERE m.project_id = p.id AND m.principal_id = $2) \
                AND ($1::uuid IS NULL OR p.customer_id = $1)",
        )
        .bind(customer_id)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok((items, total))
    }

    async fn list_requirements(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<RequirementRow>, i64), sqlx::Error> {
        let items: Vec<RequirementRow> = sqlx::query_as(
            "SELECT r.id, r.project_id, r.code, r.title, r.lifecycle_status, r.current_revision_id, \
                    r.row_revision, r.created_at \
             FROM delivery.requirement r \
             WHERE r.tenant_id = $5 \
               AND EXISTS (SELECT 1 FROM core.project_member m \
                            WHERE m.project_id = r.project_id AND m.principal_id = $4) \
               AND ($1::uuid IS NULL OR r.project_id = $1) \
             ORDER BY r.created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(project_id)
        .bind(page.size)
        .bind(page.offset)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM delivery.requirement r \
              WHERE r.tenant_id = $3 \
                AND EXISTS (SELECT 1 FROM core.project_member m \
                             WHERE m.project_id = r.project_id AND m.principal_id = $2) \
                AND ($1::uuid IS NULL OR r.project_id = $1)",
        )
        .bind(project_id)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok((items, total))
    }

    async fn list_tasks(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<TaskRow>, i64), sqlx::Error> {
        let items: Vec<TaskRow> = sqlx::query_as(
            "SELECT t.id, t.project_id, t.milestone_id, t.title, t.description, t.status, \
                    t.priority, t.assignee_id, t.revision, t.ai_role_version_id, \
                    t.created_at, t.updated_at \
             FROM delivery.task t \
             WHERE t.tenant_id = $5 \
               AND EXISTS (SELECT 1 FROM core.project_member m \
                            WHERE m.project_id = t.project_id AND m.principal_id = $4) \
               AND ($1::uuid IS NULL OR t.project_id = $1) \
             ORDER BY t.created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(project_id)
        .bind(page.size)
        .bind(page.offset)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM delivery.task t \
              WHERE t.tenant_id = $3 \
                AND EXISTS (SELECT 1 FROM core.project_member m \
                             WHERE m.project_id = t.project_id AND m.principal_id = $2) \
                AND ($1::uuid IS NULL OR t.project_id = $1)",
        )
        .bind(project_id)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok((items, total))
    }

    async fn list_milestones(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<MilestoneRow>, i64), sqlx::Error> {
        let items: Vec<MilestoneRow> = sqlx::query_as(
            "SELECT m.id, m.project_id, m.name, m.planned_at, m.status, m.revision \
             FROM delivery.milestone m \
             WHERE m.tenant_id = $5 \
               AND EXISTS (SELECT 1 FROM core.project_member pm \
                            WHERE pm.project_id = m.project_id AND pm.principal_id = $4) \
               AND ($1::uuid IS NULL OR m.project_id = $1) \
             ORDER BY m.planned_at NULLS LAST, m.name ASC LIMIT $2 OFFSET $3",
        )
        .bind(project_id)
        .bind(page.size)
        .bind(page.offset)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM delivery.milestone m \
              WHERE m.tenant_id = $3 \
                AND EXISTS (SELECT 1 FROM core.project_member pm \
                             WHERE pm.project_id = m.project_id AND pm.principal_id = $2) \
                AND ($1::uuid IS NULL OR m.project_id = $1)",
        )
        .bind(project_id)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok((items, total))
    }

    async fn list_approvals(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<ApprovalRow>, i64), sqlx::Error> {
        let items: Vec<ApprovalRow> = sqlx::query_as(
            "SELECT a.id, a.project_id, a.proposal_id, a.status, a.required_role, a.decided_by, \
                    a.decision_reason, a.decided_at, a.created_at \
             FROM governance.approval_request a \
             WHERE a.tenant_id = $5 \
               AND EXISTS (SELECT 1 FROM core.project_member m \
                            WHERE m.project_id = a.project_id AND m.principal_id = $4) \
               AND ($1::uuid IS NULL OR a.project_id = $1) \
             ORDER BY a.created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(project_id)
        .bind(page.size)
        .bind(page.offset)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM governance.approval_request a \
              WHERE a.tenant_id = $3 \
                AND EXISTS (SELECT 1 FROM core.project_member m \
                             WHERE m.project_id = a.project_id AND m.principal_id = $2) \
                AND ($1::uuid IS NULL OR a.project_id = $1)",
        )
        .bind(project_id)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok((items, total))
    }

    async fn list_decisions(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<DecisionRow>, i64), sqlx::Error> {
        let items: Vec<DecisionRow> = sqlx::query_as(
            "SELECT a.id, a.project_id, a.status, a.required_role, a.decided_by, \
                    a.decision_reason, a.decided_at, a.created_at, \
                    p.id AS proposal_id, p.status AS proposal_status, p.target_type, \
                    p.command_type, p.rationale, p.risk_level, p.evidence, \
                    p.proposed_data, p.analysis_run_id \
             FROM governance.approval_request a \
             LEFT JOIN ai.change_proposal p ON p.id = a.proposal_id \
             WHERE a.tenant_id = $5 \
               AND EXISTS (SELECT 1 FROM core.project_member m \
                            WHERE m.project_id = a.project_id AND m.principal_id = $4) \
               AND ($1::uuid IS NULL OR a.project_id = $1) \
             ORDER BY a.created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(project_id)
        .bind(page.size)
        .bind(page.offset)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM governance.approval_request a \
             WHERE a.tenant_id = $3 \
               AND EXISTS (SELECT 1 FROM core.project_member m \
                            WHERE m.project_id = a.project_id AND m.principal_id = $2) \
               AND ($1::uuid IS NULL OR a.project_id = $1)",
        )
        .bind(project_id)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok((items, total))
    }

    async fn list_audit_logs(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: ListPage,
    ) -> Result<(Vec<AuditLogRow>, i64), sqlx::Error> {
        let items: Vec<AuditLogRow> = sqlx::query_as(
            "SELECT g.id, g.project_id, g.actor_id, g.action, g.target_type, g.target_id, g.created_at \
             FROM governance.audit_log g \
             WHERE g.tenant_id = $5 \
               AND EXISTS (SELECT 1 FROM core.project_member m \
                            WHERE m.project_id = g.project_id AND m.principal_id = $4) \
               AND ($1::uuid IS NULL OR g.project_id = $1) \
             ORDER BY g.created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(project_id)
        .bind(page.size)
        .bind(page.offset)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM governance.audit_log g \
              WHERE g.tenant_id = $3 \
                AND EXISTS (SELECT 1 FROM core.project_member m \
                             WHERE m.project_id = g.project_id AND m.principal_id = $2) \
                AND ($1::uuid IS NULL OR g.project_id = $1)",
        )
        .bind(project_id)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok((items, total))
    }

    async fn list_requirement_revisions(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        requirement_id: Uuid,
    ) -> Result<Vec<RequirementRevisionRow>, sqlx::Error> {
        let items: Vec<RequirementRevisionRow> = sqlx::query_as(
            "SELECT rv.id, rv.requirement_id, rv.project_id, rv.version, rv.content, rv.state, \
                    rv.content_hash, rv.created_by, rv.approved_by, rv.created_at \
             FROM delivery.requirement_revision rv \
             WHERE rv.requirement_id = $1 AND rv.tenant_id = $2 \
               AND EXISTS (SELECT 1 FROM core.project_member m \
                            WHERE m.project_id = rv.project_id AND m.principal_id = $3) \
             ORDER BY rv.version DESC",
        )
        .bind(requirement_id)
        .bind(tenant_id)
        .bind(principal_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(items)
    }

    async fn set_project_description(
        &self,
        project_id: Uuid,
        description: &str,
    ) -> Result<(), sqlx::Error> {
        // 调用方语义：失败吞掉（描述回填不阻断建项响应）；行归属刚由同一请求
        // bootstrap 产生，故按 id 定位即可。
        sqlx::query("UPDATE core.project SET description = $2 WHERE id = $1")
            .bind(project_id)
            .bind(description)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn find_project_row(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<ProjectRow, sqlx::Error> {
        sqlx::query_as::<_, ProjectRow>(
            "SELECT p.id, p.customer_id, COALESCE(c.name, '') AS customer_name, \
                    COALESCE(c.status, 'active') AS customer_status, \
                    p.code, p.name, p.description, p.status, p.revision, p.created_at, p.updated_at \
             FROM core.project p JOIN core.customer c ON c.id = p.customer_id \
             WHERE p.id = $1 AND p.tenant_id = $2",
        )
        .bind(project_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
    }
}
