//! [`PlatformOpsRepo`] 的 Postgres 实现（任务书 B 收尾批：admin/docproc/
//! 守卫杂项/平台审计 SQL 收口）。SQL 原样迁移，行为不变。
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::repo::platform_ops::PlatformOpsRepo;

/// 无状态连接句柄（`PgPool` 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub struct PgPlatformOpsRepo {
    pool: sqlx::PgPool,
}

impl PgPlatformOpsRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PlatformOpsRepo for PgPlatformOpsRepo {
    async fn principal_memberships(
        &self,
        principal_id: Uuid,
    ) -> Result<Vec<(Uuid, String, String, Option<String>, Option<String>)>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (Uuid, String, String, Option<String>, Option<String>)>(
            // 只统计 ACTIVE 成员关系：未激活 principal 的 provision member 行
            // 不算「已加入」（onboarding_required 语义与之配套）；休眠租户
            // 的成员关系已随 dissolve 清空，双重保险再滤 is_active。
            "SELECT t.id, t.display_name, tm.role, t.slug, t.description \
               FROM core.tenant_member tm \
               JOIN core.tenant t ON t.id = tm.tenant_id \
               JOIN core.principal p ON p.id = tm.principal_id \
              WHERE tm.principal_id = $1 AND p.is_active AND t.is_active \
              ORDER BY t.created_at",
        )
        .bind(principal_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn set_principal_active(
        &self,
        principal_id: Uuid,
        is_active: bool,
        tenant_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        // 平台 service principal（`ai-pm-dispatch`）不参与人员激活开关；
        // 且只能操作**本租户**的成员（跨租户 id 一律按不存在处理）。
        let result = sqlx::query(
            "UPDATE core.principal SET is_active = $2 \
              WHERE id = $1 AND kind <> 'service' \
                AND EXISTS (SELECT 1 FROM core.tenant_member m \
                             WHERE m.principal_id = core.principal.id AND m.tenant_id = $3)",
        )
        .bind(principal_id)
        .bind(is_active)
        .bind(tenant_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    async fn load_embedding_config(
        &self,
    ) -> Result<(String, String, bool, DateTime<Utc>), sqlx::Error> {
        let row: (String, String, bool, DateTime<Utc>) = sqlx::query_as(
            "SELECT endpoint, model, enabled, updated_at FROM platform.embedding_config WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    async fn update_embedding_config(
        &self,
        endpoint: &str,
        model: &str,
        enabled: bool,
        updated_by: Uuid,
    ) -> Result<(String, String, bool, DateTime<Utc>), sqlx::Error> {
        let row: (String, String, bool, DateTime<Utc>) = sqlx::query_as(
            "UPDATE platform.embedding_config \
                SET endpoint = $2, model = $3, enabled = $4, updated_by = $5, updated_at = now() \
              WHERE id = 1 \
              RETURNING endpoint, model, enabled, updated_at",
        )
        .bind(endpoint)
        .bind(model)
        .bind(enabled)
        .bind(updated_by)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    async fn document_id_of_version(
        &self,
        tenant_id: Uuid,
        version_id: Uuid,
    ) -> Result<Uuid, sqlx::Error> {
        let id: Uuid = sqlx::query_scalar(
            "SELECT d.id FROM docproc.document d \
             JOIN docproc.document_version ver ON ver.document_id = d.id \
             WHERE ver.id = $1 AND d.tenant_id = $2",
        )
        .bind(version_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    async fn version_id_of_job(&self, job_id: Uuid) -> Result<Uuid, sqlx::Error> {
        let id: Uuid =
            sqlx::query_scalar("SELECT document_version_id FROM docproc.processing_job WHERE id = $1")
                .bind(job_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(id)
    }

    async fn document_id_of_version_row(&self, version_id: Uuid) -> Result<Uuid, sqlx::Error> {
        let id: Uuid =
            sqlx::query_scalar("SELECT document_id FROM docproc.document_version WHERE id = $1")
                .bind(version_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(id)
    }

    async fn intake_project_id(
        &self,
        tenant_id: Uuid,
        intake_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let id: Option<Uuid> = sqlx::query_scalar(
            "SELECT project_id FROM integration.intake_record \
              WHERE id = $1 AND tenant_id = $2",
        )
        .bind(intake_id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(id)
    }

    async fn device_status(
        &self,
        device_id: Uuid,
    ) -> Result<Option<(Uuid, Uuid, String)>, sqlx::Error> {
        let row: Option<(Uuid, Uuid, String)> = sqlx::query_as(
            "SELECT tenant_id, owner_principal_id, status FROM devenv.device WHERE id = $1",
        )
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn active_tenant_membership(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let active: Option<Uuid> = sqlx::query_scalar(
            "SELECT tm.tenant_id \
               FROM core.tenant_member tm \
               JOIN core.principal p ON p.id = tm.principal_id \
               JOIN core.tenant t ON t.id = tm.tenant_id AND t.is_active \
              WHERE tm.tenant_id = $1 AND tm.principal_id = $2 AND p.is_active",
        )
        .bind(tenant_id)
        .bind(principal_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(active.is_some())
    }

    async fn audit_intake_retry(
        &self,
        intake_id: Uuid,
        actor: &AuditActor,
        task_id: Uuid,
        after_data: serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO governance.audit_log \
             (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
              action, target_type, target_id, after_data, source_kind, tenant_id) \
             VALUES ((SELECT project_id FROM integration.intake_record WHERE id = $1), $2, $3, $4, $5, $6, \
                     'intake_task_retry', 'intake_task', $7, $8, 'pm_api', \
                     (SELECT tenant_id FROM core.project WHERE id = \
                       (SELECT project_id FROM integration.intake_record WHERE id = $1)))",
        )
        .bind(intake_id)
        .bind(actor.principal_id)
        .bind(actor.identity_id)
        .bind(&actor.client)
        .bind(&actor.auth_mode)
        .bind(actor.claimed_device_id)
        .bind(task_id)
        .bind(after_data)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn audit_outbox_replay(
        &self,
        project_id: Uuid,
        actor: &AuditActor,
        event_id: Uuid,
        result: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO governance.audit_log \
             (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
              action, target_type, target_id, after_data, source_kind, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, $6, 'outbox_dead_letter_replay', 'outbox_event', $7, $8, 'pm_api', \
                     (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(project_id)
        .bind(actor.principal_id)
        .bind(actor.identity_id)
        .bind(&actor.client)
        .bind(&actor.auth_mode)
        .bind(actor.claimed_device_id)
        .bind(event_id)
        .bind(serde_json::json!({ "result": result }))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn audit_device_claim_ignored(
        &self,
        tenant_id: Uuid,
        actor: &AuditActor,
        reason_json: serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO governance.tenant_audit_log \
             (tenant_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
              action, target_principal_id, before_data, after_data) \
             VALUES ($1, $2, $3, $4, $5, NULL, 'device_attribution_ignored', $2, NULL, $6)",
        )
        .bind(tenant_id)
        .bind(actor.principal_id)
        .bind(actor.identity_id)
        .bind(&actor.client)
        .bind(&actor.auth_mode)
        .bind(reason_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
