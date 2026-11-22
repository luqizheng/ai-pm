//! PA08-02 first lifecycle slice: exercise the production claim-and-spawn path
//! through the real MCP client and a scripted Platform endpoint.
use ai_pm_runner::adapter::{read_process_record, scan_orphans, spawn_agent, terminate, terminate_orphan};
use ai_pm_runner::mcp::McpClient;
use ai_pm_runner::runner_loop::{claim_and_start, recover_inflight_run, run_loop, tick, ActiveRun, Config};
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
        .expect(2)
        .mount(&server).await;

    let failed_ack = Mock::given(method("POST"))
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
        .respond_with(ResponseTemplate::new(503).set_body_string("terminate ack unavailable"))
        .expect(1)
        .mount_as_scoped(&server)
        .await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let process = ai_pm_runner::adapter::spawn_agent(&cfg.agent_cmd, &root, &[], None)
        .expect("spawn long-lived child");
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
        last_command_poll: Instant::now() - Duration::from_secs(10),
        pending_interaction: None,
        awaiting_continue: false,
        lease_failures: 0,
        last_lease_attempt: None,
    };
    tick(&cfg, &client, &mut run).await.expect("first terminate delivery should execute despite lost ACK");
    assert!(run.awaiting_continue);
    assert!(run.process.as_ref().and_then(|p| p.child.id()).is_none(), "first delivery must reap the child process");
    drop(failed_ack);
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
    run.last_command_poll = Instant::now() - Duration::from_secs(10);
    tick(&cfg, &client, &mut run).await.expect("redelivered terminate must be idempotently acknowledged");
    assert!(run.awaiting_continue);
    assert!(run.process.as_ref().and_then(|p| p.child.id()).is_none(), "redelivery must not spawn or replace a child");
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
            "params": { "name": "pm_poll_commands" }
        })))
        .respond_with(structured(json!({ "commands": [{
            "command_id": command_id,
            "command_type": "agent.task.continue",
            "payload": {
                "response": { "answer": "Use the safer migration path." },
                "guard": { "continuation_start_frozen": false }
            }
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

    run.last_command_poll = Instant::now() - Duration::from_secs(10);
    tick(&cfg, &client, &mut run)
        .await
        .expect("tick should poll and execute continue command");

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


/// A failed heartbeat must not advance the renewal clock; the next tick must
/// retry immediately rather than waiting another full heartbeat interval.
#[tokio::test]
async fn failed_heartbeat_keeps_renewal_due() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_report_progress" }
        })))
        .respond_with(ResponseTemplate::new(503).set_body_string("platform unavailable"))
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
        lease_generation: 13,
        worker_id: "client:pa08-test-device".into(),
        run_dir: root.clone(),
        stage: "started".into(),
        process: None,
        last_heartbeat: Instant::now() - Duration::from_secs(120),
        last_detect: Instant::now(),
        last_command_poll: Instant::now(),
        pending_interaction: None,
        awaiting_continue: true,
        lease_failures: 0,
        last_lease_attempt: None,
    };

    let result = tick(&cfg, &client, &mut run).await;
    assert!(result.is_err(), "HTTP 503 must propagate as a failed heartbeat");
    assert!(
        run.last_heartbeat.elapsed() >= Duration::from_secs(cfg.heartbeat_secs),
        "failed renewal must remain due for the next tick"
    );
    let _ = std::fs::remove_dir_all(root);
}


/// A restarted Runner discovers the in-flight intervention, reacquires the
/// same Run with a newer generation, and resumes in a process-less waiting state.
#[tokio::test]
async fn restart_recovery_reacquires_same_run_and_routes_continue() {
    let server = MockServer::start().await;
    let task_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let interaction_id = Uuid::new_v4();

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_discover_runs" }
        })))
        .respond_with(structured(json!({ "runs": [{
            "run_id": run_id,
            "task_id": task_id,
            "interaction_id": interaction_id,
            "lease_generation": 13
        }] })))
        .expect(1)
        .mount(&server).await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": {
                "name": "pm_reacquire_lease",
                "arguments": {
                    "project_id": Uuid::nil(),
                    "run_id": run_id
                }
            }
        })))
        .respond_with(structured(json!({
            "lease_token": "recovered-lease-token",
            "lease_generation": 14,
            "worker_id": "client:pa08-restarted-device",
            "rerouted_commands": 1
        })))
        .expect(1)
        .mount(&server).await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let recovered = recover_inflight_run(&cfg, &client)
        .await
        .expect("discovery and lease reacquisition should succeed")
        .expect("a discovered in-flight run should be recovered");

    assert_eq!(recovered.project_id, Uuid::nil());
    assert_eq!(recovered.task_id, task_id);
    assert_eq!(recovered.run_id, run_id);
    assert_eq!(recovered.lease_token, "recovered-lease-token");
    assert_eq!(recovered.lease_generation, 14);
    assert_eq!(recovered.worker_id, "client:pa08-restarted-device");
    assert_eq!(recovered.pending_interaction, Some(interaction_id));
    assert!(recovered.process.is_none(), "recovery must not invent a local child handle");
    assert!(recovered.awaiting_continue);
    assert!(root.join(run_id.to_string()).is_dir());
    let _ = std::fs::remove_dir_all(root);
}

/// A replacement Runner must reject a reacquire response that does not advance
/// the discovered generation; the old Agent must remain fenced out.
#[tokio::test]
async fn restart_recovery_rejects_non_advancing_lease_generation() {
    let server = MockServer::start().await;
    let task_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_discover_runs" }
        })))
        .respond_with(structured(json!({ "runs": [{
            "run_id": run_id,
            "task_id": task_id,
            "lease_generation": 14
        }] })))
        .expect(1)
        .mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": {
                "name": "pm_reacquire_lease",
                "arguments": { "project_id": Uuid::nil(), "run_id": run_id }
            }
        })))
        .respond_with(structured(json!({
            "lease_token": "stale-generation-token",
            "lease_generation": 14,
            "worker_id": "client:pa08-test-device"
        })))
        .expect(1)
        .mount(&server).await;
    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let error = match recover_inflight_run(&cfg, &client).await {
        Err(error) => error,
        Ok(_) => panic!("same generation must not fence old Agent"),
    };
    assert!(error.to_string().contains("did not fence prior generation"));
    let _ = std::fs::remove_dir_all(root);
}

/// No discoverable in-flight intervention must leave the Runner with no active Run.
#[tokio::test]
async fn restart_recovery_returns_none_when_no_runs_are_discoverable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_discover_runs" }
        })))
        .respond_with(structured(json!({ "runs": [] })))
        .expect(1)
        .mount(&server).await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    assert!(recover_inflight_run(&cfg, &client).await.unwrap().is_none());
    let _ = std::fs::remove_dir_all(root);
}


/// Startup discovery transport/server failure is not equivalent to an empty Run list.
/// Recovery must fail closed so run_loop cannot proceed to claim replacement work.
#[tokio::test]
async fn restart_recovery_fails_closed_when_discovery_returns_503() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_discover_runs" }
        })))
        .respond_with(ResponseTemplate::new(503).set_body_string("platform unavailable"))
        .expect(1)
        .mount(&server).await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let error = match recover_inflight_run(&cfg, &client).await {
        Err(error) => error,
        Ok(_) => panic!("503 must block startup recovery"),
    };
    assert!(error.to_string().contains("503"), "unexpected error: {error}");
    let _ = std::fs::remove_dir_all(root);
}

/// A single-active-run Runner must not silently recover one Run and ignore another.
#[tokio::test]
async fn restart_recovery_fails_closed_when_multiple_runs_are_discovered() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_discover_runs" }
        })))
        .respond_with(structured(json!({ "runs": [
            { "run_id": Uuid::new_v4(), "task_id": Uuid::new_v4() },
            { "run_id": Uuid::new_v4(), "task_id": Uuid::new_v4() }
        ] })))
        .expect(1)
        .mount(&server).await;
    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let error = match recover_inflight_run(&cfg, &client).await {
        Err(error) => error,
        Ok(_) => panic!("multiple discovered Runs must fail closed"),
    };
    assert!(error.to_string().contains("multiple in-flight Runs"));
    let _ = std::fs::remove_dir_all(root);
}

/// A failed discovery gate must return before task listing or claiming is attempted.
#[tokio::test]
async fn startup_discovery_failure_prevents_task_listing_and_claiming() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_discover_runs" }
        })))
        .respond_with(ResponseTemplate::new(503).set_body_string("platform unavailable"))
        .expect(1)
        .mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_list_tasks" }
        })))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_claim_task" }
        })))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server).await;
    let root = temp_dir();
    let orphan_dir = root.join(Uuid::new_v4().to_string());
    let mut orphan = spawn_agent(&sleeper_cmd(30), &orphan_dir, &[], None)
        .expect("previous Runner Agent must start for the fencing test");
    let orphan_pid = orphan.pid;
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    assert!(run_loop(&cfg, &client).await.is_err());
    assert!(
        ai_pm_runner::adapter::is_pid_alive(orphan_pid),
        "discovery failure must not terminate an old Agent before its lease can be fenced"
    );
    terminate(&mut orphan.child).await.expect("test must reap its child");
    server.verify().await;
    let _ = std::fs::remove_dir_all(root);
}

/// Missing lease generation cannot prove that a replacement Runner fenced the old owner.
#[tokio::test]
async fn restart_recovery_rejects_missing_lease_generation() {
    let server = MockServer::start().await;
    let run_id = Uuid::new_v4();
    let task_id = Uuid::new_v4();
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_discover_runs" }
        })))
        .respond_with(structured(json!({ "runs": [{
            "run_id": run_id,
            "task_id": task_id
        }] })))
        .expect(1)
        .mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_reacquire_lease" }
        })))
        .respond_with(structured(json!({})))
        .expect(0)
        .mount(&server).await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let error = match recover_inflight_run(&cfg, &client).await {
        Err(error) => error,
        Ok(_) => panic!("missing lease generation must fail closed"),
    };
    assert!(error.to_string().contains("lease_generation"));
    server.verify().await;
    let _ = std::fs::remove_dir_all(root);
}

/// Transport failure must propagate; it is never interpreted as an empty discovery result.
#[tokio::test]
async fn restart_recovery_fails_closed_on_transport_failure() {
    let server = MockServer::start().await;
    let root = temp_dir();
    let mut cfg = config(&server, root.clone());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    cfg.platform_mcp_url = format!("http://{address}/mcp");
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        recover_inflight_run(&cfg, &client),
    )
    .await
    .expect("connection failure must return promptly");
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("transport failure must never become an empty recovery result"),
    };
    assert!(!error.to_string().is_empty());
    let _ = std::fs::remove_dir_all(root);
}

/// A successful RPC with a missing or malformed runs field is not proof of emptiness.
#[tokio::test]
async fn restart_recovery_rejects_malformed_discovery_payload() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_discover_runs" }
        })))
        .respond_with(structured(json!({ "unexpected": [] })))
        .expect(1)
        .mount(&server).await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let error = match recover_inflight_run(&cfg, &client).await {
        Err(error) => error,
        Ok(_) => panic!("malformed payload must fail closed"),
    };
    assert!(error.to_string().contains("runs array"), "unexpected error: {error}");
    let _ = std::fs::remove_dir_all(root);
}

/// Spawn failure after a successful claim/context exchange must be surfaced,
/// not silently converted into a running Agent.
#[tokio::test]
async fn claim_and_start_surfaces_agent_spawn_failure() {
    let server = MockServer::start().await;
    let task_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_list_tasks" }
        })))
        .respond_with(structured(json!({ "tasks": [{
            "task_id": task_id, "queued_run_id": run_id
        }] })))
        .expect(1)
        .mount(&server).await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_claim_task" }
        })))
        .respond_with(structured(json!({
            "run_id": run_id,
            "lease_token": "test-lease-token",
            "lease_generation": 2,
            "lease_owner": "client:pa08-test-device"
        })))
        .expect(1)
        .mount(&server).await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_get_run_context" }
        })))
        .respond_with(structured(json!({ "repositories": [] })))
        .expect(1)
        .mount(&server).await;

    let root = temp_dir();
    let mut cfg = config(&server, root.clone());
    cfg.agent_cmd = "this-command-does-not-exist-pa08".into();
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let result = claim_and_start(&cfg, &client).await;
    assert!(result.is_err(), "a failed spawn must not produce an ActiveRun");
    let _ = std::fs::remove_dir_all(root);
}


/// A delayed command response may be redelivered after an acknowledgement is
/// delayed/lost. The Runner must acknowledge the duplicate without spawning a
/// second continuation process.
#[tokio::test]
async fn delayed_duplicate_continue_response_does_not_spawn_second_agent() {
    let server = MockServer::start().await;
    let command_id = Uuid::new_v4();
    let poll = Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_poll_commands" }
        })))
        .respond_with(
            structured(json!({ "commands": [{
                "command_id": command_id,
                "command_type": "agent.task.continue",
                "payload": {
                    "response": { "answer": "continue once" },
                    "guard": { "continuation_start_frozen": false }
                }
            }] }))
            .set_delay(Duration::from_millis(100)),
        )
        .expect(2);
    poll.mount(&server).await;

    let failed_ack = Mock::given(method("POST"))
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
        .respond_with(ResponseTemplate::new(503).set_body_string("ack temporarily unavailable"))
        .expect(1)
        .mount_as_scoped(&server)
        .await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let mut run = ActiveRun {
        project_id: Uuid::nil(),
        task_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        lease_token: "test-lease-token".into(),
        lease_generation: 17,
        worker_id: "client:pa08-test-device".into(),
        run_dir: root.clone(),
        stage: "started".into(),
        process: None,
        last_heartbeat: Instant::now(),
        last_detect: Instant::now(),
        last_command_poll: Instant::now() - Duration::from_secs(10),
        pending_interaction: None,
        awaiting_continue: true,
        lease_failures: 0,
        last_lease_attempt: None,
    };

    let first_started = Instant::now();
    tick(&cfg, &client, &mut run)
        .await
        .expect("first delayed command response should execute");
    assert!(first_started.elapsed() >= Duration::from_millis(90));
    assert!(!run.awaiting_continue);
    let first_pid = run.process.as_ref().and_then(|process| process.child.id());
    let first_session = run.process.as_ref().map(|process| process.session_id.clone());
    assert!(first_pid.is_some(), "first delivery must spawn an Agent");
    // The first acknowledgement is deliberately lost (HTTP 503). The next poll
    // redelivers the same command; the second ACK succeeds without a second spawn.
    drop(failed_ack);
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

    run.last_command_poll = Instant::now() - Duration::from_secs(10);
    tick(&cfg, &client, &mut run)
        .await
        .expect("duplicate command response should be acknowledged idempotently");

    let second_pid = run.process.as_ref().and_then(|process| process.child.id());
    let second_session = run.process.as_ref().map(|process| process.session_id.clone());
    assert_eq!(second_pid, first_pid, "duplicate delivery must not replace the live child");
    assert_eq!(second_session, first_session, "duplicate delivery must reuse the existing session");
    assert!(!run.awaiting_continue);

    if let Some(process) = run.process.as_mut() {
        let _ = process.child.kill().await;
        let _ = process.child.wait().await;
    }
    let _ = std::fs::remove_dir_all(root);
}


/// Command-poll transport failure must not advance the poll clock. The next
/// supervision tick retries immediately and can consume the delayed response.
#[tokio::test]
async fn failed_command_poll_retries_on_next_supervision_tick() {
    let server = MockServer::start().await;
    let failed = Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_poll_commands" }
        })))
        .respond_with(ResponseTemplate::new(503).set_body_string("platform unavailable"))
        .expect(1)
        .mount_as_scoped(&server)
        .await;

    let root = temp_dir();
    let cfg = config(&server, root.clone());
    let client = McpClient::new(cfg.platform_mcp_url.clone(), "test-token");
    let mut run = ActiveRun {
        project_id: Uuid::nil(),
        task_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        lease_token: "test-lease-token".into(),
        lease_generation: 18,
        worker_id: "client:pa08-test-device".into(),
        run_dir: root.clone(),
        stage: "started".into(),
        process: None,
        last_heartbeat: Instant::now(),
        last_detect: Instant::now(),
        last_command_poll: Instant::now() - Duration::from_secs(10),
        pending_interaction: None,
        awaiting_continue: true,
        lease_failures: 0,
        last_lease_attempt: None,
    };
    let due_at = run.last_command_poll;
    let first = tick(&cfg, &client, &mut run).await;
    assert!(first.is_err(), "HTTP 503 must propagate from command polling");
    assert_eq!(
        run.last_command_poll, due_at,
        "failed poll must not advance the retry clock"
    );
    drop(failed);

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": { "name": "pm_poll_commands" }
        })))
        .respond_with(structured(json!({ "commands": [] })))
        .expect(1)
        .mount(&server)
        .await;

    tick(&cfg, &client, &mut run)
        .await
        .expect("next tick should retry command polling immediately");
    assert!(run.last_command_poll.elapsed() < Duration::from_secs(2));
    let _ = std::fs::remove_dir_all(root);
}

/// A replacement Runner must reap the exact process recorded by its previous
/// instance before it is allowed to continue startup recovery.
#[cfg(unix)]
#[tokio::test]
async fn replacement_runner_terminates_identity_matched_orphan_process_group() {
    let root = temp_dir();
    let run_dir = root.join(Uuid::new_v4().to_string());
    let mut spawned = spawn_agent(&sleeper_cmd(30), &run_dir, &[], None).unwrap();
    let pid = spawned.pid;
    let reports = scan_orphans(&root);
    assert_eq!(reports.len(), 1);
    assert!(reports[0].alive);
    terminate_orphan(&reports[0]).expect("identity-matched orphan process group must be terminated");
    let _ = spawned.child.wait().await;
    assert!(!ai_pm_runner::adapter::is_pid_alive(pid), "orphan PID must no longer be alive after reap");
    let _ = std::fs::remove_dir_all(root);
}

/// A PID whose creation identity no longer matches the process record must
/// never be killed: this is the PID-reuse fencing guard.
#[tokio::test]
async fn orphan_cleanup_refuses_to_kill_pid_reuse() {
    let root = temp_dir();
    let run_dir = root.join(Uuid::new_v4().to_string());
    let mut spawned = spawn_agent(&sleeper_cmd(30), &run_dir, &[], None).unwrap();
    let pid = spawned.pid;
    let mut record = read_process_record(&run_dir).expect("spawn writes process record");
    record.identity_hint = Some(
        if cfg!(windows) {
            "win-ft-0-0"
        } else if cfg!(target_os = "linux") {
            "proc-start-0"
        } else {
            "ps-start-not-real"
        }
        .into(),
    );
    std::fs::write(run_dir.join(".aipm").join("process.json"), serde_json::to_vec(&record).unwrap()).unwrap();
    let report = scan_orphans(&root).into_iter().next().unwrap();
    terminate_orphan(&report).expect("PID reuse must be treated as an already-gone original child");
    assert!(ai_pm_runner::adapter::is_pid_alive(pid), "unrelated PID must remain alive");
    terminate(&mut spawned.child).await.expect("test child cleanup");
    let _ = std::fs::remove_dir_all(root);
}
