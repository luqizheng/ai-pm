//! PA07-IMP-005：Product 级验收表达与 Completed 门禁（PA-07 V1.0 §13）。
//!
//! 覆盖：多 Product 验收独立保存（A 通过不满足 B）、Completed 与最后一条
//! 必要验收原子一致（单点执法于 apply_task_transition）、rejected 后 approved
//! 翻转放行（取最新一条）、撤回 Product 退出有效范围后不阻塞、无关联任务
//! 行为不变。

use std::sync::Arc;

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::command_executor::{
    ActorKind, CommandSource, ExecutionContext, PgCommandExecutor,
};
use pm_application::pm_write::PmWriteError;
use pm_application::task_products::{record_product_acceptance, ProductAcceptanceInput};
use pm_application::PolicyEngine;
use pm_domain::{ProjectCommand, TaskStatus, SYSTEM_TENANT};
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    actor_id: Uuid,
}

async fn fx(pool: &PgPool, code: &str) -> Fx {
    let o = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    Fx {
        project_id: o.project_id,
        actor_id: o.human_id,
    }
}

fn actor(fx: &Fx) -> pm_application::actor::AuditActor {
    pm_application::actor::AuditActor {
        principal_id: fx.actor_id,
        identity_id: None,
        client: None,
        auth_mode: None,
        claimed_device_id: None,
    }
}

/// 建 Product（归属新 Team）+ 有效 Task-Product 归属。
async fn seed_task_product(pool: &PgPool, fx: &Fx, task_id: Uuid, name: &str) -> Uuid {
    let product_id: Uuid = sqlx::query_scalar(
        "WITH t AS ( \
             INSERT INTO core.team (tenant_id, name) \
             SELECT tenant_id, $1 || '-team' FROM delivery.task WHERE id = $2 RETURNING tenant_id, id \
         ), p AS ( \
             INSERT INTO core.product (team_id, tenant_id, key, name, status) \
             SELECT id, tenant_id, 'P-' || upper(substring(gen_random_uuid()::text from 1 for 8)), $1, 'normal' FROM t RETURNING id \
         ) SELECT id FROM p",
    )
    .bind(name)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_product (tenant_id, project_id, task_id, product_id) \
         SELECT tenant_id, project_id, $1, $2 FROM delivery.task WHERE id = $1",
    )
    .bind(task_id)
    .bind(product_id)
    .execute(pool)
    .await
    .unwrap();
    let _ = fx;
    product_id
}

async fn seed_reviewing_task(pool: &PgPool, fx: &Fx, code: &str) -> Uuid {
    let _ = code;
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id) \
         VALUES ($1, 't', 'reviewing', 1, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(fx.project_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn record(pool: &PgPool, f: &Fx, task_id: Uuid, product_id: Uuid, result: &str) {
    record_product_acceptance(
        pool,
        SYSTEM_TENANT,
        task_id,
        ProductAcceptanceInput {
            product_id,
            task_revision_id: None,
            result: result.to_string(),
            condition_scope: "product_specific".to_string(),
            evidence_ref: None,
            source: "human".to_string(),
            note: None,
        },
        Some(f.actor_id),
        &actor(f),
    )
    .await
    .unwrap();
}

fn executor(pool: &PgPool) -> PgCommandExecutor {
    PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()))
}

async fn try_complete(
    pool: &PgPool,
    f: &Fx,
    task_id: Uuid,
) -> Result<pm_application::command_executor::ExecutionReceipt, pm_domain::CommandError> {
    executor(pool)
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id,
                expected_revision: 1,
                new_status: TaskStatus::Completed,
            },
            ExecutionContext {
                project_id: f.project_id,
                actor: actor(f),
                actor_kind: ActorKind::Human,
                source: CommandSource::HumanAction,
            },
        )
        .await
}

async fn task_status(pool: &PgPool, task_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// reviewing → completed：无 Product 关联 = 门禁空过（既有行为不变）。
#[sqlx::test(migrations = "../../migrations")]
async fn completed_without_products_is_unchanged(pool: PgPool) {
    let f = fx(&pool, "AC-N1").await;
    let task = seed_reviewing_task(&pool, &f, "N1").await;
    try_complete(&pool, &f, task).await.unwrap();
    assert_eq!(task_status(&pool, task).await, "completed");
}

/// 双 Product：A approved 而 B 无记录 → Completed 被拒；B approved 后放行。
#[sqlx::test(migrations = "../../migrations")]
async fn completed_requires_every_active_product_approved(pool: PgPool) {
    let f = fx(&pool, "AC-D1").await;
    let task = seed_reviewing_task(&pool, &f, "D1").await;
    let a = seed_task_product(&pool, &f, task, "prod-a").await;
    let b = seed_task_product(&pool, &f, task, "prod-b").await;
    record(&pool, &f, task, a, "approved").await;

    let err = try_complete(&pool, &f, task).await.unwrap_err();
    assert!(
        err.to_string().contains("product acceptance incomplete"),
        "got {err:?}"
    );
    assert_eq!(task_status(&pool, task).await, "reviewing");

    record(&pool, &f, task, b, "approved").await;
    try_complete(&pool, &f, task).await.unwrap();
    assert_eq!(task_status(&pool, task).await, "completed");
}

/// 最新一条生效：B rejected → 拒；B 再 approved → 放行（追加式历史不覆盖）。
#[sqlx::test(migrations = "../../migrations")]
async fn latest_acceptance_record_decides(pool: PgPool) {
    let f = fx(&pool, "AC-L1").await;
    let task = seed_reviewing_task(&pool, &f, "L1").await;
    let b = seed_task_product(&pool, &f, task, "prod-b").await;
    record(&pool, &f, task, b, "rejected").await;
    assert!(try_complete(&pool, &f, task).await.is_err());

    record(&pool, &f, task, b, "approved").await;
    try_complete(&pool, &f, task).await.unwrap();

    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_product_acceptance WHERE task_id = $1",
    )
    .bind(task)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rows, 2, "history is append-only");
}

/// 撤回 Product 退出有效范围：其未通过验收不再阻塞 Completed（历史保留）。
#[sqlx::test(migrations = "../../migrations")]
async fn withdrawn_product_exits_completed_gate(pool: PgPool) {
    let f = fx(&pool, "AC-W1").await;
    let task = seed_reviewing_task(&pool, &f, "W1").await;
    let a = seed_task_product(&pool, &f, task, "prod-a").await;
    let gone = seed_task_product(&pool, &f, task, "prod-gone").await;
    record(&pool, &f, task, a, "approved").await;
    record(&pool, &f, task, gone, "rejected").await;

    sqlx::query(
        "UPDATE delivery.task_product SET withdrawn_at = now(), withdrawn_by = $3 \
          WHERE task_id = $1 AND product_id = $2",
    )
    .bind(task)
    .bind(gone)
    .bind(f.actor_id)
    .execute(&pool)
    .await
    .unwrap();

    try_complete(&pool, &f, task).await.unwrap();
    assert_eq!(task_status(&pool, task).await, "completed");
    let gone_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_product_acceptance WHERE product_id = $1",
    )
    .bind(gone)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(gone_rows, 1, "acceptance history preserved");
}

/// 录入面：未关联 Product 的验收 → 拒绝。
#[sqlx::test(migrations = "../../migrations")]
async fn acceptance_requires_active_link(pool: PgPool) {
    let f = fx(&pool, "AC-R1").await;
    let task = seed_reviewing_task(&pool, &f, "R1").await;
    let stranger: Uuid = sqlx::query_scalar(
        "WITH t AS ( \
             INSERT INTO core.team (tenant_id, name) \
             SELECT tenant_id, 'stranger-team' FROM delivery.task WHERE id = $1 RETURNING tenant_id, id \
         ), p AS ( \
             INSERT INTO core.product (team_id, tenant_id, key, name, status) \
             SELECT id, tenant_id, 'P-' || upper(substring(gen_random_uuid()::text from 1 for 8)), $1, 'normal' FROM t RETURNING id \
         ) SELECT id FROM p",
    )
    .bind(task)
    .fetch_one(&pool)
    .await
    .unwrap();

    let err = record_product_acceptance(
        &pool,
        SYSTEM_TENANT,
        task,
        ProductAcceptanceInput {
            product_id: stranger,
            task_revision_id: None,
            result: "approved".to_string(),
            condition_scope: "product_specific".to_string(),
            evidence_ref: None,
            source: "human".to_string(),
            note: None,
        },
        Some(f.actor_id),
        &actor(&f),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, pm_application::task_products::TaskProductError::LinkNotFound),
        "got {err:?}"
    );
}
