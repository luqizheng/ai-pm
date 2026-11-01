//! Outbox 生产写入点（spec §20.6.1 / §20.7 L3）：任务状态迁移在同一条业务事务里
//! 登记异步事件；迁移失败（revision 冲突）则事务回滚，不留下事件。

use std::sync::Arc;

use pm_application::PolicyEngine;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::command_executor::{
    ActorKind, CommandSource, ExecutionContext, PgCommandExecutor,
};
use pm_domain::{CommandError, ProjectCommand, TaskStatus};
use sqlx::PgPool;
use uuid::Uuid;
use pm_domain::SYSTEM_TENANT;

struct Fixture {
    project_id: Uuid,
    actor_id: Uuid,
    task_id: Uuid,
}

async fn seed(pool: &PgPool, code: &str) -> Fixture {
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
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id) \
         VALUES ($1, 'seeded task', 'draft', 1, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    Fixture {
        project_id: outcome.project_id,
        actor_id: outcome.human_id,
        task_id,
    }
}

fn executor(pool: &PgPool) -> PgCommandExecutor {
    PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()))
}

fn ctx(f: &Fixture) -> ExecutionContext {
    ExecutionContext {
        project_id: f.project_id,
        actor_id: f.actor_id,
        actor_kind: ActorKind::Human,
        source: CommandSource::Bootstrap,
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn ready_transition_enqueues_task_ready_in_the_same_transaction(pool: PgPool) {
    let f = seed(&pool, "OUTBOX-W1").await;
    executor(&pool)
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: f.task_id,
                expected_revision: 1,
                new_status: TaskStatus::Ready,
            },
            ctx(&f),
        )
        .await
        .expect("draft -> ready must execute");

    let (event_type, aggregate_id, status, payload): (
        String,
        String,
        String,
        serde_json::Value,
    ) = sqlx::query_as(
        "SELECT event_type, aggregate_id, status, payload \
         FROM integration.outbox_event WHERE project_id = $1",
    )
    .bind(f.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_type, "TaskReady");
    assert_eq!(aggregate_id, f.task_id.to_string());
    assert_eq!(status, "pending");
    assert_eq!(payload["to"], "ready");
}

#[sqlx::test(migrations = "../../migrations")]
async fn revision_conflict_leaves_no_event(pool: PgPool) {
    let f = seed(&pool, "OUTBOX-W2").await;
    let err = executor(&pool)
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: f.task_id,
                expected_revision: 99,
                new_status: TaskStatus::Ready,
            },
            ctx(&f),
        )
        .await
        .expect_err("a stale revision must be refused");
    assert!(matches!(err, CommandError::RevisionConflict { .. }));

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM integration.outbox_event")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "a refused command must not leave an outbox event behind"
    );
}
#[sqlx::test(migrations = "../../migrations")]
async fn submitted_transition_is_gated_off_by_default(pool: PgPool) {
    let f = seed(&pool, "OUTBOX-W3").await;
    sqlx::query("UPDATE delivery.task SET status = 'running' WHERE id = $1")
        .bind(f.task_id)
        .execute(&pool)
        .await
        .unwrap();

    executor(&pool)
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: f.task_id,
                expected_revision: 1,
                new_status: TaskStatus::Submitted,
            },
            ctx(&f),
        )
        .await
        .expect("running -> submitted must execute");

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM integration.outbox_event")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "DeliverySubmitted is gated off until review orchestration lands"
    );
}
/// A port that always fails, to prove the write is part of the business
/// transaction: if the event cannot be written, the status change rolls back.
struct FailingOutbox;

#[async_trait::async_trait]
impl pm_application::outbox::OutboxAppend for FailingOutbox {
    async fn append<'a>(
        &self,
        _tx: &mut sqlx::Transaction<'a, sqlx::Postgres>,
        _event: pm_application::outbox::OutboxEvent<'_>,
    ) -> Result<Uuid, sqlx::Error> {
        Err(sqlx::Error::RowNotFound)
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn outbox_write_failure_rolls_back_the_status_change(pool: PgPool) {
    let f = seed(&pool, "OUTBOX-W5").await;
    let ex = PgCommandExecutor::with_outbox(
        pool.clone(),
        Arc::new(PolicyEngine::for_tests()),
        Arc::new(FailingOutbox),
    );

    let result = ex
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: f.task_id,
                expected_revision: 1,
                new_status: TaskStatus::Ready,
            },
            ctx(&f),
        )
        .await;
    assert!(result.is_err(), "a failed outbox write must fail the command");

    let (status, revision): (String, i64) =
        sqlx::query_as("SELECT status, revision FROM delivery.task WHERE id = $1")
            .bind(f.task_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (status.as_str(), revision),
        ("draft", 1),
        "the status change must roll back with the outbox write"
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM integration.outbox_event")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}