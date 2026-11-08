//! Markdown 章节解析 + 打分纯函数（T-060 / P6 后半）。
//!
//! 本模块只做**纯函数**与**回退直读扫描**：解析标题树、算分、逐篇读 Git 兜底。
//! 落库索引与检索编排分别在 `knowledge_indexer`（provider 端口）与
//! `knowledge_search`（应用服务）——三层职责不可混（设计稿 §3.2）。

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::knowledge::{self, KnowledgeError, KnowledgeRoot};

/// 文档类型权重（设计稿 §6.2）：`architecture`/`decision` ×1.2、`requirement`
/// ×1.1、其余 ×1.0。整数口径为 `(base * weight + 5) / 10`。
pub fn doc_type_weight(doc_type: &str) -> i64 {
    match doc_type {
        "architecture" | "decision" => 12,
        "requirement" => 11,
        _ => 10,
    }
}

/// 章节命中的原始分（未乘文档类型权重、未加引用加成）。
///
/// 口径逐项对应设计稿 §6.2：标题全等 6 / 标题包含 3 / 锚点包含 2 /
/// 正文出现次数 `min(3, count)`。空锚点不参与锚点命中（评审项 5）。
pub fn raw_score(title: &str, anchor: &str, content: &str, needle: &str) -> i64 {
    if needle.is_empty() {
        return 0;
    }
    let lower_title = title.to_lowercase();
    let lower_anchor = anchor.to_lowercase();
    let lower_content = content.to_lowercase();
    let mut raw = 0;
    if lower_title == needle {
        raw += 6;
    } else if lower_title.contains(needle) {
        raw += 3;
    }
    if !anchor.is_empty() && lower_anchor.contains(needle) {
        raw += 2;
    }
    let occurrences = lower_content.matches(needle).count() as i64;
    raw += occurrences.min(3);
    raw
}

/// 完整分值：`(raw * 类型权重 + 5) / 10` + 引用加成。
///
/// `ref_boost` 由调用方判定（只认当前 published 修订上锚点非空且相等的引用），
/// 本函数不做引用查询，保证 SQL 路径与 Rust 路径口径逐项一致（K-14）。
pub fn score_section(doc_type: &str, raw: i64, ref_boost: i64) -> i64 {
    (raw * doc_type_weight(doc_type) + 5) / 10 + ref_boost
}

/// 一个 Markdown 章节（标题 + 正文 + 定位信息）。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MarkdownSection {
    pub document_id: Uuid,
    pub doc_key: String,
    pub canonical_path: String,
    pub title: String,
    pub level: u8,
    /// 章节锚点（标题 slug；重复标题追加序号）。
    pub anchor: String,
    /// 章节正文（标题行之后到下一个标题之前，已去除首尾空行）。
    pub content: String,
    /// 标题所在行（1-based）。
    pub start_line: usize,
}

/// 一条检索命中。`revision_id` / `revision_hash` 是**证据锚点**：证据必须绑定
/// 具体修订而非文档——文档再次发布后，历史结论不得被解释为最新正文。
/// 两者都不进 REST 响应（`skip_serializing`，守 A-4 响应契约不变）。
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub section: MarkdownSection,
    pub score: i64,
    /// 命中的修订 id（内部证据字段，不序列化）。
    #[serde(skip_serializing)]
    pub revision_id: Uuid,
    /// 命中修订的正文哈希（内部证据字段，不序列化）。
    #[serde(skip_serializing)]
    pub revision_hash: String,
}

/// 解析一篇 Markdown 的标题树。首个标题之前若有非空内容，作为 level 0 的
/// 「前言」章节返回（title 为空、anchor 为 `preamble`）。
pub fn parse_markdown_tree(path: &str, markdown: &str) -> Vec<MarkdownSection> {
    let blank = MarkdownSection {
        document_id: Uuid::nil(),
        doc_key: String::new(),
        canonical_path: path.to_string(),
        title: String::new(),
        level: 0,
        anchor: String::new(),
        content: String::new(),
        start_line: 1,
    };
    let mut sections: Vec<MarkdownSection> = Vec::new();
    let mut current: Option<MarkdownSection> = None;
    let mut preamble: Vec<String> = Vec::new();

    for (index, line) in markdown.lines().enumerate() {
        let line_no = index + 1;
        if let Some((level, title)) = parse_heading(line) {
            if let Some(mut section) = current.take() {
                section.content = trim_blank_lines(&section.content);
                sections.push(section);
            } else if !sections.is_empty() {
                // 不会发生（仅首个标题前进入 preamble）
            }
            let anchor = unique_anchor(&sections, &title);
            current = Some(MarkdownSection {
                level,
                title,
                anchor,
                start_line: line_no,
                content: String::new(),
                ..blank.clone()
            });
        } else if let Some(section) = current.as_mut() {
            section.content.push_str(line);
            section.content.push('\n');
        } else {
            preamble.push(line.to_string());
        }
    }

    if let Some(mut section) = current.take() {
        section.content = trim_blank_lines(&section.content);
        sections.push(section);
    }

    let preamble_text = trim_blank_lines(&preamble.join("\n"));
    if !preamble_text.is_empty() {
        sections.insert(
            0,
            MarkdownSection {
                title: String::new(),
                anchor: "preamble".to_string(),
                content: preamble_text,
                start_line: 1,
                ..blank
            },
        );
    }

    sections
}

fn parse_heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_end();
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &trimmed[level..];
    if !rest.starts_with(' ') {
        return None;
    }
    let title = rest.trim().trim_end_matches('#').trim().to_string();
    if title.is_empty() {
        return None;
    }
    Some((level as u8, title))
}

fn unique_anchor(sections: &[MarkdownSection], title: &str) -> String {
    let base = slugify(title);
    let mut candidate = base.clone();
    let mut suffix = 2;
    while sections.iter().any(|section| section.anchor == candidate) {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
    candidate
}

fn slugify(title: &str) -> String {
    let mut out = String::new();
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() || matches!(ch, '-' | '_') {
            if !out.ends_with('-') {
                out.push('-');
            }
        } else if !ch.is_ascii() {
            // 中文等非 ASCII 字符保留（锚点可读）
            out.push(ch);
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "section".to_string()
    } else {
        trimmed
    }
}

fn trim_blank_lines(text: &str) -> String {
    text.trim_matches('\n').trim().to_string()
}

/// **回退直读扫描**：逐篇读 Git 权威文件并按同一打分口径算分。
///
/// 只在索引不可用 / 失效时使用（设计稿 §6.4 第 4 步）。正常路径走
/// `DbSectionIndexer::search`（落库索引），本函数不承担主检索职责。
pub async fn scan_documents_direct(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    query: &str,
    limit: i64,
) -> Result<Vec<SearchHit>, KnowledgeError> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Ok(Vec::new());
    }

    let documents = knowledge::list_documents(pool, project_id).await?;
    let mut hits = Vec::new();

    for summary in documents {
        // 单篇读失败（文件缺失 / 哈希漂移）→ 该篇缺席，整体仍可用（§6.5）。
        let content = match knowledge::read_document(pool, root, project_id, summary.id).await {
            Ok(content) => content,
            Err(err) => {
                tracing::warn!(
                    project_id = %project_id,
                    document_id = %summary.id,
                    error = %err,
                    "knowledge_search_document_unreadable"
                );
                continue;
            }
        };
        let revision_id = current_revision_id(pool, summary.id).await?;
        for section in parse_markdown_tree(&content.canonical_path, &content.markdown) {
            let raw = raw_score(&section.title, &section.anchor, &section.content, &needle);
            if raw > 0 {
                hits.push(SearchHit {
                    section: MarkdownSection {
                        document_id: summary.id,
                        doc_key: summary.doc_key.clone(),
                        ..section
                    },
                    score: score_section(&content.doc_type, raw, 0),
                    revision_id: revision_id.unwrap_or_else(Uuid::nil),
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
    hits.truncate(limit.max(0) as usize);
    Ok(hits)
}

/// 文档当前生效（最新 published）修订 id。
pub async fn current_revision_id(
    pool: &PgPool,
    document_id: Uuid,
) -> Result<Option<Uuid>, KnowledgeError> {
    Ok(sqlx::query_scalar(
        "SELECT id FROM ai.document_revision \
          WHERE document_id = $1 AND status = 'published' \
          ORDER BY revision_no DESC LIMIT 1",
    )
    .bind(document_id)
    .fetch_optional(pool)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# 架构约束\n\n引言。\n\n## 背景\n\n地图太大。\n\n### 细节\n\n分块加载。\n\n## 决策\n\n采用 LOD。\n";

    #[test]
    fn parses_tree_with_levels_and_anchors() {
        let sections = parse_markdown_tree("projects/P/architecture/A.md", DOC);
        assert_eq!(sections.len(), 4); // 架构约束 / 背景 / 细节 / 决策
        assert_eq!(sections[0].title, "架构约束");
        assert_eq!(sections[0].level, 1);
        assert_eq!(sections[1].title, "背景");
        assert_eq!(sections[1].content, "地图太大。");
        assert_eq!(sections[2].level, 3);
        assert_eq!(sections[3].anchor, "决策");
        assert_eq!(sections[1].start_line, 5);
    }

    #[test]
    fn preamble_before_first_heading_is_kept() {
        let sections = parse_markdown_tree("p.md", "前言内容\n\n# 标题\n\n正文\n");
        assert_eq!(sections[0].anchor, "preamble");
        assert_eq!(sections[0].content, "前言内容");
        assert_eq!(sections[1].title, "标题");
    }

    #[test]
    fn duplicate_titles_get_unique_anchors() {
        let sections = parse_markdown_tree("p.md", "# A\n\n# A\n\n# A\n");
        assert_eq!(sections[0].anchor, "a");
        assert_eq!(sections[1].anchor, "a-2");
        assert_eq!(sections[2].anchor, "a-3");
    }

    #[test]
    fn non_ascii_titles_keep_readable_anchors() {
        let sections = parse_markdown_tree("p.md", "## 地图分块\n\n内容\n");
        assert_eq!(sections[0].anchor, "地图分块");
    }

    #[test]
    fn heading_requires_space_after_hashes() {
        let sections = parse_markdown_tree("p.md", "#NotAHeading\n\n正文\n");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].anchor, "preamble");
    }

    #[test]
    fn raw_score_ranks_title_equality_above_title_contains() {
        // 标题全等 6；标题包含 3（锚点与正文都不命中）。
        let exact = raw_score("lod", "", "body", "lod");
        let contains = raw_score("lod 决策", "", "body", "lod");
        assert_eq!(exact, 6);
        assert_eq!(contains, 3);
        assert!(exact > contains);
    }

    #[test]
    fn raw_score_counts_body_occurrences_up_to_three() {
        assert_eq!(raw_score("t", "a", "body", "x"), 0);
        assert_eq!(raw_score("t", "a", "x", "x"), 1);
        assert_eq!(raw_score("t", "a", "x x", "x"), 2);
        assert_eq!(raw_score("t", "a", "x x x x", "x"), 3);
    }

    #[test]
    fn empty_anchor_never_scores() {
        // 空锚点不参与锚点命中（评审项 5）。
        assert_eq!(raw_score("t", "", "body", "anchor"), 0);
        assert_eq!(raw_score("t", "anchor", "body", "anchor"), 2);
    }

    #[test]
    fn doc_type_weight_ranks_decision_above_knowledge() {
        let decision = score_section("decision", 10, 0);
        let requirement = score_section("requirement", 10, 0);
        let knowledge = score_section("knowledge", 10, 0);
        assert!(decision > requirement);
        assert!(requirement > knowledge);
        assert_eq!(knowledge, 10);
        assert_eq!(decision, 12);
        assert_eq!(requirement, 11);
    }

    #[test]
    fn reference_boost_is_added_on_top() {
        assert_eq!(score_section("knowledge", 10, 2), 12);
    }

    #[test]
    fn empty_needle_scores_nothing() {
        assert_eq!(raw_score("t", "a", "c", ""), 0);
    }
}
