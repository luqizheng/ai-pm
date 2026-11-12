//! [`AcceptanceReadRepo`] 的 Postgres 实现（任务书批 B 域 13：pm-api
//! `pm_acceptance.rs` / `pm_artifacts.rs` / `pm_reviews.rs` 的读 SQL 收口）。
//! SQL 原样迁移，行为不变。
use async_trait::async_trait;
use uuid::Uuid;

use pm_application::repo::acceptance_read::{
    ApprovalRow, AcceptanceReadRepo, ArtifactRow, CriterionVerdictRow, Page, ReviewFindingRow,
};

/// 无状态连接句柄（`PgPool` 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub struct PgAcceptanceReadRepo {
    pool: sqlx::PgPool,
}

impl PgAcceptanceReadRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AcceptanceReadRepo for PgAcceptanceReadRepo {
    async fn review_findings(
        &self,
        task_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<ReviewFindingRow>, sqlx::Error> {
        let rows: Vec<ReviewFindingRow> = sqlx::query_as(
            "SELECT f.id, f.review_run_id, r.attempt_no AS review_attempt_no, \
                    r.status AS review_run_status, f.criterion_id, f.finding_kind AS kind, \
                    f.severity, f.code_location, f.message, f.evidence_ref, f.created_at \
               FROM delivery.review_finding f \
               JOIN delivery.task_run r ON r.id = f.review_run_id \
              WHERE r.task_id = $1 AND r.project_id = $2 AND r.run_kind = 'review' \
              ORDER BY CASE f.severity WHEN 'blocker' THEN 0 WHEN 'warning' THEN 1 ELSE 2 END, \
                       r.attempt_no DESC, f.created_at DESC, f.id",
        )
        .bind(task_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn criterion_verdicts(
        &self,
        task_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<CriterionVerdictRow>, sqlx::Error> {
        let rows: Vec<CriterionVerdictRow> = sqlx::query_as(
            "SELECT c.id AS criterion_id, c.ordinal, c.text, c.verification_kind, \
                    (v.id IS NOT NULL) AS evaluated, \
                    v.review_run_id, v.review_attempt_no, v.verdict, v.evidence, \
                    v.decided_by, v.decided_at, v.created_at AS verdict_created_at \
               FROM delivery.task_acceptance_criterion c \
               LEFT JOIN LATERAL ( \
                SELECT v.id, v.review_run_id, r.attempt_no AS review_attempt_no, v.verdict, \
                       v.evidence, v.decided_by, v.decided_at, v.created_at \
                  FROM delivery.acceptance_criterion_verdict v \
                  JOIN delivery.task_run r ON r.id = v.review_run_id \
                 WHERE v.criterion_id = c.id \
                   AND r.run_kind = 'review' AND r.status = 'succeeded' \
                 ORDER BY r.attempt_no DESC, r.created_at DESC \
                 LIMIT 1 \
           ) v ON true \
              WHERE c.task_id = $1 AND c.project_id = $2 \
              ORDER BY c.ordinal",
        )
        .bind(task_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn task_approvals(
        &self,
        task_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<ApprovalRow>, sqlx::Error> {
        let rows: Vec<ApprovalRow> = sqlx::query_as(
            "SELECT a.id, a.task_id, a.review_run_id, a.delivery_run_id, a.kind, a.status, \
                    a.decided_by, a.decision_reason, a.decided_at, a.created_at, t.title AS task_title \
               FROM governance.review_approval a \
               JOIN delivery.task t ON t.id = a.task_id \
              WHERE a.task_id = $1 AND a.project_id = $2 \
              ORDER BY a.created_at DESC, a.id",
        )
        .bind(task_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn project_approvals(&self, project_id: Uuid) -> Result<Vec<ApprovalRow>, sqlx::Error> {
        let rows: Vec<ApprovalRow> = sqlx::query_as(
            "SELECT a.id, a.task_id, a.review_run_id, a.delivery_run_id, a.kind, a.status, \
                    a.decided_by, a.decision_reason, a.decided_at, a.created_at, t.title AS task_title \
               FROM governance.review_approval a \
               JOIN delivery.task t ON t.id = a.task_id \
              WHERE a.project_id = $1 \
              ORDER BY a.created_at DESC LIMIT 100",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn list_artifacts(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        project_id: Option<Uuid>,
        page: Page,
    ) -> Result<(Vec<ArtifactRow>, i64), sqlx::Error> {
        let items: Vec<ArtifactRow> = sqlx::query_as(
            "SELECT rv.id, rv.requirement_id, r.code AS requirement_code, \
                    r.title AS requirement_title, rv.version, rv.state, rv.content_hash, \
                    rv.created_at, rv.approved_by \
             FROM delivery.requirement_revision rv \
             JOIN delivery.requirement r ON r.id = rv.requirement_id \
             WHERE rv.tenant_id = $5 \
               AND EXISTS (SELECT 1 FROM core.project_member m \
                            WHERE m.project_id = rv.project_id AND m.principal_id = $4) \
               AND ($1::uuid IS NULL OR rv.project_id = $1) \
             ORDER BY rv.created_at DESC, rv.version DESC LIMIT $2 OFFSET $3",
        )
        .bind(project_id)
        .bind(page.size)
        .bind(page.offset)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM delivery.requirement_revision rv \
             WHERE rv.tenant_id = $3 \
               AND EXISTS (SELECT 1 FROM core.project_member m \
                            WHERE m.project_id = rv.project_id AND m.principal_id = $2) \
               AND ($1::uuid IS NULL OR rv.project_id = $1)",
        )
        .bind(project_id)
        .bind(principal_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok((items, total))
    }
}
