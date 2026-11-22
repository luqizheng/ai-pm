//! Slice D3：Codex CLI 执行器适配器（`docs/slice-d/ai-pm-slice-d3-executor-adapter-v1.md` §2
//! + `docs/slice-d/ai-pm-slice-d3-executor-hardening-v1.md` §2/§5/§6/§7）。
//!
//! 职责边界：本适配器只管**进程与工作区**——建目录、镜像仓 + worktree checkout、
//! spawn/收割/终止、pid 文件与日志文件；编排与落库在 `pm-application::run_executor`。
//! `AI_PM_RUN_EXECUTOR=mock` 复用本适配器（`AI_PM_CODEX_BIN` 指向假脚本即为 mock）。
//!
//! T-032 落地项（终裁）：
//! - **T1**：`session` 由编排器前置生成并持久化（启动意图）；pid 文件原子写 +
//!   进程身份锚点；孤儿判定走三重证据（pid 文件 / run 目录痕迹 / Job·pgid），
//!   无法确证进程已死时返回 `Uncertain`，绝不静默回收。
//! - **ADR-0002 L1/L2**：`AI_PM_LEASE_TOKEN` / `AI_PM_LEASE_GENERATION` 注入执行端
//!   环境（Model A / Model B 统一凭据链路）；明文禁止落 pid 文件 / 日志 / 事件。
//! - **T3**：`deadline_at` 只读使用（固化语义，本适配器不写）。
//! - **T4**：心跳与进展由 MCP 通道刷新（`run_context`），本适配器不碰。
//! - **§5**：Windows Job Object（`KILL_ON_JOB_CLOSE`）/ Unix Process Group——
//!   Codex 派生的 Cargo / Git / Shell 全量回收；PID 重用防误判（身份锚点比对）。
//! - **§6**：镜像仓跨进程锁、workspace_path 二次校验、日志脱敏与大小上限、
//!   结构化 `ExitOutcome`、`executor-events.jsonl`、`CodexLaunchProfile`。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;

use pm_application::run_dispatch::{
    append_executor_event, validate_workspace_path, ExecutableRun, ExecutorError, ExecutorProgress,
    ExecutorSession, ExitOutcome, RunExecutor, TerminateReason,
};
use sqlx::PgPool;

use tokio::process::Child;
use uuid::Uuid;

/// 日志大小上限（env `AI_PM_RUN_LOG_MAX_BYTES`，默认 50 MiB）。超限后继续读
/// 但丢弃（防管道背压阻塞子进程），文件尾追加一次截断标记。
const LOG_MAX_BYTES_DEFAULT: u64 = 50 * 1024 * 1024;
/// 镜像仓锁的陈旧阈值（秒）：持锁进程崩溃后锁文件残留，超时视为陈旧可清除。
const MIRROR_LOCK_STALE_SECS: u64 = 600;

/// SSH 协议仓库 url（决定是否需要落临时私钥）。与迁移 0022 起的判定口径
/// 一致：`git@` scp 风格或 `ssh://` 前缀。
fn is_ssh_url(url: &str) -> bool {
    url.starts_with("git@") || url.starts_with("ssh://")
}

/// [`CodexExecutor::ssh_command_for`] 的可测内芯：给定明文私钥，落 0700
/// 临时目录（0600 key 文件，Windows 退化为普通文件）并组装
/// `GIT_SSH_COMMAND`。返回的 `KeyMaterial` 守卫 drop 时删除整个目录。
fn ssh_command_with_key(
    url: &str,
    plain: &str,
) -> Option<(String, pm_application::git_access::KeyMaterial)> {
    if !is_ssh_url(url) {
        return None;
    }
    let key = pm_application::git_access::write_temp_key(plain).ok()?;
    let command = format!(
        "ssh -i {} -o StrictHostKeyChecking=accept-new -o IdentitiesOnly=yes",
        key.key_path().display()
    );
    Some((command, key))
}

pub struct CodexExecutor {
    pool: PgPool,
    bin: String,
    extra_args: Vec<String>,
    root: PathBuf,
    kill_grace_secs: u64,
    log_max_bytes: u64,
    children: Mutex<HashMap<String, ChildHandle>>,
}

struct ChildHandle {
    child: Child,
    /// Unix 优雅终止用 pid 发进程组信号；Windows 由 Job Object 承担（pid 仅为
    /// 日志与事件引用）。
    #[cfg_attr(windows, allow(dead_code))]
    pid: u32,
    /// T-032 §5：Windows Job Object 句柄（Drop 即 `KILL_ON_JOB_CLOSE` 级联终止
    /// 全部子进程）；Unix 无此概念（用进程组 kill(-pgid)）。
    #[cfg(windows)]
    job: job_object::JobHandle,
}

impl CodexExecutor {
    /// 显式构造（测试注入假脚本 / 指定 root；生产走 [`Self::from_env`]）。
    pub fn new(
        pool: PgPool,
        bin: String,
        extra_args: Vec<String>,
        root: PathBuf,
        kill_grace_secs: u64,
    ) -> Self {
        Self {
            pool,
            bin,
            extra_args,
            root,
            kill_grace_secs,
            log_max_bytes: std::env::var("AI_PM_RUN_LOG_MAX_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(LOG_MAX_BYTES_DEFAULT),
            children: Mutex::new(HashMap::new()),
        }
    }

    /// 从环境装配（worker 注册表入口）。`AI_PM_RUN_EXECUTOR=mock` 与 `codex`
    /// 同款装配——mock 只是把 `AI_PM_CODEX_BIN` 指向假脚本。
    pub fn from_env(pool: PgPool) -> Self {
        let bin = std::env::var("AI_PM_CODEX_BIN").unwrap_or_else(|_| "codex".to_string());
        let extra_args = std::env::var("AI_PM_CODEX_ARGS")
            .map(|raw| raw.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        let root = PathBuf::from(pm_application::run_dispatch::workspace_root_from_env());
        let kill_grace_secs = std::env::var("AI_PM_RUN_KILL_GRACE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        Self::new(pool, bin, extra_args, root, kill_grace_secs)
    }

    fn run_root(&self, run_id: Uuid) -> PathBuf {
        self.root.join(run_id.to_string())
    }

    fn pid_file(&self, run_id: Uuid) -> PathBuf {
        self.run_root(run_id).join("process.json")
    }

    /// 解密私钥并落成 RAII 临时 key 文件，返回 `(GIT_SSH_COMMAND, 守卫)`
    /// （非 ssh url 返回 None）。守卫必须与本次镜像 fetch/clone 的 git 子进程
    /// 同作用域存续，drop 时连目录删除——成功/失败/崩溃路径均无明文残留
    /// （PA07-IMP-008；复用 `git_access::write_temp_key`，即 submit 远端校验
    /// 路径同一先例）。旧实现明文写共享 `keys/{repository_id}.pem` 且无删除
    /// 路径，本方法废弃该布局。
    fn ssh_command_for(
        &self,
        url: &str,
        project_id: Uuid,
        ciphertext: &[u8],
    ) -> Option<(String, pm_application::git_access::KeyMaterial)> {
        if !is_ssh_url(url) {
            return None;
        }
        let plain =
            pm_application::repository::open_ssh_private_key(project_id, ciphertext).ok()?;
        ssh_command_with_key(url, &plain)
    }

    /// T-032 §6.1：镜像仓跨进程锁（每 repo_id 一把；原子 create_new + 陈旧检测）。
    /// 双 Run 并发首 clone / 并发 fetch 在此串行化；持锁进程崩溃后的残留锁在
    /// 超时后可安全清除。
    fn lock_mirror(&self, repository_id: Uuid) -> Result<MirrorLockGuard, ExecutorError> {
        let mirror_dir = self.root.join("git-mirror");
        std::fs::create_dir_all(&mirror_dir)
            .map_err(|e| ExecutorError::Other(format!("create mirror dir failed: {e}")))?;
        let path = mirror_dir.join(format!("{repository_id}.lock"));
        let deadline = std::time::Instant::now() + Duration::from_secs(120);
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => {
                    return Ok(MirrorLockGuard { path });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // 陈旧锁检测：mtime 超阈值 → 视为持锁进程已崩溃，清除重试。
                    if let Ok(meta) = std::fs::metadata(&path) {
                        if let Ok(modified) = meta.modified() {
                            if modified.elapsed().unwrap_or_default()
                                > Duration::from_secs(MIRROR_LOCK_STALE_SECS)
                            {
                                let _ = std::fs::remove_file(&path);
                                continue;
                            }
                        }
                    }
                    if std::time::Instant::now() >= deadline {
                        return Err(ExecutorError::Other(format!(
                            "mirror lock timeout: {}",
                            path.display()
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    return Err(ExecutorError::Other(format!(
                        "acquire mirror lock failed: {e}"
                    )));
                }
            }
        }
    }

    /// Reclaim one per-Run Git worktree. Caller must have revalidated the
    /// reclamation ledger row and process-cleanup state immediately beforehand.
    /// Only the repository worktree directory is removed; Run logs and metadata
    /// remain untouched.
    pub async fn reclaim_run_worktree(
        &self,
        run_id: Uuid,
        repository_id: Uuid,
        workspace_path: &str,
    ) -> Result<(), ExecutorError> {
        let _lock = self.lock_mirror(repository_id)?;
        let target = resolve_run_worktree_target(&self.root, run_id, workspace_path)?;
        let mirror = resolve_repository_mirror_target(&self.root, repository_id)?;

        let Some(target) = target else {
            if let Some(mirror) = mirror {
                let prune = tokio::process::Command::new("git")
                    .arg("--git-dir")
                    .arg(&mirror)
                    .args(["worktree", "prune"])
                    .output()
                    .await
                    .map_err(|e| ExecutorError::Other(format!("git worktree prune failed: {e}")))?;
                if !prune.status.success() {
                    return Err(ExecutorError::Other(format!(
                        "git worktree prune failed: {}",
                        String::from_utf8_lossy(&prune.stderr).trim()
                    )));
                }
            }
            return Ok(());
        };

        if let Some(mirror) = mirror {
            let output = tokio::process::Command::new("git")
                .arg("--git-dir")
                .arg(&mirror)
                .args(["worktree", "remove", "--force"])
                .arg(&target)
                .output()
                .await
                .map_err(|e| ExecutorError::Other(format!("git worktree remove failed: {e}")))?;
            if !output.status.success() {
                return Err(ExecutorError::Other(format!(
                    "git worktree remove failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                )));
            }
            let prune = tokio::process::Command::new("git")
                .arg("--git-dir")
                .arg(&mirror)
                .args(["worktree", "prune"])
                .output()
                .await
                .map_err(|e| ExecutorError::Other(format!("git worktree prune failed: {e}")))?;
            if !prune.status.success() {
                return Err(ExecutorError::Other(format!(
                    "git worktree prune failed: {}",
                    String::from_utf8_lossy(&prune.stderr).trim()
                )));
            }
        } else {
            std::fs::remove_dir_all(&target).map_err(|e| {
                ExecutorError::Other(format!("remove orphaned worktree directory failed: {e}"))
            })?;
        }
        Ok(())
    }


    /// Reclaim a shared bare mirror only when Git confirms no linked worktrees
    /// remain. Repository/Commit database rows and remote history are untouched.
    pub async fn reclaim_repository_mirror(
        &self,
        repository_id: Uuid,
    ) -> Result<(), ExecutorError> {
        let _lock = self.lock_mirror(repository_id)?;
        let Some(mirror) = resolve_repository_mirror_target(&self.root, repository_id)? else {
            return Ok(());
        };

        let prune = tokio::process::Command::new("git")
            .arg("--git-dir")
            .arg(&mirror)
            .args(["worktree", "prune"])
            .output()
            .await
            .map_err(|e| ExecutorError::Other(format!("git worktree prune failed: {e}")))?;
        if !prune.status.success() {
            return Err(ExecutorError::Other(format!(
                "git worktree prune failed: {}",
                String::from_utf8_lossy(&prune.stderr).trim()
            )));
        }

        let listing = tokio::process::Command::new("git")
            .arg("--git-dir")
            .arg(&mirror)
            .args(["worktree", "list", "--porcelain"])
            .output()
            .await
            .map_err(|e| ExecutorError::Other(format!("git worktree list failed: {e}")))?;
        if !listing.status.success() {
            return Err(ExecutorError::Other(format!(
                "git worktree list failed: {}",
                String::from_utf8_lossy(&listing.stderr).trim()
            )));
        }
        let mirror_canonical = std::fs::canonicalize(&mirror)
            .map_err(|e| ExecutorError::Other(format!("canonicalize mirror failed: {e}")))?;
        for line in String::from_utf8_lossy(&listing.stdout).lines() {
            let Some(path) = line.strip_prefix("worktree ") else {
                continue;
            };
            let linked = std::fs::canonicalize(path).map_err(|e| {
                ExecutorError::Other(format!(
                    "cannot verify linked worktree path {path}; refusing mirror deletion: {e}"
                ))
            })?;
            if linked != mirror_canonical {
                return Err(ExecutorError::Other(format!(
                    "mirror still has linked worktree {}",
                    linked.display()
                )));
            }
        }

        std::fs::remove_dir_all(&mirror)
            .map_err(|e| ExecutorError::Other(format!("remove bare mirror failed: {e}")))?;
        Ok(())
    }

    /// 镜像仓不存在则 `clone --mirror`（临时目录 → 原子发布），存在则
    /// `fetch --all --prune`；随后从镜像建 worktree 并 detach 到 `base_commit`。
    async fn checkout(
        &self,
        project_id: Uuid,
        repository_id: Uuid,
        base_commit: &str,
        target: &Path,
    ) -> Result<(), ExecutorError> {
        let mirror_dir = self.root.join("git-mirror");
        let mirror = mirror_dir.join(repository_id.to_string());
        let _lock = self.lock_mirror(repository_id)?;
        let is_bare = mirror.join("HEAD").exists();
        // 迁移 0022：绑定把优先，未绑定（或绑定已失效）回退项目默认/最新 active。
        let key_row: Option<(String, Option<Vec<u8>>)> = sqlx::query_as(
            "SELECT pr.url, COALESCE(bound.private_key_ciphertext, fb.private_key_ciphertext) \
               FROM integration.project_repository pr \
               LEFT JOIN integration.project_ssh_key bound \
                 ON bound.id = pr.ssh_key_id AND bound.status = 'active' \
               LEFT JOIN LATERAL (\
                 SELECT k.private_key_ciphertext \
                   FROM integration.project_ssh_key k \
                  WHERE k.project_id = pr.project_id AND k.status = 'active' \
                  ORDER BY k.is_default DESC, k.created_at DESC LIMIT 1\
               ) fb ON pr.ssh_key_id IS NULL \
              WHERE pr.id = $1 AND pr.project_id = $2",
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
        let (_ssh_key_guard, ssh_command) = match key_ciphertext
            .as_deref()
            .and_then(|cipher| self.ssh_command_for(&url, project_id, cipher))
        {
            Some((command, guard)) => (Some(guard), Some(command)),
            None => (None, None),
        };
        // IMP-008：`_ssh_key_guard` 与下方全部 git 子进程同作用域存续，
        // 函数返回（含错误提前返回）时 Drop 删除临时密钥目录。

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
                // fetch 失败保留既有可用镜像（§6.1：不因一次失败破坏共享对象库）。
                return Err(ExecutorError::Other("git fetch failed".into()));
            }
        } else {
            std::fs::create_dir_all(mirror_dir.parent().expect("mirror parent"))
                .map_err(|e| ExecutorError::Other(format!("create mirror dir failed: {e}")))?;
            // §6.1：首 clone 用临时目录，成功后原子发布——并发竞争者只会看到
            // 「不存在（继续持锁等待）」或「完整可用」两种状态。
            let staging = mirror_dir.join(format!(".staging-{repository_id}-{}", Uuid::new_v4()));
            git(&[
                "clone",
                "--mirror",
                &url,
                staging.to_string_lossy().as_ref(),
            ])
            .output()
            .await
            .and_then(|out| {
                out.status
                    .success()
                    .then_some(out)
                    .ok_or_else(|| std::io::Error::other("git clone --mirror failed"))
            })
            .map_err(|e| ExecutorError::Other(format!("git clone --mirror failed: {e}")))?;
            std::fs::rename(&staging, &mirror).map_err(|e| {
                let _ = std::fs::remove_dir_all(&staging);
                ExecutorError::Other(format!("publish mirror failed: {e}"))
            })?;
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

/// Resolve a Run worktree without following any symlink below the Run root.
/// Missing roots/targets are idempotent no-ops; malformed paths and symlink
/// escapes fail closed.
fn resolve_run_worktree_target(
    root: &Path,
    run_id: Uuid,
    workspace_path: &str,
) -> Result<Option<PathBuf>, ExecutorError> {
    let relative = validate_workspace_path(workspace_path)?;
    if relative.is_empty() {
        return Err(ExecutorError::Other("workspace_path must not be empty".into()));
    }
    let root_canonical = match std::fs::canonicalize(root) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ExecutorError::Other(format!("canonicalize workspace root failed: {error}"))),
    };
    let run_root = root.join(run_id.to_string());
    let run_meta = match std::fs::symlink_metadata(&run_root) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ExecutorError::Other(format!("inspect Run root failed: {error}"))),
    };
    if run_meta.file_type().is_symlink() {
        return Err(ExecutorError::Other("Run root is a symlink; refusing reclamation".into()));
    }
    let run_canonical = std::fs::canonicalize(&run_root)
        .map_err(|e| ExecutorError::Other(format!("canonicalize Run root failed: {e}")))?;
    if !run_canonical.starts_with(&root_canonical) {
        return Err(ExecutorError::Other("Run root escapes configured workspace root".into()));
    }

    let mut current = run_root;
    let components: Vec<_> = Path::new(&relative).components().collect();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        let meta = match std::fs::symlink_metadata(&current) {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ExecutorError::Other(format!("inspect worktree path failed: {error}"))),
        };
        if meta.file_type().is_symlink() {
            return Err(ExecutorError::Other("worktree path contains a symlink; refusing reclamation".into()));
        }
        if index + 1 < components.len() && !meta.is_dir() {
            return Err(ExecutorError::Other("worktree path parent is not a directory".into()));
        }
    }
    let target = std::fs::canonicalize(&current)
        .map_err(|e| ExecutorError::Other(format!("canonicalize worktree failed: {e}")))?;
    if !target.starts_with(&run_canonical) || !target.starts_with(&root_canonical) {
        return Err(ExecutorError::Other("worktree target escapes configured Run root".into()));
    }
    if !target.is_dir() {
        return Err(ExecutorError::Other("worktree target is not a directory".into()));
    }
    Ok(Some(target))
}

/// Resolve a mirror only beneath the canonical git-mirror directory. The
/// per-repository lock must already be held by the caller.
fn resolve_repository_mirror_target(
    root: &Path,
    repository_id: Uuid,
) -> Result<Option<PathBuf>, ExecutorError> {
    let root_canonical = std::fs::canonicalize(root)
        .map_err(|e| ExecutorError::Other(format!("canonicalize workspace root failed: {e}")))?;
    let mirror_root = root.join("git-mirror");
    let root_meta = match std::fs::symlink_metadata(&mirror_root) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ExecutorError::Other(format!("inspect mirror root failed: {error}"))),
    };
    if root_meta.file_type().is_symlink() {
        return Err(ExecutorError::Other("git-mirror root is a symlink; refusing reclamation".into()));
    }
    let mirror_root_canonical = std::fs::canonicalize(&mirror_root)
        .map_err(|e| ExecutorError::Other(format!("canonicalize mirror root failed: {e}")))?;
    if !mirror_root_canonical.starts_with(&root_canonical) {
        return Err(ExecutorError::Other("git-mirror root escapes workspace root".into()));
    }
    let mirror = mirror_root.join(repository_id.to_string());
    let meta = match std::fs::symlink_metadata(&mirror) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ExecutorError::Other(format!("inspect mirror failed: {error}"))),
    };
    if meta.file_type().is_symlink() {
        return Err(ExecutorError::Other("repository mirror is a symlink; refusing reclamation".into()));
    }
    let canonical = std::fs::canonicalize(&mirror)
        .map_err(|e| ExecutorError::Other(format!("canonicalize repository mirror failed: {e}")))?;
    if !canonical.starts_with(&mirror_root_canonical) || !canonical.starts_with(&root_canonical) {
        return Err(ExecutorError::Other("repository mirror escapes configured mirror root".into()));
    }
    if !canonical.is_dir() || !canonical.join("HEAD").is_file() {
        return Err(ExecutorError::Other("repository mirror is not a bare Git repository".into()));
    }
    Ok(Some(canonical))
}

/// 镜像锁 RAII：Drop 时删除锁文件。
struct MirrorLockGuard {
    path: PathBuf,
}

impl Drop for MirrorLockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

// ---------------------------------------------------------------------------
// T-032 §7：CodexLaunchProfile（每 Run 启动配置；AI_PM_CODEX_ARGS 降级为覆盖项）
// ---------------------------------------------------------------------------

/// 每 Run 生成的启动配置：结构化收编 executable / arguments / workdir / env /
/// MCP 配置 / instruction 文件 / 策略；序列化（脱敏后）落 Run 目录供审计。
#[derive(Debug, Clone)]
pub struct CodexLaunchProfile {
    pub executable: String,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
    /// 注入执行端的环境变量（含一次性凭据 `AI_PM_LEASE_TOKEN`——**禁止落盘**）。
    pub environment: Vec<(String, String)>,
    pub mcp_config_path: Option<PathBuf>,
    pub instruction_file: PathBuf,
    pub approval_policy: String,
    pub sandbox_policy: String,
    pub timeout_policy_secs: Option<i32>,
}

impl CodexLaunchProfile {
    pub fn build(
        run: &ExecutableRun,
        session: &str,
        root: &Path,
        bin: String,
        extra_args: Vec<String>,
    ) -> Self {
        let run_root = root.join(run.run_id.to_string());
        let working_directory = run
            .repositories
            .first()
            .map(|b| run_root.join(&b.workspace_path))
            .unwrap_or_else(|| run_root.clone());

        let mut environment = vec![
            ("AI_PM_RUN_ID".to_string(), run.run_id.to_string()),
            ("AI_PM_TASK_ID".to_string(), run.task_id.to_string()),
            ("AI_PM_PROJECT_ID".to_string(), run.project_id.to_string()),
            ("AI_PM_ATTEMPT_NO".to_string(), run.attempt_no.to_string()),
            (
                "AI_PM_WORKSPACE_ROOT".to_string(),
                root.to_string_lossy().to_string(),
            ),
            ("AI_PM_EXECUTOR_SESSION".to_string(), session.to_string()),
            (
                "AI_PM_LEASE_GENERATION".to_string(),
                run.lease_generation.to_string(),
            ),
        ];
        // ADR-0002 L2：一次性租约明文只经环境变量下发（凭据链路统一）。
        if let Some(token) = &run.lease_token {
            environment.push(("AI_PM_LEASE_TOKEN".to_string(), token.clone()));
        }

        // D2 工具面接入配置（占位：URL 由部署注入；凭据经环境变量，不进配置文件）。
        let mcp_config_path = std::env::var("AI_PM_MCP_URL").ok().map(|url| {
            let path = run_root.join("mcp.json");
            let _ = std::fs::write(
                &path,
                serde_json::to_vec_pretty(&serde_json::json!({
                    "mcpServers": { "ai-pm": { "url": url } }
                }))
                .unwrap_or_default(),
            );
            path
        });

        // instruction 文件：指向契约（不复制内容——契约事实源在库，经 MCP 拉取）。
        let instruction_file = run_root.join("instruction.md");
        let _ = std::fs::write(
            &instruction_file,
            format!(
                "# Task Execution Brief\n\n\
                 - run_id: {run_id}\n- attempt_no: {attempt}\n- lease_generation: {gen}\n\n\
                 1. 调用 MCP 工具 `pm_get_run_context` 获取完整任务契约（目标 / 约束 / \
                 逐条验收 / 产物路径 / 每仓基线 / 知识引用）。\n\
                 2. 执行期间按心跳间隔调用 `pm_report_progress`（stage 变化即进展）。\n\
                 3. 完成后调用 `pm_submit_result`；阻塞时 report blocked。\n\
                 4. 若产出可追溯证据（代码位置 / 测试结果 / 提交锚点），调用 \
                 `pm_submit_evidence` 挂到相关发现上——代码类证据必须携带 \
                 import_id / repository_id / commit_sha / attempt_no 四列。\n",
                run_id = run.run_id,
                attempt = run.attempt_no,
                gen = run.lease_generation,
            ),
        );

        Self {
            executable: bin,
            arguments: extra_args,
            working_directory,
            environment,
            mcp_config_path,
            instruction_file,
            approval_policy: std::env::var("AI_PM_CODEX_APPROVAL_POLICY")
                .unwrap_or_else(|_| "never".to_string()),
            sandbox_policy: std::env::var("AI_PM_CODEX_SANDBOX_POLICY")
                .unwrap_or_else(|_| "workspace-write".to_string()),
            timeout_policy_secs: run.timeout_seconds,
        }
    }

    /// 审计摘要（**脱敏**：environment 不落盘——含一次性凭据）。
    fn audit_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "executable": self.executable,
            "arguments": self.arguments,
            "working_directory": self.working_directory.to_string_lossy(),
            "mcp_config": self.mcp_config_path.as_ref().map(|p| p.to_string_lossy()),
            "instruction_file": self.instruction_file.to_string_lossy(),
            "approval_policy": self.approval_policy,
            "sandbox_policy": self.sandbox_policy,
            "timeout_policy_secs": self.timeout_policy_secs,
        })
    }
}

#[async_trait::async_trait]
impl RunExecutor for CodexExecutor {
    fn kind(&self) -> &'static str {
        "codex"
    }

    /// 建工作区 → 逐仓 checkout（路径校验 + 镜像锁）→ 生成 Launch Profile →
    /// spawn（Job Object / 进程组）→ pid 文件原子写 + 日志脱敏管道。
    /// 任一步失败即 `ExecutorError`，编排器按基础设施失败退避（session 保留）。
    async fn start(&self, run: &ExecutableRun, session: &str) -> Result<(), ExecutorError> {
        let run_root = self.run_root(run.run_id);
        std::fs::create_dir_all(run_root.join("logs"))
            .map_err(|e| ExecutorError::Other(format!("create run dir failed: {e}")))?;

        // §6.2：workspace_path 二次校验（DB 值防御层；算法侧已安全）。
        for baseline in &run.repositories {
            let safe = validate_workspace_path(&baseline.workspace_path)?;
            let target = run_root.join(&safe);
            if target != run_root.join(&baseline.workspace_path) {
                return Err(ExecutorError::Other(format!(
                    "workspace_path normalization mismatch: {}",
                    baseline.workspace_path
                )));
            }
            self.checkout(
                run.project_id,
                baseline.repository_id,
                &baseline.base_commit,
                &target,
            )
            .await?;
        }

        let profile = CodexLaunchProfile::build(
            run,
            session,
            &self.root,
            self.bin.clone(),
            self.extra_args.clone(),
        );
        append_executor_event(
            &self.root.to_string_lossy(),
            run.run_id,
            &serde_json::json!({
                "event": "launch_profile",
                "session": session,
                "profile": profile.audit_summary(),
                "at": chrono::Utc::now().to_rfc3339(),
            }),
        );

        // 日志管道：子进程 stdout/stderr 经脱敏（token → [REDACTED]）后落盘，
        // 带大小上限（超限丢弃写入，防磁盘耗尽——§6.3）。日志文件在本方法内
        // **同步创建**（Run 目录布局是启动事实的一部分；管道 task 只 append）。
        let stdout_log = run_root.join("logs").join("stdout.log");
        let stderr_log = run_root.join("logs").join("stderr.log");
        for log in [&stdout_log, &stderr_log] {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)
                .map_err(|e| ExecutorError::Other(format!("create log file failed: {e}")))?;
        }

        let mut cmd = tokio::process::Command::new(&profile.executable);
        cmd.args(&profile.arguments)
            .current_dir(&profile.working_directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        for (key, value) in &profile.environment {
            cmd.env(key, value);
        }
        // §5：进程组隔离（Unix）——子进程成为新组长，terminate 可 kill(-pgid)
        // 全量回收 Cargo / Git / Shell。
        #[cfg(unix)]
        cmd.process_group(0);

        let mut child = cmd.spawn().map_err(|e| {
            ExecutorError::Other(format!("spawn {} failed: {e}", profile.executable))
        })?;
        let pid = child.id().ok_or_else(|| {
            ExecutorError::Other("spawned child has no pid (already reaped)".into())
        })?;

        // §5 Windows：spawn 后立即挂入 Job Object（KILL_ON_JOB_CLOSE）——
        // 主进程死亡/适配器退出时子进程树级联终止。
        #[cfg(windows)]
        let job = {
            let job = job_object::JobHandle::create()
                .map_err(|e| ExecutorError::Other(format!("create job object failed: {e}")))?;
            job.assign(pid)
                .map_err(|e| ExecutorError::Other(format!("assign to job object failed: {e}")))?;
            job
        };

        // pid 文件原子写（T1 协议第二段：进程身份持久化）；token 明文绝不入盘。
        // identity_hint：进程身份锚点——Unix = /proc/<pid>/stat starttime（精确）；
        // Windows = StartTime.Ticks（PowerShell，写文件前的近似创建时刻）。
        // PID 重用后锚点必不一致 → reconcile 判原进程已死，绝不误认无辜进程。
        let identity_hint =
            process_identity_hint(pid).unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        let pid_payload = serde_json::json!({
            "pid": pid,
            "session": session,
            "identity_hint": identity_hint,
            "recorded_at": chrono::Utc::now().to_rfc3339(),
        });
        write_pid_file_atomic(&self.pid_file(run.run_id), &pid_payload)
            .map_err(|e| ExecutorError::Other(format!("write pid file failed: {e}")))?;

        // 日志管道：子进程 stdout/stderr 经脱敏（token → [REDACTED]）后落盘，
        // 带大小上限（超限丢弃写入，防磁盘耗尽——§6.3）。
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(pump_log(
                stdout,
                stdout_log,
                run.lease_token.clone(),
                self.log_max_bytes,
            ));
        }
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(pump_log(
                stderr,
                stderr_log,
                run.lease_token.clone(),
                self.log_max_bytes,
            ));
        }

        self.children.lock().expect("children mutex").insert(
            session.to_string(),
            ChildHandle {
                child,
                pid,
                #[cfg(windows)]
                job,
            },
        );

        append_executor_event(
            &self.root.to_string_lossy(),
            run.run_id,
            &serde_json::json!({
                "event": "spawn_ok",
                "session": session,
                "pid": pid,
                "at": chrono::Utc::now().to_rfc3339(),
            }),
        );
        Ok(())
    }

    /// 存活探测：内存句柄优先（本进程 spawn 的）；跨重启孤儿按 **T1 三重证据**
    /// 判定——pid 文件（含身份锚点比对，防 PID 重用误判）/ run 目录活跃痕迹 /
    /// Job 或进程组。无法确证进程已死 → `Uncertain`（不得静默回收）。
    async fn poll(&self, session: &ExecutorSession) -> Result<ExecutorProgress, ExecutorError> {
        enum Step {
            Progress(ExecutorProgress),
            Reaped(std::process::ExitStatus),
        }
        let step = {
            let mut guard = self.children.lock().expect("children mutex");
            // 先在内层借用里 try_wait，再按结果 remove（NLL：handle 借用在读取
            // ExitStatus 后即结束）。
            let reaped: Option<Option<std::process::ExitStatus>> = match guard
                .get_mut(&session.session)
            {
                Some(handle) => match handle.child.try_wait() {
                    Ok(Some(status)) => Some(Some(status)),
                    Ok(None) => Some(None),
                    Err(e) => return Err(ExecutorError::Other(format!("try_wait failed: {e}"))),
                },
                None => None,
            };
            match reaped {
                Some(Some(status)) => {
                    // Windows：收割后立即终止 Job（回收残余子进程）。
                    if let Some(handle) = guard.remove(&session.session) {
                        #[cfg(windows)]
                        handle.job.terminate();
                    }
                    Step::Reaped(status)
                }
                Some(None) => Step::Progress(ExecutorProgress::Running),
                // 跨重启孤儿：三重证据判定。
                None => {
                    let record = read_pid_record(self.pid_file(session.run_id).as_path())
                        .map_err(|e| ExecutorError::Other(e.to_string()))?;
                    match record {
                        Some(rec) => {
                            if !process_identity_matches(&rec) {
                                // 身份锚点不一致 = PID 已被操作系统重用：确证原进程已死。
                                Step::Progress(ExecutorProgress::Done {
                                    outcome: ExitOutcome::Lost,
                                })
                            } else if process_alive(rec.pid) {
                                Step::Progress(ExecutorProgress::Running)
                            } else {
                                Step::Progress(ExecutorProgress::Done {
                                    outcome: ExitOutcome::Lost,
                                })
                            }
                        }
                        None => {
                            // pid 文件缺失 ≠ 进程不存在（T1 强制约束）：
                            // run 目录曾初始化 → 证据不足 → Uncertain；
                            // 目录都不存在 → 从未 spawn → 确证不存在。
                            if self.run_root(session.run_id).join("logs").exists() {
                                Step::Progress(ExecutorProgress::Uncertain)
                            } else {
                                Step::Progress(ExecutorProgress::Done {
                                    outcome: ExitOutcome::Lost,
                                })
                            }
                        }
                    }
                }
            }
        };
        Ok(match step {
            Step::Progress(progress) => progress,
            Step::Reaped(status) => ExecutorProgress::Done {
                outcome: exit_outcome_of(status),
            },
        })
    }

    /// 统一终止管线（Q-D3-5 + §5）：先探测已退出；存活进程——
    /// Windows `TerminateJobObject`（整棵树）；Unix `kill(-pgid, SIGTERM)` →
    /// 宽限 → `kill(-pgid, SIGKILL)`；孤儿路径按 pid 文件强杀（含树）。
    async fn terminate(
        &self,
        session: &ExecutorSession,
        reason: TerminateReason,
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
            append_executor_event(
                &self.root.to_string_lossy(),
                session.run_id,
                &serde_json::json!({
                    "event": "terminated",
                    "session": session.session,
                    "reason": format!("{reason:?}"),
                    "at": chrono::Utc::now().to_rfc3339(),
                }),
            );
            return Ok(());
        }
        // 跨重启孤儿：pid 文件定位 + 身份比对（防 PID 重用误杀无辜进程）。
        if let Some(rec) = read_pid_record(self.pid_file(session.run_id).as_path())
            .map_err(wrap_io)?
            .filter(|rec| process_identity_matches(rec) && process_alive(rec.pid))
        {
            kill_by_pid(rec.pid).map_err(wrap_io)?;
        }
        append_executor_event(
            &self.root.to_string_lossy(),
            session.run_id,
            &serde_json::json!({
                "event": "terminated",
                "session": session.session,
                "reason": format!("{reason:?}"),
                "orphan": true,
                "at": chrono::Utc::now().to_rfc3339(),
            }),
        );
        Ok(())
    }
}

fn exit_outcome_of(status: std::process::ExitStatus) -> ExitOutcome {
    match status.code() {
        Some(0) => ExitOutcome::Success,
        Some(code) => ExitOutcome::NonZeroExit { code },
        None => {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                match status.signal() {
                    Some(signal) => ExitOutcome::Signaled { signal },
                    None => ExitOutcome::Lost,
                }
            }
            #[cfg(not(unix))]
            {
                ExitOutcome::Lost
            }
        }
    }
}

fn wrap_io(e: std::io::Error) -> ExecutorError {
    ExecutorError::Other(e.to_string())
}

// ---------------------------------------------------------------------------
// pid 文件（原子写 + 身份锚点）与跨平台进程证据
// ---------------------------------------------------------------------------

/// pid 文件记录（`process.json`）：token 明文**绝不入内**。
#[derive(Debug, Clone)]
struct PidRecord {
    pid: u32,
    /// 会话引用（事件与人工排查用；存活判定走 pid + 身份锚点）。
    #[allow(dead_code)]
    session: String,
    /// 身份锚点：Unix = /proc/<pid>/stat 的 starttime（精确）；Windows = 写入
    /// 时刻（近似——跨重启比对时目标进程创建时间必须不早于它，否则判 PID 重用）。
    identity_hint: String,
}

fn write_pid_file_atomic(path: &Path, payload: &serde_json::Value) -> Result<(), std::io::Error> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec(payload).expect("pid payload serializes"),
    )?;
    // Windows rename 覆盖已有文件的语义由 MoveFileEx(REPLACE_EXISTING) 提供；
    // std::fs::rename 在同卷目标存在时 Windows 会失败——先删再 rename 的窗口由
    // 「tmp 已就绪」缩小（半写文件永远不会被读到）。
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    std::fs::rename(&tmp, path)
}

fn read_pid_record(path: &Path) -> Result<Option<PidRecord>, std::io::Error> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let pid = value.get("pid").and_then(|p| p.as_u64()).map(|p| p as u32);
    let Some(pid) = pid else {
        return Ok(None);
    };
    let session = value
        .get("session")
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string();
    let identity_hint = value
        .get("identity_hint")
        .and_then(|s| s.as_str())
        .or_else(|| value.get("recorded_at").and_then(|s| s.as_str()))
        .unwrap_or_default()
        .to_string();
    Ok(Some(PidRecord {
        pid,
        session,
        identity_hint,
    }))
}

/// 当前进程身份锚点：Unix 精确（/proc/<pid>/stat field 22 starttime）；
/// Windows = `Get-Process` StartTime.Ticks（低频路径——仅孤儿 reconcile /
/// spawn 记录时调用）。
fn process_identity_hint(pid: u32) -> Option<String> {
    #[cfg(unix)]
    {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // field 22 (starttime)：comm 可含空格，以最后一个 ')' 之后切分。
        let after_comm = stat.rsplit(')').next()?;
        after_comm.split_whitespace().nth(19).map(str::to_string)
    }
    #[cfg(windows)]
    {
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "(Get-Process -Id {pid} -ErrorAction SilentlyContinue).StartTime.Ticks.ToString()"
                ),
            ])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

/// 身份比对：锚点可取且一致 → 同一进程；Windows（无锚点）→ 放行为可能同一
/// 进程（PID 重用残余风险由 Job Object 兜底：句柄挂接的是原进程树）。
fn process_identity_matches(record: &PidRecord) -> bool {
    match (
        process_identity_hint(record.pid),
        record.identity_hint.is_empty(),
    ) {
        (Some(current), false) => current == record.identity_hint,
        _ => true,
    }
}

/// 跨平台进程存活判定（不引入重型依赖）。
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

/// 无句柄时的跨平台强杀（孤儿路径；含进程树 / 进程组）。
fn kill_by_pid(pid: u32) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        unsafe {
            // setsid 后 pgid == pid：负值 kill 全组（Cargo/Git/Shell 全量回收）。
            if libc::kill(-(pid as i32), libc::SIGKILL) != 0
                && libc::kill(pid as i32, libc::SIGKILL) != 0
            {
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

/// Unix 优雅终止（进程组）：SIGTERM → 宽限轮询 → SIGKILL。Windows 无此语义。
#[cfg(unix)]
async fn graceful_stop(handle: &mut ChildHandle, grace_secs: u64) -> Result<(), ExecutorError> {
    unsafe {
        libc::kill(-(handle.pid as i32), libc::SIGTERM);
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
    unsafe {
        libc::kill(-(handle.pid as i32), libc::SIGKILL);
    }
    handle.child.wait().await.map_err(wrap_io)?;
    Ok(())
}

#[cfg(windows)]
async fn graceful_stop(handle: &mut ChildHandle, _grace_secs: u64) -> Result<(), ExecutorError> {
    // T-032 §5：TerminateJobObject 一次性终止整棵进程树（替代只杀主进程的
    // start_kill）；随后收割主进程。
    handle.job.terminate();
    handle.child.wait().await.map_err(wrap_io)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 日志管道（§6.3）：脱敏 + 大小上限
// ---------------------------------------------------------------------------

/// 子进程输出 → 脱敏（一次性 token 替换）→ 落盘（带大小上限；超限后继续读
/// 但丢弃，防管道背压阻塞子进程）。**按行缓冲**——凭据明文不跨行，行内替换
/// 保证无跨块切割问题。
async fn pump_log<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    path: PathBuf,
    token: Option<String>,
    max_bytes: u64,
) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    let file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await;
    let mut file = match file {
        Ok(f) => f,
        Err(_) => return, // 日志文件打不开：放弃落盘，事件流仍可观测
    };
    let mut reader = tokio::io::BufReader::new(reader);
    let secret = token.map(|t| t.into_bytes());
    let mut line: Vec<u8> = Vec::with_capacity(1024);
    let mut truncated = false;
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if let Some(secret) = &secret {
                    redact(&mut line, secret);
                }
                if truncated {
                    continue; // 已超限：读而不写（防背压）
                }
                let len = tokio::fs::metadata(&path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0);
                if len >= max_bytes {
                    let _ = file
                        .write_all(b"\n[TRUNCATED: log size limit reached]\n")
                        .await;
                    truncated = true;
                    continue;
                }
                let _ = file.write_all(&line).await;
            }
        }
    }
}

/// 行内替换一次性凭据明文（`[REDACTED]` 与凭据等长不等——重建缓冲）。
fn redact(buf: &mut Vec<u8>, secret: &[u8]) {
    if secret.is_empty() || buf.len() < secret.len() {
        return;
    }
    let mut out = Vec::with_capacity(buf.len());
    let mut i = 0;
    while i < buf.len() {
        if i + secret.len() <= buf.len() && &buf[i..i + secret.len()] == secret {
            out.extend_from_slice(b"[REDACTED]");
            i += secret.len();
        } else {
            out.push(buf[i]);
            i += 1;
        }
    }
    *buf = out;
}

// ---------------------------------------------------------------------------
// §5：Windows Job Object（KILL_ON_JOB_CLOSE）——子进程树级联终止的平台原语
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod job_object {
    use std::mem::size_of;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };

    /// Job 句柄 RAII：Drop（executor 销毁 / 子进程收割后 take 走的 handle 离开
    /// 作用域）即 `KILL_ON_JOB_CLOSE` 级联终止整棵进程树。
    pub struct JobHandle {
        handle: HANDLE,
    }

    // Job 句柄可跨线程转移（tokio spawn 的日志任务不持有它；terminate 在任意
    // worker 线程调用）。
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
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
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

#[cfg(test)]
mod imp008_tests {
    use super::*;

    /// PA07-IMP-008：ssh url 落 RAII 临时密钥文件，守卫存续期间文件存在、
    /// drop 后整个临时目录删除——不再向共享 `keys/` 明文残留。
    #[test]
    fn ssh_key_file_lives_only_as_long_as_the_guard() {
        let (command, key) = ssh_command_with_key("git@host:org/repo.git", "PRIVATE-KEY-BODY")
            .expect("ssh url must produce a GIT_SSH_COMMAND");
        assert!(
            command.contains("ssh -i "),
            "command embeds the temp key path"
        );
        assert!(command.contains("StrictHostKeyChecking=accept-new"));
        let (dir, path) = (key.dir().to_path_buf(), key.key_path());
        assert!(path.exists(), "key file must exist while the guard lives");
        let body = std::fs::read_to_string(&path).expect("key file readable");
        assert_eq!(body, "PRIVATE-KEY-BODY");
        assert!(
            !path.starts_with(std::path::Path::new(".").join("keys")),
            "must not write into the shared workspace keys/ dir"
        );
        drop(key);
        assert!(
            !dir.exists(),
            "guard drop must remove the temp dir (no plaintext residue)"
        );
    }

    /// 非 ssh url（https）不落任何密钥文件。
    #[test]
    fn https_url_never_writes_a_key_file() {
        assert!(ssh_command_with_key("https://host/org/repo.git", "PRIVATE").is_none());
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("pa08-{label}-{}", Uuid::new_v4()))
    }

    #[test]
    fn worktree_path_resolves_only_inside_its_run_root() {
        let root = temp_root("worktree-valid");
        let run_id = Uuid::new_v4();
        let target = root.join(run_id.to_string()).join("repos/repo");
        std::fs::create_dir_all(&target).unwrap();

        let resolved = resolve_run_worktree_target(&root, run_id, "repos/repo")
            .unwrap()
            .expect("existing worktree resolves");
        assert_eq!(resolved, std::fs::canonicalize(&target).unwrap());

        assert!(resolve_run_worktree_target(&root, run_id, "../../outside").is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn missing_worktree_is_an_idempotent_noop() {
        let root = temp_root("worktree-missing");
        std::fs::create_dir_all(&root).unwrap();
        assert!(resolve_run_worktree_target(&root, Uuid::new_v4(), "repos/missing")
            .unwrap()
            .is_none());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn worktree_symlink_escape_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = temp_root("worktree-symlink");
        let outside = temp_root("worktree-outside");
        let run_id = Uuid::new_v4();
        std::fs::create_dir_all(root.join(run_id.to_string()).join("repos")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join(run_id.to_string()).join("repos/escape")).unwrap();

        assert!(resolve_run_worktree_target(&root, run_id, "repos/escape").is_err());
        std::fs::remove_dir_all(&root).unwrap();
        std::fs::remove_dir_all(&outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn mirror_symlink_escape_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = temp_root("mirror-symlink");
        let outside = temp_root("mirror-outside");
        let repository_id = Uuid::new_v4();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("git-mirror")).unwrap();

        assert!(resolve_repository_mirror_target(&root, repository_id).is_err());
        std::fs::remove_dir_all(&root).unwrap();
        std::fs::remove_dir_all(&outside).unwrap();
    }
}
