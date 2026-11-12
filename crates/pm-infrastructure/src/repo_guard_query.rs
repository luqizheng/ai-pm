//! [`GuardQueryRepo`] 的 Postgres 实现（任务书批 B 域 11：pm-api 各 handler
//! 的同构锚定守卫 SQL 收口）。SQL 原样迁移，行为不变。
use async_trait::async_trait;
use uuid::Uuid;

use pm_application::repo::guard_query::GuardQueryRepo;

/// 无状态连接句柄（`PgPool` 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub struct PgGuardQueryRepo {
    pool: sqlx::PgPool,
}

impl PgGuardQueryRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

/// 生成「SELECT project_id FROM {table} WHERE id = $1 AND tenant_id = $2」的
/// 薄包装（六处同构唯一差异是表名）。
async fn project_id_of(
    pool: &sqlx::PgPool,
    table: &str,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    let sql = format!(
        "SELECT project_id FROM {table} WHERE id = $1 AND tenant_id = $2"
    );
    let id: Option<Uuid> = sqlx::query_scalar(&sql)
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await?;
    Ok(id)
}

#[async_trait]
impl GuardQueryRepo for PgGuardQueryRepo {
    async fn task_project_id(
        &self,
        tenant_id: Uuid,
        task_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        project_id_of(&self.pool, "delivery.task", tenant_id, task_id).await
    }

    async fn requirement_project_id(
        &self,
        tenant_id: Uuid,
        requirement_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        project_id_of(&self.pool, "delivery.requirement", tenant_id, requirement_id).await
    }

    async fn plan_project_id(
        &self,
        tenant_id: Uuid,
        plan_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        project_id_of(&self.pool, "plan.task_plan", tenant_id, plan_id).await
    }

    async fn interaction_project_id(
        &self,
        tenant_id: Uuid,
        interaction_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        project_id_of(&self.pool, "delivery.task_interaction", tenant_id, interaction_id).await
    }

    async fn run_project_id(
        &self,
        tenant_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        project_id_of(&self.pool, "delivery.task_run", tenant_id, run_id).await
    }

    async fn project_exists(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let id: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM core.project WHERE id = $1 AND tenant_id = $2")
                .bind(project_id)
                .bind(tenant_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(id.is_some())
    }

    async fn task_exists(&self, tenant_id: Uuid, task_id: Uuid) -> Result<bool, sqlx::Error> {
        let id: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM delivery.task WHERE id = $1 AND tenant_id = $2")
                .bind(task_id)
                .bind(tenant_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(id.is_some())
    }

    async fn approval_head(
        &self,
        tenant_id: Uuid,
        approval_id: Uuid,
    ) -> Result<Option<(Uuid, String)>, sqlx::Error> {
        let row: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT project_id, status FROM governance.approval_request \
              WHERE id = $1 AND tenant_id = $2",
        )
        .bind(approval_id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn approval_decision_row(
        &self,
        approval_id: Uuid,
    ) -> Result<Option<(String, Option<chrono::DateTime<chrono::Utc>>)>, sqlx::Error> {
        let row: Option<(String, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
            "SELECT status, decided_at FROM governance.approval_request WHERE id = $1",
        )
        .bind(approval_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn project_tenant(&self, project_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
        let t: Option<Uuid> =
            sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
                .bind(project_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(t)
    }

    async fn repository_exists(
        &self,
        tenant_id: Uuid,
        repository_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM integration.project_repository WHERE id = $1 AND tenant_id = $2",
        )
        .bind(repository_id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(id.is_some())
    }

    async fn device_owner(
        &self,
        tenant_id: Uuid,
        device_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "SELECT owner_principal_id FROM devenv.device WHERE id = $1 AND tenant_id = $2",
        )
        .bind(device_id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(owner)
    }

    async fn discovery_question_exists(
        &self,
        tenant_id: Uuid,
        session_id: Uuid,
        question_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM discovery.discovery_question \
             WHERE tenant_id = $1 AND id = $2 AND session_id = $3",
        )
        .bind(tenant_id)
        .bind(question_id)
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(id.is_some())
    }

    async fn published_document_revision_ids(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<Uuid>, sqlx::Error> {
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT r.id FROM ai.document_revision r \
               JOIN ai.project_document d ON d.id = r.document_id \
              WHERE d.project_id = $1 AND d.tenant_id = $2 AND r.status = 'published' \
              ORDER BY d.doc_key, r.revision_no",
        )
        .bind(project_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(ids)
    }
}
