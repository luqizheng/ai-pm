//! `GET /v1/pm/task-runs` 集成测试：真实数据库 + 真 HTTP router（无 socket）。

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

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    let project_id = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;
    common::join_dev(pool, project_id).await;
    project_id
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

#[sqlx::test(migrations = "../../migrations")]
async fn list_exposes_run_kind_and_filters(pool: PgPool) {
    let project_id = seed_project(&pool, "RUNS-3").await;
    let task_a = seed_task(&pool, project_id, "task a").await;
    let task_b = seed_task(&pool, project_id, "task b").await;

    let base = format!("/v1/pm/task-runs?project_id={project_id}");
    let insert = |task_id: Uuid, attempt: i32, run_kind: &str| {
        format!(
            "INSERT INTO delivery.task_run \
                 (project_id, task_id, attempt_no, run_kind, status, executor_kind, executor_session, \
                  lease_expires_at, role_id, review_policy_version, tenant_id) \
             VALUES ('{project_id}', '{task_id}', {attempt}, '{run_kind}', 'succeeded', \
                 {}, {}, now() + interval '5 minutes', {}, {}, \
                 (SELECT tenant_id FROM core.project WHERE id = '{project_id}'))",
            if run_kind == "review" { "'platform'" } else { "NULL" },
            if run_kind == "review" { "NULL" } else { "'sess-1'" },
            if run_kind == "review" { "'reviewer'" } else { "NULL" },
            if run_kind == "review" { "'v1'" } else { "NULL" },
        )
    };
    sqlx::query(&insert(task_a, 1, "delivery"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(&insert(task_b, 1, "delivery"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(&insert(task_b, 1, "review"))
        .execute(&pool)
        .await
        .unwrap();

    // 列表补列：run_kind / executor / lease 直接可见。
    let (status, body) = body_json(router(pool.clone()).oneshot(get(&base)).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["total"], 3);
    let items = body["items"].as_array().unwrap();
    assert!(items.iter().all(|r| r.get("run_kind").is_some()));
    let delivery = items
        .iter()
        .find(|r| r["executor_session"] == "sess-1")
        .unwrap();
    assert_eq!(delivery["run_kind"], "delivery");
    assert!(delivery["lease_expires_at"].is_string());

    // run_kind=review 只出审查轨。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("{base}&run_kind=review")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["run_kind"], "review");

    // task_id 过滤 + 非法 run_kind 视为未传。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("{base}&task_id={task_a}&run_kind=bogus")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["task_title"], "task a");
}

#[sqlx::test(migrations = "../../migrations")]
async fn run_detail_returns_lease_executor_and_repo_baselines(pool: PgPool) {
    let project_id = seed_project(&pool, "RUNS-4").await;
    let task_id = seed_task(&pool, project_id, "detail task").await;
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, tenant_id) \
         VALUES ($1, 'repo-x', 'ssh://git@example.com/repo-x.git', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
             (project_id, task_id, attempt_no, run_kind, status, stage, executor_kind, \
              executor_session, lease_expires_at, claimed_at, heartbeat_at, started_at, \
              deadline_at, tenant_id) \
         VALUES ($1, $2, 1, 'delivery', 'running', 'execute', 'codex', 'sess-42', \
                 now() + interval '10 minutes', now(), now(), now(), \
                 now() + interval '30 minutes', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run_repository \
             (run_id, project_id, repository_id, base_ref, base_commit, write_access, \
              workspace_path, tenant_id) \
         VALUES ($1, $2, $3, 'main', 'abc123', true, '/ws/run/repo-x', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(repo_id)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/task-runs/{run_id}")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["id"], serde_json::json!(run_id));
    assert_eq!(body["task_title"], "detail task");
    assert_eq!(body["run_kind"], "delivery");
    assert_eq!(body["status"], "running");
    assert_eq!(body["stage"], "execute");
    assert_eq!(body["executor_kind"], "codex");
    assert_eq!(body["executor_session"], "sess-42");
    assert!(body["lease_expires_at"].is_string());
    assert!(body["deadline_at"].is_string());
    let repos = body["repositories"].as_array().unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0]["repository_name"], "repo-x");
    assert_eq!(repos[0]["base_ref"], "main");
    assert_eq!(repos[0]["base_commit"], "abc123");
    assert_eq!(repos[0]["workspace_path"], "/ws/run/repo-x");

    // 404：未知 run / 跨租户由 tenant 条件兜底。
    let (status, _) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/task-runs/{}", Uuid::new_v4())))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
