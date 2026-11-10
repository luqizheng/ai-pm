//! HTML Parser Adapter：quick-xml 流式（h1-h6/p/ul/ol/li/table/img/a）。
//! 外链资源不抓取（安全边界）；script/style 跳过。

use super::{
    image_ref_path, md_cell, xml_text, ParseCtx, ParseError, ParseOutcome, ParseReport,
    ParseWarning, ParserAdapter, SourceMapping,
};
use quick_xml::events::Event;
use quick_xml::Reader;

pub struct HtmlAdapter;

impl ParserAdapter for HtmlAdapter {
    fn name(&self) -> &'static str {
        "html"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn supports(&self, ext: &str, head: &[u8]) -> bool {
        let ext_ok = matches!(ext.to_ascii_lowercase().as_str(), "html" | "htm");
        let head_ok = {
            let t = String::from_utf8_lossy(&head[..head.len().min(512)]).to_ascii_lowercase();
            t.contains("<!doctype") || t.contains("<html")
        };
        ext_ok && (head_ok || head.contains(&b'<'))
    }

    fn parse(&self, ctx: ParseCtx<'_>) -> Result<ParseOutcome, ParseError> {
        let html = String::from_utf8_lossy(ctx.bytes).into_owned();
        let mut parser = Reader::from_str(&html);
        parser.config_mut().trim_text(true);

        let mut markdown = String::new();
        let mut mapping: Vec<SourceMapping> = Vec::new();
        let mut warnings: Vec<ParseWarning> = Vec::new();

        let mut skip_depth = 0usize; // script/style
        let mut heading: Option<(usize, String)> = None;
        let mut list_stack: Vec<bool> = Vec::new(); // true = ordered
        let mut ordered_n = 0usize;
        let mut para = String::new();
        let mut link_href: Option<String> = None;
        let mut link_text = String::new();
        let mut img_seq = 0usize;
        // 表格
        let mut in_table = false;
        let mut tbl_rows: Vec<Vec<String>> = Vec::new();
        let mut tbl_row: Vec<String> = Vec::new();
        let mut tbl_cell = String::new();

        loop {
            match parser.read_event() {
                Ok(Event::Start(e)) => match e.name().as_ref() {
                    "script" | "style" => skip_depth += 1,
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let lvl: usize = e.name().as_ref()[1..].parse().unwrap_or(1);
                        heading = Some((lvl, String::new()));
                    }
                    "p" => para.clear(),
                    "ul" => list_stack.push(false),
                    "ol" => {
                        ordered_n = 0;
                        list_stack.push(true);
                    }
                    "li" => para.clear(),
                    "table" => {
                        in_table = true;
                        tbl_rows.clear();
                    }
                    "tr" => tbl_row.clear(),
                    "td" | "th" => tbl_cell.clear(),
                    "a" => {
                        link_href = e
                            .attributes()
                            .flatten()
                            .find(|a| a.key.as_ref() == "href")
                            .map(|a| a.value.into_owned());
                        link_text.clear();
                    }
                    "img" => {
                        let src = e
                            .attributes()
                            .flatten()
                            .find(|a| a.key.as_ref() == "src")
                            .map(|a| a.value.into_owned());
                        let alt = e
                            .attributes()
                            .flatten()
                            .find(|a| a.key.as_ref() == "alt")
                            .map(|a| a.value.into_owned())
                            .unwrap_or_default();
                        img_seq += 1;
                        let ref_path = image_ref_path(img_seq, "ext");
                        warnings.push(ParseWarning {
                            code: "html_image_external".into(),
                            message: format!(
                                "image src={src:?} not fetched (external resource; alt={alt:?})"
                            ),
                            location: None,
                        });
                        markdown.push_str(&format!("![image-{img_seq:03}]({ref_path})\n\n"));
                    }
                    "br" => {
                        if heading.is_some() {
                            // 标题内换行 → 空格
                            if let Some((_, t)) = heading.as_mut() {
                                t.push(' ');
                            }
                        } else {
                            para.push(' ');
                        }
                    }
                    _ => {}
                },
                Ok(Event::Empty(e)) => {
                    if e.name().as_ref() == "img" {
                        img_seq += 1;
                        warnings.push(ParseWarning {
                            code: "html_image_external".into(),
                            message: "image not fetched (external resource)".into(),
                            location: None,
                        });
                    }
                }
                Ok(Event::Text(t)) => {
                    let text = xml_text(&t);
                    if skip_depth == 0 {
                        if let Some((_, ht)) = heading.as_mut() {
                            ht.push_str(&text);
                        } else if in_table {
                            tbl_cell.push_str(&text);
                        } else {
                            para.push_str(&text);
                            if link_href.is_some() {
                                link_text.push_str(&text);
                            }
                        }
                    }
                }
                Ok(Event::End(e)) => match e.name().as_ref() {
                    "script" | "style" => skip_depth = skip_depth.saturating_sub(1),
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        if let Some((lvl, text)) = heading.take() {
                            let content = text.trim().to_string();
                            if !content.is_empty() {
                                if !markdown.is_empty() {
                                    markdown.push('\n');
                                }
                                markdown.push_str(&"#".repeat(lvl));
                                markdown.push(' ');
                                markdown.push_str(&content);
                                markdown.push('\n');
                                mapping.push(SourceMapping {
                                    mapping_type: pm_domain::docproc::MappingType::ParagraphToSection,
                                    source_location: serde_json::json!({ "tag": format!("h{lvl}") }),
                                    target_location: serde_json::json!({ "line_start": markdown.lines().count() }),
                                });
                            }
                        }
                    }
                    "p" => {
                        let content = para.trim().to_string();
                        if !content.is_empty() {
                            markdown.push_str(&content);
                            markdown.push_str("\n\n");
                        }
                        para.clear();
                    }
                    "li" => {
                        let content = para.trim().to_string();
                        if !content.is_empty() {
                            let ordered = list_stack.last().copied().unwrap_or(false);
                            if ordered {
                                ordered_n += 1;
                                markdown.push_str(&format!("{ordered_n}. {content}\n"));
                            } else {
                                markdown.push_str(&format!("- {content}\n"));
                            }
                        }
                        para.clear();
                    }
                    "ul" | "ol" => {
                        list_stack.pop();
                        if list_stack.is_empty() && !markdown.is_empty() {
                            markdown.push('\n');
                        }
                    }
                    "td" | "th" => {
                        tbl_row.push(tbl_cell.trim().to_string());
                        tbl_cell.clear();
                    }
                    "tr" => {
                        if !tbl_row.is_empty() {
                            tbl_rows.push(std::mem::take(&mut tbl_row));
                        }
                    }
                    "table" => {
                        in_table = false;
                        if !tbl_rows.is_empty() {
                            let cols = tbl_rows.iter().map(|r| r.len()).max().unwrap_or(0);
                            let line_start = markdown.lines().count() + 1;
                            for (i, row) in tbl_rows.iter().enumerate() {
                                markdown.push('|');
                                for c in 0..cols {
                                    markdown.push(' ');
                                    markdown.push_str(&md_cell(
                                        row.get(c).map(|s| s.as_str()).unwrap_or(""),
                                    ));
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
                            markdown.push('\n');
                            mapping.push(SourceMapping {
                                mapping_type: pm_domain::docproc::MappingType::TableToTable,
                                source_location: serde_json::json!({ "html_table": true }),
                                target_location: serde_json::json!({ "line_start": line_start, "table_index": 0 }),
                            });
                        }
                    }
                    "a" => {
                        // 链接文本并入正文（保留目标地址）
                        if let Some(href) = link_href.take() {
                            let t = link_text.trim().to_string();
                            if !t.is_empty() {
                                para.push_str(&format!("[{t}]({href})"));
                            }
                        }
                        link_text.clear();
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(err) => return Err(ParseError::Corrupted(format!("html xml: {err}"))),
                _ => {}
            }
        }

        if markdown.trim().is_empty() {
            return Err(ParseError::Corrupted("html has no text content".into()));
        }

        let report = ParseReport {
            sections: mapping.len(),
            tables: 0,
            images: img_seq,
            ocr_pages: 0,
            warnings: warnings.len(),
            failures: 0,
            ocr_required_pages: Vec::new(),
        };
        Ok(ParseOutcome {
            markdown,
            images: Vec::new(),
            tables: vec![],
            mapping,
            warnings,
            report,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_and_table() {
        let adapter = HtmlAdapter;
        let html = b"<html><body><h1>T</h1><p>hello</p><table><tr><th>a</th><th>b</th></tr><tr><td>1</td><td>2</td></tr></table></body></html>";
        let outcome = adapter.parse(ParseCtx { bytes: html }).expect("parse");
        assert!(outcome.markdown.starts_with("# T\n"));
        assert!(outcome.markdown.contains("| a | b |"));
        assert!(outcome.markdown.contains("| 1 | 2 |"));
    }
}
