//! S01 · I3 V1.1 简化裁决验收（仓库访问判定 + 执行期强校验）。
//!
//! 简化后的模型（`docs/ui/smark-pm-s01-i3-…-v1.md` §0.0.2）：
//!
//! ```text
//! 可访问仓库 = Task 关联仓库 ∩ 仓库状态 = verified
//! 访问档位   = read_write（可读即可写，写操作不再分级）
//! ```
//!
//! 执行期判定 = 快照（上界）∩ 现状（仓库仍 verified ∧ Project 未归档）。
//!
//! 覆盖：
//! - 快照缺失 → 保守拒绝；
//! - 快照不含该仓 → 拒绝（不得跨仓借用）；
//! - 仓库状态不再 verified → 拒绝（现状收窄）；
//! - Project 归档 → 拒绝（I1 门禁）；
//! - claim 后快照含仓清单 + SHA-256 哈希落库；
//! - 快照不可变（事后改 Task 范围不改写已定格快照）。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::repo_access::{
    access_snapshot_hash, access_snapshot_json, execution_allows, snapshot_allows,
};
use pm_application::repository::{create_repository, CreateRepositoryRequest};
use pm_application::run_dispatch::{claim_once, DispatchMode, RunDispatchConfig};
use pm_domain::repo_access::RepoAccess;
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

struct World {
    project_id: Uuid,
    repository_id: Uuid,
    other_repository_id: Uuid,
}

async fn world(pool: &PgPool, code: &str) -> World {
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
    let actor = pm_application::actor::AuditActor::principal(boot.human_id);
    let repo = create_repository(
        pool,
        boot.project_id,
        &actor,
        CreateRepositoryRequest {
            name: "主仓库".to_string(),
            url: "git@example.invalid:team/demo.git".to_string(),
            role: Some("main".to_string()),
            default_branch: Some("main".to_string()),
            description: None,
            // 0038（G-05）新增的 Confirmed Commit 字段：测试种子不写确认列
            selected_branch: None,
            selected_commit: None,
        },
    )
    .await
    .unwrap();
    let other = create_repository(
        pool,
        boot.project_id,
        &actor,
        CreateRepositoryRequest {
            name: "旁仓库".to_string(),
            url: "git@example.invalid:team/other.git".to_string(),
            role: Some("service".to_string()),
            default_branch: Some("main".to_string()),
            description: None,
            selected_branch: None,
            selected_commit: None,
        },
    )
    .await
    .unwrap();
    World {
        project_id: boot.project_id,
        repository_id: repo.id,
        other_repository_id: other.id,
    }
}

/// 造一条 Task（关联仓库）+ queued Run，并让仓库进入 `verified`。
async fn seed_task_and_run(pool: &PgPool, w: &World, code: &str) -> Uuid {
    let developer: Uuid = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member \
          WHERE project_id = $1 AND role = 'developer' LIMIT 1",
    )
    .bind(w.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
         VALUES ($1, $2, 'ready', (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(w.project_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task SET assignee_id = $2 WHERE id = $1")
        .bind(task_id)
        .bind(developer)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE integration.project_repository \
         SET status = 'verified', head_commit_sha = 'base0' WHERE id = ANY($1)",
    )
    .bind(vec![w.repository_id, w.other_repository_id])
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, tenant_id) \
         VALUES ($1, $2, $3, true, (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(w.project_id)
    .bind(w.repository_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, run_kind, status, tenant_id) \
         VALUES ($1, $2, 1, 'delivery', 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(w.project_id)
    .bind(task_id)
    .execute(pool)
    .await
    .unwrap();
    task_id
}

/// 领取 Run → 取回定格的 `policy_snapshot`。
async fn claim_and_snapshot(pool: &PgPool) -> (Uuid, serde_json::Value) {
    let cfg = RunDispatchConfig::from_env();
    let claimed = claim_once(pool, "i3s-worker", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .expect("claimable");
    let snapshot: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT policy_snapshot FROM delivery.task_run WHERE id = $1")
            .bind(claimed.run_id)
            .fetch_one(pool)
            .await
            .unwrap();
    (
        claimed.run_id,
        snapshot.expect("policy_snapshot must be present"),
    )
}

/// 快照缺失 → 保守拒绝（deny-all）。
#[sqlx::test(migrations = "../../migrations")]
async fn missing_snapshot_denies_everything(pool: PgPool) {
    let w = world(&pool, "I3S-1").await;
    assert!(!snapshot_allows(
        None,
        w.repository_id,
        RepoAccess::ReadWrite
    ));
    assert!(!execution_allows(
        &pool,
        SYSTEM_TENANT,
        w.project_id,
        w.repository_id,
        None,
        RepoAccess::ReadWrite,
    )
    .await
    .unwrap());
}

/// 快照不含该仓 → 拒绝（不得跨仓借用）。
#[sqlx::test(migrations = "../../migrations")]
async fn snapshot_of_another_repository_is_rejected(pool: PgPool) {
    let w = world(&pool, "I3S-2").await;
    let snapshot = json!({
        "repo_access": [{
            "repository_id": w.other_repository_id.to_string(),
            "access": "read_write",
        }]
    });
    assert!(!snapshot_allows(
        Some(&snapshot),
        w.repository_id,
        RepoAccess::ReadWrite
    ));
    assert!(snapshot_allows(
        Some(&snapshot),
        w.other_repository_id,
        RepoAccess::ReadWrite
    ));
}

/// 主链：Task 范围内的 verified 仓库 → 快照允许 + 现状允许。
#[sqlx::test(migrations = "../../migrations")]
async fn in_scope_verified_repository_is_allowed(pool: PgPool) {
    let w = world(&pool, "I3S-3").await;
    seed_task_and_run(&pool, &w, "acc").await;
    let (_run_id, snapshot) = claim_and_snapshot(&pool).await;

    assert!(snapshot_allows(
        Some(&snapshot),
        w.repository_id,
        RepoAccess::ReadWrite
    ));
    assert!(execution_allows(
        &pool,
        SYSTEM_TENANT,
        w.project_id,
        w.repository_id,
        Some(&snapshot),
        RepoAccess::ReadWrite,
    )
    .await
    .unwrap());
}

/// Task 范围外的仓库不在快照里 → 拒绝。
#[sqlx::test(migrations = "../../migrations")]
async fn out_of_scope_repository_is_denied(pool: PgPool) {
    let w = world(&pool, "I3S-4").await;
    seed_task_and_run(&pool, &w, "acc").await;
    let (_run_id, snapshot) = claim_and_snapshot(&pool).await;
    assert!(!snapshot_allows(
        Some(&snapshot),
        w.other_repository_id,
        RepoAccess::ReadWrite
    ));
}

/// 现状收窄：仓库状态不再 verified → 执行期拒绝（快照不动）。
#[sqlx::test(migrations = "../../migrations")]
async fn repository_no_longer_verified_blocks_execution(pool: PgPool) {
    let w = world(&pool, "I3S-5").await;
    seed_task_and_run(&pool, &w, "acc").await;
    let (_run_id, snapshot) = claim_and_snapshot(&pool).await;
    assert!(snapshot_allows(
        Some(&snapshot),
        w.repository_id,
        RepoAccess::ReadWrite
    ));

    sqlx::query("UPDATE integration.project_repository SET status = 'archived' WHERE id = $1")
        .bind(w.repository_id)
        .execute(&pool)
        .await
        .unwrap();

    assert!(
        !execution_allows(
            &pool,
            SYSTEM_TENANT,
            w.project_id,
            w.repository_id,
            Some(&snapshot),
            RepoAccess::ReadWrite,
        )
        .await
        .unwrap(),
        "a repository that left `verified` must block execution"
    );
}

/// Project 归档 → 执行期拒绝（I1 门禁）。
#[sqlx::test(migrations = "../../migrations")]
async fn archived_project_blocks_execution(pool: PgPool) {
    let w = world(&pool, "I3S-6").await;
    seed_task_and_run(&pool, &w, "acc").await;
    let (_run_id, snapshot) = claim_and_snapshot(&pool).await;

    sqlx::query("UPDATE core.project SET status = 'archived' WHERE id = $1")
        .bind(w.project_id)
        .execute(&pool)
        .await
        .unwrap();

    assert!(
        !execution_allows(
            &pool,
            SYSTEM_TENANT,
            w.project_id,
            w.repository_id,
            Some(&snapshot),
            RepoAccess::ReadWrite,
        )
        .await
        .unwrap(),
        "an archived project must block execution"
    );
}

/// 快照落库形态：可访问仓库清单 + SHA-256 内容寻址哈希。
#[sqlx::test(migrations = "../../migrations")]
async fn claim_writes_access_snapshot_and_hash(pool: PgPool) {
    let w = world(&pool, "I3S-7").await;
    seed_task_and_run(&pool, &w, "acc").await;
    let (run_id, snapshot) = claim_and_snapshot(&pool).await;

    let entries = snapshot["repo_access"]
        .as_array()
        .expect("repo_access present");
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0]["repository_id"],
        json!(w.repository_id.to_string())
    );
    assert_eq!(entries[0]["access"], json!("read_write"));

    let (hash, stored): (Option<String>, Option<serde_json::Value>) = sqlx::query_as(
        "SELECT policy_snapshot_hash, repository_policy_snapshots \
         FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(hash.is_some(), "snapshot hash must be stored");
    assert_eq!(hash.as_ref().map(|h| h.len()), Some(64), "sha256 hex");

    #[derive(serde::Deserialize)]
    struct AccessMirror {
        repository_id: Uuid,
        access: String,
    }
    let stored = stored.expect("per-repo access snapshot must be stored");
    let mirror: Vec<AccessMirror> = serde_json::from_value(stored.clone()).unwrap();
    let snaps: Vec<pm_application::repo_access::RepoAccessSnapshot> = mirror
        .into_iter()
        .map(|m| pm_application::repo_access::RepoAccessSnapshot {
            repository_id: m.repository_id,
            access: m.access,
        })
        .collect();
    assert_eq!(hash.as_deref(), Some(access_snapshot_hash(&snaps).as_str()));
    assert_eq!(access_snapshot_json(&snaps), stored);
}

/// 快照不可变：事后把仓库移出 Task 范围，不改写已定格快照（快照是上界）。
#[sqlx::test(migrations = "../../migrations")]
async fn snapshot_is_immutable_after_claim(pool: PgPool) {
    let w = world(&pool, "I3S-8").await;
    let task_id = seed_task_and_run(&pool, &w, "acc").await;
    let (_run_id, snapshot) = claim_and_snapshot(&pool).await;

    sqlx::query("DELETE FROM delivery.task_repository WHERE task_id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();

    let still: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT policy_snapshot FROM delivery.task_run \
         WHERE policy_snapshot->'repo_access' IS NOT NULL LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let still = still.expect("snapshot persists");
    assert_eq!(
        still["repo_access"], snapshot["repo_access"],
        "an already-captured snapshot must not be rewritten"
    );
}
