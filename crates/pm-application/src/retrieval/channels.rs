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

use kb_pc_contracts::retrieval::{
    AuthorityStatus, KnowledgeDomain, RetrievalChannel, RetrievalEvidence, RetrievalMode,
};

/// 通道召回结果（单通道）。
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

/// PC 全文通道（V1 占位：依赖 WP-3 的 `pc.document_revision.search_tsv`）。
pub async fn pc_fts_channel() -> ChannelResult {
    ChannelResult {
        hits: Vec::new(),
        unavailable: Some("pc_fts_index_unavailable".to_string()),
    }
}

/// KB Section 通道（V1 占位：依赖 WP-3 的 `kb.document_section_index`）。
pub async fn kb_section_channel() -> ChannelResult {
    ChannelResult {
        hits: Vec::new(),
        unavailable: Some("kb_section_index_unavailable".to_string()),
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
