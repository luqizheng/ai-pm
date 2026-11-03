//! S01 · I3-C/D 验收（V1.1 简化裁决）：Run 的**可访问仓库快照**。
//!
//! 简化后的模型：`可访问仓库 = Task 关联仓库 ∩ 仓库状态 = verified`，
//! 档位恒为 `read_write`（可读即可写，写操作不再分级）。
//!
//! 覆盖：
//! - 范围内 verified 仓 → 快照含该仓，档位 `read_write`；
//! - 哈希 = 快照 JSON 的 SHA-256，落 `task_run.policy_snapshot_hash`；
//! - 非 verified 仓 → 不在快照（缺项即拒绝）；
//! - Task 未关联任何仓库 → 空快照（哈希仍在：「无可访问仓库」是显式事实）；
//! - `write_access` 不再是访问判据（简化裁决 §0.0.5）。

use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::repo_access::{
    RepoAccessSnapshot, access_snapshot_hash, access_snapshot_json, build_task_repo_access_tx,
};
use pm_application::repository::{CreateRepositoryRequest, create_repository};
use pm_application::run_dispatch::{DispatchMode, RunDispatchConfig, claim_once};
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use sqlx::PgPool;
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
        "UPDATE integration.project_repository \
         SET status = 'verified', head_commit_sha = 'base0' WHERE id = $1",
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

/// I3-C/D：范围内 verified 仓 → 快照含该仓且档位 `read_write`；哈希 = 快照 JSON
/// 的 SHA-256；`policy_snapshot_hash` 落库。
#[sqlx::test(migrations = "../../migrations")]
async fn claim_writes_access_snapshot_and_hash(pool: PgPool) {
    let (project_id, _task_id, repo_id, _owner) = seed(&pool, "SNAP-1").await;

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
    assert_eq!(entries[0]["access"], json!("read_write"));

    // 哈希与快照内容一致（SHA-256 内容寻址）。
    #[derive(serde::Deserialize)]
    struct AccessMirror {
        repository_id: Uuid,
        access: String,
    }
    let expected: Vec<AccessMirror> = serde_json::from_value(snaps.clone()).unwrap();
    let mirror: Vec<RepoAccessSnapshot> = expected
        .into_iter()
        .map(|m| RepoAccessSnapshot {
            repository_id: m.repository_id,
            access: m.access,
        })
        .collect();
    assert_eq!(hash.as_deref(), Some(access_snapshot_hash(&mirror).as_str()));
    assert_eq!(access_snapshot_json(&mirror), snaps);
    assert_eq!(hash.as_ref().map(|h| h.len()), Some(64), "sha256 hex");
    let _ = project_id;
}

/// I3-C §4.4：仓库非 verified → 不在快照（缺项即拒绝，不做"降级只读"）。
#[sqlx::test(migrations = "../../migrations")]
async fn unverified_repository_is_absent_from_snapshot(pool: PgPool) {
    let (project_id, task_id, repo_id, _owner) = seed(&pool, "SNAP-3").await;
    sqlx::query("UPDATE integration.project_repository SET status = 'archived' WHERE id = $1")
        .bind(repo_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let snaps = build_task_repo_access_tx(&mut tx, SYSTEM_TENANT, project_id, task_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(snaps.is_empty(), "non-verified repo denies access");
}

/// I3-D：Task 未关联任何仓库 → 快照为空集，但哈希仍是显式事实。
///
/// 注：调度策略（`run_dispatch::policy`）要求「至少一个可写仓库」，因此空范围的
/// Task **不会被 claim**——空快照只能在构建层验证（本例即如此）。
#[sqlx::test(migrations = "../../migrations")]
async fn task_without_repositories_yields_empty_snapshot(pool: PgPool) {
    let (project_id, task_id, _repo_id, _owner) = seed(&pool, "SNAP-4").await;
    sqlx::query("DELETE FROM delivery.task_repository WHERE task_id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let snaps = build_task_repo_access_tx(&mut tx, SYSTEM_TENANT, project_id, task_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(snaps.is_empty());
    // 空范围也是显式事实：哈希照样可算（内容寻址）。
    assert_eq!(access_snapshot_hash(&snaps).len(), 64, "sha256 hex");
    assert_eq!(access_snapshot_json(&snaps), json!([]));
}

/// 简化裁决 §0.0.5：`task_repository.write_access` 不再是访问判据——
/// 只读标记的仓同样获得 `read_write` 档位（访问语义统一）。
#[sqlx::test(migrations = "../../migrations")]
async fn write_access_flag_is_not_an_access_criterion(pool: PgPool) {
    let (project_id, task_id, repo_id, _owner) = seed(&pool, "SNAP-5").await;
    sqlx::query("UPDATE delivery.task_repository SET write_access = false WHERE task_id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let snaps = build_task_repo_access_tx(&mut tx, SYSTEM_TENANT, project_id, task_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].repository_id, repo_id);
    assert_eq!(snaps[0].access, "read_write");
}
