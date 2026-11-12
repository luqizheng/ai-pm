//! [`CustomerRepo`] 的 Postgres 实现（任务书批 B 域 10：pm-api `customers.rs`
//! 的 SQL 收口）。SQL 与可见性谓词原样迁移，行为不变。
use async_trait::async_trait;
use uuid::Uuid;

use pm_application::repo::customer::{CustomerRepo, Page};
use pm_application::rows::CustomerRow;

/// 无状态连接句柄（`PgPool` 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub struct PgCustomerRepo {
    pool: sqlx::PgPool,
}

impl PgCustomerRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

/// 列表/详情共用的列投影；`project_count` 由子查询算，避免 GROUP BY 与 LIMIT
/// 交互出错（客户数量小，子查询代价可忽略）。
const CUSTOMER_COLUMNS: &str = "c.id, c.code, c.name, c.short_name, c.industry, c.notes, \
     c.parent_id, c.status, c.created_at, c.updated_at, \
     (SELECT count(*) FROM core.project p WHERE p.customer_id = c.id)::bigint AS project_count";

/// T-022 可见性谓词（列表/详情共用）：
///
/// - 调用者是该客户**任一项目**的成员；或
/// - 该客户**尚无项目**（待归类池，同租户活跃成员可见，避免「建完客户就消失」）。
///
/// 跨租户一律不可见。
const CUSTOMER_VISIBLE: &str = "c.tenant_id = $2 AND ( \
       NOT EXISTS (SELECT 1 FROM core.project p WHERE p.customer_id = c.id) \
    OR EXISTS (SELECT 1 FROM core.project p \
                 JOIN core.project_member m ON m.project_id = p.id \
                WHERE p.customer_id = c.id AND m.principal_id = $3))";

#[async_trait]
impl CustomerRepo for PgCustomerRepo {
    async fn visible_to(
        &self,
        tenant_id: Uuid,
        customer_id: Uuid,
        principal_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let sql = format!(
            "SELECT EXISTS (SELECT 1 FROM core.customer c WHERE c.id = $1 AND {CUSTOMER_VISIBLE})"
        );
        let ok: bool = sqlx::query_scalar(&sql)
            .bind(customer_id)
            .bind(tenant_id)
            .bind(principal_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(ok)
    }

    async fn find_visible_row(
        &self,
        tenant_id: Uuid,
        customer_id: Uuid,
        principal_id: Uuid,
    ) -> Result<Option<CustomerRow>, sqlx::Error> {
        let sql = format!(
            "SELECT {CUSTOMER_COLUMNS} FROM core.customer c \
              WHERE c.id = $1 AND {CUSTOMER_VISIBLE}"
        );
        let row = sqlx::query_as::<_, CustomerRow>(&sql)
            .bind(customer_id)
            .bind(tenant_id)
            .bind(principal_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    async fn list_visible(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        query: Option<&str>,
        status: Option<&str>,
        page: Page,
    ) -> Result<(Vec<CustomerRow>, i64), sqlx::Error> {
        // 可见性谓词与详情同构，只是占位符不同（$5=principal, $6=tenant）。
        let sql = format!(
            "SELECT {CUSTOMER_COLUMNS} FROM core.customer c \
             WHERE c.tenant_id = $6 \
               AND ( NOT EXISTS (SELECT 1 FROM core.project p WHERE p.customer_id = c.id) \
                  OR EXISTS (SELECT 1 FROM core.project p \
                               JOIN core.project_member m ON m.project_id = p.id \
                              WHERE p.customer_id = c.id AND m.principal_id = $5)) \
               AND ($1::text IS NULL OR c.name ILIKE $1 OR c.code ILIKE $1 OR c.short_name ILIKE $1) \
               AND ($2::text IS NULL OR c.status = $2) \
             ORDER BY c.created_at DESC LIMIT $3 OFFSET $4"
        );
        let items: Vec<CustomerRow> = sqlx::query_as(&sql)
            .bind(query)
            .bind(status)
            .bind(page.size)
            .bind(page.offset)
            .bind(principal_id)
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await?;

        let total_sql = "SELECT count(*) FROM core.customer c \
             WHERE c.tenant_id = $4 \
               AND ( NOT EXISTS (SELECT 1 FROM core.project p WHERE p.customer_id = c.id) \
                  OR EXISTS (SELECT 1 FROM core.project p \
                               JOIN core.project_member m ON m.project_id = p.id \
                              WHERE p.customer_id = c.id AND m.principal_id = $3)) \
               AND ($1::text IS NULL OR c.name ILIKE $1 OR c.code ILIKE $1 OR c.short_name ILIKE $1) \
               AND ($2::text IS NULL OR c.status = $2)";
        let total: i64 = sqlx::query_scalar(total_sql)
            .bind(query)
            .bind(status)
            .bind(principal_id)
            .bind(tenant_id)
            .fetch_one(&self.pool)
            .await?;
        Ok((items, total))
    }

    async fn insert_customer(
        &self,
        tenant_id: Uuid,
        code: &str,
        name: &str,
        short_name: Option<&str>,
        industry: Option<&str>,
        notes: Option<&str>,
        parent_id: Option<Uuid>,
    ) -> Result<Uuid, sqlx::Error> {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO core.customer \
             (code, name, short_name, industry, notes, parent_id, status, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, $6, 'active', $7) RETURNING id",
        )
        .bind(code)
        .bind(name)
        .bind(short_name)
        .bind(industry)
        .bind(notes)
        .bind(parent_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    async fn update_customer(
        &self,
        tenant_id: Uuid,
        customer_id: Uuid,
        code: Option<&str>,
        name: &str,
        short_name: Option<&str>,
        industry: Option<&str>,
        notes: Option<&str>,
        status: &str,
        parent_id: Option<Uuid>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE core.customer SET code = $2, name = $3, short_name = $4, industry = $5, \
                    notes = $6, status = $7, parent_id = $8, updated_at = now() \
             WHERE id = $1 AND tenant_id = $9",
        )
        .bind(customer_id)
        .bind(code)
        .bind(name)
        .bind(short_name)
        .bind(industry)
        .bind(notes)
        .bind(status)
        .bind(parent_id)
        .bind(tenant_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
