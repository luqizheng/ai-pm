//! 知识检索编排服务（T-060 / P6 后半，设计稿 §3.2、§6）。
//!
//! 职责（**只管怎么读**，不管谁能写）：
//!
//! 1. 取每篇文档最新 published 修订 + 该 provider 的索引状态；
//! 2. 按四要素（provider / 状态 / 版本 / 哈希）判定走 DB 路径还是回退直读；
//! 3. DB 路径调 `KnowledgeIndexer::search`；回退路径调 `DirectReadFallback`；
//! 4. 合并去重（DB 优先）→ 全局统一排序 → offset 分页；
//! 5. 故障分级（单篇失效 / 索引层整体不可用 / 权威元数据不可用）。
//!
//! **空结果与检索失败是两种业务含义**：权威元数据不可用时必须报错，
//! 绝不返回 `200 {items: []}`。

use std::sync::Arc;

use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::knowledge::{KnowledgeError, KnowledgeRoot};
use crate::knowledge_index::{self, SearchHit};
use crate::knowledge_indexer::{
    DB_SECTION_INDEX_VERSION, DB_SECTION_PROVIDER, DbSectionIndexer, KnowledgeIndexer, SearchPage,
};

/// 检索结果（handler 只序列化 `hits`；其余字段是内部可观测信息）。
#[derive(Debug, Clone, Serialize)]
pub struct FallbackSearchResult {
    /// 是否走了回退直读。
    pub used_fallback: bool,
    /// 因索引失效而回退的修订。
    pub stale_revision_ids: Vec<Uuid>,
    /// 命中的章节。
    pub hits: Vec<SearchHit>,
}

/// 回退原因（写入日志；设计稿 §6.5 判定原则 4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackReason {
    IndexStateMissing,
    IndexStale,
    IndexFailed,
    IndexVersionMismatch,
    IndexHashMismatch,
    IndexInfraError,
}

impl FallbackReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            FallbackReason::IndexStateMissing => "index_state_missing",
            FallbackReason::IndexStale => "index_stale",
            FallbackReason::IndexFailed => "index_failed",
            FallbackReason::IndexVersionMismatch => "index_version_mismatch",
            FallbackReason::IndexHashMismatch => "index_hash_mismatch",
            FallbackReason::IndexInfraError => "index_infra_error",
        }
    }
}

/// 一篇文档当前生效修订的索引可用性。
struct DocState {
    revision_id: Uuid,
    /// 不满足四要素时回退直读该篇（`None` = 走 DB 路径）。
    fallback: Option<FallbackReason>,
}

/// 知识检索应用服务。
pub struct KnowledgeSearchService {
    pool: PgPool,
    root: KnowledgeRoot,
    /// provider 端口（可替换）；默认是 DB 章节索引实现。
    indexer: Arc<dyn KnowledgeIndexer>,
}

impl KnowledgeSearchService {
    pub fn new(pool: PgPool, root: KnowledgeRoot) -> Self {
        let indexer = Arc::new(DbSectionIndexer::new(pool.clone(), root.clone()));
        Self { pool, root, indexer }
    }

    /// 注入自定义 provider 实现——端口可替换性的显式入口（测试用故障注入）。
    pub fn with_indexer(
        pool: PgPool,
        root: KnowledgeRoot,
        indexer: Arc<dyn KnowledgeIndexer>,
    ) -> Self {
        Self { pool, root, indexer }
    }

    /// 检索入口。
    ///
    /// - `tenant_id` 显式参与查询（评审项 5）：不靠「UUID 不易猜测」当隔离。
    /// - 分页：先合并去重、再排序、最后分页（§6.4 第 5 步）。
    pub async fn search(
        &self,
        project_id: Uuid,
        tenant_id: Uuid,
        query: &str,
        page: SearchPage,
    ) -> Result<FallbackSearchResult, KnowledgeError> {
        // 1) 权威元数据：文档清单 + 最新 published 修订 + 索引状态。
        //    这一步失败即「权威元数据不可用」→ 503，绝不返回空结果。
        let states = self.load_doc_states(project_id, tenant_id).await?;
        // 2) 分类：满足四要素 → DB 路径；否则 → 回退候选。
        let mut stale_revision_ids = Vec::new();
        for state in &states {
            match state.fallback {
                None => {}
                Some(reason) => {
                    tracing::info!(
                        project_id = %project_id,
                        revision_id = %state.revision_id,
                        reason = reason.as_str(),
                        "knowledge_index_fallback"
                    );
                    stale_revision_ids.push(state.revision_id);
                }
            }
        }

        // 3) DB 路径。索引层整体不可用（查询失败）→ 整项目回退直读，
        //    但**不返回部分成功结果**冒充完整结果（评审 P1-2 原则 1）。
        let fetch = page.limit + page.offset;
        let db_hits = match self.indexer.search(project_id, tenant_id, query, page).await {
            Ok(hits) => hits,
            Err(err) => {
                tracing::warn!(
                    project_id = %project_id,
                    error = %err,
                    reason = FallbackReason::IndexInfraError.as_str(),
                    "knowledge_search_fallback"
                );
                let hits = knowledge_index::scan_documents_direct(
                    &self.pool,
                    &self.root,
                    project_id,
                    query,
                    fetch,
                )
                .await
                .map_err(|err| {
                    // 回退直读也读不到权威正文 → 明确报错（503），不返回空结果。
                    KnowledgeError::IndexUnavailable(format!(
                        "direct read fallback failed: {err}"
                    ))
                })?;
                return Ok(FallbackSearchResult {
                    used_fallback: true,
                    stale_revision_ids,
                    hits: paginate(hits, page),
                });
            }
        };

        // 4) 回退候选：逐篇读权威文件（哈希校验失败 → 该篇作废并标 stale）。
        let fallback_hits = if stale_revision_ids.is_empty() {
            Vec::new()
        } else {
            self.scan_fallback_documents(project_id, query, fetch, &stale_revision_ids)
                .await?
        };

        // 5) 合并去重（DB 优先）→ 统一排序 → 跳过 offset → 取 limit。
        let merged = merge_hits(db_hits, fallback_hits);
        Ok(FallbackSearchResult {
            used_fallback: !stale_revision_ids.is_empty(),
            stale_revision_ids,
            hits: paginate(merged, page),
        })
    }

    /// 取每篇文档最新 published 修订 + 索引状态，判定四要素。
    ///
    /// 权威元数据不可读 → `IndexUnavailable`（503），**绝不**退化成空结果。
    async fn load_doc_states(
        &self,
        project_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Vec<DocState>, KnowledgeError> {
        let rows = sqlx::query(
            "SELECT d.id AS document_id, r.id AS revision_id, r.content_hash, \
                    st.status, st.indexed_content_hash, st.index_version \
               FROM ai.project_document d \
               JOIN LATERAL ( \
                   SELECT id, content_hash FROM ai.document_revision \
                    WHERE document_id = d.id AND status = 'published' \
                    ORDER BY revision_no DESC LIMIT 1 \
               ) r ON true \
               LEFT JOIN ai.document_index_state st \
                 ON st.document_revision_id = r.id AND st.provider = $3 \
              WHERE d.project_id = $1 AND d.tenant_id = $2 \
              ORDER BY d.doc_type, d.doc_key",
        )
        .bind(project_id)
        .bind(tenant_id)
        .bind(DB_SECTION_PROVIDER)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| {
            // 权威元数据不可读 → 明确报错（503），不退化成空结果（§6.5）。
            KnowledgeError::IndexUnavailable(format!(
                "cannot read the authoritative document list: {err}"
            ))
        })?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let revision_id: Uuid = row.get("revision_id");
                let content_hash: String = row.get("content_hash");
                let status: Option<String> = row.get("status");
                let indexed_hash: Option<String> = row.get("indexed_content_hash");
                let index_version: Option<String> = row.get("index_version");
                let fallback = match status.as_deref() {
                    None => Some(FallbackReason::IndexStateMissing),
                    Some("stale") => Some(FallbackReason::IndexStale),
                    Some("failed") => Some(FallbackReason::IndexFailed),
                    Some("indexed") => {
                        if index_version.as_deref() != Some(DB_SECTION_INDEX_VERSION) {
                            Some(FallbackReason::IndexVersionMismatch)
                        } else if indexed_hash.as_deref() != Some(content_hash.as_str()) {
                            Some(FallbackReason::IndexHashMismatch)
                        } else {
                            None
                        }
                    }
                    Some(_) => Some(FallbackReason::IndexStateMissing),
                };
                DocState {
                    revision_id,
                    fallback,
                }
            })
            .collect())
    }

    /// 逐篇回退直读。某篇哈希校验失败 → 该篇作废（不进结果），并标记 `stale`。
    ///
    /// 只扫**失效的那几篇**（其余走 DB 路径）；引用加成口径与 `DbSectionIndexer`
    /// 的 SQL 路径逐项一致（K-14）。
    async fn scan_fallback_documents(
        &self,
        project_id: Uuid,
        query: &str,
        fetch: i64,
        stale_revision_ids: &[Uuid],
    ) -> Result<Vec<SearchHit>, KnowledgeError> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        let mut hits = Vec::new();
        for revision_id in stale_revision_ids {
            let Some((document_id, doc_key, doc_type, canonical_path)) =
                self.revision_document(project_id, *revision_id).await?
            else {
                continue;
            };
            let content = match crate::knowledge::read_document(
                &self.pool,
                &self.root,
                project_id,
                document_id,
            )
            .await
            {
                Ok(content) => content,
                Err(KnowledgeError::Stale) => {
                    // 该篇磁盘正文与修订哈希不符：作废本篇并标 stale（走原语）。
                    if let Some((tenant_id, revision_id)) =
                        self.stale_state_of(project_id, document_id).await
                    {
                        let _ = crate::knowledge_indexer::mark_stale(
                            &self.pool,
                            &self.root,
                            tenant_id,
                            project_id,
                            revision_id,
                            "read path detected content hash drift",
                        )
                        .await;
                    }
                    continue;
                }
                Err(err) => {
                    // 单篇回退也失败：该篇缺席，整体仍可用。
                    tracing::warn!(
                        project_id = %project_id,
                        document_id = %document_id,
                        error = %err,
                        "knowledge_search_document_unreadable"
                    );
                    continue;
                }
            };
            let boosts = self.reference_boosts(*revision_id).await?;
            for section in
                knowledge_index::parse_markdown_tree(&content.canonical_path, &content.markdown)
            {
                let raw = knowledge_index::raw_score(
                    &section.title,
                    &section.anchor,
                    &section.content,
                    &needle,
                );
                if raw > 0 {
                    let boost = boosts
                        .iter()
                        .find(|anchor| **anchor == section.anchor)
                        .map(|_| 2)
                        .unwrap_or(0);
                    hits.push(SearchHit {
                        section: knowledge_index::MarkdownSection {
                            document_id,
                            doc_key: doc_key.clone(),
                            canonical_path: canonical_path.clone(),
                            ..section
                        },
                        score: knowledge_index::score_section(&doc_type, raw, boost),
                        revision_id: *revision_id,
                        revision_hash: content.content_hash.clone(),
                    });
                }
            }
        }
        hits.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.section.canonical_path.cmp(&b.section.canonical_path))
                .then_with(|| a.section.start_line.cmp(&b.section.start_line))
        });
        hits.truncate(fetch.max(0) as usize);
        Ok(hits)
    }

    /// 某修订所属文档的 `(document_id, doc_key, doc_type, canonical_path)`。
    async fn revision_document(
        &self,
        project_id: Uuid,
        revision_id: Uuid,
    ) -> Result<Option<(Uuid, String, String, String)>, KnowledgeError> {
        Ok(sqlx::query(
            "SELECT d.id, d.doc_key, d.doc_type, d.canonical_path \
               FROM ai.document_revision r \
               JOIN ai.project_document d ON d.id = r.document_id \
              WHERE r.id = $1 AND r.project_id = $2",
        )
        .bind(revision_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| {
            (
                row.get("id"),
                row.get("doc_key"),
                row.get("doc_type"),
                row.get("canonical_path"),
            )
        }))
    }

    /// 该修订上被引用的**非空**章节锚点集合（引用加成的判定依据）。
    async fn reference_boosts(&self, revision_id: Uuid) -> Result<Vec<String>, KnowledgeError> {
        let anchors: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT kr.section_anchor FROM ai.knowledge_reference kr \
              WHERE kr.document_revision_id = $1 \
                AND kr.section_anchor IS NOT NULL AND kr.section_anchor <> ''",
        )
        .bind(revision_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(anchors)
    }

    /// 取某文档当前生效修订的 `(tenant_id, revision_id)`。
    async fn stale_state_of(
        &self,
        project_id: Uuid,
        document_id: Uuid,
    ) -> Option<(Uuid, Uuid)> {
        sqlx::query_as::<_, (Uuid, Uuid)>(
            "SELECT r.tenant_id, r.id FROM ai.document_revision r \
              WHERE r.document_id = $1 AND r.project_id = $2 AND r.status = 'published' \
              ORDER BY r.revision_no DESC LIMIT 1",
        )
        .bind(document_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
    }
}

/// 合并去重：同一 `(document_id, anchor)` 只保留一条，**DB 优先**（评审 P1-3）。
fn merge_hits(db_hits: Vec<SearchHit>, fallback_hits: Vec<SearchHit>) -> Vec<SearchHit> {
    let mut merged: Vec<SearchHit> = Vec::with_capacity(db_hits.len() + fallback_hits.len());
    let mut seen: Vec<(Uuid, String)> = Vec::with_capacity(db_hits.len());
    for hit in db_hits {
        seen.push((hit.section.document_id, hit.section.anchor.clone()));
        merged.push(hit);
    }
    for hit in fallback_hits {
        let key = (hit.section.document_id, hit.section.anchor.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        merged.push(hit);
    }
    merged.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.section.canonical_path.cmp(&b.section.canonical_path))
            .then_with(|| a.section.start_line.cmp(&b.section.start_line))
    });
    merged
}

/// offset 分页：跳过 offset，取 limit。不返回 total / has_more（§6.3）。
fn paginate(hits: Vec<SearchHit>, page: SearchPage) -> Vec<SearchHit> {
    hits.into_iter()
        .skip(page.offset.max(0) as usize)
        .take(page.limit.max(0) as usize)
        .collect()
}
