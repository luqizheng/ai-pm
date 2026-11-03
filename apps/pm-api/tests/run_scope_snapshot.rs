//! S01 · I3-C/D 验收：Run 的每仓能力快照（仓库策略 ∩ Task 范围 ∩ 仓库状态 →
//! 依赖闭包），内容寻址哈希落 `task_run.policy_snapshot_hash`（B-6 方案 A）。

use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::git_capability::set_grant;
use pm_application::git_capability::{
    RepoCapSnapshot, build_task_repo_caps_tx, caps_snapshot_hash, caps_snapshot_json,
};
use pm_application::repository::{CreateRepositoryRequest, create_repository};
use pm_application::run_dispatch::{
    DispatchMode, RunDispatchConfig, claim_once,
};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use serde_json::json;
use uuid::Uuid;

async fn seed(pool: &PgPool, code: &str) -> (Uuid, Uuid, Uuid, Uuid) {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let repo = create_repository(
        pool,
        boot.project_id,
        &pm_application::actor::AuditActor::principal(boot.human_id),
        CreateRepositoryRequest {
            name: "主仓库".to_string(),
            url: "git@example.invalid:team/demo.git".to_string(),
            role: Some("main".to_string()),
            default_branch: Some("main".to_string()),
            description: None,
        },
    )
    .await
    .unwrap();
    // 可调度的 queued Run。
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
         VALUES ($1, 'snap', 'ready', (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(boot.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    // 判定 4：assignee 必须是 dev_agent 才可领取。
    let developer: Uuid = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member \
          WHERE project_id = $1 AND role = 'developer' LIMIT 1",
    )
    .bind(boot.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task SET assignee_id = $2 WHERE id = $1")
        .bind(task_id)
        .bind(developer)
        .execute(pool)
        .await
        .unwrap();
    // 可领取前提：仓库 verified + 基线可解析。
    sqlx::query(
        "UPDATE integration.project_repository          SET status = 'verified', head_commit_sha = 'base0' WHERE id = $1",
    )
    .bind(repo.id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, tenant_id) \
         VALUES ($1, $2, $3, true, (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(boot.project_id)
    .bind(repo.id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, run_kind, status, tenant_id) \
         VALUES ($1, $2, 1, 'delivery', 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(boot.project_id)
    .bind(task_id)
    .execute(pool)
    .await
    .unwrap();
    (boot.project_id, task_id, repo.id, boot.human_id)
}

fn config() -> RunDispatchConfig {
    RunDispatchConfig::from_env()
}

/// I3-C/D：有全链授权 → 快照含全部有效能力；哈希 = 快照 JSON 的 SHA-256；
/// `policy_snapshot_hash` 落库。
#[sqlx::test(migrations = "../../migrations")]
async fn claim_writes_capability_snapshot_and_hash(pool: PgPool) {
    let (project_id, task_id, repo_id, owner) = seed(&pool, "SNAP-1").await;
    // 授予 dev_agent（项目执行主体）全链。
    let dev_agent: Uuid = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member \
          WHERE project_id = $1 AND role = 'developer' LIMIT 1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    set_grant(
        &pool,
        &pm_application::actor::AuditActor::principal(owner),
        SYSTEM_TENANT,
        project_id,
        repo_id,
        dev_agent,
        pm_domain::GitCapability::ALL.to_vec(),
    )
    .await
    .unwrap();

    let cfg = config();
    let claimed = claim_once(&pool, "snap-worker", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .expect("queued run must be claimable");
    assert_eq!(claimed.kind, pm_application::run_dispatch::ClaimKind::Claimed, "must truly claim");

    let (hash, snaps): (Option<String>, Option<serde_json::Value>) = sqlx::query_as(
        "SELECT policy_snapshot_hash, repository_policy_snapshots \
         FROM delivery.task_run WHERE id = $1",
    )
    .bind(claimed.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let snaps = snaps.expect("per-repo snapshot must be stored");
    let entries = snaps.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["repository_id"], json!(repo_id.to_string()));
    let caps = entries[0]["effective"].as_array().unwrap();
    assert_eq!(caps.len(), 6, "full chain grants yield all six capabilities");

    // 哈希与快照内容一致（SHA-256 内容寻址）。
    #[derive(serde::Deserialize)]
    struct SnapMirror { repository_id: Uuid, effective: Vec<String> }
    let expected: Vec<SnapMirror> = serde_json::from_value(snaps.clone()).unwrap();
    let mirror: Vec<RepoCapSnapshot> = expected
        .into_iter()
        .map(|m| RepoCapSnapshot { repository_id: m.repository_id, effective: m.effective })
        .collect();
    assert_eq!(hash.as_deref(), Some(caps_snapshot_hash(&mirror).as_str()));
    assert_eq!(caps_snapshot_json(&mirror), snaps);
    assert_eq!(hash.as_ref().map(|h| h.len()), Some(64), "sha256 hex");
}

/// I3-C：Task 仓库范围收缩——`write_access=false` 的仓只保留 read。
#[sqlx::test(migrations = "../../migrations")]
async fn read_only_task_scope_collapses_capabilities(pool: PgPool) {
    let (project_id, task_id, repo_id, owner) = seed(&pool, "SNAP-2").await;
    let dev_agent: Uuid = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member \
          WHERE project_id = $1 AND role = 'developer' LIMIT 1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    set_grant(
        &pool,
        &pm_application::actor::AuditActor::principal(owner),
        SYSTEM_TENANT,
        project_id,
        repo_id,
        dev_agent,
        pm_domain::GitCapability::ALL.to_vec(),
    )
    .await
    .unwrap();
    // Task 范围：只读。
    sqlx::query("UPDATE delivery.task_repository SET write_access = false WHERE task_id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let snaps = build_task_repo_caps_tx(&mut tx, SYSTEM_TENANT, project_id, task_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].effective, vec!["read".to_string()]);
}

/// I3-C §4.4：仓库非 verified → 能力清空。
#[sqlx::test(migrations = "../../migrations")]
async fn unverified_repository_yields_empty_capabilities(pool: PgPool) {
    let (project_id, task_id, repo_id, owner) = seed(&pool, "SNAP-3").await;
    let dev_agent: Uuid = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member \
          WHERE project_id = $1 AND role = 'developer' LIMIT 1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    set_grant(
        &pool,
        &pm_application::actor::AuditActor::principal(owner),
        SYSTEM_TENANT,
        project_id,
        repo_id,
        dev_agent,
        vec![pm_domain::GitCapability::Read],
    )
    .await
    .unwrap();
    sqlx::query("UPDATE integration.project_repository SET status = 'archived' WHERE id = $1")
        .bind(repo_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let snaps = build_task_repo_caps_tx(&mut tx, SYSTEM_TENANT, project_id, task_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(snaps[0].effective, Vec::<String>::new(), "non-verified repo denies all");
}

/// I3-D 无授权 → deny-all 快照仍落库（哈希存在——「无能力」是显式事实）。
#[sqlx::test(migrations = "../../migrations")]
async fn no_grants_still_writes_deny_all_snapshot(pool: PgPool) {
    let (project_id, _task, _repo, _owner) = seed(&pool, "SNAP-4").await;
    let cfg = config();
    let claimed = claim_once(&pool, "snap-worker", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .expect("claimable");
    let run_id = claimed.run_id;
    let (hash, snaps): (Option<String>, Option<serde_json::Value>) = sqlx::query_as(
        "SELECT policy_snapshot_hash, repository_policy_snapshots \
         FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(hash.is_some());
    let entries = snaps.unwrap().as_array().unwrap().clone();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["effective"], json!([]), "deny-all is explicit");
    let _ = project_id;
}
