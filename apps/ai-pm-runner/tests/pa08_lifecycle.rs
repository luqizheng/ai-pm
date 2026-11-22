//! PA08-02 first lifecycle slice: exercise the production claim-and-spawn path
//! through the real MCP client and a scripted Platform endpoint.
use ai_pm_runner::adapter::terminate;
use ai_pm_runner::mcp::McpClient;
use ai_pm_runner::runner_loop::{claim_and_start, tick, ActiveRun, Config};
use std::time::{Duration, Instant};
use serde_json::json;
use std::path::PathBuf;
use uuid::Uuid;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn temp_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("aipm-pa08-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn sleeper_cmd(seconds: u64) -> String {
    #[cfg(windows)]
    { format!("ping -n {seconds} 127.0.0.1") }
    #[cfg(not(windows))]
    { format!("sleep {seconds}") }
}

fn config(server: &MockServer, workspace_root: PathBuf) -> Config {
    Config {
        platform_mcp_url: format!("{}/mcp", server.uri()),
        project_id: Uuid::nil(),
        client_name: "pa08-lifecycle-test".into(),
        workspace_root,
        adapter: "custom".into(),
        agent_cmd: sleeper_cmd(30),
        git_map: None,
        lease_secs: 300,
        claim_interval_secs: 10,
        heartbeat_secs: 60,
        detect_interval_secs: 5,
        command_poll_secs: 3,
        terminate_attempts: 2,
        terminate_timeout_secs: 5,
        backoff_base_ms: 100,
        backoff_cap_ms: 1_000,
    }
}

fn structured(value: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": { "structuredContent": value }
    }))
}

/// Exercises list -> claim -> run-context -> real process spawn. The test uses
/// the production claim_and_start function; only the remote Platform is scripted.
#[tokio::test]
async fn claim_and_start_claims_run_and_spawns_agent() {
    let server = MockServer::start().await;
    let task_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({ "method": "tools/call", "params": { "name": "pm_list_tasks" } })))
        .respond_with(structured(json!({ "tasks": [{
            "task_id": task_id, "queued_run_id": run_id
        }] })))
        .expect(1)
        .mount(&server).await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({ "method": "tools/call", "params": { "name": "pm_claim_task" } })))
        .respond_with(structured(json!({
            "run_id": run_id,
            "lease_token": "test-lease-token",
            "lease_generation": 7,
            "lease_owner": "client:pa08-test-device"
        })))
        .expect(1)
        .mount(&server).await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({ "method": "tools/call", "params": { "name": "pm_get_run_context" } })))
        .respond_with(structured(json!({ "repositories": [] })))
        .expect(1)
        .mount(&server).await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let mut active = claim_and_start(&cfg, &client)
        .await
        .expect("claim and spawn should succeed")
        .expect("one queued task should be claimed");

    assert_eq!(active.task_id, task_id);
    assert_eq!(active.run_id, run_id);
    assert_eq!(active.lease_generation, 7);
    assert_eq!(active.worker_id, "client:pa08-test-device");
    assert!(active.process.as_ref().and_then(|p| p.child.id()).is_some());

    // No child process is allowed to leak from an integration test.
    if let Some(process) = active.process.as_mut() {
        terminate(&mut process.child).await.expect("test child terminates");
    }
    let _ = std::fs::remove_dir_all(root);
}

/// An empty discovery result must not claim or spawn anything.
#[tokio::test]
async fn claim_and_start_returns_none_for_empty_queue() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({ "method": "tools/call", "params": { "name": "pm_list_tasks" } })))
        .respond_with(structured(json!({ "tasks": [] })))
        .expect(1)
        .mount(&server).await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let active = claim_and_start(&cfg, &client).await.unwrap();
    assert!(active.is_none());
    let _ = std::fs::remove_dir_all(root);
}

/// A single supervision tick renews the lease before polling the command channel.
#[tokio::test]
async fn tick_reports_heartbeat_and_polls_commands() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_report_progress" }
        })))
        .respond_with(structured(json!({ "ok": true })))
        .expect(1)
        .mount(&server).await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": {
                "name": "pm_poll_commands",
                "arguments": {
                    "project_id": Uuid::nil(),
                    "run_id": Uuid::nil(),
                    "worker_id": "client:pa08-test-device"
                }
            }
        })))
        .respond_with(structured(json!({ "commands": [] })))
        .expect(1)
        .mount(&server).await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let now = Instant::now();
    let mut run = ActiveRun {
        project_id: Uuid::nil(),
        task_id: Uuid::new_v4(),
        run_id: Uuid::nil(),
        lease_token: "test-lease-token".into(),
        lease_generation: 4,
        worker_id: "client:pa08-test-device".into(),
        run_dir: root.clone(),
        stage: "started".into(),
        process: None,
        last_heartbeat: now - Duration::from_secs(120),
        last_detect: now,
        last_command_poll: now - Duration::from_secs(10),
        pending_interaction: None,
        awaiting_continue: true,
        lease_failures: 0,
        last_lease_attempt: None,
    };

    tick(&cfg, &client, &mut run)
        .await
        .expect("heartbeat and command poll should succeed");
    assert!(run.last_heartbeat.elapsed() < Duration::from_secs(2));
    assert!(run.last_command_poll.elapsed() < Duration::from_secs(2));
    let _ = std::fs::remove_dir_all(root);
}


/// A terminate command must kill the real child and acknowledge the command
/// with the current lease generation and an exit outcome.
#[tokio::test]
async fn terminate_command_kills_child_and_acks_receipt() {
    let server = MockServer::start().await;
    let command_id = Uuid::new_v4();
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_poll_commands" }
        })))
        .respond_with(structured(json!({ "commands": [{
            "command_id": command_id,
            "command_type": "agent.process.terminate",
            "payload": { "issued_at": "2026-11-22T00:00:00Z" }
        }] })))
        .expect(1)
        .mount(&server).await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": {
                "name": "pm_ack_command",
                "arguments": {
                    "project_id": Uuid::nil(),
                    "command_id": command_id,
                    "worker_id": "client:pa08-test-device",
                    "outcome": "acked"
                }
            }
        })))
        .respond_with(structured(json!({ "ok": true })))
        .expect(1)
        .mount(&server).await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let process = ai_pm_runner::adapter::spawn_agent(
        &cfg.agent_cmd,
        &root,
        &[],
        None,
        &[],
    ).expect("spawn long-lived child");

    let mut run = ActiveRun {
        project_id: Uuid::nil(),
        task_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        lease_token: "test-lease-token".into(),
        lease_generation: 9,
        worker_id: "client:pa08-test-device".into(),
        run_dir: root.clone(),
        stage: "started".into(),
        process: Some(process),
        last_heartbeat: Instant::now(),
        last_detect: Instant::now(),
        last_command_poll: Instant::now(),
        pending_interaction: None,
        awaiting_continue: false,
        lease_failures: 0,
        last_lease_attempt: None,
    };

    run.last_command_poll = Instant::now() - Duration::from_secs(10);
    tick(&cfg, &client, &mut run)
        .await
        .expect("tick should poll and execute terminate command");

    assert!(run.awaiting_continue);
    let child_id = run.process.as_ref().and_then(|p| p.child.id());
    assert!(child_id.is_none(), "terminate must reap the child process");
    let _ = std::fs::remove_dir_all(root);
}

/// A continue command must inject the human reply, spawn a fresh session,
/// clear the awaiting-continue gate, and acknowledge it.
#[tokio::test]
async fn continue_command_spawns_session_and_injects_reply() {
    let server = MockServer::start().await;
    let command_id = Uuid::new_v4();
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": {
                "name": "pm_ack_command",
                "arguments": {
                    "project_id": Uuid::nil(),
                    "command_id": command_id,
                    "worker_id": "client:pa08-test-device",
                    "outcome": "acked"
                }
            }
        })))
        .respond_with(structured(json!({ "ok": true })))
        .expect(1)
        .mount(&server).await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let mut run = ActiveRun {
        project_id: Uuid::nil(),
        task_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        lease_token: "test-lease-token".into(),
        lease_generation: 11,
        worker_id: "client:pa08-test-device".into(),
        run_dir: root.clone(),
        stage: "started".into(),
        process: None,
        last_heartbeat: Instant::now(),
        last_detect: Instant::now(),
        last_command_poll: Instant::now(),
        pending_interaction: None,
        awaiting_continue: true,
        lease_failures: 0,
        last_lease_attempt: None,
    };
    let reply = json!({ "answer": "Use the safer migration path." });

    ai_pm_runner::runner_loop::execute_command(
        &cfg,
        &client,
        &mut run,
        json!({
            "command_id": command_id,
            "command_type": "agent.task.continue",
            "payload": {
                "response": reply,
                "guard": { "continuation_start_frozen": false }
            }
        }),
    ).await;

    assert!(!run.awaiting_continue);
    assert!(run.process.as_ref().and_then(|p| p.child.id()).is_some());
    let injected: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join(".aipm").join("reply.json")).expect("reply injection file")
    ).expect("valid reply JSON");
    assert_eq!(injected, reply);

    if let Some(process) = run.process.as_mut() {
        terminate(&mut process.child).await.expect("continued child terminates");
    }
    let _ = std::fs::remove_dir_all(root);
}

/// A frozen continuation guard must fail closed and must not spawn a process.
#[tokio::test]
async fn continue_command_refuses_frozen_start_without_spawning() {
    let server = MockServer::start().await;
    let command_id = Uuid::new_v4();
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": {
                "name": "pm_ack_command",
                "arguments": {
                    "project_id": Uuid::nil(),
                    "command_id": command_id,
                    "worker_id": "client:pa08-test-device",
                    "outcome": "failed"
                }
            }
        })))
        .respond_with(structured(json!({ "ok": true })))
        .expect(1)
        .mount(&server).await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let mut run = ActiveRun {
        project_id: Uuid::nil(),
        task_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        lease_token: "test-lease-token".into(),
        lease_generation: 12,
        worker_id: "client:pa08-test-device".into(),
        run_dir: root.clone(),
        stage: "started".into(),
        process: None,
        last_heartbeat: Instant::now(),
        last_detect: Instant::now(),
        last_command_poll: Instant::now(),
        pending_interaction: None,
        awaiting_continue: true,
        lease_failures: 0,
        last_lease_attempt: None,
    };

    ai_pm_runner::runner_loop::execute_command(
        &cfg,
        &client,
        &mut run,
        json!({
            "command_id": command_id,
            "command_type": "agent.task.continue",
            "payload": {
                "response": { "answer": "must not run" },
                "guard": { "continuation_start_frozen": true }
            }
        }),
    ).await;

    assert!(run.awaiting_continue);
    assert!(run.process.is_none());
    assert!(!root.join(".aipm").join("reply.json").exists());
    let _ = std::fs::remove_dir_all(root);
}
