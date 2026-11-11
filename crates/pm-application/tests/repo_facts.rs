//! 仓库事实采集的集成测试：真实 git 仓库 + 本地 file:// clone，不走网络。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use pm_application::git_access::{self, GitError};
use pm_application::repo_facts::{collect, Limits, RepoFacts};
use uuid::Uuid;

struct Fixture {
    root: PathBuf,
    repo: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ai-pm-facts-{}", Uuid::new_v4()));
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let fx = Self { root, repo };
        fx.git(&["init", "-q", "-b", "main"]);
        fx
    }

    fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(&self.repo)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    fn write(&self, path: &str, content: &[u8]) {
        let target = self.repo.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(target, content).unwrap();
    }

    fn commit_all(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["-c", "commit.gpgsign=false", "commit", "-q", "-m", message]);
    }

    fn git_stdout(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.repo)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?} failed");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// 用**一次** `git fast-import` 追加 `n` 个提交（每个提交只改 `path`）。
    ///
    /// 逐条 `git add -A` + `git commit` = 每次提交 2 次进程 spawn；Windows 下单次
    /// spawn ≈70ms，301 次提交 ≈45s，是全量测试里最大的单例。fast-import 用单进程
    /// 流式写入等价历史（提交数/顺序/subject 语义不变）。
    fn commit_many(&self, n: usize, path: &str) {
        let head = self.git_stdout(&["rev-parse", "HEAD"]);
        let mut script = String::new();
        for i in 0..n {
            let subject = format!("chore: bump {i}");
            let content = format!("{i}");
            script.push_str("commit refs/heads/main");
            script.push('\n');
            script.push_str(&format!(
                "committer Test <test@example.com> {} +0000",
                1_700_000_000 + i as i64
            ));
            script.push('\n');
            script.push_str(&format!("data {}", subject.len()));
            script.push('\n');
            script.push_str(&subject);
            script.push('\n');
            if i == 0 {
                // from 必须在提交信息 data 之后（fast-import 的命令顺序），
                // 且要显式接当前 HEAD，否则新链会被判为非快进而不更新 ref。
                script.push_str(&format!("from {head}"));
                script.push('\n');
            }
            script.push_str(&format!("M 100644 inline {path}"));
            script.push('\n');
            script.push_str(&format!("data {}", content.len()));
            script.push('\n');
            script.push_str(&content);
            script.push('\n');
        }
        script.push_str("done");
        script.push('\n');

        self.fast_import(&script);
    }

    /// 管道写入 fast-import 脚本（一次进程）。
    fn fast_import(&self, script: &str) {
        let mut child = Command::new("git")
            .arg("fast-import")
            .arg("--quiet")
            .current_dir(&self.repo)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write as _;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(script.as_bytes())
            .unwrap();
        let status = child.wait().unwrap();
        assert!(status.success(), "git fast-import failed");
    }

    fn url(&self) -> String {
        format!("file://{}", self.repo.to_string_lossy().replace('\\', "/"))
    }
}

async fn clone_and_collect(fx: &Fixture, limits: Limits) -> RepoFacts {
    let dest = fx.root.join("cache");
    git_access::clone_bare(&fx.url(), None, &dest, Duration::from_secs(60))
        .await
        .unwrap();
    collect(&dest, "main", limits).await.unwrap()
}

fn assert_docs(paths: &[String], expected: &[&str]) {
    for path in expected {
        assert!(
            paths.contains(&path.to_string()),
            "missing doc {path}: {paths:?}"
        );
    }
}

#[tokio::test]
async fn collect_reads_commits_and_docs_with_limits() {
    let fx = Fixture::new();
    fx.write("README.md", b"# demo\n");
    fx.write("docs/design.md", b"# design\n");
    fx.write("src/main.rs", b"fn main() {}\n");
    fx.write("node_modules/x.md", b"should be ignored\n");
    fx.commit_all("feat(core): first");
    fx.write("docs/design.md", b"# design v2\n");
    fx.commit_all("fix: second");
    fx.write("src/main.rs", b"fn main() { println!(\"hi\"); }\n");
    fx.commit_all("chore: third");

    let facts = clone_and_collect(&fx, Limits::default()).await;
    assert_eq!(facts.default_branch, "main");
    assert_eq!(facts.commits.len(), 3);
    assert_eq!(facts.commits[0].subject, "chore: third");
    assert!(facts.commits[0].sha.len() == 40);
    assert!(!facts.truncated);

    let paths: Vec<String> = facts.docs.iter().map(|d| d.path.clone()).collect();
    assert_docs(&paths, &["README.md", "docs/design.md"]);
    assert!(!paths.iter().any(|p| p.contains("node_modules")));
    assert!(facts.hot_files.iter().any(|h| h.path == "src/main.rs"));
    assert!(facts.languages.iter().any(|l| l.extension == "rs"));
}

#[tokio::test]
async fn collect_truncates_at_three_hundred_commits() {
    let fx = Fixture::new();
    fx.write("README.md", b"# demo\n");
    fx.commit_all("init");
    // 301 个提交（init + 300）一次 fast-import 造出：断言的是提交数与截断标志，
    // 逐条 add+commit 在 Windows 上要 600 次进程 spawn（≈45s）。
    fx.commit_many(300, "counter.txt");

    let facts = clone_and_collect(&fx, Limits::default()).await;
    assert_eq!(facts.commits.len(), 300);
    assert!(facts.truncated, "301 commits must mark truncated");
}

#[tokio::test]
async fn collect_skips_oversized_and_non_utf8_docs() {
    let fx = Fixture::new();
    fx.write("README.md", b"# demo\n");
    fx.write("docs/huge.md", &vec![b'a'; 70 * 1024]);
    fx.write("docs/binary.md", &[0xff, 0xfe, 0x00, 0x01]);
    fx.write("docs/ok.md", b"# ok\n");
    fx.commit_all("init");

    let facts = clone_and_collect(&fx, Limits::default()).await;
    let paths: Vec<String> = facts.docs.iter().map(|d| d.path.clone()).collect();
    assert!(paths.contains(&"README.md".to_string()));
    assert!(paths.contains(&"docs/ok.md".to_string()));
    assert!(!paths.contains(&"docs/huge.md".to_string()));
    assert!(!paths.contains(&"docs/binary.md".to_string()));
    assert!(facts.truncated, "oversized doc must mark truncated");
}

#[tokio::test]
async fn collect_caps_long_subject_to_two_hundred_chars() {
    let fx = Fixture::new();
    fx.write("README.md", b"# demo\n");
    let long = "x".repeat(400);
    fx.commit_all(&long);
    let facts = clone_and_collect(&fx, Limits::default()).await;
    assert_eq!(facts.commits[0].subject.chars().count(), 200);
}

#[tokio::test]
async fn collect_rejects_unsafe_branch_and_missing_repo() {
    let fx = Fixture::new();
    fx.write("README.md", b"# demo\n");
    fx.commit_all("init");
    let dest = fx.root.join("cache2");
    git_access::clone_bare(&fx.url(), None, &dest, Duration::from_secs(60))
        .await
        .unwrap();

    assert!(matches!(
        collect(&dest, "../escape", Limits::default()).await,
        Err(GitError::Other(_))
    ));
    assert!(matches!(
        collect(&fx.root.join("missing"), "main", Limits::default()).await,
        Err(GitError::Other(_) | GitError::Io(_))
    ));
}
