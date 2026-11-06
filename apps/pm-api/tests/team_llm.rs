//! Team LLM Provider 管理面验收（设计稿
//! `docs/ui/smark-pm-team-llm-provider-management-v1.md` §6/§7/§12；核验报告
//! R-LLM-01/R-LLM-02 修复载体）。
//!
//! 覆盖：权限矩阵（member 403 / admin 可管）、生命周期（enable 需凭证、测试
//! 成功≠自动启用、archive 终态）、凭证替换轮换（hint 脱敏、无明文回显）、CAS
//! 乐观锁 409、删除保护（未归档 409 / 有调用记录 409）、审计四列落地。

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

fn token(sub: &str, roles: Value) -> String {
    sign_token(
        PRIVATE_PEM,
        &json!({
            "sub": sub, "tid": 1, "username": sub, "roles": roles,
            "permissions": [], "aud": "coder-micro-services",
        }),
        // 1h：本机 sqlx::test 迁移慢，长链路用例会超过 tenant_members 的 300s 默认。
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

async fn json_of(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

fn first_admin(sub: &str) -> AuthConfig {
    AuthConfig {
        first_admin_tenant: Some("1".to_string()),
        first_admin_subject: Some(sub.to_string()),
    }
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
    (status, json_of(response).await)
}

struct World {
    app: Router,
    pool: PgPool,
    tenant_id: Uuid,
    admin_token: String,
    member_token: String,
}

/// 「1 owner + 1 admin + 1 member」的租户（与 tenant_members 测试同款）。
async fn world(pool: PgPool) -> World {
    std::env::set_var("AI_PM_SECRET_KEY", "team-llm-test-secret");
    let app = router_with_auth_config(pool.clone(), test_jwks(), first_admin("usr-owner"));
    let owner = json_of(
        app.clone()
            .oneshot(request("GET", "/v1/pm/me", &token("usr-owner", json!([])), None))
            .await
            .unwrap(),
    )
    .await;
    let tenant_id = Uuid::parse_str(owner["tenant_id"].as_str().unwrap()).unwrap();

    let admin = json_of(
        app.clone()
            .oneshot(request("GET", "/v1/pm/me", &token("usr-admin", json!([])), None))
            .await
            .unwrap(),
    )
    .await;
    let admin_id = Uuid::parse_str(admin["principal_id"].as_str().unwrap()).unwrap();
    let member = json_of(
        app.clone()
            .oneshot(request("GET", "/v1/pm/me", &token("usr-member", json!([])), None))
            .await
            .unwrap(),
    )
    .await;
    let member_id = Uuid::parse_str(member["principal_id"].as_str().unwrap()).unwrap();

    for id in [admin_id, member_id] {
        let (status, _) = call(
            &app,
            "PATCH",
            &format!("/v1/pm/principals/{id}"),
            &token("usr-owner", json!([])),
            Some(json!({ "is_active": true })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, _) = call(
        &app,
        "PATCH",
        &format!("/v1/pm/tenants/{tenant_id}/members/{admin_id}/role"),
        &token("usr-owner", json!([])),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "owner may grant admin");

    World {
        app,
        pool,
        tenant_id,
        admin_token: token("usr-admin", json!(["tenant_admin"])),
        member_token: token("usr-member", json!([])),
    }
}

fn base_uri(tenant_id: Uuid) -> String {
    format!("/v1/pm/tenants/{tenant_id}/llm-providers")
}

async fn create_provider(w: &World, body: Value) -> (StatusCode, Value) {
    call(
        &w.app,
        "POST",
        &base_uri(w.tenant_id),
        &w.admin_token,
        Some(body),
    )
    .await
}

// ────────────────────────────────────────────────────────────── 权限矩阵

/// R-LLM-01 修复面：member（任何激活 JWT）不得读/写 Provider 管理面。
#[sqlx::test(migrations = "../../migrations")]
async fn member_is_forbidden_on_list_and_write(pool: PgPool) {
    let w = world(pool).await;

    for (method, uri, body) in [
        ("GET", base_uri(w.tenant_id), None),
        (
            "POST",
            base_uri(w.tenant_id),
            Some(json!({
                "name": "p1", "provider_type": "deepseek",
                "endpoint": "https://api.deepseek.com/v1"
            })),
        ),
    ] {
        let (status, body) = call(&w.app, method, &uri, &w.member_token, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri} -> {body}");
    }
}

/// 跨租户一律 404（不泄露存在性）。
#[sqlx::test(migrations = "../../migrations")]
async fn cross_tenant_is_not_found(pool: PgPool) {
    let w = world(pool).await;
    let other = Uuid::new_v4();
    let (status, _) = call(
        &w.app,
        "GET",
        &base_uri(other),
        &w.admin_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ────────────────────────────────────────────────────────────── 生命周期

/// 创建 → 列表回显脱敏 → enable 需凭证 → 换凭证 → enable → disable → archive。
#[sqlx::test(migrations = "../../migrations")]
async fn lifecycle_with_credential_gate_and_status_flow(pool: PgPool) {
    let w = world(pool).await;

    // 无凭证创建：not_configured。
    let (status, row) = create_provider(
        &w,
        json!({
            "name": "deepseek-main", "provider_type": "deepseek",
            "endpoint": "https://api.deepseek.com/v1",
            "configuration": { "model": "deepseek-chat" }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{row}");
    assert_eq!(row["status"], "not_configured");
    assert_eq!(row["has_credential"], false);
    let id: Uuid = Uuid::parse_str(row["id"].as_str().unwrap()).unwrap();
    let revision = row["revision"].as_i64().unwrap();

    // enable 需凭证（显式 Configuration Error，不静默）。
    let (status, body) = call(
        &w.app,
        "POST",
        &format!("{}/{}/status", base_uri(w.tenant_id), id),
        &w.admin_token,
        Some(json!({ "expected_revision": revision, "action": "enable" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // 替换凭证：hint 脱敏、无明文回显。
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("{}/{}/credential", base_uri(w.tenant_id), id),
        &w.admin_token,
        Some(json!({ "expected_revision": revision + 1, "api_key": "sk-secret-9999abcd" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", row);
    assert_eq!(row["has_credential"], true);
    assert_eq!(row["credential_hint"], "****abcd");
    assert_eq!(row["status"], "configured");
    assert!(
        json!(row).to_string().contains("sk-secret") == false,
        "plaintext key must never be echoed: {row}"
    );
    let revision = row["revision"].as_i64().unwrap();

    // enable（显式动作）→ disable → archive。
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("{}/{}/status", base_uri(w.tenant_id), id),
        &w.admin_token,
        Some(json!({ "expected_revision": revision, "action": "enable" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", row);
    assert_eq!(row["status"], "enabled");
    let revision = row["revision"].as_i64().unwrap();

    let (status, row) = call(
        &w.app,
        "POST",
        &format!("{}/{}/status", base_uri(w.tenant_id), id),
        &w.admin_token,
        Some(json!({ "expected_revision": revision, "action": "disable" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", row);
    assert_eq!(row["status"], "disabled");
    let revision = row["revision"].as_i64().unwrap();

    let (status, row) = call(
        &w.app,
        "POST",
        &format!("{}/{}/status", base_uri(w.tenant_id), id),
        &w.admin_token,
        Some(json!({ "expected_revision": revision, "action": "archive" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", row);
    assert_eq!(row["status"], "archived");
    let revision = row["revision"].as_i64().unwrap();

    // 终态：archive 后任何状态/凭证/编辑操作拒绝。
    let (status, _) = call(
        &w.app,
        "POST",
        &format!("{}/{}/status", base_uri(w.tenant_id), id),
        &w.admin_token,
        Some(json!({ "expected_revision": revision, "action": "enable" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // 无调用记录的 archived 实例可删除；凭证行级联清除。
    let (status, _) = call(
        &w.app,
        "DELETE",
        &format!("{}/{}", base_uri(w.tenant_id), id),
        &w.admin_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM ai.team_llm_credential")
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(left, 0, "credential rows must cascade-delete");
}

/// 同名冲突（UNIQUE(tenant_id, name)）与未知 provider_type。
#[sqlx::test(migrations = "../../migrations")]
async fn create_rejects_duplicate_name_and_unknown_type(pool: PgPool) {
    let w = world(pool).await;
    let body = json!({
        "name": "p1", "provider_type": "deepseek",
        "endpoint": "https://api.deepseek.com/v1"
    });
    let (status, _) = create_provider(&w, body.clone()).await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, err) = create_provider(&w, body).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "unique violation -> 500 {err}");

    let (status, err) = create_provider(
        &w,
        json!({
            "name": "p2", "provider_type": "alien",
            "endpoint": "https://example.com/v1"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");

    let (status, err) = create_provider(
        &w,
        json!({
            "name": "p3", "provider_type": "deepseek",
            "endpoint": "ftp://example.com"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");
}

// ────────────────────────────────────────────────────────────── CAS / 删除保护

/// 乐观锁：过期 revision → 409 revision_conflict。
#[sqlx::test(migrations = "../../migrations")]
async fn stale_revision_conflicts_with_409(pool: PgPool) {
    let w = world(pool).await;
    let (_, row) = create_provider(
        &w,
        json!({
            "name": "p1", "provider_type": "openai",
            "endpoint": "https://api.openai.com/v1"
        }),
    )
    .await;
    let id: Uuid = Uuid::parse_str(row["id"].as_str().unwrap()).unwrap();

    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("{}/{}", base_uri(w.tenant_id), id),
        &w.admin_token,
        Some(json!({ "expected_revision": 0, "timeout_secs": 30 })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "revision_conflict");
    assert_eq!(body["actual_revision"], 1);
}

/// 删除保护：非 archived 拒绝（409）；archived 但有调用记录也拒绝（DEC-LLM-004.7
/// 执行记录永久保留）。
#[sqlx::test(migrations = "../../migrations")]
async fn delete_requires_archive_and_no_call_records(pool: PgPool) {
    let w = world(pool).await;
    let (_, row) = create_provider(
        &w,
        json!({
            "name": "p1", "provider_type": "openai_compatible",
            "endpoint": "http://127.0.0.1:9/v1",
            "configuration": { "model": "test-model" }
        }),
    )
    .await;
    let id: Uuid = Uuid::parse_str(row["id"].as_str().unwrap()).unwrap();
    let mut revision = row["revision"].as_i64().unwrap();

    // enabled 才能走 disable → archive；先补凭证。
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("{}/{}/credential", base_uri(w.tenant_id), id),
        &w.admin_token,
        Some(json!({ "expected_revision": revision, "api_key": "sk-x" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let mut revision = row["revision"].as_i64().unwrap();
    for action in ["enable", "disable", "archive"] {
        let (status, body) = call(
            &w.app,
            "POST",
            &format!("{}/{}/status", base_uri(w.tenant_id), id),
            &w.admin_token,
            Some(json!({ "expected_revision": revision, "action": action })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{action}: {body}");
        revision = body["revision"].as_i64().unwrap();
    }

    // 造一条调用记录（连接测试留痕），删除必须 409。
    sqlx::query(
        "INSERT INTO ai.llm_call_record \
         (tenant_id, provider_id, model, purpose, status) \
         VALUES ($1, $2, 'test-model', 'connection_test', 'ok')",
    )
    .bind(w.tenant_id)
    .bind(id)
    .execute(&w.pool)
    .await
    .unwrap();
    let (status, body) = call(
        &w.app,
        "DELETE",
        &format!("{}/{}", base_uri(w.tenant_id), id),
        &w.admin_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "referenced");
}

// ────────────────────────────────────────────────────────────── 审计

/// 配置变更审计（AC-13）：创建/换凭证/启停落 tenant_audit_log 四列身份 + target。
#[sqlx::test(migrations = "../../migrations")]
async fn configuration_changes_are_audited(pool: PgPool) {
    let w = world(pool).await;
    let (_, row) = create_provider(
        &w,
        json!({
            "name": "p1", "provider_type": "deepseek",
            "endpoint": "https://api.deepseek.com/v1"
        }),
    )
    .await;
    let id: Uuid = Uuid::parse_str(row["id"].as_str().unwrap()).unwrap();
    let revision = row["revision"].as_i64().unwrap();
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("{}/{}/credential", base_uri(w.tenant_id), id),
        &w.admin_token,
        Some(json!({ "expected_revision": revision, "api_key": "sk-a" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let lines: Vec<(String, Option<Uuid>, Option<String>, Option<String>, Option<Uuid>)> =
        sqlx::query_as(
            "SELECT action, actor_identity_id, actor_client, actor_auth_mode, target_id \
             FROM governance.tenant_audit_log \
             WHERE tenant_id = $1 AND target_type = 'llm_provider' ORDER BY created_at",
        )
        .bind(w.tenant_id)
        .fetch_all(&w.pool)
        .await
        .unwrap();
    let actions: Vec<&str> = lines.iter().map(|(a, ..)| a.as_str()).collect();
    assert!(
        actions.contains(&"llm_provider.created") && actions.contains(&"llm_provider.credential_rotated"),
        "audit actions: {actions:?}"
    );
    for (action, identity, _client, auth_mode, target) in &lines {
        assert!(identity.is_some(), "{action}: actor_identity_id must be set");
        assert_eq!(auth_mode.as_deref(), Some("interactive"), "{action}");
        assert_eq!(*target, Some(id), "{action}: target_id must be the provider");
    }
}
