//! 主体身份验收（T-021a / `docs/ai-pm-principal-identity-v1.md` §11 C29–C42）。
//!
//! 驱动 `router_with_auth_config`（显式首管理员配置，避免改进程环境变量）；
//! 签名沿用 `tests/keys/` 的测试密钥对。

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use digua_common_jwt::{JWKSCache, sign_token};
use pm_api::auth::AuthConfig;
use pm_api::routes::{router, router_with_auth_config};
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use pm_domain::SYSTEM_TENANT;
use uuid::Uuid;

const PRIVATE_PEM: &str = include_str!("keys/private.pem");
const PUBLIC_PEM: &str = include_str!("keys/public.pem");

fn test_jwks() -> Arc<JWKSCache> {
    Arc::new(JWKSCache::from_pem(PUBLIC_PEM).expect("test public key must parse"))
}

fn sign(claims: Value) -> String {
    sign_token(PRIVATE_PEM, &claims, Duration::from_secs(300)).expect("test token must sign")
}

fn browser_token(sub: &str, roles: Value) -> String {
    sign(json!({
        "sub": sub, "tid": 1, "username": sub, "roles": roles,
        "permissions": [], "aud": "coder-micro-services",
    }))
}

fn api_key_token(sub: &str, roles: Value) -> String {
    // API Key token 没有 `aud`，带 `client_id`（v1 §4 判别规则）。
    sign(json!({
        "sub": sub, "tid": 1, "username": sub, "roles": roles,
        "permissions": [], "client_id": "key-abc",
    }))
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

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn first_admin(sub: &str) -> AuthConfig {
    AuthConfig {
        first_admin_tenant: Some("1".to_string()),
        first_admin_subject: Some(sub.to_string()),
    }
}

/// 与测试 token（`tid: 1`）对齐的租户：真实 auth 链会把 `(lmms, "1")` 镜像成
/// `core.tenant`，夹具必须先建同一行，否则 bootstrap 出来的项目落在别的租户上，
/// 带租户谓词的写路径会返回 404。
async fn lmms_tid1_tenant(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', '1', 'lmms:1') \
         ON CONFLICT (provider, external_key) DO UPDATE SET display_name = core.tenant.display_name \
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 迁移 `0015` 已种子 `ai-pm-dispatch`（kind='service'），因此 `kind='human'`
/// 的计数才是「是否 provision 了新主体」的判据。
async fn human_principals(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM core.principal WHERE kind = 'human'")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn identities(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM core.principal_identity")
        .fetch_one(pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------- C30 / C31

#[sqlx::test(migrations = "../../migrations")]
async fn c30_service_token_is_rejected_without_provisioning(pool: PgPool) {
    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = sign(json!({
        "sub": "svc", "tid": 1, "roles": ["service"], "permissions": [],
    }));
    let response = app
        .oneshot(request("GET", "/v1/pm/projects", &token, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(body_json(response).await["code"], json!("IAM_PERM_3003"));
    assert_eq!(
        (human_principals(&pool).await, identities(&pool).await),
        (0, 0),
        "service tokens must not provision"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn c31_cross_tenant_token_is_rejected_without_provisioning(pool: PgPool) {
    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = sign(json!({
        "sub": "usr-x", "tid": 1, "roles": [], "permissions": [], "ct": true,
    }));
    let response = app
        .oneshot(request("GET", "/v1/pm/me", &token, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(body_json(response).await["code"], json!("IAM_PERM_3004"));
    assert_eq!(human_principals(&pool).await, 0, "cross-tenant tokens must not provision");
}

// ----------------------------------------------------------- C32 / C33 / C34

#[sqlx::test(migrations = "../../migrations")]
async fn c32_first_sight_provisions_an_inactive_principal(pool: PgPool) {
    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = browser_token("usr-bob", json!([]));
    let response = app
        .oneshot(request("GET", "/v1/pm/me", &token, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["is_active"], json!(false), "unknown identities start inactive");
    assert_eq!(body["is_admin"], json!(false));
    assert_eq!(body["claims_tenant_key"], json!("1"));
    assert_eq!(body["auth_mode"], json!("interactive"));

    assert_eq!(
        (human_principals(&pool).await, identities(&pool).await),
        (1, 1)
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn c33_inactive_principal_is_fail_closed_on_business_endpoints(pool: PgPool) {
    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = browser_token("usr-bob", json!([]));
    // 先触发 provision（/me 放行），再打业务端点。
    assert_eq!(
        app.clone()
            .oneshot(request("GET", "/v1/pm/me", &token, None))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let response = app
        .oneshot(request("GET", "/v1/pm/projects", &token, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(body_json(response).await["code"], json!("IAM_PERM_3002"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn c34_inactive_principal_is_fail_closed_on_mcp(pool: PgPool) {
    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = browser_token("usr-bob", json!([]));
    assert_eq!(
        app.clone()
            .oneshot(request("GET", "/v1/pm/me", &token, None))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let response = app
        .oneshot(request(
            "POST",
            "/mcp",
            &token,
            Some(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(body_json(response).await["code"], json!("IAM_PERM_3002"));
}

// ------------------------------------------------------------- C35 / C38

#[sqlx::test(migrations = "../../migrations")]
async fn c35_admin_activates_then_the_same_token_can_write_with_real_actor(pool: PgPool) {
    let tenant_id = lmms_tid1_tenant(&pool).await;
    let begin = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "ACT-1".to_string(),
            project_name: "activation".to_string(),
            tenant_id,
        },
    )
    .await
    .unwrap();
    let config = first_admin("usr-admin");
    let app = router_with_auth_config(pool.clone(), test_jwks(), config);
    let admin_token = browser_token("usr-admin", json!(["tenant_admin"]));

    // C41：配置首管理员 → 首次请求即激活。
    let me = body_json(
        app.clone()
            .oneshot(request("GET", "/v1/pm/me", &admin_token, None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(me["is_active"], json!(true));
    assert_eq!(me["is_admin"], json!(true));

    // 激活 bob。
    let bob_token = browser_token("usr-bob", json!([]));
    let bob = body_json(
        app.clone()
            .oneshot(request("GET", "/v1/pm/me", &bob_token, None))
            .await
            .unwrap(),
    )
    .await;
    let bob_id = bob["principal_id"].as_str().unwrap().to_string();
    let response = app
        .clone()
        .oneshot(request(
            "PATCH",
            &format!("/v1/pm/principals/{bob_id}"),
            &admin_token,
            Some(json!({ "is_active": true })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 同一 token 重建任务 → 201，审计 actor = bob。
    let response = app
        .oneshot(request(
            "POST",
            "/v1/pm/tasks",
            &bob_token,
            Some(json!({ "project_id": begin.project_id, "title": "audit me" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let (actor_id, mode): (String, Option<String>) = sqlx::query_as(
        "SELECT actor_id::text, actor_auth_mode FROM governance.audit_log \
          WHERE action = 'task.create' ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(actor_id, bob_id, "audit actor must be the caller");
    assert_eq!(mode.as_deref(), Some("interactive"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn c38_actor_is_the_caller_not_the_project_owner(pool: PgPool) {
    let tenant_id = lmms_tid1_tenant(&pool).await;
    let begin = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "OWN-1".to_string(),
            project_name: "owner".to_string(),
            tenant_id,
        },
    )
    .await
    .unwrap();
    // owner 是 bootstrap 造的 human；调用者用另一个（首管理员）身份。
    let app = router_with_auth_config(pool.clone(), test_jwks(), first_admin("usr-bob"));
    let token = browser_token("usr-bob", json!(["tenant_admin"]));
    let response = app
        .oneshot(request(
            "POST",
            "/v1/pm/tasks",
            &token,
            Some(json!({ "project_id": begin.project_id, "title": "by bob" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let actor_id: String = sqlx::query_scalar(
        "SELECT actor_id::text FROM governance.audit_log \
          WHERE action = 'task.create' ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(actor_id, begin.human_id.to_string(), "must not fall back to owner");
}

// ------------------------------------------------------------- C36 / C37

#[sqlx::test(migrations = "../../migrations")]
async fn c36_concurrent_first_login_creates_one_principal(pool: PgPool) {
    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = browser_token("usr-race", json!([]));
    let first = app.clone().oneshot(request("GET", "/v1/pm/me", &token, None));
    let second = app.clone().oneshot(request("GET", "/v1/pm/me", &token, None));
    let (a, b) = tokio::join!(first, second);
    assert_eq!(a.unwrap().status(), StatusCode::OK);
    assert_eq!(b.unwrap().status(), StatusCode::OK);

    assert_eq!(
        (human_principals(&pool).await, identities(&pool).await),
        (1, 1),
        "no orphan principal from the loser"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn c37_browser_and_api_key_tokens_resolve_to_one_principal(pool: PgPool) {
    let tenant_id = lmms_tid1_tenant(&pool).await;
    let begin = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "KEY-1".to_string(),
            project_name: "keys".to_string(),
            tenant_id,
        },
    )
    .await
    .unwrap();
    let app = router_with_auth_config(pool.clone(), test_jwks(), first_admin("usr-bob"));
    let browser = browser_token("usr-bob", json!(["tenant_admin"]));
    let api_key = api_key_token("usr-bob", json!(["tenant_admin"]));

    let browser_me = body_json(
        app.clone()
            .oneshot(request("GET", "/v1/pm/me", &browser, None))
            .await
            .unwrap(),
    )
    .await;
    let api_me = body_json(
        app.clone()
            .oneshot(request("GET", "/v1/pm/me", &api_key, None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        browser_me["principal_id"], api_me["principal_id"],
        "same (provider, tid, sub) must map to one principal"
    );
    assert_eq!(browser_me["auth_mode"], json!("interactive"));
    assert_eq!(api_me["auth_mode"], json!("api_key"));
    assert_eq!(identities(&pool).await, 1);

    for (token, expected) in [(&api_key, "api_key"), (&browser, "interactive")] {
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "POST",
                    "/v1/pm/tasks",
                    token,
                    Some(json!({ "project_id": begin.project_id, "title": "mode" })),
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::CREATED
        );
        let mode: Option<String> = sqlx::query_scalar(
            "SELECT actor_auth_mode FROM governance.audit_log \
              WHERE action = 'task.create' ORDER BY created_at DESC LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(mode.as_deref(), Some(expected));
    }
}

// ------------------------------------------------------------- C40 / C42

#[sqlx::test(migrations = "../../migrations")]
async fn c40_last_seen_is_throttled_to_five_minutes(pool: PgPool) {
    let app = router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default());
    let token = browser_token("usr-bob", json!([]));
    app.clone()
        .oneshot(request("GET", "/v1/pm/me", &token, None))
        .await
        .unwrap();
    sqlx::query(
        "UPDATE core.principal_identity SET last_seen_at = now() - interval '6 minutes'",
    )
    .execute(&pool)
    .await
    .unwrap();
    app.clone()
        .oneshot(request("GET", "/v1/pm/me", &token, None))
        .await
        .unwrap();
    let advanced: bool = sqlx::query_scalar(
        "SELECT last_seen_at > now() - interval '1 minute' FROM core.principal_identity",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(advanced, "a request older than the throttle window must refresh last_seen_at");

    let before: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT last_seen_at FROM core.principal_identity")
            .fetch_one(&pool)
            .await
            .unwrap();
    app.oneshot(request("GET", "/v1/pm/me", &token, None))
        .await
        .unwrap();
    let after: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT last_seen_at FROM core.principal_identity")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after, "requests inside the throttle window must not write");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c42_off_mode_injects_dev_off_principal_and_audits_it(pool: PgPool) {
    let begin = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "OFF-1".to_string(),
            project_name: "off".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let app = router(pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/pm/tasks")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "project_id": begin.project_id, "title": "off mode" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let actor_id: String = sqlx::query_scalar(
        "SELECT actor_id::text FROM governance.audit_log \
          WHERE action = 'task.create' ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(actor_id, pm_api::auth::DEV_OFF_PRINCIPAL.to_string());
}
