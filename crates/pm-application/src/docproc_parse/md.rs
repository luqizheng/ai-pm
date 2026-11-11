//! Markdown Parser Adapter：原样读取 + 结构解析（标题树供追溯映射）。

use super::{ParseCtx, ParseError, ParseOutcome, ParseReport, ParserAdapter, SourceMapping};

pub struct MarkdownAdapter;

impl ParserAdapter for MarkdownAdapter {
    fn name(&self) -> &'static str {
        "md"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn supports(&self, ext: &str, head: &[u8]) -> bool {
        matches!(ext.to_ascii_lowercase().as_str(), "md" | "markdown")
            // 文本魔数：UTF-8 BOM 或可打印开头即放行（md 无固定魔数）
            && !head.starts_with(b"PK")
    }

    fn parse(&self, ctx: ParseCtx<'_>) -> Result<ParseOutcome, ParseError> {
        let markdown = String::from_utf8_lossy(ctx.bytes).into_owned();
        let mut mapping = Vec::new();
        let mut sections = 0usize;
        for (idx, line) in markdown.lines().enumerate() {
            let trimmed = line.trim_start();
            let hashes = trimmed.chars().take_while(|c| *c == '#').count();
            if hashes > 0 && hashes <= 6 && trimmed[hashes..].starts_with(' ') {
                sections += 1;
                mapping.push(SourceMapping {
                    mapping_type: pm_domain::docproc::MappingType::ParagraphToSection,
                    source_location: serde_json::json!({ "line": idx + 1 }),
                    target_location: serde_json::json!({ "line_start": idx + 1, "heading": trimmed.to_string() }),
                });
            }
        }
        let report = ParseReport {
            sections,
            tables: 0,
            images: 0,
            ocr_pages: 0,
            warnings: 0,
            failures: 0,
            ocr_required_pages: Vec::new(),
        };
        Ok(ParseOutcome {
            markdown,
            images: Vec::new(),
            tables: Vec::new(),
            mapping,
            warnings: Vec::new(),
            report,
        })
    }
}
