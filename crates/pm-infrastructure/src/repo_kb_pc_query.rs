//! [`KbPcQueryRepo`] 的 Postgres 实现（任务书批 B 域 7：pm-api
//! `knowledge_services.rs` 的直查 SQL 收口）。SQL 原样迁移，行为不变。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use pm_application::repo::kb_pc_query::{
    AnalysisTaskRow, KbPcQueryRepo, KbRevisionRow, ObjectLineageRow, PcObjectListRow, PcObjectRow,
    PcProposalHead, PcProposalListRow,
};

/// 无状态连接句柄（`PgPool` 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub struct PgKbPcQueryRepo {
    pool: sqlx::PgPool,
}

impl PgKbPcQueryRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl KbPcQueryRepo for PgKbPcQueryRepo {
    async fn analysis_task(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        target_service: &str,
        task_type: &str,
    ) -> Result<Option<AnalysisTaskRow>, sqlx::Error> {
        let row: Option<(String, Option<Value>, Option<String>, DateTime<Utc>, DateTime<Utc>, Uuid, String, String)> =
            sqlx::query_as(
                "SELECT t.task_status, t.result_ref, t.last_error, t.created_at, t.updated_at, \
                        i.id, i.intake_type, i.title \
                   FROM integration.intake_task t \
                   JOIN integration.intake_record i ON i.id = t.intake_id \
                  WHERE t.id = $1 AND t.project_id = $2 \
                    AND t.target_service = $3 AND t.task_type = $4",
            )
            .bind(task_id)
            .bind(project_id)
            .bind(target_service)
            .bind(task_type)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(
            |(task_status, result_ref, last_error, created_at, updated_at, intake_id, intake_type, title)| {
                AnalysisTaskRow {
                    task_status,
                    result_ref,
                    last_error,
                    created_at,
                    updated_at,
                    intake_id,
                    intake_type,
                    title,
                }
            },
        ))
    }

    async fn kb_revision(
        &self,
        project_id: Uuid,
        revision_id: Uuid,
    ) -> Result<Option<KbRevisionRow>, sqlx::Error> {
        let row: Option<(Uuid, Uuid, String, String, i32, String, Option<String>, String, Option<String>, DateTime<Utc>)> =
            sqlx::query_as(
                "SELECT r.id, d.id, d.doc_key, d.doc_type, r.revision_no, r.status, \
                        r.source_commit_sha, r.content_hash, r.change_summary, r.created_at \
                   FROM kb.document_revision r \
                   JOIN kb.document d ON d.id = r.document_id \
                  WHERE r.id = $1 AND d.project_id = $2",
            )
            .bind(revision_id)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(
            |(rev_id, doc_id, doc_key, doc_type, revision_no, revision_status, source_commit_sha, content_hash, change_summary, created_at)| {
                KbRevisionRow {
                    rev_id,
                    doc_id,
                    doc_key,
                    doc_type,
                    revision_no,
                    revision_status,
                    source_commit_sha,
                    content_hash,
                    change_summary,
                    created_at,
                }
            },
        ))
    }

    async fn kb_revision_content(&self, revision_id: Uuid) -> Result<Option<String>, sqlx::Error> {
        let content: Option<String> =
            sqlx::query_scalar("SELECT content FROM kb.document_revision WHERE id = $1")
                .bind(revision_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(content)
    }

    async fn pc_proposal_head(
        &self,
        project_id: Uuid,
        proposal_id: Uuid,
    ) -> Result<Option<PcProposalHead>, sqlx::Error> {
        let row: Option<(Option<Uuid>, String, String)> = sqlx::query_as(
            "SELECT source_id, status, governance_level FROM pc.proposal \
              WHERE tenant_id = (SELECT tenant_id FROM core.project WHERE id = $1) \
                AND project_id = $1 AND id = $2",
        )
        .bind(project_id)
        .bind(proposal_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(source_id, status, governance_level)| PcProposalHead {
            source_id,
            status,
            governance_level,
        }))
    }

    async fn pc_objects_of_source(
        &self,
        project_id: Uuid,
        source_id: Uuid,
    ) -> Result<Vec<PcObjectRow>, sqlx::Error> {
        let rows: Vec<PcObjectRow> = sqlx::query_as(
            "SELECT o.id, o.object_type, o.object_key, o.display_name, o.lifecycle_status, \
                    latest.revision_no, latest.change_summary, dr.content, latest.analysis_mode, \
                    vm.in_published_version AS versioned \
               FROM (SELECT DISTINCT cr.object_id \
                       FROM pc.evidence e \
                       JOIN pc.cognition_revision cr \
                         ON cr.tenant_id = e.tenant_id AND cr.project_id = e.project_id \
                        AND cr.id = e.object_revision_id \
                      WHERE e.tenant_id = (SELECT tenant_id FROM core.project WHERE id = $1) \
                        AND e.project_id = $1 AND e.source_id = $2) hit \
               JOIN pc.cognition_object o ON o.id = hit.object_id \
               LEFT JOIN LATERAL ( \
                    SELECT r2.id, r2.revision_no, r2.change_summary, r2.analysis_mode, \
                           r2.document_revision_id \
                      FROM pc.cognition_revision r2 \
                     WHERE r2.tenant_id = o.tenant_id AND r2.project_id = o.project_id \
                       AND r2.object_id = o.id \
                     ORDER BY r2.revision_no DESC LIMIT 1 \
               ) latest ON true \
               LEFT JOIN pc.document_revision dr ON dr.id = latest.document_revision_id \
               LEFT JOIN pc.v_revision_versioned vm ON vm.object_revision_id = latest.id \
              ORDER BY o.created_at ASC, o.id ASC",
        )
        .bind(project_id)
        .bind(source_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn pc_object_legacy(
        &self,
        project_id: Uuid,
        object_id: Uuid,
    ) -> Result<Vec<PcObjectRow>, sqlx::Error> {
        let rows: Vec<PcObjectRow> = sqlx::query_as(
            "SELECT o.id, o.object_type, o.object_key, o.display_name, o.lifecycle_status, \
                    rev.revision_no, rev.change_summary, dr.content, rev.analysis_mode, \
                    vm.in_published_version AS versioned \
               FROM pc.cognition_object o \
               LEFT JOIN LATERAL ( \
                    SELECT r2.revision_no, r2.change_summary, r2.analysis_mode, \
                           r2.document_revision_id, r2.id AS rid \
                      FROM pc.cognition_revision r2 \
                     WHERE r2.tenant_id = o.tenant_id AND r2.project_id = o.project_id \
                       AND r2.object_id = o.id \
                     ORDER BY r2.revision_no DESC LIMIT 1 \
               ) rev ON true \
               LEFT JOIN pc.document_revision dr ON dr.id = rev.document_revision_id \
               LEFT JOIN pc.v_revision_versioned vm ON vm.object_revision_id = rev.rid \
              WHERE o.tenant_id = (SELECT tenant_id FROM core.project WHERE id = $1) \
                AND o.project_id = $1 AND o.id = $2 \
              ORDER BY o.created_at ASC, o.id ASC",
        )
        .bind(project_id)
        .bind(object_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn list_pc_objects(&self, project_id: Uuid) -> Result<Vec<PcObjectListRow>, sqlx::Error> {
        let rows: Vec<PcObjectListRow> = sqlx::query_as(
            "SELECT o.id, o.object_type, o.object_key, o.display_name, o.lifecycle_status, \
                    o.updated_at, rev.revision_no, rev.change_summary, \
                    left(dr.content, 240) AS excerpt \
               FROM pc.cognition_object o \
               LEFT JOIN LATERAL ( \
                    SELECT r2.revision_no, r2.change_summary, r2.document_revision_id \
                      FROM pc.cognition_revision r2 \
                     WHERE r2.tenant_id = o.tenant_id AND r2.project_id = o.project_id \
                       AND r2.object_id = o.id \
                     ORDER BY r2.revision_no DESC LIMIT 1 \
               ) rev ON true \
               LEFT JOIN pc.document_revision dr ON dr.id = rev.document_revision_id \
              WHERE o.tenant_id = (SELECT tenant_id FROM core.project WHERE id = $1) \
                AND o.project_id = $1 \
              ORDER BY o.updated_at DESC, o.display_name ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn list_pc_proposals(
        &self,
        project_id: Uuid,
        status_filter: Option<&str>,
    ) -> Result<Vec<PcProposalListRow>, sqlx::Error> {
        let rows: Vec<PcProposalListRow> = sqlx::query_as(
            "SELECT p.id, p.title, p.status, p.risk_level, p.confidence, p.origin_kind, \
                    p.expected_version_no, p.proposal_content_hash, p.submitted_at, p.created_at, \
                    (SELECT count(*) FROM pc.proposal_member m \
                      WHERE m.tenant_id = p.tenant_id AND m.project_id = p.project_id \
                        AND m.proposal_id = p.id) AS member_count \
               FROM pc.proposal p \
              WHERE p.tenant_id = (SELECT tenant_id FROM core.project WHERE id = $1) \
                AND p.project_id = $1 \
                AND ($2::text IS NULL OR p.status = $2) \
              ORDER BY p.created_at DESC",
        )
        .bind(project_id)
        .bind(status_filter)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn list_projc_objects(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<PcObjectListRow>, sqlx::Error> {
        let rows: Vec<PcObjectListRow> = sqlx::query_as(
            "SELECT o.id, o.object_type, o.object_key, o.display_name, o.lifecycle_status, \
                    o.updated_at, rev.revision_no, rev.change_summary, \
                    left(dr.content, 240) AS excerpt \
               FROM projc.cognition_object o \
               LEFT JOIN LATERAL ( \
                    SELECT r2.revision_no, r2.change_summary, r2.document_revision_id \
                      FROM projc.cognition_revision r2 \
                     WHERE r2.tenant_id = o.tenant_id AND r2.project_id = o.project_id \
                       AND r2.object_id = o.id \
                     ORDER BY r2.revision_no DESC LIMIT 1 \
               ) rev ON true \
               LEFT JOIN projc.document_revision dr ON dr.id = rev.document_revision_id \
              WHERE o.tenant_id = (SELECT tenant_id FROM core.project WHERE id = $1) \
                AND o.project_id = $1 \
              ORDER BY o.updated_at DESC, o.display_name ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn projc_object_legacy(
        &self,
        project_id: Uuid,
        object_id: Uuid,
    ) -> Result<Vec<PcObjectRow>, sqlx::Error> {
        let rows: Vec<PcObjectRow> = sqlx::query_as(
            "SELECT o.id, o.object_type, o.object_key, o.display_name, o.lifecycle_status, \
                    rev.revision_no, rev.change_summary, dr.content, rev.analysis_mode, \
                    versioned.versioned \
               FROM projc.cognition_object o \
               LEFT JOIN LATERAL ( \
                    SELECT r2.revision_no, r2.change_summary, r2.analysis_mode, \
                           r2.document_revision_id, r2.id AS rid \
                      FROM projc.cognition_revision r2 \
                     WHERE r2.tenant_id = o.tenant_id AND r2.project_id = o.project_id \
                       AND r2.object_id = o.id \
                     ORDER BY r2.revision_no DESC LIMIT 1 \
               ) rev ON true \
               LEFT JOIN projc.document_revision dr ON dr.id = rev.document_revision_id \
               LEFT JOIN LATERAL ( \
                    SELECT EXISTS ( \
                        SELECT 1 FROM projc.version_member vm \
                          JOIN projc.cognition_version cv \
                            ON cv.tenant_id = vm.tenant_id \
                           AND cv.project_id = vm.project_id \
                           AND cv.id = vm.version_id \
                         WHERE vm.object_revision_id = rev.rid AND cv.status = 'published' \
                    ) AS versioned \
               ) versioned ON true \
              WHERE o.tenant_id = (SELECT tenant_id FROM core.project WHERE id = $1) \
                AND o.project_id = $1 AND o.id = $2 \
              ORDER BY o.created_at ASC, o.id ASC",
        )
        .bind(project_id)
        .bind(object_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn pc_object_lineage(
        &self,
        project_id: Uuid,
        object_id: Uuid,
    ) -> Result<Vec<ObjectLineageRow>, sqlx::Error> {
        self.object_lineage(
            project_id,
            object_id,
            "pc",
            "pc_source_id",
        )
        .await
    }

    async fn projc_object_lineage(
        &self,
        project_id: Uuid,
        object_id: Uuid,
    ) -> Result<Vec<ObjectLineageRow>, sqlx::Error> {
        self.object_lineage(
            project_id,
            object_id,
            "projc",
            "projc_source_id",
        )
        .await
    }
}

impl PgKbPcQueryRepo {
    /// 双域共用的对象 → 来源报告追溯（最近 3 条已发布版本；report 侧 LEFT JOIN，
    /// 非报告通道发布的版本 report 字段为 NULL）。`schema` ∈ {pc, projc}，
    /// `report_source_col` = 报告表上的来源列名（pc_source_id / projc_source_id）。
    ///
    /// 列名/表名由调用方字面量传入（两域枚举，非用户输入，无注入面）。
    async fn object_lineage(
        &self,
        project_id: Uuid,
        object_id: Uuid,
        schema: &str,
        report_source_col: &str,
    ) -> Result<Vec<ObjectLineageRow>, sqlx::Error> {
        let sql = format!(
            "SELECT r.id AS report_id, pr.name AS repository_name, r.commit_sha, \
                    cv.version_no, cv.confirmed_at \
               FROM {schema}.version_member vm \
               JOIN {schema}.cognition_version cv \
                 ON cv.tenant_id = vm.tenant_id AND cv.project_id = vm.project_id \
                AND cv.id = vm.version_id \
               JOIN {schema}.proposal pp \
                 ON pp.tenant_id = cv.tenant_id AND pp.project_id = cv.project_id \
                AND pp.id = cv.source_proposal_id \
               LEFT JOIN integration.repo_analysis_report r ON r.{report_source_col} = pp.source_id \
               LEFT JOIN integration.project_repository pr ON pr.id = r.repository_id \
              WHERE vm.tenant_id = (SELECT tenant_id FROM core.project WHERE id = $1) \
                AND vm.project_id = $1 AND vm.object_id = $2 AND cv.status = 'published' \
              ORDER BY cv.version_no DESC LIMIT 3"
        );
        let rows: Vec<ObjectLineageRow> = sqlx::query_as(&sql)
            .bind(project_id)
            .bind(object_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }
}
