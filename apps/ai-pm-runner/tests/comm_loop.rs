//! Runner 通信面测试：MCP 客户端解析（structuredContent / isError）、标记文件
//! 生命周期（侦测→解析→归档/隔离）、进程终止与回复注入。不依赖真实 Platform。

use ai_pm_runner::adapter::{
    archive_marker, find_unreported_marker, quarantine_marker, spawn_agent, terminate,
};
use ai_pm_runner::mcp::McpClient;
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mcp_client(base_url: String) -> McpClient {
    McpClient::new(format!("{base_url}/mcp"), "test-token")
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("aipm-runner-test-{name}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn mcp_client_parses_structured_content_and_tool_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_list_tasks" }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0", "id": 1,
            "result": { "structuredContent": { "tasks": [
                { "task_id": "00000000-0000-0000-0000-000000000001",
                  "queued_run_id": "00000000-0000-0000-0000-000000000002" },
                { "task_id": "00000000-0000-0000-0000-000000000003", "queued_run_id": null }
            ] } } }))
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_claim_task" }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0", "id": 2,
            "result": { "isError": true,
                        "content": [{ "text": "task is not ready (current status: draft)" }] } }))
        )
        .mount(&server)
        .await;

    let client = mcp_client(server.uri());
    let tasks = client.list_ready_tasks(Uuid::nil()).await.unwrap();
    assert_eq!(tasks.len(), 1, "tasks without a queued run must be filtered out");
    assert_eq!(tasks[0].1.to_string(), "00000000-0000-0000-0000-000000000002");

    let err = client
        .claim_task(Uuid::nil(), Uuid::new_v4(), "runner:test")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not ready"), "tool error text surfaces: {err}");
}

#[tokio::test]
async fn marker_file_lifecycle_detect_parse_archive_quarantine() {
    let dir = temp_dir("marker-life");
    let run_dir = dir.join("run-1");
    let aipm = run_dir.join(".aipm");
    std::fs::create_dir_all(&aipm).unwrap();

    // 未上报前：侦测不到。
    assert!(find_unreported_marker(&run_dir).is_none());

    // 合法标记：解析 + 归档后不再侦测到（ACP-443：event_id 键控归档）。
    let marker = aipm.join("interaction.json");
    std::fs::write(
        &marker,
        json!({
            "schema_version": "aipm.interaction.v1",
            "event_id": "evt-1",
            "reason": "needs_decision",
            "title": "确认迁移",
            "question": "是否允许删除旧字段？",
            "options": [{ "value": "approve", "label": "允许" }],
            "required_response": true,
            "checkpoint": { "path": ".aipm/checkpoint.json", "available": true }
        })
        .to_string(),
    )
    .unwrap();
    let (path, event) = find_unreported_marker(&run_dir).expect("marker detected");
    let event = event.expect("valid marker parses");
    assert_eq!(event.event_id, "evt-1");
    assert_eq!(event.reason, "needs_decision");
    assert!(event.required_response);
    assert_eq!(event.checkpoint_path.as_deref(), Some(".aipm/checkpoint.json"));
    archive_marker(&path, &event.event_id).unwrap();
    assert!(find_unreported_marker(&run_dir).is_none());
    assert!(aipm.join("interaction.evt-1.reported").is_file());

    // 非法 schema → 侦测返回 Err → 隔离（不产生 Interaction，§1.2）。
    std::fs::write(&marker, json!({ "schema_version": "unknown.v9" }).to_string()).unwrap();
    let (path, parsed) = find_unreported_marker(&run_dir).expect("malformed marker must be surfaced");
    assert!(parsed.is_err(), "malformed schema must not parse");
    quarantine_marker(&path).unwrap();
    assert!(aipm.join("interaction.json.invalid").is_file());
    assert!(find_unreported_marker(&run_dir).is_none());

    // 子目录层（workspace_path 段）同样可侦测。
    let nested = run_dir.join("repo-checkout").join(".aipm");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(
        nested.join("interaction.json"),
        json!({
            "schema_version": "aipm.interaction.v1",
            "event_id": "evt-2", "reason": "needs_info",
            "question": "依赖版本？"
        })
        .to_string(),
    )
    .unwrap();
    let (_, event2) = find_unreported_marker(&run_dir).expect("nested marker detected");
    let event2 = event2.expect("nested marker parses");
    assert_eq!(event2.event_id, "evt-2");
}

#[tokio::test]
async fn terminate_kills_spawned_child() {
    let dir = temp_dir("terminate");
    let mut spawn = spawn_agent(&sleeper_cmd(30), &dir, &[], None).unwrap();
    assert!(spawn.child.id().is_some(), "sleeper process is running");
    let outcome = terminate(&mut spawn.child).await.unwrap();
    assert_eq!(outcome, "Killed");
    assert!(spawn.child.id().is_none(), "child reaped after terminate");
}

#[tokio::test]
async fn reply_injection_writes_atomic_reply_file() {
    let dir = temp_dir("reply");
    let mut spawn = spawn_agent(
        &sleeper_cmd(30),
        &dir,
        &[],
        Some(&json!({ "decision": "approve", "content": "允许删除" })),
    )
    .unwrap();
    // INPUT_INJECTED 判据（V1.1 §1.4）：reply.json 原子落盘。
    let reply = dir.join(".aipm").join("reply.json");
    assert!(reply.is_file());
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(reply).unwrap()).unwrap();
    assert_eq!(value["decision"], "approve");
    assert_eq!(spawn.session_id.starts_with("runner:"), true);
    terminate(&mut spawn.child).await.unwrap();
}

#[cfg(windows)]
fn sleeper_cmd(seconds: u64) -> String {
    format!("ping -n {seconds} 127.0.0.1")
}

#[cfg(not(windows))]
fn sleeper_cmd(seconds: u64) -> String {
    format!("sleep {seconds}")
}
