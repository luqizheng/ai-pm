//! Runner 通信面测试：MCP 客户端解析（structuredContent / isError）、标记文件
//! 生命周期（侦测→解析→归档/隔离）、进程终止与回复注入。不依赖真实 Platform。

use ai_pm_runner::adapter::{
    archive_marker, find_unreported_markers, quarantine_marker, spawn_agent, terminate,
    terminate_spawned, terminate_with_retry, ExitOutcome,
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
            ] } } })))
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
                        "content": [{ "text": "task is not ready (current status: draft)" }] } })))
        .mount(&server)
        .await;

    let client = mcp_client(server.uri());
    let tasks = client.list_ready_tasks(Uuid::nil()).await.unwrap();
    assert_eq!(
        tasks.len(),
        1,
        "tasks without a queued run must be filtered out"
    );
    assert_eq!(
        tasks[0].1.to_string(),
        "00000000-0000-0000-0000-000000000002"
    );

    let err = client
        .claim_task(Uuid::nil(), Uuid::new_v4(), "runner:test")
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("not ready"),
        "tool error text surfaces: {err}"
    );
}

#[tokio::test]
async fn marker_file_lifecycle_detect_parse_archive_quarantine() {
    let dir = temp_dir("marker-life");
    let run_dir = dir.join("run-1");
    let aipm = run_dir.join(".aipm");
    std::fs::create_dir_all(&aipm).unwrap();

    // 未上报前：侦测不到。
    assert!(find_unreported_markers(&run_dir).is_empty());

    // 合法标记：解析 + 归档后不再侦测到（ACP-443：event_id 键控归档）。
    let marker = aipm.join("interaction.evt-1.json");
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
    let (path, event) = find_unreported_markers(&run_dir)
        .into_iter()
        .next()
        .expect("marker detected");
    let event = event.expect("valid marker parses");
    assert_eq!(event.event_id, "evt-1");
    assert_eq!(event.reason, "needs_decision");
    assert!(event.required_response);
    assert_eq!(
        event.checkpoint_path.as_deref(),
        Some(".aipm/checkpoint.json")
    );
    archive_marker(&path, &event.event_id).unwrap();
    assert!(find_unreported_markers(&run_dir).is_empty());
    assert!(aipm.join("interaction.evt-1.reported").is_file());

    // 非法 schema → 侦测返回 Err → 隔离（不产生 Interaction，§1.2）。
    std::fs::write(
        &marker,
        json!({ "schema_version": "unknown.v9" }).to_string(),
    )
    .unwrap();
    let (path, parsed) = find_unreported_markers(&run_dir)
        .into_iter()
        .next()
        .expect("malformed marker must be surfaced");
    assert!(parsed.is_err(), "malformed schema must not parse");
    quarantine_marker(&path).unwrap();
    assert!(aipm.join("interaction.evt-1.json.invalid").is_file());
    assert!(find_unreported_markers(&run_dir).is_empty());

    // 子目录层（workspace_path 段）同样可侦测。
    let nested = run_dir.join("repo-checkout").join(".aipm");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(
        nested.join("interaction.evt-2.json"),
        json!({
            "schema_version": "aipm.interaction.v1",
            "event_id": "evt-2", "reason": "needs_info",
            "question": "依赖版本？"
        })
        .to_string(),
    )
    .unwrap();
    let (_, event2) = find_unreported_markers(&run_dir)
        .into_iter()
        .next()
        .expect("nested marker detected");
    let event2 = event2.expect("nested marker parses");
    assert_eq!(event2.event_id, "evt-2");
}

#[tokio::test]
async fn terminate_kills_spawned_child() {
    let dir = temp_dir("terminate");
    let mut spawn = spawn_agent(&sleeper_cmd(30), &dir, &[], None).unwrap();
    assert!(spawn.child.id().is_some(), "sleeper process is running");
    let outcome = terminate(&mut spawn.child).await.unwrap();
    assert_eq!(outcome, ExitOutcome::Killed);
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
    assert!(spawn.session_id.starts_with("runner:"));
    terminate(&mut spawn.child).await.unwrap();
}

// ───────────────────────── F-1 / F-6（复审 2026-11-07 闭合项） ─────────────────────────

use ai_pm_runner::runner_loop::{execute_command, preflight_continue, tick, ActiveRun, Config};
use std::time::Instant;

fn guard_json(frozen: bool, deadline_offset_secs: i64) -> serde_json::Value {
    json!({
        "continuation_start_frozen": frozen,
        "agent_deadline_at": (chrono::Utc::now() + chrono::Duration::seconds(deadline_offset_secs)).to_rfc3339(),
    })
}

#[tokio::test]
async fn f1_preflight_refuses_frozen_and_expired_deadline() {
    // 冻结（terminate 失败）→ 拒绝（ACP-444）。
    assert!(preflight_continue(&guard_json(true, 3600)).is_err());
    // Deadline 已过（D-02 不顺延）→ 拒绝（V1.3.1 §9.2）。
    assert!(preflight_continue(&guard_json(false, -60)).is_err());
    // 健康快照 / 旧版本无 guard → 放行。
    assert!(preflight_continue(&guard_json(false, 3600)).is_ok());
    assert!(preflight_continue(&serde_json::Value::Null).is_ok());
}

#[tokio::test]
async fn f6_terminate_with_retry_reports_killed() {
    let dir = temp_dir("terminate-retry");
    let mut spawn = spawn_agent(&sleeper_cmd(30), &dir, &[], None).unwrap();
    let outcome = terminate_with_retry(&mut spawn.child, 3, 10).await.unwrap();
    assert_eq!(
        outcome,
        ExitOutcome::Killed,
        "first-attempt kill yields Killed within budget"
    );
}

#[tokio::test]
async fn acp444_executor_side_refuses_frozen_continue() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call", "params": { "name": "pm_ack_command" }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0", "id": 9,
            "result": { "structuredContent": { "command_id": "x", "status": "failed" } } })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call", "params": { "name": "pm_poll_commands" }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0", "id": 8, "result": { "structuredContent": { "commands": [] } } })))
        .mount(&server)
        .await;

    let dir = temp_dir("acp444");
    let spawn = spawn_agent(&sleeper_cmd(30), &dir, &[], None).unwrap();
    let mut run = ActiveRun {
        project_id: Uuid::nil(),
        task_id: Uuid::nil(),
        run_id: Uuid::nil(),
        lease_token: "tok".into(),
        lease_generation: 1,
        worker_id: "mcp:runner:t:tok12345".into(),
        run_dir: dir.clone(),
        stage: "started".into(),
        process: Some(spawn),
        last_heartbeat: Instant::now(),
        last_detect: Instant::now(),
        last_command_poll: Instant::now(),
        pending_interaction: None,
        awaiting_continue: true,
        lease_failures: 0,
        last_lease_attempt: None,
    };
    let cfg = Config {
        platform_mcp_url: format!("{}/mcp", server.uri()),
        project_id: Uuid::nil(),
        client_name: "runner:test".into(),
        workspace_root: dir.clone(),
        agent_cmd: sleeper_cmd(30),
        claim_interval_secs: 10,
        heartbeat_secs: 60,
        detect_interval_secs: 5,
        command_poll_secs: 3,
        terminate_attempts: 3,
        terminate_timeout_secs: 10,
        adapter: "custom".to_string(),
        lease_secs: 300,
        git_map: None,
        backoff_base_ms: 2000,
        backoff_cap_ms: 60_000,
    };
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let continue_cmd = json!({
        "command_id": "00000000-0000-0000-0000-00000000abcd",
        "command_type": "agent.task.continue",
        "payload": {
            "command_type": "agent.task.continue",
            "response": { "decision": "approve" },
            "guard": { "continuation_start_frozen": true }
        }
    });
    execute_command(&cfg, &client, &mut run, continue_cmd).await;
    // 未 spawn 新进程（原进程仍在），回复注入文件未写。
    assert!(
        run.child_id().is_some(),
        "frozen continue must not touch the running process"
    );
    assert!(!dir.join(".aipm").join("reply.json").is_file());
    run_terminate_on_drop(&mut run).await;
}

// 测试收尾辅助：终止遗留进程，避免 test 残留 sleeper。
async fn run_terminate_on_drop(run: &mut ActiveRun) {
    let _ = terminate(&mut run.process.as_mut().expect("process present").child).await;
}

trait ChildId {
    fn child_id(&self) -> Option<u32>;
}

impl ChildId for ActiveRun {
    fn child_id(&self) -> Option<u32> {
        self.process.as_ref().map(|p| p.child.id()).unwrap_or(None)
    }
}

#[cfg(windows)]
fn sleeper_cmd(seconds: u64) -> String {
    format!("ping -n {seconds} 127.0.0.1")
}

#[cfg(not(windows))]
fn sleeper_cmd(seconds: u64) -> String {
    format!("sleep {seconds}")
}

#[tokio::test]
async fn f9_tool_rejection_archives_marker_without_retry() {
    let server = MockServer::start().await;
    // Platform 业务拒绝（开放链冲突语义）→ isError 文本上抛。
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call", "params": { "name": "pm_report_interaction" }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0", "id": 7,
            "result": { "isError": true,
                        "content": [{ "text": "an open intervention chain exists for this task" }] } })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call", "params": { "name": "pm_poll_commands" }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0", "id": 6, "result": { "structuredContent": { "commands": [] } } })))
        .mount(&server)
        .await;

    let dir = temp_dir("f9");
    let run_dir = dir.join("run-f9");
    std::fs::create_dir_all(run_dir.join(".aipm")).unwrap();
    std::fs::write(
        run_dir.join(".aipm").join("interaction.evt-9.json"),
        json!({
            "schema_version": "aipm.interaction.v1",
            "event_id": "evt-9", "reason": "needs_info", "question": "q"
        })
        .to_string(),
    )
    .unwrap();
    let spawn = spawn_agent(&sleeper_cmd(30), &run_dir, &[], None).unwrap();
    let mut run = ActiveRun {
        project_id: Uuid::nil(),
        task_id: Uuid::nil(),
        run_id: Uuid::nil(),
        lease_token: "tok".into(),
        lease_generation: 1,
        worker_id: "mcp:runner:t:tok12345".into(),
        run_dir: run_dir.clone(),
        stage: "started".into(),
        process: Some(spawn),
        last_heartbeat: Instant::now(),
        last_detect: Instant::now() - std::time::Duration::from_secs(6),
        last_command_poll: Instant::now(),
        pending_interaction: None,
        awaiting_continue: false,
        lease_failures: 0,
        last_lease_attempt: None,
    };
    let cfg = Config {
        platform_mcp_url: format!("{}/mcp", server.uri()),
        project_id: Uuid::nil(),
        client_name: "runner:test".into(),
        workspace_root: dir.clone(),
        agent_cmd: sleeper_cmd(30),
        claim_interval_secs: 10,
        heartbeat_secs: 3600,
        detect_interval_secs: 5,
        command_poll_secs: 3600,
        terminate_attempts: 3,
        terminate_timeout_secs: 10,
        adapter: "custom".to_string(),
        lease_secs: 300,
        git_map: None,
        backoff_base_ms: 2000,
        backoff_cap_ms: 60_000,
    };
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    tick(&cfg, &client, &mut run).await.unwrap();
    // Tool 类拒绝 → .rejected 归档（§1.2 不重试；防队头阻塞）。
    assert!(run_dir
        .join(".aipm")
        .join("interaction.evt-9.rejected")
        .is_file());
    assert!(!run_dir
        .join(".aipm")
        .join("interaction.evt-9.json")
        .is_file());
    let _ = terminate(&mut run.process.as_mut().expect("process present").child).await;
}

#[tokio::test]
async fn acp08_job_object_assigned_and_tree_terminates() {
    let dir = temp_dir("job-object");
    let mut spawn = spawn_agent(&sleeper_cmd(30), &dir, &[], None).unwrap();
    #[cfg(windows)]
    assert!(
        spawn.job.is_some(),
        "windows spawn must carry a Job Object handle"
    );
    // 整树终止入口：Job Terminate + 收割。
    let outcome = terminate_spawned(&mut spawn).await.unwrap();
    assert_eq!(outcome, ExitOutcome::Killed);
}
