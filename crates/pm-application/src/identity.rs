//! D-4 · 业务身份综合：输入装配与三提案落地（pc product_profile/product_vision
//! 提案 + project-goal 文档提案）。
//!
//! 裁决：docs/scenarios/s01/docs/s01-reanalysis-and-product-cognition-closure-v1.md
//! D-4（2026-11-15 人工按钮触发；LLM 综合访谈答案 + 仓库产物 → 三份草案 →
//! 全部走既有提案门：PC P4/P5 + 知识 8 态管线——不新增旁门）。
//!
//! 红线：Agent 无确认权——Goal 提案经 submit_proposal 自动强制红线
//! （knowledge_proposal/submit.rs L107-114），PC 提案停在 submitted 等人工。

use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::knowledge::KnowledgeRoot;
use crate::pc::{
    create_proposal, submit_proposal, upsert_object_draft, CreateProposalInput, ProposalMemberPin,
    UpsertObjectDraft,
};
use crate::pm_write::append_audit_with_after;
use crate::project_goal::{goal_head_revision, GOAL_DOC_KEY};

/// 综合输入（LLM 侧契约的数据载体；pm-ai identity_synthesis 消费）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct IdentitySynthesisInput {
    /// 访谈问答（问题, 最新答案）——INITIAL_ONBOARDING 会话。
    pub interview_answers: Vec<(String, String)>,
    /// 最近一份分析报告的 KB 快照全文（有界）。
    pub repo_digest: Option<String>,
    /// 既有 PC 对象修订摘要（名称, 类型, 修订号, 正文截断）。
    pub pc_objects: Vec<(String, String, i32, Option<String>)>,
}

/// LLM 综合产物（pm-ai identity_synthesis 产出；本模块负责落地）。
#[derive(Debug, Clone)]
pub struct IdentityDrafts {
    pub product_profile_markdown: String,
    pub product_vision_markdown: String,
    pub project_goal_markdown: String,
    pub uncertainty_notes: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    Failed(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// 装配综合输入。无访谈会话/无报告 → 对应段为空（LLM 提示词要求缺段时
/// 在 uncertainty_notes 里如实声明，不得编造）。
pub async fn assemble_identity_input(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<IdentitySynthesisInput, sqlx::Error> {
    // 访谈：最近一条 INITIAL_ONBOARDING 会话的问题 + 最新答案。
    let mut interview_answers = Vec::new();
    let session: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM discovery.discovery_session \
          WHERE project_id = $1 \
            AND tenant_id = (SELECT tenant_id FROM core.project WHERE id = $1) \
            AND kind = 'INITIAL_ONBOARDING' \
          ORDER BY created_at DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    if let Some(session_id) = session {
        let rows: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT q.question, \
                    (SELECT a.answer FROM discovery.discovery_answer a \
                      WHERE a.question_id = q.id ORDER BY a.created_at DESC LIMIT 1) \
             FROM discovery.discovery_question q \
              WHERE q.session_id = $1 ORDER BY q.created_at ASC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await?;
        for (question, answer) in rows {
            if let Some(answer) = answer.filter(|value| !value.trim().is_empty()) {
                interview_answers.push((question, answer));
            }
        }
    }

    // 报告：最近一份有 KB 修订的报告（analyzed/closed）。
    let repo_digest: Option<String> = sqlx::query_scalar(
        "SELECT left(kv.content, 12000) \
          FROM integration.repo_analysis_report r \
          JOIN kb.document_revision kv ON kv.id = r.kb_revision_id \
         WHERE r.project_id = $1 \
           AND r.kb_revision_id IS NOT NULL \
           AND r.status IN ('analyzed', 'closed') \
         ORDER BY r.created_at DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .flatten();

    // 既有 PC 对象修订（最近一份有来源的报告；截断 1200 字/对象）。
    let pc_objects: Vec<(String, String, i32, Option<String>)> = sqlx::query_as(
        "SELECT o.display_name, o.object_type, cr.revision_no, left(dr.content, 1200) \
           FROM integration.repo_analysis_report rep \
           JOIN pc.evidence e ON e.source_id = rep.pc_source_id \
           JOIN pc.cognition_revision cr ON cr.id = e.object_revision_id \
           JOIN pc.cognition_object o ON o.id = cr.object_id \
           JOIN pc.document_revision dr ON dr.id = cr.document_revision_id \
          WHERE rep.project_id = $1 \
            AND rep.pc_source_id IS NOT NULL \
            AND rep.status IN ('analyzed', 'closed') \
          ORDER BY rep.created_at DESC, o.display_name ASC LIMIT 12",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    Ok(IdentitySynthesisInput {
        interview_answers,
        repo_digest,
        pc_objects,
    })
}

/// 三提案落地回执。
#[derive(Debug, serde::Serialize)]
pub struct IdentityApplyOutcome {
    pub profile_proposal_id: Option<Uuid>,
    pub vision_proposal_id: Option<Uuid>,
    pub goal_proposal_id: Option<Uuid>,
    /// Goal 提案状态（redline_pending = 等 Agent 红线确认，来自管线自动门）。
    pub goal_proposal_status: Option<String>,
}

async fn propose_pc_object(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    object_key: &str,
    object_type: &str,
    title: &str,
    content_markdown: &str,
) -> Result<Uuid, IdentityError> {
    let display_name: String = sqlx::query_scalar(
        "SELECT display_name FROM pc.cognition_object \
          WHERE project_id = $1 AND object_key = $2 \
          ORDER BY created_at LIMIT 1",
    )
    .bind(project_id)
    .bind(object_key)
    .fetch_optional(pool)
    .await?
    .unwrap_or_else(|| object_key.to_string());

    let receipt = upsert_object_draft(
        pool,
        project_id,
        actor,
        UpsertObjectDraft {
            object_type: object_type.to_string(),
            object_key: Some(object_key.to_string()),
            display_name,
            content_markdown: content_markdown.to_string(),
            change_summary: Some("业务身份综合（D-4 身份综合轮）".into()),
            source_context: None,
            analysis_mode: None,
        },
    )
    .await
    .map_err(|e| IdentityError::Failed(format!("pc upsert failed: {e}")))?;

    let drafted = create_proposal(
        pool,
        project_id,
        actor,
        CreateProposalInput {
            title: title.to_string(),
            rationale: "D-4 业务身份综合轮：LLM 汇聚访谈答案与仓库产物起草".into(),
            origin_kind: "ai_agent".into(),
            source_id: None,
            governance_level: None,
            risk_level: None,
            confidence: None,
            members: vec![ProposalMemberPin {
                change_kind: "update_object".into(),
                object_id: Some(receipt.object_id),
                object_revision_id: Some(receipt.revision_id),
                relationship_id: None,
                relationship_revision_id: None,
            }],
        },
    )
    .await
    .map_err(|e| IdentityError::Failed(format!("pc proposal failed: {e}")))?;
    submit_proposal(pool, project_id, actor, drafted.proposal_id, None)
        .await
        .map_err(|e| IdentityError::Failed(format!("pc submit failed: {e}")))?;
    Ok(drafted.proposal_id)
}

/// 三提案落地（单事务性不做强求——三份各自独立走门；部分失败如实报错，
/// 已落库的提案保留在各自审批面可人工处置）。
pub async fn apply_identity_drafts(
    pool: &PgPool,
    root: Option<&KnowledgeRoot>,
    project_id: Uuid,
    actor: &AuditActor,
    drafts: &IdentityDrafts,
) -> Result<IdentityApplyOutcome, IdentityError> {
    let profile_proposal_id = propose_pc_object(
        pool,
        project_id,
        actor,
        "product-profile",
        "product_profile",
        "业务身份提案（产品画像）",
        &drafts.product_profile_markdown,
    )
    .await?;
    let vision_proposal_id = propose_pc_object(
        pool,
        project_id,
        actor,
        "product-vision",
        "product_vision",
        "业务身份提案（产品愿景）",
        &drafts.product_vision_markdown,
    )
    .await?;

    // Goal：经知识 8 态管线；Agent 提案自动强制红线（submit.rs Q2 裁决）。
    let (goal_proposal_id, goal_proposal_status) = match root {
        Some(root) => {
            let base = goal_head_revision(pool, project_id).await.ok().flatten();
            let outcome = crate::knowledge_proposal::submit_proposal(
                pool,
                root,
                crate::knowledge_proposal::ProposalRequest {
                    project_id,
                    doc_key: GOAL_DOC_KEY.to_string(),
                    doc_type: "knowledge".to_string(),
                    markdown: drafts.project_goal_markdown.clone(),
                    change_summary: Some("业务身份综合（D-4 身份综合轮）".into()),
                    source_event_id: None,
                    analysis_run_id: None,
                    source_ref: None,
                    redline_kind: None, // project-goal 自动强制红线
                    base_revision_id: base,
                    operation_id: format!("identity-synthesis:{}:{}", project_id, Uuid::new_v4()),
                    created_by: actor.principal_id,
                },
            )
            .await
            .map_err(|e| IdentityError::Failed(format!("goal proposal failed: {e}")))?;
            use crate::knowledge_proposal::ProposalOutcome;
            match outcome {
                ProposalOutcome::RedlinePending { proposal_id } => {
                    (Some(proposal_id), Some("redline_pending".to_string()))
                }
                ProposalOutcome::Duplicate { revision_id, .. } => {
                    (Some(revision_id), Some("duplicate".to_string()))
                }
                ProposalOutcome::Rejected { proposal_id, failures } => {
                    return Err(IdentityError::Failed(format!(
                        "goal proposal rejected by quality gates: {failures:?} (proposal {proposal_id})"
                    )));
                }
                ProposalOutcome::Committed { revision_id, .. } => {
                    (Some(revision_id), Some("committed".to_string()))
                }
            }
        }
        None => (None, None),
    };

    let outcome = IdentityApplyOutcome {
        profile_proposal_id: profile_proposal_id.into(),
        vision_proposal_id: vision_proposal_id.into(),
        goal_proposal_id,
        goal_proposal_status,
    };

    let mut audit_tx = pool.begin().await?;
    append_audit_with_after(
        &mut audit_tx,
        project_id,
        actor,
        "identity_synthesis.applied",
        "project",
        Some(project_id),
        serde_json::json!({
            "profile_proposal_id": outcome.profile_proposal_id,
            "vision_proposal_id": outcome.vision_proposal_id,
            "goal_proposal_id": outcome.goal_proposal_id,
        }),
    )
    .await
    .map_err(IdentityError::Db)?;
    audit_tx.commit().await?;

    Ok(outcome)
}
