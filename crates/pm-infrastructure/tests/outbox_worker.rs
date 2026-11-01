use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use pm_domain::SYSTEM_TENANT;
use pm_infrastructure::outbox::{self, NewOutboxEvent};
use pm_infrastructure::outbox_worker::{DispatchOutcome, OutboxHandler, OutboxWorker};
use sqlx::PgPool;
use uuid::Uuid;

struct CountingHandler { calls: AtomicUsize, fail: bool }

#[async_trait::async_trait]
impl OutboxHandler for CountingHandler {
    async fn handle(&self, _event: &outbox::ClaimedOutboxEvent) -> Result<(), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail { Err("handler exploded".to_string()) } else { Ok(()) }
    }
}

async fn seed_event(pool: &PgPool, key: &str) -> Uuid {
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('ow', $1) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
        .fetch_one(pool).await.unwrap();
    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, 'ow', $3) RETURNING id",
    ).bind(customer_id).bind(format!("OW-{key}")).bind(SYSTEM_TENANT).fetch_one(pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let id = outbox::append(&mut tx, NewOutboxEvent {
        project_id,
        event_type: "TaskReady",
        aggregate_type: "Task",
        aggregate_id: "task-1",
        payload: serde_json::json!({}),
        idempotency_key: key,
    }).await.unwrap();
    tx.commit().await.unwrap();
    id
}

#[sqlx::test(migrations = "../../migrations")]
async fn dispatch_delivers_when_the_handler_succeeds(pool: PgPool) {
    seed_event(&pool, "k-ok").await;
    let handler = Arc::new(CountingHandler { calls: AtomicUsize::new(0), fail: false });
    let worker = OutboxWorker::new(pool.clone(), "w".to_string())
        .with_handler("TaskReady", handler.clone());
    assert_eq!(worker.dispatch_once().await.unwrap(), DispatchOutcome::Delivered);
    assert_eq!(handler.calls.load(Ordering::SeqCst), 1);
    assert!(worker.dispatch_once().await.unwrap() == DispatchOutcome::Idle);
}

#[sqlx::test(migrations = "../../migrations")]
async fn handler_failure_hits_the_retry_ceiling_then_dead_letters(pool: PgPool) {
    let id = seed_event(&pool, "k-fail").await;
    let handler = Arc::new(CountingHandler { calls: AtomicUsize::new(0), fail: true });
    // plan 字面指引传 OUTBOX_MAX_ATTEMPTS=5 会让「第二次失败即触顶」无法成立
    // （attempts=2 < 5），此处显式把预算降到 2 以按 plan 意图钉死触顶路径；
    // 生产接线保持 crate 默认 5（见 OutboxWorker::new）。
    let worker = OutboxWorker::new(pool.clone(), "w".to_string())
        .with_retry_limits(2, 5)
        .with_handler("TaskReady", handler);
    // 首次失败 → 重试排队
    assert_eq!(worker.dispatch_once().await.unwrap(), DispatchOutcome::Retried);
    // 让退避立即可到期，第二次失败即触顶 → 死信
    sqlx::query("UPDATE integration.outbox_event SET next_attempt_at = now() WHERE id = $1")
        .bind(id).execute(&pool).await.unwrap();
    assert_eq!(worker.dispatch_once().await.unwrap(), DispatchOutcome::DeadLettered);
    let status: String = sqlx::query_scalar("SELECT status FROM integration.outbox_event WHERE id = $1")
        .bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "dead_letter");
}

#[sqlx::test(migrations = "../../migrations")]
async fn unregistered_event_type_goes_to_dead_letter(pool: PgPool) {
    let id = seed_event(&pool, "k-unreg").await;
    let worker = OutboxWorker::new(pool.clone(), "w".to_string());
    assert_eq!(worker.dispatch_once().await.unwrap(), DispatchOutcome::DeadLettered);
    let reason: Option<String> = sqlx::query_scalar(
        "SELECT last_error FROM integration.outbox_event WHERE id = $1",
    ).bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        reason.as_deref(),
        Some("no handler registered for TaskReady"),
        "an unhandled event must be visible for replay, never silently dropped"
    );
}