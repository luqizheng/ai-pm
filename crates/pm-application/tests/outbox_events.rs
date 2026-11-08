//! T-061 · Outbox 事件目录与 `RunCompleted` 生产者验收
//! （`docs/ai-pm-t061-outbox-events-v1.md` §5 / §6）。
//!
//! 覆盖映射：
//!
//! | 用例 | 断言 |
//! |---|---|
//! | E-01 | `RunCompleted` 在 Run 终态**同事务**产生 |
//! | E-02 | 同一 Run 的终态只产生**一条** `RunCompleted`（并发收尾坍缩） |
//! | E-03 | 幂等键是 `run_completed:<run_id>` |
//! | E-19 | 环境变量**不能**开启无 handler 事件；未登记类型一律禁产 |

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::outbox::{
    event_is_enabled, event_spec, handled_event_types, OutboxAppend, OutboxEvent, PgOutboxAppend,
    CATALOG,
};
use pm_application::run_dispatch::NoopExecutor;
use sqlx::PgPool;
use uuid::Uuid;

use pm_domain::SYSTEM_TENANT;

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .expect("bootstrap project");
    outcome.project_id
}

/// 建一条处于 `claimed` 态的 delivery run（终态路径的前置状态）。
///
/// `executor_kind` 不能是 `'platform'`——0026 的 CHECK 约束把「平台内确定性
/// Reviewer Run」与「真实执行器 Run」在库层隔开。
async fn seed_claimed_run(pool: &PgPool, project_id: Uuid, task_id: Uuid) -> Uuid {
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
         (project_id, task_id, attempt_no, status, run_kind, executor_kind, tenant_id) \
         VALUES ($1, $2, 1, 'claimed', 'delivery', 'codex', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .expect("seed claimed run");
    run_id
}

/// 建一条任务（终态路径需要 task_id）。
async fn seed_task(pool: &PgPool, project_id: Uuid, title: &str) -> Uuid {
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
         VALUES ($1, $2, 'running', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .fetch_one(pool)
    .await
    .expect("seed task");
    task_id
}

// ---------------------------------------------------------------------------
// E-01 / E-02 / E-03：RunCompleted 生产者
// ---------------------------------------------------------------------------

/// E-01：`RunCompleted` 与 Run 终态**同事务**产生。
///
/// 同事务的含义可观察：终态提交后事件立即可见；终态回滚则事件不存在。
#[sqlx::test(migrations = "../../migrations")]
async fn run_completed_is_produced_in_the_terminal_transaction(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-E01").await;
    let task_id = seed_task(&pool, project_id, "t").await;
    let run_id = seed_claimed_run(&pool, project_id, task_id).await;

    pm_application::run_executor::cancel_run(&pool, &NoopExecutor, run_id, "user_cancel")
        .await
        .expect("cancel run");

    let rows: Vec<(String, String, serde_json::Value)> = sqlx::query_as(
        "SELECT event_type, idempotency_key, payload FROM integration.outbox_event \
          WHERE project_id = $1 AND event_type = 'RunCompleted'",
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "the terminal transition must produce exactly one event"
    );
    assert_eq!(rows[0].1, format!("run_completed:{run_id}"));
    assert_eq!(rows[0].2["run_id"], serde_json::json!(run_id));
    assert_eq!(rows[0].2["task_id"], serde_json::json!(task_id));
    assert_eq!(rows[0].2["status"], serde_json::json!("cancelled"));
}

/// E-02：同一 Run 的终态只产生**一条** `RunCompleted`（并发收尾坍缩）。
#[sqlx::test(migrations = "../../migrations")]
async fn a_run_produces_only_one_run_completed_event(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-E02").await;
    let task_id = seed_task(&pool, project_id, "t").await;
    let run_id = seed_claimed_run(&pool, project_id, task_id).await;

    let executor = NoopExecutor;
    // 第二个收尾者到达时 Run 已终态 → 条件 UPDATE 不命中 → 不写事件。
    let first = pm_application::run_executor::cancel_run(&pool, &executor, run_id, "user_cancel")
        .await
        .expect("first terminal transition");
    let second = pm_application::run_executor::cancel_run(&pool, &executor, run_id, "user_cancel")
        .await
        .expect("second terminal transition");
    assert!(first, "the first terminal transition must win");
    assert!(!second, "the second terminal transition must be rejected");

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.outbox_event \
          WHERE project_id = $1 AND event_type = 'RunCompleted'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "a run must not produce a second RunCompleted event"
    );
}

/// E-03：幂等键以 run_id 钉死（重复写入坍缩为同一行）。
#[sqlx::test(migrations = "../../migrations")]
async fn run_completed_idempotency_key_is_pinned_to_run_id(pool: PgPool) {
    let project_id = seed_project(&pool, "T061-E03").await;
    let task_id = seed_task(&pool, project_id, "t").await;
    let run_id = seed_claimed_run(&pool, project_id, task_id).await;

    // 直接走生产端端口写两次同一幂等键：写入时即坍缩。
    let mut tx = pool.begin().await.unwrap();
    let aggregate_id = run_id.to_string();
    let key = format!("run_completed:{run_id}");
    for _ in 0..2 {
        PgOutboxAppend
            .append(
                &mut tx,
                OutboxEvent {
                    project_id,
                    event_type: "RunCompleted",
                    aggregate_type: "TaskRun",
                    aggregate_id: &aggregate_id,
                    payload: serde_json::json!({ "run_id": run_id }),
                    idempotency_key: &key,
                },
            )
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.outbox_event WHERE idempotency_key = $1",
    )
    .bind(&key)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "the same idempotency key must collapse to one row"
    );
}

// ---------------------------------------------------------------------------
// E-19：事件目录（目录优先，环境变量不得突破）
// ---------------------------------------------------------------------------

/// E-19：环境变量**不能**开启无 handler 事件（P0-4）。
///
/// 旧实现里 `AI_PM_OUTBOX_EVENTS` 分支直接 return，管理员配一个无 handler 的
/// 事件类型即可制造悬空事件，随后被 Worker 死信。
///
/// 注：`AI_PM_OUTBOX_EVENTS` 是**进程级全局**，同文件内并行用例会互相干扰。
/// 本用例的断言全部是「env 无关」的（无论 env 设成什么，无 handler 的事件都
/// 必须禁产；未登记类型永远禁产），因此不依赖执行顺序。
#[sqlx::test(migrations = "../../migrations")]
async fn the_env_switch_cannot_enable_events_without_a_handler(_pool: PgPool) {
    // 串行用例：环境变量是进程级全局。
    unsafe {
        std::env::set_var("AI_PM_OUTBOX_EVENTS", "TaskReady,ReviewCompleted");
    }
    assert!(
        !event_is_enabled("ReviewCompleted"),
        "an event without a handler must stay disabled even when the switch lists it"
    );
    assert!(
        !event_is_enabled("KnowledgeBaseWriteRequested"),
        "an event without a handler must stay disabled"
    );
    assert!(
        !event_is_enabled("NeverRegisteredEvent"),
        "an unregistered event type must never be produced"
    );
    assert!(
        event_is_enabled("TaskReady"),
        "a handled event may be selected by the switch"
    );
    unsafe {
        std::env::remove_var("AI_PM_OUTBOX_EVENTS");
    }
}

/// 目录本身：`has_handler` 与 `enabled_by_default` 两个字段各自表达一件事。
///
/// 断言只读目录字段（纯函数，与 env 无关）；「开关不能突破目录」由 E-19 覆盖。
#[sqlx::test(migrations = "../../migrations")]
async fn the_catalog_separates_handler_presence_from_default_enablement(_pool: PgPool) {
    // 无 handler 的事件默认禁产。
    for spec in CATALOG.iter().filter(|spec| !spec.has_handler) {
        assert!(
            !spec.enabled_by_default,
            "`{}` has no handler, so it cannot be enabled by default",
            spec.event_type
        );
    }
    // 有 handler 的事件默认可产。
    for spec in CATALOG.iter().filter(|spec| spec.has_handler) {
        assert!(
            spec.enabled_by_default,
            "`{}` has a handler, so it is enabled by default",
            spec.event_type
        );
    }
    // `RunRequested` 刻意不在目录里（建 Run 由 TaskReady handler 承担）。
    assert!(event_spec("RunRequested").is_none());
    // 目录里「有 handler」的类型集合 = worker 必须装配的集合。
    let handled: Vec<&str> = handled_event_types().collect();
    assert!(handled.contains(&"TaskReady"));
    assert!(handled.contains(&"DeliverySubmitted"));
    assert!(handled.contains(&"RunCompleted"));
    assert!(!handled.contains(&"ReviewCompleted"));
    assert!(!handled.contains(&"KnowledgeBaseWriteRequested"));
}
