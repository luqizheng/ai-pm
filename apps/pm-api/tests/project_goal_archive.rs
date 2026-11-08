//! S01 · I4-F 归档与审计集成验收（Goal 归档只读 / 归档竞争）。
//!
//! 依据：`docs/ui/smark-pm-s01-i4-project-goal-knowledge-v1.md` §9（归档后 Goal
//! 规则）、§11.3（确认与归档竞争）、§13 I4-F（归档和审计集成）。
//!
//! 覆盖：
//! - I4-019 归档只读：归档后不得新建 Goal 提案、不得确认发布；
//! - I4-019/§9.2：归档时未完成的 Goal 提案不能继续提交（保留原文与审计）；
//! - I4-020 归档竞争：归档与提交并发有确定结果——**归档之后不得出现新的正式修订**；
//! - 历史查询保留：归档后正式 Goal、提案与修订历史仍可读；
//! - 审计完整性：被拒的提交不产生修订，也不产生发布审计。

use pm_application::actor::AuditActor;
use pm_application::archive::archive_project;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::caller::{CallerContext, DeviceClaim, HumanActor};
use pm_application::knowledge::KnowledgeRoot;
use pm_application::project_goal::{ensure_goal_document, load_official_goal, GOAL_DOC_KEY};
use pm_application::run_dispatch::NoopExecutor;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

fn temp_root() -> KnowledgeRoot {
    KnowledgeRoot::new(std::env::temp_dir().join(format!("ai-pm-goal-arc-{}", Uuid::new_v4())))
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

async fn archive(boot: &pm_application::bootstrap::BootstrapOutcome, pool: &PgPool) {
    archive_project(
        pool,
        &NoopExecutor,
        &AuditActor::principal(boot.human_id),
        SYSTEM_TENANT,
        boot.project_id,
        "i4f-test",
    )
    .await
    .unwrap();
}

async fn propose_goal(
    pool: &PgPool,
    root: &KnowledgeRoot,
    boot: &pm_application::bootstrap::BootstrapOutcome,
    body: &str,
) -> serde_json::Value {
    pm_mcp::tools::propose_document(
        pool,
        root,
        pm_mcp::tools::ProposeDocumentArgs {
            project_id: boot.project_id,
            doc_key: GOAL_DOC_KEY.to_string(),
            doc_type: "knowledge".to_string(),
            markdown: body.to_string(),
            change_summary: Some("I4-F 提案".to_string()),
            source_event_id: None,
            analysis_run_id: None,
            source_ref: Some("goal-bootstrap".to_string()),
            redline_kind: None,
            base_revision_id: None,
            operation_id: format!("i4f-propose-{}", Uuid::new_v4()),
        },
        &test_caller(boot),
    )
    .await
    .unwrap()
}

fn is_archived_conflict(err: &pm_mcp::tools::ToolError) -> bool {
    matches!(err, pm_mcp::tools::ToolError::Conflict(msg) if msg.contains("archived"))
}

/// I4-019：归档后不得新建 Goal 提案（提案表零新增）。
#[sqlx::test(migrations = "../../migrations")]
async fn archived_project_rejects_new_goal_proposal(pool: PgPool) {
    let root = temp_root();
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "I4F-1".to_string(),
            project_name: "i4f".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    ensure_goal_document(&pool, &root, boot.project_id, boot.human_id)
        .await
        .unwrap();

    archive(&boot, &pool).await;

    let err = pm_mcp::tools::propose_document(
        &pool,
        &root,
        pm_mcp::tools::ProposeDocumentArgs {
            project_id: boot.project_id,
            doc_key: GOAL_DOC_KEY.to_string(),
            doc_type: "knowledge".to_string(),
            markdown: "# Project Goal\n\n归档后提案\n\nsource_events: goal-bootstrap\n".to_string(),
            change_summary: Some("归档后提案".to_string()),
            source_event_id: None,
            analysis_run_id: None,
            source_ref: Some("goal-bootstrap".to_string()),
            redline_kind: None,
            base_revision_id: None,
            operation_id: format!("i4f-propose-{}", Uuid::new_v4()),
        },
        &test_caller(&boot),
    )
    .await;
    assert!(
        is_archived_conflict(&err.unwrap_err()),
        "archived project must reject proposals"
    );

    let proposals: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.document_proposal WHERE project_id = $1")
            .bind(boot.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(proposals, 0, "a rejected proposal must leave no row");

    cleanup(&root);
}

/// I4-019 / §9.2：归档时未完成的 Goal 提案不能继续确认发布；提案与审计保留。
#[sqlx::test(migrations = "../../migrations")]
async fn archived_project_rejects_confirming_a_pending_goal_proposal(pool: PgPool) {
    let root = temp_root();
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "I4F-2".to_string(),
            project_name: "i4f".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    ensure_goal_document(&pool, &root, boot.project_id, boot.human_id)
        .await
        .unwrap();

    // 归档前提案（红线挂起，未发布）。
    let proposal = propose_goal(
        &pool,
        &root,
        &boot,
        "# Project Goal\n\n待确认版本\n\nsource_events: goal-bootstrap\n",
    )
    .await;
    assert_eq!(proposal["outcome"], "redline_pending");
    let proposal_id: Uuid = Uuid::parse_str(proposal["proposal_id"].as_str().unwrap()).unwrap();

    archive(&boot, &pool).await;

    let err = pm_mcp::tools::confirm_document_proposal(
        &pool,
        &root,
        pm_mcp::tools::ConfirmDocumentProposalArgs {
            project_id: boot.project_id,
            proposal_id,
            confirmed_by: boot.human_id,
            operation_id: format!("i4f-confirm-{}", Uuid::new_v4()),
        },
        &test_caller(&boot),
    )
    .await;
    assert!(
        is_archived_conflict(&err.unwrap_err()),
        "a pending goal proposal must not be confirmable after archive"
    );

    // 正式修订数不变（仍为引导建立的 rev 1）；提案保留原文与状态。
    let revisions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.document_revision WHERE project_id = $1")
            .bind(boot.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(revisions, 1, "archive must not publish a new revision");
    let status: String =
        sqlx::query_scalar("SELECT status FROM ai.document_proposal WHERE id = $1")
            .bind(proposal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "redline_pending", "proposal history is preserved");

    cleanup(&root);
}

/// 历史查询保留（I4-F 工作项 4）：归档后正式 Goal、提案列表与修订历史仍可读。
#[sqlx::test(migrations = "../../migrations")]
async fn archived_project_keeps_goal_history_readable(pool: PgPool) {
    let root = temp_root();
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "I4F-3".to_string(),
            project_name: "i4f".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    ensure_goal_document(&pool, &root, boot.project_id, boot.human_id)
        .await
        .unwrap();
    propose_goal(
        &pool,
        &root,
        &boot,
        "# Project Goal\n\n历史提案\n\nsource_events: goal-bootstrap\n",
    )
    .await;

    archive(&boot, &pool).await;

    // 正式 Goal 仍可读。
    let goal = load_official_goal(&pool, boot.project_id)
        .await
        .unwrap()
        .expect("goal readable");
    assert_eq!(goal.revision_no, 1);
    // 提案历史仍可读。
    let proposals =
        pm_application::knowledge_proposal::list_proposals(&pool, boot.project_id, None)
            .await
            .unwrap();
    assert_eq!(proposals.len(), 1, "proposal history survives archive");
    // 修订历史仍可读。
    let revisions: Vec<(i32, String)> = sqlx::query_as(
        "SELECT revision_no, status FROM ai.document_revision \
         WHERE project_id = $1 ORDER BY revision_no",
    )
    .bind(boot.project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].1, "published");

    cleanup(&root);
}

/// I4-020 归档竞争：确认发布与归档并发 → **归档之后不得出现新的正式修订**。
#[sqlx::test(migrations = "../../migrations")]
async fn archive_racing_with_confirm_never_publishes_after_archive(pool: PgPool) {
    for round in 0..3 {
        let root = temp_root();
        let boot = bootstrap_project(
            &pool,
            BootstrapRequest {
                project_code: format!("I4F-4-{round}"),
                project_name: "i4f".to_string(),
                tenant_id: SYSTEM_TENANT,
            },
        )
        .await
        .unwrap();
        ensure_goal_document(&pool, &root, boot.project_id, boot.human_id)
            .await
            .unwrap();
        let proposal = propose_goal(
            &pool,
            &root,
            &boot,
            "# Project Goal\n\n并发版本\n\nsource_events: goal-bootstrap\n",
        )
        .await;
        let proposal_id: Uuid = Uuid::parse_str(proposal["proposal_id"].as_str().unwrap()).unwrap();

        // 并发：确认发布 ‖ 归档。
        let caller = test_caller(&boot);
        let confirm = pm_mcp::tools::confirm_document_proposal(
            &pool,
            &root,
            pm_mcp::tools::ConfirmDocumentProposalArgs {
                project_id: boot.project_id,
                proposal_id,
                confirmed_by: boot.human_id,
                operation_id: format!("i4f-confirm-race-{}", Uuid::new_v4()),
            },
            &caller,
        );
        let archive_fut = archive(&boot, &pool);
        let (confirm_result, _) = tokio::join!(confirm, archive_fut);

        // 归档一定落定。
        let status: String = sqlx::query_scalar("SELECT status FROM core.project WHERE id = $1")
            .bind(boot.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "archived");

        // 不变式：归档审计时刻之后不得出现新的 published 修订。
        let archived_at: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
            "SELECT created_at FROM governance.audit_log \
             WHERE project_id = $1 AND action = 'project.archived' LIMIT 1",
        )
        .bind(boot.project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let late: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ai.document_revision \
             WHERE project_id = $1 AND status = 'published' AND created_at > $2",
        )
        .bind(boot.project_id)
        .bind(archived_at)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            late, 0,
            "no published revision may appear after the archive (round {round})"
        );

        // 并发结论二选一，但都必须自洽：
        // 确认成功 → 归档时它已是最终版本；确认失败 → 归档把它挡在门外。
        match confirm_result {
            Ok(_) => {
                let revisions: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM ai.document_revision WHERE project_id = $1",
                )
                .bind(boot.project_id)
                .fetch_one(&pool)
                .await
                .unwrap();
                assert_eq!(
                    revisions, 2,
                    "a successful confirm publishes exactly one revision"
                );
            }
            Err(err) => {
                assert!(
                    is_archived_conflict(&err),
                    "a failed confirm must be the archive guard: {err:?}"
                );
                let revisions: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM ai.document_revision WHERE project_id = $1",
                )
                .bind(boot.project_id)
                .fetch_one(&pool)
                .await
                .unwrap();
                assert_eq!(revisions, 1, "a rejected confirm publishes nothing");
            }
        }

        cleanup(&root);
    }
}

/// 审计完整性（I4-F 工作项 6）：被拒的提交不产生发布审计；归档本身有审计。
#[sqlx::test(migrations = "../../migrations")]
async fn rejected_writes_leave_no_publish_audit(pool: PgPool) {
    let root = temp_root();
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "I4F-5".to_string(),
            project_name: "i4f".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    ensure_goal_document(&pool, &root, boot.project_id, boot.human_id)
        .await
        .unwrap();
    archive(&boot, &pool).await;

    let _ = pm_mcp::tools::propose_document(
        &pool,
        &root,
        pm_mcp::tools::ProposeDocumentArgs {
            project_id: boot.project_id,
            doc_key: GOAL_DOC_KEY.to_string(),
            doc_type: "knowledge".to_string(),
            markdown: "# Project Goal\n\n审计检查\n\nsource_events: goal-bootstrap\n".to_string(),
            change_summary: Some("审计检查".to_string()),
            source_event_id: None,
            analysis_run_id: None,
            source_ref: Some("goal-bootstrap".to_string()),
            redline_kind: None,
            base_revision_id: None,
            operation_id: format!("i4f-propose-{}", Uuid::new_v4()),
        },
        &test_caller(&boot),
    )
    .await;

    let publish_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND action = 'commit_document_revision'",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        publish_audits, 0,
        "a rejected write must not emit a publish audit"
    );

    let archive_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND action = 'project.archived'",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(archive_audits, 1, "archive itself stays auditable");

    cleanup(&root);
}
