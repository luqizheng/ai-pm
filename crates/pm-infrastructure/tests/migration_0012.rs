//! 0012_task_run_event_idempotency 迁移验证（L1 事件级幂等）：`source_event_id`
//! 列存在、事件级部分唯一索引拒绝「同一事件第二条 Run」、不同事件可并存、NULL
//! 不受约束，且与任务级唯一索引的谓词单一真源一致。真实数据库，不 Mock。

use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('m', $1) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, 'm', $3) RETURNING id",
    )
    .bind(customer_id)
    .bind(code.to_string())
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_task(pool: &PgPool, project_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id) \
         VALUES ($1, 't', 'ready', 1, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_run(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    attempt: i32,
    status: &str,
    source_event_id: Option<Uuid>,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
             (project_id, task_id, attempt_no, status, source_event_id, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(attempt)
    .bind(status.to_string())
    .bind(source_event_id)
    .fetch_one(pool)
    .await
}

#[sqlx::test(migrations = "../../migrations")]
async fn source_event_id_column_exists(pool: PgPool) {
    sqlx::query("SELECT source_event_id FROM delivery.task_run LIMIT 0")
        .fetch_all(&pool)
        .await
        .expect("0012 must add the source_event_id column");
}

/// L1：同一事件至多一条 Run——用**两个不同任务**隔离掉 `uq_task_run_active`，
/// 只让事件级唯一索引生效。
#[sqlx::test(migrations = "../../migrations")]
async fn same_event_cannot_create_a_second_run(pool: PgPool) {
    let p = seed_project(&pool, "MIG12-1").await;
    let t1 = seed_task(&pool, p).await;
    let t2 = seed_task(&pool, p).await;
    let event = Uuid::new_v4();

    insert_run(&pool, p, t1, 1, "queued", Some(event))
        .await
        .expect("first run for the event must succeed");
    let err = insert_run(&pool, p, t2, 1, "queued", Some(event)).await;
    assert!(
        err.is_err(),
        "an outbox event must map to at most one run across tasks"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn distinct_events_can_each_create_a_run(pool: PgPool) {
    let p = seed_project(&pool, "MIG12-2").await;
    let t1 = seed_task(&pool, p).await;
    let t2 = seed_task(&pool, p).await;
    insert_run(&pool, p, t1, 1, "queued", Some(Uuid::new_v4()))
        .await
        .unwrap();
    insert_run(&pool, p, t2, 1, "queued", Some(Uuid::new_v4()))
        .await
        .expect("distinct events must be able to create their own run");
}

/// 恢复路径 / 历史 Run 的 `source_event_id IS NULL`：部分唯一索引不约束空值，
/// 多条 NULL 必须可以并存。
#[sqlx::test(migrations = "../../migrations")]
async fn null_source_event_is_unconstrained(pool: PgPool) {
    let p = seed_project(&pool, "MIG12-3").await;
    let t1 = seed_task(&pool, p).await;
    let t2 = seed_task(&pool, p).await;
    insert_run(&pool, p, t1, 1, "queued", None).await.unwrap();
    insert_run(&pool, p, t2, 1, "queued", None)
        .await
        .expect("NULL source_event_id rows must not be constrained by the partial index");
}

/// 谓词单一真源（L5）：任务级唯一索引的谓词必须覆盖 `ACTIVE_RUN_STATUSES` 常量；
/// 事件级唯一索引必须存在。任一漂移即红。
#[sqlx::test(migrations = "../../migrations")]
async fn active_run_index_predicates_match_the_single_source(pool: PgPool) {
    let active_def: String = sqlx::query_scalar(
        "SELECT indexdef FROM pg_indexes \
          WHERE schemaname = 'delivery' AND indexname = 'uq_task_run_active'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for status in pm_application::run_dispatch::ACTIVE_RUN_STATUSES {
        assert!(
            active_def.contains(status),
            "uq_task_run_active predicate must cover `{status}`; got: {active_def}"
        );
    }
    // D5 收窄（0026）：活跃 delivery Run 的判定必须带 run_kind 谓词，否则一个
    // 活跃 review run 会挡住 delivery run 的创建（§2.1 反例）。
    assert!(
        active_def.contains("run_kind") && active_def.contains("delivery"),
        "uq_task_run_active must be narrowed to run_kind='delivery'; got: {active_def}"
    );

    // D5 对偶约束：同一 task 至多一个活跃 review run（§3.6 R1）。
    let review_def: String = sqlx::query_scalar(
        "SELECT indexdef FROM pg_indexes \
          WHERE schemaname = 'delivery' AND indexname = 'uq_task_review_active'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for status in pm_application::run_dispatch::ACTIVE_REVIEW_STATUSES {
        assert!(
            review_def.contains(status),
            "uq_task_review_active predicate must cover `{status}`; got: {review_def}"
        );
    }
    assert!(
        review_def.contains("run_kind") && review_def.contains("review"),
        "uq_task_review_active must be scoped to run_kind='review'; got: {review_def}"
    );

    let event_def: String = sqlx::query_scalar(
        "SELECT indexdef FROM pg_indexes \
          WHERE schemaname = 'delivery' AND indexname = 'uq_task_run_source_event'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        event_def.contains("source_event_id"),
        "event-level unique index must exist; got: {event_def}"
    );
}

#[test]
fn migration_file_is_present_and_forward() {
    // 0012 为 forward-only：只补列 / 建索引，不删已有列（代码审查锚点）。
    let sql = include_str!("../../../migrations/0012_task_run_event_idempotency.sql");
    assert!(sql.contains("ADD COLUMN IF NOT EXISTS source_event_id"));
    assert!(sql.contains("uq_task_run_source_event"));
    assert!(!sql.contains("DROP COLUMN"));
}
