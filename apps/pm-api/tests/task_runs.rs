//! `GET /v1/pm/task-runs` 集成测试：真实数据库 + 真 HTTP router（无 socket）。

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;
use pm_domain::SYSTEM_TENANT;

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
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id
}

async fn seed_task(pool: &PgPool, project_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
         VALUES ($1, $2, 'ready', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn lists_runs_with_task_title_and_failure_summary(pool: PgPool) {
    let project_id = seed_project(&pool, "RUNS-1").await;
    let task_id = seed_task(&pool, project_id, "run task").await;

    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(&pool)
    .await
    .unwrap();
    let failed: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, outcome, tenant_id) \
         VALUES ($1, $2, 2, 'failed', 'failed', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run_failure (run_id, project_id, category, retryable, reason_code, tenant_id) \
         VALUES ($1, $2, 'execution', true, 'BOOM', \
                 (SELECT tenant_id FROM delivery.task_run WHERE id = $1))",
    )
    .bind(failed)
    .bind(project_id)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/task-runs?project_id={project_id}")))
            .await
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["total"], 2);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["attempt_no"], 2, "newest attempt first: {body}");
    assert_eq!(items[0]["status"], "failed");
    assert_eq!(items[0]["task_title"], "run task");
    assert_eq!(items[0]["failure_reason"], "BOOM");
    assert_eq!(items[1]["attempt_no"], 1);
    assert_eq!(items[1]["status"], "queued");
    assert!(items[1]["failure_reason"].is_null());
}

#[sqlx::test(migrations = "../../migrations")]
async fn empty_project_returns_no_items(pool: PgPool) {
    let project_id = seed_project(&pool, "RUNS-2").await;

    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/task-runs?project_id={project_id}")))
            .await
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
}