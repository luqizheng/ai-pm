//! 平台杂项读写 port（任务书 B 收尾批：admin/docproc/守卫杂项/审计 INSERT
//! 自 pm-api 各文件收口）。
//!
//! - 管理面：principal 激活开关与 memberships、embedding 平台配置；
//! - docproc 守卫：版本→文档链锚定（tenant 边界）；
//! - 杂项守卫：intake 归属、设备状态、活跃租户判定；
//! - 平台审计：intake 重试 / outbox 死信重放（governance.audit_log）与设备
//!   声明忽略（governance.tenant_audit_log）——四身份列全写（C174 守卫清单
//!   随迁移更新）。
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[async_trait]
pub trait PlatformOpsRepo: Send + Sync {
    // ── 管理面 ──
    /// principal 的 ACTIVE 租户成员关系（V1 单团队 0/1 行；契约多团队 shape）。
    async fn principal_memberships(
        &self,
        principal_id: Uuid,
    ) -> Result<Vec<(Uuid, String, String, Option<String>, Option<String>)>, sqlx::Error>;

    /// 人员激活开关：平台 service principal 不参与；只操作本租户成员。
    async fn set_principal_active(
        &self,
        principal_id: Uuid,
        is_active: bool,
        tenant_id: Uuid,
    ) -> Result<u64, sqlx::Error>;

    async fn load_embedding_config(
        &self,
    ) -> Result<(String, String, bool, DateTime<Utc>), sqlx::Error>;

    async fn update_embedding_config(
        &self,
        endpoint: &str,
        model: &str,
        enabled: bool,
        updated_by: Uuid,
    ) -> Result<(String, String, bool, DateTime<Utc>), sqlx::Error>;

    // ── docproc 守卫 ──
    /// 版本 → 文档 id（租户边界）。
    async fn document_id_of_version(
        &self,
        tenant_id: Uuid,
        version_id: Uuid,
    ) -> Result<Uuid, sqlx::Error>;

    /// 处理任务 → 版本 id。
    async fn version_id_of_job(&self, job_id: Uuid) -> Result<Uuid, sqlx::Error>;

    /// 版本 → 文档 id。
    async fn document_id_of_version_row(&self, version_id: Uuid) -> Result<Uuid, sqlx::Error>;

    // ── 杂项守卫 ──
    async fn intake_project_id(
        &self,
        tenant_id: Uuid,
        intake_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error>;

    /// 设备状态：不存在 → None。
    async fn device_status(
        &self,
        device_id: Uuid,
    ) -> Result<Option<(Uuid, Uuid, String)>, sqlx::Error>;

    /// 活跃 Team 成员判定（守卫 Team 读面：principal ACTIVE 且在 Team 名册）。
    async fn active_team_membership(
        &self,
        team_id: Uuid,
        principal_id: Uuid,
    ) -> Result<bool, sqlx::Error>;

    // ── 平台审计（四身份列全写，C174）──
    /// intake 任务人工重试留痕（project/tenant 由子查询锚定）。
    #[allow(clippy::too_many_arguments)] // 审计行字段齐写
    async fn audit_intake_retry(
        &self,
        intake_id: Uuid,
        actor: &crate::actor::AuditActor,
        task_id: Uuid,
        after_data: serde_json::Value,
    ) -> Result<(), sqlx::Error>;

    /// outbox 死信重放留痕。
    #[allow(clippy::too_many_arguments)] // 审计行字段齐写
    async fn audit_outbox_replay(
        &self,
        project_id: Uuid,
        actor: &crate::actor::AuditActor,
        event_id: Uuid,
        result: &str,
    ) -> Result<(), sqlx::Error>;

    /// 设备声明被忽略（tenant_audit_log；target_principal_id 形态）。
    async fn audit_device_claim_ignored(
        &self,
        tenant_id: Uuid,
        actor: &crate::actor::AuditActor,
        reason_json: serde_json::Value,
    ) -> Result<(), sqlx::Error>;
}
