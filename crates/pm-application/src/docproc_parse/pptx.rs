//! PPTX Parser Adapter（OOXML PresentationML）。
//!
//! 策略（TD-03 §2.3）：每页 → `## Slide N: <标题>`；shape 文本按序合并；
//! `p:pic` → rels → media 图片提取；备注 → 引用块。

use super::{
    image_ref_path, open_zip_guarded, xml_attr, xml_text, ExtractedImage, ParseCtx, ParseError,
    ParseOutcome, ParseReport, ParserAdapter, SourceMapping, ZipBudget,
};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::Read as _;

pub struct PptxAdapter;

impl ParserAdapter for PptxAdapter {
    fn name(&self) -> &'static str {
        "pptx"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn supports(&self, ext: &str, head: &[u8]) -> bool {
        ext.eq_ignore_ascii_case("pptx") && head.starts_with(b"PK")
    }

    fn parse(&self, ctx: ParseCtx<'_>) -> Result<ParseOutcome, ParseError> {
        parse_pptx(ctx.bytes)
    }
}

/// slideN.xml 文件名排序键（slide10 > slide2）。
fn slide_num(name: &str) -> u32 {
    name.rsplit('/')
        .next()
        .and_then(|f| {
            f.trim_start_matches("slide")
                .trim_end_matches(".xml")
                .parse()
                .ok()
        })
        .unwrap_or(u32::MAX)
}

/// rels 文件（同目录 _rels/*.rels）→ {rId → target}。
fn load_slide_rels(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    budget: &mut ZipBudget,
    slide: &str,
) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let rel_path = format!(
        "ppt/slides/_rels/{name}.rels",
        name = slide.rsplit('/').next().unwrap_or(slide)
    );
    if let Ok(mut entry) = archive.by_name(&rel_path) {
        if let Ok(xml_bytes) = budget.read_entry(&mut entry, &rel_path) {
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

struct SlideText {
    title: Option<String>,
    body: Vec<String>,
    #[allow(dead_code)]
    images: Vec<(String, String)>, // (rId, alt)
}

/// 提取单页文本结构（shape 顺序；ph type=title/ctrTitle → 标题）。
fn parse_slide(xml: &str) -> SlideText {
    let mut parser = Reader::from_str(xml);
    parser.config_mut().trim_text(false);
    let mut out = SlideText {
        title: None,
        body: Vec::new(),
        images: Vec::new(),
    };
    let mut cur_shape_is_title = false;
    let mut in_pic = false;
    let mut para = String::new();
    let mut in_txbody = false;
    loop {
        match parser.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                "p:sp" => {
                    cur_shape_is_title = false;
                    para.clear();
                }
                "p:ph" => {
                    let t = xml_attr(&e, "type").unwrap_or_default();
                    if t == "title" || t == "ctrTitle" {
                        cur_shape_is_title = true;
                    }
                }
                "p:txBody" => in_txbody = true,
                "a:p" => para.clear(),
                "p:pic" => in_pic = true,
                _ => {}
            },
            Ok(Event::Empty(e)) => {
                if e.name().as_ref() == "p:ph" {
                    let t = xml_attr(&e, "type").unwrap_or_default();
                    if t == "title" || t == "ctrTitle" {
                        cur_shape_is_title = true;
                    }
                }
            }
            Ok(Event::Text(t)) => {
                if in_txbody && !in_pic {
                    para.push_str(&xml_text(&t));
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                "a:p" => {
                    let content = para.trim().to_string();
                    if !content.is_empty() {
                        if cur_shape_is_title && out.title.is_none() {
                            out.title = Some(content);
                        } else {
                            out.body.push(content);
                        }
                    }
                    para.clear();
                }
                "p:sp" => cur_shape_is_title = false,
                "p:txBody" => in_txbody = false,
                "p:pic" => in_pic = false,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    out
}

fn parse_pptx(bytes: &[u8]) -> Result<ParseOutcome, ParseError> {
    let mut archive = open_zip_guarded(bytes, "pptx")?;
    let mut budget = ZipBudget::new();
    if archive
        .file_names()
        .any(|n| n.eq_ignore_ascii_case("EncryptedPackage"))
    {
        return Err(ParseError::Encrypted("pptx EncryptedPackage".into()));
    }

    let mut slide_files: Vec<String> = archive
        .file_names()
        .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
        .map(|n| n.to_string())
        .collect();
    slide_files.sort_by_key(|n| slide_num(n));

    if slide_files.is_empty() {
        return Err(ParseError::Corrupted("pptx has no slides".into()));
    }

    let mut markdown = String::new();
    let mut images: Vec<ExtractedImage> = Vec::new();
    let mut mapping: Vec<SourceMapping> = Vec::new();
    let mut image_seq = 0usize;

    for (idx, file) in slide_files.iter().enumerate() {
        let slide_no = idx + 1;
        let xml_bytes = {
            let mut entry = archive
                .by_name(file)
                .map_err(|e| ParseError::Corrupted(format!("pptx missing {file}: {e}")))?;
            budget.read_entry(&mut entry, file)?
        };
        let xml = String::from_utf8_lossy(&xml_bytes).into_owned();
        let slide = parse_slide(&xml);

        if !markdown.is_empty() {
            markdown.push('\n');
        }
        let heading_line = markdown.lines().count() + 1;
        match slide.title.as_deref() {
            Some(t) => markdown.push_str(&format!("## Slide {slide_no}: {t}\n\n")),
            None => markdown.push_str(&format!("## Slide {slide_no}\n\n")),
        }
        mapping.push(SourceMapping {
            mapping_type: pm_domain::docproc::MappingType::SlideToSection,
            source_location: serde_json::json!({ "slide": slide_no }),
            target_location: serde_json::json!({
                "line_start": heading_line,
                "heading": slide.title.as_deref().map(|t| format!("## Slide {slide_no}: {t}"))
                    .unwrap_or_else(|| format!("## Slide {slide_no}"))
            }),
        });

        for para in &slide.body {
            markdown.push_str(para);
            markdown.push_str("\n\n");
        }

        // 图片：p:pic 的 blip embed → rels → media
        let rels = load_slide_rels(&mut archive, &mut budget, file);
        let slide_xml = xml.clone();
        let mut blip_rids: Vec<String> = Vec::new();
        {
            let mut parser = Reader::from_str(&slide_xml);
            parser.config_mut().trim_text(false);
            loop {
                match parser.read_event() {
                    Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                        if e.name().as_ref() == "a:blip" {
                            if let Some(rid) = xml_attr(&e, "r:embed") {
                                blip_rids.push(rid);
                            }
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        }
        for rid in blip_rids {
            let Some(target) = rels.get(&rid) else {
                continue;
            };
            // target 形如 ../media/image1.png → ppt/media/image1.png
            let media_path = format!("ppt/{}", target.trim_start_matches("../"));
            let ext = media_path
                .rsplit('.')
                .next()
                .unwrap_or("bin")
                .to_ascii_lowercase();
            if let Ok(mut entry) = archive.by_name(&media_path) {
                let mut data = Vec::new();
                if entry.read_to_end(&mut data).is_ok() && !data.is_empty() {
                    image_seq += 1;
                    let ref_path = image_ref_path(image_seq, &ext);
                    markdown.push_str(&format!("![image-{image_seq:03}]({ref_path})\n\n"));
                    images.push(ExtractedImage {
                        data,
                        ext,
                        source_location: serde_json::json!({ "slide": slide_no }),
                        alt: None,
                        ref_path,
                    });
                    mapping.push(SourceMapping {
                        mapping_type: pm_domain::docproc::MappingType::AssetRef,
                        source_location: serde_json::json!({ "slide": slide_no }),
                        target_location: serde_json::json!({ "media_index": image_seq }),
                    });
                }
            }
        }

        // 备注（可选）
        let notes_path = format!(
            "ppt/notesSlides/notesSlide{}.xml",
            file.rsplit('/')
                .next()
                .unwrap_or("")
                .trim_start_matches("slide")
                .trim_end_matches(".xml")
        );
        if let Ok(mut entry) = archive.by_name(&notes_path) {
            if let Ok(notes_bytes) = budget.read_entry(&mut entry, &notes_path) {
                let notes_xml = String::from_utf8_lossy(&notes_bytes).into_owned();
                let mut notes_text = String::new();
                let mut parser = Reader::from_str(&notes_xml);
                parser.config_mut().trim_text(false);
                loop {
                    match parser.read_event() {
                        Ok(Event::Text(t)) => notes_text.push_str(&xml_text(&t)),
                        Ok(Event::Eof) => break,
                        Err(_) => break,
                        _ => {}
                    }
                }
                let notes = notes_text.trim();
                if !notes.is_empty() {
                    for line in notes.lines().filter(|l| !l.trim().is_empty()) {
                        markdown.push_str("> ");
                        markdown.push_str(line.trim());
                        markdown.push('\n');
                    }
                    markdown.push('\n');
                }
            }
        }
    }

    let report = ParseReport {
        sections: slide_files.len(),
        tables: 0,
        images: images.len(),
        ocr_pages: 0,
        warnings: 0,
        failures: 0,
        ocr_required_pages: Vec::new(),
    };

    Ok(ParseOutcome {
        markdown,
        images,
        tables: Vec::new(),
        mapping,
        warnings: Vec::new(),
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_order_numeric() {
        assert!(slide_num("ppt/slides/slide10.xml") > slide_num("ppt/slides/slide2.xml"));
    }
}
