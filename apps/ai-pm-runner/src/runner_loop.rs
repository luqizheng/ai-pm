//! Runner 自领循环主体（lib 面，F-12：供 bin 与集成测试共用）。
//!
//! 自领循环（客户端化 worker run_executor 的通信面，检查点 2 §3 定稿）：
//! 1. 发现：`pm_list_tasks`（项目内 ready 且有 queued Run 的任务）；
//! 2. 领取：`pm_claim_task`（cpk 凭据通道 → `lease_owner=client:{device_id}`
//!    稳定身份 + execution_mode='spawn'；命令路由 worker_id = 响应
//!    `lease_owner` 快照**原样回带**）；
//! 3. 工作区准备（V-05-2）：`pm_get_run_context` 仓库子集 → git map →
//!    fetch + detach 于 base_commit（用户会话凭据，Runner 不碰凭据）；
//! 4. 监督：spawn Agent（模板库），心跳 `pm_report_progress` 续租，
//!    标记侦测 → `pm_report_interaction`，命令轮询 `pm_poll_commands` /
//!    `pm_ack_command`（terminate 杀进程、continue 写回复重启新 Session →
//!    消费回执 `pm_report_consumption`）；
//! 5. 交付：Agent 自行经 MCP `pm_submit_result`（Model A 既有模式，Runner
//!    不代交交付物）。
//!
//! 检查点 2 补齐（设计稿 §3.4）：I1 凭据 fail-fast（401 即退出，supervisor
//! 重启后保持响亮失败）· I2 租约类错误 → 同主体续领（退避）· I3 心跳节拍
//! 守卫 · I4 指数退避+jitter · I5 多发现项告警 · I7 回执信封 schema v1。

use crate::adapter::{self, ExitOutcome, ResolvedLaunch, SpawnedProcess};
use crate::mcp;
use crate::profiles;
use crate::workspace::{self, ContractRepo, RepoMapping};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// 回执信封 Schema（与平台 `worker_command::RECEIPT_SCHEMA` 对齐；客户端
/// 独立定义——不依赖服务端应用栈）。
pub const RECEIPT_SCHEMA: &str = "aipm.receipt.v1";

#[derive(Debug, Clone)]
pub struct Config {
    pub platform_mcp_url: String,
    pub project_id: Uuid,
    pub client_name: String,
    pub workspace_root: PathBuf,
    /// Adapter 选择（zcode|codex|dsh|custom）。
    pub adapter: String,
    /// raw 命令模板（adapter=custom 必填；其余作 diagnostics）。
    pub agent_cmd: String,
    pub git_map: Option<PathBuf>,
    /// 平台 `AI_PM_RUN_LEASE_SECS` 镜像值（心跳节拍守卫用；须与平台一致）。
    pub lease_secs: u64,
    pub claim_interval_secs: u64,
    pub heartbeat_secs: u64,
    pub detect_interval_secs: u64,
    pub command_poll_secs: u64,
    /// F-6：终止重试预算（次数 × 单次等待超时）。
    pub terminate_attempts: u32,
    pub terminate_timeout_secs: u64,
    /// I4：续领退避（指数基数/上限，jitter ±20% 内置）。
    pub backoff_base_ms: u64,
    pub backoff_cap_ms: u64,
}

impl Config {
    pub fn terminate_attempts_default() -> u32 {
        env_num("AI_PM_RUNNER_TERMINATE_ATTEMPTS", 3) as u32
    }

    pub fn terminate_timeout_secs_default() -> u64 {
        env_num("AI_PM_PROCESS_TERMINATION_TIMEOUT_SECS", 10)
    }

    /// I3：心跳节拍守卫——两拍未及一续租即拒绝启动（防配置踩 INV-1）。
    pub fn validate(&self) -> Result<(), String> {
        if self.heartbeat_secs == 0 || self.heartbeat_secs * 2 >= self.lease_secs {
            return Err(format!(
                "AI_PM_RUNNER_HEARTBEAT_SECS={} must satisfy heartbeat*2 < lease_secs={} \
                 (heartbeat is the lease renewal signal; a beat must always land inside the TTL)",
                self.heartbeat_secs, self.lease_secs
            ));
        }
        Ok(())
    }
}

pub fn env_num(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

pub fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "runner".into())
}

/// I4：指数退避 + ±20% jitter（无随机依赖：以纳秒熵取抖动）。
pub fn backoff_delay_ms(failure_count: u32, base_ms: u64, cap_ms: u64) -> u64 {
    let factor = 1u64 << failure_count.min(5); // base → 32x 上限内封顶
    let raw = base_ms.saturating_mul(factor);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let jitter = (nanos % 41) as i64 - 20; // [-20, +20] 百分比
    let shifted = if jitter >= 0 {
        raw.saturating_add(raw.saturating_mul(jitter as u64) / 100)
    } else {
        raw.saturating_sub(raw.saturating_mul(jitter.unsigned_abs()) / 100)
    };
    shifted.clamp(base_ms, cap_ms)
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
    /// 恢复认领（R-11/§3.6）的 Run 在本机无进程句柄 → None：tick 心跳/命令
    /// 轮询照常，收割/terminate/标记侦测跳过；continue 到达时 spawn 新句柄。
    pub process: Option<SpawnedProcess>,
    pub last_heartbeat: Instant,
    pub last_detect: Instant,
    pub last_command_poll: Instant,
    /// 已上报未消费的 Interaction（responded/consumed 前持有）。
    pub pending_interaction: Option<Uuid>,
    /// terminate 已 acked、等待 continue 命令的窗口。
    pub awaiting_continue: bool,
    /// I2：租约类失败计数与上次续领尝试时刻（退避门）。
    pub lease_failures: u32,
    pub last_lease_attempt: Option<Instant>,
}

/// Recover a pending human-intervention Run after Runner process restart.
/// Exposed as a narrow production seam so lifecycle recovery is integration-tested.
pub async fn recover_inflight_run(cfg: &Config, client: &mcp::McpClient) -> Option<ActiveRun> {
    let discovered = match client.discover_runs(cfg.project_id).await {
        Ok(value) => value,
        Err(e) => {
            tracing::warn!(error = %e, "startup discovery failed; starting without recovery");
            return None;
        }
    };
    let runs = discovered
        .get("runs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if runs.len() > 1 {
        // I5：多发现项显式告警——V1 单 Run 串行，余项待当前 Run 终局后
        // 下轮 discover 再领（幂等、无副作用）。
        tracing::warn!(
            count = runs.len(),
            "multiple recoverable runs discovered; single-run runner takes the first, rest re-discovered later"
        );
    }
    let entry = runs.first()?.clone();
    let run_id: Uuid = entry.get("run_id")?.as_str()?.parse().ok()?;
    let task_id: Uuid = entry.get("task_id")?.as_str()?.parse().ok()?;
    // I4：续领失败退避重试（有界），耗尽后转入正常循环（平台侧恢复路径接管）。
    let mut attempt: u32 = 0;
    let reacquired = loop {
        match client.reacquire_lease(cfg.project_id, run_id).await {
            Ok(value) => break Some(value),
            Err(e) => {
                if e.is_unauthorized() {
                    tracing::error!(error = %e, "credential rejected during recovery reacquire");
                    return None;
                }
                attempt += 1;
                let delay = backoff_delay_ms(attempt, cfg.backoff_base_ms, cfg.backoff_cap_ms);
                tracing::warn!(
                    error = %e, attempt, retry_in_ms = delay,
                    "recovery reacquire failed; backing off"
                );
                if attempt >= 5 {
                    break None;
                }
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
        }
    };
    let Some(reacquired) = reacquired else {
        tracing::error!(run = %run_id, "recovery reacquire exhausted bounded retries; run left to platform recovery paths");
        return None;
    };
    let run_dir = cfg.workspace_root.join(run_id.to_string());
    let _ = std::fs::create_dir_all(&run_dir);
    tracing::info!(
        run = %run_id,
        generation = %reacquired.get("lease_generation").and_then(|v| v.as_i64()).unwrap_or(0),
        rerouted = %reacquired.get("rerouted_commands").and_then(|v| v.as_i64()).unwrap_or(0),
        "recovered in-flight run after offline window (same-owner re-lease)"
    );
    Some(ActiveRun {
        project_id: cfg.project_id,
        task_id,
        run_id,
        lease_token: reacquired
            .get("lease_token")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        lease_generation: reacquired
            .get("lease_generation")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as i32,
        worker_id: reacquired
            .get("worker_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        run_dir,
        stage: "started".into(),
        process: None,
        last_heartbeat: Instant::now(),
        last_detect: Instant::now(),
        last_command_poll: Instant::now() - Duration::from_secs(cfg.command_poll_secs),
        pending_interaction: entry
            .get("interaction_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok()),
        awaiting_continue: true,
        lease_failures: 0,
        last_lease_attempt: None,
    })
}

pub async fn run_loop(cfg: &Config, client: &mcp::McpClient) -> Result<(), mcp::RunnerError> {
    cfg.validate().map_err(mcp::RunnerError::Adapter)?;
    // A replacement Runner must not start a second Agent while a previous
    // instance's child may still be executing. Terminate only when the
    // persisted PID identity anchor still matches; any unconfirmed cleanup
    // fails startup closed before recovery or new Task claims.
    for orphan in adapter::scan_orphans(&cfg.workspace_root) {
        if !orphan.alive {
            continue;
        }
        match adapter::terminate_orphan(&orphan) {
            Ok(()) => tracing::warn!(
                run_dir = %orphan.run_dir.display(),
                pid = orphan.record.pid,
                session = %orphan.record.session_id,
                "terminated identity-matched Agent left by previous Runner instance"
            ),
            Err(error) => {
                if let Err(evidence_error) = write_orphan_cleanup_escalation_evidence(&orphan, &error.to_string()) {
                    tracing::error!(run_dir = %orphan.run_dir.display(), error = %evidence_error, "failed to persist orphan cleanup escalation evidence");
                }
                return Err(mcp::RunnerError::Adapter(format!(
                    "startup blocked: orphan Agent cleanup could not be confirmed for pid {}: {}",
                    orphan.record.pid, error
                )));
            }
        }
    }
    // I1：启动自检——轻量调用验证凭据；401/403 即退出（不进入 401 循环）。
    if let Err(e) = client.list_ready_tasks(cfg.project_id).await {
        if e.is_unauthorized() {
            tracing::error!(error = %e, "credential self-check failed; exiting (supervisor will restart after human action)");
            return Err(e);
        }
        tracing::warn!(error = %e, "startup probe failed (non-credential); continuing");
    }

    let mut active: Option<ActiveRun> = recover_inflight_run(cfg, client).await?;
    let mut logged_awaiting = false;
    let mut lease_lost_cleanup_pending = false;
    let mut last_claim_attempt = Instant::now() - Duration::from_secs(cfg.claim_interval_secs);

    loop {
        // A lease-loss cleanup state blocks heartbeat/commands and all new claims.
        // Keep retrying termination (or lease reacquisition for a recovered Run)
        // until the old execution is safely stopped or supervision is restored.
        if lease_lost_cleanup_pending {
            let has_local_child = active.as_ref().and_then(|run| run.process.as_ref()).is_some();
            let cleanup_result = match active.as_mut() {
                Some(run) if has_local_child => match run.process.as_mut() {
                    Some(process) => adapter::terminate_spawned(process)
                        .await
                        .map(|_| ())
                        .map_err(|error| mcp::RunnerError::Adapter(error.to_string())),
                    None => Ok(()),
                },
                Some(run) => reacquire_active_run(cfg, client, run).await.map(|_| ()),
                None => Ok(()),
            };
            match cleanup_result {
                Ok(()) => {
                    if has_local_child {
                        if let Some(run) = active.as_ref() {
                            let _ = std::fs::remove_file(run.run_dir.join(".aipm").join("cleanup-needs-human.json"));
                        }
                        active = None;
                    } else if let Some(run) = active.as_ref() {
                        let _ = std::fs::remove_file(run.run_dir.join(".aipm").join("cleanup-needs-human.json"));
                    }
                    lease_lost_cleanup_pending = false;
                }
                Err(error) => {
                    if let Some(run) = active.as_ref() {
                        if let Err(evidence_error) = write_cleanup_escalation_evidence(run, &error.to_string()) {
                            tracing::error!(run = %run.run_id, error = %evidence_error, "failed to persist Runner cleanup escalation evidence");
                        }
                        tracing::error!(run = %run.run_id, error = %error, "lease-loss cleanup remains unresolved; retaining supervision slot and blocking new claims");
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
            }
            if active.is_none() {
                continue;
            }
        }

        // ① 进程收割与退出判定。
        if let Some(run) = active.as_mut() {
            let outcome = match run.process.as_mut() {
                Some(spawned) => adapter::reap(spawned).await,
                // 恢复认领的 Run（无本地进程句柄）：无收割语义。
                None => None,
            };
            if let Some(outcome) = outcome {
                if run.awaiting_continue {
                    // 人工介入流：进程是 Runner 自己按 terminate 命令杀的
                    //（D-03）——预期退出，Run 保持存活等待 continue 命令。
                    // 误判为退出会丢弃 Run → continue 永远无人消费。
                    if !logged_awaiting {
                        logged_awaiting = true;
                        tracing::info!(
                            run = %run.run_id, exit = outcome.as_str(),
                            "agent terminated for intervention; awaiting continue"
                        );
                    }
                } else {
                    tracing::info!(run = %run.run_id, exit = outcome.as_str(), "agent process exited");
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
                // I1：凭据拒绝即终止（fail-fast；supervisor 重启后仍响亮失败）。
                Err(e) if e.is_unauthorized() => {
                    tracing::error!(error = %e, "credential rejected during claim; exiting");
                    return Err(e);
                }
                Err(e) => tracing::warn!(error = %e, "claim attempt failed"),
            }
        }

        // ③ 活跃 Run 的通信拍：心跳 / 标记侦测 / 命令轮询。
        if let Some(run) = active.as_mut() {
            if let Err(e) = tick(cfg, client, run).await {
                // I2：租约类拒绝 → 同主体续领（退避；R-16 方案 a 真进程落位）。
                if e.is_lease_error() {
                    match reacquire_active_run(cfg, client, run).await {
                        Ok(()) => tracing::info!(
                            run = %run.run_id,
                            generation = run.lease_generation,
                            "lease reacquired mid-run (lease-class rejection recovered)"
                        ),
                        Err(reacquire_error) => {
                            tracing::warn!(
                                error = %reacquire_error,
                                run = %run.run_id,
                                "lease reacquire failed; terminating and reaping local Agent before releasing supervision"
                            );
                            lease_lost_cleanup_pending = true;
                            if let Err(evidence_error) = write_cleanup_escalation_evidence(run, &reacquire_error.to_string()) {
                                tracing::error!(run = %run.run_id, error = %evidence_error, "failed to persist Runner cleanup escalation evidence");
                            }
                            tracing::error!(run = %run.run_id, "lease reacquisition failed; entering fail-closed cleanup state and blocking new claims");
                            continue;
                        }
                    }
                } else if e.is_unauthorized() {
                    tracing::error!(error = %e, "credential rejected during supervision; exiting");
                    return Err(e);
                } else {
                    tracing::warn!(error = %e, run = %run.run_id, "supervision tick failed");
                }
            }
            // 收割竞态兜底：进程句柄已失效。awaiting_continue 期间的
            // None 是 terminate 的预期结果（§4.1 修复：不得在此丢弃 Run，
            // 否则 continue 命令永远无人消费——E2E 实测踩中）。
            let process_dead = match run.process.as_ref() {
                None => true,
                Some(spawned) => spawned.child.id().is_none(),
            };
            if process_dead && !run.awaiting_continue {
                active = None;
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Persist startup cleanup evidence for an orphan that could not be safely reaped.
fn write_orphan_cleanup_escalation_evidence(
    orphan: &adapter::OrphanReport,
    error: &str,
) -> Result<(), std::io::Error> {
    let directory = orphan.run_dir.join(".aipm");
    std::fs::create_dir_all(&directory)?;
    let run_id = orphan.run_dir.file_name().and_then(|name| name.to_str());
    let evidence = json!({
        "schema_version": "aipm.runner-cleanup-escalation.v1",
        "run_id": run_id,
        "pid": orphan.record.pid,
        "session_id": orphan.record.session_id,
        "identity_hint": orphan.record.identity_hint,
        "reason": "previous_runner_orphan_cleanup_unconfirmed",
        "error": error,
        "recorded_at": chrono::Utc::now().to_rfc3339(),
    });
    let bytes = serde_json::to_vec_pretty(&evidence)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    std::fs::write(directory.join("cleanup-needs-human.json"), bytes)
}
/// Persist local evidence when a child cannot be confirmed terminated. Credentials are excluded.
fn write_cleanup_escalation_evidence(run: &ActiveRun, error: &str) -> Result<(), std::io::Error> {
    let directory = run.run_dir.join(".aipm");
    std::fs::create_dir_all(&directory)?;
    let process = run.process.as_ref();
    let evidence = json!({
        "schema_version": "aipm.runner-cleanup-escalation.v1",
        "project_id": run.project_id,
        "task_id": run.task_id,
        "run_id": run.run_id,
        "lease_generation": run.lease_generation,
        "pid": process.map(|p| p.pid),
        "session_id": process.map(|p| p.session_id.as_str()),
        "reason": "lease_reacquisition_failed_and_termination_unconfirmed",
        "error": error,
        "recorded_at": chrono::Utc::now().to_rfc3339(),
    });
    let path = directory.join("cleanup-needs-human.json");
    let bytes = serde_json::to_vec_pretty(&evidence)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    std::fs::write(path, bytes)
}
/// I2：租约类失败后的同主体续领（带退避门；成功刷新 ActiveRun 三要素）。
async fn reacquire_active_run(
    cfg: &Config,
    client: &mcp::McpClient,
    run: &mut ActiveRun,
) -> Result<(), mcp::RunnerError> {
    if let Some(last) = run.last_lease_attempt {
        let delay = backoff_delay_ms(run.lease_failures, cfg.backoff_base_ms, cfg.backoff_cap_ms);
        if last.elapsed() < Duration::from_millis(delay) {
            return Err(mcp::RunnerError::Adapter(format!(
                "reacquire backing off for {}ms",
                delay
            )));
        }
    }
    run.last_lease_attempt = Some(Instant::now());
    run.lease_failures += 1;
    let reacquired = client.reacquire_lease(run.project_id, run.run_id).await?;
    run.lease_token = reacquired
        .get("lease_token")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    run.lease_generation = reacquired
        .get("lease_generation")
        .and_then(|v| v.as_i64())
        .unwrap_or(run.lease_generation as i64) as i32;
    if let Some(worker_id) = reacquired.get("worker_id").and_then(|v| v.as_str()) {
        run.worker_id = worker_id.to_string();
    }
    run.lease_failures = 0;
    Ok(())
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
    // 命令路由身份 = Platform 侧 lease_owner 快照（cpk 通道 = `client:{device_id}`，
    // 真人通道 = `mcp:{client}:{token8}`）——**必须原样使用**（poll/ack 路由按此绑定）；
    // 旧平台无该字段时回退本地重建。
    let worker_id = claimed
        .get("lease_owner")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "mcp:{}:{}",
                cfg.client_name,
                &lease_token[..8.min(lease_token.len())]
            )
        });
    let run_dir = cfg.workspace_root.join(run_id.to_string());
    std::fs::create_dir_all(&run_dir)?;

    // 工作区准备（V-05-2）：契约仓库子集 → git map → fetch + detach。
    // 失败 = Run 走 blocked 上报（平台可见），本槽位释放。
    let contract = client
        .get_run_context(cfg.project_id, run_id, &lease_token, lease_generation)
        .await?;
    let repos = workspace::contract_repos(&contract);
    let git_map: HashMap<String, RepoMapping> =
        workspace::load_git_map(cfg.git_map.as_deref()).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "git map unusable; workspace prep will fail for mapped repos");
            HashMap::new()
        });
    let mut prepared: Vec<(ContractRepo, PathBuf)> = Vec::new();
    for repo in &repos {
        let mapping = git_map
            .get(&repo.repository_id)
            .or_else(|| git_map.get(&repo.name));
        let fallback = run_dir.join(&repo.workspace_path);
        match workspace::prepare_repo(
            mapping,
            &fallback,
            &repo.base_commit,
            Duration::from_secs(120),
        )
        .await
        {
            Ok(path) => prepared.push((repo.clone(), path)),
            Err(e) => {
                // 工作区准备失败：blocked 上报（best-effort；平台可见）+ 释放槽位。
                tracing::error!(run = %run_id, repo = %repo.name, error = %e, "workspace prep failed; run reported blocked");
                let _ = client
                    .report_progress(
                        cfg.project_id,
                        run_id,
                        &lease_token,
                        lease_generation,
                        "started",
                        &format!("workspace prep failed (blocked): {e}"),
                    )
                    .await;
                return Ok(None);
            }
        }
    }
    let note = workspace::workspace_note(&prepared);
    let _ = std::fs::create_dir_all(run_dir.join(".aipm"));
    let _ = std::fs::write(
        run_dir.join(".aipm").join("workspace.json"),
        serde_json::to_vec_pretty(
            &workspace::workspace_env(&prepared)
                .map(|(_, value)| value)
                .unwrap_or_else(|| "[]".into()),
        )
        .unwrap_or_default(),
    );

    let process = spawn_agent_session(
        cfg,
        &run_dir,
        task_id,
        run_id,
        &lease_token,
        lease_generation,
        None,
        Some(note),
    )
    .await?;
    Ok(Some(ActiveRun {
        project_id: cfg.project_id,
        task_id,
        run_id,
        lease_token,
        lease_generation,
        worker_id,
        run_dir,
        stage: "started".into(),
        process: Some(process),
        last_heartbeat: Instant::now(),
        last_detect: Instant::now(),
        last_command_poll: Instant::now() - Duration::from_secs(cfg.command_poll_secs),
        pending_interaction: None,
        awaiting_continue: false,
        lease_failures: 0,
        last_lease_attempt: None,
    }))
}

/// 统一 spawn 入口（claim 与 continue 共用）：模板解析 → 提示文件 → spawn
/// （密钥脱敏表 = lease_token；工作区地图经 env 注入）。
#[allow(clippy::too_many_arguments)]
async fn spawn_agent_session(
    cfg: &Config,
    run_dir: &Path,
    task_id: Uuid,
    run_id: Uuid,
    lease_token: &str,
    lease_generation: i32,
    reply: Option<&Value>,
    workspace_note: Option<String>,
) -> Result<SpawnedProcess, mcp::RunnerError> {
    let prompt = profiles::build_prompt(
        &cfg.project_id.to_string(),
        &task_id.to_string(),
        &run_id.to_string(),
        lease_generation,
        reply,
        workspace_note.as_deref(),
    );
    let prompt_path = profiles::write_prompt_file(run_dir, &prompt)
        .map_err(|e| mcp::RunnerError::Adapter(format!("prompt file write failed: {e}")))?;
    let launch: ResolvedLaunch =
        profiles::resolve(&cfg.adapter, Some(&cfg.agent_cmd), &prompt, &prompt_path)
            .map_err(|e| mcp::RunnerError::Adapter(e.to_string()))?;
    let workspace_env = workspace_note.map(|note| ("AI_PM_WORKSPACE_NOTE", note));
    let mut envs: Vec<(&str, String)> = vec![
        ("AI_PM_RUN_ID", run_id.to_string()),
        ("AI_PM_TASK_ID", task_id.to_string()),
        ("AI_PM_PROJECT_ID", cfg.project_id.to_string()),
        ("AI_PM_MCP_URL", cfg.platform_mcp_url.clone()),
        ("AI_PM_LEASE_TOKEN", lease_token.to_string()),
        ("AI_PM_LEASE_GENERATION", lease_generation.to_string()),
    ];
    if let Some((key, value)) = workspace_env {
        envs.push((key, value));
    }
    adapter::spawn_resolved(&launch, run_dir, &envs, reply, &[lease_token.to_string()])
        .map_err(|e| mcp::RunnerError::Adapter(e.to_string()))
}

/// 活跃 Run 的一个通信拍（心跳 / 标记侦测 / 命令执行）。
pub async fn tick(
    cfg: &Config,
    client: &mcp::McpClient,
    run: &mut ActiveRun,
) -> Result<(), mcp::RunnerError> {
    // ① 心跳 = 续租（stage 已推进过 started，重复上报仅续租）。
    if run.last_heartbeat.elapsed() >= Duration::from_secs(cfg.heartbeat_secs) {
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
        // Advance the heartbeat clock only after Platform confirms renewal.
        // A transport/HTTP failure must be retried on the next supervision tick.
        run.last_heartbeat = Instant::now();
    }

    // ② 标记文件侦测（N-2 ①→②：`interaction.{event_id}.json` → pm_report_interaction）。
    if run.pending_interaction.is_none()
        && !run.awaiting_continue
        && run.process.is_some()
        && run.last_detect.elapsed() >= Duration::from_secs(cfg.detect_interval_secs)
    {
        run.last_detect = Instant::now();
        for (path, parsed) in adapter::find_unreported_markers(&run.run_dir) {
            let event = match parsed {
                Ok(event) => event,
                // 非法标记：隔离不重试、不产生 Interaction（§1.2 / 评审 P1-4）。
                Err(e) => {
                    if let Err(io_err) = adapter::quarantine_marker(&path) {
                        tracing::warn!(error = %io_err, "marker quarantine failed");
                    }
                    tracing::warn!(error = %e, "invalid interaction marker quarantined");
                    continue;
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
                            return Err(e);
                        }
                    }
                }
            }
        }
    }

    // ③ 命令轮询与执行（terminate / continue）。
    if run.last_command_poll.elapsed() >= Duration::from_secs(cfg.command_poll_secs) {
        let commands = client
            .poll_commands(run.project_id, run.run_id, &run.worker_id)
            .await?;
        // A transport/HTTP failure must leave polling due so the next
        // supervision tick retries immediately rather than waiting a full interval.
        run.last_command_poll = Instant::now();
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
    if obj
        .get("continuation_start_frozen")
        .and_then(|v| v.as_bool())
        == Some(true)
    {
        return Err("continuation start frozen (previous termination unconfirmed; ACP-444)".into());
    }
    if let Some(deadline) = obj.get("agent_deadline_at").and_then(|v| v.as_str()) {
        let deadline = chrono::DateTime::parse_from_rfc3339(deadline)
            .map_err(|e| format!("malformed guard.agent_deadline_at: {e}"))?;
        if chrono::Utc::now() > deadline.with_timezone(&chrono::Utc) + chrono::Duration::seconds(5)
        {
            return Err(
                "agent deadline expired; requires explicit re-authorization (V1.3.1 §9.2)".into(),
            );
        }
    }
    Ok(())
}

/// I7：回执信封（schema v1 + 当前代次；与平台 `RECEIPT_SCHEMA` 对齐）。
pub fn receipt_v1(lease_generation: i32, extra: Value) -> Value {
    let mut receipt = json!({
        "schema_version": RECEIPT_SCHEMA,
        "run_generation": lease_generation,
    });
    let obj = receipt.as_object_mut().expect("receipt object");
    if let Value::Object(fields) = extra {
        for (key, value) in fields {
            obj.insert(key, value);
        }
    }
    receipt
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
            if let Some(job) = run.process.as_ref().and_then(|p| p.job.as_ref()) {
                job.terminate();
            }
            let outcome = match run.process.as_mut() {
                Some(spawned) => {
                    adapter::terminate_with_retry(
                        &mut spawned.child,
                        cfg.terminate_attempts,
                        cfg.terminate_timeout_secs,
                    )
                    .await
                }
                // R-11/ACP-C4：恢复认领的 Run 本机无进程句柄——旧进程已不存在，
                // 幂等终止确认（专家第 1 块 3.4：支持幂等终止确认）。
                None => Ok(ExitOutcome::Lost),
            };
            let pid_receipt = run
                .process
                .as_ref()
                .map(|p| json!(p.pid))
                .unwrap_or(json!(null));
            // I7：请求/确认两时刻（请求 = 平台 payload.issued_at 事实；确认 = 本地现刻）。
            let requested_at = command
                .get("payload")
                .and_then(|p| p.get("issued_at"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
            let (outcome_word, receipt, error) = match outcome {
                Ok(exit_outcome) => {
                    tracing::info!(run = %run.run_id, exit = exit_outcome.as_str(), "agent terminated");
                    (
                        "acked",
                        receipt_v1(
                            run.lease_generation,
                            json!({
                                "exit_outcome": exit_outcome.as_str(),
                                "pid": pid_receipt,
                                "terminate_requested_at": requested_at,
                                "terminate_confirmed_at": chrono::Utc::now().to_rfc3339(),
                            }),
                        ),
                        None,
                    )
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
            // Platform may redeliver the same command when its first ack was delayed
            // or lost. If this Runner already has a live continuation process, do not
            // spawn a second Agent session; acknowledge the duplicate against the
            // existing session instead.
            if !run.awaiting_continue && run.process.is_some() {
                let session_id = run
                    .process
                    .as_ref()
                    .map(|process| process.session_id.clone())
                    .unwrap_or_default();
                ack(
                    client,
                    run,
                    command_id,
                    "acked",
                    receipt_v1(
                        run.lease_generation,
                        json!({
                            "duplicate": true,
                            "session_created": false,
                            "session_id": session_id,
                        }),
                    ),
                    None,
                )
                .await;
                return;
            }
            // D-03 时序：terminate 已确认后才可能收到 continue（Platform 情形 6 过滤）。
            // F-1：启动前置校验（guard 快照）——拒绝时命令 failed，不 spawn.
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
            match spawn_agent_session(
                cfg,
                &run.run_dir,
                run.task_id,
                run.run_id,
                &run.lease_token,
                run.lease_generation,
                Some(&response),
                None,
            )
            .await
            {
                Ok(process) => {
                    let session_created = true;
                    let input_injected = run.run_dir.join(".aipm").join("reply.json").is_file();
                    run.process = Some(process);
                    run.awaiting_continue = false;
                    // 消费回执（N-6 证据组合）：注入文件在 + 进程存活。
                    let _ = tokio::time::sleep(Duration::from_millis(500)).await;
                    let alive = run
                        .process
                        .as_ref()
                        .map(|p| p.child.id().is_some())
                        .unwrap_or(false);
                    let session_id = run
                        .process
                        .as_ref()
                        .map(|p| p.session_id.clone())
                        .unwrap_or_default();
                    let ack_receipt = receipt_v1(
                        run.lease_generation,
                        json!({
                            "session_created": session_created,
                            "input_injected": input_injected,
                            "session_id": session_id,
                        }),
                    );
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
        .ack_command(
            run.project_id,
            command_id,
            &run.worker_id,
            outcome,
            receipt,
            error,
        )
        .await
    {
        // 回执失败：下一轮 poll 重领（claim 过期或 claimed_by=自方）后重新回执（幂等）。
        tracing::warn!(error = %e, command = %command_id, "ack failed; will re-ack after redelivery");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_guard_rejects_bad_ratios() {
        let mut cfg = test_config();
        cfg.heartbeat_secs = 200; // 2*200 >= 300
        assert!(cfg.validate().is_err());
        cfg.heartbeat_secs = 0;
        assert!(cfg.validate().is_err());
        cfg.heartbeat_secs = 60; // 120 < 300
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn backoff_grows_and_clamps() {
        let base = 2000;
        let cap = 60_000;
        let d0 = backoff_delay_ms(0, base, cap); // factor 1，±20% jitter
        assert!((1600..=2400).contains(&d0), "{d0}");
        let d3 = backoff_delay_ms(3, base, cap);
        assert!(d3 >= base, "{d3}");
        assert!(
            backoff_delay_ms(9, base, cap) >= 51_200,
            "32x jittered must reach the cap zone"
        );
    }

    #[test]
    fn receipt_envelope_carries_schema_and_generation() {
        let receipt = receipt_v1(
            2,
            json!({ "exit_outcome": "Killed", "terminate_confirmed_at": "t" }),
        );
        assert_eq!(receipt["schema_version"], RECEIPT_SCHEMA);
        assert_eq!(receipt["run_generation"], 2);
        assert_eq!(receipt["exit_outcome"], "Killed");
    }

    fn test_config() -> Config {
        Config {
            platform_mcp_url: "http://127.0.0.1:8090/mcp".into(),
            project_id: Uuid::nil(),
            client_name: "runner-59".into(),
            workspace_root: std::env::temp_dir(),
            adapter: "custom".into(),
            agent_cmd: "cmd".into(),
            git_map: None,
            lease_secs: 300,
            claim_interval_secs: 10,
            heartbeat_secs: 60,
            detect_interval_secs: 5,
            command_poll_secs: 3,
            terminate_attempts: 3,
            terminate_timeout_secs: 10,
            backoff_base_ms: 2000,
            backoff_cap_ms: 60_000,
        }
    }
}
