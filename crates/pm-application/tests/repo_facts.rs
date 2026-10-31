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

    fn url(&self) -> String {
        format!(
            "file://{}",
            self.repo.to_string_lossy().replace('\\', "/")
        )
    }
}

async fn clone_and_collect(fx: &Fixture, limits: Limits) -> RepoFacts {
    let dest = fx.root.join("cache");
    git_access::clone_bare(
        &fx.url(),
        None,
        &dest,
        Duration::from_secs(60),
    )
    .await
    .unwrap();
    collect(&dest, "main", limits).await.unwrap()
}

fn assert_docs(paths: &[String], expected: &[&str]) {
    for path in expected {
        assert!(paths.contains(&path.to_string()), "missing doc {path}: {paths:?}");
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
    for i in 0..300 {
        fx.write("counter.txt", format!("{i}\n").as_bytes());
        fx.commit_all(&format!("chore: bump {i}"));
    }

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
