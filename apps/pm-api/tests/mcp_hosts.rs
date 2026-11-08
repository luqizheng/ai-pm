//! T-081：`/mcp` 的 Host 白名单在 API 层的接线——部署配置 → router → rmcp。
//!
//! 传输层的语义在 `crates/pm-mcp/tests/hosts.rs` 钉住；这里钉的是 pm-api 的
//! 装配路径：`McpHosts` 必须真的落到 rmcp 的 allow-list 上。

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pm_api::audit_stream::AuditFeed;
use pm_api::mcp_hosts::McpHosts;
use pm_api::routes::router_with_feed_and_hosts;
use sqlx::PgPool;
use tower::ServiceExt;

const TOOLS_LIST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;

fn mcp_request(host: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("host", host)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(Body::from(TOOLS_LIST))
        .expect("request must build")
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_machine_host_is_rejected_without_the_allow_list(pool: PgPool) {
    let app = router_with_feed_and_hosts(pool, AuditFeed::new(), McpHosts::default());
    let response = app.oneshot(mcp_request("192.168.2.10:8090")).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "未声明的跨机 Host 必须被拒"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_declared_cross_machine_host_is_served(pool: PgPool) {
    let app = router_with_feed_and_hosts(
        pool,
        AuditFeed::new(),
        McpHosts {
            extra_allowed: Some(vec!["192.168.2.10".to_string()]),
        },
    );
    let response = app.oneshot(mcp_request("192.168.2.10:8090")).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "声明过的跨机 Host 必须可达"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn loopback_keeps_working_even_with_the_allow_list_set(pool: PgPool) {
    let app = router_with_feed_and_hosts(
        pool,
        AuditFeed::new(),
        McpHosts {
            extra_allowed: Some(vec!["192.168.2.10".to_string()]),
        },
    );
    let response = app.oneshot(mcp_request("127.0.0.1:8090")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "回环 Host 不受影响");
}
