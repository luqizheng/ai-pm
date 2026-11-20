//! S01 · I1-C 验收：Worker claim 两阶段协议（阶段 A 候选发现 → 阶段 B 锁 Project 重查）。
//!
//! 依据：`docs/ui/smark-pm-s01-i1-archive-lifecycle-v1.md` §5.2/§17.1；验收 I1-010/011/014。
//!
//! TaskReady 归档抑制用例在 `crates/pm-infrastructure/tests/outbox_handlers.rs`
//!（依赖方向：infrastructure → application 正确）。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::run_dispatch::{claim_once, DispatchMode, RunDispatchConfig};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_project(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    (boot.project_id, boot.human_id)
}

async fn seed_ready_task_and_run(pool: &PgPool, project_id: Uuid, title: &str) -> Uuid {
    // 判定 4：执行主体必须是 `dev_agent` 执行实例（另造 dev_agent principal，
    // 项目授权行落 ai.project_agent_member——P4-ORG Phase 4：职能 ≠ 成员角色）。
    let dev_agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) \
         VALUES ('dev_agent', $2) RETURNING id",
    )
    .bind(title)
    .bind(format!("dev-agent-{title}"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai.project_agent_member (tenant_id, project_id, principal_id, role_kind) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'developer')",
    )
    .bind(project_id)
    .bind(dev_agent)
    .execute(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, assignee_id, tenant_id) \
         VALUES ($1, $2, 'ready', $3, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .bind(dev_agent)
    .fetch_one(pool)
    .await
    .unwrap();
    // 判定 5/6：至少一个可写仓库且状态 verified、基线可解析。
    let repository_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
            (project_id, name, url, role, status, head_commit_sha, default_branch, tenant_id) \
         VALUES ($1, $2, $3, 'main', 'verified', 'base0', 'main', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(format!("repo-{title}"))
    .bind(format!("git@example.invalid:team/{title}.git"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, base_ref, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(repository_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, run_kind, status, tenant_id) \
         VALUES ($1, $2, 1, 'delivery', 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(pool)
    .await
    .unwrap();
    task_id
}

/// I1-010：归档项目被 claim 跳过后，**其他 Project 的 Run 仍可领取**——
/// 两阶段协议的关键收益（阶段 A 候选 → 阶段 B 按项目行锁 + 重查）。
#[sqlx::test(migrations = "../../migrations")]
async fn claim_skips_archived_project_and_claims_the_next_one(pool: PgPool) {
    let cfg = RunDispatchConfig::from_env();
    // 两个项目各挂一条 queued run；先归档排在前面的第一个。
    let (p1, _o1) = seed_project(&pool, "I1C-1").await;
    let (p2, _o2) = seed_project(&pool, "I1C-2").await;
    seed_ready_task_and_run(&pool, p1, "归档项目任务").await;
    seed_ready_task_and_run(&pool, p2, "活跃项目任务").await;
    sqlx::query(
        "UPDATE delivery.task_run SET next_dispatch_at = now() - interval '10 minutes' \
          WHERE project_id = $1",
    )
    .bind(p1)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE delivery.task_run SET next_dispatch_at = now() - interval '5 minutes' \
          WHERE project_id = $1",
    )
    .bind(p2)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE core.project SET status = 'archived' WHERE id = $1")
        .bind(p1)
        .execute(&pool)
        .await
        .unwrap();

    // 第一次 claim：p1 的候选在阶段 B 被跳过 → 领取 p2 的 run。
    let first = claim_once(&pool, "i1c-worker", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .expect("归档项目被跳过后必须能领取下一项目的 Run");
    assert_eq!(first.kind, pm_application::run_dispatch::ClaimKind::Claimed);
    let claimed_project: Uuid =
        sqlx::query_scalar("SELECT project_id FROM delivery.task_run WHERE id = $1")
            .bind(first.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(claimed_project, p2, "必须领取的是活跃项目的 Run");

    // 第二次 claim：剩余候选只有已归档项目 → None（不报错、不死锁）。
    let second = claim_once(&pool, "i1c-worker", &cfg, DispatchMode::On)
        .await
        .unwrap();
    assert!(second.is_none(), "归档项目的候选必须被合法跳过");

    // 归档项目的 run 保持 queued 原状（claim 不得改它的状态——跳过≠取消）。
    let p1_status: String = sqlx::query_scalar(
        "SELECT r.status FROM delivery.task_run r \
          JOIN delivery.task t ON t.id = r.task_id \
          WHERE r.project_id = $1 LIMIT 1",
    )
    .bind(p1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        p1_status, "queued",
        "跳过不是取消：归档 Run 的终态化归归档事务"
    );
}

/// I1-012/013 归档竞态：创建先拿锁 → 建出的 queued Run 随后被归档事务条件取消；
/// 归档先落定 → claim 跳过（不产生「归档后仍有可领取 queued Run」的窗口）。
#[sqlx::test(migrations = "../../migrations")]
async fn run_created_before_archive_gets_conditionally_cancelled(pool: PgPool) {
    let (project_id, owner) = seed_project(&pool, "I1C-3").await;
    seed_ready_task_and_run(&pool, project_id, "竞态任务").await;

    // 归档（P1 条件取消 queued）。
    let executor = pm_application::run_dispatch::NoopExecutor;
    let outcome = pm_application::archive::archive_project(
        &pool,
        &executor,
        &pm_application::actor::AuditActor::principal(owner),
        SYSTEM_TENANT,
        project_id,
        "i1c-race",
    )
    .await
    .unwrap();
    assert_eq!(outcome.queued_runs_cancelled, 1);

    // 归档之后 claim 不再领取任何 Run。
    let cfg = RunDispatchConfig::from_env();
    let claim = claim_once(&pool, "i1c-worker", &cfg, DispatchMode::On)
        .await
        .unwrap();
    assert!(claim.is_none(), "归档后 claim 必须跳过该项目");
}

/// I1-014：两个活跃项目的 queued Run 并发存在时，claim 依次领取**不产生死锁、
/// 不产生饥饿**（锁序同向 + 每轮只锁一个 Project 的行）。
#[sqlx::test(migrations = "../../migrations")]
async fn claim_drains_multiple_projects_without_deadlock(pool: PgPool) {
    let cfg = RunDispatchConfig::from_env();
    let (p1, _o1) = seed_project(&pool, "I1C-4").await;
    let (p2, _o2) = seed_project(&pool, "I1C-5").await;
    let (p3, _o3) = seed_project(&pool, "I1C-6").await;
    for (pid, idx) in [(p1, 1), (p2, 2), (p3, 3)] {
        seed_ready_task_and_run(&pool, pid, &format!("多项目任务{idx}")).await;
    }

    let mut claimed_projects: Vec<Uuid> = Vec::new();
    for _ in 0..3 {
        let claim = claim_once(&pool, "i1c-worker", &cfg, DispatchMode::On)
            .await
            .unwrap()
            .expect("三个候选项目都应可领取");
        assert_eq!(claim.kind, pm_application::run_dispatch::ClaimKind::Claimed);
        let pid: Uuid =
            sqlx::query_scalar("SELECT project_id FROM delivery.task_run WHERE id = $1")
                .bind(claim.run_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            !claimed_projects.contains(&pid),
            "一个项目只应被领一次：{pid}"
        );
        claimed_projects.push(pid);
    }
    assert_eq!(claimed_projects.len(), 3);
    // 第四次：全部领完 → None。
    let drained = claim_once(&pool, "i1c-worker", &cfg, DispatchMode::On)
        .await
        .unwrap();
    assert!(drained.is_none());
}
