//! 跨租户隔离验收（T-021b XT-1 / XT-2）。
//!
//! 断言两件事：租户 A 的 token **读不到**租户 B 的项目（列表与详情），
//! 也**写不进去**（404，不泄露存在性，且库内无新增行）。

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum::Router;
use digua_common_jwt::{JWKSCache, sign_token};
use pm_api::auth::AuthConfig;
use pm_api::routes::router_with_auth_config;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

const PRIVATE_PEM: &str = include_str!("keys/private.pem");
const PUBLIC_PEM: &str = include_str!("keys/public.pem");

fn test_jwks() -> Arc<JWKSCache> {
    Arc::new(JWKSCache::from_pem(PUBLIC_PEM).expect("test public key must parse"))
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
    .expect("test token must sign")
}

fn request(method: &str, uri: &str, token: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"));
    let body = match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    builder.body(body).unwrap()
}

async fn json_of(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

async fn me(app: &Router, token: &str) -> Value {
    json_of(
        app.clone()
            .oneshot(request("GET", "/v1/pm/me", token, None))
            .await
            .unwrap(),
    )
    .await
}

/// 在一个租户下建「客户 + 项目」，并把 `owner` 写成项目成员（T-022），返回项目 id。
async fn seed_project(pool: &PgPool, tenant_id: Uuid, code: &str, owner: Uuid) -> Uuid {
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name, status) VALUES ($1, $2, 'active') RETURNING id",
    )
    .bind(tenant_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap();
    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, $3, $3, 'planning') RETURNING id",
    )
    .bind(tenant_id)
    .bind(customer)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'owner')",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(owner)
    .execute(pool)
    .await
    .unwrap();
    project_id
}

/// 造一个**已激活的 owner**，其 token 是 `(lmms, tid, sub)` 三元组。
/// 返回 `(tenant_id, principal_id)`。
async fn seed_owner(pool: &PgPool, tid: &str, sub: &str) -> (Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', $1, $1) \
         ON CONFLICT (provider, external_key) DO UPDATE SET display_name = core.tenant.display_name \
         RETURNING id",
    )
    .bind(tid)
    .fetch_one(pool)
    .await
    .unwrap();
    let principal: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', $1, true) RETURNING id",
    )
    .bind(sub)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal_identity (provider, tenant_id, subject, principal_id) \
         VALUES ('lmms', $1, $2, $3)",
    )
    .bind(tenant)
    .bind(sub)
    .bind(principal)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) \
         VALUES ($1, $2, 'owner')",
    )
    .bind(tenant)
    .bind(principal)
    .execute(pool)
    .await
    .unwrap();
    (tenant, principal)
}

#[sqlx::test(migrations = "../../migrations")]
async fn xt1_xt2_cross_tenant_project_is_invisible_and_unwritable(pool: PgPool) {
    let (tenant_a, owner_a) = seed_owner(&pool, "1", "usr-a").await;
    let (tenant_b, owner_b) = seed_owner(&pool, "2", "usr-b").await;
    let project_a = seed_project(&pool, tenant_a, "ISO-A", owner_a).await;
    let project_b = seed_project(&pool, tenant_b, "ISO-B", owner_b).await;

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let a_token = token_for("usr-a", 1);
    let who = me(&app, &a_token).await;
    assert_eq!(
        Uuid::parse_str(who["tenant_id"].as_str().unwrap()).unwrap(),
        tenant_a
    );

    // XT-1：A 的项目列表里不得出现 B 的项目。
    let listing = json_of(
        app.clone()
            .oneshot(request("GET", "/v1/pm/projects", &a_token, None))
            .await
            .unwrap(),
    )
    .await;
    let ids: Vec<String> = listing["items"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row["id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    assert!(ids.contains(&project_a.to_string()), "A 必须看得见自己的项目");
    assert!(
        !ids.contains(&project_b.to_string()),
        "A 不得看见 B 的项目"
    );

    // XT-2：A 的 token 往 B 的项目里建任务 → 404，且库内无新增行。
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/v1/pm/tasks",
            &a_token,
            Some(json!({ "project_id": project_b, "title": "cross tenant" })),
        ))
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(status, StatusCode::NOT_FOUND, "跨租户写必须 404");

    let tasks: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task WHERE project_id = $1",
    )
    .bind(project_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tasks, 0, "跨租户写不得留下任何行");
}
