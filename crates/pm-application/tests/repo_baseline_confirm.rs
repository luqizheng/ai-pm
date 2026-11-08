//! S01-03 §5 Branch 与 Commit 基准治理（G-05 / U-R12）集成测试（真实 PostgreSQL）。
//!
//! 覆盖：接入即确认（首条确认记录）、人类确认更新（append-only 历史 + previous
//! 值可追溯）、PATCH 不得触碰 Confirmed 基准、归档后禁止确认、缺分支拒绝。

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::repository::{
    archive_repository, confirm_repository_version, create_repository, update_repository,
    ConfirmVersionRequest, CreateRepositoryRequest, RepositoryError, UpdateRepositoryRequest,
};
use pm_domain::SYSTEM_TENANT;
use sqlx::{query_scalar, PgPool};
use uuid::Uuid;

fn actor(principal_id: Uuid) -> AuditActor {
    AuditActor::principal(principal_id)
}

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
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
    o.project_id
}

async fn principal_id(pool: &PgPool) -> Uuid {
    query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', 'confirmer', true) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn confirmation_count(pool: &PgPool, repository_id: Uuid) -> i64 {
    query_scalar(
        "SELECT count(*) FROM integration.repository_version_confirmation \
         WHERE repository_id = $1",
    )
    .bind(repository_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_with_selected_commit_writes_initial_confirmation(pool: PgPool) {
    let project_id = seed_project(&pool, "RB-C1").await;
    let who = principal_id(&pool).await;

    let row = create_repository(
        &pool,
        project_id,
        &actor(who),
        CreateRepositoryRequest {
            name: "payment-service".into(),
            url: "git@git.example.com:acme/payment-service.git".into(),
            role: Some("main".into()),
            default_branch: Some("main".into()),
            description: None,
            selected_branch: Some("main".into()),
            selected_commit: Some("abc1234".into()),
        },
    )
    .await
    .unwrap();

    assert_eq!(row.selected_branch.as_deref(), Some("main"));
    assert_eq!(row.selected_commit.as_deref(), Some("abc1234"));
    assert_eq!(row.selected_confirmed_by, Some(who));
    assert!(row.selected_confirmed_at.is_some());
    // 首条确认记录：接入即确认（previous 为空）。
    assert_eq!(confirmation_count(&pool, row.id).await, 1);
    let (prev_branch, prev_commit): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT previous_branch, previous_commit \
             FROM integration.repository_version_confirmation WHERE repository_id = $1",
    )
    .bind(row.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(prev_branch, None);
    assert_eq!(prev_commit, None);
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_without_commit_keeps_baseline_unconfirmed(pool: PgPool) {
    let project_id = seed_project(&pool, "RB-C2").await;
    let who = principal_id(&pool).await;

    let row = create_repository(
        &pool,
        project_id,
        &actor(who),
        CreateRepositoryRequest {
            name: "docs".into(),
            url: "git@git.example.com:acme/docs.git".into(),
            role: Some("docs".into()),
            default_branch: None,
            description: None,
            selected_branch: None,
            selected_commit: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(row.selected_commit, None);
    assert_eq!(confirmation_count(&pool, row.id).await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_with_commit_but_no_branch_is_rejected(pool: PgPool) {
    let project_id = seed_project(&pool, "RB-C3").await;
    let who = principal_id(&pool).await;

    let err = create_repository(
        &pool,
        project_id,
        &actor(who),
        CreateRepositoryRequest {
            name: "x".into(),
            url: "git@git.example.com:acme/x.git".into(),
            role: None,
            default_branch: None,
            description: None,
            selected_branch: None,
            selected_commit: Some("abc1234".into()),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, RepositoryError::Validation(_)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn confirm_moves_baseline_and_records_previous_values(pool: PgPool) {
    let project_id = seed_project(&pool, "RB-M1").await;
    let who = principal_id(&pool).await;

    let row = create_repository(
        &pool,
        project_id,
        &actor(who),
        CreateRepositoryRequest {
            name: "payment-service".into(),
            url: "git@git.example.com:acme/payment-service.git".into(),
            role: None,
            default_branch: Some("main".into()),
            description: None,
            selected_branch: Some("main".into()),
            selected_commit: Some("abc1234".into()),
        },
    )
    .await
    .unwrap();
    let revision_before = row.revision;

    // Observed HEAD 已前移（verify 回写），人类确认采用新基准。
    sqlx::query(
        "UPDATE integration.project_repository SET head_commit_sha = 'def5678' WHERE id = $1",
    )
    .bind(row.id)
    .execute(&pool)
    .await
    .unwrap();

    let updated = confirm_repository_version(
        &pool,
        row.id,
        &actor(who),
        ConfirmVersionRequest {
            selected_branch: "main".into(),
            selected_commit: "def5678".into(),
            reason: Some("quarterly rebase".into()),
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.selected_commit.as_deref(), Some("def5678"));
    assert_eq!(updated.selected_confirmed_by, Some(who));
    assert_eq!(updated.revision, revision_before + 1);

    // append-only：两条记录，第二条记录 previous = 第一条的基准。
    assert_eq!(confirmation_count(&pool, row.id).await, 2);
    let (prev_branch, prev_commit, reason): (Option<String>, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT previous_branch, previous_commit, reason \
             FROM integration.repository_version_confirmation \
             WHERE repository_id = $1 ORDER BY confirmed_at DESC, id DESC LIMIT 1",
        )
        .bind(row.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(prev_branch.as_deref(), Some("main"));
    assert_eq!(prev_commit.as_deref(), Some("abc1234"));
    assert_eq!(reason.as_deref(), Some("quarterly rebase"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn patch_update_cannot_touch_selected_baseline(pool: PgPool) {
    let project_id = seed_project(&pool, "RB-P1").await;
    let who = principal_id(&pool).await;

    let row = create_repository(
        &pool,
        project_id,
        &actor(who),
        CreateRepositoryRequest {
            name: "payment-service".into(),
            url: "git@git.example.com:acme/payment-service.git".into(),
            role: None,
            default_branch: Some("main".into()),
            description: None,
            selected_branch: Some("main".into()),
            selected_commit: Some("abc1234".into()),
        },
    )
    .await
    .unwrap();

    // UpdateRepositoryRequest 结构上就没有 selected_* 字段：PATCH 只改身份/角色/分支。
    let patched = update_repository(
        &pool,
        row.id,
        &actor(who),
        UpdateRepositoryRequest {
            revision: row.revision,
            name: row.name.clone(),
            url: row.url.clone(),
            role: Some("service".into()),
            default_branch: Some("develop".into()),
            description: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(patched.selected_branch.as_deref(), Some("main"));
    assert_eq!(patched.selected_commit.as_deref(), Some("abc1234"));
    assert_eq!(patched.role, "service");
    assert_eq!(confirmation_count(&pool, row.id).await, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn archived_repository_rejects_confirmation(pool: PgPool) {
    let project_id = seed_project(&pool, "RB-A1").await;
    let who = principal_id(&pool).await;

    let row = create_repository(
        &pool,
        project_id,
        &actor(who),
        CreateRepositoryRequest {
            name: "legacy".into(),
            url: "git@git.example.com:acme/legacy.git".into(),
            role: None,
            default_branch: None,
            description: None,
            selected_branch: Some("main".into()),
            selected_commit: Some("abc1234".into()),
        },
    )
    .await
    .unwrap();
    archive_repository(&pool, row.id, &actor(who))
        .await
        .unwrap();

    let err = confirm_repository_version(
        &pool,
        row.id,
        &actor(who),
        ConfirmVersionRequest {
            selected_branch: "main".into(),
            selected_commit: "def5678".into(),
            reason: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, RepositoryError::Archived));
    // 归档后基准未被改动。
    let commit: Option<String> =
        query_scalar("SELECT selected_commit FROM integration.project_repository WHERE id = $1")
            .bind(row.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(commit.as_deref(), Some("abc1234"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn confirm_on_missing_repository_is_not_found(pool: PgPool) {
    let who = principal_id(&pool).await;
    let err = confirm_repository_version(
        &pool,
        Uuid::new_v4(),
        &actor(who),
        ConfirmVersionRequest {
            selected_branch: "main".into(),
            selected_commit: "def5678".into(),
            reason: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, RepositoryError::NotFound));
}
