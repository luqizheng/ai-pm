//! Slice D3：Codex CLI 执行器适配器（`docs/ai-pm-slice-d3-executor-adapter-v1.md` §2）。
//!
//! 职责边界：本适配器只管**进程与工作区**——建目录、镜像仓 + worktree checkout、
//! spawn/收割/终止、pid 文件与日志文件；编排与落库在 `pm-application::run_executor`。
//! `AI_PM_RUN_EXECUTOR=mock` 复用本适配器（`AI_PM_CODEX_BIN` 指向假脚本即为 mock）。
//!
//! V1 边界（实施裁定，随交付记录）：`claim_and_start` 走 Model B（无 lease_token
//! 明文可下发），spawn 的执行端暂无 MCP 心跳凭据——租约续期由编排器 poll 兜底
//! 通道（Q-D3-4 的平台侧）维持；执行端心跳通道留给 Model A（自拉）场景与后续
//! token 下发裁点。`claimed→running` 由编排器在 start 成功后经受守卫入口推进
//! （D1 §九问题 4 的本意：D3 在执行端启动成功后经平台写入）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;

use pm_application::run_dispatch::{
    ExecutableRun, ExecutorError, ExecutorProgress, ExecutorSession, RunExecutor, TerminateReason,
};
use sqlx::PgPool;
use tokio::process::Child;
use uuid::Uuid;

pub struct CodexExecutor {
    pool: PgPool,
    bin: String,
    args: Vec<String>,
    root: PathBuf,
    kill_grace_secs: u64,
    children: Mutex<HashMap<String, ChildHandle>>,
}

struct ChildHandle {
    child: Child,
    #[cfg_attr(windows, allow(dead_code))] // Unix 优雅终止用 pid 发 SIGTERM
    pid: u32,
}

impl CodexExecutor {
    /// 显式构造（测试注入假脚本 / 指定 root；生产走 [`Self::from_env`]）。
    pub fn new(pool: PgPool, bin: String, args: Vec<String>, root: PathBuf, kill_grace_secs: u64) -> Self {
        Self {
            pool,
            bin,
            args,
            root,
            kill_grace_secs,
            children: Mutex::new(HashMap::new()),
        }
    }

    /// 从环境装配（worker 注册表入口）。`AI_PM_RUN_EXECUTOR=mock` 与 `codex`
    /// 同款装配——mock 只是把 `AI_PM_CODEX_BIN` 指向假脚本。
    pub fn from_env(pool: PgPool) -> Self {
        let bin = std::env::var("AI_PM_CODEX_BIN").unwrap_or_else(|_| "codex".to_string());
        let args = std::env::var("AI_PM_CODEX_ARGS")
            .map(|raw| raw.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        let root = PathBuf::from(pm_application::run_dispatch::workspace_root_from_env());
        let kill_grace_secs = std::env::var("AI_PM_RUN_KILL_GRACE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        Self::new(pool, bin, args, root, kill_grace_secs)
    }


    fn run_root(&self, run_id: Uuid) -> PathBuf {
        self.root.join(run_id.to_string())
    }

    fn pid_file(&self, run_id: Uuid) -> PathBuf {
        self.run_root(run_id).join("process.json")
    }

    /// 一次性把私钥写成临时 key 文件，返回 `GIT_SSH_COMMAND`（非 ssh url 返回 None）。
    fn ssh_command_for(&self, url: &str, project_id: Uuid, ciphertext: &[u8]) -> Option<String> {
        if !(url.starts_with("git@") || url.starts_with("ssh://")) {
            return None;
        }
        let plain = pm_application::repository::open_ssh_private_key(project_id, ciphertext).ok()?;
        let key_path = self.root.join("keys").join(format!("{project_id}.pem"));
        std::fs::create_dir_all(key_path.parent()?).ok()?;
        std::fs::write(&key_path, plain).ok()?;
        Some(format!(
            "ssh -i {} -o StrictHostKeyChecking=accept-new -o IdentitiesOnly=yes",
            key_path.display()
        ))
    }

    /// 镜像仓不存在则 `clone --mirror`，存在则 `fetch --all --prune`；随后从镜像
    /// 建 worktree 并 detach 到 `base_commit`。git 失败一律 `ExecutorError::Other`。
    async fn checkout(
        &self,
        project_id: Uuid,
        repository_id: Uuid,
        base_commit: &str,
        target: &Path,
    ) -> Result<(), ExecutorError> {
        let mirror = self.root.join("git-mirror").join(repository_id.to_string());
        let is_bare = mirror.join("HEAD").exists();
        let key_row: Option<(String, Option<Vec<u8>>)> = sqlx::query_as(
            "SELECT pr.url, k.private_key_ciphertext \
               FROM integration.project_repository pr \
               LEFT JOIN integration.project_ssh_key k \
                 ON k.project_id = pr.project_id AND k.revoked_at IS NULL \
              WHERE pr.id = $1 AND pr.project_id = $2 \
              ORDER BY k.created_at DESC NULLS LAST \
              LIMIT 1",
        )
        .bind(repository_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ExecutorError::Other(e.to_string()))?;
        let Some((url, key_ciphertext)) = key_row else {
            return Err(ExecutorError::Other(format!(
                "repository {repository_id} not found for project {project_id}"
            )));
        };
        let ssh_command = key_ciphertext
            .as_deref()
            .and_then(|cipher| self.ssh_command_for(&url, project_id, cipher));

        let git = |args: &[&str]| {
            let mut cmd = tokio::process::Command::new("git");
            cmd.args(args).stdin(Stdio::null());
            if let Some(ssh) = &ssh_command {
                cmd.env("GIT_SSH_COMMAND", ssh);
            }
            cmd
        };

        if is_bare {
            let out = git(&[
                "-C",
                mirror.to_string_lossy().as_ref(),
                "fetch",
                "--all",
                "--prune",
            ])
            .output()
            .await
            .map_err(|e| ExecutorError::Other(format!("git fetch failed: {e}")))?;
            if !out.status.success() {
                return Err(ExecutorError::Other("git fetch failed".into()));
            }
        } else {
            std::fs::create_dir_all(mirror.parent().expect("mirror parent"))
                .map_err(|e| ExecutorError::Other(format!("create mirror dir failed: {e}")))?;
            git(&["clone", "--mirror", &url, mirror.to_string_lossy().as_ref()])
                .output()
                .await
                .and_then(|out| {
                    out.status
                        .success()
                        .then_some(out)
                        .ok_or_else(|| std::io::Error::other("git clone --mirror failed"))
                })
                .map_err(|e| ExecutorError::Other(format!("git clone --mirror failed: {e}")))?;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ExecutorError::Other(format!("create workspace dir failed: {e}")))?;
        }
        git(&[
            "-C",
            mirror.to_string_lossy().as_ref(),
            "worktree",
            "add",
            "--detach",
            target.to_string_lossy().as_ref(),
            base_commit,
        ])
        .output()
        .await
        .and_then(|out| {
            out.status
                .success()
                .then_some(out)
                .ok_or_else(|| std::io::Error::other("git worktree add failed"))
        })
        .map_err(|e| ExecutorError::Other(format!("git worktree add failed: {e}")))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl RunExecutor for CodexExecutor {
    fn kind(&self) -> &'static str {
        "codex"
    }

    /// 建工作区 → 逐仓 checkout → spawn → pid 文件 + 日志重定向。
    /// 任一步失败即 `ExecutorError`，编排器按基础设施失败退避。
    async fn start(&self, run: &ExecutableRun) -> Result<ExecutorSession, ExecutorError> {
        let run_root = self.run_root(run.run_id);
        std::fs::create_dir_all(run_root.join("logs"))
            .map_err(|e| ExecutorError::Other(format!("create run dir failed: {e}")))?;
        for baseline in &run.repositories {
            let target = run_root.join(&baseline.workspace_path);
            self.checkout(
                run.project_id,
                baseline.repository_id,

                &baseline.base_commit,
                &target,
            )
            .await?;
        }

        let stdout = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(run_root.join("logs").join("stdout.log"))
            .map_err(|e| ExecutorError::Other(format!("open stdout log failed: {e}")))?;
        let stderr = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(run_root.join("logs").join("stderr.log"))
            .map_err(|e| ExecutorError::Other(format!("open stderr log failed: {e}")))?;

        let workdir = run
            .repositories
            .first()
            .map(|b| run_root.join(&b.workspace_path))
            .unwrap_or_else(|| run_root.clone());
        let mut cmd = tokio::process::Command::new(&self.bin);
        cmd.args(&self.args)
            .current_dir(&workdir)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .stdin(Stdio::null())
            .env("AI_PM_RUN_ID", run.run_id.to_string())
            .env("AI_PM_TASK_ID", run.task_id.to_string())
            .env("AI_PM_PROJECT_ID", run.project_id.to_string())
            .env("AI_PM_ATTEMPT_NO", run.attempt_no.to_string())
            .env("AI_PM_WORKSPACE_ROOT", &self.root);
        let child = cmd
            .spawn()
            .map_err(|e| ExecutorError::Other(format!("spawn {} failed: {e}", self.bin)))?;
        let pid = child.id().ok_or_else(|| {
            ExecutorError::Other("spawned child has no pid (already reaped)".into())
        })?;

        let session = format!("{}:{}", self.kind(), Uuid::new_v4());
        let pid_payload = serde_json::json!({
            "pid": pid,
            "session": session,
            "started_at": chrono::Utc::now().to_rfc3339(),
        });
        std::fs::write(
            self.pid_file(run.run_id),
            serde_json::to_vec(&pid_payload).expect("pid payload serializes"),
        )
        .map_err(|e| ExecutorError::Other(format!("write pid file failed: {e}")))?;

        self.children
            .lock()
            .expect("children mutex")
            .insert(session.clone(), ChildHandle { child, pid });

        Ok(ExecutorSession {
            session,
            run_id: run.run_id,
            workspace_root: self.root.to_string_lossy().to_string(),
        })
    }

    /// 存活探测：内存句柄优先（本进程 spawn 的）；跨重启的孤儿读 pid 文件判定。
    async fn poll(&self, session: &ExecutorSession) -> Result<ExecutorProgress, ExecutorError> {
        enum Step {
            Progress(ExecutorProgress),
            /// 已收割（从 map 移除），携带退出码。
            Reaped(Option<i32>),
        }
        let step = {
            let mut guard = self.children.lock().expect("children mutex");
            match guard.get_mut(&session.session) {
                Some(handle) => match handle.child.try_wait() {
                    Ok(Some(status)) => {
                        guard.remove(&session.session);
                        Step::Reaped(status.code())
                    }
                    Ok(None) => Step::Progress(ExecutorProgress::Running),
                    Err(e) => return Err(ExecutorError::Other(format!("try_wait failed: {e}"))),
                },
                // 跨重启孤儿：pid 文件存在且进程仍存活 → Running；否则退出码不可归因。
                None => {
                    let pid = read_pid(self.pid_file(session.run_id).as_path())
                        .map_err(|e| ExecutorError::Other(e.to_string()))?;
                    match pid {
                        Some(pid) if process_alive(pid) => {
                            Step::Progress(ExecutorProgress::Running)
                        }
                        _ => Step::Progress(ExecutorProgress::Done { exit_code: None }),
                    }
                }
            }
        };
        Ok(match step {
            Step::Progress(progress) => progress,
            Step::Reaped(exit_code) => ExecutorProgress::Done { exit_code },
        })
    }

    /// 统一终止管线（Q-D3-5）：先探测已退出；存活进程按平台给信号——
    /// Unix SIGTERM → 宽限 → 强杀；Windows 直接强杀。
    async fn terminate(
        &self,
        session: &ExecutorSession,
        _reason: TerminateReason,
    ) -> Result<(), ExecutorError> {
        // take 出句柄再 await（std::MutexGuard 不得跨 await，future 必须 Send）。
        let taken: Option<ChildHandle> = {
            let mut guard = self.children.lock().expect("children mutex");
            guard.remove(&session.session)
        };
        if let Some(mut handle) = taken {
            if handle.child.try_wait().map_err(wrap_io)?.is_none() {
                graceful_stop(&mut handle, self.kill_grace_secs).await?;
            }
            return Ok(());
        }
        // 跨重启孤儿：pid 文件定位进程，直接强杀（无句柄做不了优雅终止）。
        if let Some(pid) = read_pid(self.pid_file(session.run_id).as_path())
            .map_err(wrap_io)?
            .filter(|pid| process_alive(*pid))
        {
            kill_by_pid(pid).map_err(wrap_io)?;
        }
        Ok(())
    }
}

fn wrap_io(e: std::io::Error) -> ExecutorError {
    ExecutorError::Other(e.to_string())
}

fn read_pid(path: &Path) -> Result<Option<u32>, std::io::Error> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    Ok(value.get("pid").and_then(|pid| pid.as_u64()).map(|pid| pid as u32))
}

/// 跨平台进程存活判定（不引入 winapi/winapi 依赖）。
fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // kill(pid, 0) 只探测存在性，不发送信号。
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(windows)]
    {
        let output = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output();
        match output {
            Ok(out) => String::from_utf8_lossy(&out.stdout).contains(&pid.to_string()),
            Err(_) => false,
        }
    }
}

/// Unix 优雅终止：SIGTERM → 宽限轮询 → SIGKILL。Windows 无此语义。
#[cfg(unix)]
async fn graceful_stop(handle: &mut ChildHandle, grace_secs: u64) -> Result<(), ExecutorError> {
    unsafe {
        libc::kill(handle.pid as i32, libc::SIGTERM);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(grace_secs);
    loop {
        if handle.child.try_wait().map_err(wrap_io)?.is_some() {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    handle.child.start_kill().map_err(wrap_io)?;
    handle.child.wait().await.map_err(wrap_io)?;
    Ok(())
}

#[cfg(windows)]
async fn graceful_stop(handle: &mut ChildHandle, _grace_secs: u64) -> Result<(), ExecutorError> {
    // Windows 控制台进程没有通用 SIGTERM 等价物（Q-D3-5 平台差异）：直接强杀。
    handle.child.start_kill().map_err(wrap_io)?;
    handle.child.wait().await.map_err(wrap_io)?;
    Ok(())
}

/// 无句柄时的跨平台强杀（孤儿路径）。
fn kill_by_pid(pid: u32) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        unsafe {
            if libc::kill(pid as i32, libc::SIGKILL) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        let status = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!("taskkill {pid} failed")))
        }
    }
}