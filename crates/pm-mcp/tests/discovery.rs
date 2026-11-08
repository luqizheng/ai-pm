//! T-085 `pm_discovery_*` 六工具测试：session 解析（空/显式/活跃优先）、
//! findings 提交与 kind 校验、answers 采集、propose-only 断言（不直写
//! `ai.project_document`，红线 held 回填 document_proposal_id）、complete 状态机。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::discovery::create_session;
use pm_domain::SYSTEM_TENANT;

/// T-088：直调工具函数用的测试调用人（SYSTEM_TENANT 成员；设备声明缺席）。
fn test_caller(human_id: Uuid) -> pm_application::caller::CallerContext {
    pm_application::caller::CallerContext::new(
        pm_application::caller::HumanActor {
            principal_id: human_id,
            tenant_id: SYSTEM_TENANT,
            identity_id: None,
            client: Some("test".into()),
            auth_mode: "api_key".into(),
        },
        pm_application::caller::DeviceClaim::absent(),
    )
}

use pm_mcp::tools::{
    discovery_complete, discovery_get, discovery_get_context, discovery_submit_answers,
    discovery_submit_findings, discovery_submit_proposal, DiscoveryAnswerInput,
    DiscoveryCompleteArgs, DiscoveryFindingInput, DiscoveryGetArgs, DiscoveryQuestionInput,
    DiscoverySubmitAnswersArgs, DiscoverySubmitFindingsArgs, DiscoverySubmitProposalArgs,
    ToolError,
};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

fn seed(code: &str) -> BootstrapRequest {
    BootstrapRequest {
        project_code: code.to_string(),
        project_name: format!("project {code}"),
        tenant_id: SYSTEM_TENANT,
    }
}

fn with_env() {
    std::env::set_var("AI_PM_SECRET_KEY", "mcp-discovery-test-key");
    let root = std::env::temp_dir().join(format!("ai-pm-disc-knowledge-{}", Uuid::new_v4()));
    std::env::set_var("AI_PM_KNOWLEDGE_ROOT", root);
}

fn finding_input(kind: &str, content: &str, confidence: Option<f64>) -> DiscoveryFindingInput {
    DiscoveryFindingInput {
        kind: kind.to_string(),
        content: content.to_string(),
        confidence,
        evidence: serde_json::json!([
            {"repository": "backend", "file": "src/payment/mod.rs", "description": "存在未实现的支付接口"}
        ]),
        repository_ids: vec![],
        actor_role: None,
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn discovery_get_resolves_live_session_or_answers_not_found(pool: PgPool) {
    let boot = bootstrap_project(&pool, seed("MCP-D1")).await.unwrap();

    let err = discovery_get(
        &pool,
        DiscoveryGetArgs {
            project_id: boot.project_id,
            session_id: None,
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&err, ToolError::NotFound(message) if message.contains("no discovery session")),
        "empty project must answer not-found: {err}"
    );

    let session = create_session(
        &pool,
        SYSTEM_TENANT,
        boot.project_id,
        "CODEX",
        "INITIAL_ONBOARDING",
        boot.human_id,
    )
    .await
    .unwrap();
    let detail = discovery_get(
        &pool,
        DiscoveryGetArgs {
            project_id: boot.project_id,
            session_id: None,
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap();
    assert_eq!(detail["session"]["id"], json!(session.id));
    assert_eq!(detail["session"]["status"], "IN_PROGRESS");
    assert_eq!(detail["findings"].as_array().unwrap().len(), 0);

    let ghost = discovery_get(
        &pool,
        DiscoveryGetArgs {
            project_id: boot.project_id,
            session_id: Some(Uuid::new_v4()),
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap_err();
    assert!(matches!(ghost, ToolError::NotFound(_)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn discovery_submit_findings_roundtrip_and_validates(pool: PgPool) {
    let boot = bootstrap_project(&pool, seed("MCP-D2")).await.unwrap();
    let session = create_session(
        &pool,
        SYSTEM_TENANT,
        boot.project_id,
        "CODEX",
        "INITIAL_ONBOARDING",
        boot.human_id,
    )
    .await
    .unwrap();

    let result = discovery_submit_findings(
        &pool,
        DiscoverySubmitFindingsArgs {
            project_id: boot.project_id,
            session_id: session.id,
            findings: vec![
                finding_input("observed", "仓库使用 cargo workspace", Some(0.9)),
                finding_input("speculation", "支付模块可能尚未完成", Some(0.78)),
            ],
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap();
    assert_eq!(result["added"], 2);
    assert_eq!(result["findings"][0]["kind"], "observed");
    assert_eq!(result["findings"][1]["verification_status"], "PENDING");

    let detail = discovery_get(
        &pool,
        DiscoveryGetArgs {
            project_id: boot.project_id,
            session_id: Some(session.id),
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap();
    let findings = detail["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 2);
    assert_eq!(
        findings[0]["evidence"][0]["file"],
        json!("src/payment/mod.rs")
    );

    let empty = discovery_submit_findings(
        &pool,
        DiscoverySubmitFindingsArgs {
            project_id: boot.project_id,
            session_id: session.id,
            findings: vec![],
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap_err();
    assert!(matches!(empty, ToolError::InvalidRequest(_)));

    let bad_kind = discovery_submit_findings(
        &pool,
        DiscoverySubmitFindingsArgs {
            project_id: boot.project_id,
            session_id: session.id,
            findings: vec![finding_input("rumor", "x", None)],
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&bad_kind, ToolError::InvalidRequest(message) if message.contains("kind")),
        "invalid kind must be a tool error: {bad_kind}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn discovery_submit_answers_collects_questions_and_answers(pool: PgPool) {
    let boot = bootstrap_project(&pool, seed("MCP-D3")).await.unwrap();
    let session = create_session(
        &pool,
        SYSTEM_TENANT,
        boot.project_id,
        "CODEX",
        "INITIAL_ONBOARDING",
        boot.human_id,
    )
    .await
    .unwrap();

    let result = discovery_submit_answers(
        &pool,
        DiscoverySubmitAnswersArgs {
            project_id: boot.project_id,
            session_id: session.id,
            questions: vec![DiscoveryQuestionInput {
                question: "支付模块由谁实现？".to_string(),
                why_blocking: Some("阻塞架构认知".to_string()),
                finding_id: None,
            }],
            answers: vec![],
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap();
    assert_eq!(result["questions_added"], 1);

    let detail = discovery_get(
        &pool,
        DiscoveryGetArgs {
            project_id: boot.project_id,
            session_id: Some(session.id),
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap();
    let question_id: Uuid =
        Uuid::parse_str(detail["questions"][0]["id"].as_str().unwrap()).unwrap();

    let answered = discovery_submit_answers(
        &pool,
        DiscoverySubmitAnswersArgs {
            project_id: boot.project_id,
            session_id: session.id,
            questions: vec![],
            answers: vec![DiscoveryAnswerInput {
                question_id,
                answer: "外部支付服务实现".to_string(),
            }],
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap();
    assert_eq!(answered["answers_submitted"], 1);

    let detail = discovery_get(
        &pool,
        DiscoveryGetArgs {
            project_id: boot.project_id,
            session_id: Some(session.id),
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap();
    assert_eq!(detail["questions"][0]["status"], "answered");

    let both_empty = discovery_submit_answers(
        &pool,
        DiscoverySubmitAnswersArgs {
            project_id: boot.project_id,
            session_id: session.id,
            questions: vec![],
            answers: vec![],
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap_err();
    assert!(matches!(both_empty, ToolError::InvalidRequest(_)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn discovery_context_lists_documents_repositories_and_open_questions(pool: PgPool) {
    with_env();
    let boot = bootstrap_project(&pool, seed("MCP-D4")).await.unwrap();
    let session = create_session(
        &pool,
        SYSTEM_TENANT,
        boot.project_id,
        "WEB",
        "INITIAL_ONBOARDING",
        boot.human_id,
    )
    .await
    .unwrap();
    discovery_submit_answers(
        &pool,
        DiscoverySubmitAnswersArgs {
            project_id: boot.project_id,
            session_id: session.id,
            questions: vec![DiscoveryQuestionInput {
                question: "部署形态？".to_string(),
                why_blocking: None,
                finding_id: None,
            }],
            answers: vec![],
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap();

    let context = discovery_get_context(
        &pool,
        DiscoveryGetArgs {
            project_id: boot.project_id,
            session_id: Some(session.id),
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap();
    assert_eq!(context["session"]["id"], json!(session.id));
    assert_eq!(context["documents"].as_array().unwrap().len(), 0);
    assert_eq!(context["repositories"].as_array().unwrap().len(), 0);
    assert_eq!(context["open_questions"].as_array().unwrap().len(), 1);
}

/// Propose-only 红线：提案走 `propose_document` 链且红线 held 时，
/// `ai.project_document` 不得出现新文档；`discovery_proposal` 记录 pending
/// 并回填 `document_proposal_id`。
#[sqlx::test(migrations = "../../migrations")]
async fn discovery_submit_proposal_is_propose_only(pool: PgPool) {
    with_env();
    let boot = bootstrap_project(&pool, seed("MCP-D5")).await.unwrap();
    let session = create_session(
        &pool,
        SYSTEM_TENANT,
        boot.project_id,
        "CODEX",
        "INITIAL_ONBOARDING",
        boot.human_id,
    )
    .await
    .unwrap();

    let result = discovery_submit_proposal(
        &pool,
        &pm_application::knowledge::KnowledgeRoot::from_env(),
        DiscoverySubmitProposalArgs {
            project_id: boot.project_id,
            session_id: session.id,
            action: "add".to_string(),
            target_document: "knowledge/discovery-note.md".to_string(),
            doc_key: "discovery-note".to_string(),
            doc_type: "knowledge".to_string(),
            document_id: None,
            base_revision_id: None,
            proposed_content: "# 发现笔记\n\nsource_events: mcp-discovery\n".to_string(),
            change_summary: Some("discovery round 1".to_string()),
            source_ref: Some(format!("discovery_session:{}", session.id)),
            redline_kind: Some("external_commitment".to_string()),
            operation_id: format!("disc-prop-{}", Uuid::new_v4().simple()),
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap();

    assert_eq!(result["outcome"]["outcome"], "redline_pending");
    let discovery_proposal_id: Uuid =
        Uuid::parse_str(result["discovery_proposal_id"].as_str().unwrap()).unwrap();

    let doc_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.project_document WHERE project_id = $1 AND doc_key = 'discovery-note'",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        doc_count, 0,
        "propose-only: red-line proposal must not create ai.project_document"
    );

    let row: (String, Option<Uuid>) = sqlx::query_as(
        "SELECT status, document_proposal_id FROM discovery.discovery_proposal \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(SYSTEM_TENANT)
    .bind(discovery_proposal_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "pending");
    assert!(
        row.1.is_some(),
        "discovery_proposal must link the knowledge-chain proposal"
    );

    let modify_missing_base = discovery_submit_proposal(
        &pool,
        &pm_application::knowledge::KnowledgeRoot::from_env(),
        DiscoverySubmitProposalArgs {
            project_id: boot.project_id,
            session_id: session.id,
            action: "modify".to_string(),
            target_document: "knowledge/discovery-note.md".to_string(),
            doc_key: "discovery-note".to_string(),
            doc_type: "knowledge".to_string(),
            document_id: None,
            base_revision_id: None,
            proposed_content: "x".to_string(),
            change_summary: None,
            source_ref: None,
            redline_kind: None,
            operation_id: format!("disc-prop-{}", Uuid::new_v4().simple()),
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap_err();
    assert!(matches!(modify_missing_base, ToolError::InvalidRequest(_)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn discovery_complete_follows_the_state_machine(pool: PgPool) {
    let boot = bootstrap_project(&pool, seed("MCP-D6")).await.unwrap();
    let session = create_session(
        &pool,
        SYSTEM_TENANT,
        boot.project_id,
        "CODEX",
        "INITIAL_ONBOARDING",
        boot.human_id,
    )
    .await
    .unwrap();

    let invalid = discovery_complete(
        &pool,
        DiscoveryCompleteArgs {
            project_id: boot.project_id,
            session_id: session.id,
            status: "PAUSED".to_string(),
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap_err();
    assert!(matches!(invalid, ToolError::InvalidRequest(_)));

    let review = discovery_complete(
        &pool,
        DiscoveryCompleteArgs {
            project_id: boot.project_id,
            session_id: session.id,
            status: "REVIEW_REQUIRED".to_string(),
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap();
    assert_eq!(review["status"], "REVIEW_REQUIRED");

    let done = discovery_complete(
        &pool,
        DiscoveryCompleteArgs {
            project_id: boot.project_id,
            session_id: session.id,
            status: "COMPLETED".to_string(),
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap();
    assert_eq!(done["status"], "COMPLETED");

    let again = discovery_complete(
        &pool,
        DiscoveryCompleteArgs {
            project_id: boot.project_id,
            session_id: session.id,
            status: "COMPLETED".to_string(),
        },
        &test_caller(boot.human_id),
    )
    .await
    .unwrap_err();
    assert!(matches!(again, ToolError::Conflict(_)));
}
