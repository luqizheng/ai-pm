//! Document Processing 域类型（规格 docs/ai-pm-document-processing-v1.md §5/§6；
//! 数据模型真源 docs/ai-pm-document-processing-td01-data-model-v1.md）。
//!
//! 枚举值与迁移 0016 的 CHECK 约束一一对应；任何一侧改动必须同步另一侧。

/// 转换任务类型（`docproc.processing_job.processing_type`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingType {
    Parse,
    Compare,
    Export,
    OcrDetect,
}

impl ProcessingType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Compare => "compare",
            Self::Export => "export",
            Self::OcrDetect => "ocr_detect",
        }
    }
}

/// 任务状态（规格 §5.2 六态；迁移 0016 `job_status_check`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    PartialSuccess,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::PartialSuccess => "partial_success",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// 终态判定（failed 仍可重试回 pending，但作为状态本身是终态）。
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::PartialSuccess | Self::Failed | Self::Cancelled
        )
    }
}

/// 转换产物类型（规格 §6.4 七值；迁移 0016 `artifact_type_check`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactType {
    Markdown,
    ExtractedText,
    Image,
    TableData,
    OcrResult,
    ConversionReport,
    ExportFile,
}

impl ArtifactType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::ExtractedText => "extracted_text",
            Self::Image => "image",
            Self::TableData => "table_data",
            Self::OcrResult => "ocr_result",
            Self::ConversionReport => "conversion_report",
            Self::ExportFile => "export_file",
        }
    }
}

/// 追溯映射类型（规格 §6.5；迁移 0016 `mapping_type_check`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingType {
    /// PDF 页 → Markdown 章节
    PageToSection,
    /// PPTX 幻灯片 → Markdown 章节
    SlideToSection,
    /// DOCX 段落 → Markdown 章节
    ParagraphToSection,
    /// XLSX 工作表 → Markdown 章节
    SheetToSection,
    /// 表格 → 表格
    TableToTable,
    /// 图片/附件引用
    AssetRef,
}

impl MappingType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PageToSection => "page_to_section",
            Self::SlideToSection => "slide_to_section",
            Self::ParagraphToSection => "paragraph_to_section",
            Self::SheetToSection => "sheet_to_section",
            Self::TableToTable => "table_to_table",
            Self::AssetRef => "asset_ref",
        }
    }
}
