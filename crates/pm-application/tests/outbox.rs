//! Outbox 写入端口的事务语义（spec §20.6.1 / §20.6.3）：事件随业务事务一起
//! 提交才可见、回滚即随之消失、重复的语义事件按幂等键坍缩为一行。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::outbox::{OutboxAppend, OutboxEvent, PgOutboxAppend};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id
}

fn event<'a>(project_id: Uuid, key: &'a str) -> OutboxEvent<'a> {
    OutboxEvent {
        project_id,
        event_type: "TaskReady",
        aggregate_type: "Task",
        aggregate_id: "task-1",
        payload: serde_json::json!({ "task_id": "task-1" }),
        idempotency_key: key,
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn append_rolls_back_with_the_business_transaction(pool: PgPool) {
    let project_id = seed_project(&pool, "OUTBOX-1").await;
    let writer = PgOutboxAppend;

    let mut tx = pool.begin().await.unwrap();
    writer
        .append(&mut tx, event(project_id, "key-rollback"))
        .await
        .unwrap();
    tx.rollback().await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM integration.outbox_event")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "a rolled-back transaction must take its outbox event with it"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn append_is_visible_after_commit(pool: PgPool) {
    let project_id = seed_project(&pool, "OUTBOX-2").await;
    let writer = PgOutboxAppend;

    let mut tx = pool.begin().await.unwrap();
    let id = writer
        .append(&mut tx, event(project_id, "key-commit"))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let (status, key): (String, String) = sqlx::query_as(
        "SELECT status, idempotency_key FROM integration.outbox_event WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "pending");
    assert_eq!(key, "key-commit");
}

#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_idempotency_key_collapses_to_one_row(pool: PgPool) {
    let project_id = seed_project(&pool, "OUTBOX-3").await;
    let writer = PgOutboxAppend;

    let mut tx = pool.begin().await.unwrap();
    let first = writer
        .append(&mut tx, event(project_id, "key-dup"))
        .await
        .unwrap();
    let second = writer
        .append(&mut tx, event(project_id, "key-dup"))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(
        first, second,
        "the same idempotency key must reuse the same event row"
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM integration.outbox_event")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}
