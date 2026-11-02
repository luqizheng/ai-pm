//! T-061 · Outbox 运维端点验收（`docs/ai-pm-t061-outbox-events-v1.md` §4）。
//!
//! 覆盖映射：
//!
//! | 用例 | 断言 |
//! |---|---|
//! | E-10 | 死信列表端点返回 `items` + `total`（项目全部死信数） |
//! | E-11 | 死信列表端点**不泄露**其他项目的死信 |
//! | E-12 | 重放端点放回死信（200 + `replayed: true`） |
//! | E-13 | 重放**跨项目**事件 → 404（与不存在同形，防事件 ID 探测） |
//! | E-14 | 重放**非死信**事件 → 409 |
//! | E-18b | `total` 独立于分页 |
//! | E-22 | 重放写审计（操作者 / 项目 / 事件 ID / 结果），失败也记 |

mod common;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use pm_domain::SYSTEM_TENANT;

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .expect("bootstrap project");
    outcome.project_id
}

/// 直接写一条死信事件。
async fn seed_dead_letter(pool: &PgPool, project_id: Uuid, key: &str) -> Uuid {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.outbox_event \
         (project_id, event_type, aggregate_type, aggregate_id, payload, idempotency_key, \
          status, attempts, last_error, tenant_id) \
         VALUES ($1, 'TaskReady', 'Task', 'agg-1', '{}'::jsonb, $2, 'dead_letter', 5, 'boom', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(key)
    .fetch_one(pool)
    .await
    .expect("seed dead letter");
    id
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn post(uri: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// ---------------------------------------------------------------------------
// E-10 / E-11 / E-18b：死信列表
// ---------------------------------------------------------------------------

/// E-10 + E-11 + E-18b：列表端点返回本项目死信 + 项目总数，且不泄露别项目。
#[sqlx::test(migrations = "../../migrations")]
async fn dead_letter_listing_is_project_scoped(pool: PgPool) {
    let project_a = seed_project(&pool, "T061-API-A").await;
    let project_b = seed_project(&pool, "T061-API-B").await;
    common::join_dev(&pool, project_a).await;
    common::join_dev(&pool, project_b).await;

    seed_dead_letter(&pool, project_a, "a-1").await;
    seed_dead_letter(&pool, project_a, "a-2").await;
    seed_dead_letter(&pool, project_b, "b-1").await;

    let app = router(pool.clone());
    let response = app
        .clone()
        .oneshot(get(&format!(
            "/v1/pm/outbox/dead-letters?project_id={project_a}"
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(
        body["items"].as_array().unwrap().len(),
        2,
        "only this project's dead letters may be listed"
    );
    assert_eq!(
        body["total"], json!(2),
        "total is this project's dead letter count"
    );
    let ids: Vec<String> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect();
    let foreign = seed_dead_letter(&pool, project_b, "b-probe").await;
    assert!(
        !ids.contains(&foreign.to_string()),
        "a foreign event id must never appear in the listing"
    );
}

/// E-18b：`total` 独立于分页（`limit` 不影响 `total`）。
#[sqlx::test(migrations = "../../migrations")]
async fn dead_letter_total_is_independent_of_the_page_size(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-API-TOTAL").await;
    common::join_dev(&pool, project_id).await;
    for idx in 0..3 {
        seed_dead_letter(&pool, project_id, &format!("k-{idx}")).await;
    }

    let app = router(pool.clone());
    let response = app
        .clone()
        .oneshot(get(&format!(
            "/v1/pm/outbox/dead-letters?project_id={project_id}&limit=1"
        )))
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1, "the page is truncated");
    assert_eq!(body["total"], json!(3), "total is the project-wide count");
}

// ---------------------------------------------------------------------------
// E-12 / E-13 / E-14 / E-22：重放
// ---------------------------------------------------------------------------

/// E-12：重放端点放回死信。
#[sqlx::test(migrations = "../../migrations")]
async fn replay_puts_a_dead_letter_back_in_the_queue(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-API-REPLAY").await;
    common::join_dev(&pool, project_id).await;
    let id = seed_dead_letter(&pool, project_id, "k-replay").await;

    let app = router(pool.clone());
    let response = app
        .clone()
        .oneshot(post(&format!(
            "/v1/pm/outbox/events/{id}/replay?project_id={project_id}"
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["replayed"], json!(true));

    let status: String =
        sqlx::query_scalar("SELECT status FROM integration.outbox_event WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "pending");
}

/// E-13：跨项目事件 → 404（与不存在同形）。
#[sqlx::test(migrations = "../../migrations")]
async fn replay_of_a_cross_project_event_is_not_found(pool: PgPool) {
    let project_a = seed_project(&pool, "T061-API-XA").await;
    let project_b = seed_project(&pool, "T061-API-XB").await;
    common::join_dev(&pool, project_a).await;
    common::join_dev(&pool, project_b).await;
    let foreign = seed_dead_letter(&pool, project_b, "b-dead").await;

    let app = router(pool.clone());
    // 调用方有 A 的权限，却构造 B 的 event_id 并携带 A 的 project_id。
    let response = app
        .clone()
        .oneshot(post(&format!(
            "/v1/pm/outbox/events/{foreign}/replay?project_id={project_a}"
        )))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "a cross-project event must be indistinguishable from a missing one"
    );

    // 真正不存在 → 同样的 404。
    let missing = app
        .oneshot(post(&format!(
            "/v1/pm/outbox/events/{}/replay?project_id={project_a}",
            Uuid::new_v4()
        )))
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    // 跨项目事件未被改动。
    let status: String =
        sqlx::query_scalar("SELECT status FROM integration.outbox_event WHERE id = $1")
            .bind(foreign)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "dead_letter");
}

/// E-14：本项目内的非死信事件 → 409。
#[sqlx::test(migrations = "../../migrations")]
async fn replay_of_a_live_event_conflicts(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-API-LIVE").await;
    common::join_dev(&pool, project_id).await;
    let live: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.outbox_event \
         (project_id, event_type, aggregate_type, aggregate_id, payload, idempotency_key, \
          status, tenant_id) \
         VALUES ($1, 'TaskReady', 'Task', 'agg-1', '{}'::jsonb, 'k-live', 'pending', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let app = router(pool.clone());
    let response = app
        .clone()
        .oneshot(post(&format!(
            "/v1/pm/outbox/events/{live}/replay?project_id={project_id}"
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

/// E-22：重放写审计（成功与失败都记）。
#[sqlx::test(migrations = "../../migrations")]
async fn replay_is_audited(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-API-AUDIT").await;
    common::join_dev(&pool, project_id).await;
    let id = seed_dead_letter(&pool, project_id, "k-audit").await;

    let app = router(pool.clone());
    let response = app
        .clone()
        .oneshot(post(&format!(
            "/v1/pm/outbox/events/{id}/replay?project_id={project_id}"
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let audits: Vec<(String, Uuid, serde_json::Value)> = sqlx::query_as(
        "SELECT action, target_id, after_data FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'outbox_dead_letter_replay'",
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(audits.len(), 1, "a replay must leave an audit trail");
    assert_eq!(audits[0].0, "outbox_dead_letter_replay");
    assert_eq!(audits[0].1, id, "the audit names the replayed event");
    assert_eq!(audits[0].2["result"], json!("replayed"));

    // 失败的探测尝试也记（失败尝试本身就是排障信息）。
    let failed = app
        .oneshot(post(&format!(
            "/v1/pm/outbox/events/{}/replay?project_id={project_id}",
            Uuid::new_v4()
        )))
        .await
        .unwrap();
    assert_eq!(failed.status(), StatusCode::NOT_FOUND);
    let failure_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'outbox_dead_letter_replay' \
            AND after_data->>'result' = 'not_found'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failure_audits, 1, "a refused replay must also be audited");
}

// ---------------------------------------------------------------------------
// 监控指标端点
// ---------------------------------------------------------------------------

/// 统计端点返回四状态计数与失败率。
#[sqlx::test(migrations = "../../migrations")]
async fn stats_endpoint_reports_the_outbox_state(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-API-STATS").await;
    common::join_dev(&pool, project_id).await;
    seed_dead_letter(&pool, project_id, "k-dead").await;
    sqlx::query(
        "INSERT INTO integration.outbox_event \
         (project_id, event_type, aggregate_type, aggregate_id, payload, idempotency_key, \
          status, tenant_id) \
         VALUES ($1, 'TaskReady', 'Task', 'agg-2', '{}'::jsonb, 'k-delivered', 'delivered', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .execute(&pool)
    .await
    .unwrap();

    let app = router(pool.clone());
    let response = app
        .clone()
        .oneshot(get(&format!("/v1/pm/outbox/stats?project_id={project_id}")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["by_status"]["dead_letter"], json!(1));
    assert_eq!(body["by_status"]["delivered"], json!(1));
    assert_eq!(body["dead_letter_count"], json!(1));
    assert_eq!(
        body["failure_rate"],
        json!(0.5),
        "failure_rate = dead_letter / (delivered + dead_letter)"
    );
    assert_eq!(body["by_event_type"][0]["event_type"], json!("TaskReady"));
}
