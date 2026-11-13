//! T-061 · Outbox 运维服务层验收（`docs/tasks/ai-pm-t061-outbox-events-v1.md` §4）。
//!
//! 覆盖映射：
//!
//! | 用例 | 断言 |
//! |---|---|
//! | E-05 | 死信列表只含本项目；`total` 是项目全部死信数（独立 COUNT） |
//! | E-06 | 重放把死信放回 `pending` 并归零 `attempts` |
//! | E-07 | 重放拒绝**不存在 / 跨项目**事件（统一 `NotFound`，防事件 ID 探测） |
//! | E-08 | 重放拒绝本项目内的**非死信**事件（`NotDeadLetter`） |
//! | E-09 | 重放**不等待**消费完成（不等 worker 领取） |
//! | E-18a | `total` 与分页无关（总数独立于 limit） |
//! | E-21 | 重放后再消费不产生第二份通知（幂等锚点） |

mod common;
use pm_application::outbox_ops::{self, ReplayOutcome};
use sqlx::PgPool;
use uuid::Uuid;


async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    common::bootstrap_system_project(pool, code).await.0
}

/// 直接写一条 outbox 事件（绕过生产端，专注运维层语义）。
async fn seed_event(
    pool: &PgPool,
    project_id: Uuid,
    event_type: &str,
    key: &str,
    status: &str,
) -> Uuid {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.outbox_event \
         (project_id, event_type, aggregate_type, aggregate_id, payload, idempotency_key, \
          status, attempts, last_error, tenant_id) \
         VALUES ($1, $2, 'Task', 'agg-1', '{}'::jsonb, $3, $4, 5, 'boom', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(event_type)
    .bind(key)
    .bind(status)
    .fetch_one(pool)
    .await
    .expect("seed outbox event");
    id
}

// ---------------------------------------------------------------------------
// E-05 / E-18a：死信列表与 total 口径
// ---------------------------------------------------------------------------

/// E-05 + E-18a：列表只含本项目；`total` 是**项目全部死信数**（独立 COUNT，
/// 与 limit 无关）。
#[sqlx::test(migrations = "../../migrations")]
async fn dead_letter_listing_is_project_scoped_and_total_is_the_project_total(pool: PgPool) {
    let project_a = seed_project(&pool, "T061-E05-A").await;
    let project_b = seed_project(&pool, "T061-E05-B").await;

    seed_event(&pool, project_a, "TaskReady", "a-1", "dead_letter").await;
    seed_event(&pool, project_a, "TaskReady", "a-2", "dead_letter").await;
    seed_event(&pool, project_a, "TaskReady", "a-live", "pending").await;
    seed_event(&pool, project_b, "TaskReady", "b-1", "dead_letter").await;

    let listing = outbox_ops::dead_letters(&pool, project_a, 10)
        .await
        .unwrap();
    assert_eq!(
        listing.items.len(),
        2,
        "only this project's dead letters may be listed"
    );
    assert!(
        listing
            .items
            .iter()
            .all(|row| row.last_error.as_deref() == Some("boom")),
        "the failure reason must be visible for triage"
    );
    assert_eq!(
        listing.total, 2,
        "total counts this project's dead letters (not the other project's)"
    );

    // E-18a：total 与 limit 无关（分页条数 ≠ 总数）。
    let paged = outbox_ops::dead_letters(&pool, project_a, 1).await.unwrap();
    assert_eq!(paged.items.len(), 1, "the page is truncated");
    assert_eq!(
        paged.total, 2,
        "total must be the project-wide count, independent of the page size"
    );
}

// ---------------------------------------------------------------------------
// E-06 / E-07 / E-08 / E-09：重放语义
// ---------------------------------------------------------------------------

/// E-06 + E-09：重放把死信放回 `pending` 并归零 `attempts`；**不等待**消费完成。
#[sqlx::test(migrations = "../../migrations")]
async fn replay_requeues_the_dead_letter_and_does_not_wait_for_consumption(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-E06").await;
    let id = seed_event(&pool, project_id, "TaskReady", "k-replay", "dead_letter").await;

    let outcome = outbox_ops::replay(&pool, project_id, id).await.unwrap();
    assert_eq!(outcome, ReplayOutcome::Replayed);

    let row: (String, i32, Option<String>) = sqlx::query_as(
        "SELECT status, attempts, last_error FROM integration.outbox_event WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "pending", "a replayed event goes back to the queue");
    assert_eq!(row.1, 0, "a replayed event starts a fresh retry budget");
    assert_eq!(
        row.2.as_deref(),
        Some("boom"),
        "last_error is kept for the audit trail"
    );
    // E-09：重放只改状态，不消费——事件仍在队列里等 worker。
    let still_pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.outbox_event \
          WHERE id = $1 AND status = 'pending' AND delivered_at IS NULL",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        still_pending, 1,
        "replay must not wait for (or perform) consumption"
    );
}

/// E-07：跨项目事件与不存在事件**统一 `NotFound`**（防事件 ID 探测）。
#[sqlx::test(migrations = "../../migrations")]
async fn replay_refuses_cross_project_and_missing_events_alike(pool: PgPool) {
    let project_a = seed_project(&pool, "T061-E07-A").await;
    let project_b = seed_project(&pool, "T061-E07-B").await;
    let foreign = seed_event(&pool, project_b, "TaskReady", "b-dead", "dead_letter").await;

    // 跨项目：调用方有 A 的权限，却构造 B 的 event_id。
    let outcome = outbox_ops::replay(&pool, project_a, foreign).await.unwrap();
    assert_eq!(
        outcome,
        ReplayOutcome::NotFound,
        "a cross-project event must be indistinguishable from a missing one"
    );

    // 真正不存在：同样的答案。
    let missing = outbox_ops::replay(&pool, project_a, Uuid::new_v4())
        .await
        .unwrap();
    assert_eq!(missing, ReplayOutcome::NotFound);

    // 跨项目事件**未被改动**。
    let status: String =
        sqlx::query_scalar("SELECT status FROM integration.outbox_event WHERE id = $1")
            .bind(foreign)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        status, "dead_letter",
        "a refused replay must not touch the foreign event"
    );
}

/// E-08：本项目内的**非死信**事件不可重放（活事件仍在飞行中）。
#[sqlx::test(migrations = "../../migrations")]
async fn replay_refuses_a_live_event_of_the_same_project(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-E08").await;
    let live = seed_event(&pool, project_id, "TaskReady", "k-live", "pending").await;

    let outcome = outbox_ops::replay(&pool, project_id, live).await.unwrap();
    assert_eq!(
        outcome,
        ReplayOutcome::NotDeadLetter,
        "a live event must not be replayed (it would duplicate delivery)"
    );
}

// ---------------------------------------------------------------------------
// E-21：重放后重复消费不产生第二份通知
// ---------------------------------------------------------------------------

/// E-21：死信被重放并再次消费时，`RunCompletedHandler` 的幂等锚点让审计与
/// 通知**各只有一份**——重放不是「再记一次」。
#[sqlx::test(migrations = "../../migrations")]
async fn a_replayed_run_completed_event_does_not_duplicate_the_notification(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-E21").await;
    let run_id = Uuid::new_v4();
    let event_id = seed_event(&pool, project_id, "RunCompleted", "k-run", "dead_letter").await;

    // 模拟一次已消费（handler 写过审计 + 通知），随后事件被死信。
    let payload =
        serde_json::json!({ "run_id": run_id, "task_id": Uuid::new_v4(), "status": "succeeded" });
    sqlx::query("UPDATE integration.outbox_event SET payload = $2 WHERE id = $1")
        .bind(event_id)
        .bind(&payload)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, action, target_type, target_id, after_data, source_kind, tenant_id) \
         VALUES ($1, $2, 'run_completed_event', 'task_run', $3, $4, 'run_dispatch', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(pm_application::run_dispatch::DISPATCH_SERVICE_PRINCIPAL)
    .bind(run_id)
    .bind(&payload)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO integration.notification (project_id, source_proposal_id, kind, payload, tenant_id) \
         VALUES ($1, NULL, 'run_completed', $2, \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(&payload)
    .execute(&pool)
    .await
    .unwrap();

    // 重放 → 再次消费（幂等锚点命中 → 不重复写）。
    let outcome = outbox_ops::replay(&pool, project_id, event_id)
        .await
        .unwrap();
    assert_eq!(outcome, ReplayOutcome::Replayed);

    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'run_completed_event' AND target_id = $2",
    )
    .bind(project_id)
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let notifications: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.notification \
          WHERE project_id = $1 AND kind = 'run_completed' AND payload->>'run_id' = $2",
    )
    .bind(project_id)
    .bind(run_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1, "the audit fact is recorded exactly once");
    assert_eq!(
        notifications, 1,
        "the notification is produced exactly once"
    );
}

// ---------------------------------------------------------------------------
// 监控指标口径
// ---------------------------------------------------------------------------

/// 监控口径：`failure_rate` 只算**终态**事件（delivered + dead_letter）；
/// `pending` 是「还没处理」不是「处理失败」。
#[sqlx::test(migrations = "../../migrations")]
async fn stats_exclude_pending_from_the_failure_rate(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-STATS").await;
    seed_event(&pool, project_id, "TaskReady", "k-1", "delivered").await;
    seed_event(&pool, project_id, "TaskReady", "k-2", "dead_letter").await;
    seed_event(&pool, project_id, "TaskReady", "k-3", "pending").await;
    seed_event(&pool, project_id, "TaskReady", "k-4", "processing").await;

    let stats = outbox_ops::stats(&pool, project_id).await.unwrap();
    assert_eq!(stats.by_status.pending, 1);
    assert_eq!(stats.by_status.processing, 1);
    assert_eq!(stats.by_status.delivered, 1);
    assert_eq!(stats.by_status.dead_letter, 1);
    assert_eq!(
        stats.failure_rate, 0.5,
        "failure_rate = dead_letter / (delivered + dead_letter); pending must not count"
    );
    assert_eq!(stats.dead_letter_count, 1);
    assert!(
        stats.oldest_pending_age_secs >= 0,
        "the backlog age must be reported (pending + processing share the oldest row)"
    );
    assert_eq!(stats.by_event_type.len(), 1, "one event type is present");
    assert_eq!(stats.by_event_type[0].event_type, "TaskReady");
    assert_eq!(stats.by_event_type[0].dead_letter, 1);
}
