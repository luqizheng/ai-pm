//! T-061 · Worker 装配一致性 + `RunCompletedHandler` 并发幂等验收
//! （`docs/ai-pm-t061-outbox-events-v1.md` §5.6.1 / §6.2 / §6.2.1）。
//!
//! 覆盖映射：
//!
//! | 用例 | 断言 |
//! |---|---|
//! | E-04 | `RunCompletedHandler` 顺序重复消费只写一份审计 + 一份通知 |
//! | E-15 | 启动校验：注册表与 `EventCatalog` 不一致即 panic（fail fast） |
//! | E-17 | **并发**重复消费只写一份审计 + 一份通知（advisory lock） |
//! | E-20 | 审计成功、通知失败 → 两者一起回滚（事件不标 delivered） |
//! | E-23 | `register_all_handlers` 装配集合 == 目录 `has_handler` 全集 |

use std::sync::Arc;

use pm_infrastructure::outbox::{self, ClaimedOutboxEvent};
use pm_infrastructure::outbox_handlers::{register_all_handlers, RunCompletedHandler};
use pm_infrastructure::outbox_worker::{OutboxHandler, OutboxWorker};
use sqlx::PgPool;
use uuid::Uuid;

use pm_domain::SYSTEM_TENANT;

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $1) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer_id)
    .bind(code)
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 写一条 `RunCompleted` 事件并返回它的 claim 形态。
async fn seed_run_completed(pool: &PgPool, project_id: Uuid, run_id: Uuid) -> ClaimedOutboxEvent {
    let mut tx = pool.begin().await.unwrap();
    let id = outbox::append(
        &mut tx,
        outbox::NewOutboxEvent {
            project_id,
            event_type: "RunCompleted",
            aggregate_type: "TaskRun",
            aggregate_id: "run-1",
            payload: serde_json::json!({ "run_id": run_id, "status": "succeeded" }),
            idempotency_key: "run_completed:seed",
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    ClaimedOutboxEvent {
        id,
        project_id,
        event_type: "RunCompleted".to_string(),
        aggregate_type: "TaskRun".to_string(),
        aggregate_id: "run-1".to_string(),
        payload: serde_json::json!({ "run_id": run_id, "status": "succeeded" }),
        attempts: 1,
        schema_version: 1,
        idempotency_key: "run_completed:seed".to_string(),
    }
}

async fn count_audits(pool: &PgPool, project_id: Uuid, run_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'run_completed_event' AND target_id = $2",
    )
    .bind(project_id)
    .bind(run_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn count_notifications(pool: &PgPool, project_id: Uuid, run_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM integration.notification \
          WHERE project_id = $1 AND kind = 'run_completed' AND payload->>'run_id' = $2",
    )
    .bind(project_id)
    .bind(run_id.to_string())
    .fetch_one(pool)
    .await
    .unwrap()
}

// ---------------------------------------------------------------------------
// E-04 / E-17：幂等（顺序 + 并发）
// ---------------------------------------------------------------------------

/// E-04：顺序重复消费只写一份审计 + 一份通知。
#[sqlx::test(migrations = "../../migrations")]
async fn repeated_sequential_consumption_writes_one_audit_and_one_notification(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-E04").await;
    let run_id = Uuid::new_v4();
    let event = seed_run_completed(&pool, project_id, run_id).await;
    let handler = RunCompletedHandler::new(pool.clone());

    handler.handle(&event).await.expect("first consumption");
    handler.handle(&event).await.expect("second consumption");

    assert_eq!(count_audits(&pool, project_id, run_id).await, 1);
    assert_eq!(count_notifications(&pool, project_id, run_id).await, 1);
}

/// E-17：**并发**重复消费只写一份审计 + 一份通知（P0-2）。
///
/// 裸 `WHERE NOT EXISTS` 在这里会双写：A、B 同时查到「不存在」→ 各插一份。
/// 事务级 advisory lock（键 = run_id）把「检查 + 写入」串行化。
#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_duplicate_consumption_writes_one_audit_and_one_notification(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-E17").await;
    let run_id = Uuid::new_v4();
    let event = seed_run_completed(&pool, project_id, run_id).await;
    let handler = Arc::new(RunCompletedHandler::new(pool.clone()));

    let mut tasks = Vec::new();
    for _ in 0..4 {
        let handler = Arc::clone(&handler);
        let event = ClaimedOutboxEvent {
            id: event.id,
            project_id: event.project_id,
            event_type: event.event_type.clone(),
            aggregate_type: event.aggregate_type.clone(),
            aggregate_id: event.aggregate_id.clone(),
            payload: event.payload.clone(),
            attempts: event.attempts,
            schema_version: event.schema_version,
            idempotency_key: event.idempotency_key.clone(),
        };
        tasks.push(tokio::spawn(async move { handler.handle(&event).await }));
    }
    for task in tasks {
        task.await.expect("join").expect("concurrent consumption");
    }

    assert_eq!(
        count_audits(&pool, project_id, run_id).await,
        1,
        "concurrent consumption must not duplicate the audit fact"
    );
    assert_eq!(
        count_notifications(&pool, project_id, run_id).await,
        1,
        "concurrent consumption must not duplicate the notification"
    );
}

// ---------------------------------------------------------------------------
// E-20：审计成功、通知失败 → 整体回滚
// ---------------------------------------------------------------------------

/// E-20：通知写入失败时，**审计一起回滚**——不允许出现「审计成功、通知失败、
/// 却把事件标记为 delivered」。
///
/// 注入方式：让通知插入违反 `integration.notification.source_proposal_id` 的
/// UNIQUE 约束——把审计的 `after_data` 里塞一个已被占用的 proposal id 是不行的
/// （通知载荷与审计载荷独立）。改为**预先占用 notification 的唯一列**不可行
/// （source_proposal_id 必须是 NULL）。因此这里直接断言事务边界：用一个
/// 触发器让通知插入失败。
#[sqlx::test(migrations = "../../migrations")]
async fn a_failed_notification_rolls_back_the_audit_written_in_the_same_transaction(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-E20").await;
    let run_id = Uuid::new_v4();
    let event = seed_run_completed(&pool, project_id, run_id).await;

    // 让通知插入失败（测试专用的触发器）。
    sqlx::query(
        "CREATE OR REPLACE FUNCTION t061_fail_notification() RETURNS trigger AS $$ \
           BEGIN RAISE EXCEPTION 't061: forced notification failure'; END; $$ LANGUAGE plpgsql",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TRIGGER t061_block_notification BEFORE INSERT ON integration.notification \
         FOR EACH ROW EXECUTE FUNCTION t061_fail_notification()",
    )
    .execute(&pool)
    .await
    .unwrap();

    let handler = RunCompletedHandler::new(pool.clone());
    let result = handler.handle(&event).await;
    assert!(result.is_err(), "the handler must report the failure");

    // 关键断言：审计**没有**留下——两者在同一事务，通知失败则审计一起回滚。
    assert_eq!(
        count_audits(&pool, project_id, run_id).await,
        0,
        "a failed notification must roll the audit back (same transaction)"
    );
    assert_eq!(
        count_notifications(&pool, project_id, run_id).await,
        0,
        "no notification may survive a failed transaction"
    );
}

// ---------------------------------------------------------------------------
// E-15 / E-23：装配一致性
// ---------------------------------------------------------------------------

/// E-23：`register_all_handlers` 装配集合 == 目录 `has_handler` 全集。
///
/// 这是「零 Handler 空壳」退化的防线：两个 worker 都必须经统一装配入口，
/// 装配集合必须与目录一致。
#[sqlx::test(migrations = "../../migrations")]
async fn the_shared_assembly_covers_every_catalog_event_with_a_handler(pool: PgPool) {
    let worker = register_all_handlers(OutboxWorker::new(pool.clone(), "w-1".to_string()), &pool);
    let mut registered = worker.handled_event_types();
    registered.sort_unstable();
    let mut expected: Vec<&str> = pm_application::outbox::handled_event_types().collect();
    expected.sort_unstable();

    assert_eq!(
        registered, expected,
        "the shared assembly must cover every catalog event that has a handler"
    );
    assert!(registered.contains(&"TaskReady"));
    assert!(registered.contains(&"DeliverySubmitted"));
    assert!(registered.contains(&"RunCompleted"));
    // 目录一致性校验通过（不 panic）。
    worker.assert_catalog_consistent();
}

/// E-15：注册表与目录不一致即 panic（fail fast）。
///
/// 两个方向都要拦：注册了目录外的类型（拼写错）；目录里有 handler 却漏注册。
#[tokio::test]
#[should_panic(expected = "not in EventCatalog")]
async fn registering_a_type_outside_the_catalog_is_refused() {
    let pool = sqlx::PgPool::connect_lazy("postgres://localhost/none").unwrap();
    let worker = OutboxWorker::new(pool.clone(), "w-2".to_string()).with_handler(
        "TotallyUnknownEvent",
        Arc::new(pm_infrastructure::outbox_handlers::TaskReadyHandler::new(
            pool,
        )),
    );
    worker.assert_catalog_consistent();
}

#[tokio::test]
#[should_panic(expected = "no handler is registered")]
async fn omitting_a_catalog_handler_is_refused() {
    let pool = sqlx::PgPool::connect_lazy("postgres://localhost/none").unwrap();
    let worker = OutboxWorker::new(pool, "w-3".to_string());
    worker.assert_catalog_consistent();
}
