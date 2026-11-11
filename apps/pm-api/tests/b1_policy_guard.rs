//! B1 安全边界集成测试（DEF-P0-01 默认拒绝 + DEF-P0-02 真人审批守卫）。
//!
//! 覆盖两条红线：
//!   1. **默认拒绝**——未显式授权时能力类字段不得放行（策略合成层，单测见
//!      `pm-application::policy_composition` 模块内测试）；
//!   2. **真人守卫**——正式基线确认必须由真人主体在可信交互上下文中执行，
//!      API Key（Agent / Service）上下文一律 403。

use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::auth::AuthConfig;
use pm_api::routes::router_with_auth_config;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const PUBLIC_PEM: &str = include_str!("keys/public.pem");
const PRIVATE_PEM: &str = include_str!("keys/private.pem");

fn sign(claims: serde_json::Value) -> String {
    sign_token(PRIVATE_PEM, &claims, Duration::from_secs(300)).expect("test token must sign")
}

fn call(token: &str, method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn post_json(token: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn status_of(app: axum::Router, req: Request<Body>) -> StatusCode {
    app.oneshot(req).await.unwrap().status()
}

async fn json_of(response: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

/// 种子：租户 + 项目 + 一条已确认知识（基线提案的派生源）。
async fn seed(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
    let tenant_id: Uuid = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', '1', 'lmms:1') \
         ON CONFLICT (provider, external_key) DO UPDATE SET display_name = core.tenant.display_name \
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("Project {code}"),
            tenant_id,
        },
    )
    .await
    .unwrap();
    let project_id = outcome.project_id;
    sqlx::query(
        "INSERT INTO ai.project_knowledge \
         (tenant_id, project_id, domain, content, confirmed_by) \
         SELECT tenant_id, $1, 'identity', 'Confirmed identity from cognition', \
                (SELECT principal_id FROM core.project_member WHERE project_id = $1 LIMIT 1) \
           FROM core.project WHERE id = $1",
    )
    .bind(project_id)
    .execute(pool)
    .await
    .unwrap();
    (tenant_id, project_id)
}

fn authed_app(pool: PgPool) -> axum::Router {
    router_with_auth_config(
        pool.clone(),
        Arc::new(JWKSCache::from_pem(PUBLIC_PEM).unwrap()),
        AuthConfig {
            first_admin_tenant: Some("1".to_string()),
            first_admin_subject: Some("usr-lead".to_string()),
        },
    )
}

/// 交互式真人令牌（`aud=coder-micro-services` → auth_mode=interactive）。
fn browser_token() -> String {
    sign(serde_json::json!({
        "sub": "usr-lead", "tid": 1, "username": "usr-lead",
        "roles": ["tenant_admin"], "permissions": [], "aud": "coder-micro-services",
    }))
}

/// API Key 令牌（Agent / Service Key 上下文 → auth_mode=api_key）。
fn api_key_token() -> String {
    sign(serde_json::json!({
        "sub": "usr-lead", "tid": 1, "username": "usr-lead",
        "roles": ["tenant_admin"], "permissions": [], "client_id": "key-abc",
    }))
}

/// 真人激活 + 入项目成员 → 返回真人 principal id。
async fn join_human(app: &axum::Router, pool: &PgPool, project_id: Uuid) -> Uuid {
    let browser = browser_token();
    let me = app
        .clone()
        .oneshot(call(&browser, "GET", "/v1/pm/me"))
        .await
        .unwrap();
    let me = json_of(me).await;
    let principal_id: Uuid = me["principal_id"].as_str().unwrap().parse().unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         SELECT tenant_id, $1, $2, 'owner' FROM core.project WHERE id = $1 \
         ON CONFLICT DO NOTHING",
    )
    .bind(project_id)
    .bind(principal_id)
    .execute(pool)
    .await
    .unwrap();
    principal_id
}

/// 走完「派生提案 → 逐项确认」，进入可确认状态。
async fn prepare_proposal(app: &axum::Router, project_id: Uuid) {
    let browser = browser_token();
    let derived = app
        .clone()
        .oneshot(post_json(
            &browser,
            &format!("/v1/pm/projects/{project_id}/baseline/proposals"),
            serde_json::json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(derived.status(), StatusCode::OK, "proposal derivation");
    // 条目 id 取自基线总览（提案响应只给 item_count）。
    let overview = app
        .clone()
        .oneshot(call(
            &browser,
            "GET",
            &format!("/v1/pm/projects/{project_id}/baseline"),
        ))
        .await
        .unwrap();
    assert_eq!(overview.status(), StatusCode::OK, "baseline overview");
    let overview = json_of(overview).await;
    let item_id = overview["items"][0]["id"].as_str().unwrap().to_string();
    let decided = app
        .clone()
        .oneshot(post_json(
            &browser,
            &format!("/v1/pm/projects/{project_id}/baseline/items/{item_id}/decision"),
            serde_json::json!({ "action": "confirm" }),
        ))
        .await
        .unwrap();
    assert_eq!(decided.status(), StatusCode::OK, "item decision");
}

/// DEF-P0-02：API Key 上下文确认正式基线 → 403（不可绕过）。
#[sqlx::test(migrations = "../../migrations")]
async fn baseline_confirm_rejects_api_key_context(pool: PgPool) {
    let (_tenant, project_id) = seed(&pool, "B1-KEY").await;
    let app = authed_app(pool.clone());
    join_human(&app, &pool, project_id).await;
    prepare_proposal(&app, project_id).await;

    let status = status_of(
        app,
        call(
            &api_key_token(),
            "POST",
            &format!("/v1/pm/projects/{project_id}/baseline/confirm"),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "api_key context must not confirm a formal baseline"
    );
}

/// DEF-P0-02：交互式真人确认正式基线 → 200（门不误伤真人路径），
/// 且确认人必须是真人主体。
#[sqlx::test(migrations = "../../migrations")]
async fn baseline_confirm_requires_human_principal(pool: PgPool) {
    let (_tenant, project_id) = seed(&pool, "B1-HUMAN").await;
    let app = authed_app(pool.clone());
    let human = join_human(&app, &pool, project_id).await;
    prepare_proposal(&app, project_id).await;

    let browser = browser_token();
    let response = app
        .clone()
        .oneshot(post_json(
            &browser,
            &format!("/v1/pm/projects/{project_id}/baseline/confirm"),
            serde_json::json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "human confirm must pass");

    let confirmer: Option<Uuid> = sqlx::query_scalar::<_, Uuid>(
        "SELECT confirmed_by FROM ai.project_baseline WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(
        confirmer,
        Some(human),
        "the confirmer must be the human principal"
    );
}

/// DEF-P0-02：AI 主体（`core.principal.kind='ai_pm'`）不得成为确认人。
///
/// 机制：真人交互会话可确认（基线），但确认人必须是 `kind='human'`；
/// AI 主体即便持有项目成员身份，也不能经任何路径成为确认人。
#[sqlx::test(migrations = "../../migrations")]
async fn ai_principal_is_never_the_baseline_confirmer(pool: PgPool) {
    let (_tenant, project_id) = seed(&pool, "B1-AI").await;
    let app = authed_app(pool.clone());
    join_human(&app, &pool, project_id).await;

    // 造一个 AI 主体并给同样的项目成员身份（证明「有成员身份」不等于「可确认」）。
    let ai_principal: Uuid = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO core.principal (kind, display_name) \
         VALUES ('ai_pm', 'pm-agent') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         SELECT tenant_id, $1, $2, 'owner' FROM core.project WHERE id = $1",
    )
    .bind(project_id)
    .bind(ai_principal)
    .execute(&pool)
    .await
    .unwrap();

    prepare_proposal(&app, project_id).await;

    let browser = browser_token();
    let response = app
        .clone()
        .oneshot(post_json(
            &browser,
            &format!("/v1/pm/projects/{project_id}/baseline/confirm"),
            serde_json::json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let confirmer: Option<Uuid> = sqlx::query_scalar::<_, Uuid>(
        "SELECT confirmed_by FROM ai.project_baseline WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_ne!(
        confirmer,
        Some(ai_principal),
        "AI principal must never be the baseline confirmer"
    );
}
