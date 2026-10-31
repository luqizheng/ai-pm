//! Markdown 树索引（知识空间 V2 的索引层 V1）：自建标题/章节解析 + 内存检索。
//! 设计稿 §九 路径 A：不依赖 PageIndex 的 Markdown 支持，先把「文档树 + 章节定位」
//! 做成可替换的检索适配层；返回的 locator（path + anchor + 行号）可回溯到具体章节。

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::knowledge::{self, KnowledgeError, KnowledgeRoot};

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

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub section: MarkdownSection,
    pub score: u32,
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

/// 在项目全部文档里做关键词检索（大小写不敏感）。V1 为内存扫描：文档量小，
/// 且每次都走 `read_document`（含 content_hash 漂移校验）。
pub async fn search_documents(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    query: &str,
) -> Result<Vec<SearchHit>, KnowledgeError> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Ok(Vec::new());
    }

    let documents = knowledge::list_documents(pool, project_id).await?;
    let mut hits = Vec::new();

    for summary in documents {
        let content =
            knowledge::read_document(pool, root, project_id, summary.id).await?;
        for section in parse_markdown_tree(&content.canonical_path, &content.markdown) {
            let mut score = 0;
            if section.title.to_lowercase().contains(&needle) {
                score += 2;
            }
            if section.content.to_lowercase().contains(&needle) {
                score += 1;
            }
            if score > 0 {
                hits.push(SearchHit {
                    section: MarkdownSection {
                        document_id: summary.id,
                        doc_key: summary.doc_key.clone(),
                        ..section
                    },
                    score,
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
    hits.truncate(20);
    Ok(hits)
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
}