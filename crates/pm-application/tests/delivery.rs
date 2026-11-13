//! Slice D4 交付与验收的数据库集成测试（`docs/slice-d/ai-pm-slice-d4-delivery-v1.md` §6
//! 验收矩阵 C71–C97）。
//!
//! 用真实 git 工作区驱动：每个用例在临时目录里 `git init` 出仓，按 D3 的派生
//! 规则放到 `{base}/{run_id}/{workspace_path}`，让管线在**实际工作区**上校验
//! HEAD / 后代关系 / 变更文件计数。

use std::path::{Path, PathBuf};
use std::time::Duration;

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::delivery::{
    submit_delivery_with, DeliveryError, DeliveryOptions, SubmitResultRequest, SubmittedRepository,
    SubmittedTest,
};
use pm_application::run_context::{report_progress, ProgressReport};
use pm_application::run_dispatch::{claim_task_run, workspace_path_for, RunDispatchConfig};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// git 工作区夹具
// ---------------------------------------------------------------------------

fn git(root: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .expect("git must be installed for the D4 delivery tests");
    assert!(status.success(), "git {args:?} failed in {root:?}");
}

fn head_sha(root: &Path) -> String {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .expect("git rev-parse");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// 在工作区里建一个仓并落下基线 commit，返回基线 sha。
fn init_workspace(base: &Path, run_id: Uuid, workspace_path: &str) -> (PathBuf, String) {
    let root = base.join(run_id.to_string()).join(workspace_path);
    std::fs::create_dir_all(&root).expect("create workspace");
    git(&root, &["init", "-q", "-b", "main"]);
    std::fs::write(root.join("README.md"), "# base\n").expect("write base file");
    git(&root, &["add", "."]);
    git(
        &root,
        &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "base"],
    );
    let sha = head_sha(&root);
    (root, sha)
}

/// 在工作区里落一个新 commit，返回新 sha。
fn commit_change(root: &Path, files: &[&str]) -> String {
    for file in files {
        std::fs::write(root.join(file), "changed\n").expect("write changed file");
    }
    git(root, &["add", "."]);
    git(
        root,
        &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "change"],
    );
    head_sha(root)
}

fn temp_base(label: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("ai-pm-d4-{label}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&base).expect("create workspace base");
    base
}

// ---------------------------------------------------------------------------
// 数据库夹具
// ---------------------------------------------------------------------------

struct Repo {
    repository_id: Uuid,
}

struct Fx {
    project_id: Uuid,
    task_id: Uuid,
    run_id: Uuid,
    lease_token: String,
    lease_generation: i32,
    repos: Vec<Repo>,
    base: PathBuf,
    workspaces: Vec<(PathBuf, String)>,
}

fn cfg() -> RunDispatchConfig {
    RunDispatchConfig::from_env()
}

/// 建项目 + 任务 + N 个仓 + queued Run，并领取租约、推进到 `running`。
///
/// `write_access` 决定仓是否可写；只读仓也进契约集合（校验范围 = 契约全集）。
async fn seed(pool: &PgPool, code: &str, repos: &[(&str, bool)]) -> Fx {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', 'dev') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, tenant_id) \
         VALUES ($1, 'deliver', 'ready', 1, $2, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(agent)
    .fetch_one(pool)
    .await
    .unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let base = temp_base(code);
    let mut repo_rows = Vec::new();
    let mut workspaces = Vec::new();
    for (name, write_access) in repos {
        let repository_id: Uuid = sqlx::query_scalar(
            "INSERT INTO integration.project_repository (project_id, name, url, status, head_commit_sha, tenant_id) \
             VALUES ($1, $2, $3, 'verified', 'headsha', \
                     (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
        )
        .bind(outcome.project_id)
        .bind(name.to_string())
        .bind(format!("git@host:{code}-{name}"))
        .fetch_one(pool)
        .await
        .unwrap();
        let workspace_path = workspace_path_for(repository_id, name);
        let (root, base_sha) = init_workspace(&base, run_id, &workspace_path);
        sqlx::query(
            "INSERT INTO delivery.task_repository \
             (task_id, project_id, repository_id, write_access, base_ref, base_commit, tenant_id) \
             VALUES ($1, $2, $3, $4, 'main', $5, \
                     (SELECT tenant_id FROM core.project WHERE id = $2))",
        )
        .bind(task_id)
        .bind(outcome.project_id)
        .bind(repository_id)
        .bind(write_access)
        .bind(&base_sha)
        .execute(pool)
        .await
        .unwrap();
        repo_rows.push(Repo { repository_id });
        workspaces.push((root, base_sha));
    }

    // 领取（Model A）→ 首报 started → 任务进入 `running`。
    let claimed = claim_task_run(
        pool,
        outcome.project_id,
        task_id,
        "codex",
        Some("1.2.0"),
        &cfg(),
    )
    .await
    .unwrap();
    report_progress(
        pool,
        outcome.project_id,
        run_id,
        &claimed.lease_token,
        ProgressReport {
            stage: "started",
            note: None,
            blocked: false,
        },
        &cfg(),
        claimed.lease_generation,
    )
    .await
    .unwrap();

    Fx {
        project_id: outcome.project_id,
        task_id,
        run_id,
        lease_token: claimed.lease_token,
        lease_generation: claimed.lease_generation,
        repos: repo_rows,
        base,
        workspaces,
    }
}

impl Fx {
    fn options(&self) -> DeliveryOptions {
        DeliveryOptions {
            workspace_base: self.base.clone(),
            git_timeout: Duration::from_secs(30),
        }
    }

    fn request(&self, repositories: Vec<SubmittedRepository>) -> SubmitResultRequest {
        SubmitResultRequest {
            project_id: self.project_id,
            run_id: self.run_id,
            lease_token: self.lease_token.clone(),
            lease_generation: self.lease_generation,
            integration_status: None,
            repositories,
            summary: "done".to_string(),
            knowledge_proposals: vec![],
            caller: None,
        }
    }

    /// 可写仓的正常提交声明（commit_sha 取工作区实际 HEAD）。
    fn writable_report(
        &self,
        index: usize,
        commit_sha: &str,
        base_commit: &str,
    ) -> SubmittedRepository {
        SubmittedRepository {
            repository_id: self.repos[index].repository_id,
            base_commit: base_commit.to_string(),
            commit_sha: commit_sha.to_string(),
            changed_files: 1,
            tests: vec![SubmittedTest {
                name: "cargo test".to_string(),
                passed: true,
                evidence_ref: Some("log://cargo-test".to_string()),
            }],
        }
    }
}

async fn commit_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM delivery.task_repository_commit")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn task_status(pool: &PgPool, task_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn audit_count(pool: &PgPool, action: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM governance.audit_log WHERE action = $1")
        .bind(action)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// C71 迁移断言
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c71_migration_shape_is_in_place(pool: PgPool) {
    // integration_status 列存在且默认 not_run。
    let default: String = sqlx::query_scalar(
        "SELECT column_default FROM information_schema.columns \
          WHERE table_schema = 'delivery' AND table_name = 'task_run' \
            AND column_name = 'integration_status'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(default.contains("not_run"), "{default}");

    // changed_files 的 CHECK 约束存在。
    let checks: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint con \
           JOIN pg_class c ON c.oid = con.conrelid \
           JOIN pg_namespace n ON n.oid = c.relnamespace \
          WHERE n.nspname = 'delivery' AND c.relname = 'task_repository_commit' \
            AND con.contype = 'c' AND pg_get_constraintdef(con.oid) LIKE '%changed_files%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(checks, 1, "changed_files must carry a CHECK constraint");

    // 外键 RESTRICT：删 Run 不得带走交付记录。
    let deletes: String = sqlx::query_scalar(
        "SELECT confdeltype::text FROM pg_constraint con \
           JOIN pg_class c ON c.oid = con.conrelid \
           JOIN pg_namespace n ON n.oid = c.relnamespace \
          WHERE n.nspname = 'delivery' AND c.relname = 'task_repository_commit' \
            AND con.contype = 'f' AND pg_get_constraintdef(con.oid) LIKE '%task_run%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        deletes, "r",
        "the run foreign key must be ON DELETE RESTRICT"
    );

    // 聚合视图存在。
    let view: String = sqlx::query_scalar(
        "SELECT table_name FROM information_schema.views \
          WHERE table_schema = 'delivery' AND table_name = 'v_task_delivery_manifest'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(view, "v_task_delivery_manifest");
}

// ---------------------------------------------------------------------------
// C72 / C95 正常三仓交付
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c72_three_repository_delivery_lands_commits_and_submits_the_task(pool: PgPool) {
    let fx = seed(
        &pool,
        "D4-C72",
        &[("alpha", true), ("beta", true), ("gamma", true)],
    )
    .await;
    let mut reports = Vec::new();
    for (index, (root, base_sha)) in fx.workspaces.iter().enumerate() {
        let commit = commit_change(root, &[&format!("file{index}.md")]);
        reports.push(fx.writable_report(index, &commit, base_sha));
    }
    let mut request = fx.request(reports);
    request.integration_status = Some("passed".to_string());

    let outcome = submit_delivery_with(&pool, &request, &fx.options())
        .await
        .unwrap();
    let manifest = outcome
        .manifest()
        .expect("a three-repository delivery is accepted");
    assert_eq!(manifest.commit_count, 3);
    assert_eq!(manifest.integration_status, "passed");
    assert_eq!(commit_count(&pool).await, 3);
    assert_eq!(task_status(&pool, fx.task_id).await, "submitted");
    assert_eq!(audit_count(&pool, "delivery.submitted").await, 1);

    // 逐仓对账：落库的 commit_sha == 工作区实际 HEAD。
    for repo in &fx.repos {
        let (sha, changed): (String, i32) = sqlx::query_as(
            "SELECT commit_sha, changed_files FROM delivery.task_repository_commit \
              WHERE task_run_id = $1 AND repository_id = $2",
        )
        .bind(fx.run_id)
        .bind(repo.repository_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(sha, head_sha(&repo_root(&fx, repo.repository_id)));
        assert_eq!(changed, 1);
    }
}

fn repo_root(fx: &Fx, repository_id: Uuid) -> PathBuf {
    let index = fx
        .repos
        .iter()
        .position(|repo| repo.repository_id == repository_id)
        .expect("unknown repository");
    fx.workspaces[index].0.clone()
}

// ---------------------------------------------------------------------------
// C73 / C87 只读仓出现 commit
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c73_readonly_repository_with_a_commit_rejects_the_whole_order(pool: PgPool) {
    let fx = seed(&pool, "D4-C73", &[("alpha", true), ("beta", false)]).await;
    let alpha_commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    // 只读仓的 HEAD 被动过（执行端漏报它）。
    let _beta_commit = commit_change(&fx.workspaces[1].0, &["beta.md"]);

    let reports = vec![fx.writable_report(0, &alpha_commit, &fx.workspaces[0].1)];
    let outcome = submit_delivery_with(&pool, &fx.request(reports), &fx.options())
        .await
        .unwrap();
    let rejection = outcome.rejection().expect("the whole order is rejected");
    assert_eq!(rejection.code, "readonly_repo_commit");
    assert_eq!(commit_count(&pool).await, 0);
    assert_eq!(task_status(&pool, fx.task_id).await, "running");
    assert_eq!(audit_count(&pool, "delivery.rejected").await, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn c87_readonly_repository_moved_but_not_reported_rejects_the_whole_order(pool: PgPool) {
    let fx = seed(&pool, "D4-C87", &[("alpha", true), ("beta", false)]).await;
    let alpha_commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    // 只读仓动了但**不上报**——契约仓全集校验正是为堵这个洞。
    let _beta_commit = commit_change(&fx.workspaces[1].0, &["beta.md"]);

    let reports = vec![fx.writable_report(0, &alpha_commit, &fx.workspaces[0].1)];
    let outcome = submit_delivery_with(&pool, &fx.request(reports), &fx.options())
        .await
        .unwrap();
    let rejection = outcome.rejection().expect("the whole order is rejected");
    assert_eq!(rejection.code, "readonly_repo_commit");
    assert_eq!(commit_count(&pool).await, 0);
}

// ---------------------------------------------------------------------------
// C74 基线漂移
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c74_baseline_drift_is_rejected(pool: PgPool) {
    let fx = seed(&pool, "D4-C74", &[("alpha", true)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    let mut reports = vec![fx.writable_report(0, &commit, &fx.workspaces[0].1)];
    // 声明的基线不是领取快照。
    reports[0].base_commit = "0000000000000000000000000000000000000000".to_string();

    let outcome = submit_delivery_with(&pool, &fx.request(reports), &fx.options())
        .await
        .unwrap();
    let rejection = outcome.rejection().expect("baseline drift is rejected");
    assert_eq!(rejection.code, "baseline_mismatch");
    assert_eq!(commit_count(&pool).await, 0);
}

// ---------------------------------------------------------------------------
// C75 / C76 / C88 commit 校验
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c75_commit_that_is_not_a_descendant_is_rejected(pool: PgPool) {
    let fx = seed(&pool, "D4-C75", &[("alpha", true)]).await;
    // 工作区的 HEAD 停在基线上；上报另一个 commit（非后代 / ≠ HEAD）。
    let orphan = "1".repeat(40);
    let mut reports = vec![fx.writable_report(0, &orphan, &fx.workspaces[0].1)];
    reports[0].changed_files = 0;

    let outcome = submit_delivery_with(&pool, &fx.request(reports), &fx.options())
        .await
        .unwrap();
    let rejection = outcome
        .rejection()
        .expect("a non-descendant commit is rejected");
    assert_eq!(rejection.code, "commit_not_descendant");
    assert_eq!(commit_count(&pool).await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn c76_nonexistent_commit_is_rejected(pool: PgPool) {
    let fx = seed(&pool, "D4-C76", &[("alpha", true)]).await;
    let mut reports = vec![fx.writable_report(0, &"deadbeef".repeat(5), &fx.workspaces[0].1)];
    reports[0].changed_files = 0;

    let outcome = submit_delivery_with(&pool, &fx.request(reports), &fx.options())
        .await
        .unwrap();
    let rejection = outcome.rejection().expect("a missing commit is rejected");
    assert_eq!(rejection.code, "commit_not_descendant");
    assert_eq!(commit_count(&pool).await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn c88_commit_that_is_not_the_workspace_head_is_rejected(pool: PgPool) {
    let fx = seed(&pool, "D4-C76-HEAD", &[("alpha", true)]).await;
    // 落两个 commit：上报较早的那个——它是基线后代，但不是当前 HEAD。
    let first = commit_change(&fx.workspaces[0].0, &["first.md"]);
    let _second = commit_change(&fx.workspaces[0].0, &["second.md"]);
    let mut reports = vec![fx.writable_report(0, &first, &fx.workspaces[0].1)];
    reports[0].changed_files = 1;

    let outcome = submit_delivery_with(&pool, &fx.request(reports), &fx.options())
        .await
        .unwrap();
    let rejection = outcome
        .rejection()
        .expect("a stale HEAD commit is rejected");
    assert_eq!(rejection.code, "commit_not_descendant");
    assert_eq!(commit_count(&pool).await, 0);
}

// ---------------------------------------------------------------------------
// C77 / C91 仓不在契约
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c77_c91_repository_outside_the_contract_is_rejected(pool: PgPool) {
    let fx = seed(&pool, "D4-C77", &[("alpha", true)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    let mut reports = vec![fx.writable_report(0, &commit, &fx.workspaces[0].1)];
    // 同项目内的另一个合法仓，但不属于本任务契约。
    let other: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, status, tenant_id) \
         VALUES ($1, 'other', 'git@host:other', 'verified', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    reports[0].repository_id = other;

    let outcome = submit_delivery_with(&pool, &fx.request(reports), &fx.options())
        .await
        .unwrap();
    let rejection = outcome
        .rejection()
        .expect("a repository outside the contract is rejected");
    assert_eq!(rejection.code, "repo_not_in_contract");
    assert_eq!(commit_count(&pool).await, 0);

    // C91 的库层防线：交付记录的外键是 (project_id, repository_id) → 项目仓，
    // 跨项目的仓根本写不进交付表。
    let other_project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         SELECT customer_id, 'D4-C91-OTHER', 'other', tenant_id FROM core.project WHERE id = $1 \
         RETURNING id",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let foreign_repo: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, status, tenant_id) \
         VALUES ($1, 'foreign', 'git@host:foreign', 'verified', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(other_project)
    .fetch_one(&pool)
    .await
    .unwrap();
    let insert = sqlx::query(
        "INSERT INTO delivery.task_repository_commit \
         (tenant_id, project_id, task_run_id, repository_id, commit_sha, changed_files) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4, 1)",
    )
    .bind(fx.project_id)
    .bind(fx.run_id)
    .bind(foreign_repo)
    .bind("a".repeat(40))
    .execute(&pool)
    .await;
    assert!(
        insert.is_err(),
        "a repository from another project must be refused by the foreign key"
    );
}

// ---------------------------------------------------------------------------
// C78 / C85 幂等
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c78_repeated_submission_after_success_replays_the_same_manifest(pool: PgPool) {
    let fx = seed(&pool, "D4-C78", &[("alpha", true)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    let reports = vec![fx.writable_report(0, &commit, &fx.workspaces[0].1)];
    let request = fx.request(reports);

    let first = submit_delivery_with(&pool, &request, &fx.options())
        .await
        .unwrap();
    let first_manifest = first
        .manifest()
        .expect("the first submission is accepted")
        .clone();

    // 幂等识别在管线最前部——成功提交后 Run 已非活跃，fencing 会拒绝重试，
    // 幂等若不在最前部就永远到不了。
    let second = submit_delivery_with(&pool, &request, &fx.options())
        .await
        .unwrap();
    let second_manifest = second.manifest().expect("the replay is accepted");
    assert_eq!(second_manifest.commit_count, first_manifest.commit_count);
    assert_eq!(second_manifest.run_id, first_manifest.run_id);
    assert_eq!(
        commit_count(&pool).await,
        1,
        "the replay must not land a second commit row"
    );
    assert_eq!(
        audit_count(&pool, "delivery.submitted").await,
        1,
        "the replay must not produce a second audit"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn c86_same_run_with_a_different_commit_is_a_conflict(pool: PgPool) {
    let fx = seed(&pool, "D4-C86", &[("alpha", true)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    let request = fx.request(vec![fx.writable_report(0, &commit, &fx.workspaces[0].1)]);
    submit_delivery_with(&pool, &request, &fx.options())
        .await
        .unwrap();

    // 同一 Run、同一仓，但提交内容不同 → 指纹不一致 → 409。
    let mut conflicting = request.clone();
    conflicting.repositories[0].commit_sha = "a".repeat(40);
    let error = submit_delivery_with(&pool, &conflicting, &fx.options())
        .await
        .expect_err("a different commit on the same run is a conflict");
    assert!(matches!(error, DeliveryError::SubmitConflict(_)));
    assert_eq!(commit_count(&pool).await, 1);
}

// ---------------------------------------------------------------------------
// C79 integration_status 非法
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c79_invalid_integration_status_is_rejected(pool: PgPool) {
    let fx = seed(&pool, "D4-C79", &[("alpha", true)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    let mut request = fx.request(vec![fx.writable_report(0, &commit, &fx.workspaces[0].1)]);
    request.integration_status = Some("green".to_string());

    let error = submit_delivery_with(&pool, &request, &fx.options())
        .await
        .expect_err("an unknown integration status is refused");
    assert!(matches!(error, DeliveryError::IntegrationStatusInvalid(_)));
    assert_eq!(commit_count(&pool).await, 0);
}

// ---------------------------------------------------------------------------
// C80 租约过期
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c80_expired_lease_does_not_land_a_row(pool: PgPool) {
    let fx = seed(&pool, "D4-C80", &[("alpha", true)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    sqlx::query(
        "UPDATE delivery.task_run SET lease_expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();

    let error = submit_delivery_with(
        &pool,
        &fx.request(vec![fx.writable_report(0, &commit, &fx.workspaces[0].1)]),
        &fx.options(),
    )
    .await
    .expect_err("an expired lease is refused");
    assert!(matches!(error, DeliveryError::LeaseExpired(_)));
    assert_eq!(commit_count(&pool).await, 0);
}

// ---------------------------------------------------------------------------
// C81 not_run 交付
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c81_not_run_delivery_is_recorded_as_a_fact(pool: PgPool) {
    let fx = seed(&pool, "D4-C81", &[("alpha", true)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    // 缺省即 `not_run`——事实记录，**不推断为 failed**。
    let outcome = submit_delivery_with(
        &pool,
        &fx.request(vec![fx.writable_report(0, &commit, &fx.workspaces[0].1)]),
        &fx.options(),
    )
    .await
    .unwrap();
    let manifest = outcome
        .manifest()
        .expect("a not_run delivery is still accepted");
    assert_eq!(manifest.integration_status, "not_run");
    let stored: String =
        sqlx::query_scalar("SELECT integration_status FROM delivery.task_run WHERE id = $1")
            .bind(fx.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored, "not_run");
}

// ---------------------------------------------------------------------------
// C82 / C83 / C90 视图对账
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c82_manifest_view_reports_the_three_states(pool: PgPool) {
    let fx = seed(
        &pool,
        "D4-C82",
        &[("alpha", true), ("beta", true), ("gamma", true)],
    )
    .await;

    // 从未提交 → never_submitted。
    let state: String = sqlx::query_scalar(
        "SELECT delivery_state FROM delivery.v_task_delivery_manifest \
          WHERE task_id = $1 AND repository_id = $2",
    )
    .bind(fx.task_id)
    .bind(fx.repos[0].repository_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "never_submitted");

    // 提交两仓 → partial（分母只计可写仓）。
    //
    // D4 §5：`submitted` ≠ 交付完整——提交 2/3 个仓仍是有效的 D4 提交事实，
    // 整单驳回是 D5 的裁决权，不是 D4 的。
    let mut reports = Vec::new();
    for index in [0usize, 1usize] {
        let commit = commit_change(&fx.workspaces[index].0, &[&format!("f{index}.md")]);
        reports.push(fx.writable_report(index, &commit, &fx.workspaces[index].1));
    }
    let outcome = submit_delivery_with(&pool, &fx.request(reports), &fx.options())
        .await
        .unwrap();
    assert!(
        outcome.manifest().is_some(),
        "a partial delivery is accepted"
    );
    let (expected, actual): (i32, i32) = sqlx::query_as(
        "SELECT expected_delivery_count, actual_delivery_count \
           FROM delivery.v_task_delivery_manifest \
          WHERE task_id = $1 AND repository_id = $2",
    )
    .bind(fx.task_id)
    .bind(fx.repos[0].repository_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(expected, 3);
    assert_eq!(actual, 2);
    let state: String = sqlx::query_scalar(
        "SELECT delivery_state FROM delivery.v_task_delivery_manifest \
          WHERE task_id = $1 AND repository_id = $2",
    )
    .bind(fx.task_id)
    .bind(fx.repos[0].repository_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "partial");

    // 第二个 Run 补齐三仓 → delivered（视图按 latest_run 的现状陈述事实）。
    sqlx::query("UPDATE delivery.task_run SET status = 'succeeded' WHERE id = $1")
        .bind(fx.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let second_run: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 2, 'running', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(fx.project_id)
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    for index in 0..3 {
        let commit = commit_change(&fx.workspaces[index].0, &[&format!("g{index}.md")]);
        sqlx::query(
            "INSERT INTO delivery.task_repository_commit \
             (tenant_id, project_id, task_run_id, repository_id, commit_sha, changed_files) \
             VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4, 1)",
        )
        .bind(fx.project_id)
        .bind(second_run)
        .bind(fx.repos[index].repository_id)
        .bind(commit)
        .execute(&pool)
        .await
        .unwrap();
    }
    let state: String = sqlx::query_scalar(
        "SELECT delivery_state FROM delivery.v_task_delivery_manifest \
          WHERE task_id = $1 AND repository_id = $2",
    )
    .bind(fx.task_id)
    .bind(fx.repos[0].repository_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "delivered");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c90_latest_run_without_submission_does_not_borrow_the_previous_run(pool: PgPool) {
    let fx = seed(&pool, "D4-C90", &[("alpha", true)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    submit_delivery_with(
        &pool,
        &fx.request(vec![fx.writable_report(0, &commit, &fx.workspaces[0].1)]),
        &fx.options(),
    )
    .await
    .unwrap();

    // 第二个 Run（未提交）。旧 Run 已终态，活跃唯一索引不冲突。
    let second_run: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 2, 'succeeded', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(fx.project_id)
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let (latest_run, latest_submitted, state): (Uuid, Uuid, String) = sqlx::query_as(
        "SELECT latest_run_id, latest_submitted_run_id, delivery_state \
           FROM delivery.v_task_delivery_manifest \
          WHERE task_id = $1 AND repository_id = $2",
    )
    .bind(fx.task_id)
    .bind(fx.repos[0].repository_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(latest_run, second_run, "latest_run is the newest attempt");
    assert_eq!(
        latest_submitted, fx.run_id,
        "latest_submitted_run keeps the delivered run"
    );
    assert_eq!(
        state, "never_submitted",
        "the newest run has no delivery of its own"
    );
}

// ---------------------------------------------------------------------------
// C89 changed_files 不一致
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c89_changed_files_must_match_the_git_count(pool: PgPool) {
    let fx = seed(&pool, "D4-C89", &[("alpha", true)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    let mut reports = vec![fx.writable_report(0, &commit, &fx.workspaces[0].1)];
    reports[0].changed_files = 12;

    let outcome = submit_delivery_with(&pool, &fx.request(reports), &fx.options())
        .await
        .unwrap();
    let rejection = outcome.rejection().expect("the count mismatch is rejected");
    assert_eq!(rejection.code, "commit_not_descendant");
    assert_eq!(commit_count(&pool).await, 0);
}

// ---------------------------------------------------------------------------
// C92 驳回审计
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c92_rejection_is_audited_with_a_summary(pool: PgPool) {
    let fx = seed(&pool, "D4-C92", &[("alpha", true)]).await;
    // 基线漂移 → 业务驳回（不是事务失败）：驳回审计可靠存在，0 行 commit。
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    let mut report = fx.writable_report(0, &commit, &fx.workspaces[0].1);
    report.base_commit = "b".repeat(40);
    let outcome = submit_delivery_with(&pool, &fx.request(vec![report]), &fx.options())
        .await
        .unwrap();
    assert!(outcome.rejection().is_some());
    assert_eq!(commit_count(&pool).await, 0);

    let after: serde_json::Value = sqlx::query_scalar(
        "SELECT after_data FROM governance.audit_log WHERE action = 'delivery.rejected'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after["code"], "baseline_mismatch");
    assert_eq!(after["knowledge_proposal_count"], 0);
    assert_eq!(after["repository_count"], 1);
}

// ---------------------------------------------------------------------------
// C93 fencing
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c93_a_stale_lease_generation_cannot_submit(pool: PgPool) {
    let fx = seed(&pool, "D4-C93", &[("alpha", true)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    // 执行权世代已被回收递增——旧执行器永远不能重新获得授权。
    sqlx::query(
        "UPDATE delivery.task_run SET lease_generation = lease_generation + 1 WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();

    let error = submit_delivery_with(
        &pool,
        &fx.request(vec![fx.writable_report(0, &commit, &fx.workspaces[0].1)]),
        &fx.options(),
    )
    .await
    .expect_err("a stale generation is refused");
    assert!(matches!(error, DeliveryError::LeaseInvalid(_)));
    assert_eq!(commit_count(&pool).await, 0);
}

// ---------------------------------------------------------------------------
// C94 git 校验超时
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c94_git_timeout_rejects_the_whole_order(pool: PgPool) {
    let fx = seed(&pool, "D4-C94", &[("alpha", true)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    let mut options = fx.options();
    options.git_timeout = Duration::from_nanos(1);

    let outcome = submit_delivery_with(
        &pool,
        &fx.request(vec![fx.writable_report(0, &commit, &fx.workspaces[0].1)]),
        &options,
    )
    .await
    .unwrap();
    let rejection = outcome
        .rejection()
        .expect("a git timeout rejects the order");
    assert_eq!(rejection.code, "commit_not_descendant");
    assert_eq!(commit_count(&pool).await, 0, "no partial commit may land");
}

// ---------------------------------------------------------------------------
// C95 只读仓不计入应交付数
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c95_readonly_repositories_do_not_count_towards_the_delivery_denominator(pool: PgPool) {
    let fx = seed(&pool, "D4-C95", &[("alpha", true), ("beta", false)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    submit_delivery_with(
        &pool,
        &fx.request(vec![fx.writable_report(0, &commit, &fx.workspaces[0].1)]),
        &fx.options(),
    )
    .await
    .unwrap();

    let (expected, actual): (i32, i32) = sqlx::query_as(
        "SELECT expected_delivery_count, actual_delivery_count \
           FROM delivery.v_task_delivery_manifest \
          WHERE task_id = $1 AND repository_id = $2",
    )
    .bind(fx.task_id)
    .bind(fx.repos[0].repository_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(expected, 1, "only writable repositories count");
    assert_eq!(actual, 1);

    let state: String = sqlx::query_scalar(
        "SELECT delivery_state FROM delivery.v_task_delivery_manifest \
          WHERE task_id = $1 AND repository_id = $2",
    )
    .bind(fx.task_id)
    .bind(fx.repos[1].repository_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "delivered");
}

// ---------------------------------------------------------------------------
// C96 重复 repository_id
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c96_duplicate_repository_id_is_rejected(pool: PgPool) {
    let fx = seed(&pool, "D4-C96", &[("alpha", true)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    let report = fx.writable_report(0, &commit, &fx.workspaces[0].1);
    let error = submit_delivery_with(
        &pool,
        &fx.request(vec![report.clone(), report]),
        &fx.options(),
    )
    .await
    .expect_err("a duplicate repository_id is refused");
    assert!(matches!(error, DeliveryError::InvalidRequest(_)));
    assert_eq!(commit_count(&pool).await, 0);
}

// ---------------------------------------------------------------------------
// C83 视图历史：latest_run 与 latest_submitted_run 分离
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c83_view_separates_the_latest_run_from_the_latest_submitted_run(pool: PgPool) {
    let fx = seed(&pool, "D4-C83", &[("alpha", true), ("beta", true)]).await;

    // 第一个 Run 交付两仓（Run 之后进入 `submitted`，不再是活跃 Run）。
    let mut reports = Vec::new();
    for index in 0..2 {
        let commit = commit_change(&fx.workspaces[index].0, &[&format!("r1-{index}.md")]);
        reports.push(fx.writable_report(index, &commit, &fx.workspaces[index].1));
    }
    submit_delivery_with(&pool, &fx.request(reports), &fx.options())
        .await
        .unwrap();

    // 第二个 Run（attempt_no = 2）也交付两仓。
    sqlx::query("UPDATE delivery.task_run SET status = 'succeeded' WHERE id = $1")
        .bind(fx.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let second_run: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 2, 'succeeded', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(fx.project_id)
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    for index in 0..2 {
        let commit = commit_change(&fx.workspaces[index].0, &[&format!("r2-{index}.md")]);
        sqlx::query(
            "INSERT INTO delivery.task_repository_commit \
             (tenant_id, project_id, task_run_id, repository_id, commit_sha, changed_files) \
             VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4, 1)",
        )
        .bind(fx.project_id)
        .bind(second_run)
        .bind(fx.repos[index].repository_id)
        .bind(commit)
        .execute(&pool)
        .await
        .unwrap();
    }

    let (latest_run, latest_submitted, attempt, state): (Uuid, Uuid, i32, String) = sqlx::query_as(
        "SELECT latest_run_id, latest_submitted_run_id, latest_attempt_no, delivery_state \
               FROM delivery.v_task_delivery_manifest \
              WHERE task_id = $1 AND repository_id = $2",
    )
    .bind(fx.task_id)
    .bind(fx.repos[0].repository_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(latest_run, second_run, "latest_run is the newest attempt");
    assert_eq!(
        latest_submitted, second_run,
        "the newest attempt also submitted, so both pointers agree"
    );
    assert_eq!(attempt, 2);
    assert_eq!(state, "delivered");
}

// ---------------------------------------------------------------------------
// C97 RESTRICT 保交付历史
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c97_deleting_a_run_must_not_drop_the_delivery_history(pool: PgPool) {
    let fx = seed(&pool, "D4-C97", &[("alpha", true)]).await;
    let commit = commit_change(&fx.workspaces[0].0, &["alpha.md"]);
    submit_delivery_with(
        &pool,
        &fx.request(vec![fx.writable_report(0, &commit, &fx.workspaces[0].1)]),
        &fx.options(),
    )
    .await
    .unwrap();
    assert_eq!(commit_count(&pool).await, 1);

    let deleted = sqlx::query("DELETE FROM delivery.task_run WHERE id = $1")
        .bind(fx.run_id)
        .execute(&pool)
        .await;
    assert!(
        deleted.is_err(),
        "deleting a run with delivery records must be refused"
    );
    assert_eq!(commit_count(&pool).await, 1);
}
