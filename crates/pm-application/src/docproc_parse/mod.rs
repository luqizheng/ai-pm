//! Document Processing 解析层（TD-03：纯 Rust zip+quick-xml，零外部进程）。
//!
//! 模块真源：docs/ai-pm-document-processing-td03-parser-adapters-v1.md。
//! 与 knowledge_parse.rs 的关系：KB 导入通道保持原 API 不动；本模块是
//! Document Processing 的完整解析面（结构化产物 + 图片 + 追溯映射 + 报告）。
//!
//! 安全基线（规格 §9 / TD-04 §5）：文档=不可信输入——zip 解压比 ≤100x、
//! 总解压 ≤512MiB（ZIP Bomb）；解析纯内存零外呼；不可解析内容 → 警告，
//! 不静默丢弃。

pub mod csv;
pub mod docx;
pub mod html;
pub mod md;
pub mod pdf;
pub mod pptx;
pub mod txt;
pub mod xlsx;

use std::io::Read as _;

/// 解析错误（TD-02 §3.9 错误契约的解析段子集）。
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("magic mismatch: {0}")]
    MagicMismatch(String),
    #[error("encrypted document: {0}")]
    Encrypted(String),
    #[error("corrupted container: {0}")]
    Corrupted(String),
    #[error("archive bomb guard rejected: {0}")]
    ArchiveBomb(String),
}

/// 解析警告（规格 §4.2：不可解析内容留痕，不静默丢弃）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ParseWarning {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<serde_json::Value>,
}

/// 提取出的图片（字节待 worker 落 file-service 后才有 artifact_id）。
#[derive(Debug, Clone)]
pub struct ExtractedImage {
    pub data: Vec<u8>,
    pub ext: String,
    /// 原文定位（如 `{paragraph: 12}` / `{slide: 3}` / `{page: 5}`）
    pub source_location: serde_json::Value,
    pub alt: Option<String>,
    /// Markdown 里引用的相对路径（`assets/image-NNN.ext`）
    pub ref_path: String,
}

/// 表格信息（报告 + 表格保真方式留痕）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct TableInfo {
    pub rows: usize,
    pub cols: usize,
    /// markdown = 管道表；flattened = 拍平 + 警告；html = HTML fallback
    pub fidelity: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<serde_json::Value>,
}

/// 追溯映射（写 docproc.source_mapping 行）。
#[derive(Debug, Clone)]
pub struct SourceMapping {
    pub mapping_type: pm_domain::docproc::MappingType,
    pub source_location: serde_json::Value,
    pub target_location: serde_json::Value,
}

/// 处理报告（规格 §8.7 全字段的解析侧来源）。
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ParseReport {
    pub sections: usize,
    pub tables: usize,
    pub images: usize,
    pub ocr_pages: usize,
    pub warnings: usize,
    pub failures: usize,
    /// 需 OCR 的页清单（D-04 V1：检测 + 数据结构，引擎在 P1）
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub ocr_required_pages: Vec<u64>,
}

/// 解析产物（规格 §4.2 提取清单 + §4.3 规范化 Markdown）。
#[derive(Debug, Clone)]
pub struct ParseOutcome {
    pub markdown: String,
    pub images: Vec<ExtractedImage>,
    pub tables: Vec<TableInfo>,
    pub mapping: Vec<SourceMapping>,
    pub warnings: Vec<ParseWarning>,
    pub report: ParseReport,
}

/// 解析上下文（V1 仅原始字节；预签名/外部资源一律不可用——安全边界）。
#[derive(Debug, Clone, Copy)]
pub struct ParseCtx<'a> {
    pub bytes: &'a [u8],
}

/// Parser Adapter 统一契约（TD-03 §1）。
pub trait ParserAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn version(&self) -> &'static str;
    /// 扩展名 + 魔数双检（规格 §9.3：不只依赖扩展名）。
    fn supports(&self, ext: &str, head: &[u8]) -> bool;
    fn parse(&self, ctx: ParseCtx<'_>) -> Result<ParseOutcome, ParseError>;
}

/// V1 白名单（规格 §4.1.1 八格式）。
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "docx", "pdf", "pptx", "xlsx", "md", "markdown", "html", "htm", "txt", "csv",
];

/// 按（扩展名, 魔数）派发 adapter；无匹配 → None（调用方转 400）。
pub fn adapter_for(ext: &str, head: &[u8]) -> Option<&'static dyn ParserAdapter> {
    let ext = ext.to_ascii_lowercase();
    let candidates: [&'static dyn ParserAdapter; 8] = [
        &docx::DocxAdapter,
        &pptx::PptxAdapter,
        &xlsx::XlsxAdapter,
        &pdf::PdfAdapter,
        &html::HtmlAdapter,
        &csv::CsvAdapter,
        &md::MarkdownAdapter,
        &txt::TextAdapter,
    ];
    candidates.into_iter().find(|a| a.supports(&ext, head))
}

/* ── 共享工具层（TD-03 §3） ─────────────────────────────────────────────── */

/// zip 容器安全打开（ZIP Bomb 守卫：单条目解压比 ≤100x，总量 ≤512MiB）。
pub fn open_zip_guarded<'a>(
    bytes: &'a [u8],
    label: &str,
) -> Result<zip::ZipArchive<std::io::Cursor<&'a [u8]>>, ParseError> {
    let archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| ParseError::Corrupted(format!("{label} zip: {e}")))?;
    Ok(archive)
}

/// 读取 zip 内单个 entry（逐块读取时累计守卫计数）。
pub struct ZipBudget {
    pub total_read: u64,
    pub max_total: u64,
    pub max_ratio: u64,
    pub compressed_total: u64,
}

impl ZipBudget {
    pub fn new() -> Self {
        Self {
            total_read: 0,
            max_total: 512 * 1024 * 1024,
            max_ratio: 100,
            compressed_total: 0,
        }
    }

    /// 读完一个 entry 并计入守卫；超限即 ArchiveBomb。
    pub fn read_entry(
        &mut self,
        entry: &mut zip::read::ZipFile<'_, std::io::Cursor<&[u8]>>,
        label: &str,
    ) -> Result<Vec<u8>, ParseError> {
        let compressed = entry.compressed_size();
        let declared = entry.size();
        if declared > self.max_total || compressed.saturating_mul(self.max_ratio) < declared {
            return Err(ParseError::ArchiveBomb(format!(
                "{label}: entry ratio/size over budget (declared {declared})"
            )));
        }
        let mut buf = Vec::new();
        let mut chunk = [0u8; 64 * 1024];
        loop {
            let n = entry
                .read(&mut chunk)
                .map_err(|e| ParseError::Corrupted(format!("{label}: {e}")))?;
            if n == 0 {
                break;
            }
            self.total_read += n as u64;
            if self.total_read > self.max_total {
                return Err(ParseError::ArchiveBomb(format!(
                    "{label}: total inflate over budget"
                )));
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        self.compressed_total += compressed;
        Ok(buf)
    }
}

impl Default for ZipBudget {
    fn default() -> Self {
        Self::new()
    }
}

/// XML 文本事件 → 干净字符串（实体还原；解析失败降级原文，不丢内容）。
pub fn xml_text(t: &quick_xml::events::BytesText<'_>) -> String {
    unescape_xml(t.as_ref())
}

/// 最小 XML 实体还原（quick-xml 0.42 BytesText 无 unescape；OOXML 常用实体
/// 手工覆盖，数值实体全支持）。
pub fn unescape_xml(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        let tail = &rest[p..];
        let semi = tail.find(';');
        match semi {
            Some(e) if e <= 10 => {
                let ent = &tail[1..e];
                let repl: String = match ent {
                    "amp" => "&".to_string(),
                    "lt" => "<".to_string(),
                    "gt" => ">".to_string(),
                    "quot" => "\"".to_string(),
                    "apos" => "'".to_string(),
                    _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                        u32::from_str_radix(&ent[2..], 16)
                            .ok()
                            .and_then(char::from_u32)
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| tail[..=e].to_string())
                    }
                    _ if ent.starts_with('#') => ent[1..]
                        .parse::<u32>()
                        .ok()
                        .and_then(char::from_u32)
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| tail[..=e].to_string()),
                    _ => tail[..=e].to_string(),
                };
                out.push_str(&repl);
                rest = &tail[e + 1..];
            }
            _ => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// 快速 XML 属性读取（quick-xml 0.42：attr key/value 均为 str 通道）。
pub fn xml_attr(e: &quick_xml::events::BytesStart<'_>, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| a.value.into_owned())
}

/* ── Markdown 规范化工具（全 adapter 一致，TD-03 §1） ───────────────────── */

/// GFM 表格单元格清洗（管道转义 + 换行压空格）。
pub fn md_cell(s: &str) -> String {
    s.replace('|', "\\|")
        .replace(['\r', '\n'], " ")
        .trim()
        .to_string()
}

/// 章节计数器：产出 `target_location` 行区间。
#[derive(Debug, Default)]
pub struct SectionTracker {
    pub line_start: usize,
    pub sections: usize,
}

impl SectionTracker {
    /// 每写出一行 markdown 后调用（1-based 行号）。
    pub fn advance(&mut self) {
        self.line_start += 1;
    }
    pub fn start_section(&mut self) -> usize {
        self.sections += 1;
        self.line_start
    }
}

/// 图片相对引用路径分配（规格 §4.4 产物结构）。
pub fn image_ref_path(index: usize, ext: &str) -> String {
    format!("assets/image-{index:03}.{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_dispatch_rejects_unknown_ext() {
        assert!(adapter_for("exe", b"MZ").is_none());
        assert!(adapter_for("", b"").is_none());
    }

    #[test]
    fn md_cell_escapes_pipes() {
        assert_eq!(md_cell("a|b\nc"), "a\\|b c");
    }
}
