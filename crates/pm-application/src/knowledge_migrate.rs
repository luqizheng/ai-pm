//! 知识空间 V2 需求正文迁移：把 `delivery.requirement_revision.content`（JSONB）
//! 镜像成 Markdown 知识文档。
//!
//! 兼容与回滚边界：PG 原行**不删除、不改写**，Markdown 是新增的权威副本；读取
//! 旧路径（产物页 / requirement_revision）保持不变。回滚只删除迁移生成的文档，
//! 对 PG 无副作用。

use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::knowledge::{self, KnowledgeError, KnowledgeRoot};
use crate::knowledge_proposal::{self, ProposalOutcome, ProposalRequest};

/// 把一条需求修订镜像为 Markdown 文档（doc_key = 需求 code）。
pub async fn migrate_requirement_revision(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    requirement_revision_id: Uuid,
    created_by: Uuid,
) -> Result<ProposalOutcome, KnowledgeError> {
    let row = sqlx::query(
        "SELECT req.code, req.title, rev.content, rev.version \
           FROM delivery.requirement_revision rev \
           JOIN delivery.requirement req ON req.id = rev.requirement_id \
          WHERE rev.id = $1 AND rev.project_id = $2",
    )
    .bind(requirement_revision_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or(KnowledgeError::NotFound)?;

    let code: String = row.get("code");
    let title: String = row.get("title");
    let content: Value = row.get("content");
    let version: i32 = row.get("version");
    let markdown = render_requirement(&code, &title, requirement_revision_id, &content);

    knowledge_proposal::submit_proposal(
        pool,
        root,
        ProposalRequest {
            project_id,
            doc_key: code,
            doc_type: "requirement".to_string(),
            markdown,
            change_summary: Some(format!(
                "migrated from requirement_revision {requirement_revision_id} (v{version})"
            )),
            source_event_id: None,
            analysis_run_id: None,
            source_ref: Some(format!("requirement_revision:{requirement_revision_id}")),
            redline_kind: None,
            base_revision_id: None,
            operation_id: format!("migrate_requirement:{requirement_revision_id}"),
            created_by,
        },
    )
    .await
}

/// 回滚一次需求迁移：仅当该文档的修订全部来自迁移时删除（Git 文件 + 两表行），
/// PG 需求修订保持不变。
pub async fn rollback_requirement_migration(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    document_id: Uuid,
) -> Result<(), KnowledgeError> {
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.document_revision \
          WHERE document_id = $1 AND project_id = $2",
    )
    .bind(document_id)
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    let migrated: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.document_revision \
          WHERE document_id = $1 AND project_id = $2 \
            AND change_summary LIKE 'migrated from requirement_revision%'",
    )
    .bind(document_id)
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    if total == 0 || total != migrated {
        return Err(KnowledgeError::Conflict(
            "document has non-migration revisions; refusing to delete it".to_string(),
        ));
    }
    knowledge::delete_document(pool, root, project_id, document_id).await
}

fn render_requirement(code: &str, title: &str, revision_id: Uuid, content: &Value) -> String {
    let pretty = serde_json::to_string_pretty(content).unwrap_or_default();
    format!(
        "---\nid: {code}\ntype: requirement\nsource_events:\n  - requirement_revision:{revision_id}\n---\n\n# {title}\n\n## 结构化内容\n\n```json\n{pretty}\n```\n"
    )
}
