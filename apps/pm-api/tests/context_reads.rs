//! B4 集成测试：权威读接口（CX3）——interactions 读源 + 候选对象聚合。

use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::auth::AuthConfig;
use pm_api::routes::router_with_auth_config;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const PUBLIC_PEM: &str = include_str!("keys/public.pem");
const PRIVATE_PEM: &str = include_str!("keys/private.pem");

fn sign(claims: Value) -> String {
    sign_token(PRIVATE_PEM, &claims, Duration::from_secs(300)).expect("test token must sign")
}

fn browser_token() -> String {
    sign(json!({
        "sub": "usr-lead", "tid": 1, "username": "usr-lead",
        "roles": ["tenant_admin"], "permissions": [], "aud": "coder-micro-services",
    }))
}

fn req(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {}", browser_token()));
    if let Some(value) = body {
        builder = builder.header("content-type", "application/json");
        builder.body(Body::from(value.to_string())).unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    }
}

async fn send(app: &axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn app(pool: PgPool) -> axum::Router {
    router_with_auth_config(
        pool.clone(),
        Arc::new(JWKSCache::from_pem(PUBLIC_PEM).unwrap()),
        AuthConfig {
            first_admin_tenant: Some("1".to_string()),
            first_admin_subject: Some("usr-lead".to_string()),
        },
    )
}

/// 种子：租户 + 项目 + 真人激活 + 成员 + 会话 + 一条需求。返回 (app, pool, project, conversation)。
async fn seed(pool: &PgPool, code: &str) -> (axum::Router, PgPool, Uuid, Uuid) {
    let tenant_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', '1', 'lmms:1') \
         ON CONFLICT (provider, external_key) DO UPDATE SET display_name = core.tenant.display_name \
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let outcome = pm_application::bootstrap::bootstrap_project(
        pool,
        pm_application::bootstrap::BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("Project {code}"),
            tenant_id,
        },
    )
    .await
    .unwrap();
    let project_id = outcome.project_id;
    let a = app(pool.clone());

    let me = send(&a, req("GET", "/v1/pm/me", None)).await;
    assert_eq!(me.0, StatusCode::OK, "activation");
    let human: Uuid = me.1["principal_id"].as_str().unwrap().parse().unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         SELECT tenant_id, $1, $2, 'owner' FROM core.project WHERE id = $1 \
         ON CONFLICT DO NOTHING",
    )
    .bind(project_id)
    .bind(human)
    .execute(pool)
    .await
    .unwrap();

    let conversation: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.conversation (tenant_id, project_id, principal_id, title) \
         VALUES ($1, $2, $3, 'b4 fixture') RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(human)
    .fetch_one(pool)
    .await
    .unwrap();

    (a, pool.clone(), project_id, conversation)
}

#[sqlx::test(migrations = "../../migrations")]
async fn interactions_read_returns_authoritative_records(pool: PgPool) {
    let (a, pool, project, conversation) = seed(&pool, "B4-I1").await;

    // 种消息 + 交互记录（写回链产物；这里直插聚焦读面）。
    let message: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.conversation_message (conversation_id, role, content) \
         SELECT id, 'assistant', 'AI 提议：为需求补充验收口径。' \
           FROM ai.conversation WHERE id = $1 RETURNING id",
    )
    .bind(conversation)
    .fetch_one(&pool)
    .await
    .unwrap();
    let human: Uuid = sqlx::query_scalar("SELECT principal_id FROM ai.conversation WHERE id = $1")
        .bind(conversation)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO ai.conversation_interaction \
         (tenant_id, project_id, conversation_id, message_id, action, status, reason, \
          answer, decided_by, reference) \
         VALUES ($1, $2, $3, $4, 'accept', 'accepted', NULL, '{}', $5, 'INT-B40001')",
    )
    .bind(
        sqlx::query_scalar::<_, Uuid>("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap(),
    )
    .bind(project)
    .bind(conversation)
    .bind(message)
    .bind(human)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = send(
        &a,
        req(
            "GET",
            &format!("/v1/pm/projects/{project}/conversations/{conversation}/interactions"),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let records = body["interactions"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    // 权威字段完整：操作者 / 原始 action / 处理状态 / 记录号 / 关联消息摘要。
    assert_eq!(record["action"], json!("accept"));
    assert_eq!(record["status"], json!("accepted"));
    assert_eq!(record["reference"], json!("INT-B40001"));
    assert_eq!(record["decided_by"], json!(human));
    assert!(record["decided_by_name"].is_string());
    assert!(
        record["message_excerpt"]
            .as_str()
            .unwrap_or_default()
            .contains("验收口径"),
        "message excerpt must be present: {record}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn interactions_read_is_owner_isolated(pool: PgPool) {
    let (a, _pool, project, conversation) = seed(&pool, "B4-I2").await;
    let owner_token = browser_token();

    // 另一真人：激活 + 加入项目（成员但非会话属主）。
    let peer_token = sign(json!({
        "sub": "usr-peer", "tid": 1, "username": "usr-peer",
        "roles": ["tenant_admin"], "permissions": [], "aud": "coder-micro-services",
    }));
    let peer_req = |method: &str, uri: &str| {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {peer_token}"))
            .body(Body::empty())
            .unwrap()
    };
    let me = send(&a, peer_req("GET", "/v1/pm/me")).await;
    assert_eq!(me.0, StatusCode::OK);
    let peer: Uuid = me.1["principal_id"].as_str().unwrap().parse().unwrap();
    let activated = send(
        &a,
        req(
            "PATCH",
            &format!("/v1/pm/principals/{peer}"),
            Some(json!({ "is_active": true })),
        ),
    )
    .await;
    assert_eq!(activated.0, StatusCode::OK);
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         SELECT tenant_id, $1, $2, 'member' FROM core.project WHERE id = $1 \
         ON CONFLICT DO NOTHING",
    )
    .bind(project)
    .bind(peer)
    .execute(&_pool)
    .await
    .unwrap();

    // peer 读属主的会话交互 → 404 同形（不泄露存在性）。
    let uri = format!("/v1/pm/projects/{project}/conversations/{conversation}/interactions");
    let (status, _) = send(&a, peer_req("GET", &uri)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "peer must 404");

    // 属主正常（404 同形不误伤）。
    let owner_req = |method: &str, uri: &str| {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {owner_token}"))
            .body(Body::empty())
            .unwrap()
    };
    let (status, _) = send(&a, owner_req("GET", &uri)).await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "../../migrations")]
async fn candidates_aggregate_with_filter_pagination_and_attached_flag(pool: PgPool) {
    let (a, pool, project, conversation) = seed(&pool, "B4-C1").await;

    // 三条需求（其中一条被附加、一条标题带关键词）。
    for (code, title) in [
        ("REQ-B4A", "alpha requirement"),
        ("REQ-B4B", "beta requirement"),
        ("REQ-B4C", "gamma special"),
    ] {
        sqlx::query(
            "INSERT INTO delivery.requirement (project_id, code, title, tenant_id) \
             VALUES ($1, $2, $3, (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(project)
        .bind(code)
        .bind(title)
        .execute(&pool)
        .await
        .unwrap();
    }
    let attached_req: Uuid =
        sqlx::query_scalar("SELECT id FROM delivery.requirement WHERE code = 'REQ-B4A'")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO ai.conversation_context_item \
         (tenant_id, project_id, conversation_id, object_type, object_id, source, added_by) \
         VALUES ($1, $2, $3, 'requirement', $4, 'page', \
                 (SELECT principal_id FROM ai.conversation WHERE id = $3))",
    )
    .bind(
        sqlx::query_scalar::<_, Uuid>("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap(),
    )
    .bind(project)
    .bind(conversation)
    .bind(attached_req)
    .execute(&pool)
    .await
    .unwrap();

    let uri = format!(
        "/v1/pm/projects/{project}/context-candidates?type=requirement&conversation_id={conversation}"
    );
    let (status, body) = send(&a, req("GET", &uri, None)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], json!(3));
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    // 已附加标识只落在 REQ-B4A 上。
    let attached_flags: Vec<&Value> = items
        .iter()
        .filter(|item| item["attached"] == json!(true))
        .collect();
    assert_eq!(attached_flags.len(), 1);
    assert_eq!(attached_flags[0]["code"], json!("REQ-B4A"));

    // 关键词过滤。
    let (status, body) = send(
        &a,
        req(
            "GET",
            &format!("/v1/pm/projects/{project}/context-candidates?type=requirement&q=special"),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], json!(1));
    assert_eq!(body["items"][0]["code"], json!("REQ-B4C"));

    // 分页。
    let (status, body) = send(
        &a,
        req(
            "GET",
            &format!("/v1/pm/projects/{project}/context-candidates?type=requirement&page=2&size=2"),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], json!(3));
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    // 未知类型 → 400。
    let (status, _) = send(
        &a,
        req(
            "GET",
            &format!("/v1/pm/projects/{project}/context-candidates?type=unknown"),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn candidates_cover_multiple_domains(pool: PgPool) {
    let (a, pool, project, _conversation) = seed(&pool, "B4-C2").await;

    sqlx::query(
        "INSERT INTO delivery.task (project_id, title, goal, tenant_id) \
         VALUES ($1, 'fixture task', 'B4 goal', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai.project_knowledge \
         (tenant_id, project_id, domain, content, confirmed_by) \
         SELECT tenant_id, $1, 'identity', 'B4 knowledge fact.', $2 \
           FROM core.project WHERE id = $1",
    )
    .bind(project)
    .bind(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT pm.principal_id FROM core.project_member pm \
             WHERE pm.project_id = $1 LIMIT 1",
        )
        .bind(project)
        .fetch_one(&pool)
        .await
        .unwrap(),
    )
    .execute(&pool)
    .await
    .unwrap();

    for (dtype, expect_code) in [("task", "fixture task"), ("knowledge", "identity")] {
        let (status, body) = send(
            &a,
            req(
                "GET",
                &format!("/v1/pm/projects/{project}/context-candidates?type={dtype}"),
                None,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{dtype}: {body}");
        assert_eq!(body["total"], json!(1), "{dtype}");
        assert_eq!(body["items"][0]["code"], json!(expect_code), "{dtype}");
    }
}
