//! Runner 自领循环主体（lib 面，F-12：供 bin 与集成测试共用）。
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
//! F-1（复审 2026-11-07）：continue 执行前执行 §9.4 前置校验——前置事实由
//! Platform 随命令 payload 下发（`guard` 快照，见 worker_command.rs
//! issue_continue_command）：`continuation_start_frozen`（terminate 失败冻结，
//! ACP-444）与 `agent_deadline_at`（D-02 不顺延，过期须显式授权）。拒绝 =
//! 命令 failed，回复保留幂等重投/转授权路径。

use crate::adapter::{self, SpawnedProcess};
use crate::mcp;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Config {
    pub platform_mcp_url: String,
    pub project_id: Uuid,
    pub client_name: String,
    pub workspace_root: PathBuf,
    pub agent_cmd: String,
    pub claim_interval_secs: u64,
    pub heartbeat_secs: u64,
    pub detect_interval_secs: u64,
    pub command_poll_secs: u64,
    /// F-6：终止重试预算（次数 × 单次等待超时）。
    pub terminate_attempts: u32,
    pub terminate_timeout_secs: u64,
}

impl Config {
    pub fn terminate_attempts_default() -> u32 {
        env_num("AI_PM_RUNNER_TERMINATE_ATTEMPTS", 3) as u32
    }

    pub fn terminate_timeout_secs_default() -> u64 {
        env_num("AI_PM_PROCESS_TERMINATION_TIMEOUT_SECS", 10)
    }
}

pub fn env_num(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

pub fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "runner".into())
}

/// 一个受监督 Run 的运行时状态（字段 pub 供测试构造）。
pub struct ActiveRun {
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub lease_token: String,
    pub lease_generation: i32,
    pub worker_id: String,
    pub run_dir: PathBuf,
    pub stage: String,
    pub process: SpawnedProcess,
    pub last_heartbeat: Instant,
    pub last_detect: Instant,
    pub last_command_poll: Instant,
    /// 已上报未消费的 Interaction（responded/consumed 前持有）。
    pub pending_interaction: Option<Uuid>,
    /// terminate 已 acked、等待 continue 命令的窗口。
    pub awaiting_continue: bool,
}

pub async fn run_loop(cfg: &Config, client: &mcp::McpClient) {
    let mut active: Option<ActiveRun> = None;
    let mut last_claim_attempt = Instant::now() - Duration::from_secs(cfg.claim_interval_secs);
    loop {
        // ① 进程收割与退出判定。
        if let Some(run) = active.as_mut() {
            if let Some(outcome) = adapter::reap(&mut run.process).await {
                if run.awaiting_continue {
                    // 人工介入流：进程是 Runner 自己按 terminate 命令杀的
                    //（D-03）——预期退出，Run 保持存活等待 continue 命令。
                    // 误判为退出会丢弃 Run → continue 永远无人消费。
                    tracing::info!(
                        run = %run.run_id, exit = %outcome,
                        "agent terminated for intervention; awaiting continue"
                    );
                } else {
                    tracing::info!(run = %run.run_id, exit = %outcome, "agent process exited");
                    // 交付提交由 Agent 经 MCP 自行完成（Model A 既有模式）；
                    // Runner 侧仅记录退出证据。
                    active = None;
                }
            }
        }

        // ② 无活跃 Run → 按间隔自领。
        if active.is_none()
            && last_claim_attempt.elapsed() >= Duration::from_secs(cfg.claim_interval_secs)
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
            // 收割竞态兜底：进程句柄已失效。awaiting_continue 期间的
            // None 是 terminate 的预期结果（§4.1 修复：不得在此丢弃 Run，
            // 否则 continue 命令永远无人消费——E2E 实测踩中）。
            if run.process.child.id().is_none() && !run.awaiting_continue {
                active = None;
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

pub async fn claim_and_start(
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
pub async fn tick(
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
                // 非法标记：隔离不重试、不产生 Interaction（§1.2 / 评审 P1-4）。
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
                    // F-9 错误分类：Tool 类 = Platform 业务拒绝（链冲突/运行态
                    // 校验/not found）→ 永久，按 §1.2 归档 `.rejected` 不重试
                    //（链关闭后重报旧事件会产生重复介入）；传输/IO 类 → 保留
                    // 原文件下轮重试（event_id 幂等，ACP-430）。
                    match &e {
                        mcp::RunnerError::Tool(_) => {
                            match adapter::reject_marker(&path, &event.event_id) {
                                Ok(_) => tracing::warn!(
                                    error = %e, event = %event.event_id,
                                    "interaction rejected by platform; marker archived as .rejected"
                                ),
                                Err(io_err) => tracing::warn!(
                                    error = %io_err, "rejected marker archive failed; will re-report idempotently"
                                ),
                            }
                        }
                        _ => {
                            tracing::warn!(error = %e, "interaction report failed; will retry");
                        }
                    }
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

/// F-1：continue 启动前置校验（§9.4 子集，事实来自命令 payload 的 guard 快照）。
/// guard 缺失（旧版本 Platform）→ 放行 + 由调用方告警；frozen 显式 true 或
/// Deadline 已过（5s 钟偏容忍）→ 拒绝（ACP-444 / V1.3.1 §9.2）。
pub fn preflight_continue(guard: &Value) -> Result<(), String> {
    let Some(obj) = guard.as_object() else {
        return Ok(());
    };
    if obj.get("continuation_start_frozen").and_then(|v| v.as_bool()) == Some(true) {
        return Err(
            "continuation start frozen (previous termination unconfirmed; ACP-444)".into(),
        );
    }
    if let Some(deadline) = obj.get("agent_deadline_at").and_then(|v| v.as_str()) {
        let deadline = chrono::DateTime::parse_from_rfc3339(deadline)
            .map_err(|e| format!("malformed guard.agent_deadline_at: {e}"))?;
        if chrono::Utc::now() > deadline.with_timezone(&chrono::Utc) + chrono::Duration::seconds(5) {
            return Err(
                "agent deadline expired; requires explicit re-authorization (V1.3.1 §9.2)"
                    .into(),
            );
        }
    }
    Ok(())
}

pub async fn execute_command(
    cfg: &Config,
    client: &mcp::McpClient,
    run: &mut ActiveRun,
    command: Value,
) {
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
            // F-6：有界重试（§6.4/§11.4）——kill 后限时等待，超时重试，耗尽才
            // 回执 failed（Platform 侧冻结启动 + 阻塞路径接手）。
            // ACP-08：Windows 先 Job Object 整树终止，再走有界重试收割。
            #[cfg(windows)]
            if let Some(job) = run.process.job.as_ref() {
                job.terminate();
            }
            let outcome = adapter::terminate_with_retry(
                &mut run.process.child,
                cfg.terminate_attempts,
                cfg.terminate_timeout_secs,
            )
            .await;
            let (outcome_word, receipt, error) = match outcome {
                Ok(exit_outcome) => {
                    tracing::info!(run = %run.run_id, exit = %exit_outcome, "agent terminated");
                    ("acked", json!({ "exit_outcome": exit_outcome, "pid": run.process.pid }), None)
                }
                Err(e) => (
                    // 平台对 terminate acked 要求 exit_outcome（§3.2 从严判据）：
                    // 预算耗尽按 failed 回执，进入 Platform 侧冻结/阻塞路径。
                    "failed",
                    Value::Null,
                    Some(format!("termination failed after retries: {e}")),
                ),
            };
            run.awaiting_continue = true;
            ack(client, run, command_id, outcome_word, receipt, error).await;
        }
        "agent.task.continue" => {
            // D-03 时序：terminate 已确认后才可能收到 continue（Platform 情形 6 过滤）。
            // F-1：启动前置校验（guard 快照）——拒绝时命令 failed，不 spawn。
            let guard = command
                .get("payload")
                .and_then(|p| p.get("guard"))
                .cloned()
                .unwrap_or(Value::Null);
            if let Err(reason) = preflight_continue(&guard) {
                tracing::warn!(run = %run.run_id, reason = %reason, "continue preflight refused startup");
                ack(
                    client,
                    run,
                    command_id,
                    "failed",
                    json!({ "preflight": reason }),
                    Some(format!("startup_preflight_failed: {reason}")),
                )
                .await;
                return;
            }
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
                    ack(client, run, command_id, "acked", ack_receipt, None).await;
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
    client: &mcp::McpClient,
    run: &ActiveRun,
    command_id: Uuid,
    outcome: &str,
    receipt: Value,
    error: Option<String>,
) {
    if let Err(e) = client
        .ack_command(run.project_id, command_id, &run.worker_id, outcome, receipt, error)
        .await
    {
        // 回执失败：下一轮 poll 重领（claim 过期或 claimed_by=自方）后重新回执（幂等）。
        tracing::warn!(error = %e, command = %command_id, "ack failed; will re-ack after redelivery");
    }
}
