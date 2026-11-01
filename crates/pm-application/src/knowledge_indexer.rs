//! 知识空间 V2 索引层（设计稿 §九 路径 A）：可替换的 `KnowledgeIndexer` 抽象 +
//! 自建 Markdown 树索引。索引是**派生**数据：内容哈希变化即视为过期，检索回退
//! 直读 Git 权威文件，绝不把旧索引当事实。

use async_trait::async_trait;
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::knowledge::{self, KnowledgeError, KnowledgeRoot};
use crate::knowledge_index;

pub const MARKDOWN_TREE_PROVIDER: &str = "markdown-tree";
pub const MARKDOWN_TREE_INDEX_VERSION: &str = "v2-a";

#[derive(Debug, Clone, Serialize)]
pub struct IndexReference {
    pub document_id: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RetrievedSection {
    pub document_path: String,
    pub title: String,
    pub content: String,
    pub revision_hash: String,
    pub locator: serde_json::Value,
}

/// 可替换的检索适配层；A 路径实现见 [`MarkdownTreeIndexer`]。
#[async_trait]
pub trait KnowledgeIndexer: Send + Sync {
    fn provider(&self) -> &'static str;

    async fn index_document(
        &self,
        path: &str,
        content_hash: &str,
    ) -> Result<IndexReference, KnowledgeError>;

    async fn search(
        &self,
        project_id: &str,
        question: &str,
    ) -> Result<Vec<RetrievedSection>, KnowledgeError>;
}

/// A 路径：直接解析 Markdown 标题树，检索时校验内容哈希，漂移则报 `Stale`。
pub struct MarkdownTreeIndexer {
    pool: PgPool,
    root: KnowledgeRoot,
}

impl MarkdownTreeIndexer {
    pub fn new(pool: PgPool, root: KnowledgeRoot) -> Self {
        Self { pool, root }
    }
}

#[async_trait]
impl KnowledgeIndexer for MarkdownTreeIndexer {
    fn provider(&self) -> &'static str {
        MARKDOWN_TREE_PROVIDER
    }

    async fn index_document(
        &self,
        path: &str,
        content_hash: &str,
    ) -> Result<IndexReference, KnowledgeError> {
        let abs = knowledge::safe_join(self.root.path(), path)?;
        let markdown = std::fs::read_to_string(&abs)
            .map_err(|err| KnowledgeError::Io(format!("{}: {err}", abs.display())))?;
        if knowledge::content_hash(&markdown) != content_hash {
            return Err(KnowledgeError::Stale);
        }
        let sections = knowledge_index::parse_markdown_tree(path, &markdown);
        if !sections.iter().any(|section| section.level > 0) {
            return Err(KnowledgeError::Conflict(
                "document has no Markdown heading to index".to_string(),
            ));
        }
        Ok(IndexReference {
            document_id: path.to_string(),
            content_hash: content_hash.to_string(),
        })
    }

    async fn search(
        &self,
        project_id: &str,
        question: &str,
    ) -> Result<Vec<RetrievedSection>, KnowledgeError> {
        let pid = Uuid::parse_str(project_id).map_err(|_| {
            KnowledgeError::Config(format!("invalid project id `{project_id}`"))
        })?;
        let hits = knowledge_index::search_documents(&self.pool, &self.root, pid, question).await?;
        Ok(hits
            .into_iter()
            .map(|hit| RetrievedSection {
                document_path: hit.section.canonical_path.clone(),
                title: hit.section.title.clone(),
                content: hit.section.content.clone(),
                revision_hash: String::new(),
                locator: serde_json::json!({
                    "document_id": hit.section.document_id,
                    "anchor": hit.section.anchor,
                    "start_line": hit.section.start_line,
                    "score": hit.score,
                }),
            })
            .collect())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexStateRow {
    pub document_revision_id: Uuid,
    pub provider: String,
    pub status: String,
    pub indexed_content_hash: Option<String>,
    pub revision_content_hash: Option<String>,
    pub indexed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FallbackSearchResult {
    pub used_fallback: bool,
    pub stale_revision_ids: Vec<Uuid>,
    pub sections: Vec<RetrievedSection>,
}

#[allow(clippy::too_many_arguments)]
async fn upsert_state(
    pool: &PgPool,
    project_id: Uuid,
    revision_id: Uuid,
    provider: &str,
    status: &str,
    indexed_hash: Option<&str>,
    index_version: Option<&str>,
    last_error: Option<&str>,
) -> Result<(), sqlx::Error> {
    let indexed_at = if status == "indexed" {
        Some(chrono::Utc::now())
    } else {
        None
    };
    sqlx::query(
        "INSERT INTO ai.document_index_state \
         (project_id, document_revision_id, provider, indexed_content_hash, index_version, \
          status, last_error, indexed_at, updated_at, tenant_id) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,now(), \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) \
         ON CONFLICT (document_revision_id, provider) DO UPDATE SET \
            status = EXCLUDED.status, \
            indexed_content_hash = EXCLUDED.indexed_content_hash, \
            index_version = EXCLUDED.index_version, \
            last_error = EXCLUDED.last_error, \
            indexed_at = EXCLUDED.indexed_at, \
            updated_at = now()",
    )
    .bind(project_id)
    .bind(revision_id)
    .bind(provider)
    .bind(indexed_hash)
    .bind(index_version)
    .bind(status)
    .bind(last_error)
    .bind(indexed_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// 重建一条修订的索引状态；哈希漂移 → `stale`，解析失败 → `failed`。
pub async fn index_revision(
    pool: &PgPool,
    indexer: &dyn KnowledgeIndexer,
    project_id: Uuid,
    revision_id: Uuid,
) -> Result<(), KnowledgeError> {
    let row = sqlx::query(
        "SELECT d.canonical_path, r.content_hash \
           FROM ai.document_revision r \
           JOIN ai.project_document d ON d.id = r.document_id \
          WHERE r.id = $1 AND r.project_id = $2",
    )
    .bind(revision_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or(KnowledgeError::NotFound)?;
    let path: String = row.get("canonical_path");
    let content_hash: String = row.get("content_hash");

    match indexer.index_document(&path, &content_hash).await {
        Ok(reference) => {
            upsert_state(
                pool,
                project_id,
                revision_id,
                indexer.provider(),
                "indexed",
                Some(&reference.content_hash),
                Some(MARKDOWN_TREE_INDEX_VERSION),
                None,
            )
            .await?;
            Ok(())
        }
        Err(KnowledgeError::Stale) => {
            upsert_state(
                pool,
                project_id,
                revision_id,
                indexer.provider(),
                "stale",
                None,
                Some(MARKDOWN_TREE_INDEX_VERSION),
                Some("document content hash drifted from the revision"),
            )
            .await?;
            Err(KnowledgeError::Stale)
        }
        Err(err) => {
            upsert_state(
                pool,
                project_id,
                revision_id,
                indexer.provider(),
                "failed",
                None,
                Some(MARKDOWN_TREE_INDEX_VERSION),
                Some(&err.to_string()),
            )
            .await?;
            Err(err)
        }
    }
}

/// 检索入口：先校验每个文档最新修订的索引哈希；不一致即标记 `stale` 并回退直读
/// Git 权威文件（`MarkdownTreeIndexer::search` 本身就是直读，不做缓存事实）。
pub async fn search_with_fallback(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    query: &str,
) -> Result<FallbackSearchResult, KnowledgeError> {
    let rows = sqlx::query(
        "SELECT r.id AS revision_id, r.content_hash, s.status, s.indexed_content_hash \
           FROM ai.project_document d \
           JOIN LATERAL ( \
               SELECT id, content_hash FROM ai.document_revision \
                WHERE document_id = d.id AND status = 'published' \
                ORDER BY revision_no DESC LIMIT 1 \
           ) r ON true \
           LEFT JOIN ai.document_index_state s \
             ON s.document_revision_id = r.id AND s.provider = $2 \
          WHERE d.project_id = $1",
    )
    .bind(project_id)
    .bind(MARKDOWN_TREE_PROVIDER)
    .fetch_all(pool)
    .await?;

    let mut stale = Vec::new();
    for row in &rows {
        let revision_id: Uuid = row.get("revision_id");
        let content_hash: String = row.get("content_hash");
        let status: Option<String> = row.get("status");
        let indexed_hash: Option<String> = row.get("indexed_content_hash");
        let is_indexed = status.as_deref() == Some("indexed")
            && indexed_hash.as_deref() == Some(content_hash.as_str());
        if !is_indexed {
            stale.push(revision_id);
            upsert_state(
                pool,
                project_id,
                revision_id,
                MARKDOWN_TREE_PROVIDER,
                "stale",
                indexed_hash.as_deref(),
                Some(MARKDOWN_TREE_INDEX_VERSION),
                Some("index hash does not match the current revision"),
            )
            .await?;
        }
    }

    let indexer = MarkdownTreeIndexer::new(pool.clone(), root.clone());
    let sections = indexer.search(&project_id.to_string(), query).await?;
    Ok(FallbackSearchResult {
        used_fallback: !stale.is_empty(),
        stale_revision_ids: stale,
        sections,
    })
}

pub async fn index_states(
    pool: &PgPool,
    project_id: Uuid,
    document_id: Uuid,
) -> Result<Vec<IndexStateRow>, KnowledgeError> {
    let rows = sqlx::query(
        "SELECT s.document_revision_id, s.provider, s.status, s.indexed_content_hash, \
                r.content_hash AS revision_content_hash, s.indexed_at, s.last_error \
           FROM ai.document_index_state s \
           JOIN ai.document_revision r ON r.id = s.document_revision_id \
          WHERE s.project_id = $1 AND r.document_id = $2 \
          ORDER BY r.revision_no DESC, s.provider",
    )
    .bind(project_id)
    .bind(document_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| IndexStateRow {
            document_revision_id: row.get("document_revision_id"),
            provider: row.get("provider"),
            status: row.get("status"),
            indexed_content_hash: row.get("indexed_content_hash"),
            revision_content_hash: row.get("revision_content_hash"),
            indexed_at: row.get("indexed_at"),
            last_error: row.get("last_error"),
        })
        .collect())
}