//! 文档解析层（WP-0 交付物②；裁决 Q-B=A：纯 Rust ZIP+XML，零 LibreOffice）。
//!
//! **解析与发布严格分离（长期安全约束）**：AI 可以解析/整理/提取标题/生成草稿，
//! 但**不能越过真人确认门**——本模块只产出 [`ParsedDocument`]（喂
//! `kb.document_revision` draft），发布权在 `confirm_revision`。
//!
//! V1 范围（裁决）：Markdown / Text 直读 + **docx**（Word 大纲级别 → `#` 层级
//! 优先还原）；pdf/xlsx/image 仅枚举占位（选型稿 Q5 后置）。复杂批注/复杂表格
//! 不列入 V1 硬指标——表格拍平为 `|` 分隔行并记 warning。
//!
//! **硬门槛**：合成 docx 大纲还原率 ≥90%（`tests/knowledge_parse.rs`）。

use std::io::Read as _;

use quick_xml::events::Event;
use quick_xml::Reader;

/// 解析错误。
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unsupported or corrupted docx: {0}")]
    Docx(String),
    #[error("unknown parse kind: {0}")]
    UnknownKind(String),
}

/// 解析输入类型（pdf/xlsx/image 占位——选型稿 Q5 后置）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseKind {
    Markdown,
    Text,
    Docx,
}

impl ParseKind {
    /// 从文件扩展名推断（小写；未知扩展 → None）。
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "md" | "markdown" => Some(Self::Markdown),
            "txt" => Some(Self::Text),
            "docx" => Some(Self::Docx),
            _ => None,
        }
    }
}

/// 解析产物：规范化 markdown + 警告（版式噪声/拍平动作留痕）。
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub markdown: String,
    pub warnings: Vec<String>,
}

/// 解析入口（纯函数；同输入同输出）。
pub fn parse(kind: ParseKind, bytes: &[u8]) -> Result<ParsedDocument, ParseError> {
    match kind {
        ParseKind::Markdown => Ok(ParsedDocument {
            markdown: String::from_utf8_lossy(bytes).into_owned(),
            warnings: Vec::new(),
        }),
        ParseKind::Text => Ok(ParsedDocument {
            markdown: String::from_utf8_lossy(bytes).into_owned(),
            warnings: vec!["text parsed as plain paragraphs; no outline levels".into()],
        }),
        ParseKind::Docx => parse_docx(bytes),
    }
}

/* ── docx ────────────────────────────────────────────────────────────────── */

/// Word 样式 → markdown 标题层级（大纲级别还原的核心映射）。
/// 返回 None = 非标题段落。
fn style_to_heading(style: &str) -> Option<usize> {
    match style {
        "Title" | "Subtitle" => Some(1),
        "Heading1" => Some(1),
        "Heading2" => Some(2),
        "Heading3" => Some(3),
        "Heading4" => Some(4),
        "Heading5" => Some(5),
        "Heading6" => Some(6),
        _ => None,
    }
}

/// 解析 docx（ZIP 容器 + `word/document.xml`）。
fn parse_docx(bytes: &[u8]) -> Result<ParsedDocument, ParseError> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| ParseError::Docx(format!("zip: {e}")))?;
    let mut document_xml = archive
        .by_name("word/document.xml")
        .map_err(|e| ParseError::Docx(format!("missing word/document.xml: {e}")))?;
    let mut xml_bytes = Vec::new();
    document_xml
        .read_to_end(&mut xml_bytes)
        .map_err(|e| ParseError::Docx(format!("read document.xml: {e}")))?;
    let xml = String::from_utf8_lossy(&xml_bytes).into_owned();

    let mut parser = Reader::from_str(&xml);
    parser.config_mut().trim_text(false);

    let mut out = String::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut in_paragraph = false;
    let mut in_table = false;
    let mut style = String::new();
    let mut is_list = false;
    let mut text = String::new();

    loop {
        match parser.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                "w:p" => {
                    in_paragraph = true;
                    style.clear();
                    is_list = false;
                    text.clear();
                }
                "w:pStyle" => {
                    // 段落样式（w:val 属性；大纲级别的唯一来源）。
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == "w:val" {
                            style = attr.value.clone().into_owned();
                        }
                    }
                }
                "w:numPr" => is_list = true,
                "w:tbl" => {
                    in_table = true;
                    warnings.push("table flattened to text rows".into());
                }
                "w:tc" if in_table => {
                    // 表格单元格：单元格边界 → " | " 分隔。
                    if !out.ends_with(" | ") && !out.is_empty() {
                        out.push_str(" | ");
                    }
                }
                "w:sectPr" | "w:bookmarkStart" | "w:bookmarkEnd" => {}
                _ => {}
            },
            Ok(Event::Empty(e)) => {
                // `<w:numPr/>` 等自闭合元素走 Empty 事件。
                if e.name().as_ref() == "w:numPr" {
                    is_list = true;
                }
                if e.name().as_ref() == "w:pStyle" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == "w:val" {
                            style = attr.value.clone().into_owned();
                        }
                    }
                }
            }
            Ok(Event::Text(t)) => {
                if in_paragraph || in_table {
                    text.push_str(t.as_ref());
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                "w:p" => {
                    in_paragraph = false;
                    let content = text.trim().to_string();
                    if content.is_empty() {
                        continue;
                    }
                    if let Some(level) = style_to_heading(&style) {
                        if !out.is_empty() {
                            out.push('\n');
                        }
                        out.push_str(&"#".repeat(level));
                        out.push(' ');
                        out.push_str(&content);
                        out.push('\n');
                    } else if is_list {
                        out.push_str("- ");
                        out.push_str(&content);
                        out.push('\n');
                    } else if in_table {
                        out.push_str(&content);
                        out.push('\n');
                    } else {
                        out.push_str(&content);
                        out.push_str("\n\n");
                    }
                }
                "w:tbl" => {
                    in_table = false;
                    out.push_str("\n\n");
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => return Err(ParseError::Docx(format!("xml: {err}"))),
            _ => {}
        }
    }

    if out.trim().is_empty() {
        return Err(ParseError::Docx("document has no text content".into()));
    }

    Ok(ParsedDocument {
        markdown: out,
        warnings,
    })
}
