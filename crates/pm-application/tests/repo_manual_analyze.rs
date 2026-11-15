//! S1（2026-11-13）单仓库手动增量分析集成测试（真实 PostgreSQL）。
//!
//! 覆盖：幂等重放、进行中 409、无锚点 422、失败释放名额、
//! HEAD 未前进仍可重触发、归档仓库拒绝。

mod common;
use pm_application::actor::AuditActor;
use pm_application::repository::{
    analyze_repository, confirm_repository_version, create_repository, AnalyzeRepositoryRequest,
    ConfirmVersionRequest, CreateRepositoryRequest, RepositoryError,
};
use sqlx::{query_scalar, PgPool};
use uuid::Uuid;

fn actor(principal_id: Uuid) -> AuditActor {
    AuditActor::principal(principal_id)
}

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    common::bootstrap_system_project(pool, code).await.0
}

async fn principal_id(pool: &PgPool) -> Uuid {
    query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', 'analyzer', true) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 建仓库并按用例设定 status / 锚点；`label` 保证 url 不撞唯一约束。
async fn seed_repo(
    pool: &PgPool,
    project_id: Uuid,
    who: Uuid,
    status: &str,
    commit: Option<&str>,
    label: &str,
) -> Uuid {
    let row = create_repository(
        pool,
        project_id,
        &actor(who),
        CreateRepositoryRequest {
            name: label.to_string(),
            url: format!("git@git.example.com:acme/{label}.git"),
            role: Some("main".into()),
            default_branch: Some("main".into()),
            description: None,
            selected_branch: commit.map(|_| "main".to_string()),
            selected_commit: commit.map(|value| value.to_string()),
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE integration.project_repository \
            SET status = $2, head_commit_sha = $3 WHERE id = $1",
    )
    .bind(row.id)
    .bind(status)
    .bind(commit)
    .execute(pool)
    .await
    .unwrap();
    row.id
}

async fn manual_intake_count(pool: &PgPool, project_id: Uuid) -> i64 {
    query_scalar(
        "SELECT count(*) FROM integration.intake_record \
          WHERE project_id = $1 AND idempotency_key LIKE 'repo_analyze:%'",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn req(key: Option<&str>) -> AnalyzeRepositoryRequest {
    AnalyzeRepositoryRequest {
        idempotency_key: key.map(str::to_string),
        mode: None,
    }
}

/// D-5：把项目最新报告置为指定状态（模拟 close / mark_failed 终态——
/// 真实链路由钩子驱动，单测直改行以聚焦 analyze 门本身）。
async fn set_latest_report_status(pool: &PgPool, project_id: Uuid, status: &str) {
    sqlx::query(
        "UPDATE integration.repo_analysis_report \
          SET status = $2, updated_at = now() \
         WHERE id = (SELECT id FROM integration.repo_analysis_report \
                      WHERE project_id = $1 ORDER BY created_at DESC LIMIT 1)",
    )
    .bind(project_id)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}

async fn set_all_task_status(pool: &PgPool, project_id: Uuid, status: &str) {
    sqlx::query(
        "UPDATE integration.intake_task SET task_status = $2 \
          WHERE intake_id IN \
            (SELECT id FROM integration.intake_record WHERE project_id = $1)",
    )
    .bind(project_id)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn manual_analyze_replays_same_idempotency_key(pool: PgPool) {
    let project_id = seed_project(&pool, "RA-R1").await;
    let who = principal_id(&pool).await;
    let repo_id = seed_repo(&pool, project_id, who, "verified", Some("abc1234"), "svc").await;

    let first = analyze_repository(&pool, repo_id, &actor(who), req(Some("fixed-key")))
        .await
        .unwrap();
    assert!(!first.replayed);
    assert_eq!(first.commit, "abc1234");
    assert_eq!(manual_intake_count(&pool, project_id).await, 1);

    // D-5：报告开着（analyzing）时重放被报告门遮蔽 → 409 ReportOpen。
    let replay_err = analyze_repository(&pool, repo_id, &actor(who), req(Some("fixed-key")))
        .await
        .unwrap_err();
    assert!(
        matches!(replay_err, RepositoryError::ReportOpen { .. }),
        "replay while report open: {replay_err:?}"
    );
    assert_eq!(manual_intake_count(&pool, project_id).await, 1);

    // 报告关闭后：同键重放命中自身 intake，零新增行。
    set_latest_report_status(&pool, project_id, "closed").await;
    let replay = analyze_repository(&pool, repo_id, &actor(who), req(Some("fixed-key")))
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.intake_id, first.intake_id);
    assert_eq!(manual_intake_count(&pool, project_id).await, 1);

    let key: String =
        query_scalar("SELECT idempotency_key FROM integration.intake_record WHERE id = $1")
            .bind(first.intake_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(key.starts_with(&format!("repo_analyze:{repo_id}:abc1234:")));
    // 与 verify / confirm 联动键族互不干扰。
    assert_ne!(key, format!("repo_verify:{repo_id}:abc1234"));
    assert_ne!(key, format!("repo_confirm:{repo_id}:abc1234"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn manual_analyze_conflicts_while_task_active(pool: PgPool) {
    let project_id = seed_project(&pool, "RA-C1").await;
    let who = principal_id(&pool).await;
    let repo_id = seed_repo(&pool, project_id, who, "verified", Some("abc1234"), "svc").await;

    analyze_repository(&pool, repo_id, &actor(who), req(Some("key-a")))
        .await
        .unwrap();
    assert_eq!(manual_intake_count(&pool, project_id).await, 1);

    // D-5：首个 intake 的报告仍 analyzing → 409 ReportOpen（报告门优先于任务门）。
    let err = analyze_repository(&pool, repo_id, &actor(who), req(Some("key-b")))
        .await
        .unwrap_err();
    assert!(matches!(err, RepositoryError::ReportOpen { .. }), "{err:?}");
    assert_eq!(manual_intake_count(&pool, project_id).await, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn manual_analyze_requires_verified_anchor(pool: PgPool) {
    let project_id = seed_project(&pool, "RA-V1").await;
    let who = principal_id(&pool).await;

    // 未核验且无锚点 → 422。
    let pending = seed_repo(&pool, project_id, who, "pending", None, "pending-svc").await;
    let err = analyze_repository(&pool, pending, &actor(who), req(None))
        .await
        .unwrap_err();
    assert!(matches!(err, RepositoryError::AnalyzeNoAnchor));

    // 已核验但无锚点同样 422（不臆测分支 tip）。
    let no_commit = seed_repo(&pool, project_id, who, "verified", None, "empty-svc").await;
    let err = analyze_repository(&pool, no_commit, &actor(who), req(None))
        .await
        .unwrap_err();
    assert!(matches!(err, RepositoryError::AnalyzeNoAnchor));
    assert_eq!(manual_intake_count(&pool, project_id).await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn manual_analyze_releases_slot_after_failure(pool: PgPool) {
    let project_id = seed_project(&pool, "RA-F1").await;
    let who = principal_id(&pool).await;
    let repo_id = seed_repo(&pool, project_id, who, "verified", Some("abc1234"), "svc").await;

    analyze_repository(&pool, repo_id, &actor(who), req(Some("key-a")))
        .await
        .unwrap();
    set_all_task_status(&pool, project_id, "FAILED").await;
    set_latest_report_status(&pool, project_id, "failed").await;

    // 任务 FAILED + 报告 failed 终态（不占门）→ 新触发时刻允许重发（同 commit 也放行）。
    let second = analyze_repository(&pool, repo_id, &actor(who), req(Some("key-b")))
        .await
        .unwrap();
    assert!(!second.replayed);
    assert_eq!(second.commit, "abc1234");
    assert_eq!(manual_intake_count(&pool, project_id).await, 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn manual_analyze_allows_retrigger_without_head_advance(pool: PgPool) {
    let project_id = seed_project(&pool, "RA-H1").await;
    let who = principal_id(&pool).await;
    let repo_id = seed_repo(&pool, project_id, who, "verified", Some("abc1234"), "svc").await;

    analyze_repository(&pool, repo_id, &actor(who), req(None))
        .await
        .unwrap();
    set_all_task_status(&pool, project_id, "COMPLETED").await;
    set_latest_report_status(&pool, project_id, "closed").await;

    // HEAD 未前进（commit 不变）、前次已完成、报告已关闭 → 仍可人工重触发。
    let second = analyze_repository(&pool, repo_id, &actor(who), req(None))
        .await
        .unwrap();
    assert!(!second.replayed);
    assert_eq!(manual_intake_count(&pool, project_id).await, 2);
}

/// 空白幂等键视同未传：服务端生成新触发段（不得把空白键当成可重放键）。
#[sqlx::test(migrations = "../../migrations")]
async fn blank_idempotency_key_generates_new_trigger(pool: PgPool) {
    let project_id = seed_project(&pool, "RA-B1").await;
    let who = principal_id(&pool).await;
    let repo_id = seed_repo(&pool, project_id, who, "verified", Some("abc1234"), "svc").await;

    let first = analyze_repository(&pool, repo_id, &actor(who), req(Some("   ")))
        .await
        .unwrap();
    set_all_task_status(&pool, project_id, "COMPLETED").await;
    set_latest_report_status(&pool, project_id, "closed").await;
    let second = analyze_repository(&pool, repo_id, &actor(who), req(Some("   ")))
        .await
        .unwrap();

    assert!(!first.replayed);
    assert!(!second.replayed, "空字符串不得命中重放");
    assert_ne!(first.intake_id, second.intake_id);
    assert_eq!(manual_intake_count(&pool, project_id).await, 2);
}

/// 回归（S1 契约「不得改动既有 `repo_confirm:` 幂等键语义」）：
/// 确认联动的键仍是 `repo_confirm:{repo}:{commit}` 三段式（trigger_token=None），
/// 未被手动分析新增的「触发时刻」段污染；同 commit 二次确认命中重放零新增行。
#[sqlx::test(migrations = "../../migrations")]
async fn confirm_linked_key_format_is_unchanged(pool: PgPool) {
    let project_id = seed_project(&pool, "RA-K1").await;
    let who = principal_id(&pool).await;
    let repo_id = seed_repo(&pool, project_id, who, "verified", Some("abc1234"), "svc").await;

    let confirm = || async {
        confirm_repository_version(
            &pool,
            repo_id,
            &actor(who),
            ConfirmVersionRequest {
                selected_branch: "main".into(),
                selected_commit: "abc1234".into(),
                reason: None,
            },
        )
        .await
        .unwrap()
    };
    confirm().await;
    confirm().await;

    let keys: Vec<String> = sqlx::query_scalar(
        "SELECT idempotency_key FROM integration.intake_record \
          WHERE project_id = $1 AND idempotency_key LIKE 'repo_confirm:%'",
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        keys,
        vec![format!("repo_confirm:{repo_id}:abc1234")],
        "confirm 联动键必须保持三段式且不新增行"
    );
    // 与手动族完全隔离：确认不产生任何 repo_analyze 键。
    assert_eq!(manual_intake_count(&pool, project_id).await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn archived_repository_rejects_manual_analysis(pool: PgPool) {
    let project_id = seed_project(&pool, "RA-A1").await;
    let who = principal_id(&pool).await;
    let repo_id = seed_repo(&pool, project_id, who, "archived", Some("abc1234"), "svc").await;

    let err = analyze_repository(&pool, repo_id, &actor(who), req(None))
        .await
        .unwrap_err();
    assert!(matches!(err, RepositoryError::Archived));
    assert_eq!(manual_intake_count(&pool, project_id).await, 0);
}
