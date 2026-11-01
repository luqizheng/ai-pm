//! Audit SSE contract: auth surface, `text/event-stream` framing and the
//! `pm_audit` trigger payload. Split into two stable halves — the transport is
//! driven by `AuditFeed::publish` (no PG timing), while the trigger is proven
//! with a direct `LISTEN` in the test process.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use digua_common_jwt::{JWKSCache, sign_token};
use pm_api::audit_stream::{AuditFeed, AuditLog, parse_notification, run_listener};
use pm_api::routes::router_with_auth_and_feed;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use serde_json::json;
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tower::ServiceExt;
use pm_domain::SYSTEM_TENANT;

const PRIVATE_PEM: &str = include_str!("keys/private.pem");
const PUBLIC_PEM: &str = include_str!("keys/public.pem");

fn test_jwks() -> Arc<JWKSCache> {
    Arc::new(JWKSCache::from_pem(PUBLIC_PEM).expect("test public key must parse"))
}

fn sign(claims: serde_json::Value, ttl: Duration) -> String {
    sign_token(PRIVATE_PEM, &claims, ttl).expect("test token must sign")
}

fn valid_token() -> String {
    sign(
        json!({ "sub": "usr-1", "tid": 1, "username": "alice", "roles": ["tenant_admin"], "permissions": [] }),
        Duration::from_secs(300),
    )
}

async fn wait_for(stream: &mut TcpStream, needle: &str, timeout: Duration) -> String {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    tokio::time::timeout(timeout, async {
        loop {
            let read = stream.read(&mut chunk).await.expect("socket read");
            if read == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..read]);
            if String::from_utf8_lossy(&buf).contains(needle) {
                break;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {needle:?}"));
    String::from_utf8_lossy(&buf).into_owned()
}

/// Boots the real router on an ephemeral port with its PG listener running.
async fn spawn_server(pool: PgPool) -> (String, AuditFeed) {
    let feed = AuditFeed::new();
    tokio::spawn(run_listener(pool.clone(), feed.clone()));
    let app = router_with_auth_and_feed(pool, test_jwks(), feed.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let listener_ready = format!("{addr}");
    // Give the spawned LISTEN loop a beat so tests never race its handshake.
    tokio::time::sleep(Duration::from_millis(300)).await;
    (listener_ready, feed)
}

async fn open_stream(addr: &str, query: &str) -> TcpStream {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let request = format!(
        "GET /v1/pm/events/stream{query} HTTP/1.1\r\nHost: {addr}\r\nAccept: text/event-stream\r\nConnection: keep-alive\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    stream
}

#[sqlx::test(migrations = "../../migrations")]
async fn stream_without_token_is_unauthorized(pool: PgPool) {
    let app = router_with_auth_and_feed(pool, test_jwks(), AuditFeed::new());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/pm/events/stream")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn forged_query_token_is_unauthorized(pool: PgPool) {
    let app = router_with_auth_and_feed(pool, test_jwks(), AuditFeed::new());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/pm/events/stream?access_token=not-a-jwt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn query_token_streams_published_audit_events(pool: PgPool) {
    // T-021a：SSE 属业务端点，token 的 (provider, tid, sub) 必须映射到已激活
    // principal，否则首次见到会 provision 为未激活并 403。
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "SSE-1".to_string(),
            project_name: "sse".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', '1', 'lmms:1') ON CONFLICT (provider, external_key) DO NOTHING",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal_identity \
             (provider, tenant_id, subject, principal_id) \
         VALUES ('lmms', \
                 (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'), \
                 'usr-1', $1)",
    )
    .bind(boot.human_id)
    .execute(&pool)
    .await
    .unwrap();
    let (addr, feed) = spawn_server(pool).await;
    let mut stream = open_stream(&addr, &format!("?access_token={}", valid_token())).await;

    let head = wait_for(&mut stream, "\r\n\r\n", Duration::from_secs(5)).await;
    assert!(head.starts_with("HTTP/1.1 200"), "expected 200, got: {head}");
    assert!(
        head.to_ascii_lowercase().contains("content-type: text/event-stream"),
        "expected SSE content type, got: {head}"
    );

    feed.publish(AuditLog {
        id: "11111111-1111-1111-1111-111111111111".into(),
        project_id: "22222222-2222-2222-2222-222222222222".into(),
        actor_id: "33333333-3333-3333-3333-333333333333".into(),
        action: "task.create".into(),
        target_type: "task".into(),
        target_id: Some("44444444-4444-4444-4444-444444444444".into()),
        created_at: "2026-10-31T03:00:00.000000Z".into(),
    });

    let frame = wait_for(&mut stream, "event: audit", Duration::from_secs(5)).await;
    assert!(frame.contains("data: "), "missing data line: {frame}");
    let data_line = frame
        .lines()
        .find(|line| line.starts_with("data: "))
        .expect("data line");
    let payload: serde_json::Value =
        serde_json::from_str(data_line.trim_start_matches("data: ")).expect("json payload");
    assert_eq!(payload["action"], json!("task.create"));
    assert_eq!(payload["target_type"], json!("task"));
    assert_eq!(payload["target_id"], json!("44444444-4444-4444-4444-444444444444"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn audit_insert_fires_pm_audit_notification(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "SSE-1".to_string(),
            project_name: "sse project".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();

    let mut listener = PgListener::connect_with(&pool).await.unwrap();
    listener.listen("pm_audit").await.unwrap();

    let target = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO governance.audit_log (project_id, actor_id, action, target_type, target_id, tenant_id) \
         VALUES ($1, $2, 'task.create', 'task', $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(boot.project_id)
    .bind(boot.human_id)
    .bind(target)
    .execute(&pool)
    .await
    .unwrap();

    let notification = tokio::time::timeout(Duration::from_secs(5), listener.recv())
        .await
        .expect("timed out waiting for pm_audit")
        .unwrap();
    assert_eq!(notification.channel(), "pm_audit");

    let log = parse_notification(notification.payload()).expect("payload must parse");
    assert_eq!(log.action, "task.create");
    assert_eq!(log.target_type, "task");
    assert_eq!(log.target_id.as_deref(), Some(target.to_string().as_str()));
    assert_eq!(log.project_id, boot.project_id.to_string());
    assert_eq!(log.actor_id, boot.human_id.to_string());
    assert!(log.created_at.ends_with('Z'), "created_at must be UTC ISO8601");
}