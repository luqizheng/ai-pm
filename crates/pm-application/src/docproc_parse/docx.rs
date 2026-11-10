//! DOCX Parser Adapter（OOXML WordprocessingML）。
//!
//! 基座沿用 knowledge_parse.rs 裁决（ZIP+quick-xml，样式→大纲级别），
//! 升级点：styles.xml 的 outlineLvl 兜底（WPS 中文样式名不依赖拼写）、
//! numbering.xml 列表类型、表格逐单元格、图片提取、段落级追溯映射。

use super::{
    image_ref_path, md_cell, open_zip_guarded, xml_attr, xml_text, ExtractedImage, ParseCtx,
    ParseError, ParseOutcome, ParseReport, ParseWarning, ParserAdapter, SourceMapping, TableInfo,
    ZipBudget,
};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::io::Read as _;

pub struct DocxAdapter;

impl ParserAdapter for DocxAdapter {
    fn name(&self) -> &'static str {
        "docx"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn supports(&self, ext: &str, head: &[u8]) -> bool {
        ext.eq_ignore_ascii_case("docx") && head.starts_with(b"PK")
    }

    fn parse(&self, ctx: ParseCtx<'_>) -> Result<ParseOutcome, ParseError> {
        parse_docx(ctx.bytes)
    }
}

/// 样式表：style id → markdown 标题层级（1-6）。
/// 依据 styles.xml 的 `<w:outlineLvl w:val="N"/>`（大纲级别是唯一可信源，
/// 规避「标题 1/Heading 1/标题1」拼写差异——TD-03 §2.1 中文样式用例）。
fn load_style_levels(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    budget: &mut ZipBudget,
) -> HashMap<String, usize> {
    if let Ok(mut entry) = archive.by_name("word/styles.xml") {
        if let Ok(xml_bytes) = budget.read_entry(&mut entry, "styles") {
            let xml = String::from_utf8_lossy(&xml_bytes).into_owned();
            return parse_style_levels(&xml);
        }
    }
    HashMap::new()
}

/// 从 styles.xml 抽取 styleId → outlineLvl+1（大纲级别 0 起 → markdown 1 级）。
fn parse_style_levels(xml: &str) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    let mut parser = Reader::from_str(xml);
    parser.config_mut().trim_text(true);
    let mut current_style: Option<String> = None;
    loop {
        match parser.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                "w:style" => current_style = xml_attr(&e, "w:styleId"),
                "w:outlineLvl" => {
                    if let (Some(style), Some(val)) =
                        (current_style.as_ref(), xml_attr(&e, "w:val"))
                    {
                        if let Ok(lvl) = val.parse::<usize>() {
                            // outlineLvl 0 → H1；上限 H6
                            map.insert(style.clone(), (lvl + 1).min(6));
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::End(e)) => {
                if e.name().as_ref() == "w:style" {
                    current_style = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    map
}

/// 编号定义表：numId → 是否有序（numbering.xml 中任一 lvl 的
/// numFmt=decimal 即有序）。两级解析：numId→abstractNumId→fmt。
fn load_ordered_lists(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    budget: &mut ZipBudget,
) -> HashMap<String, bool> {
    let mut out = HashMap::new();
    if let Ok(mut entry) = archive.by_name("word/numbering.xml") {
        if let Ok(xml_bytes) = budget.read_entry(&mut entry, "numbering") {
            let xml = String::from_utf8_lossy(&xml_bytes).into_owned();
            let mut parser = Reader::from_str(&xml);
            parser.config_mut().trim_text(true);
            let mut num_to_abstract: HashMap<String, String> = HashMap::new();
            let mut abstract_decimal: HashMap<String, bool> = HashMap::new();
            let mut cur_num: Option<String> = None;
            let mut cur_abs: Option<String> = None;
            loop {
                match parser.read_event() {
                    Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                        "w:num" => cur_num = xml_attr(&e, "w:numId"),
                        "w:abstractNum" => {
                            cur_abs = xml_attr(&e, "w:abstractNumId");
                            if let Some(abs) = &cur_abs {
                                abstract_decimal.entry(abs.clone()).or_insert(false);
                            }
                        }
                        "w:abstractNumId" => {
                            if let (Some(num), Some(val)) =
                                (cur_num.as_ref(), xml_attr(&e, "w:val"))
                            {
                                num_to_abstract.insert(num.clone(), val);
                            }
                        }
                        "w:numFmt" => {
                            if let (Some(abs), Some(fmt)) =
                                (cur_abs.as_ref(), xml_attr(&e, "w:val"))
                            {
                                if fmt == "decimal" {
                                    abstract_decimal.insert(abs.clone(), true);
                                }
                            }
                        }
                        _ => {}
                    },
                    Ok(Event::End(e)) => match e.name().as_ref() {
                        "w:num" => cur_num = None,
                        "w:abstractNum" => cur_abs = None,
                        _ => {}
                    },
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
            for (num, abs) in num_to_abstract {
                if let Some(ordered) = abstract_decimal.get(&abs) {
                    out.insert(num, *ordered);
                }
            }
        }
    }
    out
}

/// 关系表：rId → 目标路径（word/ 相对）。
fn load_rels(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    budget: &mut ZipBudget,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(mut entry) = archive.by_name("word/_rels/document.xml.rels") {
        if let Ok(xml_bytes) = budget.read_entry(&mut entry, "rels") {
            let xml = String::from_utf8_lossy(&xml_bytes).into_owned();
            let mut parser = Reader::from_str(&xml);
            parser.config_mut().trim_text(true);
            loop {
                match parser.read_event() {
                    Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                        if e.name().as_ref() == "Relationship" {
                            if let (Some(id), Some(target)) =
                                (xml_attr(&e, "Id"), xml_attr(&e, "Target"))
                            {
                                map.insert(id, target);
                            }
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        }
    }
    map
}

struct ParagraphState {
    text: String,
    style: String,
    num_id: Option<String>,
    link_target: Option<String>,
}

fn parse_docx(bytes: &[u8]) -> Result<ParseOutcome, ParseError> {
    let mut archive = open_zip_guarded(bytes, "docx")?;
    let mut budget = ZipBudget::new();

    // 加密容器检测（ECMA-376 Standard Encryption 以 EncryptedPackage entry
    // 为标志；规格 §5.3 加密文件 → 明确错误）。
    if archive.by_name("EncryptedPackage").is_ok() {
        return Err(ParseError::Encrypted("docx EncryptedPackage".into()));
    }

    let document_xml = {
        let mut entry = archive
            .by_name("word/document.xml")
            .map_err(|e| ParseError::Corrupted(format!("docx missing document.xml: {e}")))?;
        budget.read_entry(&mut entry, "document.xml")?
    };
    let style_levels = load_style_levels(&mut archive, &mut budget);
    let ordered = load_ordered_lists(&mut archive, &mut budget);
    let rels = load_rels(&mut archive, &mut budget);

    let xml = String::from_utf8_lossy(&document_xml).into_owned();
    let mut parser = Reader::from_str(&xml);
    parser.config_mut().trim_text(false);

    let mut markdown = String::new();
    let mut images: Vec<ExtractedImage> = Vec::new();
    let mut tables: Vec<TableInfo> = Vec::new();
    let mut mapping: Vec<SourceMapping> = Vec::new();
    let mut warnings: Vec<ParseWarning> = Vec::new();

    let mut current_line = 1usize;
    let mut paragraph_index: usize = 0;
    let mut p = ParagraphState {
        text: String::new(),
        style: String::new(),
        num_id: None,
        link_target: None,
    };
    let mut ordered_counters: HashMap<String, usize> = HashMap::new();

    // 表格状态
    let mut tbl_depth = 0usize;
    let mut tbl_rows: Vec<Vec<String>> = Vec::new();
    let mut tbl_row: Vec<String> = Vec::new();
    let mut tbl_cell = String::new();

    loop {
        match parser.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                "w:p" => {
                    p.text.clear();
                    p.style.clear();
                    p.num_id = None;
                    p.link_target = None;
                    paragraph_index += 1;
                }
                "w:pStyle" => p.style = xml_attr(&e, "w:val").unwrap_or_default(),
                "w:numId" => p.num_id = xml_attr(&e, "w:val"),
                "w:hyperlink" => {
                    p.link_target = xml_attr(&e, "r:id").and_then(|id| rels.get(&id).cloned());
                }
                "w:tbl" => {
                    tbl_depth += 1;
                    tbl_rows.clear();
                }
                "w:tr" => tbl_row.clear(),
                "w:tc" => tbl_cell.clear(),
                _ => {}
            },
            Ok(Event::Empty(e)) => match e.name().as_ref() {
                "w:pStyle" => p.style = xml_attr(&e, "w:val").unwrap_or_default(),
                "w:numId" => p.num_id = xml_attr(&e, "w:val"),
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if tbl_depth > 0 {
                    tbl_cell.push_str(&xml_text(&t));
                } else {
                    p.text.push_str(&xml_text(&t));
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                "w:p" => {
                    if tbl_depth > 0 {
                        // 表格内的段落 = 单元格文本行
                        tbl_cell.push_str(p.text.trim());
                    } else {
                        let content = p.text.trim().to_string();
                        if !content.is_empty() {
                            if !markdown.is_empty() && !markdown.ends_with('\n') {
                                markdown.push('\n');
                            }
                            let line_start = current_line;
                            if let Some(level) = style_levels.get(&p.style).copied() {
                                markdown.push_str(&"#".repeat(level));
                                markdown.push(' ');
                                markdown.push_str(&content);
                                markdown.push('\n');
                                mapping.push(SourceMapping {
                                    mapping_type: pm_domain::docproc::MappingType::ParagraphToSection,
                                    source_location: serde_json::json!({ "paragraph": paragraph_index }),
                                    target_location: serde_json::json!({ "line_start": line_start }),
                                });
                                current_line += 1;
                            } else if let Some(num) = p.num_id.as_ref() {
                                if ordered.get(num).copied().unwrap_or(false) {
                                    let n = ordered_counters.entry(num.clone()).or_insert(0);
                                    *n += 1;
                                    markdown.push_str(&format!("{n}. {content}\n"));
                                } else {
                                    markdown.push_str(&format!("- {content}\n"));
                                }
                                current_line += 1;
                            } else {
                                markdown.push_str(&content);
                                markdown.push_str("\n\n");
                                current_line += 2;
                            }
                        }
                        // 超链接目标未并入正文时补记（保留目标地址，规格 §4.3）
                        if let Some(target) = p.link_target.take() {
                            if !content.is_empty() && !markdown.contains(&format!("]({target})")) {
                                markdown.push_str(&format!("[{target}]({target})\n"));
                                current_line += 1;
                            }
                        }
                    }
                    p.text.clear();
                }
                "w:tc" => {
                    tbl_row.push(tbl_cell.trim().to_string());
                    tbl_cell.clear();
                }
                "w:tr" => {
                    if !tbl_row.is_empty() {
                        tbl_rows.push(std::mem::take(&mut tbl_row));
                    }
                }
                "w:tbl" => {
                    tbl_depth -= 1;
                    if tbl_depth == 0 && !tbl_rows.is_empty() {
                        let cols = tbl_rows.iter().map(|r| r.len()).max().unwrap_or(0);
                        let line_start = current_line;
                        render_md_table(&mut markdown, &tbl_rows, cols);
                        current_line += tbl_rows.len() + 2;
                        tables.push(TableInfo {
                            rows: tbl_rows.len(),
                            cols,
                            fidelity: "markdown",
                            location: Some(serde_json::json!({ "paragraph": paragraph_index })),
                        });
                        mapping.push(SourceMapping {
                            mapping_type: pm_domain::docproc::MappingType::TableToTable,
                            source_location: serde_json::json!({ "paragraph": paragraph_index }),
                            target_location: serde_json::json!({
                                "line_start": line_start,
                                "table_index": tables.len() - 1
                            }),
                        });
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => return Err(ParseError::Corrupted(format!("docx xml: {err}"))),
            _ => {}
        }
    }

    // 图片提取（word/media/*；rels 已载入，V1 按 media 序号定位——规格 §4.4
    // 「记录图片在原文中的位置」的 V1 粒度）。
    let media_names: Vec<String> = archive
        .file_names()
        .filter(|n| n.starts_with("word/media/"))
        .map(|n| n.to_string())
        .collect();
    for (idx, name) in media_names.iter().enumerate() {
        let ext = name
            .rsplit('.')
            .next()
            .unwrap_or("bin")
            .to_ascii_lowercase();
        let Ok(mut entry) = archive.by_name(name) else {
            continue;
        };
        let mut data = Vec::new();
        if entry.read_to_end(&mut data).is_err() || data.is_empty() {
            warnings.push(ParseWarning {
                code: "image_extraction_failed".into(),
                message: format!("failed to read {name}"),
                location: None,
            });
            continue;
        }
        let ref_path = image_ref_path(idx + 1, &ext);
        markdown.push_str(&format!("![image-{idx:03}]({ref_path})\n\n"));
        mapping.push(SourceMapping {
            mapping_type: pm_domain::docproc::MappingType::AssetRef,
            source_location: serde_json::json!({ "media_index": idx + 1 }),
            target_location: serde_json::json!({ "ref_path": ref_path }),
        });
        images.push(ExtractedImage {
            data,
            ext,
            source_location: serde_json::json!({ "media_index": idx + 1 }),
            alt: None,
            ref_path,
        });
        let _ = &mut current_line; // 尾部更新仅维护行号一致性
    }

    if markdown.trim().is_empty() && images.is_empty() {
        return Err(ParseError::Corrupted("docx has no text or images".into()));
    }

    let report = ParseReport {
        sections: mapping
            .iter()
            .filter(|m| m.mapping_type == pm_domain::docproc::MappingType::ParagraphToSection)
            .count(),
        tables: tables.len(),
        images: images.len(),
        ocr_pages: 0,
        warnings: warnings.len(),
        failures: 0,
        ocr_required_pages: Vec::new(),
    };

    Ok(ParseOutcome {
        markdown,
        images,
        tables,
        mapping,
        warnings,
        report,
    })
}

/// 行矩阵 → GFM 管道表（首行为表头）。
fn render_md_table(markdown: &mut String, rows: &[Vec<String>], cols: usize) {
    if cols == 0 {
        return;
    }
    for (i, row) in rows.iter().enumerate() {
        markdown.push('|');
        for c in 0..cols {
            markdown.push(' ');
            markdown.push_str(&md_cell(row.get(c).map(|s| s.as_str()).unwrap_or("")));
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_levels_parses_outline() {
        let xml = r##"<w:styles xmlns:w="x">
            <w:style w:styleId="Heading1"><w:outlineLvl w:val="0"/></w:style>
            <w:style w:styleId="2"><w:outlineLvl w:val="1"/></w:style>
        </w:styles>"##;
        let m = parse_style_levels(xml);
        assert_eq!(m.get("Heading1"), Some(&1));
        assert_eq!(m.get("2"), Some(&2));
    }
}
