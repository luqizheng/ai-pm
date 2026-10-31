//! Slice D1 `run_dispatch` 真实数据库集成测试：并发领取、租约回收（B3-1/B3-2）、
//! 可领取性判定、多仓快照、审计、开关语义与工作区路径算法（R6）。

use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::run_dispatch::{
    accept_executor_result, claim_once, renew_lease, workspace_path_for, DispatchMode,
    RunDispatchConfig,
};
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    task_id: Uuid,
    repo_id: Uuid,
    run_id: Uuid,
    human_id: Uuid,
}

fn cfg() -> RunDispatchConfig {
    RunDispatchConfig {
        lease_secs: 300,
        backoff_cap_secs: 300,
        max_run_attempts: 3,
        max_infrastructure_attempts: 5,
    }
}

async fn seed(pool: &PgPool, code: &str, assigned: bool) -> Fx {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
        },
    )
    .await
    .unwrap();
    let agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', 'dev') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let assignee: Option<Uuid> = if assigned { Some(agent) } else { None };
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id) \
         VALUES ($1, 't', 'ready', 1, $2) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(assignee)
    .fetch_one(pool)
    .await
    .unwrap();
    let repo_id = add_repo(pool, outcome.project_id, code, "web", "verified").await;
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, base_ref) \
         VALUES ($1, $2, $3, true, 'main')",
    )
    .bind(task_id)
    .bind(outcome.project_id)
    .bind(repo_id)
    .execute(pool)
    .await
    .unwrap();
    // TaskReadyHandler 的常态产物：一条待调度的 queued Run。
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status) \
         VALUES ($1, $2, 1, 'queued') RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();
    Fx {
        project_id: outcome.project_id,
        task_id,
        repo_id,
        run_id,
        human_id: outcome.human_id,
    }
}

async fn add_repo(pool: &PgPool, project_id: Uuid, code: &str, name: &str, status: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, status, head_commit_sha) \
         VALUES ($1, $2, $3, $4, 'headsha') RETURNING id",
    )
    .bind(project_id)
    .bind(name.to_string())
    .bind(format!("git@host:{code}-{name}"))
    .bind(status.to_string())
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn run_status(pool: &PgPool, run_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM delivery.task_run WHERE id = $1")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn task_status(pool: &PgPool, task_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_claims_yield_exactly_one_claimed_run(pool: PgPool) {
    let fx = seed(&pool, "RD-C1", true).await;
    let cfg = cfg();
    let (a, b, c, d) = tokio::join!(
        claim_once(&pool, "w1", &cfg, DispatchMode::On),
        claim_once(&pool, "w2", &cfg, DispatchMode::On),
        claim_once(&pool, "w3", &cfg, DispatchMode::On),
        claim_once(&pool, "w4", &cfg, DispatchMode::On),
    );
    let claimed = [a, b, c, d]
        .into_iter()
        .filter(|r| matches!(r, Ok(Some(o)) if o.kind == pm_application::run_dispatch::ClaimKind::Claimed))
        .count();
    assert_eq!(claimed, 1, "exactly one worker may claim the run");
    let claimed_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run WHERE task_id = $1 AND status = 'claimed'",
    )
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(claimed_rows, 1);
    assert_eq!(task_status(&pool, fx.task_id).await, "claimed");
}

#[sqlx::test(migrations = "../../migrations")]
async fn second_claim_of_same_run_returns_none(pool: PgPool) {
    let _fx = seed(&pool, "RD-C2", true).await;
    let cfg = cfg();
    let first = claim_once(&pool, "w1", &cfg, DispatchMode::On).await.unwrap();
    assert!(first.is_some());
    let second = claim_once(&pool, "w1", &cfg, DispatchMode::On).await.unwrap();
    assert!(second.is_none(), "a claimed run with a live lease is not re-claimable");
}

#[sqlx::test(migrations = "../../migrations")]
async fn unstarted_expired_lease_recovers_same_run(pool: PgPool) {
    let fx = seed(&pool, "RD-C3", true).await;
    let cfg = cfg();
    let claimed = claim_once(&pool, "w1", &cfg, DispatchMode::On).await.unwrap().unwrap();
    // 执行器尚未启动（executor_session IS NULL），租约过期。
    sqlx::query("UPDATE delivery.task_run SET lease_expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(claimed.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let again = claim_once(&pool, "w2", &cfg, DispatchMode::On).await.unwrap().unwrap();
    assert_eq!(again.run_id, claimed.run_id, "B3-1: the same run must be recovered, not recreated");
    assert_eq!(run_status(&pool, claimed.run_id).await, "queued");
    assert_eq!(task_status(&pool, fx.task_id).await, "ready");
    let failures: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.task_run_failure WHERE run_id = $1")
        .bind(claimed.run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(failures, 0, "an unstarted lease expiry is not an execution failure");
}

#[sqlx::test(migrations = "../../migrations")]
async fn started_lost_run_is_isolated_and_superseded(pool: PgPool) {
    let fx = seed(&pool, "RD-C4", true).await;
    let cfg = cfg();
    let claimed = claim_once(&pool, "w1", &cfg, DispatchMode::On).await.unwrap().unwrap();
    // 执行器已启动（记录 session），随后租约过期。
    sqlx::query("UPDATE delivery.task_run SET executor_session = 'sess-1', lease_expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(claimed.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let reclaimed = claim_once(&pool, "w2", &cfg, DispatchMode::On).await.unwrap().unwrap();
    assert_eq!(reclaimed.kind, pm_application::run_dispatch::ClaimKind::Reclaimed);
    assert_ne!(reclaimed.run_id, claimed.run_id, "B3-2: a new run is created");
    assert_eq!(run_status(&pool, claimed.run_id).await, "timed_out");
    let terminated: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT terminated_at FROM delivery.task_run WHERE id = $1")
            .bind(claimed.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(terminated.is_some(), "the old instance termination must be recorded");
    let (attempt_no, supersedes): (i32, Option<Uuid>) =
        sqlx::query_as("SELECT attempt_no, supersedes_run_id FROM delivery.task_run WHERE id = $1")
            .bind(reclaimed.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attempt_no, 2);
    assert_eq!(supersedes, Some(claimed.run_id));
    let _ = fx;
}

#[sqlx::test(migrations = "../../migrations")]
async fn unassigned_task_is_skipped(pool: PgPool) {
    let fx = seed(&pool, "RD-C5", false).await;
    let cfg = cfg();
    let outcome = claim_once(&pool, "w1", &cfg, DispatchMode::On).await.unwrap().unwrap();
    assert_eq!(outcome.kind, pm_application::run_dispatch::ClaimKind::Skipped);
    assert_eq!(run_status(&pool, fx.run_id).await, "queued");
    let reason: Option<String> = sqlx::query_scalar("SELECT release_reason FROM delivery.task_run WHERE id = $1")
        .bind(fx.run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(reason.as_deref(), Some("no_executor_assignee"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn retry_exhausted_task_is_skipped(pool: PgPool) {
    let fx = seed(&pool, "RD-C6", true).await;
    sqlx::query("UPDATE delivery.task SET retry_exhausted = true WHERE id = $1")
        .bind(fx.task_id)
        .execute(&pool)
        .await
        .unwrap();

    let out = claim_once(&pool, "w1", &cfg(), DispatchMode::On).await.unwrap().unwrap();
    assert_eq!(out.kind, pm_application::run_dispatch::ClaimKind::Skipped);
    let reason: Option<String> = sqlx::query_scalar("SELECT release_reason FROM delivery.task_run WHERE id = $1")
        .bind(out.run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(reason.as_deref(), Some("retry_exhausted"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn unverified_repository_is_skipped_with_backoff(pool: PgPool) {
    let fx = seed(&pool, "RD-C7", true).await;
    sqlx::query("UPDATE integration.project_repository SET status = 'pending' WHERE id = $1")
        .bind(fx.repo_id)
        .execute(&pool)
        .await
        .unwrap();

    let out = claim_once(&pool, "w1", &cfg(), DispatchMode::On).await.unwrap().unwrap();
    assert_eq!(out.kind, pm_application::run_dispatch::ClaimKind::Skipped);
    let (reason, dispatch_attempts): (Option<String>, i32) =
        sqlx::query_as("SELECT release_reason, dispatch_attempts FROM delivery.task_run WHERE id = $1")
            .bind(out.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("repository_not_verified"));
    assert_eq!(dispatch_attempts, 1, "a denied claim must back off, not drop the run");
}

#[sqlx::test(migrations = "../../migrations")]
async fn multi_repository_snapshot_has_unique_paths(pool: PgPool) {
    let fx = seed(&pool, "RD-C8", true).await;
    for name in ["svc", "docs"] {
        let repo = add_repo(&pool, fx.project_id, "RD-C8", name, "verified").await;
        sqlx::query(
            "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, base_ref) \
             VALUES ($1, $2, $3, $4, 'main')",
        )
        .bind(fx.task_id)
        .bind(fx.project_id)
        .bind(repo)
        .bind(name != "docs")
        .execute(&pool)
        .await
        .unwrap();
    }
    let out = claim_once(&pool, "w1", &cfg(), DispatchMode::On).await.unwrap().unwrap();
    let rows: Vec<(Uuid, String, bool)> = sqlx::query_as(
        "SELECT repository_id, workspace_path, write_access FROM delivery.task_run_repository WHERE run_id = $1",
    )
    .bind(out.run_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3, "each declared repository lands one snapshot row");
    let mut paths: Vec<String> = rows.iter().map(|r| r.1.clone()).collect();
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), 3, "workspace paths must be unique");
    assert!(rows.iter().any(|r| !r.2), "the read-only repository is kept in the snapshot");
}

#[sqlx::test(migrations = "../../migrations")]
async fn claim_writes_exactly_one_audit_row(pool: PgPool) {
    let fx = seed(&pool, "RD-C9", true).await;
    let _ = claim_once(&pool, "w1", &cfg(), DispatchMode::On).await.unwrap().unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE target_type = 'task_run' AND action = 'claim_task_run'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
    let _ = fx;
}

#[sqlx::test(migrations = "../../migrations")]
async fn off_mode_does_not_claim_but_still_reclaims(pool: PgPool) {
    let fx = seed(&pool, "RD-C10", true).await;
    let cfg = cfg();
    // 先以 On 领取一条。
    let claimed = claim_once(&pool, "w1", &cfg, DispatchMode::On).await.unwrap().unwrap();
    // 再放一条 queued（另一任务），off 下不得领取。
    let task2: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id) \
         VALUES ($1, 't2', 'ready', 1, (SELECT assignee_id FROM delivery.task WHERE id = $2)) RETURNING id",
    )
    .bind(fx.project_id)
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let run2: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status) VALUES ($1, $2, 1, 'queued') RETURNING id",
    )
    .bind(fx.project_id)
    .bind(task2)
    .fetch_one(&pool)
    .await
    .unwrap();
    let none = claim_once(&pool, "w1", &cfg, DispatchMode::Off).await.unwrap();
    assert!(none.is_none(), "R4: off must not claim new runs");
    assert_eq!(run_status(&pool, run2).await, "queued");

    // R4：off 下回收仍执行——把已领取 Run 的租约置过期。
    sqlx::query("UPDATE delivery.task_run SET lease_expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(claimed.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let reclaimed = claim_once(&pool, "w1", &cfg, DispatchMode::Off).await.unwrap();
    assert!(reclaimed.is_some(), "off must still run expired-lease reclamation");
}

#[test]
fn workspace_path_is_stable_safe_and_id_keyed() {
    let id = Uuid::nil();
    let a = workspace_path_for(id, "Web App! ../etc");
    let b = workspace_path_for(id, "Web App! ../etc");
    assert_eq!(a, b, "R6: the path must be reproducible");
    assert!(a.starts_with("repos/web-app-etc-"), "unsafe chars are stripped: {a}");
    assert!(!a.contains("..") && !a.contains('/') || a.starts_with("repos/"), "no traversal: {a}");
    assert!(!a.contains("../"), "path traversal must be impossible");
    // 仓库改名（同 id）不改变映射。
    let renamed = workspace_path_for(id, "totally-different-name");
    assert_ne!(renamed, a, "name change yields a different safe-name segment");
    let same_id = workspace_path_for(id, "x");
    assert!(same_id.ends_with(&id.to_string()), "the id suffix keeps the mapping unique");
}
async fn grant_approval(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    actor: Uuid,
    status: &str,
) -> Uuid {
    let event_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO integration.inbound_event \
         (id, project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash) \
         VALUES ($1, $2, $3, 'test', 'bug_report', $4, '{}', 'h')",
    )
    .bind(event_id)
    .bind(project_id)
    .bind(actor)
    .bind(Uuid::new_v4().to_string())
    .execute(pool)
    .await
    .unwrap();
    let analysis_run_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO ai.analysis_run \
         (id, project_id, event_id, model_name, policy_version, config_snapshot) \
         VALUES ($1, $2, $3, 'm', 1, '{}')",
    )
    .bind(analysis_run_id)
    .bind(project_id)
    .bind(event_id)
    .execute(pool)
    .await
    .unwrap();
    let proposal_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO ai.change_proposal \
         (id, project_id, analysis_run_id, proposal_seq, target_type, target_id, command_type, \
          proposed_data, rationale, risk_level, status) \
         VALUES ($1, $2, $3, 0, 'task', $4, 'update_task_status', '{}', 'r', 'low', 'proposed')",
    )
    .bind(proposal_id)
    .bind(project_id)
    .bind(analysis_run_id)
    .bind(task_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO governance.approval_request \
         (id, project_id, proposal_id, status, required_role) \
         VALUES ($1, $2, $3, $4, 'project_owner')",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(proposal_id)
    .bind(status.to_string())
    .execute(pool)
    .await
    .unwrap();
    proposal_id
}

#[sqlx::test(migrations = "../../migrations")]
async fn lease_renewal_requires_the_current_owner(pool: PgPool) {
    let _fx = seed(&pool, "RD-T13", true).await;
    let out = claim_once(&pool, "owner", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert!(renew_lease(&pool, out.run_id, "owner", 300).await.unwrap());
    assert!(
        !renew_lease(&pool, out.run_id, "intruder", 300).await.unwrap(),
        "a non-owner must not renew the lease"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn outcome_stays_null_until_a_terminal_result(pool: PgPool) {
    let _fx = seed(&pool, "RD-T20", true).await;
    let cfg = cfg();
    let out = claim_once(&pool, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    let outcome: Option<String> =
        sqlx::query_scalar("SELECT outcome FROM delivery.task_run WHERE id = $1")
            .bind(out.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(outcome.is_none(), "S2: a claimed run must not carry an outcome");
    sqlx::query("UPDATE delivery.task_run SET lease_expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(out.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let again = claim_once(&pool, "w2", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again.run_id, out.run_id);
    let outcome2: Option<String> =
        sqlx::query_scalar("SELECT outcome FROM delivery.task_run WHERE id = $1")
            .bind(out.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        outcome2.is_none(),
        "an unstarted lease expiry must not be recorded as timed_out"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn policy_snapshot_records_the_minimal_fields(pool: PgPool) {
    let _fx = seed(&pool, "RD-T24", true).await;
    let out = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    let snap: serde_json::Value =
        sqlx::query_scalar("SELECT policy_snapshot FROM delivery.task_run WHERE id = $1")
            .bind(out.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(snap["snapshot_scope"], serde_json::json!("d1-minimal"));
    assert_eq!(snap["approval_required"], serde_json::json!(false));
    assert_eq!(snap["execution_enabled"], serde_json::json!(true));
    assert!(snap["captured_at"].is_string());
    assert!(snap["repository_baselines"].is_array());
    assert!(snap.get("policy_version").is_some());
    assert!(snap.get("approval_status").is_some());
}

#[sqlx::test(migrations = "../../migrations")]
async fn approval_gate_blocks_until_approved_evidence_exists(pool: PgPool) {
    let fx = seed(&pool, "RD-T21", true).await;
    let cfg = cfg();
    sqlx::query(
        "INSERT INTO delivery.task_policy_override \
         (task_id, project_id, require_human_approval, updated_by) VALUES ($1, $2, true, $3)",
    )
    .bind(fx.task_id)
    .bind(fx.project_id)
    .bind(fx.human_id)
    .execute(&pool)
    .await
    .unwrap();
    let out = claim_once(&pool, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(out.kind, pm_application::run_dispatch::ClaimKind::Skipped);
    let reason: Option<String> =
        sqlx::query_scalar("SELECT release_reason FROM delivery.task_run WHERE id = $1")
            .bind(out.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("policy_requires_approval"));

    let _approved = grant_approval(&pool, fx.project_id, fx.task_id, fx.human_id, "approved").await;
    // 首次 skip 已写入退避窗口：把下一次可领取时刻拨到当下再试。
    sqlx::query("UPDATE delivery.task_run SET next_dispatch_at = now() WHERE id = $1")
        .bind(out.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let out2 = claim_once(&pool, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        out2.kind,
        pm_application::run_dispatch::ClaimKind::Claimed,
        "approved evidence must unlock the claim"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn fencing_rejects_stale_executor_sessions(pool: PgPool) {
    let _fx = seed(&pool, "RD-T25", true).await;
    let out = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE delivery.task_run SET executor_session = 'sess-live' WHERE id = $1")
        .bind(out.run_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(accept_executor_result(&pool, out.run_id, "sess-live").await.unwrap());
    assert!(
        !accept_executor_result(&pool, out.run_id, "sess-stale").await.unwrap(),
        "a stale executor session must be fenced out"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn handler_and_claim_do_not_deadlock_or_duplicate(pool: PgPool) {
    use pm_infrastructure::outbox::ClaimedOutboxEvent;
    use pm_infrastructure::outbox_handlers::TaskReadyHandler;
    use pm_infrastructure::outbox_worker::OutboxHandler;

    let fx = seed(&pool, "RD-T17", true).await;
    let handler = TaskReadyHandler::new(pool.clone());
    let event = ClaimedOutboxEvent {
        id: Uuid::new_v4(),
        project_id: fx.project_id,
        event_type: "TaskReady".to_string(),
        aggregate_type: "Task".to_string(),
        aggregate_id: fx.task_id.to_string(),
        payload: serde_json::json!({ "task_id": fx.task_id }),
        attempts: 1,
        schema_version: 1,
        idempotency_key: "t17".to_string(),
    };
    let cfg = cfg();
    let (handled, _claimed) = tokio::join!(
        handler.handle(&event),
        claim_once(&pool, "w1", &cfg, DispatchMode::On),
    );
    handled.expect("B2: handler must not error (no deadlock, no unique violation)");
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run \
          WHERE task_id = $1 AND status IN ('queued', 'claimed', 'running')",
    )
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(active <= 1, "I1: at most one active run");
}
async fn insert_policy(pool: &PgPool, scope: &str, key: &str, required: bool, actor: Uuid) {
    sqlx::query(
        "INSERT INTO delivery.policy_scope (scope, scope_key, require_human_approval, updated_by) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (scope, scope_key) DO UPDATE \
            SET require_human_approval = EXCLUDED.require_human_approval, updated_at = now()",
    )
    .bind(scope)
    .bind(key)
    .bind(required)
    .bind(actor)
    .execute(pool)
    .await
    .unwrap();
}

async fn release_backoff(pool: &PgPool, run_id: Uuid) {
    sqlx::query("UPDATE delivery.task_run SET next_dispatch_at = now() WHERE id = $1")
        .bind(run_id)
        .execute(pool)
        .await
        .unwrap();
}

/// R5：项目级策略要求审批时，领取被拒；补足 approved 证据后放行。
#[sqlx::test(migrations = "../../migrations")]
async fn r5_project_level_requirement_blocks_until_approved(pool: PgPool) {
    let fx = seed(&pool, "RD-R5P", true).await;
    insert_policy(&pool, "project", &fx.project_id.to_string(), true, fx.human_id).await;
    let out = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(out.kind, pm_application::run_dispatch::ClaimKind::Skipped);
    let reason: Option<String> =
        sqlx::query_scalar("SELECT release_reason FROM delivery.task_run WHERE id = $1")
            .bind(out.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("policy_requires_approval"));

    let _ = grant_approval(&pool, fx.project_id, fx.task_id, fx.human_id, "approved").await;
    release_backoff(&pool, out.run_id).await;
    let out2 = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(out2.kind, pm_application::run_dispatch::ClaimKind::Claimed);
}

/// R5：低层（任务级 false）**不得**放宽高层（项目级 true）的强制审批。
#[sqlx::test(migrations = "../../migrations")]
async fn r5_lower_level_false_cannot_relax_a_higher_requirement(pool: PgPool) {
    let fx = seed(&pool, "RD-R5L", true).await;
    insert_policy(&pool, "project", &fx.project_id.to_string(), true, fx.human_id).await;
    sqlx::query(
        "INSERT INTO delivery.task_policy_override \
         (task_id, project_id, require_human_approval, updated_by) VALUES ($1, $2, false, $3)",
    )
    .bind(fx.task_id)
    .bind(fx.project_id)
    .bind(fx.human_id)
    .execute(&pool)
    .await
    .unwrap();
    let out = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        out.kind,
        pm_application::run_dispatch::ClaimKind::Skipped,
        "a task-level false must not relax a project-level requirement"
    );
}

/// R5：organization 与 task_type 层级同样参与合成。
#[sqlx::test(migrations = "../../migrations")]
async fn r5_organization_and_task_type_levels_are_synthesized(pool: PgPool) {
    let fx = seed(&pool, "RD-R5O", true).await;
    insert_policy(&pool, "organization", "*", true, fx.human_id).await;
    let out = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(out.kind, pm_application::run_dispatch::ClaimKind::Skipped, "org level gates");

    sqlx::query("DELETE FROM delivery.policy_scope WHERE scope = 'organization'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE delivery.task SET task_type = 'refactor' WHERE id = $1")
        .bind(fx.task_id)
        .execute(&pool)
        .await
        .unwrap();
    insert_policy(&pool, "task_type", "refactor", true, fx.human_id).await;
    release_backoff(&pool, out.run_id).await;
    let out2 = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        out2.kind,
        pm_application::run_dispatch::ClaimKind::Skipped,
        "task_type level gates"
    );
}

/// T19：B3 恢复二分——未启动保持同一 Run；已启动则新建 Run 并 supersede。
#[sqlx::test(migrations = "../../migrations")]
async fn t19_b3_recovery_split_unstarted_keeps_run_started_supersedes(pool: PgPool) {
    let fx = seed(&pool, "RD-T19", true).await;
    let cfg = cfg();
    let first = claim_once(&pool, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    // 未启动执行器：租约过期 → 同一 Run 恢复。
    sqlx::query("UPDATE delivery.task_run SET lease_expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(first.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let recovered = claim_once(&pool, "w2", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.run_id, first.run_id, "unstarted: the same run is recovered");

    // 已启动执行器：租约过期 → 旧 Run 隔离 + 新建 Run 并 supersede。
    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'claimed', executor_session = 'sess-old', lease_owner = 'w2', \
                lease_expires_at = now() - interval '1 second' \
          WHERE id = $1",
    )
    .bind(first.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let superseded = claim_once(&pool, "w3", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(superseded.run_id, first.run_id, "started: a new run supersedes");
    assert_eq!(run_status(&pool, first.run_id).await, "timed_out");
    let supersedes: Option<Uuid> =
        sqlx::query_scalar("SELECT supersedes_run_id FROM delivery.task_run WHERE id = $1")
            .bind(superseded.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(supersedes, Some(first.run_id));
    let _ = fx;
}

/// T26：重试耗尽后不再新建 Run，且记 `retry_exhausted`。
#[sqlx::test(migrations = "../../migrations")]
async fn t26_exhausted_retry_creates_no_new_run(pool: PgPool) {
    let fx = seed(&pool, "RD-T26", true).await;
    sqlx::query("UPDATE delivery.task SET retry_exhausted = true WHERE id = $1")
        .bind(fx.task_id)
        .execute(&pool)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.task_run WHERE task_id = $1")
        .bind(fx.task_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let out = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(out.kind, pm_application::run_dispatch::ClaimKind::Skipped);
    let reason: Option<String> =
        sqlx::query_scalar("SELECT release_reason FROM delivery.task_run WHERE id = $1")
            .bind(out.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("retry_exhausted"));
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.task_run WHERE task_id = $1")
        .bind(fx.task_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after, before, "an exhausted retry budget must not spawn a new run");
}