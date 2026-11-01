//! T-031/T-032 进程管理层测试（`docs/ai-pm-slice-d3-executor-adapter-v1.md` §5 第二层
//! + `docs/ai-pm-slice-d3-executor-hardening-v1.md` §5/§6/§8）：
//! `AI_PM_CODEX_BIN` 指向假脚本、`file://` 裸仓作 origin，验收 C67–C73。
//! 编排语义（FakeExecutor）见 `pm-application/tests/run_executor.rs`。

use std::path::{Path, PathBuf};
use std::process::Command;

use pm_application::run_dispatch::{
ExecutableRun, ExitOutcome, ExecutorProgress, ExecutorSession,
    RunExecutor, RunRepositoryBaseline, TerminateReason,
};
use pm_domain::SYSTEM_TENANT;
use pm_infrastructure::codex_executor::CodexExecutor;
use sqlx::PgPool;
use uuid::Uuid;

struct TempDir(PathBuf);

impl TempDir {
    fn create(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("codex-exec-{tag}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("temp dir");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(args: &[&str], cwd: Option<&Path>) {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com");
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let output = cmd.output().expect("git must be available in dev/test env");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 本地裸仓 + 一个已提交文件；返回 (裸仓根, base_commit)。
fn seed_bare_repo(tag: &str) -> (TempDir, String) {
    let dir = TempDir::create(tag);
    let bare = dir.0.join("origin.git");
    let work = dir.0.join("seed");
    git(&["init", "--bare", "-b", "main", &bare.to_string_lossy()], None);
    std::fs::create_dir_all(&work).unwrap();
    git(&["init", "-b", "main", "."], Some(&work));
    std::fs::write(work.join("README.md"), "seed\n").unwrap();
    git(&["add", "."], Some(&work));
    git(&["commit", "-m", "seed"], Some(&work));
    git(&["remote", "add", "origin", &bare.to_string_lossy()], Some(&work));
    git(&["push", "origin", "main"], Some(&work));
    let head = Command::new("git")
        .args(["-C", &work.to_string_lossy(), "rev-parse", "HEAD"])
        .output()
        .unwrap();
    let base_commit = String::from_utf8_lossy(&head.stdout).trim().to_string();
    (dir, base_commit)
}

/// 平台差异的假脚本：返回 (exit0, hang)。
fn write_scripts(dir: &Path) -> (PathBuf, PathBuf) {
    #[cfg(windows)]
    {
        let exit0 = dir.join("fake-exit0.cmd");
        let hang = dir.join("fake-hang.cmd");
        std::fs::write(&exit0, "@echo codex-ready\r\n@exit 0\r\n").unwrap();
        std::fs::write(&hang, "@ping -n 60 127.0.0.1 > nul\r\n").unwrap();
        (exit0, hang)
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let exit0 = dir.join("fake-exit0.sh");
        let hang = dir.join("fake-hang.sh");
        std::fs::write(&exit0, "#!/bin/sh\necho codex-ready\nexit 0\n").unwrap();
        std::fs::write(&hang, "#!/bin/sh\nsleep 60\n").unwrap();
        for script in [&exit0, &hang] {
            std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        (exit0, hang)
    }
}

/// CodexExecutor：bin 指向假脚本（Windows 经 cmd /C），root 与宽限显式注入。
fn executor(pool: PgPool, bin: &Path, root: &Path) -> CodexExecutor {
    let (bin, args): (String, Vec<String>) = if cfg!(windows) {
        (
            "cmd".into(),
            vec!["/C".into(), bin.to_string_lossy().to_string()],
        )
    } else {
        (bin.to_string_lossy().to_string(), vec![])
    };
    CodexExecutor::new(pool, bin, args, root.to_path_buf(), 1)
}

/// core.customer → core.project 最小种子（project_repository 的租户子查询需要）。
async fn seed_project(pool: &PgPool, tag: &str) -> Uuid {
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("customer {tag}"))
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer_id)
    .bind(format!("P-{tag}"))
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// project_repository 种子（url 为 file://，无 SSH 凭据 → GIT_SSH_COMMAND 不注入）。
async fn seed_repo_row(pool: &PgPool, project_id: Uuid, repository_id: Uuid, url: &str) {
    sqlx::query(
        "INSERT INTO integration.project_repository \
         (id, project_id, name, url, status, head_commit_sha, tenant_id) \
         VALUES ($1, $2, 'test-repo', $3, 'verified', 'headsha', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(repository_id)
    .bind(project_id)
    .bind(url)
    .execute(pool)
    .await
    .unwrap();
}

fn executable_run(run_id: Uuid, project_id: Uuid, repo_id: Uuid, base_commit: &str) -> ExecutableRun {
    ExecutableRun {
        run_id,
        task_id: Uuid::new_v4(),
        project_id,
        attempt_no: 1,
        timeout_seconds: None,
        lease_generation: 1,
        lease_token: Some("f".repeat(64)),
        deadline_at: None,
        repositories: vec![RunRepositoryBaseline {
            repository_id: repo_id,
            base_ref: "main".to_string(),
            base_commit: base_commit.to_string(),
            write_access: true,
            workspace_path: format!("repos/test-{repo_id}"),
        }],
    }
}

/// T-032 T1：session 由编排器前置生成——测试同构：先造 session 再 start，
/// 然后手工组装 ExecutorSession（workspace_root 为测试 root）。
fn make_session(run: &ExecutableRun, root: &Path) -> (String, ExecutorSession) {
    let session = format!("codex:{}", Uuid::new_v4());
    (
        session.clone(),
        ExecutorSession {
            session,
            run_id: run.run_id,
            workspace_root: root.to_string_lossy().to_string(),
        },
    )
}

/// 等待进程退出（轮询 poll，最多 10s）。
async fn wait_done(executor: &CodexExecutor, session: &ExecutorSession) -> ExecutorProgress {
    let mut progress = ExecutorProgress::Pending;
    for _ in 0..100 {
        progress = executor.poll(session).await.unwrap();
        if matches!(progress, ExecutorProgress::Done { .. }) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    progress
}

#[sqlx::test(migrations = "../../migrations")]
async fn c67_spawn_records_session_pid_and_reaps_exit_code(pool: PgPool) {
    let project_id = seed_project(&pool, "c67").await;
    let workspace = TempDir::create("c67-ws");
    let scripts = TempDir::create("c67-bin");
    let (bare, base_commit) = seed_bare_repo("c67");
    let (exit0, _hang) = write_scripts(&scripts.0);
    let repo_id = Uuid::new_v4();
    let url = format!("file://{}", bare.0.join("origin.git").display());
    seed_repo_row(&pool, project_id, repo_id, &url).await;

    let executor = executor(pool, &exit0, &workspace.0);
    let run = executable_run(Uuid::new_v4(), project_id, repo_id, &base_commit);
    let (session_text, session) = make_session(&run, &workspace.0);
    executor
        .start(&run, &session_text)
        .await
        .expect("start must succeed");

    // pid 文件 + 日志文件就位（C69 的派生规则同源）。
    let run_root = workspace.0.join(run.run_id.to_string());
    assert!(run_root.join("process.json").exists());
    assert!(run_root.join("logs").join("stdout.log").exists());
    assert!(run_root.join("logs").join("stderr.log").exists());

    let progress = wait_done(&executor, &session).await;
    assert_eq!(
        progress,
        ExecutorProgress::Done {
            outcome: ExitOutcome::Success
        },
        "fake codex must be reaped with a structured Success outcome"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn c68_terminate_kills_a_hung_process(pool: PgPool) {
    let project_id = seed_project(&pool, "c68").await;
    let workspace = TempDir::create("c68-ws");
    let scripts = TempDir::create("c68-bin");
    let (bare, base_commit) = seed_bare_repo("c68");
    let (_exit0, hang) = write_scripts(&scripts.0);
    let repo_id = Uuid::new_v4();
    let url = format!("file://{}", bare.0.join("origin.git").display());
    seed_repo_row(&pool, project_id, repo_id, &url).await;

    let executor = executor(pool, &hang, &workspace.0);
    let run = executable_run(Uuid::new_v4(), project_id, repo_id, &base_commit);
    let (session_text, session) = make_session(&run, &workspace.0);
    executor.start(&run, &session_text).await.expect("start must succeed");
    assert_eq!(executor.poll(&session).await.unwrap(), ExecutorProgress::Running);

    executor
        .terminate(&session, TerminateReason::TimedOut)
        .await
        .expect("terminate must succeed");

    // 进程确实死亡：句柄已移除，poll 走孤儿路径 Done{Lost}（退出码不可归因）。
    let progress = executor.poll(&session).await.unwrap();
    assert_eq!(
        progress,
        ExecutorProgress::Done {
            outcome: ExitOutcome::Lost
        }
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn c69_workspace_layout_matches_the_ruled_derivation(pool: PgPool) {
    let project_id = seed_project(&pool, "c69").await;
    let workspace = TempDir::create("c69-ws");
    let scripts = TempDir::create("c69-bin");
    let (bare, base_commit) = seed_bare_repo("c69");
    let (exit0, _) = write_scripts(&scripts.0);
    let repo_id = Uuid::new_v4();
    let url = format!("file://{}", bare.0.join("origin.git").display());
    seed_repo_row(&pool, project_id, repo_id, &url).await;

    let executor = executor(pool, &exit0, &workspace.0);
    let run = executable_run(Uuid::new_v4(), project_id, repo_id, &base_commit);
    let (session_text, session) = make_session(&run, &workspace.0);
    executor.start(&run, &session_text).await.expect("start must succeed");

    let run_root = workspace.0.join(run.run_id.to_string());
    let worktree = run_root.join(&run.repositories[0].workspace_path);
    assert!(worktree.exists(), "workspace_path dir must exist");
    assert!(worktree.join(".git").exists(), "worktree must be a git worktree");
    assert_eq!(
        session.workspace_root,
        workspace.0.to_string_lossy().to_string()
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn c70_worktree_is_checked_out_at_the_locked_base_commit(pool: PgPool) {
    let project_id = seed_project(&pool, "c70").await;
    let workspace = TempDir::create("c70-ws");
    let scripts = TempDir::create("c70-bin");
    let (bare, base_commit) = seed_bare_repo("c70");
    let (exit0, _) = write_scripts(&scripts.0);
    let repo_id = Uuid::new_v4();
    let url = format!("file://{}", bare.0.join("origin.git").display());
    seed_repo_row(&pool, project_id, repo_id, &url).await;

    let executor = executor(pool, &exit0, &workspace.0);
    let run = executable_run(Uuid::new_v4(), project_id, repo_id, &base_commit);
    let (session_text, _session) = make_session(&run, &workspace.0);
    executor.start(&run, &session_text).await.expect("start must succeed");

    let worktree = workspace
        .0
        .join(run.run_id.to_string())
        .join(&run.repositories[0].workspace_path);
    let head = Command::new("git")
        .args(["-C", &worktree.to_string_lossy(), "rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&head.stdout).trim(), base_commit);
    assert!(worktree.join("README.md").exists(), "seed file must be checked out");
}

/// T-032 §6.3（C71）：一次性 token 明文经日志管道脱敏——stdout 上报的 token
/// 在 `stdout.log` 中以 `[REDACTED]` 出现，且 `process.json` / `mcp.json` /
/// events 文件均不含 token 明文（凭据禁止面）。
#[sqlx::test(migrations = "../../migrations")]
async fn c71_lease_token_is_redacted_in_logs_and_never_persisted(pool: PgPool) {
    let project_id = seed_project(&pool, "c71").await;
    let workspace = TempDir::create("c71-ws");
    let scripts = TempDir::create("c71-bin");
    let (bare, base_commit) = seed_bare_repo("c71");
    let (_exit0, _) = write_scripts(&scripts.0);
    let repo_id = Uuid::new_v4();
    let url = format!("file://{}", bare.0.join("origin.git").display());
    seed_repo_row(&pool, project_id, repo_id, &url).await;

    // 脚本回显 token（Windows cmd 需避开特殊字符；token 是纯 hex）。
    let token = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
    #[cfg(windows)]
    let echo_script = format!("@echo {token}\r\n@exit 0\r\n");
    #[cfg(not(windows))]
    let echo_script = format!("#!/bin/sh\necho {token}\nexit 0\n");
    let echo_path = scripts.0.join("fake-echo.cmd");
    std::fs::write(&echo_path, &echo_script).unwrap();
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&echo_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let executor = executor(pool, &echo_path, &workspace.0);
    let run = ExecutableRun {
        lease_token: Some(token.to_string()),
        ..executable_run(Uuid::new_v4(), project_id, repo_id, &base_commit)
    };
    let (session_text, _session) = make_session(&run, &workspace.0);
    executor.start(&run, &session_text).await.expect("start must succeed");

    let run_root = workspace.0.join(run.run_id.to_string());
    let stdout_log = run_root.join("logs").join("stdout.log");
    let mut body = String::new();
    for _ in 0..50 {
        if let Ok(text) = std::fs::read_to_string(&stdout_log) {
            if text.contains(token) || text.contains("[REDACTED]") {
                body = text;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(body.contains("[REDACTED]"), "token must be redacted in logs");
    assert!(!body.contains(token), "raw token must never hit the log file");

    // 禁止面：pid 文件、mcp.json、events 均不含明文。
    for probe in [
        run_root.join("process.json"),
        run_root.join("mcp.json"),
        run_root.join("logs").join("executor-events.jsonl"),
    ] {
        if let Ok(text) = std::fs::read_to_string(&probe) {
            assert!(!text.contains(token), "token leaked into {}", probe.display());
        }
    }
}

/// T-032 §5（C72）：孤儿三重证据——pid 文件存在但身份锚点不匹配（模拟 PID
/// 重用）时判定原进程已死（Done{Lost}），不误认无辜进程。
#[sqlx::test(migrations = "../../migrations")]
async fn c72_orphan_pid_reuse_is_detected_via_identity_anchor(pool: PgPool) {
    let project_id = seed_project(&pool, "c72").await;
    let workspace = TempDir::create("c72-ws");
    let scripts = TempDir::create("c72-bin");
    let (bare, base_commit) = seed_bare_repo("c72");
    let (_exit0, hang) = write_scripts(&scripts.0);
    let repo_id = Uuid::new_v4();
    let url = format!("file://{}", bare.0.join("origin.git").display());
    seed_repo_row(&pool, project_id, repo_id, &url).await;

    let first = executor(pool.clone(), &hang, &workspace.0);
    let run = executable_run(Uuid::new_v4(), project_id, repo_id, &base_commit);
    let (session_text, session) = make_session(&run, &workspace.0);
    first.start(&run, &session_text).await.expect("start must succeed");

    // 篡改 pid 文件的身份锚点 → 模拟「该 PID 已被另一个进程占用」。
    let pid_path = workspace.0.join(run.run_id.to_string()).join("process.json");
    let mut payload: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&pid_path).unwrap()).unwrap();
    payload["identity_hint"] = serde_json::json!("999999999999");
    std::fs::write(&pid_path, serde_json::to_vec(&payload).unwrap()).unwrap();

    // 模拟平台重启：内存句柄随进程丢失，用全新 executor 实例走孤儿判定路径。
    let rebooted = executor(pool, &hang, &workspace.0);
    let progress = rebooted.poll(&session).await.unwrap();
    assert_eq!(
        progress,
        ExecutorProgress::Done {
            outcome: ExitOutcome::Lost
        },
        "mismatched identity anchor must read as original-process-dead (PID reuse)"
    );
}

/// T-032 T1（C73）：pid 文件缺失但 run 目录已初始化 → 三重证据不足 →
/// `Uncertain`（按可能存活处理，不静默回收）。
#[sqlx::test(migrations = "../../migrations")]
async fn c73_missing_pid_file_with_initialized_run_dir_is_uncertain(pool: PgPool) {
    let project_id = seed_project(&pool, "c73").await;
    let workspace = TempDir::create("c73-ws");
    let scripts = TempDir::create("c73-bin");
    let (bare, base_commit) = seed_bare_repo("c73");
    let (exit0, _) = write_scripts(&scripts.0);
    let repo_id = Uuid::new_v4();
    let url = format!("file://{}", bare.0.join("origin.git").display());
    seed_repo_row(&pool, project_id, repo_id, &url).await;

    let first = executor(pool.clone(), &exit0, &workspace.0);
    let run = executable_run(Uuid::new_v4(), project_id, repo_id, &base_commit);
    let (session_text, session) = make_session(&run, &workspace.0);
    first.start(&run, &session_text).await.expect("start must succeed");

    // 删除 pid 文件（保留 run 目录与 logs）→ 证据不足。先收割自然退出（内存
    // 句柄移除），再模拟「平台重启」：用全新 executor 实例走孤儿判定路径。
    let _ = wait_done(&first, &session).await;
    std::fs::remove_file(workspace.0.join(run.run_id.to_string()).join("process.json")).unwrap();

    let rebooted = executor(pool, &exit0, &workspace.0);
    let progress = rebooted.poll(&session).await.unwrap();
    assert_eq!(
        progress,
        ExecutorProgress::Uncertain,
        "missing pid file with initialized run dir must be Uncertain, never auto-reclaimed"
    );
}