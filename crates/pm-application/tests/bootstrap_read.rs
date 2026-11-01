//! Project bootstrap and project-scoped read service tests (Task 8, Step 1).
//!
//! Bootstrap semantics under test: idempotency guard on `core.project.code`
//! (second call with the same code → `BootstrapError::Duplicate`, no second
//! project, no upsert), the seeded principal/member/AI-config/skill-binding
//! rows, and the cross-project invisibility rule of `task_context`
//! (`task.project_id != project_id` → `ReadError::NotFound`).

use pm_application::bootstrap::{
    BootstrapError, BootstrapRequest, bootstrap_project, resolve_assignee,
};
use pm_application::read::{ReadError, task_context};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

fn req(code: &str, name: &str) -> BootstrapRequest {
    BootstrapRequest {
        project_code: code.to_string(),
        project_name: name.to_string(),
    }
}

/// Step 1 test 1: bootstrap is idempotent by project code — a second call with
/// the same code fails with `Duplicate` and leaves exactly one project, the
/// first one (the name must still be the first call's name: no upsert).
#[sqlx::test(migrations = "../../migrations")]
async fn bootstrap_rejects_duplicate_project_code_without_second_project(pool: PgPool) {
    let first = bootstrap_project(&pool, req("TP-1", "first name")).await.unwrap();

    let err = bootstrap_project(&pool, req("TP-1", "second name")).await.unwrap_err();
    match err {
        BootstrapError::Duplicate(code) => assert_eq!(code, "TP-1"),
        other => panic!("expected BootstrapError::Duplicate, got {other:?}"),
    }

    // Exactly one project with this code and it is the first one: the second
    // call must not have created a second row nor upserted the first.
    let row = sqlx::query(
        "SELECT count(*)::int8 AS n, max(name) AS name, max(status) AS status \
         FROM core.project WHERE code = 'TP-1'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<i64, _>("n"), 1, "second bootstrap created a second project");
    assert_eq!(row.get::<String, _>("name"), "first name", "second bootstrap upserted the project");
    assert_eq!(row.get::<String, _>("status"), "planning");

    // The failed call must not leave extra customers either (name == code).
    let customers: i64 =
        sqlx::query_scalar("SELECT count(*) FROM core.customer WHERE name = 'TP-1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(customers, 1);

    // The outcome identifies the surviving project.
    let project_id: Uuid = sqlx::query_scalar("SELECT id FROM core.project WHERE code = 'TP-1'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(first.project_id, project_id);
}

/// Step 1 test 2: one bootstrap call seeds 5 principals
/// (human/ai_pm/architect/developer/tester, code-suffixed display names) and 5
/// project_member rows, the AI config (fake model, ai_lead, policy_version 1)
/// and 5 skill bindings pinned to the 0003 seed at 'v1'.
#[sqlx::test(migrations = "../../migrations")]
async fn bootstrap_seeds_principals_members_config_and_bindings(pool: PgPool) {
    let outcome = bootstrap_project(&pool, req("TP-2", "seeded project")).await.unwrap();

    // The five principal ids are distinct and resolve to the right kinds and
    // display names (base name + code suffix).
    assert_ne!(outcome.human_id, outcome.ai_principal_id);
    assert_ne!(outcome.ai_principal_id, outcome.dev_agent_id);
    assert_ne!(outcome.human_id, outcome.dev_agent_id);
    assert_ne!(outcome.architect_id, outcome.developer_id);
    assert_ne!(outcome.architect_id, outcome.tester_id);
    assert_ne!(outcome.developer_id, outcome.tester_id);
    for (id, kind, prefix) in [
        (outcome.human_id, "human", "owner-"),
        (outcome.ai_principal_id, "ai_pm", "ai-pm-"),
        (outcome.architect_id, "dev_agent", "architect-"),
        (outcome.developer_id, "dev_agent", "dev-agent-"),
        (outcome.tester_id, "dev_agent", "tester-"),
    ] {
        let row = sqlx::query("SELECT kind, display_name FROM core.principal WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|_| panic!("principal {id} missing"));
        assert_eq!(row.get::<String, _>("kind"), kind, "wrong kind for {id}");
        let display = row.get::<String, _>("display_name");
        assert!(
            display.starts_with(prefix) && display.ends_with("TP-2"),
            "display_name `{display}` must carry the base name and the code suffix"
        );
    }
    // 0015 幂等种入的平台 service principal（ai-pm-dispatch）不属于 bootstrap
    // 产物，统计时排除它。
    let principal_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM core.principal WHERE kind <> 'service'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(principal_count, 5, "bootstrap must create exactly 5 principals");

    // Project members: each principal has exactly the expected role.
    let roles: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT principal_id, role FROM core.project_member WHERE project_id = $1 ORDER BY role",
    )
    .bind(outcome.project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        roles,
        vec![
            (outcome.ai_principal_id, "ai_pm".to_string()),
            (outcome.architect_id, "architect".to_string()),
            (outcome.developer_id, "developer".to_string()),
            (outcome.human_id, "owner".to_string()),
            (outcome.tester_id, "tester".to_string()),
        ]
    );

    // AI config row: fake model, ai_lead, policy_version 1, bound to the ai_pm
    // principal.
    let row = sqlx::query(
        "SELECT ai_principal_id, model_provider, model_name, model_config, automation_mode, \
         policy_version FROM ai.project_ai_config WHERE project_id = $1",
    )
    .bind(outcome.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<Uuid, _>("ai_principal_id"), outcome.ai_principal_id);
    assert_eq!(row.get::<String, _>("model_provider"), "fake");
    assert_eq!(row.get::<String, _>("model_name"), "fake");
    assert_eq!(row.get::<Value, _>("model_config"), json!({}));
    assert_eq!(row.get::<String, _>("automation_mode"), "ai_lead");
    assert_eq!(row.get::<i32, _>("policy_version"), 1);

    // Skill bindings: 5 rows, one per 0003-seeded skill, all at 'v1', enabled.
    let bindings: Vec<(String, String, bool)> = sqlx::query_as(
        "SELECT skill_id, skill_version, enabled FROM ai.project_skill_binding \
         WHERE project_id = $1 ORDER BY skill_id",
    )
    .bind(outcome.project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        bindings,
        vec![
            ("architecture_consistency".to_string(), "v1".to_string(), true),
            ("requirement_analysis".to_string(), "v1".to_string(), true),
            ("risk_detection".to_string(), "v1".to_string(), true),
            ("scheduling_orchestration".to_string(), "v1".to_string(), true),
            ("task_analysis".to_string(), "v1".to_string(), true),
        ]
    );
}

/// Step 1 test 3: cross-project invisibility — `task_context` with project A's
/// id over a task of project B must return `ReadError::NotFound`.
#[sqlx::test(migrations = "../../migrations")]
async fn task_context_hides_tasks_from_other_projects(pool: PgPool) {
    let a = bootstrap_project(&pool, req("TP-A", "project a")).await.unwrap();
    let b = bootstrap_project(&pool, req("TP-B", "project b")).await.unwrap();

    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title) VALUES ($1, 'b task') RETURNING id",
    )
    .bind(b.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    match task_context(&pool, a.project_id, task_id).await {
        Err(ReadError::NotFound) => {}
        Err(other) => panic!("expected ReadError::NotFound, got {other:?}"),
        Ok(row) => panic!("cross-project task must not be visible, got {row:?}"),
    }
}

/// Companion happy path for the read service: requirement content via
/// `delivery.task_requirement` (latest linked revision wins), milestone row as
/// JSONB, and a bare task with neither → both `None`.
#[sqlx::test(migrations = "../../migrations")]
async fn task_context_returns_requirement_and_milestone_json(pool: PgPool) {
    let outcome = bootstrap_project(&pool, req("TP-C", "project c")).await.unwrap();
    let project_id = outcome.project_id;

    let milestone_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.milestone (project_id, name) VALUES ($1, 'M1') RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let requirement_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement (project_id, code, title) \
         VALUES ($1, 'REQ-1', 'login') RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    // Two revisions; the latest linked one must win.
    let v1_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
         (requirement_id, project_id, version, content, state, content_hash, created_by) \
         VALUES ($1, $2, 1, '{\"goal\":\"login v1\"}'::jsonb, 'superseded', 'hash1', $3) RETURNING id",
    )
    .bind(requirement_id)
    .bind(project_id)
    .bind(outcome.human_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let v2_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
         (requirement_id, project_id, version, content, state, content_hash, created_by) \
         VALUES ($1, $2, 2, '{\"goal\":\"login v2\"}'::jsonb, 'approved', 'hash2', $3) RETURNING id",
    )
    .bind(requirement_id)
    .bind(project_id)
    .bind(outcome.human_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, milestone_id, title) \
         VALUES ($1, $2, 'implement login') RETURNING id",
    )
    .bind(project_id)
    .bind(milestone_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    for revision_id in [v1_id, v2_id] {
        sqlx::query(
            "INSERT INTO delivery.task_requirement (project_id, task_id, requirement_id, revision_id) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(project_id)
        .bind(task_id)
        .bind(requirement_id)
        .bind(revision_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let row = task_context(&pool, project_id, task_id).await.unwrap();
    assert_eq!(row.task_id, task_id);
    assert_eq!(row.title, "implement login");
    assert_eq!(row.status, "draft");
    assert_eq!(row.revision, 1);
    assert_eq!(row.requirement, Some(json!({"goal": "login v2"})), "latest linked revision must win");
    let milestone = row.milestone.expect("milestone must be present");
    assert_eq!(milestone["name"], json!("M1"));
    assert_eq!(milestone["status"], json!("planned"));

    // A task without requirement links or milestone → both None.
    let bare_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title) VALUES ($1, 'bare') RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let bare = task_context(&pool, project_id, bare_id).await.unwrap();
    assert_eq!(bare.requirement, None);
    assert_eq!(bare.milestone, None);
}

#[sqlx::test(migrations = "../../migrations")]
async fn resolve_assignee_maps_agent_roles(pool: PgPool) {
    let outcome = bootstrap_project(&pool, req("TP-3", "roles")).await.unwrap();
    assert_eq!(
        resolve_assignee(&pool, outcome.project_id, "developer").await.unwrap(),
        Some(outcome.developer_id)
    );
    assert_eq!(
        resolve_assignee(&pool, outcome.project_id, "architect").await.unwrap(),
        Some(outcome.architect_id)
    );
    assert_eq!(
        resolve_assignee(&pool, outcome.project_id, "tester").await.unwrap(),
        Some(outcome.tester_id)
    );
    assert_eq!(
        resolve_assignee(&pool, outcome.project_id, "unknown").await.unwrap(),
        None
    );
}
