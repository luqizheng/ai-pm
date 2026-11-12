//! [`DesignGuardRepo`] 的 Postgres 实现（任务书批 B 域 8：pm-api
//! `pm_designs.rs` 的守卫 SQL 收口）。SQL 原样迁移，行为不变。
use async_trait::async_trait;
use uuid::Uuid;

use pm_application::repo::design_guard::DesignGuardRepo;

/// 无状态连接句柄（`PgPool` 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub struct PgDesignGuardRepo {
    pool: sqlx::PgPool,
}

impl PgDesignGuardRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DesignGuardRepo for PgDesignGuardRepo {
    async fn design_project_id(
        &self,
        tenant_id: Uuid,
        design_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let id: Option<Uuid> =
            sqlx::query_scalar("SELECT project_id FROM design.design WHERE id = $1 AND tenant_id = $2")
                .bind(design_id)
                .bind(tenant_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(id)
    }

    async fn adr_project_id(
        &self,
        tenant_id: Uuid,
        adr_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let id: Option<Uuid> =
            sqlx::query_scalar("SELECT project_id FROM design.adr WHERE id = $1 AND tenant_id = $2")
                .bind(adr_id)
                .bind(tenant_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(id)
    }

    async fn revision_exists(
        &self,
        tenant_id: Uuid,
        design_id: Uuid,
        revision_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let exists: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM design.design_revision WHERE id = $1 AND design_id = $2 AND tenant_id = $3",
        )
        .bind(revision_id)
        .bind(design_id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(exists.is_some())
    }

    async fn approval_project_id(
        &self,
        tenant_id: Uuid,
        approval_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let id: Option<Uuid> = sqlx::query_scalar(
            "SELECT project_id FROM design.design_approval WHERE id = $1 AND tenant_id = $2",
        )
        .bind(approval_id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(id)
    }

    async fn baseline_project_id(
        &self,
        tenant_id: Uuid,
        baseline_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let id: Option<Uuid> = sqlx::query_scalar(
            "SELECT project_id FROM design.implementation_baseline WHERE id = $1 AND tenant_id = $2",
        )
        .bind(baseline_id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(id)
    }

    async fn approved_revision_ids(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<Uuid>, sqlx::Error> {
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM design.design_revision \
             WHERE tenant_id = $1 AND project_id = $2 AND state = 'approved' \
             ORDER BY created_at",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(ids)
    }
}
