//! Project Goal（S01 · I4，`docs/ui/smark-pm-i4-…` / 用户裁决 Q1、Q2）。
//!
//! **Q1（2026-11-03 裁决）**：Goal 随 Project 创建时建立——每 Project 恰好一条
//! 正式目标性知识资产。**Q2**：Agent 只能提案，永不直接写正式 Goal。
//!
//! 落地口径（复用既有知识机制，**零新表**）：
//!
//! - Goal = `ai.project_document` 中 `doc_key='project-goal'` 的文档；唯一性由
//!   既有 `UNIQUE (project_id, doc_key)` 数据库级保证（I4 §3.1）。
//! - 正文走独立知识 Git 仓（`KnowledgeRoot`），修订走既有 8 态管线——修改必须
//!   经 DocumentProposal 提案 + 人类确认（Q2 由管线结构保证：Agent 提案只会
//!   挂起为红线待确认，不会自动发布）。
//! - "正式 Goal" = 最新 `published` 修订（`knowledge_write_coordinator::current_head`
//!   同口径）。建立前 Agent 侧读到 `goal: null`——缺失是显式信号（I4 §4.4）。

use sqlx::PgPool;
use uuid::Uuid;

use crate::knowledge::{self, CreateDocumentRequest, KnowledgeError, KnowledgeRoot};

/// Goal 文档的固定 doc_key。
pub const GOAL_DOC_KEY: &str = "project-goal";

/// Goal 初稿占位正文（§2.3 建议结构；由人类/提案逐步充实）。
pub const INITIAL_GOAL_MARKDOWN: &str = "# Project Goal\n\n\
> 本文档是项目的正式目标（Project Goal）。修改必须经提案 + 人工确认。\n\n\
## 要解决的问题\n\n（待补充）\n\n\
## 期望结果\n\n（待补充）\n\n\
## 核心使用对象\n\n（待补充）\n\n\
## 范围\n\n（待补充）\n\n\
## 明确不做\n\n（待补充）\n\n\
## 成功标准\n\n（待补充）\n";

/// 正式 Goal 的只读快照。
#[derive(Debug, Clone, serde::Serialize)]
pub struct OfficialGoal {
    pub document_id: Uuid,
    pub revision_id: Uuid,
    pub revision_no: i32,
    pub content_hash: String,
    pub git_commit_sha: String,
    pub change_summary: Option<String>,
    pub published_at: chrono::DateTime<chrono::Utc>,
}

/// 确保 Goal 文档存在（Q1：随 Project 创建建立；幂等）。
///
/// 已存在（含并发输家）→ 直接返回 Ok，不产生新修订；新建走
/// `create_document` 的协调器锁 + 幂等账本（operation_id 固定派生自
/// project_id，重复引导不会重复建修订）。
pub async fn ensure_goal_document(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    created_by: Uuid,
) -> Result<(), KnowledgeError> {
    let exists: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM ai.project_document WHERE project_id = $1 AND doc_key = $2",
    )
    .bind(project_id)
    .bind(GOAL_DOC_KEY)
    .fetch_optional(pool)
    .await?;
    if exists.is_some() {
        return Ok(());
    }

    root.ensure_ready()?;
    knowledge::create_document(
        pool,
        root,
        project_id,
        created_by,
        CreateDocumentRequest {
            doc_key: GOAL_DOC_KEY.to_string(),
            doc_type: "knowledge".to_string(),
            markdown: INITIAL_GOAL_MARKDOWN.to_string(),
            change_summary: Some("Project Goal bootstrap (Q1)".to_string()),
            operation_id: format!("goal-bootstrap-{project_id}"),
            base_revision_id: None,
        },
    )
    .await
    .map(|_| ())
}

/// 读正式 Goal（最新 published 修订）；未建立 → `None`（缺失是显式信号）。
pub async fn load_official_goal(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Option<OfficialGoal>, sqlx::Error> {
    let row: Option<(
        Uuid,
        Uuid,
        i32,
        String,
        String,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT d.id, r.id, r.revision_no, r.content_hash, r.git_commit_sha, \
                    r.change_summary, r.created_at \
             FROM ai.project_document d \
             JOIN LATERAL ( \
                  SELECT id, revision_no, content_hash, git_commit_sha, change_summary, created_at \
                    FROM ai.document_revision \
                   WHERE document_id = d.id AND status = 'published' \
                   ORDER BY revision_no DESC LIMIT 1 \
             ) r ON true \
             WHERE d.project_id = $1 AND d.doc_key = $2",
    )
    .bind(project_id)
    .bind(GOAL_DOC_KEY)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(
        |(
            document_id,
            revision_id,
            revision_no,
            content_hash,
            git_commit_sha,
            change_summary,
            published_at,
        )| {
            OfficialGoal {
                document_id,
                revision_id,
                revision_no,
                content_hash,
                git_commit_sha,
                change_summary,
                published_at,
            }
        },
    ))
}

/// Goal 文档的当前 head 修订（提案管线 base_revision 校验的锚点）；未建立 → None。
pub async fn goal_head_revision(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Option<Uuid>, KnowledgeError> {
    crate::knowledge_write_coordinator::current_head(pool, project_id, GOAL_DOC_KEY).await
}

/// 供审计/展示：Goal 文档 id（不存在 → None）。
pub async fn goal_document_id(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM ai.project_document WHERE project_id = $1 AND doc_key = $2")
        .bind(project_id)
        .bind(GOAL_DOC_KEY)
        .fetch_optional(pool)
        .await
}
