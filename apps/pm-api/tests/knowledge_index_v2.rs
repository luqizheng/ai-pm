//! T-060 知识索引 REST 层验收（设计稿 §8 K-12 / K-20）。
//!
//! - **K-12**：检索响应的字段集合与改造前一致——`revision_id` / `revision_hash`
//!   是内部证据字段，**不进 JSON**。
//! - **K-20**：故障不伪装空结果——索引层整体不可用 → 整项目回退直读且 `200`；
//!   权威元数据不可用 → `503 KNOWLEDGE_INDEX_UNAVAILABLE`；真无匹配才 `200` 空。
//!
//! 三个用例共用一个临时 Git 知识根（`AI_PM_KNOWLEDGE_ROOT` 是进程级环境变量，
//! 并行用例各自改写会互相污染），因此合并为一条顺序执行的用例。

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::knowledge::{create_document, CreateDocumentRequest, KnowledgeRoot};
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use pm_domain::SYSTEM_TENANT;

mod common;

fn temp_root() -> KnowledgeRoot {
    let path: PathBuf = std::env::temp_dir().join(format!("ai-pm-kn-rest-{}", Uuid::new_v4()));
    KnowledgeRoot::new(path)
}

fn cleanup(root: &KnowledgeRoot) {
    let _ = std::fs::remove_dir_all(root.path());
}

async fn body_of(response: axum::response::Response) -> Value {
    serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn k12_and_k20_search_response_contract_and_failure_grading(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "KN-R2".to_string(),
            project_name: "kn-r2".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(&pool, boot.project_id).await;

    let root = temp_root();
    std::env::set_var("AI_PM_KNOWLEDGE_ROOT", root.path());

    create_document(
        &pool,
        &root,
        boot.project_id,
        boot.human_id,
        CreateDocumentRequest {
            doc_key: "REST-R2".to_string(),
            doc_type: "architecture".to_string(),
            markdown: "# 架构约束\n\n## 决策\n\n采用 LOD 分块加载。\n".to_string(),
            change_summary: Some("v1".to_string()),
            operation_id: format!("rest-r2-{}", Uuid::new_v4()),
            base_revision_id: None,
        },
    )
    .await
    .unwrap();

    let search_uri = format!(
        "/v1/pm/documents/search?project_id={}&q=lod",
        boot.project_id
    );

    // ── K-12：响应字段集合与改造前一致 ─────────────────────────────
    let response = router(pool.clone())
        .oneshot(
            Request::builder()
                .uri(&search_uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_of(response).await;
    let items = body["items"].as_array().expect("items must be an array");
    assert_eq!(items.len(), 1);
    let hit = &items[0];
    assert!(
        hit.get("section").is_some(),
        "section must stay in the payload"
    );
    assert!(hit.get("score").is_some(), "score must stay in the payload");
    assert!(
        hit.get("revision_id").is_none(),
        "revision_id is an internal evidence field and must not be serialized"
    );
    assert!(
        hit.get("revision_hash").is_none(),
        "revision_hash is an internal evidence field and must not be serialized"
    );
    // 响应结构不扩展：无 total / has_more（§6.3）。
    assert!(body.get("total").is_none());
    assert!(body.get("has_more").is_none());

    // ── 真无匹配 → 200 空结果 ──────────────────────────────────────
    let empty = router(pool.clone())
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/pm/documents/search?project_id={}&q=不存在的关键词",
                    boot.project_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(empty.status(), StatusCode::OK);
    assert_eq!(body_of(empty).await["items"], serde_json::json!([]));

    // ── K-20①：索引层整体不可用 → 整项目回退直读，仍 200 且有结果 ──
    sqlx::query("DROP TABLE ai.document_section_index")
        .execute(&pool)
        .await
        .unwrap();
    let degraded = router(pool.clone())
        .oneshot(
            Request::builder()
                .uri(&search_uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        degraded.status(),
        StatusCode::OK,
        "an unavailable index layer must fall back, not fail"
    );
    let degraded_body = body_of(degraded).await;
    let degraded_items = degraded_body["items"].as_array().unwrap();
    assert!(
        !degraded_items.is_empty(),
        "the fallback path must still return the authoritative hit"
    );

    // ── K-20②：权威元数据不可用 → 503，绝不是 200 空 ──────────────
    sqlx::query("DROP TABLE ai.document_revision CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    let unavailable = router(pool.clone())
        .oneshot(
            Request::builder()
                .uri(&search_uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        unavailable.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "authoritative metadata must not be reported as an empty result"
    );
    let unavailable_body = body_of(unavailable).await;
    assert_eq!(
        unavailable_body["code"], "KNOWLEDGE_INDEX_UNAVAILABLE",
        "the 503 body must carry the explicit error code"
    );
    assert!(
        unavailable_body.get("items").is_none(),
        "a failure response must not pretend to be a result set"
    );

    cleanup(&root);
}
