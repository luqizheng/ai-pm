//! `TaskReadyHandler` 的真实数据库集成测试（评审要求 6）：重复消费、并发创建、
//! 过期事件、资格校验、事务语义，以及经 `OutboxWorker` 的端到端分发。

use std::sync::Arc;

use pm_infrastructure::outbox::{self, ClaimedOutboxEvent, NewOutboxEvent};
use pm_infrastructure::outbox_handlers::TaskReadyHandler;
use pm_infrastructure::outbox_worker::{DispatchOutcome, OutboxHandler, OutboxWorker};
use sqlx::PgPool;
use uuid::Uuid;

struct Seeded {
    project_id: Uuid,
    task_id: Uuid,
}

/// customer → Agent principal → project → 一个任务（指定状态与是否指派）。
async fn seed_task(pool: &PgPool, code: &str, status: &str, assigned: bool) -> Seeded {
    let customer_id: Uuid =
        sqlx::query_scalar("INSERT INTO core.customer (name) VALUES ('oh') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    let agent_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('ai_pm', 'agent') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name) VALUES ($1, $2, 'oh') RETURNING id",
    )
    .bind(customer_id)
    .bind(code.to_string())
    .fetch_one(pool)
    .await
    .unwrap();
    let assignee: Option<Uuid> = if assigned { Some(agent_id) } else { None };
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id) \
         VALUES ($1, 'seeded', $2, 1, $3) RETURNING id",
    )
    .bind(project_id)
    .bind(status.to_string())
    .bind(assignee)
    .fetch_one(pool)
    .await
    .unwrap();
    Seeded {
        project_id,
        task_id,
    }
}

fn event(seeded: &Seeded, key: &str) -> ClaimedOutboxEvent {
    ClaimedOutboxEvent {
        id: Uuid::new_v4(),
        project_id: seeded.project_id,
        event_type: "TaskReady".to_string(),
        aggregate_type: "Task".to_string(),
        aggregate_id: seeded.task_id.to_string(),
        payload: serde_json::json!({ "task_id": seeded.task_id }),
        attempts: 1,
        schema_version: 1,
        idempotency_key: key.to_string(),
    }
}

async fn run_count(pool: &PgPool, task_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM delivery.task_run WHERE task_id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn creates_one_queued_run_and_bumps_attempt_count(pool: PgPool) {
    let s = seed_task(&pool, "OH-1", "ready", true).await;
    let handler = TaskReadyHandler::new(pool.clone());
    handler.handle(&event(&s, "k1")).await.unwrap();

    let (status, attempt_no): (String, i32) =
        sqlx::query_as("SELECT status, attempt_no FROM delivery.task_run WHERE task_id = $1")
            .bind(s.task_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((status.as_str(), attempt_no), ("queued", 1));

    let task_status: String = sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(s.task_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        task_status, "ready",
        "creating a queued run must not move the task to running"
    );
    let attempt_count: i32 = sqlx::query_scalar("SELECT attempt_count FROM delivery.task WHERE id = $1")
        .bind(s.task_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempt_count, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_consumption_creates_a_single_run(pool: PgPool) {
    let s = seed_task(&pool, "OH-2", "ready", true).await;
    let handler = TaskReadyHandler::new(pool.clone());
    let e = event(&s, "k2");
    handler.handle(&e).await.unwrap();
    handler.handle(&e).await.unwrap();
    assert_eq!(
        run_count(&pool, s.task_id).await,
        1,
        "at-least-once delivery must not create a second run"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_consumption_creates_a_single_run(pool: PgPool) {
    let s = seed_task(&pool, "OH-3", "ready", true).await;
    let handler = TaskReadyHandler::new(pool.clone());
    let first = event(&s, "k3a");
    let second = event(&s, "k3b");
    let (a, b) = tokio::join!(handler.handle(&first), handler.handle(&second));
    a.unwrap();
    b.unwrap();
    assert_eq!(
        run_count(&pool, s.task_id).await,
        1,
        "the task row lock must serialize concurrent scheduling"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn stale_event_for_non_ready_task_is_consumed_without_a_run(pool: PgPool) {
    let s = seed_task(&pool, "OH-4", "completed", true).await;
    let handler = TaskReadyHandler::new(pool.clone());
    handler.handle(&event(&s, "k4")).await.unwrap();
    assert_eq!(
        run_count(&pool, s.task_id).await,
        0,
        "a task no longer ready must not be scheduled"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_without_assignee_is_not_scheduled(pool: PgPool) {
    let s = seed_task(&pool, "OH-5", "ready", false).await;
    let handler = TaskReadyHandler::new(pool.clone());
    handler.handle(&event(&s, "k5")).await.unwrap();
    assert_eq!(
        run_count(&pool, s.task_id).await,
        0,
        "an unassigned task has no executor to dispatch to"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn malformed_event_fails_without_leaving_a_run(pool: PgPool) {
    let s = seed_task(&pool, "OH-6", "ready", true).await;
    let handler = TaskReadyHandler::new(pool.clone());
    let mut e = event(&s, "k6");
    e.payload = serde_json::json!({});
    e.aggregate_id = "not-a-uuid".to_string();
    assert!(handler.handle(&e).await.is_err());
    assert_eq!(run_count(&pool, s.task_id).await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn worker_dispatches_task_ready_end_to_end(pool: PgPool) {
    let s = seed_task(&pool, "OH-7", "ready", true).await;
    let mut tx = pool.begin().await.unwrap();
    outbox::append(
        &mut tx,
        NewOutboxEvent {
            project_id: s.project_id,
            event_type: "TaskReady",
            aggregate_type: "Task",
            aggregate_id: &s.task_id.to_string(),
            payload: serde_json::json!({ "task_id": s.task_id }),
            idempotency_key: "k7",
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let worker = OutboxWorker::new(pool.clone(), "w7".to_string())
        .with_handler("TaskReady", Arc::new(TaskReadyHandler::new(pool.clone())));
    assert_eq!(
        worker.dispatch_once().await.unwrap(),
        DispatchOutcome::Delivered
    );
    assert_eq!(run_count(&pool, s.task_id).await, 1);
}
/// B1 回归：Run 被领取进入 `claimed` 后，at-least-once 的重复投递必须坍缩为 no-op
/// ——不得新建第二条活跃 Run，也不得因唯一索引冲突而上抛（不进死信）。
#[sqlx::test(migrations = "../../migrations")]
async fn redelivery_does_not_create_a_second_run_after_claim(pool: PgPool) {
    let s = seed_task(&pool, "OH-8", "ready", true).await;
    let handler = TaskReadyHandler::new(pool.clone());
    handler.handle(&event(&s, "k8")).await.unwrap();

    // 模拟被领取：活跃集合必须包含 `claimed`（B1 的统一判据）。
    sqlx::query("UPDATE delivery.task_run SET status = 'claimed' WHERE task_id = $1")
        .bind(s.task_id)
        .execute(&pool)
        .await
        .unwrap();

    handler
        .handle(&event(&s, "k8b"))
        .await
        .expect("B1: redelivery after a claim must be consumed without error");
    assert_eq!(
        run_count(&pool, s.task_id).await,
        1,
        "a claimed active run must collapse redelivery to a no-op"
    );
}