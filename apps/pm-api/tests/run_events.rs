//! S05 执行监控事件面（0013）REST 集成测试：
//! `GET /v1/pm/task-runs/{run_id}/events`（分页 / type 过滤 / 非成员 404）与
//! `GET /v1/pm/task-runs/summary`（项目级聚合）。
//!
//! 打点写入路径已在 `pm-application/tests/run_events.rs` 覆盖；本文件直接
//! SQL 种子事件，只测查询面契约。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
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

async fn seed_project(pool: &PgPool, code: &str, join: bool) -> Uuid {
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
    if join {
        common::join_dev(pool, project_id).await;
    }
    project_id
}

async fn seed_run(pool: &PgPool, project_id: Uuid, attempt_no: i32, status: &str) -> Uuid {
    let outcome: Option<&str> = if status == "queued" {
        None
    } else {
        Some(status)
    };
    sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, outcome, tenant_id) \
         SELECT $1, t.id, $2, $3, $4, (SELECT tenant_id FROM core.project WHERE id = $1) \
           FROM (SELECT id FROM delivery.task WHERE project_id = $1 ORDER BY created_at LIMIT 1) t \
         RETURNING id",
    )
    .bind(project_id)
    .bind(attempt_no)
    .bind(status)
    .bind(outcome)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 任务种子（run 外键需要）。
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

#[allow(dead_code)]
async fn seed_event(
    pool: &PgPool,
    run_id: Uuid,
    event_type: &str,
    tag: &str,
    detail: serde_json::Value,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task_run_event \
         (tenant_id, project_id, task_id, run_id, event_type, lease_generation, dedupe_tag, detail) \
         SELECT r.tenant_id, r.project_id, r.task_id, r.id, $2, 1, $3, $4 \
           FROM delivery.task_run r WHERE r.id = $1 RETURNING id",
    )
    .bind(run_id)
    .bind(event_type)
    .bind(tag)
    .bind(detail)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn events_list_orders_filters_and_hides_non_members(pool: PgPool) {
    let project_id = seed_project(&pool, "RVEV-1", true).await;
    let task_id = seed_task(&pool, project_id, "ev task").await;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'running', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let e1 = seed_event(&pool, run_id, "run_claimed", "", json!({})).await;
    let e2 = seed_event(&pool, run_id, "run_stage_changed", "coding", json!({})).await;
    let _e3 = seed_event(
        &pool,
        run_id,
        "run_finished",
        "",
        json!({"outcome": "failed"}),
    )
    .await;

    // 全量：occurred_at 降序（同刻按插入序回落，断言集合即可）。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/task-runs/{run_id}/events")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["total"], 3);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert!(items.iter().all(|i| i["occurred_at"].is_string()));
    assert_eq!(items[2]["id"], e1.to_string());
    assert_eq!(items[1]["id"], e2.to_string());

    // type 过滤只认白名单值。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!(
                "/v1/pm/task-runs/{run_id}/events?type=run_stage_changed"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["dedupe_tag"], "coding");

    // 非白名单 type 视为未传（不过滤）。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/task-runs/{run_id}/events?type=bogus")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 3);

    // 分页。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!(
                "/v1/pm/task-runs/{run_id}/events?page=2&size=2"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 3);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    // 非成员 404（另一个项目不 join dev-off）。
    let other = seed_project(&pool, "RVEV-1-OUTSIDER", false).await;
    let other_task = seed_task(&pool, other, "hidden").await;
    let hidden_run: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'running', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(other)
    .bind(other_task)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (status, _) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/task-runs/{hidden_run}/events")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // 未知 run 同口径。
    let (status, _) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/task-runs/{}/events", Uuid::new_v4())))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn summary_aggregates_runs_and_events_per_project(pool: PgPool) {
    let project_id = seed_project(&pool, "RVEV-2", true).await;
    let _task = seed_task(&pool, project_id, "summary task").await;
    let _queued = seed_run(&pool, project_id, 1, "queued").await;
    let failed = seed_run(&pool, project_id, 2, "failed").await;
    let _succeeded = seed_run(&pool, project_id, 3, "succeeded").await;
    seed_event(&pool, failed, "run_claimed", "", json!({})).await;
    seed_event(
        &pool,
        failed,
        "run_finished",
        "",
        json!({"outcome": "failed"}),
    )
    .await;

    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!(
                "/v1/pm/task-runs/summary?project_id={project_id}"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["project_id"], project_id.to_string());
    assert_eq!(body["total"], 3);
    assert_eq!(body["active"], 1);
    assert_eq!(body["by_status"]["queued"], 1);
    assert_eq!(body["by_status"]["failed"], 1);
    assert_eq!(body["by_status"]["succeeded"], 1);
    assert_eq!(body["by_outcome"]["failed"], 1);
    assert_eq!(body["events_24h"], 2);
    assert!(body["last_event_at"].is_string());

    // project_id 必填；无可见性时不泄露（404 与既有一致口径）。
    let (status, _) = body_json(
        router(pool.clone())
            .oneshot(get("/v1/pm/task-runs/summary"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let outsider = seed_project(&pool, "RVEV-2-OUTSIDER", false).await;
    let (status, _) = body_json(
        router(pool.clone())
            .oneshot(get(&format!(
                "/v1/pm/task-runs/summary?project_id={outsider}"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
