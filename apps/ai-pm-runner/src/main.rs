//! ai-pm-runner：用户 PC 客户端（V0.5 目标 1 三方通信的执行端载体，WP-3）。
//!
//! 自领循环（客户端化 worker run_executor 的通信面）：
//! 1. 发现：`pm_list_tasks`（项目内 ready 且有 queued Run 的任务）；
//! 2. 领取：`pm_claim_task`（client_name 携 `runner:` 前缀 → Platform stamp
//!    execution_mode='spawn'）；命令路由身份 worker_id = lease_owner 快照
//!    （`mcp:{client_name}:{token8}`，§3.3）；
//! 3. 监督：spawn Agent 进程（`AI_PM_RUNNER_AGENT_CMD` 模板，env 注入租约），
//!    心跳 `pm_report_progress` 续租，标记文件侦测 → `pm_report_interaction`，
//!    命令轮询 `pm_poll_commands` / `pm_ack_command`（terminate 杀进程、
//!    continue 写回复重启新 Session → 消费回执 `pm_report_consumption`）；
//! 4. 交付：Agent 自行经 MCP `pm_submit_result`（Model A 既有模式，Runner 不
//!    代交交付物）。
//!
//! 非目标（V0.5 基线 §八）：多 Runner 高可用、自动升级、LLM 凭据透传。

use ai_pm_runner::adapter::{self, SpawnedProcess};
use ai_pm_runner::mcp;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone)]
struct Config {
    platform_mcp_url: String,
    project_id: Uuid,
    client_name: String,
    workspace_root: PathBuf,
    agent_cmd: String,
    claim_interval_secs: u64,
    heartbeat_secs: u64,
    detect_interval_secs: u64,
    command_poll_secs: u64,
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let url = std::env::var("AI_PM_PLATFORM_URL")
            .map_err(|_| "AI_PM_PLATFORM_URL is required (e.g. http://192.168.2.10:8090/mcp)".to_string())?;
        if std::env::var("AI_PM_RUNNER_TOKEN").is_err() {
            return Err("AI_PM_RUNNER_TOKEN is required (Bearer credential for /mcp)".to_string());
        }
        let project_id: Uuid = std::env::var("AI_PM_RUNNER_PROJECT_ID")
            .map_err(|_| "AI_PM_RUNNER_PROJECT_ID is required (V1: single-project runner)".to_string())?
            .parse()
            .map_err(|_| "AI_PM_RUNNER_PROJECT_ID is not a UUID".to_string())?;
        let host = std::env::var("AI_PM_RUNNER_CLIENT_NAME").unwrap_or_else(|_| hostname());
        let client_name = format!("runner:{host}");
        let workspace_root = std::env::var("AI_PM_RUNNER_WORKSPACE_ROOT")
            .map_err(|_| "AI_PM_RUNNER_WORKSPACE_ROOT is required".to_string())?
            .into();
        let agent_cmd = std::env::var("AI_PM_RUNNER_AGENT_CMD")
            .map_err(|_| "AI_PM_RUNNER_AGENT_CMD is required (agent launch template)".to_string())?;
        Ok(Self {
            platform_mcp_url: url,
            project_id,
            client_name,
            workspace_root,
            agent_cmd,
            claim_interval_secs: env_num("AI_PM_RUNNER_CLAIM_INTERVAL_SECS", 10),
            heartbeat_secs: env_num("AI_PM_RUNNER_HEARTBEAT_SECS", 60),
            detect_interval_secs: env_num("AI_PM_INTERACTION_DETECT_INTERVAL_SECS", 5),
            command_poll_secs: env_num("AI_PM_COMMAND_POLL_INTERVAL_SECS", 3),
        })
    }
}

fn env_num(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "runner".into())
}

/// 一个受监督 Run 的运行时状态。
struct ActiveRun {
    project_id: Uuid,
    task_id: Uuid,
    run_id: Uuid,
    lease_token: String,
    lease_generation: i32,
    worker_id: String,
    run_dir: PathBuf,
    stage: String,
    process: SpawnedProcess,
    last_heartbeat: Instant,
    last_detect: Instant,
    last_command_poll: Instant,
    /// 已上报未消费的 Interaction（responded/consumed 前持有）。
    pending_interaction: Option<Uuid>,
    /// terminate 已 acked、等待 continue 命令的窗口。
    awaiting_continue: bool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let cfg = match Config::from_env() {
        Ok(cfg) => cfg,
        Err(message) => {
            eprintln!("ai-pm-runner: {message}");
            std::process::exit(2);
        }
    };
    let bearer_token = std::env::var("AI_PM_RUNNER_TOKEN").unwrap_or_default();
    let client = mcp::McpClient::new(&cfg.platform_mcp_url, &bearer_token);
    tracing::info!(project = %cfg.project_id, client = %cfg.client_name, "ai-pm-runner starting");
    run_loop(&cfg, &client).await;
}

async fn run_loop(cfg: &Config, client: &mcp::McpClient) {
    let mut active: Option<ActiveRun> = None;
    let mut last_claim_attempt = Instant::now() - Duration::from_secs(cfg.claim_interval_secs);
    loop {
        // ① 进程收割与退出判定。
        if let Some(run) = active.as_mut() {
            if let Some(outcome) = adapter::reap(&mut run.process).await {
                tracing::info!(run = %run.run_id, exit = %outcome, "agent process exited");
                // 交付提交由 Agent 经 MCP 自行完成（Model A 既有模式）；
                // Runner 侧仅记录退出证据。
                active = None;
            }
        }

        // ② 无活跃 Run → 按间隔自领。
        if active.is_none() && last_claim_attempt.elapsed() >= Duration::from_secs(cfg.claim_interval_secs)
        {
            last_claim_attempt = Instant::now();
            match claim_and_start(cfg, client).await {
                Ok(Some(run)) => {
                    tracing::info!(run = %run.run_id, task = %run.task_id, "claimed run");
                    active = Some(run);
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "claim attempt failed"),
            }
        }

        // ③ 活跃 Run 的通信拍：心跳 / 标记侦测 / 命令轮询。
        if let Some(run) = active.as_mut() {
            if let Err(e) = tick(cfg, client, run).await {
                tracing::warn!(error = %e, run = %run.run_id, "supervision tick failed");
            }
            if run.process.child.id().is_none() {
                // 收割竞态兜底：进程句柄已失效。
                active = None;
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn claim_and_start(
    cfg: &Config,
    client: &mcp::McpClient,
) -> Result<Option<ActiveRun>, mcp::RunnerError> {
    let candidates = client.list_ready_tasks(cfg.project_id).await?;
    let Some((task_id, _queued)) = candidates.first().copied() else {
        return Ok(None);
    };
    let claimed = client
        .claim_task(cfg.project_id, task_id, &cfg.client_name)
        .await?;
    let run_id: Uuid = claimed
        .get("run_id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| mcp::RunnerError::Malformed("claim response missing run_id".into()))?;
    let lease_token = claimed
        .get("lease_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| mcp::RunnerError::Malformed("claim response missing lease_token".into()))?
        .to_string();
    let lease_generation = claimed
        .get("lease_generation")
        .and_then(|v| v.as_i64())
        .unwrap_or(1) as i32;
    // 命令路由身份 = Platform 侧 lease_owner 快照（claim.rs ⑨：mcp:{client}:{token8}）。
    let worker_id = format!(
        "mcp:{}:{}",
        cfg.client_name,
        &lease_token[..8.min(lease_token.len())]
    );
    let run_dir = cfg.workspace_root.join(run_id.to_string());
    std::fs::create_dir_all(&run_dir)?;
    let spawn = adapter::spawn_agent(
        &cfg.agent_cmd,
        &run_dir,
        &[
            ("AI_PM_RUN_ID", run_id.to_string()),
            ("AI_PM_TASK_ID", task_id.to_string()),
            ("AI_PM_PROJECT_ID", cfg.project_id.to_string()),
            ("AI_PM_MCP_URL", cfg.platform_mcp_url.clone()),
            ("AI_PM_LEASE_TOKEN", lease_token.clone()),
            ("AI_PM_LEASE_GENERATION", lease_generation.to_string()),
            ("AI_PM_WORKER_ID", worker_id.clone()),
        ],
        None,
    )
    .map_err(|e| mcp::RunnerError::Adapter(e.to_string()))?;
    Ok(Some(ActiveRun {
        project_id: cfg.project_id,
        task_id,
        run_id,
        lease_token,
        lease_generation,
        worker_id,
        run_dir,
        stage: "started".into(),
        process: spawn,
        last_heartbeat: Instant::now(),
        last_detect: Instant::now(),
        last_command_poll: Instant::now() - Duration::from_secs(cfg.command_poll_secs),
        pending_interaction: None,
        awaiting_continue: false,
    }))
}

/// 活跃 Run 的一个通信拍（心跳 / 标记侦测 / 命令执行）。
async fn tick(
    cfg: &Config,
    client: &mcp::McpClient,
    run: &mut ActiveRun,
) -> Result<(), mcp::RunnerError> {
    // ① 心跳 = 续租（stage 已推进过 started，重复上报仅续租）。
    if run.last_heartbeat.elapsed() >= Duration::from_secs(cfg.heartbeat_secs) {
        run.last_heartbeat = Instant::now();
        client
            .report_progress(
                run.project_id,
                run.run_id,
                &run.lease_token,
                run.lease_generation,
                &run.stage,
                "runner heartbeat",
            )
            .await?;
    }

    // ② 标记文件侦测（N-2 ①→②：`.aipm/interaction.json` → pm_report_interaction）。
    if run.pending_interaction.is_none()
        && !run.awaiting_continue
        && run.last_detect.elapsed() >= Duration::from_secs(cfg.detect_interval_secs)
    {
        run.last_detect = Instant::now();
        if let Some((path, parsed)) = adapter::find_unreported_marker(&run.run_dir) {
            let event = match parsed {
                Ok(event) => event,
                // 非法标记：隔离不重试、不产生 Interaction（§1.2 / ACP 评审 P1-4）。
                Err(e) => {
                    if let Err(io_err) = adapter::quarantine_marker(&path) {
                        tracing::warn!(error = %io_err, "marker quarantine failed");
                    }
                    tracing::warn!(error = %e, "invalid interaction marker quarantined");
                    return Ok(());
                }
            };
            match client
                .report_interaction(
                    run.project_id,
                    run.task_id,
                    run.run_id,
                    &run.lease_token,
                    run.lease_generation,
                    &event,
                )
                .await
            {
                Ok(reported) => {
                    let interaction_id = reported
                        .get("interaction_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok());
                    if let Err(e) = adapter::archive_marker(&path, &event.event_id) {
                        // 改名失败保留原文件：下轮幂等重报（ACP-442）。
                        tracing::warn!(error = %e, "marker archive failed; will re-report idempotently");
                    } else {
                        tracing::info!(interaction = ?interaction_id, event = %event.event_id, "interaction reported");
                        run.pending_interaction = interaction_id;
                    }
                }
                Err(e) => {
                    // Platform 拒绝（链冲突/运行态校验等）：保留原文件待下轮；
                    // 幂等键 event_id 保证重复上报不新建（ACP-430）。
                    tracing::warn!(error = %e, "interaction report failed; will retry");
                }
            }
        }
    }

    // ③ 命令轮询与执行（terminate / continue）。
    if run.last_command_poll.elapsed() >= Duration::from_secs(cfg.command_poll_secs) {
        run.last_command_poll = Instant::now();
        let commands = client
            .poll_commands(run.project_id, run.run_id, &run.worker_id)
            .await?;
        for command in commands {
            execute_command(cfg, client, run, command).await;
        }
    }
    Ok(())
}

async fn execute_command(cfg: &Config, client: &mcp::McpClient, run: &mut ActiveRun, command: Value) {
    let command_id: Uuid = match command
        .get("command_id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
    {
        Some(id) => id,
        None => {
            tracing::warn!("command without id; ignored");
            return;
        }
    };
    let command_type = command
        .get("command_type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    match command_type.as_str() {
        "agent.process.terminate" => {
            let outcome = adapter::terminate(&mut run.process.child).await;
            let (outcome_word, receipt, error) = match outcome {
                Ok(exit_outcome) => {
                    tracing::info!(run = %run.run_id, exit = %exit_outcome, "agent terminated");
                    ("acked", json!({ "exit_outcome": exit_outcome, "pid": run.process.pid }), None)
                }
                Err(e) => (
                    // 平台对 terminate acked 要求 exit_outcome（§3.2 从严判据）：
                    // 终止本身失败时按 failed 回执（有界重试在 Platform 侧预算内，
                    // Runner 侧单次失败即回执，由 Platform 决定重投）。
                    "failed", Value::Null, Some(format!("termination failed: {e}")),
                ),
            };
            run.awaiting_continue = true;
            ack(cfg, client, run, command_id, outcome_word, receipt, error).await;
        }
        "agent.task.continue" => {
            // D-03 时序：terminate 已确认后才可能收到 continue（Platform 情形 6 过滤）。
            let response = command
                .get("payload")
                .and_then(|p| p.get("response"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            let spawn = adapter::spawn_agent(
                &cfg.agent_cmd,
                &run.run_dir,
                &[
                    ("AI_PM_RUN_ID", run.run_id.to_string()),
                    ("AI_PM_TASK_ID", run.task_id.to_string()),
                    ("AI_PM_PROJECT_ID", run.project_id.to_string()),
                    ("AI_PM_MCP_URL", cfg.platform_mcp_url.clone()),
                    ("AI_PM_LEASE_TOKEN", run.lease_token.clone()),
                    ("AI_PM_LEASE_GENERATION", run.lease_generation.to_string()),
                    ("AI_PM_WORKER_ID", run.worker_id.clone()),
                ],
                Some(&response),
            );
            match spawn {
                Ok(process) => {
                    let session_created = true;
                    let input_injected = run.run_dir.join(".aipm").join("reply.json").is_file();
                    run.process = process;
                    run.awaiting_continue = false;
                    // 消费回执（N-6 证据组合）：注入文件在 + 进程存活。
                    let _ = tokio::time::sleep(Duration::from_millis(500)).await;
                    let alive = run.process.child.id().is_some();
                    let ack_receipt = json!({
                        "session_created": session_created,
                        "input_injected": input_injected,
                        "session_id": run.process.session_id,
                    });
                    let interaction_id = run.pending_interaction;
                    ack(cfg, client, run, command_id, "acked", ack_receipt, None).await;
                    if input_injected && alive {
                        if let Some(interaction_id) = interaction_id {
                            if let Err(e) = client
                                .report_consumption(
                                    run.project_id,
                                    run.run_id,
                                    &run.lease_token,
                                    run.lease_generation,
                                    interaction_id,
                                )
                                .await
                            {
                                tracing::warn!(error = %e, "consumption receipt rejected (stays unconsumed; N-6)");
                            } else {
                                run.pending_interaction = None;
                            }
                        }
                    }
                }
                Err(e) => {
                    ack(
                        cfg,
                        client,
                        run,
                        command_id,
                        "failed",
                        Value::Null,
                        Some(format!("session creation failed: {e}")),
                    )
                    .await;
                }
            }
        }
        other => tracing::warn!(command = %other, "unknown command type; ignored"),
    }
}

async fn ack(
    cfg: &Config,
    client: &mcp::McpClient,
    run: &ActiveRun,
    command_id: Uuid,
    outcome: &str,
    receipt: Value,
    error: Option<String>,
) {
    let _ = cfg;
    if let Err(e) = client
        .ack_command(run.project_id, command_id, &run.worker_id, outcome, receipt, error)
        .await
    {
        // 回执失败：下一轮 poll 重领（claim 过期或 claimed_by=自方）后重新回执（幂等）。
        tracing::warn!(error = %e, command = %command_id, "ack failed; will re-ack after redelivery");
    }
}
