//! 项目 AI Role 目录与启停 REST 契约（V2 D-1=A，迁移 0058；
//! `docs/project-ai-team-v2/ai-pm-ai-team-kickoff-adjudication-pack-v1.md` §二）。
//!
//! 覆盖：bootstrap 种子（5 角色、全 enabled、版本 1.0）、developer 启停往返、
//! `project_manager` 禁用 409（AT-D12）、非授权成员 403、存量项目懒生效视图
//! （无项目行 → 模板全 false + 无版本）。
//!
//! 夹具沿用 `pm_devices.rs` 的种子 harness（tenant external_key = JWT tid）。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::auth::AuthConfig;
use pm_api::routes::router_with_auth_config;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
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
        // 1h：本机 sqlx::test 迁移慢，长链路用例会超过 300s 默认（team_llm 同款）。
        Duration::from_secs(3600),
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

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(request(method, uri, token, body))
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn app(pool: PgPool) -> Router {
    router_with_auth_config(pool, test_jwks(), AuthConfig::default())
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

/// principal + identity + tenant_member 一条龙（role = 租户角色）。
async fn seed_tenant_member(pool: &PgPool, tenant_id: Uuid, sub: &str, role: &str) -> Uuid {
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
         VALUES ($1, $2, $3)",
    )
    .bind(tenant_id)
    .bind(principal)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    principal
}

async fn seed_project_member(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    role: &str,
) {
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
}

/// 手工造项目（不走 bootstrap：存量项目 = 无 AI Role 项目行的懒生效场景）。
async fn seed_project(pool: &PgPool, tenant_id: Uuid, code: &str) -> Uuid {
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, 'c') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, $3, $3, 'planning') RETURNING id",
    )
    .bind(tenant_id)
    .bind(customer_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// bootstrap 建项（走 0058 后的完整种子链，含 5 个 AI Role）。
async fn bootstrapped_project(pool: &PgPool, tenant_id: Uuid, code: &str) -> Uuid {
    bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id,
        },
    )
    .await
    .unwrap()
    .project_id
}

async fn roles_of(app: &Router, token: &str, project_id: Uuid) -> Vec<Value> {
    let (status, body) = call(
        app,
        "GET",
        &format!("/v1/pm/projects/{project_id}/ai-roles"),
        token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body.as_array().expect("roles must be an array").clone()
}

fn role_id_of(roles: &[Value], role_key: &str) -> Uuid {
    let row = roles
        .iter()
        .find(|r| r["role_key"] == role_key)
        .unwrap_or_else(|| panic!("role `{role_key}` must be in the catalog"));
    Uuid::parse_str(row["role_id"].as_str().unwrap()).unwrap()
}

/// bootstrap 建项 → 目录 5 角色全 enabled，pm 有 v1.0 版本锚。
#[sqlx::test(migrations = "../../migrations")]
async fn bootstrap_seeds_five_roles_with_pm_enabled(pool: PgPool) {
    let tenant_id = seed_tenant(&pool, "21").await;
    let owner = seed_tenant_member(&pool, tenant_id, "usr-owner", "owner").await;
    let project_id = bootstrapped_project(&pool, tenant_id, "AR-1").await;
    seed_project_member(&pool, tenant_id, project_id, owner, "owner").await;

    let a = app(pool);
    let token = token_for("usr-owner", 21);
    let roles = roles_of(&a, &token, project_id).await;

    assert_eq!(roles.len(), 5, "{roles:?}");
    for key in [
        "project_manager",
        "requirement_analyst",
        "architect",
        "developer",
        "tester",
    ] {
        let row = roles.iter().find(|r| r["role_key"] == key).unwrap();
        assert_eq!(row["enabled"], true, "{key}");
        assert_eq!(row["current_version"]["version"], "1.0", "{key}");
    }
    let pm = roles
        .iter()
        .find(|r| r["role_key"] == "project_manager")
        .unwrap();
    assert_eq!(pm["category"], "management");
}

/// developer 禁用 → 目录可见 false → 重新启用 → 版本历史含 1.0。
#[sqlx::test(migrations = "../../migrations")]
async fn developer_disable_and_reenable_roundtrip(pool: PgPool) {
    let tenant_id = seed_tenant(&pool, "22").await;
    let owner = seed_tenant_member(&pool, tenant_id, "usr-owner", "owner").await;
    let project_id = bootstrapped_project(&pool, tenant_id, "AR-2").await;
    seed_project_member(&pool, tenant_id, project_id, owner, "owner").await;

    let a = app(pool);
    let token = token_for("usr-owner", 22);
    let roles = roles_of(&a, &token, project_id).await;
    let developer_id = role_id_of(&roles, "developer");

    // 禁用
    let (status, body) = call(
        &a,
        "PUT",
        &format!("/v1/pm/projects/{project_id}/ai-roles/{developer_id}/enabled"),
        &token,
        Some(json!({ "enabled": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["enabled"], false);

    let roles = roles_of(&a, &token, project_id).await;
    let developer = roles.iter().find(|r| r["role_key"] == "developer").unwrap();
    assert_eq!(developer["enabled"], false);
    // 版本锚保留：禁用不抹掉 current_version。
    assert_eq!(developer["current_version"]["version"], "1.0");

    // 重新启用
    let (status, body) = call(
        &a,
        "PUT",
        &format!("/v1/pm/projects/{project_id}/ai-roles/{developer_id}/enabled"),
        &token,
        Some(json!({ "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["enabled"], true);

    // 版本历史（append-only：行已在 bootstrap 建过，往返不再派生新版本）
    let (status, versions) = call(
        &a,
        "GET",
        &format!("/v1/pm/projects/{project_id}/ai-roles/{developer_id}/versions"),
        &token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{versions}");
    let versions = versions.as_array().unwrap();
    assert_eq!(versions.len(), 1, "{versions:?}");
    assert_eq!(versions[0]["version"], "1.0");
}

/// project_manager 不允许项目禁用（AT-D12）→ 409。
#[sqlx::test(migrations = "../../migrations")]
async fn project_manager_is_not_disableable(pool: PgPool) {
    let tenant_id = seed_tenant(&pool, "23").await;
    let owner = seed_tenant_member(&pool, tenant_id, "usr-owner", "owner").await;
    let project_id = bootstrapped_project(&pool, tenant_id, "AR-3").await;
    seed_project_member(&pool, tenant_id, project_id, owner, "owner").await;

    let a = app(pool);
    let token = token_for("usr-owner", 23);
    let roles = roles_of(&a, &token, project_id).await;
    let pm_id = role_id_of(&roles, "project_manager");

    let (status, body) = call(
        &a,
        "PUT",
        &format!("/v1/pm/projects/{project_id}/ai-roles/{pm_id}/enabled"),
        &token,
        Some(json!({ "enabled": false })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // 目录不受影响
    let roles = roles_of(&a, &token, project_id).await;
    let pm = roles
        .iter()
        .find(|r| r["role_key"] == "project_manager")
        .unwrap();
    assert_eq!(pm["enabled"], true);
}

/// 普通成员（非租户 admin、非项目 owner）PUT → 403。
#[sqlx::test(migrations = "../../migrations")]
async fn plain_member_cannot_toggle(pool: PgPool) {
    let tenant_id = seed_tenant(&pool, "24").await;
    let owner = seed_tenant_member(&pool, tenant_id, "usr-owner", "owner").await;
    let member = seed_tenant_member(&pool, tenant_id, "usr-member", "member").await;
    let project_id = bootstrapped_project(&pool, tenant_id, "AR-4").await;
    seed_project_member(&pool, tenant_id, project_id, owner, "owner").await;
    seed_project_member(&pool, tenant_id, project_id, member, "member").await;

    let a = app(pool);
    let roles = roles_of(&a, &token_for("usr-owner", 24), project_id).await;
    let developer_id = role_id_of(&roles, "developer");

    let (status, body) = call(
        &a,
        "PUT",
        &format!("/v1/pm/projects/{project_id}/ai-roles/{developer_id}/enabled"),
        &token_for("usr-member", 24),
        Some(json!({ "enabled": false })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}

/// 存量项目（无项目行）→ 目录呈现懒生效视图：5 模板全 false、无版本。
#[sqlx::test(migrations = "../../migrations")]
async fn legacy_project_lazy_view(pool: PgPool) {
    let tenant_id = seed_tenant(&pool, "25").await;
    let owner = seed_tenant_member(&pool, tenant_id, "usr-owner", "owner").await;
    let project_id = seed_project(&pool, tenant_id, "AR-5").await;
    seed_project_member(&pool, tenant_id, project_id, owner, "owner").await;

    let a = app(pool);
    let token = token_for("usr-owner", 25);
    let roles = roles_of(&a, &token, project_id).await;

    assert_eq!(roles.len(), 5, "{roles:?}");
    for row in &roles {
        assert_eq!(row["enabled"], false, "{}", row["role_key"]);
        assert!(row["current_version"].is_null(), "{}", row["role_key"]);
    }
}
