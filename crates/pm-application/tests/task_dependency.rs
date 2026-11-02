//! T-024 Task 依赖与解锁：真实数据库集成测试（验收矩阵 C49–C56 / C58）。
//!
//! 覆盖领取侧判定（前置未满足 → Skip 回退避）、终态触发级联解锁、加边/删边后
//! 的立即解锁、幂等事件与事务原子性。跨项目/环/重复边等写边校验在
//! `apps/pm-api/tests/task_dependencies.rs`（C43–C48 / C57）。

use std::sync::Arc;

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::command_executor::{
    ActorKind, CommandSource, ExecutionContext, PgCommandExecutor,
};
use pm_application::run_dispatch::{ClaimKind, DispatchMode, RunDispatchConfig, claim_once};
use pm_application::task_dependency::{
    DependencyError, add_dependency, list_dependencies, remove_dependency,
};
use pm_application::PolicyEngine;
use pm_domain::{ProjectCommand, SYSTEM_TENANT, TaskStatus};
use pm_infrastructure::outbox::ClaimedOutboxEvent;
use pm_infrastructure::outbox_handlers::TaskReadyHandler;
use pm_infrastructure::outbox_worker::OutboxHandler;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    actor_id: Uuid,
}

async fn seed_project(pool: &PgPool, code: &str) -> Fx {
    let o = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    Fx {
        project_id: o.project_id,
        actor_id: o.human_id,
    }
}

async fn seed_task(pool: &PgPool, project_id: Uuid, status: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id) \
         VALUES ($1, 't', $2, 1, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(status.to_string())
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 种子一条活跃等待（`delivery.task_waiting`）。仓库当前没有业务写入口，
/// 该表只有领取侧读取，因此 C61 只能按库层语义直接种入（T-025 §三）。
async fn seed_waiting(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    reason: &str,
    question: &str,
) {
    sqlx::query(
        "INSERT INTO delivery.task_waiting (project_id, task_id, reason, questions, tenant_id) \
         VALUES ($1, $2, $3, jsonb_build_array($4), \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(reason)
    .bind(question)
    .execute(pool)
    .await
    .unwrap();
}

/// 结束活跃等待（模拟「等待已被解决」这一外部事实）。
async fn end_waiting(pool: &PgPool, task_id: Uuid) {
    sqlx::query(
        "UPDATE delivery.task_waiting SET ended_at = now() \
          WHERE task_id = $1 AND ended_at IS NULL",
    )
    .bind(task_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_run(pool: &PgPool, project_id: Uuid, task_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 领取前置（可领取性判定 §5.6）：指派 dev_agent、一个 verified 且可写的仓库。
async fn make_claimable(pool: &PgPool, project_id: Uuid, task_id: Uuid, code: &str) {
    let agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', $1) RETURNING id",
    )
    .bind(format!("dev {code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task SET assignee_id = $2 WHERE id = $1")
        .bind(task_id)
        .bind(agent)
        .execute(pool)
        .await
        .unwrap();
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
         (project_id, name, url, status, head_commit_sha, tenant_id) \
         VALUES ($1, $2, $3, 'verified', 'headsha', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(format!("repo-{code}"))
    .bind(format!("git@host:{code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository \
         (task_id, project_id, repository_id, write_access, base_ref, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(repo_id)
    .execute(pool)
    .await
    .unwrap();
}

fn cfg() -> RunDispatchConfig {
    RunDispatchConfig {
        lease_secs: 300,
        backoff_cap_secs: 300,
        max_run_attempts: 3,
        max_infrastructure_attempts: 5,
        timeout_secs: 1800,
            deadline_default_secs: 3600,
            agent_heartbeat_stale_secs: 900,
            progress_stale_secs: 3600,
    }
}

fn executor(pool: &PgPool) -> PgCommandExecutor {
    PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()))
}

fn ctx(project_id: Uuid, actor_id: Uuid) -> ExecutionContext {
    ExecutionContext {
        project_id,
        actor: pm_application::actor::AuditActor::principal(actor_id),
        actor_kind: ActorKind::Human,
        source: CommandSource::Bootstrap,
    }
}

async fn task_status(pool: &PgPool, task_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn run_status(pool: &PgPool, run_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM delivery.task_run WHERE id = $1")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn task_ready_events(pool: &PgPool, project_id: Uuid, task_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM integration.outbox_event \
          WHERE project_id = $1 AND event_type = 'TaskReady' AND aggregate_id = $2",
    )
    .bind(project_id)
    .bind(task_id.to_string())
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn transition(
    pool: &PgPool,
    fx: &Fx,
    task_id: Uuid,
    to: TaskStatus,
) {
    executor(pool)
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id,
                expected_revision: 1,
                new_status: to,
            },
            ctx(fx.project_id, fx.actor_id),
        )
        .await
        .expect("transition must succeed");
}

fn actor(fx: &Fx) -> AuditActor {
    AuditActor::principal(fx.actor_id)
}

// ---------------------------------------------------------------- C49 / C50

#[sqlx::test(migrations = "../../migrations")]
async fn c49_unmet_dependency_requeues_the_run_without_failure(pool: PgPool) {
    let fx = seed_project(&pool, "TD-C49").await;
    let upstream = seed_task(&pool, fx.project_id, "running").await;
    let downstream = seed_task(&pool, fx.project_id, "ready").await;
    let run = seed_run(&pool, fx.project_id, downstream).await;
    add_dependency(&pool, downstream, upstream, &actor(&fx))
        .await
        .unwrap_err();
    // 下游是 ready，D6 不允许加边 → 改为直接插边（模拟历史已存在的依赖）。
    sqlx::query(
        "INSERT INTO delivery.task_dependency \
         (tenant_id, project_id, task_id, depends_on_task_id, created_by) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4)",
    )
    .bind(fx.project_id)
    .bind(downstream)
    .bind(upstream)
    .bind(fx.actor_id)
    .execute(&pool)
    .await
    .unwrap();

    let outcome = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .expect("a candidate run exists");
    assert_eq!(outcome.kind, ClaimKind::Skipped);
    assert_eq!(run_status(&pool, run).await, "queued");

    let (reason, attempts): (Option<String>, i32) =
        sqlx::query_as("SELECT release_reason, dispatch_attempts FROM delivery.task_run WHERE id = $1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("dependency_unmet"));
    assert_eq!(attempts, 1);

    let failures: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run_failure WHERE run_id = $1",
    )
    .bind(run)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failures, 0, "unmet dependency is not an execution failure");
    assert_eq!(task_status(&pool, downstream).await, "ready");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c50_satisfied_dependency_allows_claim(pool: PgPool) {
    let fx = seed_project(&pool, "TD-C50").await;
    let upstream = seed_task(&pool, fx.project_id, "completed").await;
    let downstream = seed_task(&pool, fx.project_id, "ready").await;
    make_claimable(&pool, fx.project_id, downstream, "C50").await;
    let run = seed_run(&pool, fx.project_id, downstream).await;
    sqlx::query(
        "INSERT INTO delivery.task_dependency \
         (tenant_id, project_id, task_id, depends_on_task_id, created_by) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4)",
    )
    .bind(fx.project_id)
    .bind(downstream)
    .bind(upstream)
    .bind(fx.actor_id)
    .execute(&pool)
    .await
    .unwrap();

    let outcome = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .expect("a candidate run exists");
    assert_eq!(outcome.kind, ClaimKind::Claimed);
    assert_eq!(run_status(&pool, run).await, "claimed");
    assert_eq!(task_status(&pool, downstream).await, "claimed");
}

// ---------------------------------------------------------------- C51 / C52

#[sqlx::test(migrations = "../../migrations")]
async fn c51_terminal_upstream_unlocks_blocked_downstream(pool: PgPool) {
    let fx = seed_project(&pool, "TD-C51").await;
    let upstream = seed_task(&pool, fx.project_id, "reviewing").await;
    let downstream = seed_task(&pool, fx.project_id, "blocked").await;
    add_dependency(&pool, downstream, upstream, &actor(&fx))
        .await
        .unwrap();
    assert_eq!(task_status(&pool, downstream).await, "blocked");

    transition(&pool, &fx, upstream, TaskStatus::Completed).await;

    assert_eq!(task_status(&pool, downstream).await, "ready");
    assert_eq!(
        task_ready_events(&pool, fx.project_id, downstream).await,
        1,
        "unlock must produce exactly one TaskReady event"
    );

    // 后续领取成功：TaskReady 由 handler 消费后才有 queued Run；此处直接造一条。
    make_claimable(&pool, fx.project_id, downstream, "C51").await;
    let run = seed_run(&pool, fx.project_id, downstream).await;
    let outcome = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .expect("a candidate run exists");
    assert_eq!(outcome.kind, ClaimKind::Claimed);
    assert_eq!(run_status(&pool, run).await, "claimed");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c52_one_of_two_upstreams_completed_keeps_downstream_blocked(pool: PgPool) {
    let fx = seed_project(&pool, "TD-C52").await;
    let done = seed_task(&pool, fx.project_id, "reviewing").await;
    let pending = seed_task(&pool, fx.project_id, "running").await;
    let downstream = seed_task(&pool, fx.project_id, "blocked").await;
    add_dependency(&pool, downstream, done, &actor(&fx))
        .await
        .unwrap();
    add_dependency(&pool, downstream, pending, &actor(&fx))
        .await
        .unwrap();

    transition(&pool, &fx, done, TaskStatus::Completed).await;

    assert_eq!(task_status(&pool, downstream).await, "blocked");
    assert_eq!(
        task_ready_events(&pool, fx.project_id, downstream).await,
        0,
        "an unmet upstream must not produce TaskReady"
    );
}

// --------------------------------------------------------------------- C53

#[sqlx::test(migrations = "../../migrations")]
async fn c53_cancelled_upstream_also_unlocks(pool: PgPool) {
    let fx = seed_project(&pool, "TD-C53").await;
    let upstream = seed_task(&pool, fx.project_id, "reviewing").await;
    let downstream = seed_task(&pool, fx.project_id, "blocked").await;
    add_dependency(&pool, downstream, upstream, &actor(&fx))
        .await
        .unwrap();

    transition(&pool, &fx, upstream, TaskStatus::Cancelled).await;

    assert_eq!(task_status(&pool, downstream).await, "ready");
    assert_eq!(task_ready_events(&pool, fx.project_id, downstream).await, 1);
}

// --------------------------------------------------------------------- C54

#[sqlx::test(migrations = "../../migrations")]
async fn c54_repeated_task_ready_consumption_creates_a_single_run(pool: PgPool) {
    let fx = seed_project(&pool, "TD-C54").await;
    let upstream = seed_task(&pool, fx.project_id, "reviewing").await;
    let downstream = seed_task(&pool, fx.project_id, "blocked").await;
    add_dependency(&pool, downstream, upstream, &actor(&fx))
        .await
        .unwrap();
    transition(&pool, &fx, upstream, TaskStatus::Completed).await;
    make_claimable(&pool, fx.project_id, downstream, "C54").await;

    let event = ClaimedOutboxEvent {
        id: Uuid::new_v4(),
        project_id: fx.project_id,
        event_type: "TaskReady".to_string(),
        aggregate_type: "Task".to_string(),
        aggregate_id: downstream.to_string(),
        payload: serde_json::json!({ "task_id": downstream }),
        attempts: 1,
        schema_version: 1,
        idempotency_key: format!("task:{downstream}:TaskReady:rev2"),
    };
    let handler = TaskReadyHandler::new(pool.clone());
    handler.handle(&event).await.unwrap();
    handler.handle(&event).await.unwrap();

    let runs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run \
          WHERE task_id = $1 AND status IN ('queued', 'claimed', 'running')",
    )
    .bind(downstream)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(runs, 1, "duplicate TaskReady must collapse to one active run");
    assert_eq!(task_ready_events(&pool, fx.project_id, downstream).await, 1);
}

// ---------------------------------------------------------------- C55 / C56

#[sqlx::test(migrations = "../../migrations")]
async fn c55_adding_an_already_satisfied_edge_unlocks_immediately(pool: PgPool) {
    let fx = seed_project(&pool, "TD-C55").await;
    let upstream = seed_task(&pool, fx.project_id, "completed").await;
    let downstream = seed_task(&pool, fx.project_id, "blocked").await;

    let graph = add_dependency(&pool, downstream, upstream, &actor(&fx))
        .await
        .unwrap();

    assert_eq!(graph.upstream.len(), 1);
    assert_eq!(task_status(&pool, downstream).await, "ready");
    assert_eq!(task_ready_events(&pool, fx.project_id, downstream).await, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn c56_removing_an_edge_unlocks_the_downstream(pool: PgPool) {
    let fx = seed_project(&pool, "TD-C56").await;
    let blocking = seed_task(&pool, fx.project_id, "running").await;
    let done = seed_task(&pool, fx.project_id, "completed").await;
    let downstream = seed_task(&pool, fx.project_id, "blocked").await;
    add_dependency(&pool, downstream, blocking, &actor(&fx))
        .await
        .unwrap();
    add_dependency(&pool, downstream, done, &actor(&fx))
        .await
        .unwrap();
    assert_eq!(task_status(&pool, downstream).await, "blocked");

    let removed = remove_dependency(&pool, downstream, blocking, &actor(&fx))
        .await
        .unwrap();
    assert!(removed);
    assert_eq!(task_status(&pool, downstream).await, "ready");
    assert_eq!(task_ready_events(&pool, fx.project_id, downstream).await, 1);

    // 幂等：重复删除不报错、不产生第二条事件。
    let removed_again = remove_dependency(&pool, downstream, blocking, &actor(&fx))
        .await
        .unwrap();
    assert!(!removed_again);
    assert_eq!(task_ready_events(&pool, fx.project_id, downstream).await, 1);
}

// --------------------------------------------------------------------- C58

#[sqlx::test(migrations = "../../migrations")]
async fn c58_rejected_edge_leaves_no_partial_rows_or_audit(pool: PgPool) {
    let fx = seed_project(&pool, "TD-C58").await;
    let a = seed_task(&pool, fx.project_id, "draft").await;
    let b = seed_task(&pool, fx.project_id, "draft").await;
    add_dependency(&pool, b, a, &actor(&fx)).await.unwrap();

    let err = add_dependency(&pool, a, b, &actor(&fx)).await.unwrap_err();
    assert!(matches!(err, DependencyError::Cycle));

    let edges: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_dependency WHERE project_id = $1",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(edges, 1, "the rejected edge must not be persisted");

    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'task.dependency.add'",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1, "the rejected edge must not leave an audit row");

    // 上游仍是 draft，解锁判定不应改动它（D7：draft 不自动进 ready）。
    assert_eq!(task_status(&pool, b).await, "draft");
    let graph = list_dependencies(&pool, SYSTEM_TENANT, b).await.unwrap();
    assert_eq!(graph.upstream.len(), 1);
    assert!(graph.downstream.is_empty());
}

// --------------------------------------------------------------------- C61

/// C61（T-025 §三 / 评审 §3.1）：`blocked` 任务同时存在 `needs_approval` 等待时，
/// 依赖满足**不得**解除阻塞；等待真正结束后，下一次解锁判定必须能放行。
///
/// 现状的故障形态不是「错误执行」而是「状态 + 事件污染」：状态被谎报为 `ready`、
/// 发出 `TaskReady`、建出领不到的 Run（领取侧以 `deny = task_waiting` 拒绝）。
#[sqlx::test(migrations = "../../migrations")]
async fn c61_active_waiting_survives_dependency_satisfaction(pool: PgPool) {
    let fx = seed_project(&pool, "TD-C61").await;
    let upstream = seed_task(&pool, fx.project_id, "reviewing").await;
    let done = seed_task(&pool, fx.project_id, "completed").await;
    let downstream = seed_task(&pool, fx.project_id, "blocked").await;
    add_dependency(&pool, downstream, upstream, &actor(&fx))
        .await
        .unwrap();
    assert_eq!(task_status(&pool, downstream).await, "blocked");

    // 非依赖类阻塞：等待人工审批（按库层语义种入，见 `seed_waiting`）。
    seed_waiting(
        &pool,
        fx.project_id,
        downstream,
        "needs_approval",
        "approve the migration plan",
    )
    .await;

    // 依赖满足的触发点：上游进入终态。
    transition(&pool, &fx, upstream, TaskStatus::Completed).await;

    assert_eq!(
        task_status(&pool, downstream).await,
        "blocked",
        "an active needs_approval must survive dependency satisfaction"
    );
    assert_eq!(
        task_ready_events(&pool, fx.project_id, downstream).await,
        0,
        "no TaskReady may be emitted while a non-dependency blocker is active"
    );

    // 等待解除后的重评估（对称义务）：加一条已满足的边触发一次解锁判定。
    end_waiting(&pool, downstream).await;
    add_dependency(&pool, downstream, done, &actor(&fx))
        .await
        .unwrap();

    assert_eq!(
        task_status(&pool, downstream).await,
        "ready",
        "once the waiting is over, the satisfied graph must unlock"
    );
    assert_eq!(
        task_ready_events(&pool, fx.project_id, downstream).await,
        1,
        "the deferred unlock must produce exactly one TaskReady"
    );
}

// --------------------------------------------------------------------- C62

/// C62（T-025 §四 / 评审 §3.2）：`dependency_unmet` 只退避回队，**不消耗执行失败预算**。
///
/// 口径：连续 6 次（已超过 `max_run_attempts = 3` 与 `max_infrastructure_attempts = 5`）
/// 之后，同一条 Run 仍 `queued`、无 `task_run_failure`、`infrastructure_retry_count`
/// 仍为 0、任务状态/`retry_exhausted`/`attempt_count` 全不变；上游完成后它仍能被
/// 正常领取（退避不是永久禁用）。
#[sqlx::test(migrations = "../../migrations")]
async fn c62_dependency_unmet_never_consumes_the_execution_budget(pool: PgPool) {
    let fx = seed_project(&pool, "TD-C62").await;
    let upstream = seed_task(&pool, fx.project_id, "reviewing").await;
    let downstream = seed_task(&pool, fx.project_id, "ready").await;
    let run = seed_run(&pool, fx.project_id, downstream).await;
    // D6 不允许给 ready 任务加边 → 直接插边（模拟历史已存在的依赖）。
    sqlx::query(
        "INSERT INTO delivery.task_dependency \
         (tenant_id, project_id, task_id, depends_on_task_id, created_by) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4)",
    )
    .bind(fx.project_id)
    .bind(downstream)
    .bind(upstream)
    .bind(fx.actor_id)
    .execute(&pool)
    .await
    .unwrap();

    let task_before: (String, bool, i32) = sqlx::query_as(
        "SELECT status, retry_exhausted, attempt_count FROM delivery.task WHERE id = $1",
    )
    .bind(downstream)
    .fetch_one(&pool)
    .await
    .unwrap();

    for attempt in 1..=6i32 {
        // 只清指数退避，让同一条 Run 立刻重新成为候选；不触碰任何计数。
        sqlx::query("UPDATE delivery.task_run SET next_dispatch_at = now() WHERE id = $1")
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        let outcome = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
            .await
            .unwrap()
            .expect("a candidate run exists");
        assert_eq!(outcome.kind, ClaimKind::Skipped);
        let (reason, attempts, infra, status): (Option<String>, i32, i32, String) =
            sqlx::query_as(
                "SELECT release_reason, dispatch_attempts, infrastructure_retry_count, status \
                   FROM delivery.task_run WHERE id = $1",
            )
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "queued", "round {attempt}: the run must stay claimable");
        assert_eq!(reason.as_deref(), Some("dependency_unmet"));
        assert_eq!(attempts, attempt, "only the dispatch backoff counter moves");
        assert_eq!(infra, 0, "round {attempt}: infra budget must stay untouched");
    }

    let failures: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_run_failure WHERE run_id = $1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(failures, 0, "dependency waits are not execution failures");

    let task_after: (String, bool, i32) = sqlx::query_as(
        "SELECT status, retry_exhausted, attempt_count FROM delivery.task WHERE id = $1",
    )
    .bind(downstream)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(task_after, task_before, "the task must be untouched");

    let skips: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'skip_task_run' \
            AND after_data->>'release_reason' = 'dependency_unmet'",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(skips, 6, "each deferral leaves exactly one audit row");

    // 依赖真正满足后，同一条 Run 仍能被领取（退避不是永久禁用）。
    transition(&pool, &fx, upstream, TaskStatus::Completed).await;
    make_claimable(&pool, fx.project_id, downstream, "C62").await;
    sqlx::query("UPDATE delivery.task_run SET next_dispatch_at = now() WHERE id = $1")
        .bind(run)
        .execute(&pool)
        .await
        .unwrap();
    let claimed = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .expect("the deferred run must become claimable");
    assert_eq!(claimed.kind, ClaimKind::Claimed);
    assert_eq!(run_status(&pool, run).await, "claimed");
}

// --------------------------------------------------------------------- C63

/// C63（T-025 §八 / 评审 §4.1）：`failed` 上游**不算满足**，下游保持阻塞；恢复路径
/// 是人工动作（重跑上游 / 删边 / 换新任务），删边后立即解锁。
#[sqlx::test(migrations = "../../migrations")]
async fn c63_failed_upstream_keeps_the_downstream_blocked(pool: PgPool) {
    let fx = seed_project(&pool, "TD-C63").await;
    let upstream = seed_task(&pool, fx.project_id, "failed").await;
    let downstream = seed_task(&pool, fx.project_id, "blocked").await;

    add_dependency(&pool, downstream, upstream, &actor(&fx))
        .await
        .unwrap();

    assert_eq!(
        task_status(&pool, downstream).await,
        "blocked",
        "a failed upstream must not satisfy a dependency"
    );
    assert_eq!(
        task_ready_events(&pool, fx.project_id, downstream).await,
        0,
        "no TaskReady may be emitted for an unmet (failed) upstream"
    );

    // 恢复路径：人工删掉这条阻塞边（v1 唯一的自动化可达路径）。
    let removed = remove_dependency(&pool, downstream, upstream, &actor(&fx))
        .await
        .unwrap();
    assert!(removed);
    assert_eq!(task_status(&pool, downstream).await, "ready");
    assert_eq!(task_ready_events(&pool, fx.project_id, downstream).await, 1);
}

/// D5 的终态集合：`failed` 必须排除在「已满足」之外（失败上游需重试/取消/取代）。
#[test]
fn d5_satisfied_set_excludes_failed() {
    use pm_application::task_dependency::{SATISFIED_STATUSES, unlocks_downstream};
    assert!(!SATISFIED_STATUSES.contains(&"failed"));
    assert!(unlocks_downstream(TaskStatus::Completed));
    assert!(unlocks_downstream(TaskStatus::Cancelled));
    assert!(unlocks_downstream(TaskStatus::Superseded));
    assert!(!unlocks_downstream(TaskStatus::Failed));
}
// ------------------- T-025 §10.9 遗留二（2026-11-03）-------------------

/// 遗留二：`list_dependencies` 按租户锚定——跨租户 task_id 读不出任何边
/// （即使上层可见性校验被绕过，应用层也不跨租户返回数据）。
#[sqlx::test(migrations = "../../migrations")]
async fn t025_list_dependencies_is_tenant_scoped(pool: PgPool) {
    let fx = seed_project(&pool, "TD-LIST-T").await;
    let other_tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'td-list-other', 'other') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let other_member: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', 'other', true) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(other_tenant)
    .bind(other_member)
    .execute(&pool)
    .await
    .unwrap();

    // 本租户：一条边可见
    let upstream = seed_task(&pool, fx.project_id, "ready").await;
    let downstream = seed_task(&pool, fx.project_id, "draft").await;
    add_dependency(&pool, downstream, upstream, &AuditActor::principal(fx.actor_id))
        .await
        .unwrap();

    let graph = list_dependencies(&pool, SYSTEM_TENANT, downstream).await.unwrap();
    assert_eq!(graph.upstream.len(), 1);

    // 跨租户 task_id（同一 UUID，换租户视角）→ 404 语义（TaskNotFound）
    let err = list_dependencies(&pool, other_tenant, downstream)
        .await
        .unwrap_err();
    assert!(matches!(err, DependencyError::TaskNotFound), "got {err:?}");
}
