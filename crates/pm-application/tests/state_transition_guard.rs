//! R1：Task 状态变更统一经 `pm-domain::validate_transition` 守卫的回归测试。
//!
//! 既有 `update_task_status` 用例过去可直写非法边（如 `draft→completed`）；接入
//! 统一守卫后这些边必须被拒，且拒绝不得改动状态。真实数据库，不 Mock。

use std::sync::Arc;

use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::command_executor::{
    ActorKind, CommandSource, ExecutionContext, PgCommandExecutor,
};
use pm_application::PolicyEngine;
use pm_domain::{CommandError, ProjectCommand, TaskStatus};
use sqlx::PgPool;
use uuid::Uuid;
use pm_domain::SYSTEM_TENANT;

async fn seed(pool: &PgPool, code: &str, status: &str) -> (Uuid, Uuid, Uuid) {
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
    let task: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id) \
         VALUES ($1, 't', $2, 1, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(o.project_id)
    .bind(status.to_string())
    .fetch_one(pool)
    .await
    .unwrap();
    (o.project_id, o.human_id, task)
}

fn ctx(project_id: Uuid, actor_id: Uuid) -> ExecutionContext {
    ExecutionContext {
        project_id,
        actor_id,
        actor_kind: ActorKind::Human,
        source: CommandSource::Bootstrap,
    }
}

fn executor(pool: &PgPool) -> PgCommandExecutor {
    PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()))
}

async fn task_status(pool: &PgPool, task_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn legal_edge_passes_through_the_unified_path(pool: PgPool) {
    let (p, a, t) = seed(&pool, "STG-1", "draft").await;
    let receipt = executor(&pool)
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: t,
                expected_revision: 1,
                new_status: TaskStatus::Ready,
            },
            ctx(p, a),
        )
        .await
        .expect("draft -> ready is a legal edge");
    assert_eq!(receipt.resulting_revision, 2);
    assert_eq!(task_status(&pool, t).await, "ready");
}

#[sqlx::test(migrations = "../../migrations")]
async fn illegal_edge_is_refused_and_leaves_state_untouched(pool: PgPool) {
    let (p, a, t) = seed(&pool, "STG-2", "draft").await;
    let err = executor(&pool)
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: t,
                expected_revision: 1,
                new_status: TaskStatus::Completed,
            },
            ctx(p, a),
        )
        .await
        .expect_err("draft -> completed must be refused by the state machine");
    assert!(
        matches!(err, CommandError::Validation(_)),
        "the guard must reject with a validation error, got {err:?}"
    );
    assert_eq!(task_status(&pool, t).await, "draft", "refused edge must not change state");
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE target_id = $1",
    )
    .bind(t)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 0, "a refused transition must not write an audit row");
}

#[sqlx::test(migrations = "../../migrations")]
async fn running_to_completed_refused_but_running_to_submitted_allowed(pool: PgPool) {
    let (p, a, t) = seed(&pool, "STG-3", "running").await;
    let err = executor(&pool)
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: t,
                expected_revision: 1,
                new_status: TaskStatus::Completed,
            },
            ctx(p, a),
        )
        .await
        .expect_err("running -> completed is not an allowed edge");
    assert!(matches!(err, CommandError::Validation(_)));

    let ok = executor(&pool)
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: t,
                expected_revision: 1,
                new_status: TaskStatus::Submitted,
            },
            ctx(p, a),
        )
        .await
        .expect("running -> submitted is legal when delivery evidence is declared");
    assert_eq!(ok.resulting_revision, 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn dispatch_path_uses_the_same_guard(pool: PgPool) {
    // run_dispatch 的 ready→claimed 经同一服务；此处仅断言 entry 已受守卫约束：
    // task 处于 draft 时领取被 skip（不产生 claimed），证明没有绕过状态机的直写。
    use pm_application::run_dispatch::{claim_once, DispatchMode, RunDispatchConfig};
    let (p, _a, t) = seed(&pool, "STG-4", "draft").await;
    let run: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(p)
    .bind(t)
    .fetch_one(&pool)
    .await
    .unwrap();
    let cfg = RunDispatchConfig {
        lease_secs: 300,
        backoff_cap_secs: 300,
        max_run_attempts: 3,
        max_infrastructure_attempts: 5,
        timeout_secs: 1800,
            deadline_default_secs: 3600,
            agent_heartbeat_stale_secs: 900,
            progress_stale_secs: 3600,
    };
    let out = claim_once(&pool, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(out.kind, pm_application::run_dispatch::ClaimKind::Skipped);
    assert_eq!(task_status(&pool, t).await, "draft", "a non-ready task must not be claimed");
    let run_status: String = sqlx::query_scalar("SELECT status FROM delivery.task_run WHERE id = $1")
        .bind(run)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(run_status, "queued");
}