//! S01 · I4 Project Goal 验收（Q1/Q2 已裁 2026-11-03）。
//!
//! - I4-001/002：Goal 归属唯一 Project、每 Project 唯一（UNIQUE project_id+doc_key）
//! - Q1：随创建建立（ensure 幂等；未建立时 Agent 侧缺失是显式信号）
//! - Q2 / I4-012：Agent 只能提案——修改走 DocumentProposal 红线挂起，人类确认后
//!   才产生新 published 修订；不得静默覆盖
//! - I4-E：Run 契约带正式 Goal 快照（建立前 null）

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::caller::{CallerContext, DeviceClaim, HumanActor};
use pm_application::knowledge::KnowledgeRoot;
use pm_application::project_goal::{
    self, ensure_goal_document, goal_head_revision, load_official_goal, GOAL_DOC_KEY,
};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

fn temp_root() -> KnowledgeRoot {
    KnowledgeRoot::new(std::env::temp_dir().join(format!("ai-pm-goal-{}", Uuid::new_v4())))
}

fn cleanup(root: &KnowledgeRoot) {
    let _ = std::fs::remove_dir_all(root.path());
}

fn test_caller(boot: &pm_application::bootstrap::BootstrapOutcome) -> CallerContext {
    CallerContext::new(
        HumanActor {
            principal_id: boot.human_id,
            tenant_id: SYSTEM_TENANT,
            identity_id: None,
            client: Some("test".into()),
            auth_mode: "api_key".into(),
        },
        DeviceClaim::absent(),
    )
}

/// I4-001/002 + Q1：ensure 建立 `project-goal` 文档（published 修订 1），
/// 唯一性由 UNIQUE (project_id, doc_key) 保证；重复 ensure 幂等零新修订。
#[sqlx::test(migrations = "../../migrations")]
async fn goal_is_created_with_project_and_is_unique(pool: PgPool) {
    let root = temp_root();
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "GOAL-1".to_string(),
            project_name: "goal".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();

    // 建立前：缺失是显式信号。
    assert!(load_official_goal(&pool, boot.project_id)
        .await
        .unwrap()
        .is_none());

    ensure_goal_document(&pool, &root, boot.project_id, boot.human_id)
        .await
        .unwrap();

    let goal = load_official_goal(&pool, boot.project_id)
        .await
        .unwrap()
        .expect("goal exists");
    assert_eq!(goal.revision_no, 1);
    assert!(!goal.content_hash.is_empty());

    // 重复 ensure 幂等：仍只有 1 条修订、1 条文档。
    ensure_goal_document(&pool, &root, boot.project_id, boot.human_id)
        .await
        .unwrap();
    let revisions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.document_revision WHERE project_id = $1")
            .bind(boot.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        revisions, 1,
        "duplicate bootstrap must not create a revision"
    );

    let docs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.project_document WHERE project_id = $1 AND doc_key = $2",
    )
    .bind(boot.project_id)
    .bind(GOAL_DOC_KEY)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(docs, 1, "one project owns exactly one goal document");

    cleanup(&root);
}

/// Q2 / I4-012：Agent 只能提案——修改 project-goal 经 DocumentProposal 走红线
/// 挂起（不自动发布），人类确认后才有 revision 2；未建立前修改被拒（无 base）。
#[sqlx::test(migrations = "../../migrations")]
async fn goal_modification_requires_proposal_and_human_confirmation(pool: PgPool) {
    let root = temp_root();
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "GOAL-2".to_string(),
            project_name: "goal".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let caller = test_caller(&boot);

    // Q2（I4 §4.4）：Agent 可以提案初稿，但 **绝不自动发布**——Goal 提案强制
    // 红线挂起，正式 Goal 在人类确认前保持缺失。
    let early = pm_mcp::tools::propose_document(
        &pool,
        &root,
        pm_mcp::tools::ProposeDocumentArgs {
            project_id: boot.project_id,
            doc_key: GOAL_DOC_KEY.to_string(),
            doc_type: "knowledge".to_string(),
            markdown: "# Project Goal\n\n新目标\n\nsource_events: goal-bootstrap\n".to_string(),
            change_summary: Some("agent 提案".to_string()),
            source_event_id: None,
            analysis_run_id: None,
            source_ref: Some("goal-bootstrap".to_string()),
            redline_kind: None,
            base_revision_id: None,
            operation_id: format!("goal-propose-{}", Uuid::new_v4()),
        },
        &caller,
    )
    .await
    .unwrap();
    assert_eq!(
        early["outcome"], "redline_pending",
        "agent goal proposal must pend: {early}"
    );
    assert!(
        load_official_goal(&pool, boot.project_id)
            .await
            .unwrap()
            .is_none(),
        "an unconfirmed agent proposal must not publish the goal"
    );

    // 引导建立 → Agent 提案修改（携带 base）→ 红线挂起，不自动发布。
    ensure_goal_document(&pool, &root, boot.project_id, boot.human_id)
        .await
        .unwrap();
    let base = goal_head_revision(&pool, boot.project_id)
        .await
        .unwrap()
        .unwrap();

    let proposal = pm_mcp::tools::propose_document(
        &pool,
        &root,
        pm_mcp::tools::ProposeDocumentArgs {
            project_id: boot.project_id,
            doc_key: GOAL_DOC_KEY.to_string(),
            doc_type: "knowledge".to_string(),
            markdown: "# Project Goal\n\n## 要解决的问题\n\n把部署时间从天级降到小时级。\n\nsource_events: goal-bootstrap\n".to_string(),
            change_summary: Some("Agent 提案：补充目标".to_string()),
            source_event_id: None,
            analysis_run_id: None,
            source_ref: Some("goal-bootstrap".to_string()),
            redline_kind: None,
            base_revision_id: Some(base),
            operation_id: format!("goal-propose-{}", Uuid::new_v4()),
        },
        &caller,
    )
    .await
    .unwrap();

    // 提案后 head 未变（未自动发布）。
    let head_after_propose = goal_head_revision(&pool, boot.project_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(head_after_propose, base, "agent proposal must not publish");

    // 人类确认红线 → 新 published 修订（revision 2）。
    pm_mcp::tools::confirm_document_proposal(
        &pool,
        &root,
        pm_mcp::tools::ConfirmDocumentProposalArgs {
            project_id: boot.project_id,
            proposal_id: Uuid::parse_str(proposal["proposal_id"].as_str().unwrap()).unwrap(),
            confirmed_by: boot.human_id,
            operation_id: format!("goal-confirm-{}", Uuid::new_v4()),
        },
        &caller,
    )
    .await
    .unwrap();

    let goal = load_official_goal(&pool, boot.project_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        goal.revision_no, 2,
        "human confirmation publishes revision 2"
    );

    cleanup(&root);
}

/// Q2 加强：携带过期 base 的提案被乐观并发拒绝（不得静默覆盖）。
#[sqlx::test(migrations = "../../migrations")]
async fn goal_proposal_with_stale_base_is_rejected(pool: PgPool) {
    let root = temp_root();
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "GOAL-3".to_string(),
            project_name: "goal".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let caller = test_caller(&boot);
    ensure_goal_document(&pool, &root, boot.project_id, boot.human_id)
        .await
        .unwrap();
    let base = goal_head_revision(&pool, boot.project_id)
        .await
        .unwrap()
        .unwrap();

    // 第一次提案 + 人类确认 → head 前进到 rev 2。
    let p1 = pm_mcp::tools::propose_document(
        &pool,
        &root,
        pm_mcp::tools::ProposeDocumentArgs {
            project_id: boot.project_id,
            doc_key: GOAL_DOC_KEY.to_string(),
            doc_type: "knowledge".to_string(),
            markdown: "# Project Goal\n\nv2\n\nsource_events: goal-bootstrap\n".to_string(),
            change_summary: Some("第一轮".to_string()),
            source_event_id: None,
            analysis_run_id: None,
            source_ref: Some("goal-bootstrap".to_string()),
            redline_kind: None,
            base_revision_id: Some(base),
            operation_id: format!("goal-propose-{}", Uuid::new_v4()),
        },
        &caller,
    )
    .await
    .unwrap();
    pm_mcp::tools::confirm_document_proposal(
        &pool,
        &root,
        pm_mcp::tools::ConfirmDocumentProposalArgs {
            project_id: boot.project_id,
            proposal_id: Uuid::parse_str(p1["proposal_id"].as_str().unwrap()).unwrap(),
            confirmed_by: boot.human_id,
            operation_id: format!("goal-confirm-{}", Uuid::new_v4()),
        },
        &caller,
    )
    .await
    .unwrap();

    // 用旧 base（rev 1）再提案 → RevisionConflict（不静默覆盖）。
    let stale = pm_mcp::tools::propose_document(
        &pool,
        &root,
        pm_mcp::tools::ProposeDocumentArgs {
            project_id: boot.project_id,
            doc_key: GOAL_DOC_KEY.to_string(),
            doc_type: "knowledge".to_string(),
            markdown: "# Project Goal\n\n过期基线\n\nsource_events: goal-bootstrap\n".to_string(),
            change_summary: Some("过期提案".to_string()),
            source_event_id: None,
            analysis_run_id: None,
            source_ref: Some("goal-bootstrap".to_string()),
            redline_kind: None,
            base_revision_id: Some(base),
            operation_id: format!("goal-propose-{}", Uuid::new_v4()),
        },
        &caller,
    )
    .await;
    assert!(
        matches!(&stale, Err(pm_mcp::tools::ToolError::Conflict(msg)) if msg.contains("does not match the current head")),
        "stale base must be rejected: {stale:?}"
    );

    cleanup(&root);
}
