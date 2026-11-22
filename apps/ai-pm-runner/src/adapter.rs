//! Runner ↔ Agent Adapter（V1.1 §1.2 / §7.3 + 检查点 2 R1 补齐）：
//! 本地进程 spawn/收割/终止（Job Object / 进程组 / ExitOutcome 五值）+
//! `.aipm/interaction.{event_id}.json` 标记文件侦测与归档（P2-1）+ 回复注入 +
//! Run 级证据面（launch 摘要 / 脱敏日志管道 / 原子 pid 文件 + identity_hint）。
//!
//! 标记文件非权威存储（Platform 的 task_interaction 才是），Agent 只创建新
//! 事件不改写已提交事件。进程硬化继承 `codex_executor.rs` 先例（§4.3 对照表）。

use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Child;

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

/// ExitOutcome 五值语义（检查点 2 I10：对齐平台权威枚举
/// `pm-application::run_dispatch::port::ExitOutcome`；Runner 独立定义——
/// 客户端不依赖服务端应用栈）。回执以 `as_str()` 序列化。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitOutcome {
    /// 进程正常退出且退出码为 0。
    Success,
    /// 进程退出且退出码非 0。
    NonZeroExit(i32),
    /// 进程被信号终止（Unix；Windows 无此语义）。
    Signaled(i32),
    /// 执行端主动终止（terminate 命令 / 终止预算路径）。
    Killed,
    /// 孤儿跨重启后退出码不可归因（pid 证据消失 / 进程不存在）。
    Lost,
}

impl ExitOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExitOutcome::Success => "Success",
            ExitOutcome::NonZeroExit(_) => "NonZeroExit",
            ExitOutcome::Signaled(_) => "Signaled",
            ExitOutcome::Killed => "Killed",
            ExitOutcome::Lost => "Lost",
        }
    }

    /// 自然退出（收割路径）的 ExitOutcome 判定。
    fn from_exit_status(status: &std::process::ExitStatus) -> ExitOutcome {
        if status.success() {
            return ExitOutcome::Success;
        }
        if let Some(code) = status.code() {
            return ExitOutcome::NonZeroExit(code);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(signal) = status.signal() {
                return ExitOutcome::Signaled(signal);
            }
        }
        ExitOutcome::Lost
    }
}

/// 标记文件事件（schema `aipm.interaction.v1`，§1.2 全表）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct MarkerEvent {
    pub event_id: String,
    pub reason: String,
    pub title: String,
    pub question: String,
    pub options: Option<Value>,
    pub required_response: bool,
    pub checkpoint_path: Option<String>,
}

/// 一次 spawn 的进程（pid/session 落 `{run_dir}/.aipm/process.json` 原子文件 +
/// identity_hint 锚点，I9；Windows 另持 Job Object 句柄整树终止）。
pub struct SpawnedProcess {
    pub child: Child,
    pub pid: u32,
    pub session_id: String,
    /// 子进程被终止时的预期语义（Killed）；自然收割走 [`reap`]。
    #[cfg(windows)]
    pub job: Option<job_object::JobHandle>,
}

const MARKER_SCHEMA: &str = "aipm.interaction.v1";
/// 单个 Run 级日志文件上限（超限静默丢写，防磁盘耗尽——codex_executor §6.3 先例）。
const LOG_CAP_BYTES: u64 = 10 * 1024 * 1024;
const REDACTED: &str = "[REDACTED]";

/// 一次 spawn 的解析结果（profiles 模板库产物；raw 模板路径等价构造）。
#[derive(Debug, Clone)]
pub struct ResolvedLaunch {
    pub program: String,
    pub args: Vec<String>,
    /// stdin 流式注入正文（DSH 形态；None = 启动参数/文件注入形态）。
    pub stdin_prompt: Option<String>,
}

/// spawn Agent 进程（raw 命令模板路径：不经 shell、空白切分；测试与
/// `AI_PM_RUNNER_AGENT_CMD` 自定义兜底共用）。
pub fn spawn_agent(
    agent_cmd: &str,
    run_dir: &Path,
    envs: &[(&str, String)],
    reply: Option<&Value>,
) -> Result<SpawnedProcess, AdapterError> {
    let mut parts = agent_cmd.split_whitespace();
    let Some(program) = parts.next() else {
        return Err(AdapterError::Invalid(
            "agent command template is empty".into(),
        ));
    };
    let launch = ResolvedLaunch {
        program: program.to_string(),
        args: parts.map(str::to_string).collect(),
        stdin_prompt: None,
    };
    spawn_resolved(&launch, run_dir, envs, reply, &[])
}

/// spawn Agent 进程（完整路径：解析后的启动配置 + 密钥脱敏表）。
/// 环境注入对齐 `codex_executor.rs` 惯例 + 回复注入（Continuation 初始输入）；
/// stdout/stderr 经脱敏管道落 `{run_dir}/logs/`（I4'）；spawn 摘要落
/// `.aipm/launch.json`（I12）；pid 文件原子写 + identity_hint（I9）。
pub fn spawn_resolved(
    launch: &ResolvedLaunch,
    run_dir: &Path,
    envs: &[(&str, String)],
    reply: Option<&Value>,
    secrets: &[String],
) -> Result<SpawnedProcess, AdapterError> {
    std::fs::create_dir_all(run_dir.join("logs"))?;
    let mut cmd = tokio::process::Command::new(&launch.program);
    cmd.args(&launch.args)
        .current_dir(run_dir)
        .env("AI_PM_WORKSPACE", run_dir)
        .env("AI_PM_AIPM_DIR", run_dir.join(".aipm"));
    for (key, value) in envs {
        cmd.env(key, value);
    }
    let stdin_inject = launch.stdin_prompt.is_some();
    if stdin_inject {
        cmd.stdin(std::process::Stdio::piped());
    } else {
        cmd.stdin(std::process::Stdio::null());
    }
    if let Some(reply) = reply {
        // Continuation 初始输入注入（V1：env + `.aipm/reply.json` 落盘双通道；
        // 注入成功 = INPUT_INJECTED 判据的文件证据，Platform 端不代写）。
        write_reply(run_dir, reply)?;
        cmd.env(
            "AI_PM_HUMAN_REPLY",
            serde_json::to_string(reply).unwrap_or_default(),
        );
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    #[cfg(unix)]
    cmd.process_group(0);
    let mut child = cmd.spawn()?;
    let pid = child.id().unwrap_or_default();
    let session_id = format!("runner:{pid}");

    // stdin 流式注入（R-6：注入后观测首个输出轮次才算已接收——V1 以回执
    // 证据分层承接；此处只保证「写入成功」事实入 launch 摘要）。写入在独立
    // task 完成，task 结束即 drop stdin → EOF，CLI 开始消费。
    if let Some(prompt) = &launch.stdin_prompt {
        if let Some(mut stdin) = child.stdin.take() {
            let prompt = prompt.clone();
            tokio::spawn(async move {
                use tokio::io::AsyncWriteExt;
                let _ = stdin.write_all(prompt.as_bytes()).await;
                let _ = stdin.flush().await;
            });
        }
    }

    // 日志脱敏管道（I4'）：子进程输出 → token → [REDACTED] → 落盘。
    let secret_list: Arc<Vec<String>> = Arc::new(
        envs.iter()
            .filter(|(key, _)| key == &"AI_PM_LEASE_TOKEN" || key == &"AI_PM_HUMAN_REPLY")
            .map(|(_, value)| value.clone())
            .chain(secrets.to_owned())
            .filter(|value| !value.is_empty())
            .collect(),
    );
    if let Some(stdout) = child.stdout.take() {
        pump_stream(
            stdout,
            run_dir.join("logs").join("stdout.log"),
            secret_list.clone(),
        );
    }
    if let Some(stderr) = child.stderr.take() {
        pump_stream(stderr, run_dir.join("logs").join("stderr.log"), secret_list);
    }

    // Windows：spawn 后立即入 Job——进程树随句柄关闭整树终止（含孙进程）。
    #[cfg(windows)]
    let job = match job_object::JobHandle::create() {
        Ok(job) => {
            if let Err(e) = job.assign(pid) {
                tracing::warn!(pid, error = %e, "job assign failed; tree-kill degraded to direct child");
                None
            } else {
                Some(job)
            }
        }
        Err(e) => {
            tracing::warn!(pid, error = %e, "job create failed; tree-kill degraded to direct child");
            None
        }
    };

    write_launch_summary(run_dir, launch, envs, stdin_inject)?;
    let identity_hint =
        process_identity_hint(pid).unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    write_process_record(run_dir, pid, &session_id, &identity_hint)?;
    Ok(SpawnedProcess {
        child,
        pid,
        session_id,
        #[cfg(windows)]
        job,
    })
}

/// 终止 + 收割：返回 ExitOutcome 五值语义之一。
/// 对已退出进程调用 kill 为 Err（is_ok=FALSE），wait 仍返回真实 status。
pub async fn terminate(child: &mut Child) -> Result<ExitOutcome, AdapterError> {
    terminate_with_retry(child, 1, 10).await
}

/// 带进程树硬化的终止入口（Windows Job Object 先整树 TerminateJobObject）。
pub async fn terminate_spawned(spawned: &mut SpawnedProcess) -> Result<ExitOutcome, AdapterError> {
    #[cfg(windows)]
    if let Some(job) = spawned.job.as_ref() {
        job.terminate();
    }
    terminate_with_retry(&mut spawned.child, 1, 10).await
}

/// F-6（审计 2026-11-07）：有界重试终止（§6.4/§11.4）——kill 后限时等待，
/// 超时/失败重试，预算耗尽返回 Err（调用方回执 failed → Platform 冻结启动）。
/// 终止路径的退出判定 = `Killed`（平台主动终止语义；真实自然退出状态由
/// 收割路径 [`reap`] 承载）。
pub async fn terminate_with_retry(
    child: &mut Child,
    attempts: u32,
    per_attempt_secs: u64,
) -> Result<ExitOutcome, AdapterError> {
    let mut last_err: Option<AdapterError> = None;
    for attempt in 1..attempts.max(1) + 1 {
        // kill 已退出进程返回 Err：不影响 wait 取真实退出状态。
        let _ = child.kill().await;
        match tokio::time::timeout(
            std::time::Duration::from_secs(per_attempt_secs.max(1)),
            child.wait(),
        )
        .await
        {
            Ok(Ok(status)) => {
                return Ok(if status.success() {
                    ExitOutcome::Success
                } else {
                    ExitOutcome::Killed
                });
            }
            Ok(Err(e)) => last_err = Some(AdapterError::Io(e)),
            Err(_elapsed) => {
                last_err = Some(AdapterError::Invalid(format!(
                    "termination attempt {attempt}/{attempts} timed out after {per_attempt_secs}s"
                )));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| AdapterError::Invalid("termination failed".into())))
}

/// 退出收割（无终止请求时的自然退出判定）：Success / NonZeroExit / Signaled。
pub async fn reap(spawned: &mut SpawnedProcess) -> Option<ExitOutcome> {
    match spawned.child.try_wait() {
        Ok(Some(status)) => Some(ExitOutcome::from_exit_status(&status)),
        Ok(None) => None,
        Err(_) => Some(ExitOutcome::Lost),
    }
}

/// Terminate a process left by a previous Runner instance, but only when the
/// persisted process identity still matches. This prevents PID-reuse kills.
pub fn terminate_orphan(report: &OrphanReport) -> Result<(), AdapterError> {
    if !report.alive {
        return Ok(());
    }
    let expected = report.record.identity_hint.as_deref().ok_or_else(|| {
        AdapterError::Invalid("orphan process has no identity anchor; refusing unsafe termination".into())
    })?;
    let Some(actual) = process_identity_hint(report.record.pid) else {
        return Ok(());
    };
    if actual != expected {
        // PID has been reused or the recorded process is already gone. Never
        // terminate a process whose creation identity differs from the record.
        return Ok(());
    }

    #[cfg(unix)]
    {
        let result = unsafe { libc::kill(-(report.record.pid as i32), libc::SIGKILL) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(AdapterError::Io(error));
            }
        }
    }
    #[cfg(windows)]
    {
        let output = std::process::Command::new("taskkill")
            .args(["/PID", &report.record.pid.to_string(), "/T", "/F"])
            .output()?;
        if !output.status.success() && process_identity_hint(report.record.pid).as_deref() == Some(expected) {
            return Err(AdapterError::Invalid(format!(
                "taskkill failed for orphan pid {}: {}",
                report.record.pid,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        if process_identity_hint(report.record.pid).as_deref() != Some(expected) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Err(AdapterError::Invalid(format!(
        "orphan pid {} still matches its recorded process identity after termination",
        report.record.pid
    )))
}

/// 扫描未上报标记文件（P2-1 收敛：文件名含 event_id）：
/// `{run_dir}/.aipm/interaction.{event_id}.json` 与子目录 `{workspace_path}/
/// .aipm/…` 两层约定。文件名 event_id 与正文不一致 = 非法（双键必须一致）。
/// 非法标记也必须返回——调用方据此隔离（§1.2：不产生 Interaction、不重试）。
pub fn find_unreported_markers(
    run_dir: &Path,
) -> Vec<(PathBuf, Result<MarkerEvent, AdapterError>)> {
    let mut hits: Vec<(PathBuf, Result<MarkerEvent, AdapterError>)> = Vec::new();
    let scan = |aipm: &Path, hits: &mut Vec<_>| {
        let Ok(entries) = std::fs::read_dir(aipm) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            let Some(event_id) = name
                .strip_prefix("interaction.")
                .and_then(|rest| rest.strip_suffix(".json"))
                .map(str::to_string)
            else {
                continue;
            };
            let parsed = parse_marker(&path);
            if let Ok(event) = &parsed {
                if event.event_id != event_id {
                    hits.push((
                        path,
                        Err(AdapterError::Invalid(format!(
                            "marker filename event_id `{event_id}` does not match content `{}`",
                            event.event_id
                        ))),
                    ));
                    continue;
                }
            }
            hits.push((path, parsed));
        }
    };
    scan(&run_dir.join(".aipm"), &mut hits);
    if let Ok(entries) = std::fs::read_dir(run_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                scan(&entry.path().join(".aipm"), &mut hits);
            }
        }
    }
    hits
}

fn parse_marker(path: &Path) -> Result<MarkerEvent, AdapterError> {
    let raw = std::fs::read_to_string(path)?;
    if raw.len() > 64 * 1024 {
        return Err(AdapterError::Invalid("marker file exceeds 64KiB".into()));
    }
    let value: Value = serde_json::from_str(&raw)
        .map_err(|e| AdapterError::Invalid(format!("marker file is not valid JSON: {e}")))?;
    if value.get("schema_version").and_then(|v| v.as_str()) != Some(MARKER_SCHEMA) {
        return Err(AdapterError::Invalid(format!(
            "unsupported schema_version (expected {MARKER_SCHEMA})"
        )));
    }
    let get_str = |key: &str| -> Result<String, AdapterError> {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| AdapterError::Invalid(format!("marker field `{key}` is required")))
    };
    Ok(MarkerEvent {
        event_id: get_str("event_id")?,
        reason: get_str("reason")?,
        title: get_str("title").unwrap_or_default(),
        question: get_str("question")?,
        options: value.get("options").cloned(),
        required_response: value
            .get("required_response")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        checkpoint_path: value
            .get("checkpoint")
            .and_then(|c| c.get("path"))
            .and_then(|p| p.as_str())
            .map(|p| p.to_string()),
    })
}

/// 归档（event_id 键控，§1.2）：上报成功后调用；改名失败保留原文件，
/// 下一轮按同 event_id 幂等重报（ACP-442）。
pub fn archive_marker(path: &Path, event_id: &str) -> std::io::Result<PathBuf> {
    let target = path.with_file_name(format!("interaction.{event_id}.reported"));
    std::fs::rename(path, &target)?;
    Ok(target)
}

/// F-9（审计 2026-11-07）：Platform 业务拒绝（开放链冲突/运行态校验等
/// Tool 类错误）的标记归档——设计 §1.2"不重试"；链关闭后重报旧事件反而
/// 会产生重复介入（回复已被消费），故拒绝即终局。
pub fn reject_marker(path: &Path, event_id: &str) -> std::io::Result<PathBuf> {
    let target = path.with_file_name(format!("interaction.{event_id}.rejected"));
    std::fs::rename(path, &target)?;
    Ok(target)
}

/// 非法标记隔离（schema/枚举校验失败，§1.2：不产生 Interaction）。
pub fn quarantine_marker(path: &Path) -> std::io::Result<PathBuf> {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let target = path.with_file_name(format!("{name}.invalid"));
    std::fs::rename(path, &target)?;
    Ok(target)
}

fn write_reply(run_dir: &Path, reply: &Value) -> Result<(), AdapterError> {
    let dir = run_dir.join(".aipm");
    std::fs::create_dir_all(&dir)?;
    atomic_write(
        &dir.join("reply.json"),
        serde_json::to_vec_pretty(reply)
            .unwrap_or_default()
            .as_slice(),
    )?;
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

fn write_launch_summary(
    run_dir: &Path,
    launch: &ResolvedLaunch,
    envs: &[(&str, String)],
    stdin_injected: bool,
) -> Result<(), AdapterError> {
    // I12（R-6 第一类注入证据落点）：spawn 参数摘要——脱敏（凭据类 env 只记
    // 键名），环境变量全量值不落盘（codex_executor audit_summary 同款纪律）。
    let _ = std::fs::create_dir_all(run_dir.join(".aipm"));
    let env_keys: Vec<&str> = envs.iter().map(|(key, _)| *key).collect();
    atomic_write(
        &run_dir.join(".aipm").join("launch.json"),
        serde_json::to_vec_pretty(&json!({
            "program": launch.program,
            "arguments": launch.args,
            "working_directory": run_dir,
            "stdin_injected": stdin_injected,
            "env_keys": env_keys,
            "spawned_at": chrono::Utc::now().to_rfc3339(),
        }))?
        .as_slice(),
    )?;
    Ok(())
}

fn write_process_record(
    run_dir: &Path,
    pid: u32,
    session_id: &str,
    identity_hint: &str,
) -> Result<(), AdapterError> {
    let dir = run_dir.join(".aipm");
    std::fs::create_dir_all(&dir)?;
    // I9：原子写 + identity_hint（PID 重用防误认——codex_executor 先例）。
    atomic_write(
        &dir.join("process.json"),
        serde_json::to_vec_pretty(&json!({
            "pid": pid,
            "session_id": session_id,
            "identity_hint": identity_hint,
            "started_at": chrono::Utc::now().to_rfc3339(),
        }))?
        .as_slice(),
    )?;
    Ok(())
}

/// 进程身份锚点：Unix = /proc/{pid}/stat starttime；Windows = 进程创建时刻
/// FILETIME（GetProcessTimes）。取不到 → 调用方退化为当前时刻近似。
pub fn process_identity_hint(pid: u32) -> Option<String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
        use windows_sys::Win32::System::Threading::{
            GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            let mut created: FILETIME = std::mem::zeroed();
            let mut exited: FILETIME = std::mem::zeroed();
            let mut kernel: FILETIME = std::mem::zeroed();
            let mut user: FILETIME = std::mem::zeroed();
            let ok = GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user);
            CloseHandle(handle);
            if ok == 0 {
                return None;
            }
            Some(format!(
                "win-ft-{}-{}",
                created.dwHighDateTime, created.dwLowDateTime
            ))
        }
    }
    #[cfg(unix)]
    {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // comm 可含空格/括号：取最后一个 ')' 之后；其后首字段 = state（第 3 列），
        // starttime = 第 22 列 → 偏移 19。
        let after = stat.rsplit_once(')')?.1;
        let fields: Vec<&str> = after.split_whitespace().collect();
        fields.get(19).map(|value| format!("proc-start-{value}"))
    }
}

/// pid 存活判定（孤儿扫描用；不校验 identity——只做「是否还在」）。
pub fn is_pid_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return false;
            }
            CloseHandle(handle);
            true
        }
    }
    #[cfg(unix)]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
}

/// 本地进程账本记录（孤儿扫描的数据源）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessRecord {
    pub pid: u32,
    pub session_id: String,
    #[serde(default)]
    pub identity_hint: Option<String>,
}

pub fn read_process_record(run_dir: &Path) -> Option<ProcessRecord> {
    let raw = std::fs::read_to_string(run_dir.join(".aipm").join("process.json")).ok()?;
    serde_json::from_str(&raw).ok()
}

/// 孤儿扫描报告（Q-RC-5 裁决：只报告不强杀——用户 PC 上强杀无 Runner 上下文
/// 的进程越权风险大于收益；codex_executor 的按 pid 强杀先例属服务器容器独占域）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct OrphanReport {
    pub run_dir: PathBuf,
    pub record: ProcessRecord,
    pub alive: bool,
}

/// 启动时扫描 `{workspace_root}/*/.aipm/process.json`：报告上一实例遗留的
/// Agent 进程（含 pid 存活状态），不采取任何终止动作。
pub fn scan_orphans(workspace_root: &Path) -> Vec<OrphanReport> {
    let Ok(entries) = std::fs::read_dir(workspace_root) else {
        return Vec::new();
    };
    let mut reports = Vec::new();
    for entry in entries.flatten() {
        let run_dir = entry.path();
        if !run_dir.is_dir() {
            continue;
        }
        if let Some(record) = read_process_record(&run_dir) {
            let alive = is_pid_alive(record.pid);
            reports.push(OrphanReport {
                run_dir,
                record,
                alive,
            });
        }
    }
    reports
}

/// 脱敏：替换行内全部密钥出现（I4'）。
fn redact(line: &str, secrets: &[String]) -> String {
    let mut out = line.to_string();
    for secret in secrets {
        if !secret.is_empty() {
            out = out.replace(secret.as_str(), REDACTED);
        }
    }
    out
}

/// 子进程输出管道：逐行读取 → 脱敏 → append 落盘（带大小上限；超限丢写）。
fn pump_stream<R>(stream: R, path: PathBuf, secrets: Arc<Vec<String>>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let mut lines = BufReader::new(stream).lines();
        let mut written: u64 = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        // 读到流尾或 IO 错误即结束（Err 与 None 同义——进程已退出/管道已关）。
        while let Ok(Some(line)) = lines.next_line().await {
            let redacted = redact(&line, &secrets);
            if written + redacted.len() as u64 + 1 > LOG_CAP_BYTES {
                continue; // 超限丢写（防磁盘耗尽），进程照常收割
            }
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                if let Ok(()) = writeln!(file, "{redacted}") {
                    written += redacted.len() as u64 + 1;
                }
            }
        }
    });
}

// ───────────── Windows Job Object（ACP-08 硬化，移植自 codex_executor.rs） ─────────────

#[cfg(windows)]
pub mod job_object {
    //! 进程树终止硬化：Job Object + `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`——
    //! 句柄关闭即整树终止（含孙进程），覆盖 Runner 场景下 CLI 拉起的一切子进程。

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };

    pub struct JobHandle {
        handle: HANDLE,
    }
    unsafe impl Send for JobHandle {}
    unsafe impl Sync for JobHandle {}

    impl JobHandle {
        pub fn create() -> Result<Self, String> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return Err("CreateJobObjectW failed".into());
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const core::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    CloseHandle(handle);
                    return Err("SetInformationJobObject failed".into());
                }
                Ok(Self { handle })
            }
        }

        pub fn assign(&self, pid: u32) -> Result<(), String> {
            unsafe {
                let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
                if process.is_null() {
                    return Err(format!("OpenProcess({pid}) failed"));
                }
                let ok = AssignProcessToJobObject(self.handle, process);
                CloseHandle(process);
                if ok == 0 {
                    return Err(format!("AssignProcessToJobObject({pid}) failed"));
                }
                Ok(())
            }
        }

        pub fn terminate(&self) {
            unsafe {
                TerminateJobObject(self.handle, 1);
            }
        }
    }

    impl Drop for JobHandle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}
