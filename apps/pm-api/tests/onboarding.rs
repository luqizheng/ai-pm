//! `/health` + `/install` + `/install/{tool}`：执行端接入引导（T-080）。
//!
//! 覆盖设计稿 `docs/ai-pm-t080-executor-onboarding-url-v1.md` §六.1 的
//! O-01~O-14。要点：
//!
//! - 公开路径免鉴权（agent 在拿到 key 之前就要拉指引）；
//! - `/v1/pm/me` **不**被误放行；
//! - 内容协商（Markdown / JSON）双向；
//! - **凭据不外泄**：响应里只允许 `<API_KEY>` 占位符。

use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::auth::AuthConfig;
use pm_api::routes::router_with_auth_config;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

const PRIVATE_PEM: &str = include_str!("keys/private.pem");
const PUBLIC_PEM: &str = include_str!("keys/public.pem");

fn test_jwks() -> Arc<JWKSCache> {
    Arc::new(JWKSCache::from_pem(PUBLIC_PEM).expect("test public key must parse"))
}

fn sign(subject: &str) -> String {
    sign_token(
        PRIVATE_PEM,
        &serde_json::json!({
            "sub": subject,
            "tid": 1,
            "username": "alice",
            "roles": ["tenant_admin"],
            "permissions": [],
        }),
        Duration::from_secs(300),
    )
    .expect("test token must sign")
}

/// 免鉴权路由的 app（`first_admin` 让 `usr-1` 首次即激活）。
fn app(pool: PgPool) -> axum::Router {
    router_with_auth_config(
        pool,
        test_jwks(),
        AuthConfig {
            first_admin_tenant: Some("1".to_string()),
            first_admin_subject: Some("usr-1".to_string()),
        },
    )
}

fn get(uri: &str, accept: Option<&str>, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method("GET").uri(uri);
    if let Some(accept) = accept {
        builder = builder.header("accept", accept);
    }
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::empty()).unwrap()
}

async fn body_text(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8_lossy(&bytes).into_owned()
}

async fn body_json(response: axum::response::Response) -> Value {
    serde_json::from_str(&body_text(response).await).expect("body must be json")
}

fn content_type(response: &axum::response::Response) -> String {
    response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

// ---------------------------------------------------------------- O-01 / O-05

#[sqlx::test(migrations = "../../migrations")]
async fn health_is_public_and_me_is_not(pool: PgPool) {
    let app = app(pool);

    // O-01：`/health` 免鉴权且真的存在（此前只在放行名单里、路由缺失）。
    let response = app
        .clone()
        .oneshot(get("/health", None, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["status"], serde_json::json!("ok"));

    // O-05：`/v1/pm/me` 不在公开路径里——缺 token 必须 401。
    let response = app.oneshot(get("/v1/pm/me", None, None)).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------- O-02 / O-03

#[sqlx::test(migrations = "../../migrations")]
async fn catalog_and_guide_are_public_and_carry_the_tool_list(pool: PgPool) {
    let app = app(pool);

    // O-02：`/install` 返回工具清单。
    let response = app
        .clone()
        .oneshot(get("/install", None, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let markdown = body_text(response).await;
    for slug in ["trae", "codex", "workbuddy"] {
        assert!(markdown.contains(slug), "catalog must mention {slug}");
    }

    // O-03：`/install/trae` 给出 trae 的配置片段与 key 占位符。
    let response = app.oneshot(get("/install/trae", None, None)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let markdown = body_text(response).await;
    assert!(
        markdown.contains("<API_KEY>"),
        "guide must carry the placeholder"
    );
    assert!(
        markdown.contains("/mcp"),
        "guide must carry the MCP endpoint"
    );
}

// ---------------------------------------------------------------- O-04 / O-10

#[sqlx::test(migrations = "../../migrations")]
async fn unknown_slug_falls_back_and_invalid_slug_is_rejected(pool: PgPool) {
    let app = app(pool);

    // O-04：未知 slug 落通用条目，**不 404**。
    let response = app
        .clone()
        .oneshot(get("/install/zzz-unknown", Some("application/json"), None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["tool"]["verified"], serde_json::json!(false));

    // O-10：非法 slug → 400。
    let response = app
        .oneshot(get("/install/Bad_Slug", None, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------- O-06~O-08

#[sqlx::test(migrations = "../../migrations")]
async fn content_negotiation_serves_markdown_and_json(pool: PgPool) {
    let app = app(pool);

    // O-08：默认 Markdown。
    let response = app
        .clone()
        .oneshot(get("/install/trae", None, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        content_type(&response).starts_with("text/markdown"),
        "default must be markdown, got {}",
        content_type(&response)
    );

    // O-06：`?format=json`。
    let response = app
        .clone()
        .oneshot(get("/install/trae?format=json", None, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(content_type(&response), "application/json; charset=utf-8");
    let body = body_json(response).await;
    for key in [
        "platform_base_url",
        "base_url_source",
        "mcp_endpoint",
        "transport",
        "tool",
        "config_template",
        "requirements",
        "steps",
        "verify",
        "activation",
        "next_steps",
        "lease_hint",
        "known_constraints",
    ] {
        assert!(body.get(key).is_some(), "json guide must carry `{key}`");
    }

    // O-07：`Accept: application/json`。
    let response = app
        .clone()
        .oneshot(get("/install/trae", Some("application/json"), None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(content_type(&response), "application/json; charset=utf-8");

    // O-11：非法 format → 400（不静默回落）。
    let response = app
        .clone()
        .oneshot(get("/install/trae?format=xml", None, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------- O-09

#[sqlx::test(migrations = "../../migrations")]
async fn no_response_carries_a_real_credential(pool: PgPool) {
    let app = app(pool);

    for uri in [
        "/install",
        "/install/trae",
        "/install/codex",
        "/install/unknown-tool",
        "/install/trae?format=json",
    ] {
        let response = app.clone().oneshot(get(uri, None, None)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{uri} must be 200");
        let text = body_text(response).await;

        // 只允许占位符形态的 Authorization：机器形态 `<API_KEY>`，
        // 人读形态「在此填入你的 API Key」（Markdown 渲染时替换）。
        for line in text.lines().filter(|l| l.contains("Bearer ")) {
            let value = line
                .split_once("Bearer ")
                .map(|(_, rest)| rest.split('"').next().unwrap_or_default().trim())
                .unwrap_or_default();
            assert!(
                value == "<API_KEY>" || value == "在此填入你的 API Key",
                "{uri} leaked a real credential: {line}"
            );
        }
        // T-089 收紧启发式：`sk-` 前有词字符（如 "task-execution"）是普通英文
        // 单词，不是 key 形态；只拦「引号/空白/行首后的 sk-<alnum>」。
        let leak = text
            .split(|c: char| c == '"' || c.is_whitespace())
            .any(|token| {
                token.starts_with("sk-")
                    && token[3..]
                        .chars()
                        .next()
                        .map_or(false, |c| c.is_ascii_alphanumeric())
            });
        assert!(!leak, "{uri} leaked an api-key-shaped string");
    }
}

// ---------------------------------------------------------------- O-12

#[sqlx::test(migrations = "../../migrations")]
async fn public_base_url_env_drives_the_endpoint_and_host_header(pool: PgPool) {
    // O-12：平台地址来自 `AI_PM_PUBLIC_BASE_URL`。
    let app = app(pool);

    let response = app
        .clone()
        .oneshot(get("/install/trae?format=json", None, None))
        .await
        .unwrap();
    let body = body_json(response).await;
    let base = body["platform_base_url"].as_str().unwrap().to_string();
    let source = body["base_url_source"].as_str().unwrap().to_string();
    assert_eq!(
        body["mcp_endpoint"].as_str().unwrap(),
        format!("{base}/mcp")
    );
    if source == "default" {
        assert_eq!(base, "http://127.0.0.1:8090");
    }

    // 模板里的 Host 头必须来自部署地址的 host，不是写死的回环地址。
    let template = body["config_template"].to_string();
    assert!(
        template.contains("/mcp"),
        "template must carry the endpoint"
    );
}

// ---------------------------------------------------------------- O-13

#[sqlx::test(migrations = "../../migrations")]
async fn inactive_principal_can_read_its_own_state(pool: PgPool) {
    // O-13：`/v1/pm/me` 对未激活主体仍返回 200 且 `is_active=false`
    // —— 引导页靠这个字段给「请管理员激活」的下一步。
    let app = app(pool);
    // 换一个非首管理员的 subject —— 首管理员首次见到即激活，
    // 而引导页要覆盖的是「主体尚未被激活」这条分支。
    let token = sign("usr-inactive");
    let response = app
        .oneshot(get("/v1/pm/me", None, Some(&token)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["is_active"], serde_json::json!(false));
}

// ---------------------------------------------------------------- O-14

#[sqlx::test(migrations = "../../migrations")]
async fn guide_steps_are_unique_and_verify_entries_are_well_formed(pool: PgPool) {
    // O-14：步骤 id 唯一且非空；verify 项字段齐全。
    let app = app(pool);
    let response = app
        .oneshot(get("/install/codex?format=json", None, None))
        .await
        .unwrap();
    let body = body_json(response).await;

    let mut ids = Vec::new();
    for step in body["steps"].as_array().unwrap() {
        let id = step["id"].as_str().unwrap();
        assert!(!id.is_empty(), "step id must not be empty");
        assert!(!ids.contains(&id.to_string()), "duplicate step id: {id}");
        ids.push(id.to_string());
    }

    let verify = body["verify"].as_array().unwrap();
    assert!(verify.len() >= 2, "verify must cover health and identity");
    for item in verify {
        assert!(item["name"].as_str().is_some_and(|s| !s.is_empty()));
        assert_eq!(item["method"], serde_json::json!("GET"));
        assert!(item["url"].as_str().is_some_and(|s| s.starts_with("http")));
        assert!(item["expect_status"].as_u64().is_some());
    }
}
