//! 跨域锚定守卫 port（任务书批 B 域 11，自 pm-api 各 handler 的同构
//! 「租户内取归属 project_id / 存在性」单查询收口）。
//!
//! 全部语义：租户边界内按 id 锚定归属（不存在/跨租户 → None，HTTP 层一律
//! 404 不泄露存在性）。业务读写在各自域仓储。
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait GuardQueryRepo: Send + Sync {
    async fn task_project_id(
        &self,
        tenant_id: Uuid,
        task_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error>;

    async fn requirement_project_id(
        &self,
        tenant_id: Uuid,
        requirement_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error>;

    async fn plan_project_id(
        &self,
        tenant_id: Uuid,
        plan_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error>;

    async fn interaction_project_id(
        &self,
        tenant_id: Uuid,
        interaction_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error>;

    async fn run_project_id(
        &self,
        tenant_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error>;

    /// 项目存在性（pm_write 任务创建的归属前置）。
    async fn project_exists(&self, tenant_id: Uuid, project_id: Uuid)
        -> Result<bool, sqlx::Error>;

    /// 任务存在性（pm_write 编辑/决策的归属前置）。
    async fn task_exists(&self, tenant_id: Uuid, task_id: Uuid) -> Result<bool, sqlx::Error>;

    /// 审批单头（decide 校验序第①步：存在性 + 归属 + 状态）。
    async fn approval_head(
        &self,
        tenant_id: Uuid,
        approval_id: Uuid,
    ) -> Result<Option<(Uuid, String)>, sqlx::Error>;

    /// 审批单当前状态与裁决时刻（并发输家的回读）。
    async fn approval_decision_row(
        &self,
        approval_id: Uuid,
    ) -> Result<Option<(String, Option<chrono::DateTime<chrono::Utc>>)>, sqlx::Error>;
}
