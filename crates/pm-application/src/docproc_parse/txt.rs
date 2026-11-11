//! 纯文本 Parser Adapter：按空行分段（knowledge_parse 同语义）。

use super::{ParseCtx, ParseError, ParseOutcome, ParseReport, ParseWarning, ParserAdapter};

pub struct TextAdapter;

impl ParserAdapter for TextAdapter {
    fn name(&self) -> &'static str {
        "txt"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn supports(&self, ext: &str, _head: &[u8]) -> bool {
        ext.eq_ignore_ascii_case("txt")
    }

    fn parse(&self, ctx: ParseCtx<'_>) -> Result<ParseOutcome, ParseError> {
        let text = String::from_utf8_lossy(ctx.bytes).into_owned();
        Ok(ParseOutcome {
            markdown: text,
            images: Vec::new(),
            tables: Vec::new(),
            mapping: Vec::new(),
            warnings: vec![ParseWarning {
                code: "no_outline".into(),
                message: "text parsed as plain paragraphs; no outline levels".into(),
                location: None,
            }],
            report: ParseReport::default(),
        })
    }
}
