//! `GET /v1/pm/artifacts` 集成测试：真实数据库 + 真 HTTP router（无 socket）。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
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
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(pool, outcome.project_id).await;
    let requirement_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement (project_id, code, title, tenant_id) \
         VALUES ($1, 'REQ-1', '产物需求', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    for (version, state) in [(1_i32, "superseded"), (2_i32, "approved")] {
        sqlx::query(
            "INSERT INTO delivery.requirement_revision \
             (requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, \
                     (SELECT tenant_id FROM core.project WHERE id = $2))",
        )
        .bind(requirement_id)
        .bind(outcome.project_id)
        .bind(version)
        .bind(serde_json::json!({ "markdown": format!("# rev {version}") }))
        .bind(state)
        .bind(format!("hash-{version}"))
        .bind(outcome.human_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    outcome.project_id
}

#[sqlx::test(migrations = "../../migrations")]
async fn lists_requirement_revisions_as_artifacts(pool: PgPool) {
    let project_id = seed(&pool, "ART-1").await;

    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/artifacts?project_id={project_id}")))
            .await
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["total"], 2);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["version"], 2, "newest revision first: {body}");
    assert_eq!(items[0]["state"], "approved");
    assert_eq!(items[0]["requirement_code"], "REQ-1");
    assert_eq!(items[0]["content_hash"], "hash-2");
    assert_eq!(items[1]["version"], 1);
    assert_eq!(items[1]["state"], "superseded");
}

#[sqlx::test(migrations = "../../migrations")]
async fn empty_project_returns_no_items(pool: PgPool) {
    let project_id = seed(&pool, "ART-2").await;
    sqlx::query("DELETE FROM delivery.requirement_revision WHERE project_id = $1")
        .bind(project_id)
        .execute(&pool)
        .await
        .unwrap();

    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/artifacts?project_id={project_id}")))
            .await
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
}
