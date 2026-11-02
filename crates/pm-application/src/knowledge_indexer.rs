//! 知识索引 provider 端口与 DB 章节索引实现（T-060 / P6 后半）。
//!
//! 三层职责（设计稿 §3.1–§3.2）：
//!
//! - 本模块（`knowledge_indexer`）= **provider 端口**：只做「重建」与「检索」；
//! - `knowledge_search` = **应用服务**：取当前修订、判有效性、编排 DB/回退、
//!   合并去重、排序、分页；
//! - `knowledge_index` = **纯函数**：解析与打分。
//!
//! 不变量（INV-1 / INV-2，设计稿 §5.3）：
//! - `status ≠ 'indexed'` ⇒ 该 `(revision, provider)` 章节行数 = 0；
//! - `status = 'indexed'` ⇒ 哈希与版本均匹配当前修订 ∧ 章节行数 > 0。
//!
//! 状态与产物的原子性由「唯一转换原语 `apply_transition`」保证——任何路径
//! （发布 / 手动 reindex / 读路径漂移 / veto）都只能经它写入。

use async_trait::async_trait;
use serde::Serialize;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::knowledge::{self, KnowledgeError, KnowledgeRoot};
use crate::knowledge_index::{self, MarkdownSection, SearchHit};

/// DB 章节索引 provider 名。
pub const DB_SECTION_PROVIDER: &str = "db-section";
/// DB 章节索引的解析规则版本；**参与有效性判定**（设计稿 §4.2）。
/// 上调后全部旧索引即时失效，需跑项目级全量重建（§5.5）。
pub const DB_SECTION_INDEX_VERSION: &str = "v1";

pub const DEFAULT_SEARCH_LIMIT: i64 = 20;
pub const MAX_SEARCH_LIMIT: i64 = 100;

/// 分页参数（缺省 20 / 0；`limit` 夹在 1..=100）。
#[derive(Debug, Clone, Copy)]
pub struct SearchPage {
    pub limit: i64,
    pub offset: i64,
}

impl Default for SearchPage {
    fn default() -> Self {
        Self {
            limit: DEFAULT_SEARCH_LIMIT,
            offset: 0,
        }
    }
}

impl SearchPage {
    /// 夹紧：`limit` 落在 1..=100；`offset` 负数按 0。
    pub fn normalized(limit: Option<i64>, offset: Option<i64>) -> Self {
        let limit = limit
            .unwrap_or(DEFAULT_SEARCH_LIMIT)
            .clamp(1, MAX_SEARCH_LIMIT);
        let offset = offset.unwrap_or(0).max(0);
        Self { limit, offset }
    }
}

/// 一次重建的结果。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum IndexOutcome {
    /// 索引就绪，产物已落库。
    Indexed { sections: i64 },
    /// 磁盘正文哈希 ≠ 修订哈希（产物已清空）。
    Stale,
    /// 文件缺失 / 无标题 / 解析失败（产物已清空）。
    Failed { message: String },
    /// 锁内前置条件已变化：本次操作无任何写入。
    Superseded,
}

/// 可替换的索引 provider 端口。实现只需提供「重建」与「检索」两个语义。
#[async_trait]
pub trait KnowledgeIndexer: Send + Sync {
    fn provider(&self) -> &'static str;
    fn index_version(&self) -> &'static str;

    /// 重建一篇文档当前生效修订的索引。哈希漂移 → `Stale`；解析失败 → `Failed`。
    async fn rebuild_document(
        &self,
        project_id: Uuid,
        revision_id: Uuid,
    ) -> Result<IndexOutcome, KnowledgeError>;

    /// 在已落库的索引产物里检索。基础设施不可用时必须返回 `Err`，**不得返回空 Vec**。
    async fn search(
        &self,
        project_id: Uuid,
        tenant_id: Uuid,
        query: &str,
        page: SearchPage,
    ) -> Result<Vec<SearchHit>, KnowledgeError>;
}

/// 重建要写入的 transition（设计稿 §5.3）。
#[derive(Debug, Clone)]
pub enum Transition {
    /// 章节由解析结果给出。
    Indexed { sections: Vec<SectionRow> },
    /// 磁盘正文哈希 ≠ 修订哈希。
    Stale { reason: String },
    /// 文件缺失 / 无标题 / 解析失败。
    Failed { message: String },
}

/// 一条章节产物行。
#[derive(Debug, Clone)]
pub struct SectionRow {
    pub section_no: i32,
    pub level: i16,
    pub title: String,
    pub anchor: String,
    pub content: String,
    pub start_line: i32,
}

/// DB 章节索引实现（provider = `db-section`）。
pub struct DbSectionIndexer {
    pool: PgPool,
    root: KnowledgeRoot,
}

impl DbSectionIndexer {
    pub fn new(pool: PgPool, root: KnowledgeRoot) -> Self {
        Self { pool, root }
    }
}

#[async_trait]
impl KnowledgeIndexer for DbSectionIndexer {
    fn provider(&self) -> &'static str {
        DB_SECTION_PROVIDER
    }

    fn index_version(&self) -> &'static str {
        DB_SECTION_INDEX_VERSION
    }

    async fn rebuild_document(
        &self,
        project_id: Uuid,
        revision_id: Uuid,
    ) -> Result<IndexOutcome, KnowledgeError> {
        let row = sqlx::query(
            "SELECT d.canonical_path, r.content_hash, r.status, r.tenant_id \
               FROM ai.document_revision r \
               JOIN ai.project_document d ON d.id = r.document_id \
              WHERE r.id = $1 AND r.project_id = $2",
        )
        .bind(revision_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(KnowledgeError::NotFound)?;
        let canonical_path: String = row.get("canonical_path");
        let content_hash: String = row.get("content_hash");
        let status: String = row.get("status");
        let tenant_id: Uuid = row.get("tenant_id");
        if status != "published" {
            return Err(KnowledgeError::Conflict(format!(
                "revision {revision_id} is {status}, only published revisions can be indexed"
            )));
        }

        // 读权威文件并算盘上哈希（锁外观测只用于早失败；锁内会再算一次）。
        let transition = match self.read_and_parse(&canonical_path, &content_hash).await {
            Ok(sections) => Transition::Indexed { sections },
            Err(KnowledgeError::Stale) => Transition::Stale {
                reason: "document content hash drifted from the revision".to_string(),
            },
            Err(err) => Transition::Failed {
                message: err.to_string(),
            },
        };

        apply_transition(
            &self.pool,
            &self.root,
            tenant_id,
            project_id,
            revision_id,
            DB_SECTION_PROVIDER,
            DB_SECTION_INDEX_VERSION,
            transition,
        )
        .await
    }

    async fn search(
        &self,
        project_id: Uuid,
        tenant_id: Uuid,
        query: &str,
        page: SearchPage,
    ) -> Result<Vec<SearchHit>, KnowledgeError> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        // 两路候选各取 limit + offset 条，足以覆盖合并后的前 limit + offset 条
        // （设计稿 §6.3）。
        let fetch = page.limit + page.offset;

        let rows = sqlx::query(
            "WITH latest AS ( \
               SELECT d.id AS document_id, d.doc_key, d.doc_type, d.canonical_path, \
                      r.id AS revision_id, r.content_hash \
                 FROM ai.project_document d \
                 JOIN LATERAL ( \
                     SELECT id, content_hash FROM ai.document_revision \
                      WHERE document_id = d.id AND status = 'published' \
                      ORDER BY revision_no DESC LIMIT 1 \
                 ) r ON true \
                WHERE d.project_id = $1 AND d.tenant_id = $2 \
             ), base AS ( \
               SELECT l.*, s.section_no, s.level, s.title, s.anchor, s.content, s.start_line, \
                      (CASE WHEN lower(s.title) = $3 THEN 6 \
                            WHEN strpos(lower(s.title), $3) > 0 THEN 3 \
                            ELSE 0 END) \
                      + (CASE WHEN s.anchor <> '' AND strpos(lower(s.anchor), $3) > 0 THEN 2 \
                            ELSE 0 END) \
                      + LEAST(3, (length(lower(s.content)) - length(replace(lower(s.content), $3, ''))) \
                              / greatest(length($3), 1))::bigint AS raw \
                 FROM ai.document_section_index s \
                 JOIN latest l ON l.revision_id = s.document_revision_id \
                 JOIN ai.document_index_state st \
                   ON st.document_revision_id = l.revision_id \
                  AND st.provider = $4 \
                  AND st.status = 'indexed' \
                  AND st.index_version = $5 \
                  AND st.indexed_content_hash = l.content_hash \
                WHERE s.provider = $4 AND s.tenant_id = $2 \
             ), scored AS ( \
               SELECT base.*, \
                      (((raw * CASE doc_type WHEN 'architecture' THEN 12 \
                                            WHEN 'decision'     THEN 12 \
                                            WHEN 'requirement'  THEN 11 \
                                            ELSE 10 END) + 5) / 10)::bigint AS score, \
                      (CASE WHEN EXISTS ( \
                          SELECT 1 FROM ai.knowledge_reference kr \
                           WHERE kr.document_revision_id = base.revision_id \
                             AND base.anchor <> '' \
                             AND kr.section_anchor IS NOT NULL \
                             AND kr.section_anchor <> '' \
                             AND kr.section_anchor = base.anchor \
                      ) THEN 2 ELSE 0 END)::bigint AS ref_boost \
                 FROM base \
                WHERE raw > 0 \
             ) \
             SELECT document_id, doc_key, doc_type, canonical_path, revision_id, content_hash, \
                    section_no, level, title, anchor, content, start_line, raw, ref_boost, \
                    score + ref_boost AS total \
               FROM scored \
              ORDER BY total DESC, canonical_path, start_line, section_no \
              LIMIT $6",
        )
        .bind(project_id)
        .bind(tenant_id)
        .bind(&needle)
        .bind(DB_SECTION_PROVIDER)
        .bind(DB_SECTION_INDEX_VERSION)
        .bind(fetch)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let doc_type: String = row.get("doc_type");
                let raw: i64 = row.get("raw");
                let ref_boost: i64 = row.get("ref_boost");
                let level: i16 = row.get("level");
                SearchHit {
                    section: MarkdownSection {
                        document_id: row.get("document_id"),
                        doc_key: row.get("doc_key"),
                        canonical_path: row.get("canonical_path"),
                        title: row.get("title"),
                        level: level.max(0) as u8,
                        anchor: row.get("anchor"),
                        content: row.get("content"),
                        start_line: row.get::<i32, _>("start_line").max(0) as usize,
                    },
                    score: knowledge_index::score_section(&doc_type, raw, ref_boost),
                    revision_id: row.get("revision_id"),
                    revision_hash: row.get("content_hash"),
                }
            })
            .collect())
    }
}

impl DbSectionIndexer {
    /// 读权威文件 → 算盘上哈希 → 解析标题树。哈希不符即 `Stale`。
    async fn read_and_parse(
        &self,
        canonical_path: &str,
        expected_hash: &str,
    ) -> Result<Vec<SectionRow>, KnowledgeError> {
        let abs = knowledge::safe_join(self.root.path(), canonical_path)?;
        let markdown = std::fs::read_to_string(&abs)
            .map_err(|err| KnowledgeError::Io(format!("{}: {err}", abs.display())))?;
        if knowledge::content_hash(&markdown) != expected_hash {
            return Err(KnowledgeError::Stale);
        }
        let sections = knowledge_index::parse_markdown_tree(canonical_path, &markdown);
        if !sections.iter().any(|section| section.level > 0) {
            return Err(KnowledgeError::Conflict(
                "document has no Markdown heading to index".to_string(),
            ));
        }
        Ok(sections
            .into_iter()
            .enumerate()
            .map(|(index, section)| SectionRow {
                section_no: index as i32,
                level: section.level as i16,
                title: section.title,
                anchor: section.anchor,
                content: section.content,
                start_line: section.start_line as i32,
            })
            .collect())
    }
}

/// **唯一状态转换原语**（设计稿 §5.3）：写索引产物与状态的唯一入口。
///
/// 单事务内：取 `(revision, provider)` 级 advisory 锁 → 锁内**重算**前置条件
/// → 按最终 transition 删旧写新（章节与状态原子一致）→ 提交。
pub async fn apply_transition(
    pool: &PgPool,
    root: &KnowledgeRoot,
    tenant_id: Uuid,
    project_id: Uuid,
    revision_id: Uuid,
    provider: &str,
    index_version: &str,
    transition: Transition,
) -> Result<IndexOutcome, KnowledgeError> {
    let mut tx = pool.begin().await?;

    // 1) 锁：`(revision, provider)` 粒度。成功重建 / stale 清理 / failed 清理
    //    共用同一把锁——否则「旧操作删掉新产物」。
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))")
        .bind(format!("ai.document_section_index:{revision_id}:{provider}"))
        .execute(&mut *tx)
        .await?;

    // 2) 锁内重算前置条件（不信任锁外观测）。
    let row = sqlx::query(
        "SELECT r.content_hash, r.status, r.document_id, d.canonical_path, \
                (SELECT id FROM ai.document_revision \
                  WHERE document_id = r.document_id AND status = 'published' \
                  ORDER BY revision_no DESC LIMIT 1) AS head_revision_id \
           FROM ai.document_revision r \
           JOIN ai.project_document d ON d.id = r.document_id \
          WHERE r.id = $1 AND r.project_id = $2 AND r.tenant_id = $3",
    )
    .bind(revision_id)
    .bind(project_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(KnowledgeError::NotFound)?;
    let content_hash: String = row.get("content_hash");
    let status: String = row.get("status");
    let canonical_path: String = row.get("canonical_path");
    let head_revision_id: Option<Uuid> = row.get("head_revision_id");

    // 锁内重读盘上正文并算哈希。
    let disk_hash = match std::fs::read_to_string(
        knowledge::safe_join(root.path(), &canonical_path)?,
    ) {
        Ok(text) => Some(knowledge::content_hash(&text)),
        Err(_) => None,
    };

    let transition = match transition {
        Transition::Indexed { sections } => {
            // 历史修订不再重建：只有当前生效修订进入索引（评审 P0-1）。
            if head_revision_id != Some(revision_id) || status != "published" {
                tx.rollback().await?;
                return Ok(IndexOutcome::Superseded);
            }
            match disk_hash {
                Some(hash) if hash == content_hash => Transition::Indexed { sections },
                Some(_) => Transition::Stale {
                    reason: "document content hash drifted from the revision".to_string(),
                },
                None => Transition::Failed {
                    message: "knowledge file is unreadable".to_string(),
                },
            }
        }
        Transition::Stale { reason } => {
            // 别人刚修好 → 本次操作无任何写入。
            if disk_hash.as_deref() == Some(content_hash.as_str()) {
                tx.rollback().await?;
                return Ok(IndexOutcome::Superseded);
            }
            Transition::Stale { reason }
        }
        Transition::Failed { message } => {
            // 当前已是「哈希与版本均匹配的 indexed」→ 本次失败判定无意义。
            let current: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT status, indexed_content_hash, index_version FROM ai.document_index_state \
                  WHERE document_revision_id = $1 AND provider = $2",
            )
            .bind(revision_id)
            .bind(provider)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some((state_status, indexed_hash, state_version)) = current {
                if state_status == "indexed"
                    && indexed_hash.as_deref() == Some(content_hash.as_str())
                    && state_version.as_deref() == Some(index_version)
                {
                    tx.rollback().await?;
                    return Ok(IndexOutcome::Superseded);
                }
            }
            Transition::Failed { message }
        }
    };

    // 3) 按最终 transition 执行（章节与状态同事务）。
    let outcome = match &transition {
        Transition::Indexed { sections } => {
            sqlx::query(
                "DELETE FROM ai.document_section_index \
                  WHERE document_revision_id = $1 AND provider = $2",
            )
            .bind(revision_id)
            .bind(provider)
            .execute(&mut *tx)
            .await?;
            for section in sections {
                sqlx::query(
                    "INSERT INTO ai.document_section_index \
                     (tenant_id, project_id, document_revision_id, provider, section_no, \
                      level, title, anchor, content, start_line) \
                     VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
                )
                .bind(tenant_id)
                .bind(project_id)
                .bind(revision_id)
                .bind(provider)
                .bind(section.section_no)
                .bind(section.level)
                .bind(&section.title)
                .bind(&section.anchor)
                .bind(&section.content)
                .bind(section.start_line)
                .execute(&mut *tx)
                .await?;
            }
            upsert_state(
                &mut tx,
                project_id,
                revision_id,
                provider,
                "indexed",
                Some(&content_hash),
                Some(index_version),
                None,
            )
            .await?;
            IndexOutcome::Indexed {
                sections: sections.len() as i64,
            }
        }
        Transition::Stale { reason } => {
            sqlx::query(
                "DELETE FROM ai.document_section_index \
                  WHERE document_revision_id = $1 AND provider = $2",
            )
            .bind(revision_id)
            .bind(provider)
            .execute(&mut *tx)
            .await?;
            upsert_state(
                &mut tx,
                project_id,
                revision_id,
                provider,
                "stale",
                None,
                Some(index_version),
                Some(reason),
            )
            .await?;
            IndexOutcome::Stale
        }
        Transition::Failed { message } => {
            sqlx::query(
                "DELETE FROM ai.document_section_index \
                  WHERE document_revision_id = $1 AND provider = $2",
            )
            .bind(revision_id)
            .bind(provider)
            .execute(&mut *tx)
            .await?;
            upsert_state(
                &mut tx,
                project_id,
                revision_id,
                provider,
                "failed",
                None,
                Some(index_version),
                Some(message),
            )
            .await?;
            IndexOutcome::Failed {
                message: message.clone(),
            }
        }
    };

    tx.commit().await?;
    Ok(outcome)
}

/// 写状态行（`ON CONFLICT (document_revision_id, provider)` 与既有唯一键一致）。
async fn upsert_state(
    tx: &mut Transaction<'_, Postgres>,
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
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 重建一个修订并**降级记录**结果（设计稿 §5.4）。
///
/// 索引失败**不阻断、不回滚、不重试**已发布的 Git 修订；状态留痕，检索自动回退。
/// 四个写入点统一调它。
pub async fn rebuild_quietly(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    revision_id: Uuid,
) {
    let started = std::time::Instant::now();
    let indexer = DbSectionIndexer::new(pool.clone(), root.clone());
    match indexer.rebuild_document(project_id, revision_id).await {
        Ok(IndexOutcome::Indexed { sections }) => {
            tracing::info!(
                provider = DB_SECTION_PROVIDER,
                %revision_id,
                sections,
                elapsed_ms = started.elapsed().as_millis() as u64,
                "knowledge_index_rebuilt"
            );
        }
        Ok(IndexOutcome::Stale) => {
            tracing::warn!(
                provider = DB_SECTION_PROVIDER,
                %revision_id,
                "knowledge_index_stale"
            );
        }
        Ok(IndexOutcome::Superseded) => {
            tracing::debug!(
                provider = DB_SECTION_PROVIDER,
                %revision_id,
                "knowledge_index_superseded"
            );
        }
        Ok(IndexOutcome::Failed { message }) => {
            tracing::warn!(
                provider = DB_SECTION_PROVIDER,
                %revision_id,
                message,
                "knowledge_index_failed"
            );
        }
        Err(err) => {
            tracing::warn!(
                provider = DB_SECTION_PROVIDER,
                %revision_id,
                error = %err,
                "knowledge_index_failed"
            );
        }
    }
}

/// 便捷入口：把某修订标记为 `stale` 并清空产物（读路径发现漂移时用）。
///
/// 走的是同一个原语——**禁止**直接 `UPDATE status`（设计稿 §5.3 INV-1）。
pub async fn mark_stale(
    pool: &PgPool,
    root: &KnowledgeRoot,
    tenant_id: Uuid,
    project_id: Uuid,
    revision_id: Uuid,
    reason: &str,
) -> Result<IndexOutcome, KnowledgeError> {
    apply_transition(
        pool,
        root,
        tenant_id,
        project_id,
        revision_id,
        DB_SECTION_PROVIDER,
        DB_SECTION_INDEX_VERSION,
        Transition::Stale {
            reason: reason.to_string(),
        },
    )
    .await
}

/// 索引状态行（只读视图，给 `GET /documents/{id}/index-state`）。
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
