//! S01-04 认知端点集成测试：真实数据库 + 真 HTTP router（无 socket）。
//!
//! 覆盖：人工启动 Run（结构化事实派生）→ 概览 → 逐项确认落知识 →
//! 重复确认 409 → 拒绝保留历史。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod common;

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

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
        "INSERT INTO integration.project_repository \
         (project_id, name, url, role, default_branch, tenant_id) \
         VALUES ($1, 'atlas-platform', 'https://github.com/acme-labs/atlas-platform', \
                 'main', 'main', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(outcome.project_id)
    .execute(pool)
    .await
    .unwrap();

    outcome.project_id
}

#[sqlx::test(migrations = "../../migrations")]
async fn start_run_derives_items_and_confirm_produces_knowledge(pool: PgPool) {
    let project_id = seed(&pool, "COG-1").await;

    // 启动 Run：派生 = 1 条项目身份 + 1 条仓库元数据。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(post_json(
                &format!("/v1/pm/projects/{project_id}/cognition/runs"),
                serde_json::json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["status"], "completed");
    assert_eq!(body["item_count"], 2);

    // 概览：两条 proposed。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/cognition")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["proposed"], 2);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    let item_id = items[0]["id"].as_str().unwrap().to_string();
    assert_eq!(items[0]["state"], "proposed");

    // 逐项确认 → 项目知识落库 + confirmed_count 推进。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(post_json(
                &format!("/v1/pm/projects/{project_id}/cognition/items/{item_id}/decision"),
                serde_json::json!({ "action": "confirm" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["state"], "confirmed");
    let knowledge_id = body["knowledge_id"].as_str().unwrap();

    let knowledge: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.project_knowledge WHERE id = $1")
            .bind(Uuid::parse_str(knowledge_id).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(knowledge, 1);

    // 重复确认 409（每条只产出一份知识）。
    let (status, _) = body_json(
        router(pool.clone())
            .oneshot(post_json(
                &format!("/v1/pm/projects/{project_id}/cognition/items/{item_id}/decision"),
                serde_json::json!({ "action": "confirm" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "../../migrations")]
async fn reject_keeps_history_and_edit_confirm_updates_content(pool: PgPool) {
    let project_id = seed(&pool, "COG-2").await;

    router(pool.clone())
        .oneshot(post_json(
            &format!("/v1/pm/projects/{project_id}/cognition/runs"),
            serde_json::json!({}),
        ))
        .await
        .unwrap();

    let (_, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/cognition")))
            .await
            .unwrap(),
    )
    .await;
    let items = body["items"].as_array().unwrap();
    let first = items[0]["id"].as_str().unwrap().to_string();
    let second = items[1]["id"].as_str().unwrap().to_string();

    // Edit & Confirm：修订正文落知识（§6.1）。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(post_json(
                &format!("/v1/pm/projects/{project_id}/cognition/items/{first}/decision"),
                serde_json::json!({
                    "action": "confirm",
                    "content": "Project COG-2 — edited by reviewer"
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let knowledge_content: (String,) =
        sqlx::query_as("SELECT content FROM ai.project_knowledge WHERE id = $1")
            .bind(Uuid::parse_str(body["knowledge_id"].as_str().unwrap()).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(knowledge_content.0, "Project COG-2 — edited by reviewer");

    // 拒绝保留历史（§6.4）。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(post_json(
                &format!("/v1/pm/projects/{project_id}/cognition/items/{second}/decision"),
                serde_json::json!({ "action": "reject", "note": "stale repo" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["state"], "rejected");
    let rejected: (Option<Uuid>, Option<String>) =
        sqlx::query_as("SELECT decided_by, decision_note FROM ai.cognition_item WHERE id = $1")
            .bind(Uuid::parse_str(&second).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(rejected.0.is_some());
    assert_eq!(rejected.1.as_deref(), Some("stale repo"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn non_member_gets_not_found(pool: PgPool) {
    // 不 join_dev：dev principal 与项目无成员关系 → 404（不泄漏存在性）。
    let project_id = seed(&pool, "COG-3").await;
    sqlx::query("DELETE FROM core.project_member WHERE project_id = $1")
        .bind(project_id)
        .execute(&pool)
        .await
        .unwrap();

    let (status, _) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/cognition")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
