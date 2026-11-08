//! D4 的 git 事实校验（`docs/ai-pm-slice-d4-delivery-v1.md` §3.3 §3.4 §3.6）。
//!
//! 校验目标是**当前 Run 的实际 Git 工作区**（`{workspace_base}/{run_id}/
//! {workspace_path}`），不是镜像仓——执行器在 Worktree 中新产生的 Commit 不假定
//! 镜像仓天然可见（评审收编 4.3）。
//!
//! 四项校验：
//!
//! 1. `${commit_sha}^{commit}` —— 对象存在且是 Commit 类型；
//! 2. `merge-base --is-ancestor <base_commit> <commit_sha>` —— 是基线后代；
//! 3. `rev-parse HEAD` —— **必须等于工作区实际 HEAD**（提交别的 Run 已有的后代
//!    Commit 同样要拒）；
//! 4. `diff --name-status <base>..<commit>` —— 现算变更文件数（Q-D4-3）。
//!
//! 约束（§3.4）：明确超时、禁网络、限制规模；异常与超时统一走 Rejected 路径。

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::git_access::{self, GitError};

use super::SubmittedRepository;

/// git 校验的失败原因（映射到 D4 错误码）。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GitVerificationError {
    /// 工作区不存在或不可读。
    #[error("workspace unavailable: {0}")]
    WorkspaceUnavailable(String),
    /// commit 不存在 / 非 Commit 类型 / 非基线后代 / ≠ 工作区 HEAD。
    #[error("commit_not_descendant: {0}")]
    NotDescendant(String),
    /// git 命令失败或超时。
    #[error("git_verification_failed: {0}")]
    Git(String),
}

impl GitVerificationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::WorkspaceUnavailable(_) => "workspace_unavailable",
            Self::NotDescendant(_) => "commit_not_descendant",
            Self::Git(_) => "git_verification_failed",
        }
    }
}

/// 一仓的 git 事实（校验通过后的结果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitVerification {
    /// 工作区实际 HEAD（权威值——落库的 `commit_sha` 取它，不取执行端声明）。
    pub head_sha: String,
    /// 当前分支（`HEAD` 处于游离态时为 `None`）。
    pub branch: Option<String>,
    /// Git 现算的变更文件数。
    pub changed_files: i32,
    /// commit 作者。
    pub author: Option<String>,
    /// commit 作者时间。
    pub authored_at: Option<DateTime<Utc>>,
}

/// Run 工作区根：`{workspace_base}/{run_id}`（与 D3 派生规则一致）。
pub fn workspace_root_for_run(workspace_base: &Path, run_id: uuid::Uuid) -> PathBuf {
    workspace_base.join(run_id.to_string())
}

/// 校验一仓的工作区事实。
///
/// `submitted` 为 `None` 表示该仓是只读仓——此时只读出 HEAD 供调用方与基线比对，
/// 不做后代/计数校验（只读仓有变化即整单驳回，由调用方判定）。
pub async fn verify_repository_head(
    workspace_root: &Path,
    workspace_path: &str,
    base_commit: &str,
    submitted: Option<&SubmittedRepository>,
    timeout: Duration,
) -> Result<GitVerification, GitVerificationError> {
    let root = workspace_root.join(workspace_path);
    if !root.join(".git").exists() && !root.join("HEAD").exists() {
        return Err(GitVerificationError::WorkspaceUnavailable(format!(
            "workspace {workspace_path} is not a git worktree"
        )));
    }

    let head_sha = git_text(&root, &["rev-parse", "HEAD"], timeout).await?;
    let branch = git_text(&root, &["rev-parse", "--abbrev-ref", "HEAD"], timeout)
        .await
        .ok()
        .filter(|value| value != "HEAD");

    let Some(submitted) = submitted else {
        // 只读仓：不做后代判定，把「实际 HEAD」交给调用方与基线比对。
        return Ok(GitVerification {
            head_sha,
            branch,
            changed_files: 0,
            author: None,
            authored_at: None,
        });
    };

    let commit_sha = &submitted.commit_sha;

    // ① 对象存在且是 Commit 类型。
    let type_check = git_run(&root, &["cat-file", "-t", commit_sha], timeout).await?;
    if !type_check.ok() || type_check.stdout_text() != "commit" {
        return Err(GitVerificationError::NotDescendant(format!(
            "{commit_sha} is not an existing commit object"
        )));
    }

    // ② 是基线后代（退出码表达「是/否」，不是错误输出）。
    let ancestry = git_run(
        &root,
        &["merge-base", "--is-ancestor", base_commit, commit_sha],
        timeout,
    )
    .await?;
    if !ancestry.ok() {
        return Err(GitVerificationError::NotDescendant(format!(
            "{commit_sha} is not a descendant of {base_commit}"
        )));
    }

    // ③ 必须等于工作区实际 HEAD。
    if head_sha != *commit_sha {
        return Err(GitVerificationError::NotDescendant(format!(
            "{commit_sha} is not the workspace HEAD ({head_sha})"
        )));
    }

    // ④ 变更文件数现算（Q-D4-3：Git 现算为准）。
    let diff = git_run(
        &root,
        &[
            "diff",
            "--name-status",
            &format!("{base_commit}..{commit_sha}"),
        ],
        timeout,
    )
    .await?;
    let changed_files = diff
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .count() as i32;

    let author = git_text(&root, &["show", "-s", "--format=%an", commit_sha], timeout)
        .await
        .ok();
    let authored_at = git_text(&root, &["show", "-s", "--format=%aI", commit_sha], timeout)
        .await
        .ok()
        .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
        .map(|value| value.with_timezone(&Utc));

    Ok(GitVerification {
        head_sha,
        branch,
        changed_files,
        author,
        authored_at,
    })
}

async fn git_text(
    root: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<String, GitVerificationError> {
    let output = git_run(root, args, timeout).await?;
    if !output.ok() {
        return Err(GitVerificationError::Git(format!(
            "git {} failed: {}",
            args.join(" "),
            output.stderr
        )));
    }
    Ok(output.stdout_text())
}

async fn git_run(
    root: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<git_access::GitOutput, GitVerificationError> {
    git_access::git_run(root, args, timeout)
        .await
        .map_err(|e| GitVerificationError::Git(map_git_error(e)))
}

fn map_git_error(error: GitError) -> String {
    match error {
        GitError::Timeout => "git command timed out".to_string(),
        GitError::AuthFailed => "git authentication failed".to_string(),
        GitError::NotFound => "git object not found".to_string(),
        GitError::TooLarge => "repository is too large".to_string(),
        GitError::Other(message) => message,
        GitError::Io(message) => message,
    }
}
