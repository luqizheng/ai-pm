//! 只读 git 子进程封装：参数数组、禁交互、显式超时、临时密钥用完即删。
//!
//! 本模块只允许 `ls-remote` / `clone` / `log` / `show` / `ls-tree` 这类只读命令；
//! `fetch` 仅限刷新 AI-PM 自己持有的 bare clone 缓存（`repo_cache_dir`）——
//! 对远端仍是只读，不触碰任何用户工作区；不提供 push，也不写仓库 config。

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
    #[error("git network error: {0}")]
    Network(String),
    #[error("repository is too large")]
    TooLarge,
    #[error("git command failed: {0}")]
    Other(String),
    #[error("git io error: {0}")]
    Io(String),
}

impl GitError {
    /// 稳定错误码（DEF-CHAIN-02）：not-found / auth / network 三类可区分，
    /// 供 `project_import::git_error_code` 与 KB/PC 分析链共同复用，避免
    /// 「路径不存在」被归到认证失败。
    pub fn stable_code(&self) -> &'static str {
        match self {
            GitError::NotFound => "git_not_found",
            GitError::AuthFailed => "git_auth_failed",
            GitError::Network(_) => "git_network_error",
            GitError::Timeout => "git_timeout",
            GitError::TooLarge => "repo_too_large",
            GitError::Other(_) | GitError::Io(_) => "git_failed",
        }
    }
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
    // ① 目标不存在：路径/仓库缺失优先判定。git 对 file:// 不存在的路径只回
    //    "could not read from remote repository" 的通用尾句（见 ④）。
    if lower.contains("does not appear to be a git repository")
        || lower.contains("does not exist")
        || lower.contains("not a git repository")
        || lower.contains("no such file or directory")
        || lower.contains("repository not found")
    {
        return GitError::NotFound;
    }
    // ② 明确的认证失败。
    if lower.contains("permission denied")
        || lower.contains("authentication failed")
        || lower.contains("publickey")
        || lower.contains("invalid username or password")
        || lower.contains("could not read username")
    {
        return GitError::AuthFailed;
    }
    // ③ 网络类（DNS / 连接 / TLS）。
    if lower.contains("could not resolve host")
        || lower.contains("unable to resolve host")
        || lower.contains("connection timed out")
        || lower.contains("connection refused")
        || lower.contains("network is unreachable")
        || lower.contains("couldn't connect")
        || lower.contains("ssl_connect")
        || lower.contains("unable to access")
    {
        return GitError::Network(stderr.trim().to_string());
    }
    if lower.contains("too large") || lower.contains("exceeds") {
        return GitError::TooLarge;
    }
    // ④ DEF-CHAIN-02：`could not read from remote repository` 是 git 在多种
    //    失败下都会打印的通用尾句，单独出现时不得推断为认证失败。
    if lower.contains("could not read from remote repository") {
        return GitError::NotFound;
    }
    if lower.contains("not found") {
        return GitError::NotFound;
    }
    GitError::Other(stderr.trim().to_string())
}

/// `file://` URL → 本地路径（Windows `file://D:/…` 与 `file:///D:/…` 均支持）。
/// 仅在 clone 前置存在性探测使用，不做 percent-decoding（超出本修复范围）。
fn file_url_local_path(url: &str) -> Option<&Path> {
    let raw = url.strip_prefix("file://")?;
    // `/D:/…` 去掉前导斜杠才是 Windows 合法盘符路径；POSIX `/home/…` 保留。
    let bytes = raw.as_bytes();
    let candidate = if bytes.first() == Some(&b'/') && bytes.get(2) == Some(&b':') {
        &raw[1..]
    } else {
        raw
    };
    Some(Path::new(candidate))
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
    // DEF-CHAIN-02：file:// 本地路径不存在时直接判 NotFound，不再依赖 git
    // 的通用 stderr 尾句（历史上被 classify 误判为 git_auth_failed）。
    if let Some(local) = file_url_local_path(url) {
        if !local.exists() {
            let _ = std::fs::remove_dir_all(dest);
            return Err(GitError::NotFound);
        }
    }
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

/// 刷新 AI-PM 自己持有的 bare clone 缓存。
///
/// bare clone 没有默认 fetch refspec（裸 fetch 只更新 FETCH_HEAD，不前进
/// `refs/heads/*`），因此带显式 refspec `+refs/heads/*:refs/heads/*`——与
/// `clone --bare` 的分支布局一致。只对 [`crate::repo_facts::repo_cache_dir`]
/// 下的缓存目录调用；远端访问仍是只读（fetch = 拉取对象），绝不指向用户
/// 工作区。外部 Commit 发现扫描用它避免每次全量重 clone。
pub async fn fetch_bare(
    cache_dir: &Path,
    key: Option<&Path>,
    timeout: Duration,
) -> Result<(), GitError> {
    let known_hosts = std::env::temp_dir().join(format!("ai-pm-known-hosts-{}", Uuid::new_v4()));
    let mut cmd = prepare_command(key, Some(&known_hosts));
    cmd.arg("-C")
        .arg(cache_dir)
        .arg("fetch")
        .arg("origin")
        .arg("+refs/heads/*:refs/heads/*")
        .arg("--prune");
    cmd.kill_on_drop(true);

    let child = cmd.spawn().map_err(|e| GitError::Io(e.to_string()))?;
    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(GitError::Io(e.to_string())),
        Err(_) => return Err(GitError::Timeout),
    };
    let _ = std::fs::remove_file(&known_hosts);
    if !output.status.success() {
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

    /// DEF-CHAIN-02：三类 clone 失败必须有互不混淆的稳定错误码。
    #[test]
    fn classify_distinguishes_not_found_auth_and_network() {
        // branch-b4 的原始 stderr：git 的通用尾句，过去被判成认证失败。
        let generic = classify(
            "fatal: could not read from remote repository\n\n\
             Please make sure you have the correct access rights",
        );
        assert!(matches!(generic, GitError::NotFound), "{generic:?}");
        assert_eq!(generic.stable_code(), "git_not_found");

        let auth = classify("Permission denied (publickey).");
        assert!(matches!(auth, GitError::AuthFailed), "{auth:?}");
        assert_eq!(auth.stable_code(), "git_auth_failed");

        let network = classify("fatal: unable to access 'https://x/r/': Could not resolve host: x");
        assert!(matches!(network, GitError::Network(_)), "{network:?}");
        assert_eq!(network.stable_code(), "git_network_error");

        let not_found = classify("fatal: repository 'https://x/r' not found");
        assert!(matches!(not_found, GitError::NotFound), "{not_found:?}");
    }

    /// DEF-CHAIN-02（branch-b4 场景）：file:// 路径不存在不得再报 authentication failed。
    #[tokio::test]
    async fn missing_file_url_clone_is_not_authentication_failure() {
        let missing = std::env::temp_dir().join(format!("ai-pm-no-such-repo-{}", Uuid::new_v4()));
        let url = format!("file://{}", missing.to_string_lossy().replace('\\', "/"));
        let dest = std::env::temp_dir().join(format!("ai-pm-clone-dest-{}", Uuid::new_v4()));
        let err = clone_bare(&url, None, &dest, Duration::from_secs(30))
            .await
            .unwrap_err();
        assert!(matches!(err, GitError::NotFound), "{err:?}");
        assert_eq!(err.stable_code(), "git_not_found");
        assert!(!err.to_string().contains("authentication"));
        let _ = std::fs::remove_dir_all(&dest);
    }
}
