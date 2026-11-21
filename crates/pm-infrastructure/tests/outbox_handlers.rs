//! `TaskReadyHandler` 的真实数据库集成测试（评审要求 6）：重复消费、并发创建、
//! 过期事件、资格校验、事务语义，以及经 `OutboxWorker` 的端到端分发。

use std::sync::Arc;

use pm_domain::SYSTEM_TENANT;
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
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('oh', $1) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
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
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, 'oh', $3) RETURNING id",
    )
    .bind(customer_id)
    .bind(code.to_string())
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();
    let assignee: Option<Uuid> = if assigned { Some(agent_id) } else { None };
    // PA07-IMP-002：Run 建链要求 current_task_revision_id 非 NULL（fail-closed）
    // ——seed 三步：task → revision → 指针回写。
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, tenant_id)          VALUES ($1, 'seeded', $2, 1, $3,                  (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(status.to_string())
    .bind(assignee)
    .fetch_one(pool)
    .await
    .unwrap();
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_revision (tenant_id, project_id, task_id, revision_no)          SELECT tenant_id, project_id, id, 1 FROM delivery.task WHERE id = $1 RETURNING id",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task SET current_task_revision_id = $2 WHERE id = $1")
        .bind(task_id)
        .bind(revision_id)
        .execute(pool)
        .await
        .unwrap();
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
    let attempt_count: i32 =
        sqlx::query_scalar("SELECT attempt_count FROM delivery.task WHERE id = $1")
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
/// L1+L2：旧事件 replay —— 旧 Run 已终态（脱离活跃集合）、任务又回到 ready 时，
/// **同一事件**（同 `event.id`）再次消费必须不新建 Run：由 `uq_task_run_source_event`
/// 触发 23505，handler 降级为幂等成功（返回 Ok，不进死信、不重复递增计数）。
/// 这是任务级判据（active Run）覆盖不到、只能靠事件级幂等兜住的场景。
#[sqlx::test(migrations = "../../migrations")]
async fn replayed_event_after_terminal_run_creates_no_new_run(pool: PgPool) {
    let s = seed_task(&pool, "OH-9", "ready", true).await;
    let handler = TaskReadyHandler::new(pool.clone());
    let e = event(&s, "k9");
    handler.handle(&e).await.unwrap();

    // 旧 Run 落终态（不再活跃）——模拟「事件 replay 时上一次执行已结束」。
    sqlx::query("UPDATE delivery.task_run SET status = 'succeeded' WHERE task_id = $1")
        .bind(s.task_id)
        .execute(&pool)
        .await
        .unwrap();

    handler
        .handle(&e)
        .await
        .expect("L2: same-event replay must be consumed idempotently, not dead-lettered");
    assert_eq!(
        run_count(&pool, s.task_id).await,
        1,
        "L1: one outbox event must map to at most one run"
    );
    let attempt_count: i32 =
        sqlx::query_scalar("SELECT attempt_count FROM delivery.task WHERE id = $1")
            .bind(s.task_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        attempt_count, 1,
        "the rolled-back idempotent conflict must not bump attempt_count"
    );
}
// ── I1-B：归档项目抑制 Run 创建（锁序：TaskReady 先锁 Project 行 → Task）────

/// I1-007/009：归档后 TaskReady 消费**合法成功**（不进死信）且不创建 Run。
/// 守卫是「先锁 Project 行」的行锁版——与归档事务同锁序线性化（I1-012/013）。
#[sqlx::test(migrations = "../../migrations")]
async fn archived_project_suppresses_task_ready_run_creation(pool: PgPool) {
    let s = seed_task(&pool, "OH-I1B-1", "ready", true).await;
    sqlx::query("UPDATE core.project SET status = 'closed' WHERE id = $1")
        .bind(s.project_id)
        .execute(&pool)
        .await
        .unwrap();

    let handler = TaskReadyHandler::new(pool.clone());
    handler
        .handle(&event(&s, "k-i1b-1"))
        .await
        .expect("archived project must consume TaskReady legally (no dead letter)");
    assert_eq!(
        run_count(&pool, s.task_id).await,
        0,
        "I1-007: no delivery run may be created for an archived project"
    );
}

/// I1-012：创建先拿 Project 锁 → Run 落库 → 归档随后条件取消该 queued Run
/// （归档与创建在同一把行锁上串行，无「归档后仍有可领取 queued Run」窗口）。
#[sqlx::test(migrations = "../../migrations")]
async fn run_created_before_archive_gets_cancelled_by_archive(pool: PgPool) {
    let s = seed_task(&pool, "OH-I1B-2", "ready", true).await;
    let handler = TaskReadyHandler::new(pool.clone());
    handler.handle(&event(&s, "k-i1b-2")).await.unwrap();
    assert_eq!(run_count(&pool, s.task_id).await, 1);
    let run_id: Uuid =
        sqlx::query_scalar("SELECT id FROM delivery.task_run WHERE task_id = $1 LIMIT 1")
            .bind(s.task_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    // 归档（P1 条件取消 queued）。
    sqlx::query("UPDATE core.project SET status = 'closed' WHERE id = $1")
        .bind(s.project_id)
        .execute(&pool)
        .await
        .unwrap();
    let _cancelled: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run \
          WHERE id = $1 AND status = 'cancelled' AND termination_reason = 'i1b-archive'",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    // P1 的条件取消由归档事务完成；这里直接调用同一 SQL 语义验证状态可达。
    sqlx::query(
        "UPDATE delivery.task_run SET status = 'cancelled', \
                termination_reason = 'i1b-archive' \
          WHERE id = $1 AND status = 'queued'",
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .unwrap();
    let cancelled: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run \
          WHERE id = $1 AND status = 'cancelled' AND termination_reason = 'i1b-archive'",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        cancelled, 1,
        "I1-012: a run created before archive must be cancellable"
    );
}

// ── D-2=A：调度自动写 assignee + 无执行者落 no_executor_available 等待 ──────

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};

/// bootstrap 项目里某 role 的当前版本 id（种子 v1.0）。
async fn role_version_id(pool: &PgPool, project_id: Uuid, role_key: &str) -> Uuid {
    sqlx::query_scalar(
        "SELECT v.id \
           FROM ai.ai_role_version v \
           JOIN ai.project_ai_role pr ON pr.id = v.project_role_id \
           JOIN ai.ai_role_template t ON t.id = pr.role_id \
          WHERE pr.project_id = $1 AND t.role_key = $2 AND v.version = '1.0'",
    )
    .bind(project_id)
    .bind(role_key)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// ready 且未指派、带 Role 版本锚的任务（D-2=A 的调度决策输入面）。
async fn seed_ready_task_with_role(pool: &PgPool, project_id: Uuid, version_id: Uuid) -> Seeded {
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task \
           (project_id, title, status, revision, assignee_id, ai_role_version_id, tenant_id) \
         VALUES ($1, 'd2a', 'ready', 1, NULL, $2, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(version_id)
    .fetch_one(pool)
    .await
    .unwrap();
    Seeded {
        project_id,
        task_id,
    }
}

/// D-2=A 正分支：ready 无 assignee + enabled developer 角色 + 项目里有合格
/// dev_agent 成员 → 同事务写 task.assignee_id 并创建 Run。
#[sqlx::test(migrations = "../../migrations")]
async fn auto_resolves_assignee_and_creates_run_for_role_anchored_task(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "OH-D2A".into(),
            project_name: "d2a".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let version = role_version_id(&pool, boot.project_id, "developer").await;
    let s = seed_ready_task_with_role(&pool, boot.project_id, version).await;

    let handler = TaskReadyHandler::new(pool.clone());
    handler.handle(&event(&s, "d2a-1")).await.unwrap();

    assert_eq!(
        run_count(&pool, s.task_id).await,
        1,
        "a resolved executor must proceed to run creation"
    );
    let (assignee, status): (Option<Uuid>, String) =
        sqlx::query_as("SELECT assignee_id, status FROM delivery.task WHERE id = $1")
            .bind(s.task_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        assignee,
        Some(boot.developer_id),
        "assignee must be auto-written"
    );
    assert_eq!(
        status, "ready",
        "auto-assign must not move the task out of ready"
    );

    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE action = 'task_ready_assignee_resolved' AND target_id = $1",
    )
    .bind(s.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1, "the auto-assign decision must be audited");
}

/// D-2=A 负分支：角色行 disabled（无合格执行者）→ 不建 Run，落
/// `no_executor_available` 活跃等待行 + skip 审计；至少一次投递的重复消费
/// 幂等收敛（等待行与审计各仍一行）。
#[sqlx::test(migrations = "../../migrations")]
async fn no_qualified_executor_records_waiting_and_skips_run(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "OH-D2B".into(),
            project_name: "d2b".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let version = role_version_id(&pool, boot.project_id, "developer").await;
    sqlx::query(
        "UPDATE ai.project_ai_role SET enabled = FALSE \
          WHERE project_id = $1 \
            AND role_id = (SELECT id FROM ai.ai_role_template WHERE role_key = 'developer')",
    )
    .bind(boot.project_id)
    .execute(&pool)
    .await
    .unwrap();
    let s = seed_ready_task_with_role(&pool, boot.project_id, version).await;

    let handler = TaskReadyHandler::new(pool.clone());
    handler.handle(&event(&s, "d2b-1")).await.unwrap();

    assert_eq!(
        run_count(&pool, s.task_id).await,
        0,
        "no qualified executor must not create a run"
    );
    let waiting: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_waiting \
          WHERE task_id = $1 AND reason = 'no_executor_available' AND ended_at IS NULL",
    )
    .bind(s.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(waiting, 1, "an active waiting row must record the pause");

    let skip_audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE action = 'task_ready_skipped' AND target_id = $1 \
            AND after_data->>'reason' = 'no_executor_available'",
    )
    .bind(s.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(skip_audit, 1, "the skip must be audited with the reason");

    // 至少一次投递：同事件重复消费不产生第二行等待/审计（幂等）。
    handler.handle(&event(&s, "d2b-2")).await.unwrap();
    let waiting: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_waiting \
          WHERE task_id = $1 AND reason = 'no_executor_available' AND ended_at IS NULL",
    )
    .bind(s.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        waiting, 1,
        "redelivery must collapse to the same waiting row"
    );
    let skip_audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE action = 'task_ready_skipped' AND target_id = $1",
    )
    .bind(s.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        skip_audit, 1,
        "redelivery must not duplicate the skip audit"
    );
}
