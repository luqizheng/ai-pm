//! Command channel core tests (Task 6, Step 1): execute + audit + compensation.
//!
//! Seeding uses raw SQL (Task 8's bootstrap service does not exist yet);
//! table shapes mirror `migrations/0001_core.sql` and `0002_delivery.sql`.

use std::sync::Arc;

use pm_application::command_executor::{
    ActorKind, CommandSource, CompensationSpec, CompensableTable, ExecutionContext, PgCommandExecutor,
};
use pm_application::PolicyEngine;
use pm_domain::{CommandError, ProjectCommand, TaskStatus};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use uuid::Uuid;

struct Fixture {
    project_id: Uuid,
    human_id: Uuid,
    ai_id: Uuid,
    task_id: Uuid,
}

/// customer → project → (human, ai_pm) principals → task in `backlog`, revision 1.
async fn seed(pool: &PgPool) -> Fixture {
    let customer_id: Uuid = sqlx::query_scalar("INSERT INTO core.customer (name) VALUES ('cust') RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap();
    let human_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'owner') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let ai_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('ai_pm', 'ai lead') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name) VALUES ($1, 'CMD-1', 'command channel') RETURNING id",
    )
    .bind(customer_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, description) \
         VALUES ($1, 'seeded task', 'created by test seed') RETURNING id",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    Fixture {
        project_id,
        human_id,
        ai_id,
        task_id,
    }
}

async fn seed_milestone(pool: &PgPool, project_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.milestone (project_id, name) VALUES ($1, 'm1') RETURNING id",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn executor(pool: &PgPool) -> PgCommandExecutor {
    PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()))
}

fn ctx(fixture: &Fixture, actor_kind: ActorKind, source: CommandSource) -> ExecutionContext {
    let actor_id = match actor_kind {
        ActorKind::AiPm => fixture.ai_id,
        ActorKind::Human | ActorKind::DevAgent | ActorKind::Service => fixture.human_id,
    };
    ExecutionContext {
        project_id: fixture.project_id,
        actor_id,
        actor_kind,
        source,
    }
}

async fn task_state(pool: &PgPool, task_id: Uuid) -> (String, i64) {
    let row = sqlx::query("SELECT status, revision FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap();
    (row.get("status"), row.get::<i64, _>("revision"))
}

async fn milestone_state(pool: &PgPool, milestone_id: Uuid) -> (String, i64) {
    let row = sqlx::query("SELECT status, revision FROM delivery.milestone WHERE id = $1")
        .bind(milestone_id)
        .fetch_one(pool)
        .await
        .unwrap();
    (row.get("status"), row.get::<i64, _>("revision"))
}

async fn audit_rows_for(pool: &PgPool, target_id: Uuid) -> Vec<(String, Option<Uuid>)> {
    sqlx::query("SELECT action, compensated_command_id FROM governance.audit_log WHERE target_id = $1 ORDER BY created_at")
        .bind(target_id)
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.get::<String, _>("action"), row.get::<Option<Uuid>, _>("compensated_command_id")))
        .collect()
}

/// 1) execute UpdateTaskStatus{expected_revision: 1, Done} → revision=2,
///    status=done; audit row carries before/after plus a Restore compensation
///    snapshot of the old status guarded at the resulting revision.
#[sqlx::test(migrations = "../../migrations")]
async fn update_task_status_executes_and_writes_audit_with_restore_compensation(pool: PgPool) {
    let f = seed(&pool).await;
    let ex = executor(&pool);

    let receipt = ex
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: f.task_id,
                expected_revision: 1,
                new_status: TaskStatus::Done,
            },
            ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
        )
        .await
        .expect("update_task_status must execute");
    assert_eq!(receipt.command_kind, "update_task_status");
    assert_eq!(receipt.target_id, f.task_id);
    assert_eq!(receipt.resulting_revision, 2);

    let (status, revision) = task_state(&pool, f.task_id).await;
    assert_eq!((status.as_str(), revision), ("done", 2));

    let row = sqlx::query(
        "SELECT action, target_type, target_id, before_data, after_data, compensation_command, compensated_command_id \
         FROM governance.audit_log WHERE id = $1",
    )
    .bind(receipt.audit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("action"), "update_task_status");
    assert_eq!(row.get::<String, _>("target_type"), "task");
    assert_eq!(row.get::<Uuid, _>("target_id"), f.task_id);
    assert!(row.get::<Option<Uuid>, _>("compensated_command_id").is_none());
    let before: Value = row.get("before_data");
    assert_eq!(before, json!({ "status": "backlog", "revision": 1 }));
    let after: Value = row.get("after_data");
    assert_eq!(after, json!({ "status": "done", "revision": 2 }));

    let compensation: Value = row.get("compensation_command");
    let spec: CompensationSpec = serde_json::from_value(compensation).expect("compensation spec roundtrips");
    match spec {
        CompensationSpec::Restore(entries) => {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].table, CompensableTable::Task);
            assert_eq!(entries[0].target_id, f.task_id);
            // old status, guarded at the executed command's resulting revision
            assert_eq!(entries[0].fields, json!({ "status": "backlog", "revision": 2 }));
        }
        other => panic!("expected Restore compensation, got {other:?}"),
    }
}

/// 2) stale expected_revision → RevisionConflict{expected, actual}; no state
///    change, no audit row.
#[sqlx::test(migrations = "../../migrations")]
async fn stale_expected_revision_conflicts_and_leaves_row_untouched(pool: PgPool) {
    let f = seed(&pool).await;
    let ex = executor(&pool);

    ex.execute(
        ProjectCommand::UpdateTaskStatus {
            task_id: f.task_id,
            expected_revision: 1,
            new_status: TaskStatus::Done,
        },
        ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
    )
    .await
    .expect("first execute succeeds");

    let err = ex
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: f.task_id,
                expected_revision: 1,
                new_status: TaskStatus::InProgress,
            },
            ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
        )
        .await
        .expect_err("stale revision must conflict");
    match err {
        CommandError::RevisionConflict { expected, actual } => assert_eq!((expected, actual), (1, 2)),
        other => panic!("expected RevisionConflict, got {other:?}"),
    }

    let (status, revision) = task_state(&pool, f.task_id).await;
    assert_eq!((status.as_str(), revision), ("done", 2), "conflicting command must not change state");
    let audits = audit_rows_for(&pool, f.task_id).await;
    assert_eq!(audits.len(), 1, "rejected command must not write an audit row");
}

/// 3) compensate(audit_id) → status back to backlog, revision=3, original audit
///    row's compensated_command_id points at the rollback audit row.
#[sqlx::test(migrations = "../../migrations")]
async fn compensate_restores_previous_status_and_marks_original_audit(pool: PgPool) {
    let f = seed(&pool).await;
    let ex = executor(&pool);

    let receipt = ex
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: f.task_id,
                expected_revision: 1,
                new_status: TaskStatus::Done,
            },
            ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
        )
        .await
        .expect("execute succeeds");

    let rollback = ex
        .compensate(receipt.audit_id, "wrong move", ctx(&f, ActorKind::Human, CommandSource::HumanIntervention {
            intervention_id: Uuid::new_v4(),
        }))
        .await
        .expect("compensation must succeed");
    assert_eq!(rollback.command_kind, "rollback_command");
    assert_eq!(rollback.target_id, f.task_id);
    assert_eq!(rollback.resulting_revision, 3);

    let (status, revision) = task_state(&pool, f.task_id).await;
    assert_eq!((status.as_str(), revision), ("backlog", 3));

    let compensated: Option<Uuid> = sqlx::query_scalar(
        "SELECT compensated_command_id FROM governance.audit_log WHERE id = $1",
    )
    .bind(receipt.audit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(compensated, Some(rollback.audit_id));

    let audits = audit_rows_for(&pool, f.task_id).await;
    assert_eq!(
        audits,
        vec![
            ("update_task_status".to_string(), Some(rollback.audit_id)),
            ("rollback_command".to_string(), None),
        ]
    );
}

/// 4) compensating the same audit row twice → AlreadyCompensated; state keeps
///    the restored values.
#[sqlx::test(migrations = "../../migrations")]
async fn compensating_same_audit_twice_fails_with_already_compensated(pool: PgPool) {
    let f = seed(&pool).await;
    let ex = executor(&pool);

    let receipt = ex
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: f.task_id,
                expected_revision: 1,
                new_status: TaskStatus::Done,
            },
            ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
        )
        .await
        .expect("execute succeeds");
    ex.compensate(receipt.audit_id, "first rollback", ctx(&f, ActorKind::Human, CommandSource::Bootstrap))
        .await
        .expect("first compensation succeeds");

    let err = ex
        .compensate(receipt.audit_id, "second rollback", ctx(&f, ActorKind::Human, CommandSource::Bootstrap))
        .await
        .expect_err("second compensation must fail");
    match err {
        CommandError::AlreadyCompensated { audit_id } => assert_eq!(audit_id, receipt.audit_id),
        other => panic!("expected AlreadyCompensated, got {other:?}"),
    }

    let (status, revision) = task_state(&pool, f.task_id).await;
    assert_eq!((status.as_str(), revision), ("backlog", 3), "double compensation must not re-bump");
}

/// 5) execute → execute a second legal change (revision moves on) → compensate
///    the first audit → RevisionConflict, the newer state stays untouched.
#[sqlx::test(migrations = "../../migrations")]
async fn test_compensate_after_further_change_conflicts(pool: PgPool) {
    let f = seed(&pool).await;
    let ex = executor(&pool);

    let first = ex
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: f.task_id,
                expected_revision: 1,
                new_status: TaskStatus::Done,
            },
            ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
        )
        .await
        .expect("first execute succeeds");
    ex.execute(
        ProjectCommand::UpdateTaskStatus {
            task_id: f.task_id,
            expected_revision: 2,
            new_status: TaskStatus::InProgress,
        },
        ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
    )
    .await
    .expect("second execute succeeds");

    let err = ex
        .compensate(first.audit_id, "late rollback", ctx(&f, ActorKind::Human, CommandSource::Bootstrap))
        .await
        .expect_err("stale compensation must conflict");
    match err {
        CommandError::RevisionConflict { expected, actual } => assert_eq!((expected, actual), (2, 3)),
        other => panic!("expected RevisionConflict, got {other:?}"),
    }

    let (status, revision) = task_state(&pool, f.task_id).await;
    assert_eq!((status.as_str(), revision), ("in_progress", 3), "newer state must not be overwritten");
    let audits = audit_rows_for(&pool, f.task_id).await;
    assert_eq!(audits.len(), 2, "failed compensation must not write a rollback audit row");
    assert!(audits.iter().all(|(_, compensated)| compensated.is_none()));
}

/// 6) ChangePolicy has no handler in M1 → NotHandled.
#[sqlx::test(migrations = "../../migrations")]
async fn change_policy_without_handler_fails_with_not_handled(pool: PgPool) {
    let f = seed(&pool).await;
    let ex = executor(&pool);

    assert!(!ex.has_handler("change_policy"));
    assert!(ex.has_handler("update_task_status"));

    let err = ex
        .execute(
            ProjectCommand::ChangePolicy {
                project_id: f.project_id,
                new_policy: json!({ "automation_mode": "controlled" }),
            },
            ctx(&f, ActorKind::Human, CommandSource::Bootstrap),
        )
        .await
        .expect_err("change_policy has no handler");
    match err {
        CommandError::NotHandled { command } => assert_eq!(command, "change_policy"),
        other => panic!("expected NotHandled, got {other:?}"),
    }
}

/// 7) red line: SignAcceptance is rejected for an AI actor and executes for a
///    human through the RedlineApproval source.
#[sqlx::test(migrations = "../../migrations")]
async fn test_redline_rejected_for_ai_actor(pool: PgPool) {
    let f = seed(&pool).await;
    let milestone_id = seed_milestone(&pool, f.project_id).await;
    let ex = executor(&pool);

    let err = ex
        .execute(
            ProjectCommand::SignAcceptance {
                project_id: f.project_id,
                milestone_id,
                expected_revision: 1,
            },
            ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
        )
        .await
        .expect_err("AI actor must not pass a red line");
    match err {
        CommandError::RedLineBlocked { command } => assert_eq!(command, "sign_acceptance"),
        other => panic!("expected RedLineBlocked, got {other:?}"),
    }
    let (status, revision) = milestone_state(&pool, milestone_id).await;
    assert_eq!((status.as_str(), revision), ("planned", 1), "blocked command must not change state");

    let receipt = ex
        .execute(
            ProjectCommand::SignAcceptance {
                project_id: f.project_id,
                milestone_id,
                expected_revision: 1,
            },
            ctx(
                &f,
                ActorKind::Human,
                CommandSource::RedlineApproval { approval_id: Uuid::new_v4() },
            ),
        )
        .await
        .expect("authorized human passes the red line");
    assert_eq!(receipt.command_kind, "sign_acceptance");
    assert_eq!(receipt.resulting_revision, 2);

    let (status, revision) = milestone_state(&pool, milestone_id).await;
    assert_eq!((status.as_str(), revision), ("accepted", 2));
}
