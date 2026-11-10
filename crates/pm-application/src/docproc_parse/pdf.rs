//! PDF 最小文本提取器（TD-03 §2.4 三级降级；零第三方 PDF 依赖，flate2 已在
//! 依赖树）。V1 边界（诚实声明，不静默）：
//! - 经典 xref 结构 + FlateDecode 流；页面树若藏在 object stream（PDF 1.5+）
//!   走顺序兜底（所有含 BT 的内容流按序成节），映射降为文档级 + 警告。
//! - 编码三层：ASCII 直映射 / ToUnicode CMap（bfchar+bfrange 合并）/
//!   兜底 U+FFFD 计警告（不静默丢字）。
//! - 每页文本≈0 → `ocr_required_pages`（D-04 V1 能力）。
//! - `/Encrypt` → 明确拒绝。

use super::{
    ParseCtx, ParseError, ParseOutcome, ParseReport, ParseWarning, ParserAdapter, SourceMapping,
};
use std::collections::HashMap;
use std::io::Read as _;

pub struct PdfAdapter;

impl ParserAdapter for PdfAdapter {
    fn name(&self) -> &'static str {
        "pdf"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn supports(&self, ext: &str, head: &[u8]) -> bool {
        ext.eq_ignore_ascii_case("pdf") && head.starts_with(b"%PDF")
    }

    fn parse(&self, ctx: ParseCtx<'_>) -> Result<ParseOutcome, ParseError> {
        parse_pdf(ctx.bytes)
    }
}

/* ── 对象层 ─────────────────────────────────────────────────────────────── */

/// 对象索引：objnum → (对象起始偏移, 对象结束偏移)。
fn scan_objects(data: &[u8]) -> Vec<(u64, usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = find(data, i, b" obj") {
        if let Some((objnum, hdr_start)) = parse_obj_header(data, pos) {
            let end = find(data, pos + 4, b"endobj")
                .unwrap_or(data.len())
                .min(data.len());
            out.push((objnum, hdr_start, end.max(pos)));
            i = end.max(pos + 4);
        } else {
            i = pos + 4;
        }
    }
    out
}

fn find(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from >= hay.len() || needle.is_empty() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    find(hay, 0, needle).is_some()
}

/// 解析 "N 0 obj" 头（obj_pos 指向 "obj" 起点）→ (objnum, 头起始偏移)。
fn parse_obj_header(data: &[u8], obj_pos: usize) -> Option<(u64, usize)> {
    let mut j = obj_pos;
    while j > 0 && data[j - 1].is_ascii_whitespace() {
        j -= 1;
    }
    if j == 0 || data[j - 1] != b'0' {
        return None;
    }
    j -= 1;
    while j > 0 && data[j - 1].is_ascii_whitespace() {
        j -= 1;
    }
    let num_end = j;
    while j > 0 && data[j - 1].is_ascii_digit() {
        j -= 1;
    }
    if num_end == j {
        return None;
    }
    let objnum: u64 = std::str::from_utf8(&data[j..num_end]).ok()?.parse().ok()?;
    Some((objnum, j))
}

/// 对象 dict 部分（"obj" 之后到 "stream" 或对象结束）。
fn dict_of<'a>(data: &'a [u8], span: &(u64, usize, usize)) -> &'a [u8] {
    let (_, start, end) = *span;
    let body_start = find(data, start, b"obj").map(|p| p + 3).unwrap_or(start);
    let body = &data[body_start.min(end)..end];
    match find(body, 0, b"stream") {
        Some(p) => &body[..p],
        None => body,
    }
}

/// 对象内容流：dict + (inflate 后的) 数据。
fn stream_of<'a>(data: &'a [u8], span: &(u64, usize, usize)) -> Option<(&'a [u8], Vec<u8>)> {
    let (_, start, end) = *span;
    let body_start = find(data, start, b"obj").map(|p| p + 3).unwrap_or(start);
    let body = &data[body_start.min(end)..end];
    let s = find(body, 0, b"stream")?;
    let dict = &body[..s];
    let mut dstart = s + 7;
    if body.get(dstart) == Some(&b'\r') {
        dstart += 1;
    }
    if body.get(dstart) == Some(&b'\n') {
        dstart += 1;
    }
    let dend = find(body, s, b"endstream").unwrap_or(body.len());
    let raw = &body[dstart.min(dend)..dend];
    let inflated = if contains(dict, b"/FlateDecode") {
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(raw)
            .read_to_end(&mut out)
            .ok()?;
        out
    } else {
        raw.to_vec()
    };
    Some((dict, inflated))
}

/// 从 dict 提取 "/Key N 0 R"（含数组形式）。
fn refs_after(dict: &[u8], key: &[u8]) -> Vec<u64> {
    let mut out = Vec::new();
    let Some(mut p) = find(dict, 0, key) else {
        return out;
    };
    p += key.len();
    if dict.get(p) == Some(&b'[') {
        p += 1;
    }
    let rest = &dict[p..dict.len().min(p + 512)];
    let mut i = 0;
    while i < rest.len() {
        while i < rest.len() && rest[i].is_ascii_whitespace() {
            i += 1;
        }
        let num_start = i;
        while i < rest.len() && rest[i].is_ascii_digit() {
            i += 1;
        }
        if i == num_start {
            break;
        }
        let Ok(num) = std::str::from_utf8(&rest[num_start..i])
            .unwrap_or("")
            .parse::<u64>()
        else {
            break;
        };
        while i < rest.len() && rest[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + 1 < rest.len() && rest[i] == b'0' && rest[i + 1].is_ascii_whitespace() {
            i += 2;
            while i < rest.len() && rest[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < rest.len() && rest[i] == b'R' {
                out.push(num);
                i += 1;
                continue;
            }
        }
        break;
    }
    out
}

/* ── ToUnicode CMap ─────────────────────────────────────────────────────── */

type Cmap = HashMap<u32, String>;

fn parse_cmap(bytes: &[u8]) -> Cmap {
    let mut map = Cmap::new();
    let text = String::from_utf8_lossy(bytes).into_owned();

    let collect = |tag: &str, map: &mut Cmap, range: bool| {
        let mut rest = text.as_str();
        let begin = format!("begin{tag}");
        let end = format!("end{tag}");
        while let Some(p) = rest.find(&begin) {
            let seg_end = rest[p..].find(&end).map(|e| p + e).unwrap_or(rest.len());
            let seg = &rest[p..seg_end];
            let toks = tokenize_hex(seg);
            if range {
                let mut i = 0;
                while i + 2 < toks.len() {
                    let (Ok(lo), Ok(hi)) = (
                        u32::from_str_radix(&toks[i], 16),
                        u32::from_str_radix(&toks[i + 1], 16),
                    ) else {
                        i += 3;
                        continue;
                    };
                    let dst = &toks[i + 2];
                    if hi >= lo && hi - lo < 65536 {
                        if let Ok(base) = u32::from_str_radix(dst, 16) {
                            for k in 0..=(hi - lo) {
                                map.entry(lo + k).or_insert_with(|| {
                                    utf16_hex_to_string(&format!("{base:X}"), k)
                                });
                            }
                        } else if dst.len() > 4 {
                            // dst 已是目标串（非数值 base）：整个范围同一值
                            let v = utf16_hex_to_string(dst, 0);
                            for k in 0..=(hi - lo) {
                                map.entry(lo + k).or_insert_with(|| v.clone());
                            }
                        }
                    }
                    i += 3;
                }
            } else {
                let it = toks.chunks(2);
                for pair in it {
                    if pair.len() != 2 {
                        break;
                    }
                    let src = pair[0].as_str();
                    let dst = pair[1].as_str();
                    if let Ok(code) = u32::from_str_radix(src, 16) {
                        map.entry(code)
                            .or_insert_with(|| utf16_hex_to_string(dst, 0));
                    }
                }
            }
            rest = &rest[seg_end..];
        }
    };
    collect("bfchar", &mut map, false);
    collect("bfrange", &mut map, true);
    map
}

/// 片段内的十六进制 token。
fn tokenize_hex(seg: &str) -> Vec<String> {
    let bytes = seg.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(close) = seg[i..].find('>') {
                let hexv: String = seg[i + 1..i + close]
                    .chars()
                    .filter(|c| c.is_ascii_hexdigit())
                    .collect();
                if !hexv.is_empty() {
                    out.push(hexv);
                }
                i += close + 1;
                continue;
            }
            break;
        }
        i += 1;
    }
    out
}

/// <hex>（UTF-16BE）→ String；bfrange 偏移 +k（数值 base 时）。
/// 长度非 UTF-16 对齐（如 3 字节）时按 UTF-8 字节解释（生成器偏差兜底）。
fn utf16_hex_to_string(hex: &str, offset: u32) -> String {
    let padded = if hex.len() % 2 == 1 {
        format!("{hex}0")
    } else {
        hex.to_string()
    };
    if padded.len() % 4 == 0 {
        let units: Vec<u16> = padded
            .as_bytes()
            .chunks(4)
            .filter_map(|c| u16::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok())
            .collect();
        if offset > 0 && !units.is_empty() {
            let mut u = units;
            let last = u.len() - 1;
            u[last] = u[last].saturating_add(offset as u16);
            return String::from_utf16_lossy(&u);
        }
        return String::from_utf16_lossy(&units);
    }
    // 字节路径（UTF-8）
    let bytes: Vec<u8> = (0..padded.len())
        .step_by(2)
        .filter_map(|k| u8::from_str_radix(&padded[k..k + 2], 16).ok())
        .collect();
    let s = String::from_utf8_lossy(&bytes).into_owned();
    if offset > 0 {
        let bumped = s
            .chars()
            .last()
            .and_then(|c| char::from_u32(c as u32 + offset))
            .map(|c| c.to_string())
            .unwrap_or_default();
        let head_len = s.len() - s.chars().last().map(|c| c.len_utf8()).unwrap_or(0);
        return format!("{}{}", &s[..head_len], bumped);
    }
    s
}

/* ── 内容流文本提取 ─────────────────────────────────────────────────────── */

struct TextStats {
    chars: usize,
    fffd: usize,
}

/// 解码 PDF 字符串字节：ASCII 直读；非 ASCII 走 CMap（先 2 字节后 1 字节）；
/// 未命中 → U+FFFD 计数（TD-03：不静默丢字）。
fn decode_bytes(bytes: &[u8], cmap: &Cmap) -> (String, usize) {
    if bytes.is_empty() {
        return (String::new(), 0);
    }
    if bytes
        .iter()
        .all(|&b| (0x20..=0x7E).contains(&b) || b == b'\t' || b == b'\n' || b == b'\r')
    {
        return (String::from_utf8_lossy(bytes).into_owned(), 0);
    }
    let mut best = String::new();
    let mut best_miss = usize::MAX;
    for width in [2usize, 1] {
        let mut s = String::new();
        let mut miss = 0usize;
        let mut i = 0;
        while i < bytes.len() {
            let (code, adv) = if width == 2 && i + 1 < bytes.len() {
                (((bytes[i] as u32) << 8) | bytes[i + 1] as u32, 2)
            } else {
                (bytes[i] as u32, 1)
            };
            i += adv;
            match cmap.get(&code) {
                Some(ch) => s.push_str(ch),
                None => {
                    miss += 1;
                    s.push('\u{FFFD}');
                }
            }
        }
        if miss < best_miss {
            best_miss = miss;
            best = s;
        }
        if best_miss == 0 {
            break;
        }
    }
    (best, best_miss)
}

fn emit(out: &mut String, raw: &[u8], cmap: &Cmap, stats: &mut TextStats) {
    let (decoded, miss) = decode_bytes(raw, cmap);
    stats.chars += decoded.chars().count();
    stats.fffd += miss;
    out.push_str(&decoded);
}

/// 从 inflate 后的内容流提取文本（最小算子集）。
fn extract_stream_text(data: &[u8], cmap: &Cmap, stats: &mut TextStats) -> String {
    let mut out = String::new();
    let mut pending: Vec<String> = Vec::new();
    let mut last_string: Option<Vec<u8>> = None;
    let mut i = 0usize;

    macro_rules! newline {
        () => {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
        };
    }

    while i < data.len() {
        let c = data[i];
        match c {
            b'(' => {
                let (s, next) = read_literal(data, i);
                last_string = Some(s);
                i = next;
            }
            b'<' if data.get(i + 1) != Some(&b'<') => {
                let (s, next) = read_hex(data, i);
                last_string = Some(s);
                i = next;
            }
            b'[' => {
                pending.clear();
                i += 1;
            }
            b']' => {
                i += 1;
                let (matched, next) = skip_ws_and_match(data, i, b"TJ");
                if matched {
                    i = next;
                }
                if !pending.is_empty() {
                    out.push_str(&pending.concat());
                    pending.clear();
                }
            }
            b'/' => {
                while i < data.len() && !data[i].is_ascii_whitespace() {
                    i += 1;
                }
            }
            b'%' => {
                while i < data.len() && data[i] != b'\n' {
                    i += 1;
                }
            }
            c if c.is_ascii_digit() || c == b'-' || c == b'+' || c == b'.' => {
                while i < data.len()
                    && (data[i].is_ascii_digit()
                        || data[i] == b'-'
                        || data[i] == b'+'
                        || data[i] == b'.')
                {
                    i += 1;
                }
            }
            c if c.is_ascii_alphabetic() || c == b'\'' || c == b'"' => {
                let start = i;
                while i < data.len()
                    && (data[i].is_ascii_alphabetic() || data[i] == b'\'' || data[i] == b'"')
                {
                    i += 1;
                }
                match &data[start..i] {
                    b"Tj" => {
                        if let Some(s) = last_string.take() {
                            emit(&mut out, &s, cmap, stats);
                        }
                    }
                    b"'" | b"\"" => {
                        newline!();
                        if let Some(s) = last_string.take() {
                            emit(&mut out, &s, cmap, stats);
                        }
                    }
                    b"TJ" => {
                        if !pending.is_empty() {
                            out.push_str(&pending.concat());
                            pending.clear();
                        }
                    }
                    b"Td" | b"TD" | b"Tm" => newline!(),
                    b"T*" => newline!(),
                    b"BT" | b"ET" => newline!(),
                    _ => {}
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// 读取字面量字符串（嵌套括号 + 转义）→ 原始字节。
fn read_literal(data: &[u8], start: usize) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut i = start + 1;
    let mut depth = 1;
    while i < data.len() {
        let c = data[i];
        match c {
            b'\\' => {
                i += 1;
                if i >= data.len() {
                    break;
                }
                match data[i] {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'b' | b'f' => out.push(b' '),
                    b'(' | b')' | b'\\' => out.push(data[i]),
                    b'0'..=b'7' => {
                        let mut v = 0u32;
                        let mut n = 0;
                        while n < 3 && i < data.len() && (b'0'..=b'7').contains(&data[i]) {
                            v = v * 8 + (data[i] - b'0') as u32;
                            i += 1;
                            n += 1;
                        }
                        i -= 1;
                        out.push((v & 0xFF) as u8);
                    }
                    b'\n' => {}
                    _ => out.push(data[i]),
                }
                i += 1;
            }
            b'(' => {
                depth += 1;
                out.push(c);
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    break;
                }
                out.push(c);
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    (out, i)
}

/// 读取十六进制字符串 "<...>" → 字节。
fn read_hex(data: &[u8], start: usize) -> (Vec<u8>, usize) {
    let mut i = start + 1;
    let mut hexv = String::new();
    while i < data.len() && data[i] != b'>' {
        if data[i].is_ascii_hexdigit() {
            hexv.push(data[i] as char);
        }
        i += 1;
    }
    i += 1;
    if hexv.len() % 2 == 1 {
        hexv.push('0');
    }
    let bytes: Vec<u8> = (0..hexv.len())
        .step_by(2)
        .filter_map(|k| u8::from_str_radix(&hexv[k..k + 2], 16).ok())
        .collect();
    (bytes, i)
}

fn skip_ws_and_match(data: &[u8], from: usize, token: &[u8]) -> (bool, usize) {
    let mut i = from;
    while i < data.len() && data[i].is_ascii_whitespace() {
        i += 1;
    }
    if data[i..].starts_with(token) {
        (true, i + token.len())
    } else {
        (false, i)
    }
}

/* ── 主流程 ─────────────────────────────────────────────────────────────── */

fn parse_pdf(bytes: &[u8]) -> Result<ParseOutcome, ParseError> {
    if contains(bytes, b"/Encrypt") {
        return Err(ParseError::Encrypted("pdf /Encrypt".into()));
    }
    let objs = scan_objects(bytes);

    // ToUnicode CMap 全量收集（V1：跨字体合并，冲突按首次命中）
    let mut cmap: Cmap = Cmap::new();
    let mut to_unicode_refs: Vec<u64> = Vec::new();
    for span in &objs {
        let dict = dict_of(bytes, span);
        to_unicode_refs.extend(refs_after(dict, b"/ToUnicode"));
    }
    for refnum in &to_unicode_refs {
        if let Some(span) = objs.iter().find(|(n, _, _)| n == refnum) {
            if let Some((_, stream)) = stream_of(bytes, span) {
                for (k, v) in parse_cmap(&stream) {
                    cmap.entry(k).or_insert(v);
                }
            }
        }
    }

    // 页面收集：优先 /Type /Page 对象（objnum 序）；无则内容流兜底。
    let mut page_spans: Vec<(u64, usize, usize)> = objs
        .iter()
        .filter(|s| is_page_dict(dict_of(bytes, s)))
        .copied()
        .collect();
    page_spans.sort_by_key(|s| s.0);

    let mut warnings: Vec<ParseWarning> = Vec::new();
    let content_streams: Vec<Vec<u8>> = if !page_spans.is_empty() {
        page_spans
            .iter()
            .filter_map(|s| {
                let dict = dict_of(bytes, s);
                let mut merged: Option<Vec<u8>> = None;
                for r in refs_after(dict, b"/Contents") {
                    if let Some(ospan) = objs.iter().find(|(n, _, _)| n == &r) {
                        if let Some((_, d)) = stream_of(bytes, ospan) {
                            merged = Some(match merged {
                                Some(mut m) => {
                                    m.extend(d);
                                    m
                                }
                                None => d,
                            });
                        }
                    }
                }
                merged
            })
            .collect()
    } else {
        warnings.push(ParseWarning {
            code: "pdf_page_tree_unresolved".into(),
            message: concat!(
                "page tree not found (likely object streams); sequential content ",
                "streams used — page mapping approximated"
            )
            .into(),
            location: None,
        });
        objs.iter()
            .filter_map(|s| stream_of(bytes, s))
            .filter(|(_, d)| contains(d, b"BT"))
            .map(|(_, d)| d)
            .collect()
    };

    if content_streams.is_empty() {
        return Err(ParseError::Corrupted(
            "pdf: no content streams found (not a text PDF?)".into(),
        ));
    }

    let mut markdown = String::new();
    let mut mapping: Vec<SourceMapping> = Vec::new();
    let mut ocr_pages: Vec<u64> = Vec::new();
    let mut stats = TextStats { chars: 0, fffd: 0 };
    let mut page_no = 0u64;

    for stream in &content_streams {
        page_no += 1;
        let text = extract_stream_text(stream, &cmap, &mut stats);
        markdown.push_str(&format!("## Page {page_no}\n\n"));
        let line_start = markdown.lines().count();
        let text_trimmed = text.trim();
        if text_trimmed.is_empty() {
            ocr_pages.push(page_no);
            markdown.push_str("_(no extractable text — likely scanned page)_\n\n");
        } else {
            markdown.push_str(text_trimmed);
            markdown.push_str("\n\n");
            mapping.push(SourceMapping {
                mapping_type: pm_domain::docproc::MappingType::PageToSection,
                source_location: serde_json::json!({ "page": page_no }),
                target_location: serde_json::json!({
                    "line_start": line_start,
                    "heading": format!("## Page {page_no}")
                }),
            });
        }
    }

    if stats.chars > 0 && stats.fffd * 20 > stats.chars {
        warnings.push(ParseWarning {
            code: "font_encoding_partial".into(),
            message: format!(
                "{} of {} glyphs unresolved (missing ToUnicode CMap)",
                stats.fffd, stats.chars
            ),
            location: None,
        });
    }
    if !ocr_pages.is_empty() {
        warnings.push(ParseWarning {
            code: "needs_ocr".into(),
            message: format!("{} page(s) with no extractable text", ocr_pages.len()),
            location: Some(serde_json::json!({ "pages": ocr_pages })),
        });
    }

    let report = ParseReport {
        sections: page_no as usize,
        tables: 0,
        images: 0,
        ocr_pages: ocr_pages.len(),
        warnings: warnings.len(),
        failures: 0,
        ocr_required_pages: ocr_pages,
    };

    Ok(ParseOutcome {
        markdown,
        images: Vec::new(),
        tables: Vec::new(),
        mapping,
        warnings,
        report,
    })
}

/// `/Type /Page`（排除 `/Pages`）。
fn is_page_dict(dict: &[u8]) -> bool {
    let Some(p) = find(dict, 0, b"/Type") else {
        return false;
    };
    let rest = &dict[p..dict.len().min(p + 32)];
    contains(rest, b"/Page") && !contains(rest, b"/Pages")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_string_read() {
        let data = b"(Hello \\(World\\)) Tj";
        let (s, next) = read_literal(data, 0);
        assert_eq!(&s, b"Hello (World)");
        assert_eq!(&data[next..], b" Tj");
    }

    #[test]
    fn cmap_bfchar_parses() {
        // 中 = U+4E2D → UTF-16BE "4E2D"
        let m = parse_cmap(b"beginbfchar\n<0041> <4E2D>\nendbfchar");
        assert_eq!(m.get(&0x0041), Some(&"中".to_string()));
    }

    #[test]
    fn cmap_dst_utf8_fallback() {
        // 非 UTF-16 对齐长度（3 字节）→ UTF-8 兜底（E4 B8 AD = 中）
        let m = parse_cmap(b"beginbfchar\n<0041> <E4B8AD>\nendbfchar");
        assert_eq!(m.get(&0x0041), Some(&"中".to_string()));
    }

    #[test]
    fn cmap_bfrange_parses() {
        // base U+4E80，range +2 → 0x0042 映射 U+4E81
        let m = parse_cmap(b"beginbfrange\n<0041> <0043> <4E80>\nendbfrange");
        assert_eq!(m.get(&0x0042), Some(&"\u{4E81}".to_string()));
    }

    #[test]
    fn obj_headers_scan() {
        let data = b"1 0 obj\n<< /Type /Catalog >>\nendobj\n2 0 obj\n<< >>\nendobj";
        let spans = scan_objects(data);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].0, 1);
        assert!(is_page_dict(b"<< /Type /Page /Kids [] >>"));
        assert!(!is_page_dict(b"<< /Type /Pages /Kids [] >>"));
    }

    #[test]
    fn extract_ascii_stream() {
        let stream = b"BT /F1 12 Tf 72 720 Td (Hello World) Tj 0 -14 Td (Line Two) Tj ET";
        let mut stats = TextStats { chars: 0, fffd: 0 };
        let text = extract_stream_text(stream, &Cmap::new(), &mut stats);
        assert!(text.contains("Hello World"));
        assert!(text.contains("Line Two"));
        assert!(text.lines().count() >= 2);
    }

    #[test]
    fn decode_prefers_cmap_hit() {
        let mut cmap = Cmap::new();
        cmap.insert(0x0041, "中".into());
        let (s, miss) = decode_bytes(&[0x00, 0x41, 0x00, 0x42], &cmap);
        assert_eq!(s, "中\u{FFFD}");
        assert_eq!(miss, 1);
    }
}
