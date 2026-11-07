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
    // CAS 语义 = expected == 当前 revision（服务端 replace_team_credential 校验）。
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("{}/{}/credential", base_uri(w.tenant_id), id),
        &w.admin_token,
        Some(json!({ "expected_revision": revision, "api_key": "sk-secret-9999abcd" })),
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

// ────────────────────────────────────────────── 项目策略（§10.2 / 0043）

use pm_application::team_llm::resolve_repo_call_config;

async fn seed_project_with_owner(pool: &PgPool, tenant_id: Uuid, owner: Uuid, code: &str) -> Uuid {
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

/// §10.2：项目指定 Team enabled Provider；PUT 带守卫 + CAS；恢复继承 = provider_id 缺省。
#[sqlx::test(migrations = "../../migrations")]
async fn project_policy_set_get_and_inheritance(pool: PgPool) {
    let w = world(pool).await;

    // 造一个项目，owner = admin（is_admin 亦 manager）
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, 'c') RETURNING id",
    )
    .bind(w.tenant_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, 'P-LLM', 'P-LLM', 'planning') RETURNING id",
    )
    .bind(w.tenant_id)
    .bind(customer_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();

    // 两个 enabled Provider
    let (_, p1) = create_provider(
        &w,
        json!({
            "name": "ds-1", "provider_type": "deepseek",
            "endpoint": "https://api.deepseek.com/v1",
            "configuration": { "model": "deepseek-chat" }, "api_key": "sk-1"
        }),
    )
    .await;
    let (_, p2) = create_provider(
        &w,
        json!({
            "name": "ds-2", "provider_type": "deepseek",
            "endpoint": "https://api.deepseek.com/v1",
            "configuration": { "model": "deepseek-chat" }, "api_key": "sk-2"
        }),
    )
    .await;
    let id1 = Uuid::parse_str(p1["id"].as_str().unwrap()).unwrap();
    let id2 = Uuid::parse_str(p2["id"].as_str().unwrap()).unwrap();

    // GET 初始：无指定
    let (status, body) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/projects/{project_id}/ai-model-policies"),
        &w.admin_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["provider_id"], Value::Null);

    // PUT 指定 ds-2
    let (status, body) = call(
        &w.app,
        "PUT",
        &format!("/v1/pm/projects/{project_id}/ai-model-policies"),
        &w.admin_token,
        Some(json!({ "provider_id": id2, "expected_revision": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["provider_id"], json!(id2));
    assert_eq!(body["provider_name"], "ds-2");
    let rev = body["revision"].as_i64().unwrap();

    // CAS 409
    let (status, body) = call(
        &w.app,
        "PUT",
        &format!("/v1/pm/projects/{project_id}/ai-model-policies"),
        &w.admin_token,
        Some(json!({ "provider_id": id1, "expected_revision": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // 恢复继承（provider_id 缺省）
    let (status, body) = call(
        &w.app,
        "PUT",
        &format!("/v1/pm/projects/{project_id}/ai-model-policies"),
        &w.admin_token,
        Some(json!({ "expected_revision": rev })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["provider_id"], Value::Null);
}

/// member 既非租户 admin 也非项目 owner → 403。
#[sqlx::test(migrations = "../../migrations")]
async fn project_policy_forbidden_for_plain_member(pool: PgPool) {
    let w = world(pool).await;
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, 'c') RETURNING id",
    )
    .bind(w.tenant_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, 'P-X', 'P-X', 'planning') RETURNING id",
    )
    .bind(w.tenant_id)
    .bind(customer_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    let (status, _) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/projects/{project_id}/ai-model-policies"),
        &w.member_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// 解析序（§10.3）：项目指定优先且严格——指定非 enabled 不回退 Team 默认。
#[sqlx::test(migrations = "../../migrations")]
async fn resolution_prefers_project_policy_and_does_not_fall_back(pool: PgPool) {
    let w = world(pool).await;
    std::env::set_var("AI_PM_SECRET_KEY", "team-llm-test-secret");

    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, 'c') RETURNING id",
    )
    .bind(w.tenant_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, 'P-R', 'P-R', 'planning') RETURNING id",
    )
    .bind(w.tenant_id)
    .bind(customer_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();

    // Team 级两个 enabled：ds-1（创建序第一）与 ds-2
    let (_, p1) = create_provider(
        &w,
        json!({
            "name": "ds-1", "provider_type": "deepseek",
            "endpoint": "https://api.example.com/v1",
            "configuration": { "model": "m1" }, "api_key": "sk-1"
        }),
    )
    .await;
    let (_, p2) = create_provider(
        &w,
        json!({
            "name": "ds-2", "provider_type": "deepseek",
            "endpoint": "https://api.example.com/v1",
            "configuration": { "model": "m2" }, "api_key": "sk-2"
        }),
    )
    .await;
    let id1 = Uuid::parse_str(p1["id"].as_str().unwrap()).unwrap();
    let id2 = Uuid::parse_str(p2["id"].as_str().unwrap()).unwrap();
    let rev1 = p1["revision"].as_i64().unwrap();

    // 创建态 = configured（带 key），显式 enable 才进入可解析集合（§12 生命周期）。
    let (_, e1) = call(
        &w.app,
        "POST",
        &format!("{}/{}/status", base_uri(w.tenant_id), id1),
        &w.admin_token,
        Some(json!({ "expected_revision": rev1, "action": "enable" })),
    )
    .await;
    assert_eq!(e1["status"], "enabled", "{e1}");
    let (_, e2) = call(
        &w.app,
        "POST",
        &format!("{}/{}/status", base_uri(w.tenant_id), id2),
        &w.admin_token,
        Some(json!({ "expected_revision": p2["revision"].as_i64().unwrap(), "action": "enable" })),
    )
    .await;
    assert_eq!(e2["status"], "enabled", "{e2}");
    let rev2 = e2["revision"].as_i64().unwrap();

    // ② 未指定 → Team 默认 = ds-1（创建序第一）
    let cfg = resolve_repo_call_config(&w.pool, project_id).await.unwrap().unwrap();
    assert_eq!(cfg.provider_id, id1);

    // ① 项目指定 ds-2 → 覆盖 Team 默认
    let (_, _) = call(
        &w.app,
        "PUT",
        &format!("/v1/pm/projects/{project_id}/ai-model-policies"),
        &w.admin_token,
        Some(json!({ "provider_id": id2, "expected_revision": 1 })),
    )
    .await;
    let cfg = resolve_repo_call_config(&w.pool, project_id).await.unwrap().unwrap();
    assert_eq!(cfg.provider_id, id2);
    assert_eq!(cfg.model, "m2");

    // 严格不回退：指定的 ds-2 停用后 → None（Fake 可见），绝不落到 ds-1
    //（CAS：expected == 当前 revision，即 enable 后的 rev2）。
    let (_, d2) = call(
        &w.app,
        "POST",
        &format!("{}/{}/status", base_uri(w.tenant_id), id2),
        &w.admin_token,
        Some(json!({ "expected_revision": rev2, "action": "disable" })),
    )
    .await;
    assert_eq!(d2["status"], "disabled", "{d2}");
    let cfg = resolve_repo_call_config(&w.pool, project_id).await.unwrap();
    assert!(cfg.is_none(), "designated disabled must NOT fall back to ds-1");

    // 清掉指定 → 回到 Team 默认 ds-1
    sqlx::query("DELETE FROM ai.project_ai_model_policy WHERE project_id = $1")
        .bind(project_id)
        .execute(&w.pool)
        .await
        .unwrap();
    let cfg = resolve_repo_call_config(&w.pool, project_id).await.unwrap().unwrap();
    assert_eq!(cfg.provider_id, id1);
    let _ = rev1;
}
