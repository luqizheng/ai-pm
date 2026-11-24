//! [`CognitionRepo`] 的 Postgres 实现（任务书批 B 第三域：pm-api
//! `pm_cognition.rs` 的 SQL 收口）。SQL 与事务边界原样迁移，行为不变。

use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use pm_application::repo::cognition::{
    CognitionDecision, CognitionItemRow, CognitionRepo, CognitionRepoError, CognitionRunRow,
    DerivationError, DerivationInput, DerivedItem, KnowledgeRow, RunStarted,
};

/// 无状态连接句柄（`PgPool` 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub struct PgCognitionRepo {
    pool: sqlx::PgPool,
}

impl PgCognitionRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

type RepoResult<T> = Result<T, CognitionRepoError>;

#[async_trait]
impl CognitionRepo for PgCognitionRepo {
    async fn derivation_input(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<DerivationInput, DerivationError> {
        let project: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT name, description FROM core.project \
             WHERE id = $1 AND tenant_id = $2",
        )
        .bind(project_id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some((project_name, project_description)) = project else {
            return Err(DerivationError::NotFound);
        };
        let repositories: Vec<(Uuid, String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT id, name, url, default_branch, role \
             FROM integration.project_repository \
             WHERE tenant_id = $1 AND project_id = $2 ORDER BY created_at",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(DerivationInput {
            project_name,
            project_description,
            repositories,
        })
    }

    async fn recent_runs(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        limit: i64,
    ) -> RepoResult<Vec<CognitionRunRow>> {
        let runs: Vec<CognitionRunRow> = sqlx::query_as(
            "SELECT id, scope, item_count, confirmed_count, status, created_at \
             FROM ai.cognition_run WHERE tenant_id = $1 AND project_id = $2 \
             ORDER BY created_at DESC LIMIT $3",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(runs)
    }

    async fn active_knowledge_count(&self, tenant_id: Uuid, project_id: Uuid) -> RepoResult<i64> {
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ai.project_knowledge \
             WHERE tenant_id = $1 AND project_id = $2 AND status = 'active'",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(n)
    }

    async fn proposed_item_count(&self, tenant_id: Uuid, project_id: Uuid) -> RepoResult<i64> {
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ai.cognition_item ci \
             JOIN ai.cognition_run cr ON cr.id = ci.run_id \
             WHERE ci.tenant_id = $1 AND ci.project_id = $2 AND ci.state = 'proposed' \
               AND cr.status <> 'cancelled'",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(n)
    }

    async fn run_items(&self, tenant_id: Uuid, run_id: Uuid) -> RepoResult<Vec<CognitionItemRow>> {
        let items: Vec<CognitionItemRow> = sqlx::query_as(
            "SELECT id, domain, content, evidence, confidence, has_conflict, \
                    state, decided_by, decided_at, decision_note \
             FROM ai.cognition_item WHERE tenant_id = $1 AND run_id = $2 \
             ORDER BY created_at",
        )
        .bind(tenant_id)
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(items)
    }

    async fn active_knowledge(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> RepoResult<Vec<KnowledgeRow>> {
        let rows: Vec<KnowledgeRow> = sqlx::query_as(
            "SELECT id, domain, content, version, source_item_id, confirmed_by, confirmed_at \
             FROM ai.project_knowledge \
             WHERE tenant_id = $1 AND project_id = $2 AND status = 'active' \
             ORDER BY confirmed_at DESC",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn start_run(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        started_by: Uuid,
        scope: &Value,
        items: &[DerivedItem<'_>],
        repo_versions: &Value,
    ) -> RepoResult<RunStarted> {
        let mut tx = self.pool.begin().await?;

        let run_id: Uuid = sqlx::query_scalar(
            "INSERT INTO ai.cognition_run (tenant_id, project_id, status, scope, started_by) \
             VALUES ($1, $2, 'pending', $3, $4) RETURNING id",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(scope)
        .bind(started_by)
        .fetch_one(&mut *tx)
        .await?;

        for item in items {
            sqlx::query(
                "INSERT INTO ai.cognition_item \
                 (tenant_id, project_id, run_id, domain, content, evidence, confidence) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(run_id)
            .bind(item.domain)
            .bind(item.content)
            .bind(item.evidence)
            .bind(item.confidence)
            .execute(&mut *tx)
            .await?;
        }

        let item_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ai.cognition_item WHERE tenant_id = $1 AND run_id = $2",
        )
        .bind(tenant_id)
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        let item_count = item_count as i32;

        // V1 派生是确定性的：全部条目成立 → completed（编排切片引入部分失败语义）。
        sqlx::query(
            "UPDATE ai.cognition_run SET status = 'completed', item_count = $2, \
                    repo_versions = $3, finished_at = now() \
             WHERE tenant_id = $1 AND id = $4",
        )
        .bind(tenant_id)
        .bind(item_count)
        .bind(repo_versions)
        .bind(run_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(RunStarted { run_id, item_count })
    }

    async fn decide_item(
        &self,
        tenant_id: Uuid,
        item_id: Uuid,
        next_state: &str,
        decided_by: Uuid,
        note: Option<&str>,
        edited_content: Option<&str>,
    ) -> RepoResult<CognitionDecision> {
        let mut tx = self.pool.begin().await?;

        let item: Option<(String, String, Option<Uuid>)> = sqlx::query_as(
            "SELECT state, content, knowledge_id FROM ai.cognition_item \
             WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((state, content, knowledge_id)) = item else {
            return Err(CognitionRepoError::NotFound("cognition item not found"));
        };

        if state != "proposed" {
            return Err(CognitionRepoError::Conflict(
                "cognition item is already decided",
            ));
        }

        // Confirm：落项目知识（Edit & Confirm 的修订正文优先；重复确认 409）。
        let mut knowledge_id = knowledge_id;
        let mut final_content: Option<String> = None;
        if next_state == "confirmed" {
            if knowledge_id.is_some() {
                return Err(CognitionRepoError::Conflict(
                    "cognition item already produced knowledge",
                ));
            }
            let content = match edited_content {
                Some(edited) if !edited.trim().is_empty() && edited.trim() != content => {
                    final_content = Some(edited.trim().to_string());
                    edited.trim().to_string()
                }
                _ => content,
            };
            knowledge_id = Some(
                sqlx::query_scalar(
                    "INSERT INTO ai.project_knowledge \
                     (tenant_id, project_id, domain, content, source_item_id, confirmed_by) \
                     SELECT tenant_id, project_id, domain, $3, id, $4 FROM ai.cognition_item \
                     WHERE tenant_id = $1 AND id = $2 RETURNING id",
                )
                .bind(tenant_id)
                .bind(item_id)
                .bind(&content)
                .bind(decided_by)
                .fetch_one(&mut *tx)
                .await?,
            );
        }

        sqlx::query(
            "UPDATE ai.cognition_item SET state = $3, decided_by = $4, decided_at = now(), \
                    decision_note = $5, knowledge_id = COALESCE($6, knowledge_id), \
                    content = COALESCE($7, content) \
             WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(item_id)
        .bind(next_state)
        .bind(decided_by)
        .bind(note.filter(|n| !n.trim().is_empty()))
        .bind(knowledge_id)
        .bind(&final_content)
        .execute(&mut *tx)
        .await?;

        if next_state == "confirmed" {
            sqlx::query(
                "UPDATE ai.cognition_run SET confirmed_count = confirmed_count + 1 \
                 WHERE tenant_id = $1 AND id = \
                   (SELECT run_id FROM ai.cognition_item WHERE tenant_id = $1 AND id = $2)",
            )
            .bind(tenant_id)
            .bind(item_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(CognitionDecision { knowledge_id })
    }
}
