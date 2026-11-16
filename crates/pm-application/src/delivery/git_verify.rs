//! D4 的 git 事实校验（`docs/slice-d/ai-pm-slice-d4-delivery-v1.md` §3.3 §3.4 §3.6）。
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

// ---------------------------------------------------------------------------
// 远端模式（V-05-2 / 检查点 2 Q-RC-1）：申报 SHA 的平台只读验证
// ---------------------------------------------------------------------------

/// 判定一次提交是否走**远端验证**：lease_owner 为 cpk 通道稳定 Client 身份
/// （`client:{device_id}`）时，Run 工作区在用户 PC 上，平台无工作区可查——
/// 申报 SHA 只能对照远端验证。Worker（服务器工作区）与 Model A 真人
/// （`mcp:` 形态，含 `runner:` 过渡）维持工作区模式不变。
pub fn is_remote_carrier(lease_owner: Option<&str>) -> bool {
    lease_owner.is_some_and(|owner| owner.starts_with("client:"))
}

/// 远端只读验证一仓的申报 SHA（白名单命令组合，不扩命令面）：
///
/// 1. `ls-remote --heads`：声明 sha 必须作为**某个远端分支 head** 可见
///    （Agent 正常 push 的自然形态；只见于本地、从未推送的申报在此拒收）；
/// 2. 临时 bare clone（`clone` 在白名单内）→ `git log {sha}..{base}` 输出为空
///    ⟺ base 全部可达于 sha（对齐 `merge-base --is-ancestor` 的含相等语义，
///    分叉/落后均非空 → 拒）；
/// 3. author / authored_at 经 `git show` 现算；changed_files 远端无工作区可
///    diff → 取申报值回显（一致性比对天然通过，诚实注记由审计承载，不伪造现算）。
///
/// 临时 clone 用完即删；密钥材料由调用方经 [`git_access::write_temp_key`]
/// 提供并自行 RAII 释放。
pub async fn verify_repository_head_remote(
    url: &str,
    key: Option<&Path>,
    base_commit: &str,
    commit_sha: &str,
    changed_files: i32,
    timeout: Duration,
) -> Result<GitVerification, GitVerificationError> {
    // ① 申报 sha 必须是某个远端分支的 head。
    let heads = git_access::ls_remote(url, key, timeout)
        .await
        .map_err(|e| GitVerificationError::Git(map_git_error(e)))?;
    let branch = heads
        .iter()
        .find(|head| head.sha.eq_ignore_ascii_case(commit_sha))
        .map(|head| head.branch.clone())
        .ok_or_else(|| {
            GitVerificationError::NotDescendant(format!(
                "declared commit {commit_sha} is not visible as any remote branch head"
            ))
        })?;

    // ② 临时 bare clone + 基线可达性（`log {sha}..{base}` 为空 ⟺ 后代或相等）。
    let staging = std::env::temp_dir().join(format!("ai-pm-verify-{}", uuid::Uuid::new_v4()));
    git_access::clone_bare(url, key, &staging, timeout)
        .await
        .map_err(|e| GitVerificationError::Git(map_git_error(e)))?;
    let removal = staging.clone();
    let result = async {
        let containment = git_run(
            &staging,
            &[
                "log",
                "--format=%H",
                "-n",
                "1",
                &format!("{commit_sha}..{base_commit}"),
            ],
            timeout,
        )
        .await?;
        if !containment.ok() {
            return Err(GitVerificationError::NotDescendant(format!(
                "baseline {base_commit} is not reachable on the remote (git log failed: {})",
                containment.stderr
            )));
        }
        if !containment.stdout_text().is_empty() {
            return Err(GitVerificationError::NotDescendant(format!(
                "{commit_sha} is not a descendant of {base_commit}"
            )));
        }
        let author = git_text(
            &staging,
            &["show", "-s", "--format=%an", commit_sha],
            timeout,
        )
        .await
        .ok();
        let authored_at = git_text(
            &staging,
            &["show", "-s", "--format=%aI", commit_sha],
            timeout,
        )
        .await
        .ok()
        .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
        .map(|value| value.with_timezone(&Utc));
        Ok(GitVerification {
            head_sha: commit_sha.to_string(),
            branch: Some(branch),
            changed_files,
            author,
            authored_at,
        })
    }
    .await;
    let _ = std::fs::remove_dir_all(&removal);
    result
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
        GitError::Network(message) => message,
        GitError::TooLarge => "repository is too large".to_string(),
        GitError::Other(message) => message,
        GitError::Io(message) => message,
    }
}

#[cfg(test)]
mod remote_tests {
    use super::*;

    struct RemoteFixture {
        root: PathBuf,
        base_commit: String,
        commit_sha: String,
        url: String,
    }

    /// 本地 bare origin + 工作仓：main=base；feature 分支=base 后代（模拟
    /// Agent push 交付）；`work` 仓留在 root 下供孤儿分支用例追加操作。
    fn setup_remote() -> RemoteFixture {
        let root =
            std::env::temp_dir().join(format!("ai-pm-remote-verify-{}", uuid::Uuid::new_v4()));
        let origin = root.join("origin.git");
        let work = root.join("work");
        let run = |args: &[&str], dir: &Path| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(dir)
                .env("GIT_AUTHOR_NAME", "Test")
                .env("GIT_AUTHOR_EMAIL", "test@example.com")
                .env("GIT_COMMITTER_NAME", "Test")
                .env("GIT_COMMITTER_EMAIL", "test@example.com")
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        };
        std::fs::create_dir_all(&origin).unwrap();
        run(&["init", "-q", "--bare", "-b", "main", "."], &origin);
        std::fs::create_dir_all(&work).unwrap();
        run(&["init", "-q", "-b", "main", "."], &work);
        std::fs::write(work.join("README.md"), "# base\n").unwrap();
        run(&["add", "."], &work);
        run(
            &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "base"],
            &work,
        );
        let base_commit = rev_head(&work);
        run(
            &[
                "-c",
                "protocol.file.allow=always",
                "push",
                "-q",
                "../origin.git",
                "main",
            ],
            &work,
        );
        run(&["checkout", "-q", "-b", "feature"], &work);
        std::fs::write(work.join("feature.txt"), "delivered\n").unwrap();
        run(&["add", "."], &work);
        run(
            &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "feat"],
            &work,
        );
        let commit_sha = rev_head(&work);
        run(
            &[
                "-c",
                "protocol.file.allow=always",
                "push",
                "-q",
                "../origin.git",
                "feature",
            ],
            &work,
        );
        let url = format!("file://{}", origin.to_string_lossy().replace('\\', "/"));
        RemoteFixture {
            root,
            base_commit,
            commit_sha,
            url,
        }
    }

    fn rev_head(repo: &Path) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[tokio::test]
    async fn remote_verify_accepts_pushed_descendant() {
        let fx = setup_remote();
        let verification = verify_repository_head_remote(
            &fx.url,
            None,
            &fx.base_commit,
            &fx.commit_sha,
            3,
            Duration::from_secs(30),
        )
        .await
        .unwrap();
        assert_eq!(verification.head_sha, fx.commit_sha);
        assert_eq!(verification.branch.as_deref(), Some("feature"));
        assert_eq!(verification.changed_files, 3);
        let _ = std::fs::remove_dir_all(fx.root);
    }

    #[tokio::test]
    async fn remote_verify_rejects_unpushed_declaration() {
        let fx = setup_remote();
        let ghost = format!("{:040x}", 0xabc);
        let err = verify_repository_head_remote(
            &fx.url,
            None,
            &fx.base_commit,
            &ghost,
            1,
            Duration::from_secs(30),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("not visible as any remote branch head"),
            "{err}"
        );
        let _ = std::fs::remove_dir_all(fx.root);
    }

    #[tokio::test]
    async fn remote_verify_rejects_non_descendant() {
        // 孤儿分支（无 base 祖先）推送后申报 → 可达性校验拒绝。
        let fx = setup_remote();
        let work = fx.root.join("work");
        let run = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(&work)
                .env("GIT_AUTHOR_NAME", "Test")
                .env("GIT_AUTHOR_EMAIL", "test@example.com")
                .env("GIT_COMMITTER_NAME", "Test")
                .env("GIT_COMMITTER_EMAIL", "test@example.com")
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        };
        run(&["checkout", "-q", "--orphan", "diverge"]);
        std::fs::write(work.join("orphan.txt"), "unrelated\n").unwrap();
        run(&["add", "."]);
        run(&["-c", "commit.gpgsign=false", "commit", "-q", "-m", "orphan"]);
        let orphan_sha = rev_head(&work);
        run(&[
            "-c",
            "protocol.file.allow=always",
            "push",
            "-q",
            "../origin.git",
            "diverge",
        ]);
        let err = verify_repository_head_remote(
            &fx.url,
            None,
            &fx.base_commit,
            &orphan_sha,
            1,
            Duration::from_secs(30),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("is not a descendant of"), "{err}");
        let _ = std::fs::remove_dir_all(fx.root);
    }

    #[test]
    fn remote_carrier_predicate_matches_only_client_channel() {
        assert!(is_remote_carrier(Some("client:0d5ea1b2")));
        assert!(!is_remote_carrier(Some("mcp:human:abcd1234")));
        assert!(!is_remote_carrier(Some("mcp:runner:host:abcd1234")));
        assert!(!is_remote_carrier(Some("worker-1")));
        assert!(!is_remote_carrier(None));
    }
}
