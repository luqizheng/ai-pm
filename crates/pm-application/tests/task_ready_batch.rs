//! DK-D 验收 · E2/E5：AI 永不置 ready + 整批 draft→ready 的人类确认面。
//!
//! E5 的判据是「`TaskReady` outbox 事件数 = 确认任务数」——本文件用真库断言
//! 事件行数，而不是信任服务回包：`update_task` 的直写不产事件（既有事实），
//! 正是这个缺口让「AI 生成任务 → 人类整批确认 → 入调度」断链，本批把它接上。

use std::sync::Arc;

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::command_executor::PgCommandExecutor;
use pm_application::pm_write::{self, CreateTaskRequest, PmWriteError, UpdateTaskRequest};
use pm_application::PolicyEngine;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    human_id: Uuid,
    ai_principal: Uuid,
}

async fn fixture(pool: &PgPool, code: &str) -> Fx {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("ready-{code}"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    Fx {
        project_id: boot.project_id,
        human_id: boot.human_id,
        ai_principal: boot.ai_principal_id,
    }
}

fn executor(pool: &PgPool) -> PgCommandExecutor {
    PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()))
}

async fn draft_task(pool: &PgPool, fx: &Fx, title: &str) -> Uuid {
    pm_write::create_task(
        pool,
        CreateTaskRequest {
            project_id: Some(fx.project_id),
            milestone_id: None,
            title: Some(title.to_string()),
            description: None,
            priority: None,
            assignee_id: None,
            ai_role_version_id: None,
            acceptance_criteria: Some(vec![pm_write::AcceptanceCriterionInput {
                ordinal: None,
                text: format!("{title} 验收通过"),
                verification_kind: Some("manual".to_string()),
            }]),
        },
        &AuditActor::principal(fx.ai_principal),
    )
    .await
    .unwrap()
    .id
}

async fn task_ready_events(pool: &PgPool, project_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM integration.outbox_event \
          WHERE project_id = $1 AND event_type = 'TaskReady'",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn status_of(pool: &PgPool, task_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ─────────────────────────────────────────────────────────────── E5

#[sqlx::test(migrations = "../../migrations")]
async fn batch_ready_emits_exactly_one_task_ready_per_task(pool: PgPool) {
    let fx = fixture(&pool, "E5").await;
    let first = draft_task(&pool, &fx, "任务一").await;
    let second = draft_task(&pool, &fx, "任务二").await;
    let third = draft_task(&pool, &fx, "任务三").await;

    let outcome = pm_write::mark_task_batch_ready(
        &pool,
        &executor(&pool),
        SYSTEM_TENANT,
        fx.project_id,
        &[first, second, third],
        &AuditActor::principal(fx.human_id),
    )
    .await
    .unwrap();
    assert_eq!(outcome.ready.len(), 3);
    assert!(outcome.already_ready.is_empty());
    assert!(outcome.skipped.is_empty());

    // E5：事件数 = 确认任务数，且每条任务的迁移都落了 ready。
    assert_eq!(task_ready_events(&pool, fx.project_id).await, 3);
    for task_id in [first, second, third] {
        assert_eq!(status_of(&pool, task_id).await, "ready");
    }

    // 幂等：再确认一次 → 全部 already_ready，不重复发事件。
    let again = pm_write::mark_task_batch_ready(
        &pool,
        &executor(&pool),
        SYSTEM_TENANT,
        fx.project_id,
        &[first, second, third],
        &AuditActor::principal(fx.human_id),
    )
    .await
    .unwrap();
    assert!(again.ready.is_empty());
    assert_eq!(again.already_ready.len(), 3);
    assert_eq!(
        task_ready_events(&pool, fx.project_id).await,
        3,
        "idempotent re-approval must not duplicate TaskReady"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn batch_ready_skips_tasks_that_already_left_draft(pool: PgPool) {
    let fx = fixture(&pool, "E5-SKIP").await;
    let draft = draft_task(&pool, &fx, "草稿任务").await;
    let blocking = draft_task(&pool, &fx, "已被领取的任务").await;
    // 把第二条推到 running（模拟调度已领取），它不是 draft 就不再接受整批确认。
    sqlx::query("UPDATE delivery.task SET status = 'running' WHERE id = $1")
        .bind(blocking)
        .execute(&pool)
        .await
        .unwrap();

    let outcome = pm_write::mark_task_batch_ready(
        &pool,
        &executor(&pool),
        SYSTEM_TENANT,
        fx.project_id,
        &[draft, blocking],
        &AuditActor::principal(fx.human_id),
    )
    .await
    .unwrap();
    assert_eq!(outcome.ready, vec![draft]);
    assert_eq!(outcome.skipped, vec![blocking]);
    assert_eq!(task_ready_events(&pool, fx.project_id).await, 1);
    assert_eq!(status_of(&pool, blocking).await, "running");
}

// ─────────────────────────────────────────────────────────────── E2

/// E2：两条 AI 可达路径都被拒（批量确认 + 直写 PATCH），且库面无变化。
#[sqlx::test(migrations = "../../migrations")]
async fn ai_can_neither_batch_ready_nor_patch_to_ready(pool: PgPool) {
    let fx = fixture(&pool, "E2").await;
    let task_id = draft_task(&pool, &fx, "AI 生成的任务").await;
    let ai_actor = AuditActor::principal(fx.ai_principal);

    // ① 批量确认入口：服务层 require_human。
    let error = pm_write::mark_task_batch_ready(
        &pool,
        &executor(&pool),
        SYSTEM_TENANT,
        fx.project_id,
        &[task_id],
        &ai_actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, PmWriteError::Forbidden(_)), "got {error:?}");

    // ② REST 直写路径：draft → ready 同样要真人（不然 ① 可被绕过）。
    let error = pm_write::update_task(
        &pool,
        task_id,
        UpdateTaskRequest {
            revision: Some(1),
            title: Some("AI 生成的任务".to_string()),
            description: None,
            status: Some("ready".to_string()),
            priority: Some("normal".to_string()),
            assignee_id: None,
            goal: None,
            constraints: None,
            deliverables: None,
            acceptance_criteria: None,
            knowledge_refs: None,
        },
        &ai_actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, PmWriteError::Forbidden(_)), "got {error:?}");

    assert_eq!(status_of(&pool, task_id).await, "draft");
    assert_eq!(task_ready_events(&pool, fx.project_id).await, 0);

    // ③ 同状态 no-op（ready → ready）不取闸：人类确认后 AI 重放同一请求不应 403。
    pm_write::mark_task_batch_ready(
        &pool,
        &executor(&pool),
        SYSTEM_TENANT,
        fx.project_id,
        &[task_id],
        &AuditActor::principal(fx.human_id),
    )
    .await
    .unwrap();
    let replay = pm_write::update_task(
        &pool,
        task_id,
        UpdateTaskRequest {
            revision: Some(2),
            title: Some("AI 生成的任务".to_string()),
            description: None,
            status: Some("ready".to_string()),
            priority: Some("normal".to_string()),
            assignee_id: None,
            goal: None,
            constraints: None,
            deliverables: None,
            acceptance_criteria: None,
            knowledge_refs: None,
        },
        &ai_actor,
    )
    .await
    .unwrap();
    assert_eq!(replay.status, "ready");
}

/// 批量确认的边界：未知/跨项目 id 一律 NotFound，不泄露存在性。
#[sqlx::test(migrations = "../../migrations")]
async fn batch_ready_rejects_foreign_task_ids(pool: PgPool) {
    let fx = fixture(&pool, "E5-FOREIGN").await;
    let other = fixture(&pool, "E5-FOREIGN-2").await;
    let mine = draft_task(&pool, &fx, "我的任务").await;
    let theirs = draft_task(&pool, &other, "别人的任务").await;

    let error = pm_write::mark_task_batch_ready(
        &pool,
        &executor(&pool),
        SYSTEM_TENANT,
        fx.project_id,
        &[mine, theirs],
        &AuditActor::principal(fx.human_id),
    )
    .await
    .unwrap_err();
    assert!(matches!(error, PmWriteError::NotFound(_)), "got {error:?}");
    assert_eq!(status_of(&pool, mine).await, "draft");
    assert_eq!(task_ready_events(&pool, fx.project_id).await, 0);
}
