//! [`IdentityRepo`] 的 Postgres 实现（任务书批 B 第四域：pm-api `auth.rs`
//! 的身份供给 SQL 收口）。SQL 与并发语义原样迁移，行为不变。

use async_trait::async_trait;
use uuid::Uuid;

use pm_application::repo::identity::{
    IdentityRepo, IdentityRepoError, ProvisionIdentity, ResolvedIdentity,
};

/// 无状态连接句柄（`PgPool` 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub struct PgIdentityRepo {
    pool: sqlx::PgPool,
}

impl PgIdentityRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

type RepoResult<T> = Result<T, IdentityRepoError>;

#[async_trait]
impl IdentityRepo for PgIdentityRepo {
    async fn find_tenant(
        &self,
        provider: &str,
        external_key: &str,
    ) -> RepoResult<Option<Uuid>> {
        let id: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM core.tenant WHERE provider = $1 AND external_key = $2")
                .bind(provider)
                .bind(external_key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(id)
    }

    async fn insert_tenant(
        &self,
        provider: &str,
        external_key: &str,
        display_name: &str,
    ) -> RepoResult<Option<Uuid>> {
        let inserted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO core.tenant (provider, external_key, display_name) \
             VALUES ($1, $2, $3) \
             ON CONFLICT (provider, external_key) DO NOTHING \
             RETURNING id",
        )
        .bind(provider)
        .bind(external_key)
        .bind(display_name)
        .fetch_optional(&self.pool)
        .await?;
        Ok(inserted)
    }

    async fn find_identity(
        &self,
        provider: &str,
        tenant_id: Uuid,
        subject: &str,
    ) -> RepoResult<Option<ResolvedIdentity>> {
        let row: Option<(Uuid, Uuid, bool, String)> = sqlx::query_as(
            "SELECT pi.id, pi.principal_id, p.is_active, p.display_name \
               FROM core.principal_identity pi \
               JOIN core.principal p ON p.id = pi.principal_id \
              WHERE pi.provider = $1 AND pi.tenant_id = $2 AND pi.subject = $3",
        )
        .bind(provider)
        .bind(tenant_id)
        .bind(subject)
        .fetch_optional(&self.pool)
        .await?;
        if let Some((identity_id, ..)) = &row {
            // 节流：5 分钟内的重复请求不再写库（Q1；原 auth.rs 语义）。
            sqlx::query(
                "UPDATE core.principal_identity SET last_seen_at = now() \
                  WHERE id = $1 AND last_seen_at < now() - interval '5 minutes'",
            )
            .bind(identity_id)
            .execute(&self.pool)
            .await?;
        }
        Ok(row.map(|(identity_id, principal_id, is_active, display_name)| ResolvedIdentity {
            identity_id,
            principal_id,
            is_active,
            display_name,
        }))
    }

    async fn provision_principal(
        &self,
        input: ProvisionIdentity<'_>,
    ) -> RepoResult<ResolvedIdentity> {
        let mut tx = self.pool.begin().await?;

        let principal_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO core.principal (id, kind, display_name, is_active) \
             VALUES ($1, 'human', $2, $3)",
        )
        .bind(principal_id)
        .bind(input.display_name)
        .bind(input.is_active)
        .execute(&mut *tx)
        .await?;

        let inserted: Option<(Uuid, Uuid)> = sqlx::query_as(
            "INSERT INTO core.principal_identity \
                 (provider, tenant_id, subject, principal_id) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (provider, tenant_id, subject) DO NOTHING \
             RETURNING id, principal_id",
        )
        .bind(input.provider)
        .bind(input.tenant_id)
        .bind(input.subject)
        .bind(principal_id)
        .fetch_optional(&mut *tx)
        .await?;

        let resolved = match inserted {
            Some((identity_id, winner)) => {
                // 成员行必须在 identity 抢占成功之后插入：并发输家会删 principal，
                // 若提前插了 tenant_member 会因外键而删不掉。
                sqlx::query(
                    "INSERT INTO core.tenant_member (tenant_id, principal_id, role) \
                     VALUES ($1, $2, $3) ON CONFLICT (tenant_id, principal_id) DO NOTHING",
                )
                .bind(input.tenant_id)
                .bind(winner)
                .bind(input.role)
                .execute(&mut *tx)
                .await?;
                ResolvedIdentity {
                    identity_id,
                    principal_id: winner,
                    is_active: input.is_active,
                    display_name: input.display_name.to_string(),
                }
            }
            None => {
                // 并发输家：刚建的 principal 未被任何行引用，直接删掉。
                sqlx::query("DELETE FROM core.principal WHERE id = $1")
                    .bind(principal_id)
                    .execute(&mut *tx)
                    .await?;
                let row: (Uuid, Uuid, bool, String) = sqlx::query_as(
                    "SELECT pi.id, pi.principal_id, p.is_active, p.display_name \
                       FROM core.principal_identity pi \
                       JOIN core.principal p ON p.id = pi.principal_id \
                      WHERE pi.provider = $1 AND pi.tenant_id = $2 AND pi.subject = $3",
                )
                .bind(input.provider)
                .bind(input.tenant_id)
                .bind(input.subject)
                .fetch_one(&mut *tx)
                .await?;
                ResolvedIdentity {
                    identity_id: row.0,
                    principal_id: row.1,
                    is_active: row.2,
                    display_name: row.3,
                }
            }
        };
        tx.commit().await?;
        Ok(resolved)
    }

    async fn tenant_role(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
    ) -> RepoResult<Option<String>> {
        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM core.tenant_member \
              WHERE tenant_id = $1 AND principal_id = $2",
        )
        .bind(tenant_id)
        .bind(principal_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(role)
    }

    async fn any_tenant_role(&self, principal_id: Uuid) -> RepoResult<Option<String>> {
        let role: Option<String> =
            sqlx::query_scalar("SELECT role FROM core.tenant_member WHERE principal_id = $1 LIMIT 1")
                .bind(principal_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(role)
    }

    async fn principal_kind(&self, principal_id: Uuid) -> RepoResult<Option<String>> {
        let kind: Option<String> =
            sqlx::query_scalar("SELECT kind FROM core.principal WHERE id = $1")
                .bind(principal_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(kind)
    }

    async fn ensure_dev_off(&self, principal_id: Uuid, tenant_id: Uuid) -> RepoResult<()> {
        // principal 与 tenant_member 一起物化到系统租户（Q6）；只随写请求落库，
        // 保证空库下 `GET` 仍返回零总数。
        sqlx::query(
            "INSERT INTO core.principal (id, kind, display_name, is_active) \
             VALUES ($1, 'human', 'dev-off', true) ON CONFLICT (id) DO NOTHING",
        )
        .bind(principal_id)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO core.tenant_member (tenant_id, principal_id, role) \
             VALUES ($1, $2, 'owner') ON CONFLICT (tenant_id, principal_id) DO NOTHING",
        )
        .bind(tenant_id)
        .bind(principal_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
