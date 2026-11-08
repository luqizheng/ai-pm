//! 0011_run_dispatch 迁移验证（R2 / R3 / S6）：约束被正确替换、唯一活跃索引生效、
//! 无用途旧索引被删除、新列与新表存在。真实数据库，不 Mock。

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
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, $3, $4, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(attempt)
    .bind(status.to_string())
    .fetch_one(pool)
    .await
}

#[sqlx::test(migrations = "../../migrations")]
async fn status_check_accepts_claimed(pool: PgPool) {
    let p = seed_project(&pool, "MIG-1").await;
    let t = seed_task(&pool, p).await;
    insert_run(&pool, p, t, 1, "claimed")
        .await
        .expect("claimed must be a legal task_run status after 0011");
}

#[sqlx::test(migrations = "../../migrations")]
async fn status_check_rejects_unknown_value(pool: PgPool) {
    let p = seed_project(&pool, "MIG-2").await;
    let t = seed_task(&pool, p).await;
    let err = insert_run(&pool, p, t, 1, "bogus").await;
    assert!(err.is_err(), "an unknown status must still be refused");
}

#[sqlx::test(migrations = "../../migrations")]
async fn old_status_constraint_is_replaced_not_stacked(pool: PgPool) {
    let p = seed_project(&pool, "MIG-3").await;
    let t = seed_task(&pool, p).await;
    // 若旧约束未被 DROP，则它与新约束 AND，claimed 会被拒——上面的插入成功即证明已被替换。
    insert_run(&pool, p, t, 1, "claimed").await.unwrap();
    // 谓词按「约束落在 status 列上」限定，避免把别的列的 CHECK（如 0022 的
    // integration_status）误计入。
    let defs: Vec<String> = sqlx::query_scalar(
        "SELECT pg_get_constraintdef(c.oid) FROM pg_constraint c \
          WHERE c.conrelid = 'delivery.task_run'::regclass AND c.contype = 'c' \
            AND c.conkey = ARRAY[(SELECT attnum FROM pg_attribute \
                                  WHERE attrelid = 'delivery.task_run'::regclass \
                                    AND attname = 'status')]",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(defs.len(), 1, "exactly one status CHECK must remain");
    assert!(
        defs[0].contains("claimed"),
        "the surviving CHECK must allow claimed"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn uq_task_run_active_rejects_second_active_run(pool: PgPool) {
    let p = seed_project(&pool, "MIG-4").await;
    let t = seed_task(&pool, p).await;
    insert_run(&pool, p, t, 1, "queued").await.unwrap();
    let err = insert_run(&pool, p, t, 2, "queued").await;
    assert!(
        err.is_err(),
        "I1: a second active run must be refused by the unique index"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn obsolete_index_removed_and_new_index_present(pool: PgPool) {
    let old: Option<String> = sqlx::query_scalar(
        "SELECT indexname FROM pg_indexes WHERE schemaname = 'delivery' AND indexname = 'idx_task_run_active'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(
        old.is_none(),
        "R3: the queued/running-only index must be dropped"
    );
    let new: Option<String> = sqlx::query_scalar(
        "SELECT indexname FROM pg_indexes WHERE schemaname = 'delivery' AND indexname = 'uq_task_run_active'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(
        new.is_some(),
        "the unified unique active-run index must exist"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn new_columns_and_tables_exist(pool: PgPool) {
    // 列存在：能执行投影即证明（任务/仓库列缺失会报错）。
    sqlx::query(
        "SELECT lease_owner, lease_expires_at, heartbeat_at, dispatch_attempts, \
                next_dispatch_at, executor_session, supersedes_run_id, \
                terminated_at, infrastructure_retry_count \
           FROM delivery.task_run LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("new run columns must exist");
    sqlx::query(
        "SELECT task_id, repository_id, write_access FROM delivery.task_repository LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("task_repository must exist");
    sqlx::query(
        "SELECT run_id, repository_id, workspace_path FROM delivery.task_run_repository LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("task_run_repository must exist");
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_repository_cross_project_fk_is_enforced(pool: PgPool) {
    let p1 = seed_project(&pool, "MIG-6A").await;
    let p2 = seed_project(&pool, "MIG-6B").await;
    let t1 = seed_task(&pool, p1).await;
    let other_repo: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, status, tenant_id) \
         VALUES ($1, 'other', 'git@host:other', 'verified', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(p2)
    .fetch_one(&pool)
    .await
    .unwrap();
    let err = sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, tenant_id) \
         VALUES ($1, $2, $3, (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(t1)
    .bind(p1)
    .bind(other_repo)
    .execute(&pool)
    .await;
    assert!(
        err.is_err(),
        "a repository must belong to the same project as the task"
    );
}
