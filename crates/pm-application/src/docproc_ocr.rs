//! OCR Adapter 接口（规格 D-04：V1 建接口与页面检测，引擎 P1）。
//!
//! V1 交付（规格 §2.4 五项）：
//! 1. 判断 PDF 是否包含可提取文本（[`DetectionOnlyOcrAdapter::detect`]）；
//! 2. 检测疑似扫描页面；
//! 3. 识别需要 OCR 的页面（`OcrPageStatus::needs_ocr` 清单）；
//! 4. 生成 OCR 处理任务所需的数据结构（页级状态表）；
//! 5. 记录 OCR 状态与处理结果（worker 落 `ocr_result` artifact）；
//! 6. 为 P1 OCR Adapter（OCRmyPDF 等）预留统一接口（本 trait）。
//!
//! **OCR 识别结果不能视为绝对准确的原文**（规格 §4.6）——P1 实现必须在
//! 结果里携带引擎与质量信息。

use crate::docproc_parse::{ParseError, ParserAdapter};

/// 页级 OCR 状态（规格 §2.4 数据结构的载体）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct OcrPageStatus {
    pub page: u64,
    /// not_required = 有可提取文本；pending = 需要 OCR（P1 引擎处理）
    pub status: &'static str,
}

/// 检测结果（V1 只有 pending/not_required 两态；P1 引擎追加
/// `ocr_text` / `quality` 字段进 artifact metadata）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct OcrDetection {
    pub engine: &'static str,
    pub engine_version: &'static str,
    pub pages: Vec<OcrPageStatus>,
}

/// OCR Adapter 统一接口（P1 引擎实现同一契约，worker 无感替换）。
pub trait OcrAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn version(&self) -> &'static str;
    /// 对原始文档字节产出页级 OCR 状态。V1 仅 PDF 有页级语义；
    /// 其他格式返回空清单（无需 OCR——文本天然可读）。
    fn detect(&self, bytes: &[u8]) -> Result<OcrDetection, ParseError>;
}

/// V1 实现：只做检测（复用 PDF adapter 的页级文本判定），不做识别。
pub struct DetectionOnlyOcrAdapter;

impl OcrAdapter for DetectionOnlyOcrAdapter {
    fn name(&self) -> &'static str {
        "detection-only"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn detect(&self, bytes: &[u8]) -> Result<OcrDetection, ParseError> {
        let head = &bytes[..bytes.len().min(8)];
        if head.starts_with(b"%PDF") {
            let outcome = crate::docproc_parse::pdf::PdfAdapter
                .parse(crate::docproc_parse::ParseCtx { bytes })?;
            let pages = outcome.report.ocr_required_pages;
            let total = outcome.report.sections as u64;
            let mut statuses = Vec::with_capacity(total as usize);
            for page in 1..=total {
                statuses.push(OcrPageStatus {
                    page,
                    status: if pages.contains(&page) {
                        "pending"
                    } else {
                        "not_required"
                    },
                });
            }
            return Ok(OcrDetection {
                engine: self.name(),
                engine_version: self.version(),
                pages: statuses,
            });
        }
        // 非 PDF：文本天然可读，无需 OCR
        Ok(OcrDetection {
            engine: self.name(),
            engine_version: self.version(),
            pages: Vec::new(),
        })
    }
}

/// 生产入口：P1 换引擎只改此函数（Adapter 替换点，规格 §15.9）。
pub fn ocr_adapter() -> &'static dyn OcrAdapter {
    &DetectionOnlyOcrAdapter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_marks_scanned_pages_pending() {
        // 无文本内容流的 PDF → 页面 pending（needs_ocr）
        let pdf = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
                    2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
                    3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n\
                    trailer\n<< /Root 1 0 R >>\n%%EOF\n";
        let detection = ocr_adapter().detect(pdf).unwrap();
        assert_eq!(detection.pages.len(), 1);
        assert_eq!(detection.pages[0].status, "pending");
    }

    #[test]
    fn detect_marks_text_pages_not_required() {
        let content = "BT /F1 12 Tf 72 720 Td (Hello) Tj ET";
        let pdf = format!(
            "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
             2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
             3 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 4 0 R >>\nendobj\n\
             4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n\
             trailer\n<< /Root 1 0 R >>\n%%EOF\n",
            content.len(),
            content
        )
        .into_bytes();
        let detection = ocr_adapter().detect(&pdf).unwrap();
        assert_eq!(detection.pages.len(), 1);
        assert_eq!(detection.pages[0].status, "not_required");
    }
}
