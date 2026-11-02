//! Knowledge write path: the MCP `pm_submit_document` tool creates a
//! Git-backed document (file + commit + identity/revision rows) and refuses
//! duplicates or invalid types. Uses a temporary Git root per test.

use std::path::Path;
use std::process::Command;

use pm_application::bootstrap::{BootstrapOutcome, BootstrapRequest, bootstrap_project};
use pm_application::knowledge::KnowledgeRoot;
use pm_mcp::tools::{submit_document, SubmitDocumentArgs, ToolError};
use sqlx::PgPool;
use uuid::Uuid;
use pm_domain::SYSTEM_TENANT;

fn temp_root() -> KnowledgeRoot {
    KnowledgeRoot::new(std::env::temp_dir().join(format!("ai-pm-doc-write-{}", Uuid::new_v4())))
}

fn cleanup(root: &KnowledgeRoot) {
    let _ = std::fs::remove_dir_all(root.path());
}

/// T-088：直调工具函数的测试调用人（bootstrap 的 owner human，SYSTEM_TENANT）。
fn test_caller(boot: &BootstrapOutcome) -> pm_application::caller::CallerContext {
    pm_application::caller::CallerContext::new(
        pm_application::caller::HumanActor {
            principal_id: boot.human_id,
            tenant_id: SYSTEM_TENANT,
            identity_id: None,
            client: Some("test".into()),
            auth_mode: "api_key".into(),
        },
        pm_application::caller::DeviceClaim::absent(),
    )
}

fn args(project_id: Uuid, doc_key: &str, doc_type: &str) -> SubmitDocumentArgs {
    SubmitDocumentArgs {
        project_id,
        doc_key: doc_key.to_string(),
        doc_type: doc_type.to_string(),
        markdown: "# 架构约束\n\n地图分块 + LOD。\n".to_string(),
        change_summary: Some("initial".to_string()),
        operation_id: format!("doc-write-{doc_key}-{}", Uuid::new_v4()),
    }
}

fn commit_count(root: &Path) -> usize {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().parse().unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn submit_document_writes_file_commit_and_rows(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "DOC-1".to_string(),
            project_name: "doc project".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let root = temp_root();

    let result = submit_document(&pool, &root, args(boot.project_id, "ARCH-900", "architecture"), &test_caller(&boot))
        .await
        .unwrap();

    let canonical = result["canonical_path"].as_str().unwrap();
    assert_eq!(canonical, "projects/DOC-1/architecture/ARCH-900.md");
    assert_eq!(result["revision_no"], 1);
    assert_eq!(result["git_commit_sha"].as_str().unwrap().len(), 40);
    assert!(root.path().join(canonical).exists(), "markdown must exist");

    let docs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.project_document WHERE project_id = $1",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(docs, 1);

    let revisions: (i64, String) = sqlx::query_as(
        "SELECT count(*), min(created_by::text) FROM ai.document_revision WHERE project_id = $1",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revisions.0, 1);
    // Acting principal is the project's ai_pm (knowledge maintainer), not the MCP caller.
    let ai_pm: String = sqlx::query_scalar(
        "SELECT p.id::text FROM core.principal p JOIN core.project_member m ON m.principal_id = p.id \
         WHERE m.project_id = $1 AND p.kind = 'ai_pm' LIMIT 1",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revisions.1, ai_pm);

    let before = commit_count(root.path());
    let duplicate = submit_document(&pool, &root, args(boot.project_id, "ARCH-900", "architecture"), &test_caller(&boot)).await;
    assert!(matches!(duplicate, Err(ToolError::Conflict(_))));
    assert_eq!(commit_count(root.path()), before, "duplicate must not commit");

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn invalid_doc_type_is_rejected_before_writing(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "DOC-2".to_string(),
            project_name: "doc project".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let root = temp_root();

    let result = submit_document(&pool, &root, args(boot.project_id, "X-1", "spreadsheet"), &test_caller(&boot)).await;
    assert!(matches!(result, Err(ToolError::InvalidRequest(_))));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM ai.project_document")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn unknown_project_is_not_found(pool: PgPool) {
    let root = temp_root();
        let synthetic = pm_application::caller::CallerContext::new(
        pm_application::caller::HumanActor {
            principal_id: Uuid::new_v4(),
            tenant_id: SYSTEM_TENANT,
            identity_id: None,
            client: None,
            auth_mode: "api_key".into(),
        },
        pm_application::caller::DeviceClaim::absent(),
    );
    let result = submit_document(&pool, &root, args(Uuid::new_v4(), "ARCH-1", "architecture"), &synthetic).await;
    assert!(matches!(result, Err(ToolError::NotFound(_))));
    cleanup(&root);
}