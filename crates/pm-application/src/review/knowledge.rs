//! 知识回写（D5 §7）：**平台确定性模板**组装「任务执行记录」，在
//! `reviewing→completed` 的**同一事务**内 INSERT 一条 `draft` 提案。
//!
//! 边界（§0.2 不变量 4）：
//!
//! - 只触发 `draft` 提案，**不执行知识域状态机**（质量门 / 红线 / 确认由 0014
//!   既有提案流接管）。
//! - 不自动写入正式知识库；`auto_publish_knowledge` 不归 D5。
//! - 提案与 completed 原子绑定：任一失败全回滚——「任务已完成却没有知识提案」
//!   在库层不可能发生（§7.1）。

use sqlx::{Row, Transaction};
use uuid::Uuid;

use crate::review::queue::ReviewError;

/// 同名提案的**活跃状态集**：这些状态下不重复建提案（处置同 task 重审场景）。
pub const ACTIVE_PROPOSAL_STATUSES: [&str; 5] = [
    "draft",
    "quality_gates_passed",
    "redline_pending",
    "confirmed",
    "commit_pending",
];

/// 知识提案的载荷（事务内 INSERT 的输入）。
#[derive(Debug, Clone)]
pub struct KnowledgeWriteback {
    pub task_title: String,
    pub verdict: String,
    pub findings_summary: Vec<String>,
    pub repository_commits: Vec<(String, String)>,
    pub change_summary: Option<String>,
}

/// 生成的任务执行记录正文（确定性模板，零 LLM 依赖）。
pub fn render_markdown(writeback: &KnowledgeWriteback, task_id: Uuid) -> String {
    let mut out = String::new();
    out.push_str(&format!("# 任务执行记录：{}\n\n", writeback.task_title));
    out.push_str(&format!("- 任务 ID：`{task_id}`\n"));
    out.push_str(&format!("- 审查结论：`{}`\n", writeback.verdict));
    if let Some(summary) = &writeback.change_summary {
        out.push_str(&format!("- 变更摘要：{}\n", summary.trim()));
    }
    out.push_str("\n## 交付 commit\n\n");
    if writeback.repository_commits.is_empty() {
        out.push_str("（无可写仓交付，全只读仓任务）\n");
    } else {
        for (repository_id, commit_sha) in &writeback.repository_commits {
            out.push_str(&format!("- `{repository_id}`：`{commit_sha}`\n"));
        }
    }
    out.push_str("\n## 审查发现\n\n");
    if writeback.findings_summary.is_empty() {
        out.push_str("（无 finding）\n");
    } else {
        for finding in &writeback.findings_summary {
            out.push_str(&format!("- {finding}\n"));
        }
    }
    out.push_str(
        "\n> 本文由平台按确定性模板生成，仅为**草稿提案**；正式化须经知识域质量门与人工确认。\n",
    );
    out
}

/// 同事务 INSERT 一条 `draft` 知识提案。返回 `None` 表示同名活跃提案已存在
/// （去重命中，跳过 INSERT —— completed 照常落定）。
pub(crate) async fn write_proposal_in_tx(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    review_run_id: Uuid,
    writeback: &KnowledgeWriteback,
) -> Result<Option<Uuid>, ReviewError> {
    let doc_key = format!("task-{task_id}");
    // 去重：同名活跃提案存在即跳过（不重排、不覆盖）。
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.document_proposal \
          WHERE project_id = $1 AND doc_key = $2 AND status = ANY($3)",
    )
    .bind(project_id)
    .bind(&doc_key)
    .bind(
        ACTIVE_PROPOSAL_STATUSES
            .iter()
            .map(|status| status.to_string())
            .collect::<Vec<_>>(),
    )
    .fetch_one(&mut **tx)
    .await?;
    if active > 0 {
        return Ok(None);
    }

    let project_code: String = sqlx::query_scalar("SELECT code FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(&mut **tx)
        .await?;
    let canonical_path = crate::knowledge::canonical_path(&project_code, "knowledge", &doc_key)
        .map_err(|e| ReviewError::Transition(e.to_string()))?;
    let markdown = render_markdown(writeback, task_id);
    let content_hash = crate::knowledge::content_hash(&markdown);

    let proposal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.document_proposal \
         (project_id, doc_key, doc_type, canonical_path, proposed_markdown, proposed_content_hash, \
          change_summary, status, created_by, tenant_id) \
         VALUES ($1, $2, 'knowledge', $3, $4, $5, $6, 'draft', $7, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(&doc_key)
    .bind(&canonical_path)
    .bind(&markdown)
    .bind(&content_hash)
    .bind(writeback.change_summary.as_deref())
    .bind(crate::run_dispatch::DISPATCH_SERVICE_PRINCIPAL)
    .fetch_one(&mut **tx)
    .await?;

    // 提案与触发它的 review 建立可追溯关联（`analysis_run_id` 是既有列，语义为
    // 「哪次运行产生」——此处即 review run）。
    sqlx::query("UPDATE ai.document_proposal SET analysis_run_id = $2 WHERE id = $1")
        .bind(proposal_id)
        .bind(review_run_id)
        .execute(&mut **tx)
        .await?;

    Ok(Some(proposal_id))
}

/// 组装知识回写载荷（执行端上报的 `change_summary` 原样保留）。
pub(crate) async fn assemble_writeback(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    task_id: Uuid,
    delivery_run_id: Uuid,
    verdict: &str,
    findings_summary: Vec<String>,
    change_summary: Option<String>,
) -> Result<KnowledgeWriteback, ReviewError> {
    let task_title: String = sqlx::query_scalar("SELECT title FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(&mut **tx)
        .await?;
    let rows = sqlx::query(
        "SELECT repository_id, commit_sha FROM delivery.task_repository_commit \
          WHERE task_run_id = $1 ORDER BY repository_id",
    )
    .bind(delivery_run_id)
    .fetch_all(&mut **tx)
    .await?;
    let repository_commits = rows
        .into_iter()
        .map(|row| {
            (
                row.get::<Uuid, _>("repository_id").to_string(),
                row.get::<String, _>("commit_sha"),
            )
        })
        .collect();
    Ok(KnowledgeWriteback {
        task_title,
        verdict: verdict.to_string(),
        findings_summary,
        repository_commits,
        change_summary,
    })
}
