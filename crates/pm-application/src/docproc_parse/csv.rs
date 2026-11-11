//! CSV Parser Adapter（RFC4180：引号转义、CRLF；首行表头 → GFM 管道表）。

use super::{
    md_cell, ParseCtx, ParseError, ParseOutcome, ParseReport, ParseWarning, ParserAdapter,
    SourceMapping, TableInfo,
};

pub struct CsvAdapter;

impl ParserAdapter for CsvAdapter {
    fn name(&self) -> &'static str {
        "csv"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn supports(&self, ext: &str, _head: &[u8]) -> bool {
        ext.eq_ignore_ascii_case("csv")
    }

    fn parse(&self, ctx: ParseCtx<'_>) -> Result<ParseOutcome, ParseError> {
        let text = String::from_utf8_lossy(ctx.bytes).into_owned();
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut field = String::new();
        let mut in_quotes = false;
        let mut row: Vec<String> = Vec::new();
        let mut chars = text.chars().peekable();

        while let Some(c) = chars.next() {
            if in_quotes {
                match c {
                    '"' => {
                        if chars.peek() == Some(&'"') {
                            chars.next();
                            field.push('"');
                        } else {
                            in_quotes = false;
                        }
                    }
                    _ => field.push(c),
                }
            } else {
                match c {
                    '"' => in_quotes = true,
                    ',' => {
                        row.push(std::mem::take(&mut field));
                    }
                    '\r' => {}
                    '\n' => {
                        row.push(std::mem::take(&mut field));
                        rows.push(std::mem::take(&mut row));
                    }
                    _ => field.push(c),
                }
            }
        }
        // 尾行（无换行结尾）
        if !field.is_empty() || !row.is_empty() {
            row.push(field);
            rows.push(row);
        }
        rows.retain(|r| !r.is_empty());

        let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        if rows.is_empty() || cols == 0 {
            return Err(ParseError::Corrupted("csv: no rows".into()));
        }

        let mut markdown = String::new();
        let mut warnings = Vec::new();
        if rows.len() > 10_000 {
            warnings.push(ParseWarning {
                code: "csv_truncated".into(),
                message: format!("csv has {} rows; rendered first 10000", rows.len()),
                location: None,
            });
        }
        for (i, r) in rows.iter().take(10_000).enumerate() {
            markdown.push('|');
            for c in 0..cols {
                markdown.push(' ');
                markdown.push_str(&md_cell(r.get(c).map(|s| s.as_str()).unwrap_or("")));
                markdown.push_str(" |");
            }
            markdown.push('\n');
            if i == 0 {
                markdown.push('|');
                for _ in 0..cols {
                    markdown.push_str(" --- |");
                }
                markdown.push('\n');
            }
        }

        let report = ParseReport {
            sections: 1,
            tables: 1,
            images: 0,
            ocr_pages: 0,
            warnings: warnings.len(),
            failures: 0,
            ocr_required_pages: Vec::new(),
        };
        Ok(ParseOutcome {
            markdown,
            images: Vec::new(),
            tables: vec![TableInfo {
                rows: rows.len(),
                cols,
                fidelity: "markdown",
                location: None,
            }],
            mapping: vec![SourceMapping {
                mapping_type: pm_domain::docproc::MappingType::TableToTable,
                source_location: serde_json::json!({ "csv": true }),
                target_location: serde_json::json!({ "line_start": 1, "table_index": 0 }),
            }],
            warnings,
            report,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_fields_with_commas() {
        let adapter = CsvAdapter;
        let outcome = adapter
            .parse(ParseCtx {
                bytes: b"a,\"b,c\"\nd,e,f\n",
            })
            .expect("parse");
        assert!(outcome.markdown.contains("b,c"));
        assert_eq!(outcome.tables[0].rows, 2);
        assert_eq!(outcome.tables[0].cols, 3);
    }
}
