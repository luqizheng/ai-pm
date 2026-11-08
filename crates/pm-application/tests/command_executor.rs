//! Command channel core tests (Task 6, Step 1): execute + audit + compensation.
//!
//! Seeding uses raw SQL (Task 8's bootstrap service does not exist yet);
//! table shapes mirror `migrations/0001_core.sql` and `0002_delivery.sql`.

use std::sync::Arc;

use pm_application::command_executor::{
    ActorKind, CommandSource, CompensableTable, CompensationSpec, ExecutionContext,
    PgCommandExecutor,
};
use pm_application::PolicyEngine;
use pm_domain::SYSTEM_TENANT;
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
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('cust', $1) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
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
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, 'CMD-1', 'command channel', $2) RETURNING id",
    )
    .bind(customer_id)
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, description, tenant_id) \
         VALUES ($1, 'seeded task', 'created by test seed', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
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
        "INSERT INTO delivery.milestone (project_id, name, tenant_id) \
         VALUES ($1, 'm1', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
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
        actor: pm_application::actor::AuditActor::principal(actor_id),
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

async fn audit_source_of(pool: &PgPool, audit_id: Uuid) -> (String, Option<Uuid>) {
    let row = sqlx::query("SELECT source_kind, source_ref FROM governance.audit_log WHERE id = $1")
        .bind(audit_id)
        .fetch_one(pool)
        .await
        .unwrap();
    (row.get("source_kind"), row.get("source_ref"))
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
                new_status: TaskStatus::Ready,
            },
            ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
        )
        .await
        .expect("update_task_status must execute");
    assert_eq!(receipt.command_kind, "update_task_status");
    assert_eq!(receipt.target_id, f.task_id);
    assert_eq!(receipt.resulting_revision, 2);

    let (status, revision) = task_state(&pool, f.task_id).await;
    assert_eq!((status.as_str(), revision), ("ready", 2));

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
    assert!(row
        .get::<Option<Uuid>, _>("compensated_command_id")
        .is_none());
    let before: Value = row.get("before_data");
    assert_eq!(before, json!({ "status": "draft", "revision": 1 }));
    let after: Value = row.get("after_data");
    assert_eq!(after, json!({ "status": "ready", "revision": 2 }));

    let compensation: Value = row.get("compensation_command");
    let spec: CompensationSpec =
        serde_json::from_value(compensation).expect("compensation spec roundtrips");
    match spec {
        CompensationSpec::Restore(entries) => {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].table, CompensableTable::Task);
            assert_eq!(entries[0].target_id, f.task_id);
            // old status, guarded at the executed command's resulting revision
            assert_eq!(
                entries[0].fields,
                json!({ "status": "draft", "revision": 2 })
            );
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
            new_status: TaskStatus::Ready,
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
                new_status: TaskStatus::Ready,
            },
            ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
        )
        .await
        .expect_err("stale revision must conflict");
    match err {
        CommandError::RevisionConflict { expected, actual } => {
            assert_eq!((expected, actual), (1, 2))
        }
        other => panic!("expected RevisionConflict, got {other:?}"),
    }

    let (status, revision) = task_state(&pool, f.task_id).await;
    assert_eq!(
        (status.as_str(), revision),
        ("ready", 2),
        "conflicting command must not change state"
    );
    let audits = audit_rows_for(&pool, f.task_id).await;
    assert_eq!(
        audits.len(),
        1,
        "rejected command must not write an audit row"
    );
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
                new_status: TaskStatus::Ready,
            },
            ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
        )
        .await
        .expect("execute succeeds");

    let rollback = ex
        .compensate(
            receipt.audit_id,
            "wrong move",
            ctx(
                &f,
                ActorKind::Human,
                CommandSource::HumanIntervention {
                    intervention_id: Uuid::new_v4(),
                },
            ),
        )
        .await
        .expect("compensation must succeed");
    assert_eq!(rollback.command_kind, "rollback_command");
    assert_eq!(rollback.target_id, f.task_id);
    assert_eq!(rollback.resulting_revision, 3);

    let (status, revision) = task_state(&pool, f.task_id).await;
    assert_eq!((status.as_str(), revision), ("draft", 3));

    let compensated: Option<Uuid> =
        sqlx::query_scalar("SELECT compensated_command_id FROM governance.audit_log WHERE id = $1")
            .bind(receipt.audit_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(compensated, Some(rollback.audit_id));

    // Source identity: the executed row points at its channel, the rollback
    // row at the audit row it undoes.
    let executed = audit_source_of(&pool, receipt.audit_id).await;
    assert_eq!(executed, ("bootstrap".to_string(), None));
    let rolled_back = audit_source_of(&pool, rollback.audit_id).await;
    assert_eq!(
        rolled_back,
        ("rollback".to_string(), Some(receipt.audit_id))
    );

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
                new_status: TaskStatus::Ready,
            },
            ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
        )
        .await
        .expect("execute succeeds");
    ex.compensate(
        receipt.audit_id,
        "first rollback",
        ctx(&f, ActorKind::Human, CommandSource::Bootstrap),
    )
    .await
    .expect("first compensation succeeds");

    let err = ex
        .compensate(
            receipt.audit_id,
            "second rollback",
            ctx(&f, ActorKind::Human, CommandSource::Bootstrap),
        )
        .await
        .expect_err("second compensation must fail");
    match err {
        CommandError::AlreadyCompensated { audit_id } => assert_eq!(audit_id, receipt.audit_id),
        other => panic!("expected AlreadyCompensated, got {other:?}"),
    }

    let (status, revision) = task_state(&pool, f.task_id).await;
    assert_eq!(
        (status.as_str(), revision),
        ("draft", 3),
        "double compensation must not re-bump"
    );
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
                new_status: TaskStatus::Ready,
            },
            ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
        )
        .await
        .expect("first execute succeeds");
    ex.execute(
        ProjectCommand::UpdateTaskStatus {
            task_id: f.task_id,
            expected_revision: 2,
            new_status: TaskStatus::Claimed,
        },
        ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
    )
    .await
    .expect("second execute succeeds");

    let err = ex
        .compensate(
            first.audit_id,
            "late rollback",
            ctx(&f, ActorKind::Human, CommandSource::Bootstrap),
        )
        .await
        .expect_err("stale compensation must conflict");
    match err {
        CommandError::RevisionConflict { expected, actual } => {
            assert_eq!((expected, actual), (2, 3))
        }
        other => panic!("expected RevisionConflict, got {other:?}"),
    }

    let (status, revision) = task_state(&pool, f.task_id).await;
    assert_eq!(
        (status.as_str(), revision),
        ("claimed", 3),
        "newer state must not be overwritten"
    );
    let audits = audit_rows_for(&pool, f.task_id).await;
    assert_eq!(
        audits.len(),
        2,
        "failed compensation must not write a rollback audit row"
    );
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
    assert_eq!(
        (status.as_str(), revision),
        ("planned", 1),
        "blocked command must not change state"
    );

    let approval_id = Uuid::new_v4();
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
                CommandSource::RedlineApproval { approval_id },
            ),
        )
        .await
        .expect("authorized human passes the red line");
    assert_eq!(receipt.command_kind, "sign_acceptance");
    assert_eq!(receipt.resulting_revision, 2);

    // The approval flow (Task 15) joins approvals to their audit row through
    // the persisted source identity.
    let source = audit_source_of(&pool, receipt.audit_id).await;
    assert_eq!(source, ("redline_approval".to_string(), Some(approval_id)));

    let (status, revision) = milestone_state(&pool, milestone_id).await;
    assert_eq!((status.as_str(), revision), ("accepted", 2));
}

// ---------------------------------------------------------------------------
// Task 7: CreateTask / AdjustMilestonePlan / handler-registry invariant（approve
// 已随 S02-I6-B 退役为无条件拒绝，见 approve_command_is_refused_via_executor）and
// the handler-registry compensation invariant.
// ---------------------------------------------------------------------------

use chrono::{DateTime, Utc};

struct RequirementFixture {
    requirement_id: Uuid,
    /// Approved revision (version 1) the requirement currently points at.
    current_revision_id: Uuid,
    /// Draft revision (version 2) waiting for approval.
    draft_revision_id: Uuid,
}

/// A requirement with an approved current revision and a draft successor.
/// `code` must be unique per project (`UNIQUE (project_id, code)`).
async fn seed_requirement_with_current(
    pool: &PgPool,
    project_id: Uuid,
    author_id: Uuid,
    code: &str,
) -> RequirementFixture {
    let requirement_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement (project_id, code, title, tenant_id) \
         VALUES ($1, $2, 'seeded requirement', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap();
    let current_revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
         (requirement_id, project_id, version, content, state, content_hash, created_by, approved_by, tenant_id) \
         VALUES ($1, $2, 1, '{\"text\": \"v1\"}', 'approved', 'hash-1', $3, $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $2)) RETURNING id",
    )
    .bind(requirement_id)
    .bind(project_id)
    .bind(author_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.requirement SET current_revision_id = $2 WHERE id = $1")
        .bind(requirement_id)
        .bind(current_revision_id)
        .execute(pool)
        .await
        .unwrap();
    let draft_revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
         (requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, 2, '{\"text\": \"v2\"}', 'draft', 'hash-2', $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $2)) RETURNING id",
    )
    .bind(requirement_id)
    .bind(project_id)
    .bind(author_id)
    .fetch_one(pool)
    .await
    .unwrap();
    RequirementFixture {
        requirement_id,
        current_revision_id,
        draft_revision_id,
    }
}

/// A requirement with no approved revision yet (`current_revision_id IS NULL`)
/// and a single draft revision (version 1). Returns `(requirement_id, draft_id)`.
async fn seed_requirement_without_current(
    pool: &PgPool,
    project_id: Uuid,
    author_id: Uuid,
    code: &str,
) -> (Uuid, Uuid) {
    let requirement_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement (project_id, code, title, tenant_id) \
         VALUES ($1, $2, 'seeded requirement', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap();
    let draft_revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
         (requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, 1, '{\"text\": \"v1\"}', 'draft', 'hash-1', $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $2)) RETURNING id",
    )
    .bind(requirement_id)
    .bind(project_id)
    .bind(author_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (requirement_id, draft_revision_id)
}

async fn seed_milestone_with_plan(
    pool: &PgPool,
    project_id: Uuid,
    planned_at: DateTime<Utc>,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.milestone (project_id, name, planned_at, tenant_id) \
         VALUES ($1, 'planned milestone', $2, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(planned_at)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn revision_state(pool: &PgPool, revision_id: Uuid) -> String {
    sqlx::query_scalar("SELECT state FROM delivery.requirement_revision WHERE id = $1")
        .bind(revision_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn requirement_pointer(pool: &PgPool, requirement_id: Uuid) -> (Option<Uuid>, i64) {
    let row = sqlx::query(
        "SELECT current_revision_id, row_revision FROM delivery.requirement WHERE id = $1",
    )
    .bind(requirement_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (row.get("current_revision_id"), row.get("row_revision"))
}

/// The stored compensation spec of one audit row (`NULL` → `None`).
async fn compensation_of(pool: &PgPool, audit_id: Uuid) -> Option<Value> {
    sqlx::query_scalar("SELECT compensation_command FROM governance.audit_log WHERE id = $1")
        .bind(audit_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// 8) CreateTask inserts at revision 1, audits the new id and stores a Delete
///    compensation; compensating removes the created row.
#[sqlx::test(migrations = "../../migrations")]
async fn create_task_inserts_row_and_audits_delete_compensation(pool: PgPool) {
    let f = seed(&pool).await;
    let ex = executor(&pool);

    let receipt = ex
        .execute(
            ProjectCommand::CreateTask {
                project_id: f.project_id,
                title: "Write fallback plan".to_string(),
                description: "Draft the delivery fallback".to_string(),
                assignee_id: Some(f.human_id),
            },
            ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
        )
        .await
        .expect("create_task must execute");
    assert_eq!(receipt.command_kind, "create_task");
    assert_eq!(receipt.resulting_revision, 1);
    let task_id = receipt.target_id;
    assert_ne!(task_id, f.task_id, "a new row must be created");

    let row =
        sqlx::query("SELECT status, revision, title, assignee_id FROM delivery.task WHERE id = $1")
            .bind(task_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row.get::<String, _>("status"), "draft");
    assert_eq!(row.get::<i64, _>("revision"), 1);
    assert_eq!(row.get::<String, _>("title"), "Write fallback plan");
    assert_eq!(row.get::<Option<Uuid>, _>("assignee_id"), Some(f.human_id));

    let audit = sqlx::query(
        "SELECT action, target_type, target_id, after_data, compensation_command \
         FROM governance.audit_log WHERE id = $1",
    )
    .bind(receipt.audit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit.get::<String, _>("action"), "create_task");
    assert_eq!(audit.get::<String, _>("target_type"), "task");
    assert_eq!(audit.get::<Uuid, _>("target_id"), task_id);
    let after: Value = audit.get("after_data");
    assert_eq!(
        after["id"],
        json!(task_id),
        "audit data must record the new id"
    );
    let spec: CompensationSpec =
        serde_json::from_value(audit.get::<Value, _>("compensation_command"))
            .expect("compensation spec roundtrips");
    match spec {
        CompensationSpec::Delete { table, target_id } => {
            assert_eq!((table, target_id), (CompensableTable::Task, task_id));
        }
        other => panic!("expected Delete compensation, got {other:?}"),
    }

    let rollback = ex
        .compensate(
            receipt.audit_id,
            "undo create",
            ctx(
                &f,
                ActorKind::Human,
                CommandSource::HumanIntervention {
                    intervention_id: Uuid::new_v4(),
                },
            ),
        )
        .await
        .expect("compensation must delete the created task");
    let remaining: Option<Uuid> = sqlx::query_scalar("SELECT id FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
    assert!(
        remaining.is_none(),
        "compensated create_task must remove the row"
    );
    let compensated: Option<Uuid> =
        sqlx::query_scalar("SELECT compensated_command_id FROM governance.audit_log WHERE id = $1")
            .bind(receipt.audit_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(compensated, Some(rollback.audit_id));
}

/// 9) ApproveRequirementRevision 在执行器面被双层拒绝（S02-I6-B，S02D-02
///    #2/#3 + S02D-21）：AI 主体先被红线拦下（RedLineBlocked，V-2 回归）；
///    Human + RedlineApproval 抵达 handler 后仍被无条件拒绝——正式确认唯一
///    合法路径 = REST 统一服务。拒绝不改状态、不审计。
#[sqlx::test(migrations = "../../migrations")]
async fn approve_command_is_refused_via_executor(pool: PgPool) {
    let f = seed(&pool).await;
    let r = seed_requirement_with_current(&pool, f.project_id, f.human_id, "REQ-1").await;
    let ex = executor(&pool);

    // 层 1（纵深防御）：红线补录 → AI 永远 RedLineBlocked（V-2 场景重放）。
    let err = ex
        .execute(
            ProjectCommand::ApproveRequirementRevision {
                project_id: f.project_id,
                requirement_id: r.requirement_id,
                revision_id: r.draft_revision_id,
            },
            ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap),
        )
        .await
        .expect_err("AI must never execute a confirmation");
    match err {
        CommandError::RedLineBlocked { command } => {
            assert_eq!(command, "approve_requirement_revision")
        }
        other => panic!("expected RedLineBlocked, got {other:?}"),
    }

    // 层 2（唯一路径裁决）：人工经 RedlineApproval 抵达执行器 → 仍拒绝。
    let err = ex
        .execute(
            ProjectCommand::ApproveRequirementRevision {
                project_id: f.project_id,
                requirement_id: r.requirement_id,
                revision_id: r.draft_revision_id,
            },
            ctx(
                &f,
                ActorKind::Human,
                CommandSource::RedlineApproval {
                    approval_id: Uuid::new_v4(),
                },
            ),
        )
        .await
        .expect_err("executor must refuse confirmations — REST is the only path");
    match err {
        CommandError::Validation(message) => {
            assert!(
                message.contains("confirmation is refused"),
                "got: {message}"
            );
        }
        other => panic!("expected Validation, got {other:?}"),
    }

    // 状态与审计双不动（DEF-02 V-2 / DEF-04 V-6 回归面）。
    let (pointer, _row_revision) = requirement_pointer(&pool, r.requirement_id).await;
    assert_eq!(
        pointer,
        Some(r.current_revision_id),
        "refusal must not move the pointer"
    );
    assert_eq!(revision_state(&pool, r.draft_revision_id).await, "draft");
    assert!(
        audit_rows_for(&pool, r.requirement_id).await.is_empty(),
        "refusal must not audit"
    );
}

/// 13) Registry invariant: every command that has a handler and is not a red
///     line must declare a compensation command in its audit row. Two kinds are
///     sanctioned exceptions by design — they do NOT violate the invariant,
///     which only constrains non-red-line commands:
///     - `sign_acceptance` is a red line whose domain reversibility is
///       Irreversible; executed here under Human + RedlineApproval its audit
///       row must carry NO compensation command.
///     - `rollback_command` is itself the compensation mechanism: its execution
///       path is `compensate()` (covered by the compensation tests), so only
///       its handler-registry entry is asserted here.
#[sqlx::test(migrations = "../../migrations")]
async fn test_every_auto_command_declares_compensation(pool: PgPool) {
    let f = seed(&pool).await;
    let adjust_milestone_id = seed_milestone(&pool, f.project_id).await;
    let sign_milestone_id = seed_milestone(&pool, f.project_id).await;
    let r = seed_requirement_with_current(&pool, f.project_id, f.human_id, "REQ-1").await;
    let ex = executor(&pool);
    let policy = PolicyEngine::for_tests();

    // The seven ProjectCommand variants (pm-domain command.rs) — keep in sync.
    let kinds = [
        "create_task",
        "update_task_status",
        "approve_requirement_revision",
        "adjust_milestone_plan",
        "sign_acceptance",
        "change_policy",
        "rollback_command",
    ];

    for kind in kinds {
        match kind {
            "create_task" => {
                let cmd = ProjectCommand::CreateTask {
                    project_id: f.project_id,
                    title: "invariant task".to_string(),
                    description: "seeded by the registry invariant test".to_string(),
                    assignee_id: None,
                };
                assert!(!policy.is_redline(&cmd));
                let receipt = ex
                    .execute(cmd, ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap))
                    .await
                    .expect("create_task executes");
                let spec: CompensationSpec = serde_json::from_value(
                    compensation_of(&pool, receipt.audit_id)
                        .await
                        .unwrap_or_else(|| {
                            panic!("{kind}: non-redline command must declare a compensation")
                        }),
                )
                .expect("compensation spec roundtrips");
                assert!(matches!(spec, CompensationSpec::Delete { .. }));
            }
            "update_task_status" => {
                let cmd = ProjectCommand::UpdateTaskStatus {
                    task_id: f.task_id,
                    expected_revision: 1,
                    new_status: TaskStatus::Ready,
                };
                assert!(!policy.is_redline(&cmd));
                let receipt = ex
                    .execute(cmd, ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap))
                    .await
                    .expect("update_task_status executes");
                assert!(
                    compensation_of(&pool, receipt.audit_id).await.is_some(),
                    "{kind}: non-redline command must declare a compensation"
                );
            }
            "approve_requirement_revision" => {
                // S02-I6-B（S02D-02 #3）：approve 类入红线——AI 永远 RedLineBlocked，
                // 正式确认唯一路径 = REST 统一服务；此处验证纵深防御层。
                let cmd = ProjectCommand::ApproveRequirementRevision {
                    project_id: f.project_id,
                    requirement_id: r.requirement_id,
                    revision_id: r.draft_revision_id,
                };
                assert!(
                    policy.is_redline(&cmd),
                    "approve must be red-lined (S02D-02 #3)"
                );
                let err = ex
                    .execute(cmd, ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap))
                    .await
                    .expect_err("approve must never execute via the executor");
                match err {
                    CommandError::RedLineBlocked { command } => {
                        assert_eq!(command, "approve_requirement_revision")
                    }
                    other => panic!("expected RedLineBlocked, got {other:?}"),
                }
            }
            "adjust_milestone_plan" => {
                let planned: DateTime<Utc> = "2026-08-01T00:00:00Z".parse().unwrap();
                let cmd = ProjectCommand::AdjustMilestonePlan {
                    project_id: f.project_id,
                    milestone_id: adjust_milestone_id,
                    new_planned_at: Some(planned),
                    expected_revision: 1,
                };
                assert!(!policy.is_redline(&cmd));
                let receipt = ex
                    .execute(cmd, ctx(&f, ActorKind::AiPm, CommandSource::Bootstrap))
                    .await
                    .expect("adjust_milestone_plan executes");
                let spec: CompensationSpec = serde_json::from_value(
                    compensation_of(&pool, receipt.audit_id)
                        .await
                        .unwrap_or_else(|| {
                            panic!("{kind}: non-redline command must declare a compensation")
                        }),
                )
                .expect("compensation spec roundtrips");
                assert!(matches!(spec, CompensationSpec::Restore(_)));
            }
            "sign_acceptance" => {
                // Sanctioned exception: red line + Irreversible by design. The
                // invariant constrains only non-red-line commands; here we
                // assert the audit row carries NO compensation command.
                let cmd = ProjectCommand::SignAcceptance {
                    project_id: f.project_id,
                    milestone_id: sign_milestone_id,
                    expected_revision: 1,
                };
                assert!(
                    policy.is_redline(&cmd),
                    "sign_acceptance must stay a red line"
                );
                let receipt = ex
                    .execute(
                        cmd,
                        ctx(
                            &f,
                            ActorKind::Human,
                            CommandSource::RedlineApproval {
                                approval_id: Uuid::new_v4(),
                            },
                        ),
                    )
                    .await
                    .expect("human through RedlineApproval executes the red line");
                assert_eq!(
                    compensation_of(&pool, receipt.audit_id).await,
                    None,
                    "sign_acceptance is Irreversible by design: no compensation command"
                );
            }
            "change_policy" => {
                let cmd = ProjectCommand::ChangePolicy {
                    project_id: f.project_id,
                    new_policy: json!({ "automation_mode": "controlled" }),
                };
                assert!(!ex.has_handler(kind), "change_policy has no M1 handler");
                let err = ex
                    .execute(cmd, ctx(&f, ActorKind::Human, CommandSource::Bootstrap))
                    .await
                    .expect_err("change_policy is not handled in M1");
                match err {
                    CommandError::NotHandled { command } => assert_eq!(command, "change_policy"),
                    other => panic!("expected NotHandled, got {other:?}"),
                }
            }
            "rollback_command" => {
                // Sanctioned exception: rollback_command is itself the
                // compensation mechanism; its execution path is `compensate()`
                // (covered by the compensation tests above), not `execute()`.
                // Only its handler-registry entry is asserted here.
                assert!(
                    ex.has_handler(kind),
                    "rollback_command's handler is compensate() and must be registered"
                );
            }
            other => panic!("invariant test does not cover command kind {other}"),
        }
    }
}
