//! T-024 Task 依赖端点：写边校验与可见性（验收矩阵 C43–C48 / C57）。
//!
//! 覆盖：建边成功并可双向查询、自依赖/两跳成环/跨项目/重复边/非待办状态一律
//! 409、非项目成员一律 404、DELETE 幂等。领取与级联解锁见
//! `crates/pm-application/tests/task_dependency.rs`。

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

async fn seed_task(pool: &PgPool, project_id: Uuid, title: &str, status: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id) \
         VALUES ($1, $2, $3, 1, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn deps_uri(task_id: Uuid) -> String {
    format!("/v1/pm/tasks/{task_id}/dependencies")
}

fn edge_uri(task_id: Uuid, upstream_id: Uuid) -> String {
    format!("/v1/pm/tasks/{task_id}/dependencies/{upstream_id}?reason=test-cleanup")
}

async fn audit_count(pool: &PgPool, action: &str, task_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE action = $1 AND target_type = 'task' AND target_id = $2",
    )
    .bind(action)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn c43_add_edge_lists_both_directions_and_audits(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "C1").await;
    let project = seed_project(&pool, tenant, customer, "P1", alice).await;
    let upstream = seed_task(&pool, project, "upstream", "draft").await;
    let downstream = seed_task(&pool, project, "downstream", "draft").await;

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = token_for("alice", 1);

    let (status, body) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &deps_uri(downstream),
                &token,
                Some(json!({ "depends_on_task_id": upstream })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert_eq!(body["upstream"][0]["id"], upstream.to_string());
    assert!(body["downstream"].as_array().unwrap().is_empty());

    let (status, body) = body_json(
        app.clone()
            .oneshot(request("GET", &deps_uri(downstream), &token, None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["upstream"].as_array().unwrap().len(), 1);

    let (status, body) = body_json(
        app.clone()
            .oneshot(request("GET", &deps_uri(upstream), &token, None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["downstream"][0]["id"], downstream.to_string());

    assert_eq!(
        audit_count(&pool, "task.dependency.add", downstream).await,
        1
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn c44_self_dependency_is_rejected(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "C1").await;
    let project = seed_project(&pool, tenant, customer, "P1", alice).await;
    let task = seed_task(&pool, project, "self", "draft").await;

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = token_for("alice", 1);

    let (status, body) = body_json(
        app.oneshot(request(
            "POST",
            &deps_uri(task),
            &token,
            Some(json!({ "depends_on_task_id": task })),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["message"], "SELF_DEPENDENCY");

    let edges: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_dependency WHERE project_id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(edges, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn c45_two_hop_cycle_is_rejected(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "C1").await;
    let project = seed_project(&pool, tenant, customer, "P1", alice).await;
    let a = seed_task(&pool, project, "a", "draft").await;
    let b = seed_task(&pool, project, "b", "draft").await;

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = token_for("alice", 1);

    let (status, _) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &deps_uri(b),
                &token,
                Some(json!({ "depends_on_task_id": a })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = body_json(
        app.oneshot(request(
            "POST",
            &deps_uri(a),
            &token,
            Some(json!({ "depends_on_task_id": b })),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["message"], "DEPENDENCY_CYCLE");

    let edges: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_dependency WHERE project_id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(edges, 1, "the cycle-closing edge must not be persisted");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c46_cross_project_dependency_is_rejected(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "C1").await;
    let p1 = seed_project(&pool, tenant, customer, "P1", alice).await;
    let p2 = seed_project(&pool, tenant, customer, "P2", alice).await;
    let local = seed_task(&pool, p1, "local", "draft").await;
    let foreign = seed_task(&pool, p2, "foreign", "draft").await;

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = token_for("alice", 1);

    let (status, body) = body_json(
        app.oneshot(request(
            "POST",
            &deps_uri(local),
            &token,
            Some(json!({ "depends_on_task_id": foreign })),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["message"], "CROSS_PROJECT_DEPENDENCY");

    let edges: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_dependency WHERE task_id = $1")
            .bind(local)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(edges, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn c47_duplicate_edge_is_rejected(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "C1").await;
    let project = seed_project(&pool, tenant, customer, "P1", alice).await;
    let a = seed_task(&pool, project, "a", "draft").await;
    let b = seed_task(&pool, project, "b", "draft").await;

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = token_for("alice", 1);

    let body = || Some(json!({ "depends_on_task_id": a }));
    let (status, _) = body_json(
        app.clone()
            .oneshot(request("POST", &deps_uri(b), &token, body()))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = body_json(
        app.oneshot(request("POST", &deps_uri(b), &token, body()))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["message"], "DUPLICATE_DEPENDENCY");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c48_ready_task_cannot_gain_an_edge(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "C1").await;
    let project = seed_project(&pool, tenant, customer, "P1", alice).await;
    let upstream = seed_task(&pool, project, "upstream", "draft").await;
    let ready = seed_task(&pool, project, "ready", "ready").await;

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = token_for("alice", 1);

    let (status, body) = body_json(
        app.oneshot(request(
            "POST",
            &deps_uri(ready),
            &token,
            Some(json!({ "depends_on_task_id": upstream })),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["message"], "TASK_NOT_PENDING");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c57_non_member_gets_404(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let bob = seed_member(&pool, tenant, "bob").await;
    let customer = seed_customer(&pool, tenant, "C1").await;
    let project = seed_project(&pool, tenant, customer, "P1", alice).await;
    let task = seed_task(&pool, project, "hidden", "draft").await;
    let upstream = seed_task(&pool, project, "upstream", "draft").await;
    let _ = bob;

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let bob_token = token_for("bob", 1);

    let (status, _) = body_json(
        app.clone()
            .oneshot(request("GET", &deps_uri(task), &bob_token, None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &deps_uri(task),
                &bob_token,
                Some(json!({ "depends_on_task_id": upstream })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = body_json(
        app.oneshot(request(
            "DELETE",
            &edge_uri(task, upstream),
            &bob_token,
            None,
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn delete_edge_is_idempotent(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "C1").await;
    let project = seed_project(&pool, tenant, customer, "P1", alice).await;
    let upstream = seed_task(&pool, project, "upstream", "draft").await;
    let downstream = seed_task(&pool, project, "downstream", "draft").await;

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = token_for("alice", 1);

    let (status, _) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &deps_uri(downstream),
                &token,
                Some(json!({ "depends_on_task_id": upstream })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    for _ in 0..2 {
        let (status, _) = body_json(
            app.clone()
                .oneshot(request(
                    "DELETE",
                    &edge_uri(downstream, upstream),
                    &token,
                    None,
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    let (status, body) = body_json(
        app.oneshot(request("GET", &deps_uri(downstream), &token, None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["upstream"].as_array().unwrap().is_empty());
    assert_eq!(
        audit_count(&pool, "task.dependency.remove", downstream).await,
        1,
        "delete must be audited exactly once"
    );
}

// --------------------------------------------------------------------- C66

/// C66（T-025 §六 / ADR-0001 I1+I3）：不同租户之间**读不到也改不了**彼此的依赖边；
/// 用别的租户的任务 id 作为上游时，必须与「任务不存在」同一个 404，不泄露存在性。
#[sqlx::test(migrations = "../../migrations")]
async fn c66_cross_tenant_dependency_access_is_invisible(pool: PgPool) {
    let tenant_a = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant_a, "alice").await;
    let tenant_b = seed_tenant(&pool, "2").await;
    let bob = seed_member(&pool, tenant_b, "bob").await;
    let customer_a = seed_customer(&pool, tenant_a, "CA").await;
    let customer_b = seed_customer(&pool, tenant_b, "CB").await;
    let project_a = seed_project(&pool, tenant_a, customer_a, "PA", alice).await;
    let project_b = seed_project(&pool, tenant_b, customer_b, "PB", bob).await;

    let a_down = seed_task(&pool, project_a, "a-down", "draft").await;
    let a_up = seed_task(&pool, project_a, "a-up", "draft").await;
    let b_down = seed_task(&pool, project_b, "b-down", "draft").await;
    let b_up = seed_task(&pool, project_b, "b-up", "draft").await;

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let alice_token = token_for("alice", 1);
    let bob_token = token_for("bob", 2);

    // B 租户内先建成一条合法边（证明 B 的图本身可用）。
    let (status, _) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &deps_uri(b_down),
                &bob_token,
                Some(json!({ "depends_on_task_id": b_up })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // A 的成员对 B 的任务：读 / 写 / 删一律 404（处理函数按 tenant 过滤任务）。
    for (method, uri) in [
        ("GET", deps_uri(b_down)),
        ("DELETE", edge_uri(b_down, b_up)),
    ] {
        let (status, _) = body_json(
            app.clone()
                .oneshot(request(method, &uri, &alice_token, None))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {uri}");
    }
    let (status, _) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &deps_uri(b_down),
                &alice_token,
                Some(json!({ "depends_on_task_id": b_up })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "A must not write edges on B's task"
    );

    // 跨租户上游：与「不存在」同一个 404（不是 409 的存在性 oracle）。
    let (status, body) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &deps_uri(a_down),
                &alice_token,
                Some(json!({ "depends_on_task_id": b_up })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    assert_eq!(body["message"], "task_not_found");

    // 同租户内的正常路径不受影响（A 的边可建、可读），B 的边仍是 1 条。
    let (status, _) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &deps_uri(a_down),
                &alice_token,
                Some(json!({ "depends_on_task_id": a_up })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, body) = body_json(
        app.oneshot(request("GET", &deps_uri(a_down), &alice_token, None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["upstream"].as_array().unwrap().len(), 1);

    let a_edges: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_dependency WHERE project_id = $1")
            .bind(project_a)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        a_edges, 1,
        "the rejected cross-tenant edge must not be persisted"
    );
    let b_edges: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_dependency WHERE project_id = $1")
            .bind(project_b)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(b_edges, 1, "tenant B's edge must be untouched");
}
