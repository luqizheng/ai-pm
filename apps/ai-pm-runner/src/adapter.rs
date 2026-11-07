//! Runner ↔ Agent Adapter（V1.1 §1.2 / §7.3）：本地进程 spawn/收割/终止 +
//! `.aipm/interaction.json` 标记文件侦测与归档 + 回复注入。
//!
//! V1 侦测约定 = **marker_file**（stdout 保留给进程收割 ExitOutcome，不混载）；
//! 标记文件非权威存储（Platform 的 task_interaction 才是），Agent 只创建新
//! 事件不改写已提交事件。进程树终止硬化（Windows Job Object / Unix 进程组）
//! 随 ACP-08 Adapter 切片落实，V1 先行直接子进程 kill + pid 记录。

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
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

/// 一次 spawn 的进程（V1：直接子进程；pid 落 `{run_dir}/.aipm/process.json`
/// 供孤儿排查，ACP-02 扩展为进程组/Job Object 标识）。
pub struct SpawnedProcess {
    pub child: Child,
    pub pid: u32,
    pub session_id: String,
}

const MARKER_SCHEMA: &str = "aipm.interaction.v1";

/// spawn Agent 进程：命令模板按空白切分（不经 shell），环境注入对齐
/// `codex_executor.rs` 惯例 + 回复注入（Continuation 初始输入，§1.4）。
pub fn spawn_agent(
    agent_cmd: &str,
    run_dir: &Path,
    envs: &[(&str, String)],
    reply: Option<&Value>,
) -> Result<SpawnedProcess, AdapterError> {
    let mut parts = agent_cmd.split_whitespace();
    let Some(program) = parts.next() else {
        return Err(AdapterError::Invalid("agent command template is empty".into()));
    };
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(parts)
        .current_dir(run_dir)
        .env("AI_PM_WORKSPACE", run_dir)
        .env("AI_PM_AIPM_DIR", run_dir.join(".aipm"));
    for (key, value) in envs {
        cmd.env(key, value);
    }
    if let Some(reply) = reply {
        // Continuation 初始输入注入（V1：env + `.aipm/reply.json` 落盘双通道；
        // 注入成功 = INPUT_INJECTED 判据的文件证据，Platform 端不代写）。
        write_reply(run_dir, reply)?;
        cmd.env("AI_PM_HUMAN_REPLY", serde_json::to_string(reply).unwrap_or_default());
    }
    let child = cmd.spawn()?;
    let pid = child.id().unwrap_or_default();
    let session_id = format!("runner:{pid}");
    write_process_record(run_dir, pid, &session_id)?;
    Ok(SpawnedProcess {
        child,
        pid,
        session_id,
    })
}

/// 终止 + 收割：返回 ExitOutcome 五值语义之一（对齐 `port.rs` ExitOutcome）。
/// 对已退出进程调用 kill 为 Err（is_ok=FALSE），wait 仍返回真实 status。
pub async fn terminate(child: &mut Child) -> Result<String, AdapterError> {
    terminate_with_retry(child, 1, 10).await
}

/// F-6（审计 2026-11-07）：有界重试终止（§6.4/§11.4）——kill 后限时等待，
/// 超时/失败重试，预算耗尽返回 Err（调用方回执 failed → Platform 冻结启动）。
pub async fn terminate_with_retry(
    child: &mut Child,
    attempts: u32,
    per_attempt_secs: u64,
) -> Result<String, AdapterError> {
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
                    "Success".into()
                } else {
                    // V1 无信号级区分（进程组硬化随 ACP-08）：被 kill 的一律记 Killed。
                    "Killed".into()
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

/// 退出收割（无终止请求时的自然退出判定）。
pub async fn reap(spawned: &mut SpawnedProcess) -> Option<String> {
    match spawned.child.try_wait() {
        Ok(Some(status)) => Some(if status.success() {
            "Success".into()
        } else {
            "NonZeroExit".into()
        }),
        Ok(None) => None,
        Err(_) => Some("Lost".into()),
    }
}

/// 扫描未上报标记文件：`{run_dir}/{workspace_path}/.aipm/interaction.json`
/// 与 `{run_dir}/.aipm/interaction.json` 两层约定（§1.2 路径规则，depth 有限）。
/// 返回 `(路径, 解析结果)`：解析失败（非法 schema/字段）也必须返回——
/// 调用方据此隔离（§1.2：不产生 Interaction、不重试），静默跳过会让非法
/// 标记永久滞留工作区。
pub fn find_unreported_marker(
    run_dir: &Path,
) -> Option<(PathBuf, Result<MarkerEvent, AdapterError>)> {
    let mut candidates = vec![
        run_dir.join(".aipm").join("interaction.json"),
    ];
    if let Ok(entries) = std::fs::read_dir(run_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                candidates.push(entry.path().join(".aipm").join("interaction.json"));
            }
        }
    }
    for path in candidates {
        if !path.is_file() {
            continue;
        }
        let parsed = parse_marker(&path);
        return Some((path, parsed));
    }
    None
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
            .map(|s| s.to_string())
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
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let target = path.with_file_name(format!("{name}.invalid"));
    std::fs::rename(path, &target)?;
    Ok(target)
}

fn write_reply(run_dir: &Path, reply: &Value) -> Result<(), AdapterError> {
    let dir = run_dir.join(".aipm");
    std::fs::create_dir_all(&dir)?;
    let tmp = dir.join("reply.json.tmp");
    let target = dir.join("reply.json");
    std::fs::write(&tmp, serde_json::to_vec_pretty(reply).unwrap_or_default())?;
    std::fs::rename(&tmp, &target)?;
    Ok(())
}

fn write_process_record(run_dir: &Path, pid: u32, session_id: &str) -> Result<(), AdapterError> {
    let dir = run_dir.join(".aipm");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("process.json"),
        serde_json::to_vec_pretty(&json!({
            "pid": pid,
            "session_id": session_id,
            "started_at": chrono::Utc::now().to_rfc3339(),
        }))?,
    )?;
    Ok(())
}
