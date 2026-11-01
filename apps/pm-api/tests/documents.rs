//! Knowledge space V1 integration: create (file + commit + rows), list, read,
//! project isolation and drift detection. Every case uses a temporary Git root
//! so the real knowledge repository is never touched.

use std::path::PathBuf;
use std::process::Command;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::knowledge::{
    CreateDocumentRequest, KnowledgeError, KnowledgeRoot, content_hash, create_document,
    list_documents, read_document,
};
use pm_application::knowledge_index::search_documents;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;
use pm_domain::SYSTEM_TENANT;

mod common;

fn temp_root() -> KnowledgeRoot {
    let path: PathBuf = std::env::temp_dir().join(format!("ai-pm-kn-int-{}", Uuid::new_v4()));
    KnowledgeRoot::new(path)
}

fn cleanup(root: &KnowledgeRoot) {
    let _ = std::fs::remove_dir_all(root.path());
}

fn request(doc_key: &str) -> CreateDocumentRequest {
    CreateDocumentRequest {
        doc_key: doc_key.to_string(),
        doc_type: "requirement".to_string(),
        markdown: "# 需求 REQ-032\n\n地图最大尺寸 100,000 × 100,000。\n".to_string(),
        change_summary: Some("initial version".to_string()),
    }
}

async fn project(pool: &PgPool, code: &str) -> pm_application::bootstrap::BootstrapOutcome {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("{code} project"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(pool, boot.project_id).await;
    boot
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_list_and_read_round_trip(pool: PgPool) {
    let boot = project(&pool, "KN-1").await;
    let root = temp_root();

    let created = create_document(&pool, &root, boot.project_id, boot.human_id, request("REQ-032"))
        .await
        .unwrap();
    assert_eq!(created.revision_no, 1);
    assert_eq!(created.canonical_path, "projects/KN-1/requirement/REQ-032.md");
    assert_eq!(created.content_hash, content_hash(&request("REQ-032").markdown));
    assert_eq!(created.git_commit_sha.len(), 40);

    assert!(
        root.path().join(&created.canonical_path).exists(),
        "markdown must exist in the knowledge root"
    );

    let listed = list_documents(&pool, boot.project_id).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.document_id);
    assert_eq!(listed[0].revision_no, 1);

    let read = read_document(&pool, &root, boot.project_id, created.document_id)
        .await
        .unwrap();
    assert!(read.markdown.contains("100,000"));
    assert_eq!(read.git_commit_sha, created.git_commit_sha);
    assert_eq!(read.doc_type, "requirement");

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_creation_does_not_add_a_second_commit(pool: PgPool) {
    let boot = project(&pool, "KN-2").await;
    let root = temp_root();
    let req = request("REQ-001");

    create_document(&pool, &root, boot.project_id, boot.human_id, req.clone())
        .await
        .unwrap();
    let before = commit_count(&root);
    assert!(matches!(
        create_document(&pool, &root, boot.project_id, boot.human_id, req).await,
        Err(KnowledgeError::Exists(_))
    ));
    assert_eq!(commit_count(&root), before, "duplicate must not commit again");

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn documents_are_project_scoped(pool: PgPool) {
    let a = project(&pool, "KN-A").await;
    let b = project(&pool, "KN-B").await;
    let root = temp_root();
    let created = create_document(&pool, &root, a.project_id, a.human_id, request("REQ-9"))
        .await
        .unwrap();

    assert_eq!(list_documents(&pool, b.project_id).await.unwrap().len(), 0);
    assert!(matches!(
        read_document(&pool, &root, b.project_id, created.document_id).await,
        Err(KnowledgeError::NotFound)
    ));

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn drifted_file_is_reported_as_stale(pool: PgPool) {
    let boot = project(&pool, "KN-3").await;
    let root = temp_root();
    let created = create_document(&pool, &root, boot.project_id, boot.human_id, request("REQ-7"))
        .await
        .unwrap();

    std::fs::write(
        root.path().join(&created.canonical_path),
        "# 手改过的正文\n",
    )
    .unwrap();

    assert!(matches!(
        read_document(&pool, &root, boot.project_id, created.document_id).await,
        Err(KnowledgeError::Stale)
    ));

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn rest_list_returns_items_and_requires_project_id(pool: PgPool) {
    let boot = project(&pool, "KN-R").await;
    let app = router(pool);
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/pm/documents?project_id={}", boot.project_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["items"], json!([]));

    let missing = router_requires_project_query().await;
    assert_eq!(missing, StatusCode::BAD_REQUEST);
}

async fn router_requires_project_query() -> StatusCode {
    // Reuse a throwaway pool: the query extractor rejects before any DB access.
    let pool = sqlx::PgPool::connect_lazy("postgres://aipm:aipm@127.0.0.1:5433/aipm").unwrap();
    router(pool)
        .oneshot(
            Request::builder()
                .uri("/v1/pm/documents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[sqlx::test(migrations = "../../migrations")]
async fn search_hits_sections_with_locators(pool: PgPool) {
    let boot = project(&pool, "KN-S").await;
    let root = temp_root();
    create_document(
        &pool,
        &root,
        boot.project_id,
        boot.human_id,
        CreateDocumentRequest {
            doc_key: "ARCH-S".to_string(),
            doc_type: "architecture".to_string(),
            markdown:
                "# 架构约束\n\n## 背景\n\n地图太大。\n\n## 决策\n\n采用 LOD 分块加载。\n"
                    .to_string(),
            change_summary: None,
        },
    )
    .await
    .unwrap();

    let hits = search_documents(&pool, &root, boot.project_id, "lod")
        .await
        .unwrap();
    assert_eq!(hits.len(), 1, "only the 决策 section mentions lod");
    assert_eq!(hits[0].section.title, "决策");
    assert_eq!(hits[0].section.anchor, "决策");
    assert!(hits[0].section.start_line > 1);
    assert_eq!(hits[0].score, 1);

    let none = search_documents(&pool, &root, boot.project_id, "不存在")
        .await
        .unwrap();
    assert!(none.is_empty());

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn rest_search_with_blank_query_is_empty(pool: PgPool) {
    let boot = project(&pool, "KN-Q").await;
    let response = router(pool)
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/pm/documents/search?project_id={}&q=",
                    boot.project_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["items"], json!([]));
}

fn commit_count(root: &KnowledgeRoot) -> usize {
    let output = Command::new("git")
        .arg("-C")
        .arg(root.path())
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().parse().unwrap()
}
