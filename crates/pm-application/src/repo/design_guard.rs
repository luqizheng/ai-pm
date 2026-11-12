//! Design 域守卫读原语 port（任务书批 B 域 8，自 pm-api `pm_designs.rs` 收口）。
//!
//! 全部为横切守卫查询：租户边界内按 id 取归属 project_id（不可见按不存在
//! 处理 → None）与归属存在性判定；业务读写走 pm_application::design。
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait DesignGuardRepo: Send + Sync {
    async fn design_project_id(
        &self,
        tenant_id: Uuid,
        design_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error>;

    async fn adr_project_id(
        &self,
        tenant_id: Uuid,
        adr_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error>;

    /// 修订归属存在性（revision 属于 design，且 design 在租户内）。
    async fn revision_exists(
        &self,
        tenant_id: Uuid,
        design_id: Uuid,
        revision_id: Uuid,
    ) -> Result<bool, sqlx::Error>;

    /// 审批单归属 project_id（decide_approval 的锚定查询）。
    async fn approval_project_id(
        &self,
        tenant_id: Uuid,
        approval_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error>;

    /// 实施基线归属 project_id。
    async fn baseline_project_id(
        &self,
        tenant_id: Uuid,
        baseline_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error>;

    /// 项目内全部已批准修订 id（readiness 缺省范围）。
    async fn approved_revision_ids(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<Uuid>, sqlx::Error>;
}
