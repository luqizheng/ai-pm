use super::*;
use serde::Serialize;use sqlx::{PgPool, Postgres, Row, Transaction};use uuid::Uuid;use crate::knowledge::{self, KnowledgeError, KnowledgeRoot};use crate::knowledge_quality::{self, GateFailure, GateInput, QualityGateResult};

/// 列出项目（或某文档）的提案，最新在前。
pub async fn list_proposals(
    pool: &PgPool,
    project_id: Uuid,
    document_id: Option<Uuid>,
) -> Result<Vec<ProposalRow>, KnowledgeError> {
    let rows = sqlx::query(
        "SELECT id, doc_key, doc_type, status, quality_gate_result, redline_kind, \
                change_summary, resulting_revision_id, created_at \
           FROM ai.document_proposal \
          WHERE project_id = $1 AND ($2::uuid IS NULL OR document_id = $2) \
          ORDER BY created_at DESC",
    )
    .bind(project_id)
    .bind(document_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ProposalRow {
            id: row.get("id"),
            doc_key: row.get("doc_key"),
            doc_type: row.get("doc_type"),
            status: row.get("status"),
            quality_gate_result: row.get("quality_gate_result"),
            redline_kind: row.get("redline_kind"),
            change_summary: row.get("change_summary"),
            resulting_revision_id: row.get("resulting_revision_id"),
            created_at: row.get("created_at"),
        })
        .collect())
}


