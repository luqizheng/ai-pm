//! Team LLM Model Catalog / Authorization / Policy 验收（Phase 3A）。
//!
//! 设计基线：docs/smark-pm-team-llm-provider-management-phase3-design-v1.md
//! §5/§10/§11（AC-04/05/06/07/08 的 API 面）。覆盖：
//! - 权限矩阵（member 403 / admin 可管 / 跨租户 404）；
//! - 手动登记 + 重名拒绝 + 能力词表校验（AC-04）；
//! - CAS 乐观锁 409；
//! - 策略写入严格校验（enabled ∧ team_wide ∧ provider enabled，AC-07 API 面）；
//! - Team 策略七 Role + 继承语义（AC-06）；
//! - 授权 grant/revoke + 撤销影响面只报告（AC-05）；
//! - 项目覆盖 + Provider 约束严格校验 + 恢复继承（AC-08）；
//! - 审计四列 + target_type='llm_model'。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
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

fn token(sub: &str, roles: Value) -> String {
    sign_token(
        PRIVATE_PEM,
        &json!({
            "sub": sub, "tid": 1, "username": sub, "roles": roles,
            "permissions": [], "aud": "coder-micro-services",
        }),
        Duration::from_secs(3600),
    )
    .expect("test token must sign")
}

fn request(method: &str, uri: &str, token_str: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token_str}"));
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
    token_str: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(request(method, uri, token_str, body))
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

/// 「1 owner + 1 admin + 1 member」的租户（与 team_llm 测试同款基建）。
async fn world(pool: PgPool) -> World {
    std::env::set_var("AI_PM_SECRET_KEY", "team-llm-test-secret");
    let app = router_with_auth_config(pool.clone(), test_jwks(), first_admin("usr-owner"));
    let owner = json_of(
        app.clone()
            .oneshot(request(
                "GET",
                "/v1/pm/me",
                &token("usr-owner", json!([])),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    let tenant_id = Uuid::parse_str(owner["tenant_id"].as_str().unwrap()).unwrap();

    let admin = json_of(
        app.clone()
            .oneshot(request(
                "GET",
                "/v1/pm/me",
                &token("usr-admin", json!([])),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    let admin_id = Uuid::parse_str(admin["principal_id"].as_str().unwrap()).unwrap();
    let member = json_of(
        app.clone()
            .oneshot(request(
                "GET",
                "/v1/pm/me",
                &token("usr-member", json!([])),
                None,
            ))
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

// ─── 测试用固定装置：公网不可达 endpoint（0018 前已启 SSRF 黑名单） ───
const TEST_ENDPOINT: &str = "http://203.0.113.1:9/v1";

async fn create_enabled_provider(w: &World, name: &str) -> Uuid {
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/llm-providers", w.tenant_id),
        &w.admin_token,
        Some(json!({
            "name": name, "provider_type": "openai_compatible",
            "endpoint": TEST_ENDPOINT,
            "configuration": { "model": "legacy-model" }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{row}");
    let id = Uuid::parse_str(row["id"].as_str().unwrap()).unwrap();
    let revision = row["revision"].as_i64().unwrap();
    // openai_compatible 本地端点豁免（code facts §10 V1 例外）：无凭证 enable
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/llm-providers/{id}/status", w.tenant_id),
        &w.admin_token,
        Some(json!({ "action": "enable", "expected_revision": revision })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{row}");
    id
}

/// 登记模型并推到「可用」态（enabled + team_wide 授权）。
async fn create_usable_model(w: &World, provider_id: Uuid, model_id: &str) -> Uuid {
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/llm-models", w.tenant_id),
        &w.admin_token,
        Some(json!({
            "provider_id": provider_id,
            "model_id": model_id,
            "display_name": format!("Display {model_id}"),
            "capabilities": ["text_generation", "reasoning"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{row}");
    let id = Uuid::parse_str(row["id"].as_str().unwrap()).unwrap();
    let revision = row["revision"].as_i64().unwrap();
    let (status, row) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/tenants/{}/llm-models/{id}", w.tenant_id),
        &w.admin_token,
        Some(json!({ "expected_revision": revision, "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{row}");
    let (status, row) = call(
        &w.app,
        "PUT",
        &format!("/v1/pm/tenants/{}/llm-model-authorizations", w.tenant_id),
        &w.admin_token,
        Some(json!({ "items": [{ "model_id": id, "action": "grant" }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{row}");
    id
}

async fn seed_project(w: &World, code: &str) -> Uuid {
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, 'c') RETURNING id",
    )
    .bind(w.tenant_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, $3, $3, 'planning') RETURNING id",
    )
    .bind(w.tenant_id)
    .bind(customer_id)
    .bind(code)
    .fetch_one(&w.pool)
    .await
    .unwrap()
}

// ─────────────────────────────────────────────────────────── 用例

/// 权限矩阵：member 对目录/授权/策略全端点 403；跨租户一律 404。
#[sqlx::test(migrations = "../../migrations")]
async fn catalog_requires_tenant_admin(pool: PgPool) {
    let w = world(pool).await;
    let base = format!("/v1/pm/tenants/{}", w.tenant_id);
    for (method, uri) in [
        ("GET", format!("{base}/llm-models")),
        ("POST", format!("{base}/llm-models")),
        ("PUT", format!("{base}/llm-model-authorizations")),
        ("GET", format!("{base}/llm-model-policies")),
        ("PUT", format!("{base}/llm-model-policies")),
    ] {
        let (status, _) = call(&w.app, method, &uri, &w.member_token, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}");
    }

    // 跨租户（另一个 tenant 的 UUID 路径）→ 404
    let (status, _) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/tenants/{}/llm-models", Uuid::new_v4()),
        &w.admin_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// 手动登记（AC-04）：201；重名拒绝；能力词表校验；列表回读。
#[sqlx::test(migrations = "../../migrations")]
async fn manual_registration_and_duplicate_rejection(pool: PgPool) {
    let w = world(pool).await;
    let provider_id = create_enabled_provider(&w, "p1").await;
    let payload = json!({
        "provider_id": provider_id,
        "model_id": "glm-4-flash",
        "display_name": "GLM 4 Flash",
        "capabilities": ["text_generation"]
    });
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/llm-models", w.tenant_id),
        &w.admin_token,
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{row}");
    assert_eq!(row["enabled"], false, "发现/登记不自动启用（§8.3）");
    assert_eq!(row["source"], "manual");
    assert_eq!(row["provider_name"], "p1");

    // 重名 → 400
    let (status, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/llm-models", w.tenant_id),
        &w.admin_token,
        Some(payload),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // 未知能力 → 400（不编造能力，§8.3）
    let (status, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/llm-models", w.tenant_id),
        &w.admin_token,
        Some(json!({
            "provider_id": provider_id,
            "model_id": "m2", "display_name": "M2",
            "capabilities": ["telepathy"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // 归档 Provider 上登记 → 409
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/llm-providers", w.tenant_id),
        &w.admin_token,
        Some(json!({
            "name": "p2", "provider_type": "openai_compatible",
            "endpoint": TEST_ENDPOINT
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{row}");
    let archived_id = Uuid::parse_str(row["id"].as_str().unwrap()).unwrap();
    let revision = row["revision"].as_i64().unwrap();
    let (status, enabled) = call(
        &w.app,
        "POST",
        &format!(
            "/v1/pm/tenants/{}/llm-providers/{archived_id}/status",
            w.tenant_id
        ),
        &w.admin_token,
        Some(json!({ "action": "enable", "expected_revision": revision })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // archive 的 CAS 基准 = enable 后的 revision（create 时的 1 已被推到 2）
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/llm-providers/{archived_id}/status", w.tenant_id),
        &w.admin_token,
        Some(json!({ "action": "archive", "expected_revision": enabled["revision"].as_i64().unwrap() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{row}");
    let (status, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/llm-models", w.tenant_id),
        &w.admin_token,
        Some(json!({
            "provider_id": archived_id,
            "model_id": "m3", "display_name": "M3"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // 列表回读（provider_id 过滤）
    let (status, body) = call(
        &w.app,
        "GET",
        &format!(
            "/v1/pm/tenants/{}/llm-models?provider_id={provider_id}",
            w.tenant_id
        ),
        &w.admin_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 1);
}

/// 模型更新 CAS 409 + 启停审计（AC-04）。
#[sqlx::test(migrations = "../../migrations")]
async fn model_update_cas_409(pool: PgPool) {
    let w = world(pool).await;
    let provider_id = create_enabled_provider(&w, "p1").await;
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/llm-models", w.tenant_id),
        &w.admin_token,
        Some(json!({
            "provider_id": provider_id,
            "model_id": "m1", "display_name": "M1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{row}");
    let id = Uuid::parse_str(row["id"].as_str().unwrap()).unwrap();

    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/tenants/{}/llm-models/{id}", w.tenant_id),
        &w.admin_token,
        Some(json!({ "expected_revision": 99, "display_name": "X" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // context_window 非正 → 400
    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/tenants/{}/llm-models/{id}", w.tenant_id),
        &w.admin_token,
        Some(json!({ "expected_revision": 1, "context_window": -5 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, row) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/tenants/{}/llm-models/{id}", w.tenant_id),
        &w.admin_token,
        Some(json!({
            "expected_revision": 1,
            "display_name": "M1 renamed",
            "context_window": 128000,
            "enabled": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{row}");
    assert_eq!(row["display_name"], "M1 renamed");
    assert_eq!(row["context_window"], 128000);
    assert_eq!(row["enabled"], true);
    assert_eq!(row["revision"], 2);
}

/// 策略写入严格校验（AC-07 API 面）：disabled / 未授权 / provider 未启用的
/// 模型一律 400——与解析链（3B）同款口径。
#[sqlx::test(migrations = "../../migrations")]
async fn policy_write_requires_usable_model(pool: PgPool) {
    let w = world(pool).await;
    let provider_id = create_enabled_provider(&w, "p1").await;
    let base = format!("/v1/pm/tenants/{}/llm-model-policies", w.tenant_id);

    // 1) 未启用（默认登记态）→ 400
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/llm-models", w.tenant_id),
        &w.admin_token,
        Some(json!({ "provider_id": provider_id, "model_id": "m1", "display_name": "M1" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{row}");
    let disabled_id = Uuid::parse_str(row["id"].as_str().unwrap()).unwrap();
    let (status, body) = call(
        &w.app,
        "PUT",
        &base,
        &w.admin_token,
        Some(json!({ "items": [{ "role_key": "general", "model_id": disabled_id }] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["message"].as_str().unwrap().contains("disabled"));

    // 2) 启用但未授权 → 400
    let revision = row["revision"].as_i64().unwrap();
    let (_status, _) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/tenants/{}/llm-models/{disabled_id}", w.tenant_id),
        &w.admin_token,
        Some(json!({ "expected_revision": revision, "enabled": true })),
    )
    .await;
    let (status, body) = call(
        &w.app,
        "PUT",
        &base,
        &w.admin_token,
        Some(json!({ "items": [{ "role_key": "general", "model_id": disabled_id }] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["message"].as_str().unwrap().contains("not authorized"));

    // 3) 授权后 → 201 级 OK
    let (_status, _) = call(
        &w.app,
        "PUT",
        &format!("/v1/pm/tenants/{}/llm-model-authorizations", w.tenant_id),
        &w.admin_token,
        Some(json!({ "items": [{ "model_id": disabled_id, "action": "grant" }] })),
    )
    .await;
    let (status, body) = call(
        &w.app,
        "PUT",
        &base,
        &w.admin_token,
        Some(json!({ "items": [{ "role_key": "general", "model_id": disabled_id }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // 4) 未知 role_key → 400
    let (status, body) = call(
        &w.app,
        "PUT",
        &base,
        &w.admin_token,
        Some(json!({ "items": [{ "role_key": "product_manager", "model_id": disabled_id }] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

/// Team 策略七 Role 全量视图 + 继承语义（AC-06）。
#[sqlx::test(migrations = "../../migrations")]
async fn team_policies_put_and_inheritance(pool: PgPool) {
    let w = world(pool).await;
    let provider_id = create_enabled_provider(&w, "p1").await;
    let model_id = create_usable_model(&w, provider_id, "glm-4-flash").await;
    let base = format!("/v1/pm/tenants/{}/llm-model-policies", w.tenant_id);

    let (status, body) = call(
        &w.app,
        "PUT",
        &base,
        &w.admin_token,
        Some(json!({ "items": [
            { "role_key": "general", "model_id": model_id },
            { "role_key": "ai_pm_lead", "model_id": model_id }
        ] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 7, "七 Role 全量返回");
    let general = items.iter().find(|r| r["role_key"] == "general").unwrap();
    assert_eq!(general["model_id"], json!(model_id));
    let risk = items
        .iter()
        .find(|r| r["role_key"] == "risk_analysis")
        .unwrap();
    assert_eq!(risk["model_id"], Value::Null);
    assert_eq!(risk["inherit_general"], true, "未配置 Role 继承 general");

    // 删行恢复继承
    let (status, body) = call(
        &w.app,
        "PUT",
        &base,
        &w.admin_token,
        Some(json!({ "items": [{ "role_key": "ai_pm_lead", "model_id": null }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let lead = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["role_key"] == "ai_pm_lead")
        .unwrap();
    assert_eq!(lead["model_id"], Value::Null);
    assert_eq!(lead["inherit_general"], true);
}

/// 授权 grant/revoke + 撤销影响面只报告（AC-05）：撤销不改写策略行。
#[sqlx::test(migrations = "../../migrations")]
async fn authorization_revoke_reports_impact(pool: PgPool) {
    let w = world(pool).await;
    let provider_id = create_enabled_provider(&w, "p1").await;
    let model_id = create_usable_model(&w, provider_id, "glm-4-flash").await;

    // 配一条 Team 策略（制造影响面 = 1）
    let (status, _) = call(
        &w.app,
        "PUT",
        &format!("/v1/pm/tenants/{}/llm-model-policies", w.tenant_id),
        &w.admin_token,
        Some(json!({ "items": [{ "role_key": "general", "model_id": model_id }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // revoke → revoked + team_policy_refs=1（只报告，不自动改写）
    let (status, body) = call(
        &w.app,
        "PUT",
        &format!("/v1/pm/tenants/{}/llm-model-authorizations", w.tenant_id),
        &w.admin_token,
        Some(json!({ "items": [{ "model_id": model_id, "action": "revoke" }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item = &body["items"][0];
    assert_eq!(item["authorization"], "revoked");
    assert_eq!(item["team_policy_refs"], 1);

    // 策略行仍在（§9：撤销不得静默改写项目配置）
    let (status, body) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/tenants/{}/llm-model-policies", w.tenant_id),
        &w.admin_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let general = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["role_key"] == "general")
        .unwrap();
    assert_eq!(general["model_id"], json!(model_id), "策略行不被撤销改写");

    // revoke 后该模型不可再进任何策略面（严格校验）
    let (status, body) = call(
        &w.app,
        "PUT",
        &format!("/v1/pm/tenants/{}/llm-model-policies", w.tenant_id),
        &w.admin_token,
        Some(json!({ "items": [{ "role_key": "ai_pm_lead", "model_id": model_id }] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["message"].as_str().unwrap().contains("not authorized"));
}

/// 项目 Role 覆盖 + Provider 约束严格校验 + 恢复继承（AC-08）。
#[sqlx::test(migrations = "../../migrations")]
async fn project_override_and_provider_constraint(pool: PgPool) {
    let w = world(pool).await;
    let p1 = create_enabled_provider(&w, "p1").await;
    let p2 = create_enabled_provider(&w, "p2").await;
    let m1 = create_usable_model(&w, p1, "model-p1").await;
    let m2 = create_usable_model(&w, p2, "model-p2").await;
    let project_id = seed_project(&w, "P-LLM").await;
    let uri = format!("/v1/pm/projects/{project_id}/ai-model-policies");

    // GET 初始：无约束、无覆盖
    let (status, body) = call(&w.app, "GET", &uri, &w.admin_token, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["provider_constraint"], Value::Null);
    assert_eq!(body["overrides"], json!([]));
    assert_eq!(body["team_default"].as_array().unwrap().len(), 7);

    // 约束 = p1 + 覆盖 m1（同请求）
    let (status, body) = call(
        &w.app,
        "PUT",
        &uri,
        &w.admin_token,
        Some(json!({
            "provider_id": p1,
            "expected_revision": 1,
            "overrides": [{ "role_key": "ai_pm_lead", "model_id": m1 }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["provider_constraint"], json!(p1));
    assert_eq!(body["provider_name"], "p1");
    assert_eq!(body["overrides"].as_array().unwrap().len(), 1);
    let constraint_revision = body["provider_constraint_revision"].as_i64().unwrap();

    // 覆盖 p2 模型 → 400（违反 Provider 约束，§6.2 严格校验）
    let (status, body) = call(
        &w.app,
        "PUT",
        &uri,
        &w.admin_token,
        Some(json!({
            "provider_id": p1,
            "expected_revision": constraint_revision,
            "overrides": [{ "role_key": "general", "model_id": m2 }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["message"].as_str().unwrap().contains("constraint"));

    // 删行恢复继承（overrides 只含 null 项；约束保持）。
    // 注意：违规 PUT（400）的 provider 约束部分幂等生效（同值 upsert 仍推
    // revision）——黑盒回读当前 revision 作为 CAS 基准，不依赖失败请求前的
    // 快照值；「两段式 PUT 非原子」改进挂账 3C。
    let (_, current) = call(&w.app, "GET", &uri, &w.admin_token, None).await;
    let constraint_revision = current["provider_constraint_revision"].as_i64().unwrap();
    let (status, body) = call(
        &w.app,
        "PUT",
        &uri,
        &w.admin_token,
        Some(json!({
            "provider_id": p1,
            "expected_revision": constraint_revision,
            "overrides": [{ "role_key": "ai_pm_lead", "model_id": null }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["overrides"], json!([]), "覆盖已恢复继承");
    assert_eq!(body["provider_constraint"], json!(p1), "约束保持");
}

/// 审计四列 + target_type（§8.2；code facts §6.2 身份口径）。
#[sqlx::test(migrations = "../../migrations")]
async fn audit_four_columns_and_target_type(pool: PgPool) {
    let w = world(pool).await;
    let provider_id = create_enabled_provider(&w, "p1").await;
    let (status, row) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/llm-models", w.tenant_id),
        &w.admin_token,
        Some(json!({ "provider_id": provider_id, "model_id": "m1", "display_name": "M1" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{row}");

    let (actor_id, identity_id, auth_mode, target_type, action): (
        Uuid,
        Option<Uuid>,
        Option<String>,
        String,
        String,
    ) = sqlx::query_as(
        "SELECT actor_id, actor_identity_id, actor_auth_mode, target_type, action \
         FROM governance.tenant_audit_log \
         WHERE tenant_id = $1 AND action = 'llm_model.created' \
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(w.tenant_id)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert_eq!(target_type, "llm_model");
    assert_eq!(action, "llm_model.created");
    assert!(!actor_id.to_string().is_empty());
    assert!(
        auth_mode.is_some() || identity_id.is_some(),
        "四列身份至少落 actor_auth_mode/identity"
    );
}
