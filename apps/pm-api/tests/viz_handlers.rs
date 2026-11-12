//! viz_handlers REST 面集成测试（V0-2 API 层）。
//!
//! 覆盖：创建/读取 roundtrip + 权限矩阵（非成员 404 同形）+ 审批流转 +
//! 解释图发布 400（DV-13）。脚手架沿用 visibility.rs 范式（JWT 签发 +
//! router_with_auth_config）。

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

async fn seed_tenant(pool: &PgPool, external_key: i64) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', $1, $1) RETURNING id",
    )
    .bind(external_key.to_string())
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
         VALUES ($1, $2, $3, $3, 'active') RETURNING id",
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

/// 建一个完整场景：tenant（tid 自增）+ owner 成员 + 项目。返回 (tenant, project)。
async fn seed_scene(pool: &PgPool, seq: i64) -> (Uuid, Uuid) {
    let tenant = seed_tenant(pool, seq).await;
    let owner = seed_member(pool, tenant, &format!("owner-{seq}")).await;
    let customer = seed_customer(pool, tenant, &format!("C-{seq}")).await;
    let project = seed_project(pool, tenant, customer, &format!("P-{seq}"), owner).await;
    (tenant, project)
}

fn create_body(slot: &str, nature: &str) -> Value {
    json!({
        "slot_key": slot,
        "nature": nature,
        "spec": {
            "spec_version": "1.0",
            "slot_key": slot,
            "view": { "kind": "trace-graph" },
            "nodes": [],
            "edges": [],
            "meta": { "budget": "balanced" }
        },
        "spec_hash": "spec-hash-1",
        "source_hash": "source-hash-1",
        "provenance": { "source_domain": "delivery" }
    })
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_and_fetch_artifact_roundtrip(pool: PgPool) {
    let (_tenant, project) = seed_scene(&pool, 8101).await;
    let token = token_for("owner-8101", 8101);
    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());

    // 创建 draft
    let response = app
        .oneshot(request(
            "POST",
            &format!("/v1/pm/visualizations/projects/{project}/artifacts"),
            &token,
            Some(create_body("req-trace", "business")),
        ))
        .await
        .unwrap();
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::OK, "创建应成功: {body}");
    let artifact_id = body["id"].as_str().expect("id").to_string();
    assert_eq!(body["state"], "draft");
    assert_eq!(body["version_no"], 1);

    // 详情读取
    let app = router_with_auth_config(pool, test_jwks(), AuthConfig::default());
    let response = app
        .oneshot(request(
            "GET",
            &format!("/v1/pm/visualizations/artifacts/{artifact_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["slot_key"], "req-trace");
}

#[sqlx::test(migrations = "../../migrations")]
async fn non_member_artifact_read_is_404_same_shape(pool: PgPool) {
    let (tenant, project) = seed_scene(&pool, 8102).await;
    let owner_token = token_for("owner-8102", 8102);

    // owner 创建
    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let response = app
        .oneshot(request(
            "POST",
            &format!("/v1/pm/visualizations/projects/{project}/artifacts"),
            &owner_token,
            Some(create_body("req-trace", "business")),
        ))
        .await
        .unwrap();
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::OK);
    let artifact_id = body["id"].as_str().expect("id").to_string();

    // 同租户非成员读取 → 404 同形（不泄露存在性）
    let outsider_tenant = seed_tenant(&pool, 99998102).await;
    let _ = outsider_tenant;
    let _ = seed_member(&pool, outsider_tenant, "outsider").await;
    let outsider_token = token_for("outsider", 99998102);

    let app = router_with_auth_config(pool, test_jwks(), AuthConfig::default());
    let response = app
        .oneshot(request(
            "GET",
            &format!("/v1/pm/visualizations/artifacts/{artifact_id}"),
            &outsider_token,
            None,
        ))
        .await
        .unwrap();
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "非成员读取必须 404 同形");
    // 404 同形：错误体只带 message（与"真不存在"完全同形），不携带产物任何字段
    assert!(
        body.get("id").is_none() && body.get("spec").is_none(),
        "404 错误体不得泄露产物信息：{body}"
    );
    assert!(body.get("message").is_some(), "错误体应带 message");
    let _ = tenant;
}

#[sqlx::test(migrations = "../../migrations")]
async fn approval_flow_via_api(pool: PgPool) {
    let (_tenant, project) = seed_scene(&pool, 8103).await;
    let owner_token = token_for("owner-8103", 8103);

    // 创建
    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let response = app
        .oneshot(request(
            "POST",
            &format!("/v1/pm/visualizations/projects/{project}/artifacts"),
            &owner_token,
            Some(create_body("req-trace", "business")),
        ))
        .await
        .unwrap();
    let (_status, body) = body_json(response).await;
    let artifact_id = body["id"].as_str().expect("id").to_string();

    // 请求审批
    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let response = app
        .oneshot(request(
            "POST",
            &format!("/v1/pm/visualizations/artifacts/{artifact_id}/approval-requests"),
            &owner_token,
            None,
        ))
        .await
        .unwrap();
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::OK, "请求审批应成功: {body}");
    let approval_id = body["approvalId"].as_str().expect("approvalId").to_string();

    // 批准
    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let response = app
        .oneshot(request(
            "POST",
            &format!("/v1/pm/visualizations/approvals/{approval_id}/decision"),
            &owner_token,
            Some(json!({ "action": "approve" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 产物应已发布
    let app = router_with_auth_config(pool, test_jwks(), AuthConfig::default());
    let response = app
        .oneshot(request(
            "GET",
            &format!("/v1/pm/visualizations/artifacts/{artifact_id}"),
            &owner_token,
            None,
        ))
        .await
        .unwrap();
    let (_status, body) = body_json(response).await;
    assert_eq!(body["state"], "published");
}

#[sqlx::test(migrations = "../../migrations")]
async fn explanatory_artifact_rejects_publish_request_with_400(pool: PgPool) {
    let (_tenant, project) = seed_scene(&pool, 8104).await;
    let owner_token = token_for("owner-8104", 8104);

    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let response = app
        .oneshot(request(
            "POST",
            &format!("/v1/pm/visualizations/projects/{project}/artifacts"),
            &owner_token,
            Some(create_body("explanatory-demo", "explanatory")),
        ))
        .await
        .unwrap();
    let (_status, body) = body_json(response).await;
    let artifact_id = body["id"].as_str().expect("id").to_string();

    // 请求审批 → 400（解释图禁发布，DV-13）
    let app = router_with_auth_config(pool, test_jwks(), AuthConfig::default());
    let response = app
        .oneshot(request(
            "POST",
            &format!("/v1/pm/visualizations/artifacts/{artifact_id}/approval-requests"),
            &owner_token,
            None,
        ))
        .await
        .unwrap();
    let (status, body) = body_json(response).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "解释图发布请求应 400: {body}"
    );
}
