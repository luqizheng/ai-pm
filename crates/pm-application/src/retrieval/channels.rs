//! 检索通道（V1：`pc_entity` 真实现；其余通道待 WP-3 索引设施）。
//!
//! **约束前置纪律**（WP-4 裁决 §6.4①）：权限 / 版本 / 权威性过滤在**通道级 SQL**
//! 完成——进入融合与组装的候选**已过硬过滤**；组装器只裁剪不推翻。
//!
//! `pc_entity` 通道的四级匹配顺序（WP-1 裁决 Q-B）：精确 Key/编号 → 精确名称 →
//! ILIKE 模糊 → 类型/状态/上下文排序。**ADR 编号、Requirement Key、Design Key
//! 不依赖全文检索。**
//!
//! **历史版本不在通道内展开**：历史检索走「版本钉死直读 + 现场解析」（基线 §5.6
//! 的历史策略，WP-4 组装期落地）——通道只出**现行**候选（现行正式 / 草稿候选）。

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::retrieval::cjk::cjk_bigrams;
use kb_pc_contracts::retrieval::{
    AuthorityStatus, KnowledgeDomain, RetrievalChannel, RetrievalEvidence, RetrievalMode,
};

/// 通道召回结果（单通道）。
#[derive(Default)]
pub struct ChannelResult {
    pub hits: Vec<RetrievalEvidence>,
    /// 该通道不可用的原因码（如 `kb_index_unavailable`）——无则 None。
    pub unavailable: Option<String>,
}

/// 实体通道（PC 结构化）。
///
/// 权威过滤：只取 `lifecycle_status='active'`（现行正式）与 `draft`（草稿候选，
/// 标 `draft_candidate`）；`deprecated` / `superseded` **不进现行结果**。
pub async fn pc_entity_channel(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    queries: &[String],
    limit: usize,
) -> Result<ChannelResult, sqlx::Error> {
    let mut hits: Vec<RetrievalEvidence> = Vec::new();
    let query = queries.first().cloned().unwrap_or_default();
    if query.trim().is_empty() {
        return Ok(ChannelResult {
            hits,
            unavailable: None,
        });
    }

    // 四级匹配的排序分：精确 Key/编号 100 > 精确名称 60 > ILIKE 20。
    let rows = sqlx::query(
        "SELECT o.id, o.object_type, o.object_key, o.display_name, o.lifecycle_status, \
                r.revision_no, r.content_hash, d.content \
           FROM pc.cognition_object o \
           LEFT JOIN pc.cognition_revision r ON r.id = o.current_revision_id \
           LEFT JOIN pc.document_revision d ON d.id = r.document_revision_id \
          WHERE o.tenant_id = $1 AND o.project_id = $2 \
            AND o.lifecycle_status IN ('active', 'draft', 'identity') \
            AND (o.object_key ILIKE $3 OR o.display_name ILIKE $3) \
          ORDER BY (CASE WHEN lower(o.object_key) = lower($4) THEN 100 \
                         WHEN lower(o.display_name) = lower($4) THEN 60 \
                         ELSE 20 END) DESC, \
                   o.updated_at DESC \
          LIMIT $5",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(format!("%{query}%"))
    .bind(&query)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let object_id: Uuid = row.get("id");
        let object_type: String = row.get("object_type");
        let lifecycle: String = row.get("lifecycle_status");
        let display_name: String = row.get("display_name");
        let content: Option<String> = row.get("content");
        let content_hash: Option<String> = row.get("content_hash");
        let revision_no: Option<i32> = row.get("revision_no");

        let authority = match lifecycle.as_str() {
            "active" => AuthorityStatus::CurrentPublished,
            "draft" | "identity" => AuthorityStatus::DraftCandidate,
            _ => AuthorityStatus::Superseded,
        };

        // excerpt = display_name + 正文首段（WP-4 §6.2 按证据类型区分）。
        let excerpt = match content.as_deref().and_then(first_paragraph) {
            Some(first) if !first.is_empty() => format!("{display_name}\n{first}"),
            _ => display_name.clone(),
        };

        hits.push(RetrievalEvidence {
            evidence_id: format!("pc-object:{object_id}"),
            source_domain: KnowledgeDomain::Pc,
            source_id: object_id.to_string(),
            revision_id: None,
            authority_status: authority,
            project_id,
            relevance_score: None,
            retrieval_channel: RetrievalChannel::PcEntity,
            title: Some(display_name),
            content_excerpt: excerpt,
            locator: content_hash.map(|h| format!("pc:{object_type}:{h}")),
            freshness: None,
            truncated: false,
            relationship_path: Vec::new(),
            speculative: false,
        });
        let _ = revision_no;
    }

    Ok(ChannelResult {
        hits,
        unavailable: None,
    })
}

/// KB Section 复合打分权重（WP-4 裁决：**命名化配置，禁 SQL 魔法数字**）。
/// 这些权重属 KB 通道内部排序，不是 authority/freshness 权重。
#[derive(Debug, Clone, Copy)]
pub struct KbSectionScore {
    pub fts_rank: f32,
    pub title_match_bonus: f32,
    pub anchor_match_bonus: f32,
    pub reference_anchor_bonus: f32,
}

/// 初始权重（命名化常量；doc_type 权重见 [`doc_type_weight`]）。
impl Default for KbSectionScore {
    fn default() -> Self {
        Self {
            fts_rank: 100.0,
            title_match_bonus: 30.0,
            anchor_match_bonus: 20.0,
            reference_anchor_bonus: 15.0,
        }
    }
}

/// doc_type 权重（WP-4 定版：architecture/decision=12、requirement=11、其他=10）。
fn doc_type_weight(doc_type: &str) -> f32 {
    match doc_type {
        "architecture" | "decision" => 12.0,
        "requirement" => 11.0,
        _ => 10.0,
    }
}

/// PC 全文通道（WP-3 接真：`pc.document_revision.search_tsv`）。
///
/// 权威过滤在通道级 SQL：只经 `current_revision_id` 链到达正文行——**只有现行
/// 内容行可达**（deprecated/superseded/旧修订结构上不可见）。
pub async fn pc_fts_channel(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    queries: &[String],
    limit: usize,
) -> ChannelResult {
    let Some(first) = queries.first().map(String::as_str) else {
        return ChannelResult::default();
    };
    let tsv_query = cjk_bigrams(first);
    if tsv_query.is_empty() {
        return ChannelResult::default();
    }
    let rows = match sqlx::query(
        "SELECT o.id AS object_id, o.display_name, dr.id AS revision_id, dr.content, \
                ts_rank(dr.search_tsv, plainto_tsquery('simple', $3)) AS rank \
           FROM pc.cognition_object o \
           JOIN pc.cognition_revision cr ON cr.id = o.current_revision_id \
           JOIN pc.document_revision dr ON dr.id = cr.document_revision_id \
          WHERE o.tenant_id = $1 AND o.project_id = $2 \
            AND o.lifecycle_status IN ('active', 'draft', 'identity') \
            AND dr.search_tsv @@ plainto_tsquery('simple', $3) \
          ORDER BY rank DESC, o.updated_at DESC \
          LIMIT $4",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&tsv_query)
    .bind(limit as i64)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!(error = %err, "pc_fts channel query failed");
            return ChannelResult {
                hits: Vec::new(),
                unavailable: Some("pc_fts_index_unavailable".to_string()),
            };
        }
    };
    let hits = rows
        .iter()
        .map(|row| {
            let object_id: Uuid = row.get("object_id");
            let revision_id: Uuid = row.get("revision_id");
            let display_name: String = row.get("display_name");
            let content: String = row.get("content");
            let rank: f32 = row.get::<Option<f32>, _>("rank").unwrap_or(0.0);
            let excerpt = match first_paragraph(&content) {
                Some(first) if !first.is_empty() => format!("{display_name}\n{first}"),
                _ => display_name.clone(),
            };
            RetrievalEvidence {
                evidence_id: format!("pc-revision:{revision_id}"),
                source_domain: KnowledgeDomain::Pc,
                source_id: object_id.to_string(),
                revision_id: Some(revision_id.to_string()),
                authority_status: AuthorityStatus::CurrentPublished,
                project_id,
                relevance_score: Some(rank),
                retrieval_channel: RetrievalChannel::PcFts,
                title: Some(display_name),
                content_excerpt: excerpt,
                locator: Some(format!("pc-revision:{revision_id}")),
                freshness: None,
                truncated: false,
                relationship_path: Vec::new(),
                speculative: false,
            }
        })
        .collect();
    ChannelResult {
        hits,
        unavailable: None,
    }
}

/// KB Section 通道（WP-3 接真：`kb.document_section_index` + state 五联判定）。
///
/// 有效性判定（库内四项）：provider + status='indexed' + index_version +
/// content_hash 一致；**家族现行性**由索引写入侧保证（只索引现行 + 切换清零），
/// state 是该保证的台账面。
pub async fn kb_section_channel(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    queries: &[String],
    limit: usize,
) -> ChannelResult {
    use crate::kb_service::indexer::{KB_INDEX_VERSION, KB_SECTION_PROVIDER};
    let Some(first) = queries.first().map(String::as_str) else {
        return ChannelResult::default();
    };
    let tsv_query = cjk_bigrams(first);
    if tsv_query.is_empty() {
        return ChannelResult::default();
    }
    let rows = match sqlx::query(
        "SELECT s.document_id, s.document_revision_id, s.section_no, s.title, s.content, \
                d.doc_type, dr.content_hash, \
                ts_rank(s.search_tsv, plainto_tsquery('simple', $3)) AS rank, \
                (position(lower($4) in lower(s.title)) > 0) AS in_title \
           FROM kb.document_section_index s \
           JOIN kb.document_index_state st \
             ON st.document_revision_id = s.document_revision_id AND st.provider = s.provider \
           JOIN kb.document d ON d.id = s.document_id \
           JOIN kb.document_revision dr ON dr.id = s.document_revision_id \
          WHERE s.tenant_id = $1 AND s.project_id = $2 AND s.provider = $5 \
            AND st.status = 'indexed' AND st.index_version = $6 \
            AND st.indexed_content_hash = dr.content_hash \
            AND s.search_tsv @@ plainto_tsquery('simple', $3) \
          ORDER BY rank DESC, s.section_no ASC \
          LIMIT $7",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&tsv_query)
    .bind(first)
    .bind(KB_SECTION_PROVIDER)
    .bind(KB_INDEX_VERSION)
    .bind(limit as i64)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!(error = %err, "kb_section channel query failed");
            return ChannelResult {
                hits: Vec::new(),
                unavailable: Some("kb_section_index_unavailable".to_string()),
            };
        }
    };
    let weights = KbSectionScore::default();
    let hits = rows
        .iter()
        .map(|row| {
            let document_id: Uuid = row.get("document_id");
            let revision_id: Uuid = row.get("document_revision_id");
            let section_no: i32 = row.get("section_no");
            let title: String = row.get("title");
            let content: String = row.get("content");
            let doc_type: String = row.get("doc_type");
            let rank: f32 = row.get::<Option<f32>, _>("rank").unwrap_or(0.0);
            let in_title: bool = row.get("in_title");
            // 复合打分（命名化权重；同通道内部排序，非 authority/freshness）。
            let score = rank * weights.fts_rank
                + if in_title {
                    weights.title_match_bonus
                } else {
                    0.0
                }
                + doc_type_weight(&doc_type);
            // 章节正文是有界单元；超 2000 字符截断并标注。
            let (excerpt, truncated) = if content.chars().count() > 2000 {
                (content.chars().take(2000).collect::<String>(), true)
            } else {
                (content.clone(), false)
            };
            RetrievalEvidence {
                evidence_id: format!("kb-section:{revision_id}:{section_no}"),
                source_domain: KnowledgeDomain::Kb,
                source_id: document_id.to_string(),
                revision_id: Some(revision_id.to_string()),
                authority_status: AuthorityStatus::CurrentPublished,
                project_id,
                relevance_score: Some(score),
                retrieval_channel: RetrievalChannel::KbSection,
                title: Some(title),
                content_excerpt: excerpt,
                locator: Some(format!("kb:{revision_id}:{section_no}")),
                freshness: None,
                truncated,
                relationship_path: Vec::new(),
                speculative: false,
            }
        })
        .collect();
    ChannelResult {
        hits,
        unavailable: None,
    }
}

/// 该模式是否含 KB 域。
pub fn mode_includes_kb(mode: RetrievalMode) -> bool {
    matches!(
        mode,
        RetrievalMode::KbOnly | RetrievalMode::PcKb | RetrievalMode::CrossDomain
    )
}

/// 该模式是否含 PC 域。
pub fn mode_includes_pc(mode: RetrievalMode) -> bool {
    matches!(
        mode,
        RetrievalMode::PcOnly | RetrievalMode::PcKb | RetrievalMode::CrossDomain
    )
}

/// 正文首段（空行分隔的第一段；**跳过 markdown 标题段**——标题不是正文）。
fn first_paragraph(content: &str) -> Option<&str> {
    content
        .split("\n\n")
        .map(str::trim)
        .find(|p| !p.is_empty() && !p.starts_with('#'))
}
