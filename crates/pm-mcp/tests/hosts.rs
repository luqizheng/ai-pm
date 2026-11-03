//! T-081：rmcp 的 DNS-rebinding 防护（Host 白名单）。
//!
//! 默认只接受回环 Host（`localhost` / `127.0.0.1` / `::1`），跨机 Host 一律
//! 403；部署方声明的额外 Host 才会被放行——这正是「服务器栈 + Windows 开发机」
//! 跨机接入需要的那一步。
//!
//! 名单永不为空：rmcp 里空名单 = 放行全部，本仓不给这条路。

use pm_mcp::PmMcpServer;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

const TOOLS_LIST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;

/// 宿主校验发生在工具执行之前，因此用惰性连接即可——本用例不触碰数据库。
fn lazy_pool() -> PgPool {
    PgPoolOptions::new()
        .connect_lazy("postgres://aipm:aipm@127.0.0.1:5433/aipm")
        .expect("lazy pool must build")
}

/// 以指定的额外 Host 名单装配 `/mcp`，用给定的 `Host` 头发一次 `tools/list`。
///
/// 走真实监听端口（与 `tools.rs` 的 transport smoke 同形）：`Host` 头由客户端
/// 显式给出，正是 rmcp `parse_host_header` 优先读取的那一个。
async fn tools_list_status(extra: Option<Vec<String>>, host: &str) -> u16 {
    let service = PmMcpServer::streamable_http_service(lazy_pool(), extra);
    let app = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    // .no_proxy(): host proxy env residue (cargo config's 127.0.0.1:10808)
    // must never route loopback MCP traffic.
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let response = client
        .post(format!("http://{addr}/mcp"))
        .header("Host", host)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(TOOLS_LIST)
        .send()
        .await
        .expect("request must complete");
    response.status().as_u16()
}

#[tokio::test]
async fn default_allow_list_rejects_a_cross_machine_host() {
    assert_eq!(
        tools_list_status(None, "192.168.2.10:8090").await,
        403,
        "跨机 Host 在默认白名单下必须被拒"
    );
}

#[tokio::test]
async fn default_allow_list_still_accepts_loopback() {
    assert_eq!(
        tools_list_status(None, "127.0.0.1:8090").await,
        200,
        "回环 Host 必须照常可用"
    );
}

#[tokio::test]
async fn declared_extra_host_is_accepted_across_machines() {
    assert_eq!(
        tools_list_status(Some(vec!["192.168.2.10".to_string()]), "192.168.2.10:8090").await,
        200,
        "声明过的 Host 必须放行"
    );
}

#[tokio::test]
async fn an_undeclared_host_is_still_rejected() {
    assert_eq!(
        tools_list_status(Some(vec!["192.168.2.10".to_string()]), "evil.example:8090").await,
        403,
        "未声明的 Host 必须继续被拒"
    );
}

#[tokio::test]
async fn a_host_port_entry_requires_the_exact_port() {
    let extra = Some(vec!["ai-pm.internal:8443".to_string()]);
    assert_eq!(
        tools_list_status(extra.clone(), "ai-pm.internal:8443").await,
        200,
        "host:port 条目必须匹配精确端口"
    );
    assert_eq!(
        tools_list_status(extra, "ai-pm.internal:9090").await,
        403,
        "端口不匹配必须被拒"
    );
}

#[test]
fn the_allow_list_never_becomes_empty() {
    // 空名单在 rmcp 里等于放行全部——本仓绝不能出现这种状态。
    let hosts = pm_mcp::server::default_allowed_hosts(Some(&[]));
    assert!(!hosts.is_empty());
    assert!(hosts.contains(&"127.0.0.1".to_string()));
}

#[test]
fn the_allow_list_keeps_loopback_and_drops_blanks_and_duplicates() {
    let extras = vec![
        " 192.168.2.10 ".to_string(),
        String::new(),
        "192.168.2.10".to_string(),
    ];
    let hosts = pm_mcp::server::default_allowed_hosts(Some(&extras));
    assert_eq!(
        hosts,
        vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "::1".to_string(),
            "192.168.2.10".to_string(),
        ]
    );
}
