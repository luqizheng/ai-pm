//! S01 · I6-B-05/06 导入契约验收：`repository_ids` 必填、幂等键透传、
//! `proposed` 态显式终止、导入域审计动作。
//!
//! 全程真实 PostgreSQL（`#[sqlx::test]`）与真实 HTTP 路由。

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

/// 种一个带 verified 仓库的项目，并让 dev-off 成为项目成员。
async fn seed_project_with_repo(pool: &PgPool) -> (Uuid, Uuid) {
    let app = router(pool.clone());
    let (status, body) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/pm/projects",
                json!({ "code": format!("IMP-{}", Uuid::new_v4().simple()), "name": "导入" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let project_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    // 项目经 bootstrap 创建，dev-off 已是项目 owner（无需再插 project_member）。

    let (status, body) = body_json(
        app.oneshot(json_request(
            "POST",
            &format!("/v1/pm/projects/{project_id}/repositories"),
            json!({
                "name": "主仓库",
                "url": "git@example.invalid:team/demo.git",
                "role": "main",
                "default_branch": "main"
            }),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let repository_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    sqlx::query(
        "UPDATE integration.project_repository SET status = 'verified' WHERE id = $1",
    )
    .bind(repository_id)
    .execute(pool)
    .await
    .unwrap();
    (project_id, repository_id)
}

async fn start_import(
    pool: &PgPool,
    project_id: Uuid,
    body: Value,
) -> (StatusCode, Value) {
    let app = router(pool.clone());
    body_json(
        app.oneshot(json_request(
            "POST",
            &format!("/v1/pm/projects/{project_id}/imports"),
            body,
        ))
        .await
        .unwrap(),
    )
    .await
}

// ── N-3：repository_ids 必填且非空 ────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn repository_ids_required(pool: PgPool) {
    let (project_id, _repo) = seed_project_with_repo(&pool).await;

    // 空 body → 422
    let (status, body) = start_import(&pool, project_id, json!({})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    assert_eq!(body["error"], json!("repository_ids_required"));

    // 空数组 → 422
    let (status, body) = start_import(&pool, project_id, json!({ "repository_ids": [] })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    assert_eq!(body["error"], json!("repository_ids_required"));
}

/// 跨项目仓库不得被导入集合引用。
#[sqlx::test(migrations = "../../migrations")]
async fn repository_ids_must_belong_to_project(pool: PgPool) {
    let (project_id, _repo) = seed_project_with_repo(&pool).await;
    let (_, other_repo) = seed_project_with_repo(&pool).await;

    let (status, body) =
        start_import(&pool, project_id, json!({ "repository_ids": [other_repo] })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    assert_eq!(body["error"], json!("repository_not_in_project"));
}

/// 正常路径：集合被固化——Import 只持有传入的仓库。
#[sqlx::test(migrations = "../../migrations")]
async fn import_accepts_repository_ids_and_freezes_the_set(pool: PgPool) {
    let (project_id, repo) = seed_project_with_repo(&pool).await;

    let (status, body) =
        start_import(&pool, project_id, json!({ "repository_ids": [repo] })).await;
    assert_eq!(status, StatusCode::ACCEPTED, "body: {body}");
    let import_id = Uuid::parse_str(body["import_id"].as_str().unwrap()).unwrap();

    let sources: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.repo_import_source WHERE import_id = $1",
    )
    .bind(import_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sources, 1, "the source set must be frozen at import creation");

    // 首轮 Attempt 已开（R-4-1：每轮都留档）。
    let attempts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.repo_import_source_attempt \
         WHERE import_id = $1 AND status = 'running'",
    )
    .bind(import_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(attempts, 1, "the first attempt must be opened at creation");

    // 初始化操作被占有（R-3-3）。
    let ops: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.project_initialization_operation \
         WHERE import_id = $1 AND status = 'active'",
    )
    .bind(import_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ops, 1, "the initialization operation must be acquired");
}

// ── N-4：幂等键透传 ─────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn idempotency_key_replays_the_same_import(pool: PgPool) {
    let (project_id, repo) = seed_project_with_repo(&pool).await;
    let key = format!("import-{}", Uuid::new_v4().simple());

    let (status, first) =
        start_import(&pool, project_id, json!({ "repository_ids": [repo], "idempotency_key": key })).await;
    assert_eq!(status, StatusCode::ACCEPTED, "body: {first}");
    let first_id = Uuid::parse_str(first["import_id"].as_str().unwrap()).unwrap();

    let (status, second) =
        start_import(&pool, project_id, json!({ "repository_ids": [repo], "idempotency_key": key })).await;
    assert_eq!(status, StatusCode::ACCEPTED, "body: {second}");
    let second_id = Uuid::parse_str(second["import_id"].as_str().unwrap()).unwrap();
    assert_eq!(first_id, second_id, "the same key must replay the same import");
    assert_eq!(second["replayed"], json!(true));

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM integration.repo_import")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "an idempotent replay must not create a second import");
}

// ── C-2：proposed 态显式终止 ───────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn proposed_import_can_be_terminated_and_releases_the_slot(pool: PgPool) {
    let (project_id, repo) = seed_project_with_repo(&pool).await;
    let (status, body) =
        start_import(&pool, project_id, json!({ "repository_ids": [repo] })).await;
    assert_eq!(status, StatusCode::ACCEPTED, "body: {body}");
    let import_id = Uuid::parse_str(body["import_id"].as_str().unwrap()).unwrap();

    // 直接把导入推到 proposed（apply 需要完整草案，这里只验状态机与名额）。
    sqlx::query("UPDATE integration.repo_import SET status = 'proposed' WHERE id = $1")
        .bind(import_id)
        .execute(&pool)
        .await
        .unwrap();

    let app = router(pool.clone());
    let (status, body) = body_json(
        app.oneshot(json_request(
            "POST",
            &format!("/v1/pm/projects/{project_id}/imports/{import_id}/terminate"),
            json!({}),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["terminated"], json!(true));

    let status: String =
        sqlx::query_scalar("SELECT status FROM integration.repo_import WHERE id = $1")
            .bind(import_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "cancelled");

    // 名额释放：可以再次发起导入。
    let (status, _) = start_import(&pool, project_id, json!({ "repository_ids": [repo] })).await;
    assert_eq!(status, StatusCode::ACCEPTED, "the active slot must be released");
}

// ── I6-B-06：审计动作 ───────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn import_start_is_audited(pool: PgPool) {
    let (project_id, repo) = seed_project_with_repo(&pool).await;
    let (status, body) =
        start_import(&pool, project_id, json!({ "repository_ids": [repo] })).await;
    assert_eq!(status, StatusCode::ACCEPTED, "body: {body}");
    let import_id = Uuid::parse_str(body["import_id"].as_str().unwrap()).unwrap();

    let started: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'repo_import.started' AND target_id = $2",
    )
    .bind(project_id)
    .bind(import_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(started, 1, "import start must be audited");

    let acquired: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'project_initialization.acquired'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(acquired, 1, "acquiring the initialization slot must be audited");
}

/// 终止动作的审计行带 reason 与 previous_status。
#[sqlx::test(migrations = "../../migrations")]
async fn termination_is_audited_with_reason(pool: PgPool) {
    let (project_id, repo) = seed_project_with_repo(&pool).await;
    let (status, body) =
        start_import(&pool, project_id, json!({ "repository_ids": [repo] })).await;
    assert_eq!(status, StatusCode::ACCEPTED, "body: {body}");
    let import_id = Uuid::parse_str(body["import_id"].as_str().unwrap()).unwrap();

    let app = router(pool.clone());
    let (status, _) = body_json(
        app.oneshot(json_request(
            "POST",
            &format!("/v1/pm/projects/{project_id}/imports/{import_id}/cancel"),
            json!({ "reason": "user_abort" }),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let after: Option<Value> = sqlx::query_scalar(
        "SELECT after_data FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'repo_import.terminated'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let after = after.expect("termination must be audited");
    assert_eq!(after["reason"], json!("user_abort"));
    assert_eq!(after["previous_status"], json!("queued"));
}
