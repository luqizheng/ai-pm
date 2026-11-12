//! [`AiTeamMonitorRepo`] 的 Postgres 实现（任务书批 B 域 9：pm-api
//! `ai_team_monitor.rs` 的聚合 SQL 收口）。SQL 原样迁移，行为不变。
use async_trait::async_trait;
use uuid::Uuid;

use pm_application::repo::ai_team_monitor::{
    AiTeamEventRow, AiTeamMonitorRepo, AiTeamRunRow,
};

/// 无状态连接句柄（`PgPool` 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub struct PgAiTeamMonitorRepo {
    pool: sqlx::PgPool,
}

impl PgAiTeamMonitorRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AiTeamMonitorRepo for PgAiTeamMonitorRepo {
    async fn project_tenant(&self, project_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
        let t: Option<Uuid> =
            sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
                .bind(project_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(t)
    }

    async fn roles_summary(
        &self,
        project_id: Uuid,
    ) -> Result<(i64, i64, i64), sqlx::Error> {
        let row: (i64, i64, i64) = sqlx::query_as(
            "SELECT \
               (SELECT count(*) FROM ai.project_ai_role \
                 WHERE project_id = $1 AND enabled), \
               (SELECT count(*) FROM ai.project_ai_role \
                 WHERE project_id = $1 AND NOT enabled), \
               (SELECT count(*) FROM ai.ai_role_template)",
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    async fn runs_summary(
        &self,
        project_id: Uuid,
    ) -> Result<(i64, i64, i64), sqlx::Error> {
        let row: (i64, i64, i64) = sqlx::query_as(
            "SELECT \
               count(*) FILTER (WHERE status IN ('queued', 'claimed', 'running')), \
               count(*) FILTER (WHERE outcome = 'succeeded' \
                 AND COALESCE(finished_at, created_at) >= now() - interval '24 hours'), \
               count(*) FILTER (WHERE outcome = 'failed' \
                 AND COALESCE(finished_at, created_at) >= now() - interval '24 hours') \
             FROM delivery.task_run WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    async fn tasks_summary(
        &self,
        project_id: Uuid,
    ) -> Result<(i64, i64), sqlx::Error> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT \
               count(*) FILTER (WHERE status = 'awaiting_decision'), \
               count(*) FILTER (WHERE status = 'blocked') \
             FROM delivery.task WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    async fn designated_provider(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<Option<(String, String, String)>, sqlx::Error> {
        let row: Option<(String, String, String)> = sqlx::query_as(
            "SELECT p.name, p.status, COALESCE(p.configuration ->> 'model', '') \
             FROM ai.project_ai_model_policy pol \
             JOIN ai.team_llm_provider p ON p.id = pol.provider_id AND p.tenant_id = pol.tenant_id \
             WHERE pol.tenant_id = $1 AND pol.project_id = $2",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn team_default_provider(&self, tenant_id: Uuid) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT p.name FROM ai.team_llm_provider p \
             WHERE p.tenant_id = $1 AND p.status = 'enabled' \
               AND COALESCE(p.configuration ->> 'model', '') <> '' \
             ORDER BY p.created_at, p.id LIMIT 1",
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(name,)| name))
    }

    async fn recent_runs(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AiTeamRunRow>, sqlx::Error> {
        let rows: Vec<AiTeamRunRow> = sqlx::query_as(
            "SELECT tr.id AS run_id, tr.task_id, t.title AS task_title, \
                    tr.status, tr.stage, tr.run_kind, tr.attempt_no, \
                    tr.executor_kind, tr.execution_mode, tr.release_reason, \
                    tr.scheduling_shield, tr.claimed_at, tr.outcome, tr.terminated_at, \
                    COALESCE(tr.terminated_at, tr.finished_at) AS finished_at, \
                    tr.created_at \
             FROM delivery.task_run tr \
             JOIN delivery.task t ON t.id = tr.task_id \
             WHERE tr.project_id = $1 \
             ORDER BY tr.created_at DESC, tr.id DESC \
             LIMIT $2",
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn recent_events(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AiTeamEventRow>, sqlx::Error> {
        let rows: Vec<AiTeamEventRow> = sqlx::query_as(
            "SELECT a.action, a.target_id, t.title AS target_title, \
                    p.display_name AS actor, \
                    COALESCE(a.after_data ->> 'release_reason', a.after_data ->> 'reason') AS reason, \
                    a.created_at \
             FROM governance.audit_log a \
             LEFT JOIN core.principal p ON p.id = a.actor_id \
             LEFT JOIN delivery.task_run tr ON tr.id = a.target_id \
             LEFT JOIN delivery.task t \
               ON t.id = COALESCE(tr.task_id, CASE WHEN a.target_type = 'task' THEN a.target_id END) \
             WHERE a.project_id = $1 \
               AND a.source_kind = 'run_dispatch' \
               AND a.action IN ('claim_task_run', 'skip_task_run', 'reclaim_task_run', \
                                'task_ready_skipped', 'task_ready_assignee_resolved') \
             ORDER BY a.created_at DESC, a.id DESC \
             LIMIT $2",
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
