//! [`BaselineRepo`] 的 Postgres 实现（任务书批 B 第二域：pm-api
//! `pm_baseline.rs` 的 SQL 收口）。SQL 与事务边界原样迁移，行为不变；
//! NotFound/Conflict 域语义在本层翻译（HTTP 状态码映射留在 handler）。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use pm_application::repo::baseline::{
    BaselineConfirmed, BaselineCurrent, BaselineHistoryRow, BaselineItemRow, BaselineRepo,
    BaselineRepoError, ItemDecision, PendingProposal, ProposalCreated,
};

/// 无状态连接句柄（`PgPool` 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub struct PgBaselineRepo {
    pool: sqlx::PgPool,
}

impl PgBaselineRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

type RepoResult<T> = Result<T, BaselineRepoError>;

#[async_trait]
impl BaselineRepo for PgBaselineRepo {
    async fn current_version(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> RepoResult<Option<BaselineCurrent>> {
        let row: Option<(i32, Uuid, Uuid, DateTime<Utc>)> = sqlx::query_as(
            "SELECT version, proposal_id, confirmed_by, confirmed_at FROM ai.project_baseline \
             WHERE tenant_id = $1 AND project_id = $2 ORDER BY version DESC LIMIT 1",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(version, proposal_id, confirmed_by, confirmed_at)| BaselineCurrent {
            version,
            proposal_id,
            confirmed_by,
            confirmed_at,
        }))
    }

    async fn pending_proposal(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> RepoResult<Option<PendingProposal>> {
        let row: Option<(Uuid, i32, String, Uuid, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, version, status, created_by, created_at FROM ai.baseline_proposal \
             WHERE tenant_id = $1 AND project_id = $2 AND status = 'pending'",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(
            row.map(|(id, version, status, created_by, created_at)| PendingProposal {
                id,
                version,
                status,
                created_by,
                created_at,
            }),
        )
    }

    async fn proposal_items(
        &self,
        tenant_id: Uuid,
        proposal_id: Uuid,
    ) -> RepoResult<Vec<BaselineItemRow>> {
        let items: Vec<BaselineItemRow> = sqlx::query_as(
            "SELECT id, category, content, evidence, confidence, state, \
                    decided_by, decided_at, decision_note \
             FROM ai.baseline_item WHERE tenant_id = $1 AND proposal_id = $2 \
             ORDER BY created_at",
        )
        .bind(tenant_id)
        .bind(proposal_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(items)
    }

    async fn version_history(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> RepoResult<Vec<BaselineHistoryRow>> {
        let rows: Vec<(i32, Uuid, DateTime<Utc>)> = sqlx::query_as(
            "SELECT version, proposal_id, confirmed_at FROM ai.project_baseline \
             WHERE tenant_id = $1 AND project_id = $2 ORDER BY version DESC",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(version, proposal_id, confirmed_at)| BaselineHistoryRow {
                version,
                proposal_id,
                confirmed_at,
            })
            .collect())
    }

    async fn create_proposal(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        created_by: Uuid,
    ) -> RepoResult<ProposalCreated> {
        let mut tx = self.pool.begin().await?;

        let pending: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM ai.baseline_proposal \
             WHERE tenant_id = $1 AND project_id = $2 AND status = 'pending'",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?;
        if pending.is_some() {
            return Err(BaselineRepoError::Conflict(
                "a pending baseline proposal already exists",
            ));
        }

        let version: i32 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM ai.project_baseline \
             WHERE tenant_id = $1 AND project_id = $2",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;

        let proposal_id: Uuid = sqlx::query_scalar(
            "INSERT INTO ai.baseline_proposal (tenant_id, project_id, version, created_by) \
             VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(version)
        .bind(created_by)
        .fetch_one(&mut *tx)
        .await?;

        // 条目来源①：S01-04 已确认知识（认知 → 基线的管线）。
        // 类别沿用知识的领域映射（V1 一一对应；scope/standards 等待知识细分后启用）。
        let knowledge: Vec<(String, String, Uuid)> = sqlx::query_as(
            "SELECT domain, content, id FROM ai.project_knowledge \
             WHERE tenant_id = $1 AND project_id = $2 AND status = 'active' \
             ORDER BY confirmed_at",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_all(&mut *tx)
        .await?;
        for (domain, content, kid) in &knowledge {
            sqlx::query(
                "INSERT INTO ai.baseline_item \
                 (tenant_id, project_id, proposal_id, category, content, evidence, confidence) \
                 VALUES ($1, $2, $3, $4, $5, $6, 'high')",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(proposal_id)
            .bind(domain)
            .bind(content)
            .bind(json!({"type": "knowledge", "knowledge_id": kid}))
            .execute(&mut *tx)
            .await?;
        }

        // 条目来源②：仓库快照（role + url；primary=main 角色记录，不自动认定主仓）。
        let repos: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT name, url, role FROM integration.project_repository \
             WHERE tenant_id = $1 AND project_id = $2 ORDER BY created_at",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_all(&mut *tx)
        .await?;
        for (name, url, role) in &repos {
            sqlx::query(
                "INSERT INTO ai.baseline_item \
                 (tenant_id, project_id, proposal_id, category, content, evidence, confidence) \
                 VALUES ($1, $2, $3, 'repository', $4, $5, 'high')",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(proposal_id)
            .bind(format!("Repository '{name}' ({role}): {url}"))
            .bind(json!({"type": "repository_snapshot", "role": role}))
            .execute(&mut *tx)
            .await?;
        }

        let item_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ai.baseline_item \
             WHERE tenant_id = $1 AND proposal_id = $2",
        )
        .bind(tenant_id)
        .bind(proposal_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(ProposalCreated {
            proposal_id,
            version,
            item_count,
        })
    }

    async fn decide_item(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        item_id: Uuid,
        next_state: &str,
        decided_by: Uuid,
        note: Option<&str>,
        content: Option<&str>,
    ) -> RepoResult<ItemDecision> {
        let mut tx = self.pool.begin().await?;

        // 提案必须是 pending（confirmed 即不可变，§核心原则 6）。
        let item: Option<(String, Uuid)> = sqlx::query_as(
            "SELECT bi.state, bi.proposal_id FROM ai.baseline_item bi \
             JOIN ai.baseline_proposal bp ON bp.id = bi.proposal_id \
             WHERE bi.tenant_id = $1 AND bi.project_id = $2 AND bi.id = $3 \
               AND bp.status = 'pending' FOR UPDATE OF bi",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((state, _proposal_id)) = item else {
            return Err(BaselineRepoError::NotFound(
                "baseline item not found or proposal not pending",
            ));
        };

        if state != "proposed" {
            return Err(BaselineRepoError::Conflict(
                "baseline item is already decided",
            ));
        }

        sqlx::query(
            "UPDATE ai.baseline_item SET state = $3, decided_by = $4, decided_at = now(), \
                    decision_note = $5, content = COALESCE($6, content) \
             WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(item_id)
        .bind(next_state)
        .bind(decided_by)
        .bind(note.filter(|n| !n.trim().is_empty()))
        .bind(content.filter(|c| !c.trim().is_empty()))
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(ItemDecision {
            item_id,
            next_state: next_state.to_string(),
        })
    }

    async fn confirm_pending(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        confirmed_by: Uuid,
    ) -> RepoResult<BaselineConfirmed> {
        let mut tx = self.pool.begin().await?;

        let proposal: Option<(Uuid, i32)> = sqlx::query_as::<_, (Uuid, i32)>(
            "SELECT id, version FROM ai.baseline_proposal \
             WHERE tenant_id = $1 AND project_id = $2 AND status = 'pending' \
             ORDER BY created_at DESC LIMIT 1 FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?;
        let proposal = proposal.ok_or(BaselineRepoError::NotFound("no pending baseline proposal"))?;

        let proposed: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ai.baseline_item \
             WHERE tenant_id = $1 AND proposal_id = $2 AND state = 'proposed'",
        )
        .bind(tenant_id)
        .bind(proposal.0)
        .fetch_one(&mut *tx)
        .await?;
        if proposed > 0 {
            return Err(BaselineRepoError::Conflict(
                "baseline proposal still has undecided items",
            ));
        }

        sqlx::query(
            "INSERT INTO ai.project_baseline \
             (tenant_id, project_id, version, proposal_id, confirmed_by) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(proposal.1)
        .bind(proposal.0)
        .bind(confirmed_by)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE ai.baseline_proposal SET status = 'confirmed', confirmed_by = $3, \
                    confirmed_at = now() WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(proposal.0)
        .bind(confirmed_by)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(BaselineConfirmed {
            version: proposal.1,
        })
    }
}
