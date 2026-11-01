//! 只读 git 子进程封装：参数数组、禁交互、显式超时、临时密钥用完即删。
//!
//! 本模块只允许 `ls-remote` / `clone` / `log` / `show` / `ls-tree` 这类只读命令；
//! 不提供 push，也不写仓库 config。

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Command;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git command timed out")]
    Timeout,
    #[error("git authentication failed")]
    AuthFailed,
    #[error("git remote not found")]
    NotFound,
    #[error("repository is too large")]
    TooLarge,
    #[error("git command failed: {0}")]
    Other(String),
    #[error("git io error: {0}")]
    Io(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadRef {
    pub branch: String,
    pub sha: String,
}

/// 临时目录中的私钥材料；drop 时连目录一起删除。
pub struct KeyMaterial {
    dir: PathBuf,
}

impl KeyMaterial {
    pub fn key_path(&self) -> PathBuf {
        self.dir.join("id_ed25519")
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for KeyMaterial {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// 把私钥明文落到 0700 临时目录（Windows 无 POSIX 权限语义，退化为普通文件）。
pub fn write_temp_key(private_key: &str) -> Result<KeyMaterial, GitError> {
    let dir = std::env::temp_dir().join(format!("ai-pm-git-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).map_err(|e| GitError::Io(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| GitError::Io(e.to_string()))?;
    }
    let key_path = dir.join("id_ed25519");
    std::fs::write(&key_path, private_key.as_bytes()).map_err(|e| GitError::Io(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| GitError::Io(e.to_string()))?;
    }
    Ok(KeyMaterial { dir })
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn prepare_command(key: Option<&Path>, known_hosts: Option<&Path>) -> Command {
    let mut cmd = Command::new("git");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("LC_ALL", "C");
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(key) = key {
        let key = normalize_path(key);
        let known = known_hosts
            .map(normalize_path)
            .unwrap_or_else(|| normalize_path(&std::env::temp_dir().join("ai-pm-known-hosts")));
        cmd.env(
            "GIT_SSH_COMMAND",
            format!(
                "ssh -i \"{key}\" -o IdentitiesOnly=yes -o BatchMode=yes \
                 -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=\"{known}\""
            ),
        );
    }
    cmd
}

fn classify(stderr: &str) -> GitError {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("permission denied")
        || lower.contains("authentication failed")
        || lower.contains("could not read from remote repository")
        || lower.contains("repository not found")
    {
        return GitError::AuthFailed;
    }
    if lower.contains("too large") || lower.contains("exceeds") {
        return GitError::TooLarge;
    }
    if lower.contains("does not exist") || lower.contains("not found") {
        return GitError::NotFound;
    }
    GitError::Other(stderr.trim().to_string())
}

/// `git ls-remote --heads <url>`；返回分支名（去掉 `refs/heads/`）与 sha。
pub async fn ls_remote(
    url: &str,
    key: Option<&Path>,
    timeout: Duration,
) -> Result<Vec<HeadRef>, GitError> {
    let known_hosts = std::env::temp_dir().join(format!("ai-pm-known-hosts-{}", Uuid::new_v4()));
    let mut cmd = prepare_command(key, Some(&known_hosts));
    cmd.arg("ls-remote").arg("--heads").arg(url);
    cmd.kill_on_drop(true);

    let child = cmd.spawn().map_err(|e| GitError::Io(e.to_string()))?;
    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(GitError::Io(e.to_string())),
        Err(_) => {
            let _ = known_hosts;
            return Err(GitError::Timeout);
        }
    };
    let _ = std::fs::remove_file(&known_hosts);

    if !output.status.success() {
        return Err(classify(&String::from_utf8_lossy(&output.stderr)));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut heads = Vec::new();
    for line in text.lines() {
        let mut parts = line.split('\t');
        let (Some(sha), Some(reference)) = (parts.next(), parts.next()) else {
            continue;
        };
        if let Some(branch) = reference.strip_prefix("refs/heads/") {
            heads.push(HeadRef {
                branch: branch.to_string(),
                sha: sha.to_string(),
            });
        }
    }
    heads.sort_by(|a, b| a.branch.cmp(&b.branch));
    Ok(heads)
}

/// `git clone --bare`，失败或超时里清理目标目录。
pub async fn clone_bare(
    url: &str,
    key: Option<&Path>,
    dest: &Path,
    timeout: Duration,
) -> Result<(), GitError> {
    let known_hosts = std::env::temp_dir().join(format!("ai-pm-known-hosts-{}", Uuid::new_v4()));
    let mut cmd = prepare_command(key, Some(&known_hosts));
    cmd.arg("-c")
        .arg("protocol.file.allow=always")
        .arg("clone")
        .arg("--bare")
        .arg(url)
        .arg(dest);
    cmd.kill_on_drop(true);

    let child = cmd.spawn().map_err(|e| GitError::Io(e.to_string()))?;
    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            let _ = std::fs::remove_dir_all(dest);
            return Err(GitError::Io(e.to_string()));
        }
        Err(_) => {
            let _ = std::fs::remove_dir_all(dest);
            return Err(GitError::Timeout);
        }
    };
    let _ = std::fs::remove_file(&known_hosts);
    if !output.status.success() {
        let _ = std::fs::remove_dir_all(dest);
        return Err(classify(&String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

/// 在本地仓库根执行一条只读 git 命令并返回 stdout 原始字节。
pub async fn git_stdout(
    root: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<Vec<u8>, GitError> {
    let mut cmd = prepare_command(None, None);
    cmd.arg("-C").arg(root).args(args);
    cmd.kill_on_drop(true);
    let child = cmd.spawn().map_err(|e| GitError::Io(e.to_string()))?;
    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(GitError::Io(e.to_string())),
        Err(_) => return Err(GitError::Timeout),
    };
    if !output.status.success() {
        return Err(classify(&String::from_utf8_lossy(&output.stderr)));
    }
    Ok(output.stdout)
}

/// 一条 git 命令的完整结果：D4 的校验需要**退出码**（`merge-base --is-ancestor`
/// 以退出码表达「是/否」），[`git_stdout`] 只区分「成功/失败」不够用。
#[derive(Debug, Clone)]
pub struct GitOutput {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

impl GitOutput {
    pub fn ok(&self) -> bool {
        self.code == 0
    }
    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).trim().to_string()
    }
}

/// 在本地仓库根执行一条只读 git 命令，**返回退出码**（不把非零退出当错误）。
///
/// 与 [`git_stdout`] 的差别只有一处：非零退出码不转成 [`GitError`]，而是原样
/// 带回给调用方判断——`git merge-base --is-ancestor A B` 的「不是祖先」正是
/// 退出码 1 而非错误输出。
pub async fn git_run(root: &Path, args: &[&str], timeout: Duration) -> Result<GitOutput, GitError> {
    let mut cmd = prepare_command(None, None);
    cmd.arg("-C").arg(root).args(args);
    cmd.kill_on_drop(true);
    let child = cmd.spawn().map_err(|e| GitError::Io(e.to_string()))?;
    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(GitError::Io(e.to_string())),
        Err(_) => return Err(GitError::Timeout),
    };
    Ok(GitOutput {
        code: output.status.code().unwrap_or(-1),
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

/// 目录占用字节数（符号链接按链接本身计）。
pub fn directory_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let meta = match entry.metadata() {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if meta.is_dir() {
            total += directory_size(&entry.path());
        } else {
            total += meta.len();
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn init_repo() -> (PathBuf, String) {
        let root = std::env::temp_dir().join(format!("ai-pm-git-test-{}", Uuid::new_v4()));
        let path = root.join("repo");
        std::fs::create_dir_all(&path).unwrap();
        let run = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(&path)
                .env("GIT_AUTHOR_NAME", "Test")
                .env("GIT_AUTHOR_EMAIL", "test@example.com")
                .env("GIT_COMMITTER_NAME", "Test")
                .env("GIT_COMMITTER_EMAIL", "test@example.com")
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        };
        run(&["init", "-q", "-b", "main"]);
        std::fs::write(path.join("README.md"), "# demo\n").unwrap();
        run(&["add", "."]);
        run(&["-c", "commit.gpgsign=false", "commit", "-q", "-m", "init"]);
        let url = format!("file://{}", path.to_string_lossy().replace('\\', "/"));
        (root, url)
    }

    #[tokio::test]
    async fn ls_remote_lists_local_heads() {
        let (root, url) = init_repo();
        let heads = ls_remote(&url, None, Duration::from_secs(30))
            .await
            .unwrap();
        let _ = std::fs::remove_dir_all(root);
        assert_eq!(heads.len(), 1);
        assert_eq!(heads[0].branch, "main");
        assert_eq!(heads[0].sha.len(), 40);
    }
}