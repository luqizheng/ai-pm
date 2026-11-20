//! [`RunRepo`] 的 Postgres 实现（任务书批 B 域 6：pm-api `pm_runs.rs` 的 SQL
//! 收口）。SQL 原样迁移，行为不变。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use pm_application::repo::runs::{
    Page, RunAuthorizationCtx, RunEventRow, RunEventSummary, RunRepo, RunRepositoryRow, RunSummary,
    TaskRunDetailRow, TaskRunRow,
};

/// 无状态连接句柄（`PgPool` 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub struct PgRunRepo {
    pool: sqlx::PgPool,
}

impl PgRunRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RunRepo for PgRunRepo {
    async fn run_authorization_ctx(
        &self,
        tenant_id: Uuid,
        _principal_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<RunAuthorizationCtx>, sqlx::Error> {
        let row: Option<(Uuid, Option<String>)> = sqlx::query_as(
            "SELECT r.project_id, tm.role \
             FROM delivery.task_run r \
             LEFT JOIN core.tenant_member tm \
               ON tm.tenant_id = r.tenant_id AND tm.principal_id = $2 \
             WHERE r.id = $1 AND r.tenant_id = $2",
        )
        .bind(run_id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(project_id, role)| {
            let team_role = match role.as_deref() {
                Some("owner") => Some("owner"),
                Some("member") => Some("member"),
                _ => None,
            };
            RunAuthorizationCtx {
                project_id,
                team_role,
            }
        }))
    }

    async fn is_project_owner(
        &self,
        project_id: Uuid,
        principal_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        // 项目 owner 判定：core.project_member owner 行。
        let is_owner: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM core.project_member \
              WHERE project_id = $1 AND principal_id = $2 AND role = 'owner')",
        )
        .bind(project_id)
        .bind(principal_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(is_owner)
    }

    async fn list_runs(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        task_id: Option<Uuid>,
        run_kind: Option<&str>,
        page: Page,
    ) -> Result<(Vec<TaskRunRow>, i64), sqlx::Error> {
        let items: Vec<TaskRunRow> = sqlx::query_as(
            "SELECT r.id, r.task_id, t.title AS task_title, r.attempt_no, r.run_kind, r.status, \
                    r.outcome, r.agent_id, r.executor_kind, r.executor_session, r.lease_expires_at, \
                    r.created_at, r.started_at, r.finished_at, r.review_verdict, \
                    f.category AS failure_category, f.reason_code AS failure_reason \
             FROM delivery.task_run r \
             JOIN delivery.task t ON t.id = r.task_id \
             LEFT JOIN delivery.task_run_failure f ON f.run_id = r.id \
             WHERE r.tenant_id = $5 \
               AND EXISTS (SELECT 1 FROM core.project_member m \
                            WHERE m.project_id = r.project_id AND m.principal_id = $4) \
               AND ($1::uuid IS NULL OR r.project_id = $1) \
               AND ($6::uuid IS NULL OR r.task_id = $6) \
               AND ($7::text IS NULL OR r.run_kind = $7) \
             ORDER BY r.created_at DESC, r.attempt_no DESC LIMIT $2 OFFSET $3",
        )
        .bind(project_id)
        .bind(page.size)
        .bind(page.offset)
        .bind(principal_id)
        .bind(tenant_id)
        .bind(task_id)
        .bind(run_kind)
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM delivery.task_run r \
              WHERE r.tenant_id = $3 \
                AND EXISTS (SELECT 1 FROM core.project_member m \
                             WHERE m.project_id = r.project_id AND m.principal_id = $2) \
                AND ($1::uuid IS NULL OR r.project_id = $1) \
                AND ($4::uuid IS NULL OR r.task_id = $4) \
                AND ($5::text IS NULL OR r.run_kind = $5)",
        )
        .bind(project_id)
        .bind(principal_id)
        .bind(tenant_id)
        .bind(task_id)
        .bind(run_kind)
        .fetch_one(&self.pool)
        .await?;
        Ok((items, total))
    }

    async fn find_run_detail(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<TaskRunDetailRow>, sqlx::Error> {
        let run: Option<TaskRunDetailRow> = sqlx::query_as(
            "SELECT r.id, r.project_id, r.task_id, t.title AS task_title, r.attempt_no, r.run_kind, \
                    r.status, r.outcome, r.stage, r.agent_id, r.executor_kind, r.executor_session, \
                    r.lease_expires_at, r.claimed_at, r.heartbeat_at, r.last_agent_heartbeat_at, \
                    r.started_at, r.finished_at, r.deadline_at, r.release_reason, \
                    r.integration_status, r.review_verdict, r.created_at, \
                    f.category AS failure_category, f.reason_code AS failure_reason \
               FROM delivery.task_run r \
               JOIN delivery.task t ON t.id = r.task_id \
               LEFT JOIN delivery.task_run_failure f ON f.run_id = r.id \
              WHERE r.id = $1 AND r.tenant_id = $2 \
                AND EXISTS (SELECT 1 FROM core.project_member m \
                             WHERE m.project_id = r.project_id AND m.principal_id = $3)",
        )
        .bind(run_id)
        .bind(tenant_id)
        .bind(principal_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(run)
    }

    async fn run_repositories(&self, run_id: Uuid) -> Result<Vec<RunRepositoryRow>, sqlx::Error> {
        let rows: Vec<RunRepositoryRow> = sqlx::query_as(
            "SELECT trr.repository_id, pr.name AS repository_name, trr.base_ref, trr.base_commit, \
                    trr.write_access, trr.workspace_path \
               FROM delivery.task_run_repository trr \
               JOIN integration.project_repository pr \
                 ON pr.id = trr.repository_id AND pr.project_id = trr.project_id \
              WHERE trr.run_id = $1 \
              ORDER BY pr.name, trr.repository_id",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn run_visible(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let visible: Option<Uuid> = sqlx::query_scalar(
            "SELECT r.id FROM delivery.task_run r \
              WHERE r.id = $1 AND r.tenant_id = $2 \
                AND EXISTS (SELECT 1 FROM core.project_member m \
                             WHERE m.project_id = r.project_id AND m.principal_id = $3)",
        )
        .bind(run_id)
        .bind(tenant_id)
        .bind(principal_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(visible)
    }

    async fn list_run_events(
        &self,
        run_id: Uuid,
        event_type: Option<&str>,
        page: Page,
    ) -> Result<(Vec<RunEventRow>, i64), sqlx::Error> {
        let items: Vec<RunEventRow> = sqlx::query_as(
            "SELECT id, event_type, lease_generation, dedupe_tag, client_id, \
                    occurred_at, received_at, detail \
               FROM delivery.task_run_event \
              WHERE run_id = $1 AND ($2::text IS NULL OR event_type = $2) \
              ORDER BY occurred_at DESC, created_at DESC, id DESC LIMIT $3 OFFSET $4",
        )
        .bind(run_id)
        .bind(event_type)
        .bind(page.size)
        .bind(page.offset)
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM delivery.task_run_event \
              WHERE run_id = $1 AND ($2::text IS NULL OR event_type = $2)",
        )
        .bind(run_id)
        .bind(event_type)
        .fetch_one(&self.pool)
        .await?;
        Ok((items, total))
    }

    async fn run_summary(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<RunSummary, sqlx::Error> {
        let row: (i64, Value, Value, i64) = sqlx::query_as(
            "SELECT count(*), \
                    COALESCE((SELECT jsonb_object_agg(status, c) FROM ( \
                         SELECT status, count(*) AS c FROM delivery.task_run \
                          WHERE project_id = $1 AND tenant_id = $2 GROUP BY status) s), '{}'::jsonb), \
                    COALESCE((SELECT jsonb_object_agg(outcome, c) FROM ( \
                         SELECT outcome, count(*) AS c FROM delivery.task_run \
                          WHERE project_id = $1 AND tenant_id = $2 AND outcome IS NOT NULL \
                          GROUP BY outcome) s), '{}'::jsonb), \
                    count(*) FILTER (WHERE status IN ('queued', 'claimed', 'running')) \
               FROM delivery.task_run WHERE project_id = $1 AND tenant_id = $2",
        )
        .bind(project_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(RunSummary {
            total: row.0,
            by_status: row.1,
            by_outcome: row.2,
            active: row.3,
        })
    }

    async fn run_event_summary(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<RunEventSummary, sqlx::Error> {
        let ev: (i64, Option<DateTime<Utc>>) = sqlx::query_as(
            "SELECT count(*) FILTER (WHERE received_at > now() - interval '24 hours'), \
                    max(occurred_at) \
               FROM delivery.task_run_event WHERE project_id = $1 AND tenant_id = $2",
        )
        .bind(project_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(RunEventSummary {
            events_24h: ev.0,
            last_event_at: ev.1,
        })
    }
}
