//! T-022 可见性收口（按 `core.project_member` 过滤）。
//!
//! 覆盖：
//! - 项目列表只列「我参与的项目」，客户列表只列「我参与的客户」+「零项目客户」；
//! - 非成员访问他人客户 → 404；显式 `project_id` 指向他人项目 → 404；
//! - 项目维度读端点（tasks / task-runs / artifacts / audit-logs …）不泄露他人项目；
//! - 建项调用者自动成为 `project_member(owner)`，建完即可见。

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

fn get(uri: &str, token: &str) -> Request<Body> {
    request("GET", uri, token, None)
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

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// 已激活 principal + `(lmms, tid)` 身份映射 + 租户成员行。
async fn seed_member(pool: &PgPool, tenant_id: Uuid, sub: &str) -> Uuid {
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
    .bind(tenant_id)
    .bind(sub)
    .bind(principal)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) \
         VALUES ($1, $2, 'member')",
    )
    .bind(tenant_id)
    .bind(principal)
    .execute(pool)
    .await
    .unwrap();
    principal
}

async fn seed_tenant(pool: &PgPool, external_key: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', $1, $1) RETURNING id",
    )
    .bind(external_key)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_customer(pool: &PgPool, tenant_id: Uuid, code: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, code, name, status) \
         VALUES ($1, $2, $2, 'active') RETURNING id",
    )
    .bind(tenant_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_project(
    pool: &PgPool,
    tenant_id: Uuid,
    customer_id: Uuid,
    code: &str,
    member: Uuid,
) -> Uuid {
    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, $3, $3, 'planning') RETURNING id",
    )
    .bind(tenant_id)
    .bind(customer_id)
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
    .bind(member)
    .execute(pool)
    .await
    .unwrap();
    project_id
}

async fn seed_task(pool: &PgPool, project_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
         VALUES ($1, $2, 'draft', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn ids_of(body: &Value) -> Vec<String> {
    body["items"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row["id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[sqlx::test(migrations = "../../migrations")]
async fn projects_and_customers_are_scoped_by_membership(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let bob = seed_member(&pool, tenant, "bob").await;

    // C1 → P1（alice 成员）；C2 零项目；C3 → P2（bob 成员）。
    let c1 = seed_customer(&pool, tenant, "C1").await;
    let p1 = seed_project(&pool, tenant, c1, "P1", alice).await;
    let c2 = seed_customer(&pool, tenant, "C2").await;
    let c3 = seed_customer(&pool, tenant, "C3").await;
    let p2 = seed_project(&pool, tenant, c3, "P2", bob).await;

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let alice_token = token_for("alice", 1);

    let (status, body) = body_json(
        app.clone()
            .oneshot(get("/v1/pm/projects", &alice_token))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let ids = ids_of(&body);
    assert!(ids.contains(&p1.to_string()), "成员项目必须可见");
    assert!(!ids.contains(&p2.to_string()), "非成员项目不得可见");

    let (status, body) = body_json(
        app.clone()
            .oneshot(get("/v1/pm/customers", &alice_token))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let ids = ids_of(&body);
    assert!(ids.contains(&c1.to_string()), "参与客户的客户必须可见");
    assert!(ids.contains(&c2.to_string()), "零项目客户在租户内可见");
    assert!(!ids.contains(&c3.to_string()), "他人客户不得可见");

    // 非成员访问他人客户 → 404，不泄露存在性。
    let (status, _) = body_json(
        app.clone()
            .oneshot(get(&format!("/v1/pm/customers/{c3}"), &alice_token))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // 非成员 PATCH 他人客户 → 404。
    let (status, _) = body_json(
        app.clone()
            .oneshot(request(
                "PATCH",
                &format!("/v1/pm/customers/{c3}"),
                &alice_token,
                Some(json!({ "name": "hijack" })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // bob 的视角与 alice 对称。
    let bob_token = token_for("bob", 1);
    let (status, body) = body_json(
        app.clone()
            .oneshot(get("/v1/pm/projects", &bob_token))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let ids = ids_of(&body);
    assert!(ids.contains(&p2.to_string()));
    assert!(!ids.contains(&p1.to_string()));
}

#[sqlx::test(migrations = "../../migrations")]
async fn project_scoped_lists_do_not_leak_other_projects(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let bob = seed_member(&pool, tenant, "bob").await;

    let c1 = seed_customer(&pool, tenant, "C1").await;
    let p1 = seed_project(&pool, tenant, c1, "P1", alice).await;
    let c2 = seed_customer(&pool, tenant, "C2").await;
    let p2 = seed_project(&pool, tenant, c2, "P2", bob).await;
    let t1 = seed_task(&pool, p1, "alice-task").await;
    let t2 = seed_task(&pool, p2, "bob-task").await;

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let alice_token = token_for("alice", 1);

    // 不带过滤：只列自己项目的任务。
    let (status, body) = body_json(
        app.clone()
            .oneshot(get("/v1/pm/tasks", &alice_token))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let ids = ids_of(&body);
    assert!(ids.contains(&t1.to_string()));
    assert!(!ids.contains(&t2.to_string()), "他人项目任务不得出现");

    // 显式指向他人项目 → 404。
    let (status, _) = body_json(
        app.clone()
            .oneshot(get(&format!("/v1/pm/tasks?project_id={p2}"), &alice_token))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // 项目文档同样按成员过滤。
    let (status, _) = body_json(
        app.clone()
            .oneshot(get(
                &format!("/v1/pm/documents?project_id={p2}"),
                &alice_token,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn project_creator_becomes_owner_member(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let _alice = seed_member(&pool, tenant, "alice").await;
    let _bob = seed_member(&pool, tenant, "bob").await;
    let customer = seed_customer(&pool, tenant, "C-OPEN").await;

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let bob_token = token_for("bob", 1);

    let (status, created) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                "/v1/pm/projects",
                &bob_token,
                Some(json!({
                    "code": "NEW-P",
                    "name": "new project",
                    "customer_id": customer,
                })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {created}");
    let project_id = created["id"].as_str().unwrap().to_string();

    let member: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.project_member m \
          JOIN core.principal_identity pi ON pi.principal_id = m.principal_id \
          WHERE m.project_id = $1 AND pi.subject = 'bob' AND m.role = 'owner'",
    )
    .bind(Uuid::parse_str(&project_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(member, 1, "创建者必须是 owner 成员");

    let (status, body) = body_json(
        app.clone()
            .oneshot(get("/v1/pm/projects", &bob_token))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        ids_of(&body).contains(&project_id),
        "创建者必须看得见自己的新项目"
    );

    let alice_token = token_for("alice", 1);
    let (status, body) = body_json(
        app.clone()
            .oneshot(get("/v1/pm/projects", &alice_token))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !ids_of(&body).contains(&project_id),
        "同租户非成员不得看见新项目"
    );
}
