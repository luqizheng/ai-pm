use pm_domain::SYSTEM_TENANT;
use pm_infrastructure::outbox::{self, NewOutboxEvent, OutboxFailOutcome};
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_project(pool: &PgPool) -> Uuid {
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('ob', $1) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .fetch_one(pool).await.unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, 'OB-1', 'ob', $2) RETURNING id",
    ).bind(customer_id).bind(SYSTEM_TENANT).fetch_one(pool).await.unwrap()
}

fn event<'a>(project_id: Uuid, key: &'a str) -> NewOutboxEvent<'a> {
    NewOutboxEvent {
        project_id,
        event_type: "TaskReady",
        aggregate_type: "Task",
        aggregate_id: "task-1",
        payload: serde_json::json!({ "task_id": "task-1" }),
        idempotency_key: key,
    }
}

/// 事务内写入：提交才可见，回滚即随事务消失（§20.6 第 1 条）。
#[sqlx::test(migrations = "../../migrations")]
async fn append_is_visible_only_after_commit(pool: PgPool) {
    let project_id = seed_project(&pool).await;

    let mut tx = pool.begin().await.unwrap();
    let rolled_back = outbox::append(&mut tx, event(project_id, "k-rollback")).await.unwrap();
    tx.rollback().await.unwrap();
    let exists: i64 =
        sqlx::query_scalar("SELECT count(*) FROM integration.outbox_event WHERE id = $1")
            .bind(rolled_back).fetch_one(&pool).await.unwrap();
    assert_eq!(exists, 0, "an event written in a rolled-back tx must not exist");

    let mut tx = pool.begin().await.unwrap();
    outbox::append(&mut tx, event(project_id, "k-commit")).await.unwrap();
    tx.commit().await.unwrap();
    let committed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.outbox_event WHERE idempotency_key = 'k-commit'",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(committed, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn claim_leases_the_event_and_bumps_attempts(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    outbox::append(&mut tx, event(project_id, "k-claim")).await.unwrap();
    tx.commit().await.unwrap();

    let claimed = outbox::claim(&pool, "worker-a", 60).await.expect("one pending event");
    assert_eq!(claimed.event_type, "TaskReady");
    assert_eq!(claimed.attempts, 1, "claiming counts the attempt");
    assert!(
        outbox::claim(&pool, "worker-b", 60).await.is_none(),
        "a live lease must not be handed to a second worker"
    );
}

/// 租约过期 = Worker 崩溃恢复路径：同一行被重投，id 不变（至少一次）。
#[sqlx::test(migrations = "../../migrations")]
async fn expired_lease_is_reclaimed_as_the_same_row(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let id = outbox::append(&mut tx, event(project_id, "k-crash")).await.unwrap();
    tx.commit().await.unwrap();

    let first = outbox::claim(&pool, "worker-a", 0).await.expect("claim with zero lease");
    assert_eq!(first.id, id);
    let second = outbox::claim(&pool, "worker-b", 60).await.expect("expired lease is reclaimable");
    assert_eq!(second.id, first.id, "redelivery must reuse the event row");
    assert_eq!(second.attempts, 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn complete_marks_delivered(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let id = outbox::append(&mut tx, event(project_id, "k-done")).await.unwrap();
    tx.commit().await.unwrap();
    outbox::claim(&pool, "worker-a", 60).await.unwrap();
    outbox::complete(&pool, id).await.unwrap();
    let row: (String, bool) = sqlx::query_as(
        "SELECT status, delivered_at IS NOT NULL FROM integration.outbox_event WHERE id = $1",
    ).bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(row, ("delivered".to_string(), true));
}

#[sqlx::test(migrations = "../../migrations")]
async fn fail_retries_with_backoff_until_the_ceiling(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let id = outbox::append(&mut tx, event(project_id, "k-retry")).await.unwrap();
    tx.commit().await.unwrap();

    outbox::claim(&pool, "worker-a", 60).await.unwrap();
    assert_eq!(
        outbox::fail(&pool, id, "boom", 2, 5).await.unwrap(),
        OutboxFailOutcome::Retried
    );
    let row: (String, i32, String) = sqlx::query_as(
        "SELECT status, attempts, last_error FROM integration.outbox_event WHERE id = $1",
    ).bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(row, ("pending".to_string(), 1, "boom".to_string()));

    // 退避后到期 → 再领取一次（attempts 由 claim 自增到 2）
    sqlx::query("UPDATE integration.outbox_event SET next_attempt_at = now() WHERE id = $1")
        .bind(id).execute(&pool).await.unwrap();
    let second = outbox::claim(&pool, "worker-a", 60).await.expect("backoff elapsed");
    assert_eq!(second.attempts, 2);
    assert_eq!(
        outbox::fail(&pool, id, "boom again", 2, 5).await.unwrap(),
        OutboxFailOutcome::DeadLettered,
        "attempts >= max_attempts must dead-letter"
    );
}
/// 死信落定后的状态可见性（列表 / 重放 / 项目隔离在应用层
/// `pm_application::outbox_ops` 服务中验证，见 `pm-application/tests/outbox_ops.rs`）。
#[sqlx::test(migrations = "../../migrations")]
async fn dead_letter_is_listed_with_its_reason_and_can_be_replayed(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let id = outbox::append(&mut tx, event(project_id, "k-dead")).await.unwrap();
    tx.commit().await.unwrap();
    outbox::dead_letter(&pool, id, "no handler registered for TaskReady").await.unwrap();

    let row: (String, Option<String>) = sqlx::query_as(
        "SELECT status, last_error FROM integration.outbox_event WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "dead_letter");
    assert_eq!(
        row.1.as_deref(),
        Some("no handler registered for TaskReady")
    );
}

/// 未注册 handler 的事件被 Worker 死信后可被重新领取（replay 路径的底层语义）。
#[sqlx::test(migrations = "../../migrations")]
async fn a_dead_lettered_event_can_be_parked_and_reclaimed(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let id = outbox::append(&mut tx, event(project_id, "k-reclaim")).await.unwrap();
    tx.commit().await.unwrap();
    outbox::dead_letter(&pool, id, "no handler registered for TaskReady").await.unwrap();

    // 重放的等价底层操作（应用层服务用带 project_id 的原子 UPDATE 实现）。
    sqlx::query(
        "UPDATE integration.outbox_event \
         SET status = 'pending', attempts = 0, next_attempt_at = now() \
         WHERE id = $1 AND status = 'dead_letter'",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    let reclaimed = outbox::claim(&pool, "worker-a", 60).await.expect("replayed event is claimable");
    assert_eq!(reclaimed.id, id);
    assert_eq!(reclaimed.attempts, 1, "replay resets attempts");
}