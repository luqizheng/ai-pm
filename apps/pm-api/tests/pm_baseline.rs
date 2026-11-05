//! S01-05 基线端点集成测试：提案派生 → 逐项决策 → 确认落版本 → 不可变。

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod common;

fn get(uri: &str) -> Request<Body> {
    Request::builder().method("GET").uri(uri).body(Body::empty()).unwrap()
}

fn post_json(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn body_json(response: axum::response::Response) -> (StatusCode, serde_json::Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

/// 种子：项目 + 一条 S01-04 已确认知识（提案派生源）。
async fn seed(pool: &PgPool, code: &str) -> Uuid {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("Project {code}"),
            tenant_id: pm_domain::SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(pool, outcome.project_id).await;
    sqlx::query(
        "INSERT INTO ai.project_knowledge \
         (tenant_id, project_id, domain, content, confirmed_by) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, \
                 'identity', 'Confirmed identity from cognition', \
                 (SELECT principal_id FROM core.project_member \
                   WHERE project_id = $1 LIMIT 1))",
    )
    .bind(outcome.project_id)
    .execute(pool)
    .await
    .unwrap();
    outcome.project_id
}

#[sqlx::test(migrations = "../../migrations")]
async fn proposal_decision_and_confirm_flow(pool: PgPool) {
    let project_id = seed(&pool, "BL-1").await;

    // 生成提案：1 条知识条目（无仓库 → 只有知识来源）。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(post_json(
                &format!("/v1/pm/projects/{project_id}/baseline/proposals"),
                serde_json::json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["item_count"], 1);

    // pending 期间重复生成 → 409。
    let (status, _) = body_json(
        router(pool.clone())
            .oneshot(post_json(
                &format!("/v1/pm/projects/{project_id}/baseline/proposals"),
                serde_json::json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // 有 proposed 条目时确认 → 409（未决更正阻塞）。
    let (status, _) = body_json(
        router(pool.clone())
            .oneshot(post_json(
                &format!("/v1/pm/projects/{project_id}/baseline/confirm"),
                serde_json::json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (_, overview) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/baseline")))
            .await
            .unwrap(),
    )
    .await;
    let item_id = overview["items"][0]["id"].as_str().unwrap().to_string();

    // Edit & Confirm：修订正文随决策落库。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(post_json(
                &format!("/v1/pm/projects/{project_id}/baseline/items/{item_id}/decision"),
                serde_json::json!({ "action": "confirm", "content": "Edited baseline identity" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["state"], "confirmed");

    // 确认基线 → v1 版本头。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(post_json(
                &format!("/v1/pm/projects/{project_id}/baseline/confirm"),
                serde_json::json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["version"], 1);

    // 已确认条目不可变（再决策 404——提案非 pending）。
    let (status, _) = body_json(
        router(pool.clone())
            .oneshot(post_json(
                &format!("/v1/pm/projects/{project_id}/baseline/items/{item_id}/decision"),
                serde_json::json!({ "action": "reject" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // 概览呈现当前版本与历史。
    let (_, overview) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/baseline")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(overview["current_version"], 1);
    assert_eq!(overview["history"].as_array().unwrap().len(), 1);

    // 变更走新提案：version 顺延 2。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(post_json(
                &format!("/v1/pm/projects/{project_id}/baseline/proposals"),
                serde_json::json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["version"], 2);
}
