//! PA07-IMP-002：Task Revision 不可变快照（PA-07 V1.0 §3.1 / 实施规范 §4.2）。
//!
//! 覆盖：创建即冻结初始 Revision（含验收口径）、实质变更产生新快照且旧行
//! 不变、CAS 冲突不加行、非实质编辑与状态迁移不产生快照、Run 创建固定当时
//! 指针且跨实质变更保持执行依据（审查81）。

use std::sync::Arc;

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::command_executor::{
    ActorKind, CommandSource, ExecutionContext, PgCommandExecutor,
};
use pm_application::pm_write::{
    create_task, update_task, AcceptanceCriterionInput, CreateTaskRequest, PmWriteError,
    UpdateTaskRequest,
};
use pm_application::run_dispatch::insert_task_run;
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

fn actor(fx: &Fx) -> AuditActor {
    AuditActor {
        principal_id: fx.actor_id,
        identity_id: None,
        client: None,
        auth_mode: None,
        claimed_device_id: None,
    }
}

fn create_req(fx: &Fx, title: &str) -> CreateTaskRequest {
    CreateTaskRequest {
        project_id: Some(fx.project_id),
        milestone_id: None,
        title: Some(title.to_string()),
        description: None,
        priority: None,
        assignee_id: None,
        ai_role_version_id: None,
        implementation_baseline_id: None,
        acceptance_criteria: Some(vec![AcceptanceCriterionInput {
            ordinal: None,
            text: "交付物路径可通过验收".to_string(),
            verification_kind: None,
        }]),
        goal: None,
        deliverables: None,
    }
}

/// status/priority 必填（枚举无空哨兵）；同状态 no-op 不触发迁移校验。
fn update_req(revision: i64, goal: Option<Option<String>>) -> UpdateTaskRequest {
    UpdateTaskRequest {
        revision: Some(revision),
        title: Some("t".to_string()),
        description: None,
        status: Some("draft".to_string()),
        priority: Some("normal".to_string()),
        assignee_id: None,
        goal,
        constraints: None,
        deliverables: None,
        acceptance_criteria: None,
        knowledge_refs: None,
    }
}

#[derive(sqlx::FromRow)]
struct RevRow {
    id: Uuid,
    revision_no: i64,
    goal: Option<String>,
    acceptance_criteria: serde_json::Value,
    execution_contract: serde_json::Value,
    change_reason: Option<String>,
}

async fn revisions(pool: &PgPool, task_id: Uuid) -> Vec<RevRow> {
    sqlx::query_as(
        "SELECT id, revision_no, goal, acceptance_criteria, execution_contract, change_reason \
           FROM delivery.task_revision WHERE task_id = $1 ORDER BY revision_no",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn pointer(pool: &PgPool, task_id: Uuid) -> Option<Uuid> {
    sqlx::query_scalar("SELECT current_task_revision_id FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// 建任务 → 唯一初始快照（revision_no=1）：指针落位、验收口径冻结、
/// 执行契约捕获默认契约列。
#[sqlx::test(migrations = "../../migrations")]
async fn create_task_freezes_initial_revision(pool: PgPool) {
    let f = fx(&pool, "tr-imp2-c1").await;
    let task = create_task(&pool, create_req(&f, "初始任务"), &actor(&f))
        .await
        .unwrap();

    let revs = revisions(&pool, task.id).await;
    assert_eq!(revs.len(), 1, "creation must freeze exactly one revision");
    assert_eq!(revs[0].revision_no, 1);
    assert_eq!(revs[0].change_reason.as_deref(), Some("initial"));
    assert_eq!(pointer(&pool, task.id).await, Some(revs[0].id));

    // 验收口径冻结为结构化数组（ordinal/text/verification_kind）。
    let criteria = revs[0].acceptance_criteria.as_array().unwrap();
    assert_eq!(criteria.len(), 1);
    assert_eq!(criteria[0]["text"], "交付物路径可通过验收");
    assert_eq!(criteria[0]["verification_kind"], "manual");
    // 执行契约捕获创建时的契约列（deliverables 默认空数组等）。
    assert_eq!(
        revs[0].execution_contract["deliverables"],
        serde_json::json!([])
    );
}

/// goal 变更 → 新快照（revision_no=2，change_reason=goal）；旧快照内容不变；
/// 指针推进到新快照。
#[sqlx::test(migrations = "../../migrations")]
async fn substantive_update_appends_immutable_revision(pool: PgPool) {
    let f = fx(&pool, "tr-imp2-c2").await;
    let task = create_task(&pool, create_req(&f, "契约任务"), &actor(&f))
        .await
        .unwrap();
    let initial = revisions(&pool, task.id).await;

    update_task(
        &pool,
        task.id,
        update_req(1, Some(Some("重建登录链".to_string()))),
        &actor(&f),
    )
    .await
    .unwrap();

    let revs = revisions(&pool, task.id).await;
    assert_eq!(revs.len(), 2);
    assert_eq!(revs[1].revision_no, 2);
    assert_eq!(revs[1].goal.as_deref(), Some("重建登录链"));
    assert_eq!(revs[1].change_reason.as_deref(), Some("goal"));
    // 旧行不可变：内容保持初始快照。
    assert_eq!(revs[0].goal, initial[0].goal);
    assert_eq!(revs[0].acceptance_criteria, initial[0].acceptance_criteria);
    assert_eq!(pointer(&pool, task.id).await, Some(revs[1].id));
}

/// 乐观锁冲突（409）不加快照行、不推进指针。
#[sqlx::test(migrations = "../../migrations")]
async fn cas_conflict_leaves_revisions_untouched(pool: PgPool) {
    let f = fx(&pool, "tr-imp2-c3").await;
    let task = create_task(&pool, create_req(&f, "冲突任务"), &actor(&f))
        .await
        .unwrap();
    let before = pointer(&pool, task.id).await;

    let err = update_task(
        &pool,
        task.id,
        update_req(99, Some(Some("过期目标".to_string()))),
        &actor(&f),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, PmWriteError::Conflict("revision conflict")));

    assert_eq!(revisions(&pool, task.id).await.len(), 1);
    assert_eq!(pointer(&pool, task.id).await, before);
}

/// 非实质编辑（仅 title/description）不产生快照；状态迁移（命令通道
/// draft→ready）同样不产生快照——状态不是执行契约。
#[sqlx::test(migrations = "../../migrations")]
async fn non_substantive_and_status_transition_do_not_snapshot(pool: PgPool) {
    let f = fx(&pool, "tr-imp2-c4").await;
    let task = create_task(&pool, create_req(&f, "无契约变更任务"), &actor(&f))
        .await
        .unwrap();

    let mut req = update_req(1, None);
    req.title = Some("改标题不改契约".to_string());
    req.description = Some("补充描述".to_string());
    update_task(&pool, task.id, req, &actor(&f)).await.unwrap();
    assert_eq!(
        revisions(&pool, task.id).await.len(),
        1,
        "title/description edit must not snapshot"
    );

    let executor = PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()));
    executor
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: task.id,
                expected_revision: 2,
                new_status: TaskStatus::Ready,
            },
            ExecutionContext {
                project_id: f.project_id,
                actor: actor(&f),
                actor_kind: ActorKind::Human,
                source: CommandSource::HumanAction,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        revisions(&pool, task.id).await.len(),
        1,
        "status transition must not snapshot (state is not contract)"
    );
}

/// Run 创建固定当时的 Revision 指针；之后的实质变更只产生新快照，
/// 已建 Run 的执行依据不变（审查81：Run 快照解释历史执行）。
#[sqlx::test(migrations = "../../migrations")]
async fn run_pins_revision_and_survives_contract_change(pool: PgPool) {
    let f = fx(&pool, "tr-imp2-c5").await;
    let task = create_task(&pool, create_req(&f, "执行任务"), &actor(&f))
        .await
        .unwrap();
    let rev1 = pointer(&pool, task.id).await.unwrap();

    // insert_task_run 前置条件：同事务对 task 行 FOR UPDATE。
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM delivery.task WHERE id = $1 FOR UPDATE")
        .bind(task.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    let run1 = insert_task_run(&mut tx, f.project_id, task.id, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let pinned1: Uuid =
        sqlx::query_scalar("SELECT task_revision_id FROM delivery.task_run WHERE id = $1")
            .bind(run1.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pinned1, rev1);

    update_task(
        &pool,
        task.id,
        update_req(1, Some(Some("修订后的目标".to_string()))),
        &actor(&f),
    )
    .await
    .unwrap();
    let rev2 = pointer(&pool, task.id).await.unwrap();
    assert_ne!(rev1, rev2);

    // 旧 Run 仍固定 rev1；新 Run 固定 rev2。
    let pinned_again: Uuid =
        sqlx::query_scalar("SELECT task_revision_id FROM delivery.task_run WHERE id = $1")
            .bind(run1.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pinned_again, rev1);

    // uq_task_run_active：每任务单活跃 Run——run2 创建前 run1 先到终态
    // （run 7 态：queued/claimed/running/succeeded/failed/timed_out/cancelled）。
    sqlx::query(
        "UPDATE delivery.task_run SET status = 'succeeded', finished_at = now() WHERE id = $1",
    )
    .bind(run1.run_id)
    .execute(&pool)
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM delivery.task WHERE id = $1 FOR UPDATE")
        .bind(task.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    let run2 = insert_task_run(&mut tx, f.project_id, task.id, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let pinned2: Uuid =
        sqlx::query_scalar("SELECT task_revision_id FROM delivery.task_run WHERE id = $1")
            .bind(run2.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pinned2, rev2);

    // rev1 内容仍可完整追溯（不可变性证据）。
    let goal1: Option<String> =
        sqlx::query_scalar("SELECT goal FROM delivery.task_revision WHERE id = $1")
            .bind(rev1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(goal1, None);
}
