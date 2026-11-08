//! 知识空间 V2 质量门：8 项规则检查（设计稿 §十一）。
//!
//! 质量门取代 V1 的事前审批：常规修订通过即自动提交；红线类（对外承诺 / 删除
//! 历史 / 改写已交付版本）转 `redline_pending` 等人类确认。规则本身不依赖模型，
//! 保证「模型不可用」时仍可判定。

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::knowledge::KnowledgeError;
use crate::knowledge_index::parse_markdown_tree;

/// 三类红线（用户确认的定义）。
pub const REDLINE_KINDS: [&str; 3] = ["external_commitment", "delete_history", "rewrite_delivered"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GateFailure {
    pub gate: String,
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QualityGateResult {
    pub passed: bool,
    /// 同一文档已有完全相同内容的 published 修订：幂等无操作。
    pub duplicate: bool,
    pub redline_kind: Option<String>,
    pub reindex_required: bool,
    pub failures: Vec<GateFailure>,
}

pub struct GateInput<'a> {
    pub project_id: Uuid,
    pub doc_type: &'a str,
    pub markdown: &'a str,
    pub content_hash: &'a str,
    pub change_summary: Option<&'a str>,
    pub source_event_id: Option<Uuid>,
    pub analysis_run_id: Option<Uuid>,
    pub source_ref: Option<&'a str>,
    pub declared_redline: Option<&'a str>,
    pub current_document_id: Option<Uuid>,
    /// T-060：当前生效（最新 published）修订 id——版本检查的比对对象。
    pub current_revision_id: Option<Uuid>,
    /// T-060：请求声明的基线修订（取代原 `base_content_hash`）。
    pub base_revision_id: Option<Uuid>,
}

fn fail(result: &mut QualityGateResult, gate: &str, code: &str, detail: impl Into<String>) {
    result.failures.push(GateFailure {
        gate: gate.to_string(),
        code: code.to_string(),
        detail: detail.into(),
    });
}

/// 依次执行 8 项检查；`passed` 仅在无失败时为真。
pub async fn evaluate(
    pool: &PgPool,
    input: &GateInput<'_>,
) -> Result<QualityGateResult, KnowledgeError> {
    let mut result = QualityGateResult {
        reindex_required: true,
        ..Default::default()
    };
    let trimmed = input.markdown.trim();

    // 1) 重复检查：同内容是否已存在于其他文档；同一文档则视为幂等重复。
    let same_hash: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT d.id, r.id FROM ai.document_revision r \
           JOIN ai.project_document d ON d.id = r.document_id \
          WHERE r.project_id = $1 AND r.status = 'published' AND r.content_hash = $2 \
          LIMIT 1",
    )
    .bind(input.project_id)
    .bind(input.content_hash)
    .fetch_optional(pool)
    .await?;
    if let Some((doc_id, _)) = same_hash {
        if Some(doc_id) == input.current_document_id {
            result.duplicate = true;
        } else {
            fail(
                &mut result,
                "duplicate",
                "duplicate_fact",
                "identical content already published in another document",
            );
        }
    }

    // 2) 来源检查：必须能追溯到事件 / 分析运行 / 显式来源（如需求迁移）。
    if input.source_event_id.is_none()
        && input.analysis_run_id.is_none()
        && input.source_ref.is_none()
    {
        fail(
            &mut result,
            "source",
            "source_missing",
            "no source_event_id / analysis_run_id / source_ref",
        );
    }

    // 3) 版本检查：显式基线修订必须仍是当前 published 修订。
    if let (Some(current), Some(base)) = (input.current_revision_id, input.base_revision_id) {
        if base != current {
            fail(
                &mut result,
                "version",
                "version_conflict",
                "base_revision_id does not match the current published revision",
            );
        }
    }

    // 4) 冲突检查：改写正式需求/决策必须有变更理由。
    if input.current_document_id.is_some()
        && matches!(input.doc_type, "requirement" | "decision" | "architecture")
        && input.change_summary.map(str::trim).unwrap_or("").is_empty()
    {
        fail(
            &mut result,
            "conflict",
            "conflict_without_rationale",
            "rewriting an official document requires a change_summary",
        );
    }

    // 5) 归档检查：空正文或显式临时内容不应进入正式知识。
    if trimmed.is_empty() {
        fail(
            &mut result,
            "archive",
            "archive_informal",
            "empty body is not formal knowledge",
        );
    } else if trimmed.contains("# 临时") || trimmed.contains("<!-- temporary -->") {
        fail(
            &mut result,
            "archive",
            "archive_informal",
            "temporary content must stay in analysis records",
        );
    }

    // 6) 引用检查：正文必须保留来源事件或具体文档引用。
    let has_markdown_reference =
        input.markdown.contains("source_events:") || input.markdown.contains("projects/");
    if !has_markdown_reference {
        fail(
            &mut result,
            "reference",
            "reference_missing",
            "proposal must reference source_events or a concrete document path",
        );
    }

    // 7) 红线检查：显式声明或空正文删除历史 → 转人类事前确认。
    let inferred = if trimmed.is_empty() && input.current_document_id.is_some() {
        Some("delete_history".to_string())
    } else {
        None
    };
    let redline = input.declared_redline.map(str::to_string).or(inferred);
    if let Some(kind) = redline {
        if REDLINE_KINDS.contains(&kind.as_str()) {
            result.redline_kind = Some(kind);
        } else {
            fail(
                &mut result,
                "redline",
                "redline_unknown",
                format!("unknown redline kind `{kind}`"),
            );
        }
    }

    // 8) 索引检查：正文必须能解析出至少一个标题章节，否则派生索引无法建立。
    let sections = parse_markdown_tree("proposal.md", input.markdown);
    if !sections.iter().any(|section| section.level > 0) {
        fail(
            &mut result,
            "index",
            "index_no_heading",
            "proposal has no Markdown heading to index",
        );
    }

    result.passed = result.failures.is_empty();
    Ok(result)
}
