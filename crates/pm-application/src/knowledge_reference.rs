//! 知识空间 V2 证据引用：决策结论引用了哪篇文档、哪个章节、哪个版本。

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::knowledge::KnowledgeError;

#[derive(Debug, Clone)]
pub struct AddReferenceRequest {
    pub project_id: Uuid,
    pub document_id: Uuid,
    pub document_revision_id: Uuid,
    pub section_anchor: Option<String>,
    pub section_title: Option<String>,
    pub start_line: Option<i32>,
    pub reference_kind: String,
    pub source_kind: String,
    pub source_id: Option<Uuid>,
    pub created_by: Uuid,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReferenceRow {
    pub id: Uuid,
    pub document_id: Uuid,
    pub document_revision_id: Uuid,
    pub section_anchor: Option<String>,
    pub section_title: Option<String>,
    pub start_line: Option<i32>,
    pub reference_kind: String,
    pub source_kind: String,
    pub source_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 幂等写入一条引用：同一（修订、章节、来源）重复写入复用已有行。
pub async fn add_reference(
    pool: &PgPool,
    req: AddReferenceRequest,
) -> Result<Uuid, KnowledgeError> {
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM ai.knowledge_reference \
          WHERE document_revision_id = $1 \
            AND coalesce(section_anchor, '') = coalesce($2, '') \
            AND source_kind = $3 \
            AND coalesce(source_id, '00000000-0000-0000-0000-000000000000'::uuid) \
                = coalesce($4, '00000000-0000-0000-0000-000000000000'::uuid) \
          LIMIT 1",
    )
    .bind(req.document_revision_id)
    .bind(&req.section_anchor)
    .bind(&req.source_kind)
    .bind(req.source_id)
    .fetch_optional(pool)
    .await?;
    if let Some(id) = existing {
        return Ok(id);
    }

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.knowledge_reference \
         (project_id, document_id, document_revision_id, section_anchor, section_title, \
          start_line, reference_kind, source_kind, source_id, created_by) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING id",
    )
    .bind(req.project_id)
    .bind(req.document_id)
    .bind(req.document_revision_id)
    .bind(&req.section_anchor)
    .bind(&req.section_title)
    .bind(req.start_line)
    .bind(&req.reference_kind)
    .bind(&req.source_kind)
    .bind(req.source_id)
    .bind(req.created_by)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn list_references(
    pool: &PgPool,
    project_id: Uuid,
    document_id: Uuid,
) -> Result<Vec<ReferenceRow>, KnowledgeError> {
    let rows = sqlx::query(
        "SELECT id, document_id, document_revision_id, section_anchor, section_title, \
                start_line, reference_kind, source_kind, source_id, created_at \
           FROM ai.knowledge_reference \
          WHERE project_id = $1 AND document_id = $2 \
          ORDER BY created_at DESC",
    )
    .bind(project_id)
    .bind(document_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ReferenceRow {
            id: sqlx::Row::get(&row, "id"),
            document_id: sqlx::Row::get(&row, "document_id"),
            document_revision_id: sqlx::Row::get(&row, "document_revision_id"),
            section_anchor: sqlx::Row::get(&row, "section_anchor"),
            section_title: sqlx::Row::get(&row, "section_title"),
            start_line: sqlx::Row::get(&row, "start_line"),
            reference_kind: sqlx::Row::get(&row, "reference_kind"),
            source_kind: sqlx::Row::get(&row, "source_kind"),
            source_id: sqlx::Row::get(&row, "source_id"),
            created_at: sqlx::Row::get(&row, "created_at"),
        })
        .collect())
}