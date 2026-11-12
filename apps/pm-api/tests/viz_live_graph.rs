//! Live 图查询集成测试（V0-3 接线）。
//!
//! 覆盖：追踪图 / Task DAG 的响应结构、一致性标记（单条 SQL → current）、
//! 端点双端隔离（跨项目边不渗入）、未知视图 404。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::auth::AuthConfig;
use pm_api::routes::router_with_auth_config;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

const PRIVATE_PEM: &str = include_str!("keys/private.pem");
const PUBLIC_PEM: &str = include_str!("keys/public.pem");

fn test_jwks() -> Arc<JWKSCache> {
    Arc::new(JWKSCache::from_pem(PUBLIC_PEM).expect("test public key"))
}

fn token_for(sub: &str, tid: i64) -> String {
    sign_token(
        PRIVATE_PEM,
        &json!({
            "sub": sub, "tid": tid, "username": sub, "roles": [],
            "permissions": [], "aud": "coder-micro-services",
        }),
        Duration::from_secs(300),
    )
    .expect("token")
}

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// 造租户 / 成员 / 客户 / 项目（owner 已入成员表）。返回 (tenant, project, principal)。
async fn seed_scene(pool: &PgPool, seq: i64) -> (Uuid, Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) VALUES ('lmms', $1, $1) RETURNING id",
    )
    .bind(seq.to_string())
    .fetch_one(pool)
    .await
    .unwrap();
    let principal: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) VALUES ('human', $1, true) RETURNING id",
    )
    .bind(format!("owner-{seq}"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal_identity (provider, tenant_id, subject, principal_id) VALUES ('lmms', $1, $2, $3)",
    )
    .bind(tenant)
    .bind(format!("owner-{seq}"))
    .bind(principal)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.tenant_member (tenant_id, principal_id, role) VALUES ($1, $2, 'owner')")
        .bind(tenant)
        .bind(principal)
        .execute(pool)
        .await
        .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, code, name, status) VALUES ($1, $2, $2, 'active') RETURNING id",
    )
    .bind(tenant)
    .bind(format!("C-{seq}"))
    .fetch_one(pool)
    .await
    .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) VALUES ($1, $2, $3, $3, 'active') RETURNING id",
    )
    .bind(tenant)
    .bind(customer)
    .bind(format!("P-{seq}"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) VALUES ($1, $2, $3, 'owner')")
        .bind(tenant)
        .bind(project)
        .bind(principal)
        .execute(pool)
        .await
        .unwrap();
    (tenant, project, principal)
}

/// 造任务（delivery.task 无 created_by 列）。
async fn seed_task(pool: &PgPool, tenant: Uuid, project: Uuid, title: &str, status: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (tenant_id, project_id, title, status) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(title)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_dependency(pool: &PgPool, tenant: Uuid, project: Uuid, task: Uuid, on: Uuid, by: Uuid) {
    sqlx::query(
        "INSERT INTO delivery.task_dependency (tenant_id, project_id, task_id, depends_on_task_id, created_by) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(tenant)
    .bind(project)
    .bind(task)
    .bind(on)
    .bind(by)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn live_trace_graph_returns_current_consistency(pool: PgPool) {
    let (_tenant, project, _principal) = seed_scene(&pool, 9201).await;
    let token = token_for("owner-9201", 9201);

    let app = router_with_auth_config(pool, test_jwks(), AuthConfig::default());
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/pm/visualizations/projects/{project}/live/trace-graph"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::OK, "Live 查询应成功: {body}");
    assert_eq!(body["meta"]["consistency"], "current");
    assert_eq!(body["meta"]["truncated"], false);
    assert_eq!(body["data"]["diagram_type"], "trace-graph");
    assert!(body["data"]["nodes"].is_array());
}

#[sqlx::test(migrations = "../../migrations")]
async fn live_task_dag_returns_edges(pool: PgPool) {
    let (tenant, project, principal) = seed_scene(&pool, 9202).await;
    let task_a = seed_task(&pool, tenant, project, "前置任务", "completed").await;
    let task_b = seed_task(&pool, tenant, project, "依赖任务", "ready").await;
    seed_dependency(&pool, tenant, project, task_b, task_a, principal).await;

    let token = token_for("owner-9202", 9202);
    let app = router_with_auth_config(pool, test_jwks(), AuthConfig::default());
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/pm/visualizations/projects/{project}/live/task-dag"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["edges"].as_array().unwrap().len(), 1);
    // 库内语义方向：task_id（依赖方）→ depends_on_task_id（前置方）
    assert_eq!(body["data"]["edges"][0]["from"], task_b.to_string());
    assert_eq!(body["data"]["edges"][0]["to"], task_a.to_string());
}

#[sqlx::test(migrations = "../../migrations")]
async fn live_graph_cross_project_edges_do_not_leak(pool: PgPool) {
    let (tenant, project1, principal) = seed_scene(&pool, 9203).await;
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, code, name, status) VALUES ($1, 'C2-9203', 'C2', 'active') RETURNING id",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    let project2: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) VALUES ($1, $2, 'P2-9203', 'P2', 'active') RETURNING id",
    )
    .bind(tenant)
    .bind(customer)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) VALUES ($1, $2, $3, 'owner')")
        .bind(tenant)
        .bind(project2)
        .bind(principal)
        .execute(&pool)
        .await
        .unwrap();

    // 两个项目各造一条依赖
    for project in [project1, project2] {
        let a = seed_task(&pool, tenant, project, "T-a", "ready").await;
        let b = seed_task(&pool, tenant, project, "T-b", "ready").await;
        seed_dependency(&pool, tenant, project, b, a, principal).await;
    }

    let token = token_for("owner-9203", 9203);
    let app = router_with_auth_config(pool, test_jwks(), AuthConfig::default());
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/pm/visualizations/projects/{project1}/live/task-dag"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (_status, body) = body_json(response).await;
    assert_eq!(
        body["data"]["edges"].as_array().unwrap().len(),
        1,
        "只应看到本项目的一条边（双端隔离）"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn unknown_view_kind_is_404(pool: PgPool) {
    let (_tenant, project, _principal) = seed_scene(&pool, 9204).await;
    let token = token_for("owner-9204", 9204);

    let app = router_with_auth_config(pool, test_jwks(), AuthConfig::default());
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/pm/visualizations/projects/{project}/live/nope"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

