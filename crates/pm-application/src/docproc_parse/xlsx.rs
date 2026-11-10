//! XLSX Parser Adapter（OOXML SpreadsheetML）。
//!
//! 策略（TD-03 §2.2）：每 worksheet → `## Sheet: <名>` 章节；sharedStrings
//! 解索引；公式取缓存值 + 警告；合并单元格首格取值 + 警告。

use super::{
    md_cell, open_zip_guarded, xml_attr, xml_text, ParseCtx, ParseError, ParseOutcome, ParseReport,
    ParseWarning, ParserAdapter, SourceMapping, TableInfo,
};
use quick_xml::events::Event;
use quick_xml::Reader;

pub struct XlsxAdapter;

impl ParserAdapter for XlsxAdapter {
    fn name(&self) -> &'static str {
        "xlsx"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn supports(&self, ext: &str, head: &[u8]) -> bool {
        ext.eq_ignore_ascii_case("xlsx") && head.starts_with(b"PK")
    }

    fn parse(&self, ctx: ParseCtx<'_>) -> Result<ParseOutcome, ParseError> {
        parse_xlsx(ctx.bytes)
    }
}

/// sharedStrings 表（`<si>` 内所有 `<t>` 拼接）。
fn load_shared_strings(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    budget: &mut super::ZipBudget,
) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(mut entry) = archive.by_name("xl/sharedStrings.xml") {
        if let Ok(xml_bytes) = budget.read_entry(&mut entry, "sharedStrings") {
            let xml = String::from_utf8_lossy(&xml_bytes).into_owned();
            let mut parser = Reader::from_str(&xml);
            parser.config_mut().trim_text(false);
            let mut cur = String::new();
            let mut in_si = false;
            let mut in_t = false;
            loop {
                match parser.read_event() {
                    Ok(Event::Start(e)) => match e.name().as_ref() {
                        "si" => {
                            cur.clear();
                            in_si = true;
                        }
                        "t" => in_t = true,
                        _ => {}
                    },
                    Ok(Event::End(e)) => match e.name().as_ref() {
                        "t" => in_t = false,
                        "si" => {
                            if in_si {
                                out.push(std::mem::take(&mut cur));
                            }
                            in_si = false;
                        }
                        _ => {}
                    },
                    Ok(Event::Text(t)) => {
                        if in_t && in_si {
                            cur.push_str(&xml_text(&t));
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        }
    }
    out
}

/// workbook.xml 的 sheet 清单（名称，按声明序）。
fn load_sheet_names(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    budget: &mut super::ZipBudget,
) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(mut entry) = archive.by_name("xl/workbook.xml") {
        if let Ok(xml_bytes) = budget.read_entry(&mut entry, "workbook") {
            let xml = String::from_utf8_lossy(&xml_bytes).into_owned();
            let mut parser = Reader::from_str(&xml);
            parser.config_mut().trim_text(true);
            loop {
                match parser.read_event() {
                    Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                        if e.name().as_ref() == "sheet" {
                            names.push(xml_attr(&e, "name").unwrap_or_default());
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        }
    }
    names
}

/// sheetN.xml 文件名自然排序键（sheet10 > sheet2）。
fn sheet_sort_key(name: &str) -> u32 {
    name.rsplit('/')
        .next()
        .and_then(|f| {
            f.trim_start_matches("sheet")
                .trim_end_matches(".xml")
                .parse()
                .ok()
        })
        .unwrap_or(u32::MAX)
}

/// A1 引用 → (row, col)（0-based）；无 r 属性时退化为顺序推进。
fn cell_rc(r: &str) -> (usize, usize) {
    let mut row = 0usize;
    let mut col = 0usize;
    let mut seen_row = false;
    for ch in r.chars() {
        if ch.is_ascii_alphabetic() {
            col = col * 26 + (ch.to_ascii_uppercase() as usize - 'A' as usize + 1);
        } else if ch.is_ascii_digit() {
            row = row * 10 + (ch as usize - '0' as usize);
            seen_row = true;
        }
    }
    if !seen_row {
        row = 0;
    }
    (row.saturating_sub(1), col.saturating_sub(1))
}

struct Cell {
    row: usize,
    col: usize,
    value: String,
    formula: bool,
}

fn parse_sheet(xml: &str, shared: &[String]) -> (Vec<Cell>, Vec<(usize, usize)>) {
    let mut parser = Reader::from_str(xml);
    parser.config_mut().trim_text(false);
    let mut cells = Vec::new();
    let mut merges = Vec::new();
    let mut cur_ref = String::new();
    let mut cur_type = String::new();
    let mut cur_value = String::new();
    let mut cur_formula = false;
    let mut in_v = false;
    let mut in_is = false;
    let mut is_text = String::new();
    loop {
        match parser.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                "c" => {
                    cur_ref = xml_attr(&e, "r").unwrap_or_default();
                    cur_type = xml_attr(&e, "t").unwrap_or_default();
                    cur_value.clear();
                    cur_formula = false;
                }
                "v" => {
                    in_v = true;
                    cur_value.clear();
                }
                "is" => {
                    in_is = true;
                    is_text.clear();
                }
                "f" => cur_formula = true,
                "mergeCell" => {
                    if let Some(r) = xml_attr(&e, "ref") {
                        // "A1:B2" → 左上角
                        let left = r.split(':').next().unwrap_or("");
                        let (row, col) = cell_rc(left);
                        merges.push((row, col));
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(e)) => {
                if e.name().as_ref() == "c" {
                    cur_ref = xml_attr(&e, "r").unwrap_or_default();
                    cur_type = xml_attr(&e, "t").unwrap_or_default();
                    cur_value.clear();
                    cur_formula = false;
                }
            }
            Ok(Event::Text(t)) => {
                if in_v {
                    cur_value.push_str(&xml_text(&t));
                } else if in_is {
                    is_text.push_str(&xml_text(&t));
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                "v" => in_v = false,
                "is" => {
                    in_is = false;
                    cur_value = is_text.clone();
                }
                "c" => {
                    let (row, col) = cell_rc(&cur_ref);
                    let value = match cur_type.as_str() {
                        "s" => shared
                            .get(cur_value.trim().parse::<usize>().unwrap_or(usize::MAX))
                            .cloned()
                            .unwrap_or_default(),
                        _ => cur_value.clone(),
                    };
                    if !value.trim().is_empty() || cur_formula {
                        cells.push(Cell {
                            row,
                            col,
                            value,
                            formula: cur_formula,
                        });
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    (cells, merges)
}

fn parse_xlsx(bytes: &[u8]) -> Result<ParseOutcome, ParseError> {
    let mut archive = open_zip_guarded(bytes, "xlsx")?;
    let mut budget = super::ZipBudget::new();
    if archive
        .file_names()
        .any(|n| n.eq_ignore_ascii_case("EncryptedPackage"))
    {
        return Err(ParseError::Encrypted("xlsx EncryptedPackage".into()));
    }

    let shared = load_shared_strings(&mut archive, &mut budget);
    let names = load_sheet_names(&mut archive, &mut budget);

    let mut sheet_files: Vec<String> = archive
        .file_names()
        .filter(|n| n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml"))
        .map(|n| n.to_string())
        .collect();
    sheet_files.sort_by_key(|n| sheet_sort_key(n));

    let mut markdown = String::new();
    let mut tables: Vec<TableInfo> = Vec::new();
    let mut mapping: Vec<SourceMapping> = Vec::new();
    let mut warnings: Vec<ParseWarning> = Vec::new();
    let mut formula_sheets = 0usize;

    for (sheet_idx, file) in sheet_files.iter().enumerate() {
        let name = names.get(sheet_idx).cloned().unwrap_or_else(|| {
            file.trim_start_matches("xl/worksheets/")
                .trim_end_matches(".xml")
                .to_string()
        });
        let mut entry = archive
            .by_name(file)
            .map_err(|e| ParseError::Corrupted(format!("xlsx missing {file}: {e}")))?;
        let xml_bytes = budget.read_entry(&mut entry, file)?;
        let xml = String::from_utf8_lossy(&xml_bytes).into_owned();
        let (cells, merges) = parse_sheet(&xml, &shared);

        if !markdown.is_empty() {
            markdown.push('\n');
        }
        markdown.push_str(&format!("## Sheet: {name}\n\n"));
        let heading_line = {
            // 行号 = markdown 现有行数 + 1
            markdown.lines().count()
        };

        if cells.is_empty() {
            markdown.push_str("_(empty worksheet)_\n\n");
            continue;
        }

        // 行矩阵化
        let max_row = cells.iter().map(|c| c.row).max().unwrap_or(0);
        let max_col = cells.iter().map(|c| c.col).max().unwrap_or(0);
        // 大表守卫（TD-03 §2.5 截断策略）
        let truncated = max_row + 1 > 10_000;
        let mut grid = vec![vec![String::new(); max_col + 1]; (max_row + 1).min(10_000)];
        for c in &cells {
            if c.row < grid.len() && c.col <= max_col {
                grid[c.row][c.col] = c.value.clone();
            }
        }
        if truncated {
            warnings.push(ParseWarning {
                code: "worksheet_truncated".into(),
                message: format!(
                    "sheet {name} has {} rows; rendered first 10000",
                    max_row + 1
                ),
                location: Some(serde_json::json!({ "sheet": name })),
            });
        }
        if !merges.is_empty() {
            warnings.push(ParseWarning {
                code: "merged_cells_flattened".into(),
                message: format!(
                    "sheet {name} has {} merged ranges; first cell keeps value",
                    merges.len()
                ),
                location: Some(serde_json::json!({ "sheet": name })),
            });
        }
        if cells.iter().any(|c| c.formula) {
            formula_sheets += 1;
        }

        let cols = max_col + 1;
        markdown.push('|');
        for cell in &grid[0] {
            markdown.push(' ');
            markdown.push_str(&md_cell(cell));
            markdown.push_str(" |");
        }
        markdown.push('\n');
        markdown.push('|');
        for _ in 0..cols {
            markdown.push_str(" --- |");
        }
        markdown.push('\n');
        for row in grid.iter().skip(1) {
            markdown.push('|');
            for cell in row {
                markdown.push(' ');
                markdown.push_str(&md_cell(cell));
                markdown.push_str(" |");
            }
            markdown.push('\n');
        }
        markdown.push('\n');

        tables.push(TableInfo {
            rows: max_row + 1,
            cols,
            fidelity: "markdown",
            location: Some(serde_json::json!({ "sheet": name })),
        });
        mapping.push(SourceMapping {
            mapping_type: pm_domain::docproc::MappingType::SheetToSection,
            source_location: serde_json::json!({ "sheet": name, "sheet_index": sheet_idx }),
            target_location: serde_json::json!({ "line_start": heading_line, "heading": format!("## Sheet: {name}") }),
        });
    }

    if markdown.trim().is_empty() {
        return Err(ParseError::Corrupted(
            "xlsx has no worksheet content".into(),
        ));
    }

    if formula_sheets > 0 {
        warnings.push(ParseWarning {
            code: "formula_cached_values".into(),
            message: "formula cells render cached values; formulas not preserved".into(),
            location: None,
        });
    }

    let report = ParseReport {
        sections: sheet_files.len(),
        tables: tables.len(),
        images: 0,
        ocr_pages: 0,
        warnings: warnings.len(),
        failures: 0,
        ocr_required_pages: Vec::new(),
    };

    Ok(ParseOutcome {
        markdown,
        images: Vec::new(),
        tables,
        mapping,
        warnings,
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a1_refs_parse() {
        assert_eq!(cell_rc("A1"), (0, 0));
        assert_eq!(cell_rc("B2"), (1, 1));
        assert_eq!(cell_rc("AA10"), (9, 26));
    }

    #[test]
    fn sheet_sort_key_numeric() {
        assert!(
            sheet_sort_key("xl/worksheets/sheet10.xml")
                > sheet_sort_key("xl/worksheets/sheet2.xml")
        );
    }
}
