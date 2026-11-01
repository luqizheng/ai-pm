//! D2.1 真实 PostgreSQL 集成测试：四层策略合成落 Run 快照、内容寻址版本可复现。
//!
//! 纯合成规则的单测在 `pm-application/src/policy_composition.rs`；本文件只验证
//! 「读库 → 合成 → 写入 `policy_snapshot`」这条真实链路。

use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::run_dispatch::{ClaimKind, DispatchMode, RunDispatchConfig, claim_once};
use sqlx::PgPool;
use uuid::Uuid;

struct Project {
    project_id: Uuid,
    human_id: Uuid,
    agent_id: Uuid,
    repository_id: Uuid,
}

fn cfg() -> RunDispatchConfig {
    RunDispatchConfig {
        lease_secs: 300,
        backoff_cap_secs: 300,
        max_run_attempts: 3,
        max_infrastructure_attempts: 5,
        timeout_secs: 1800,
    }
}

async fn seed_project(pool: &PgPool, code: &str) -> Project {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
        },
    )
    .await
    .unwrap();
    let agent_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', 'dev') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let repository_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, status, head_commit_sha) \
         VALUES ($1, 'web', $2, 'verified', 'headsha') RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(format!("git@host:{code}-web"))
    .fetch_one(pool)
    .await
    .unwrap();
    Project {
        project_id: outcome.project_id,
        human_id: outcome.human_id,
        agent_id,
        repository_id,
    }
}

async fn add_ready_task(
    pool: &PgPool,
    project: &Project,
    title: &str,
    task_type: Option<&str>,
) -> (Uuid, Uuid) {
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, task_type) \
         VALUES ($1, $2, 'ready', 1, $3, $4) RETURNING id",
    )
    .bind(project.project_id)
    .bind(title.to_string())
    .bind(project.agent_id)
    .bind(task_type)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository \
         (task_id, project_id, repository_id, write_access, base_ref) \
         VALUES ($1, $2, $3, true, 'main')",
    )
    .bind(task_id)
    .bind(project.project_id)
    .bind(project.repository_id)
    .execute(pool)
    .await
    .unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status) \
         VALUES ($1, $2, 1, 'queued') RETURNING id",
    )
    .bind(project.project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (task_id, run_id)
}

#[allow(clippy::too_many_arguments)]
async fn upsert_scope(
    pool: &PgPool,
    scope: &str,
    key: &str,
    capabilities: (Option<bool>, Option<bool>, Option<bool>),
    require_human_approval: bool,
    checks: &[&str],
    actor: Uuid,
) {
    let (auto_accept, auto_merge, auto_publish_knowledge) = capabilities;
    sqlx::query(
        "INSERT INTO delivery.policy_scope \
         (scope, scope_key, require_human_approval, auto_accept, auto_merge, \
          auto_publish_knowledge, additional_checks, updated_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (scope, scope_key) DO UPDATE SET \
            require_human_approval = EXCLUDED.require_human_approval, \
            auto_accept = EXCLUDED.auto_accept, \
            auto_merge = EXCLUDED.auto_merge, \
            auto_publish_knowledge = EXCLUDED.auto_publish_knowledge, \
            additional_checks = EXCLUDED.additional_checks, \
            updated_at = now()",
    )
    .bind(scope)
    .bind(key)
    .bind(require_human_approval)
    .bind(auto_accept)
    .bind(auto_merge)
    .bind(auto_publish_knowledge)
    .bind(serde_json::json!(checks))
    .bind(actor)
    .execute(pool)
    .await
    .unwrap();
}

async fn snapshot(pool: &PgPool, run_id: Uuid) -> serde_json::Value {
    sqlx::query_scalar("SELECT policy_snapshot FROM delivery.task_run WHERE id = $1")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn four_layer_capability_composition_lands_in_snapshot(pool: PgPool) {
    let project = seed_project(&pool, "PC-L4").await;
    let (_task_id, _run_id) = add_ready_task(&pool, &project, "t", Some("refactor")).await;

    upsert_scope(
        &pool,
        "organization",
        "*",
        (Some(false), None, None),
        false,
        &["security_review"],
        project.human_id,
    )
    .await;
    upsert_scope(
        &pool,
        "project",
        &project.project_id.to_string(),
        (None, Some(false), None),
        false,
        &["architecture_review"],
        project.human_id,
    )
    .await;
    upsert_scope(
        &pool,
        "task_type",
        "refactor",
        (None, None, Some(false)),
        false,
        &[],
        project.human_id,
    )
    .await;
    // 任务级试图重新打开 auto_accept：低层不能放宽高层的关闭。
    sqlx::query(
        "INSERT INTO delivery.task_policy_override \
         (task_id, project_id, auto_accept, additional_checks, updated_by) \
         VALUES ($1, $2, true, '[]', $3)",
    )
    .bind(_task_id)
    .bind(project.project_id)
    .bind(project.human_id)
    .execute(&pool)
    .await
    .unwrap();

    let out = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(out.kind, ClaimKind::Claimed);

    let snapshot = snapshot(&pool, out.run_id).await;
    assert_eq!(snapshot["policy_schema"], serde_json::json!("composed-v1"));
    assert_eq!(snapshot["snapshot_scope"], serde_json::json!("composed-v1"));
    assert_eq!(
        snapshot["effective_policy"]["auto_accept"],
        serde_json::json!(false),
        "an organization-level false must not be reopened by the task override"
    );
    assert_eq!(
        snapshot["effective_policy"]["auto_merge"],
        serde_json::json!(false)
    );
    assert_eq!(
        snapshot["effective_policy"]["auto_publish_knowledge"],
        serde_json::json!(false)
    );
    assert_eq!(
        snapshot["effective_policy"]["require_human_approval"],
        serde_json::json!(false)
    );
    assert_eq!(
        snapshot["effective_policy"]["additional_checks"],
        serde_json::json!(["architecture_review", "security_review"]),
        "constraint-type fields accumulate as a deduplicated union"
    );
    assert_eq!(snapshot["execution_enabled"], serde_json::json!(true));
    assert_eq!(
        snapshot["policy_version"].as_str().unwrap().len(),
        12,
        "policy_version is derived from the content hash"
    );
    assert_eq!(
        snapshot["policy_hash"].as_str().unwrap().len(),
        64,
        "policy_hash is the full lowercase sha256 hex"
    );
    assert!(snapshot["captured_at"].is_string());
}

#[sqlx::test(migrations = "../../migrations")]
async fn policy_hash_is_content_addressed_across_runs(pool: PgPool) {
    let project = seed_project(&pool, "PC-HASH").await;
    upsert_scope(
        &pool,
        "organization",
        "*",
        (Some(true), Some(true), Some(true)),
        false,
        &["security_review"],
        project.human_id,
    )
    .await;
    add_ready_task(&pool, &project, "first", None).await;
    add_ready_task(&pool, &project, "second", None).await;

    let first = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    let second = claim_once(&pool, "w2", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.kind, ClaimKind::Claimed);
    assert_eq!(second.kind, ClaimKind::Claimed);
    assert_ne!(first.run_id, second.run_id);

    let first = snapshot(&pool, first.run_id).await;
    let second = snapshot(&pool, second.run_id).await;
    assert_eq!(
        first["policy_hash"], second["policy_hash"],
        "identical effective policy must hash identically even when captured at different times"
    );
    assert_eq!(first["policy_version"], second["policy_version"]);
}

#[sqlx::test(migrations = "../../migrations")]
async fn policy_hash_moves_when_policy_content_changes(pool: PgPool) {
    let project = seed_project(&pool, "PC-DIFF").await;
    upsert_scope(
        &pool,
        "organization",
        "*",
        (None, None, None),
        false,
        &["security_review"],
        project.human_id,
    )
    .await;
    add_ready_task(&pool, &project, "first", None).await;
    add_ready_task(&pool, &project, "second", None).await;

    let before = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    upsert_scope(
        &pool,
        "organization",
        "*",
        (None, None, None),
        false,
        &["security_review", "architecture_review"],
        project.human_id,
    )
    .await;
    let after = claim_once(&pool, "w2", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();

    let before = snapshot(&pool, before.run_id).await;
    let after = snapshot(&pool, after.run_id).await;
    assert_ne!(
        before["policy_hash"], after["policy_hash"],
        "changing any policy field must change the content-addressed hash"
    );
    assert_ne!(before["policy_version"], after["policy_version"]);
}