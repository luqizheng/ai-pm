//! `/v1/pm/projects/{id}/repositories` 与 `/v1/pm/projects/{id}/ssh-key`
//! 的 HTTP 契约（P0 CRUD）。全程用真实 router + `#[sqlx::test]` 临时库，
//! 断言两件关键事：CRUD 语义正确，且私钥永不回显。

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;
use pm_domain::SYSTEM_TENANT;

fn with_secret() {
    std::env::set_var("AI_PM_SECRET_KEY", "test-secret-for-repositories");
}

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn delete(uri: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("{code} project"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id
}

#[sqlx::test(migrations = "../../migrations")]
async fn repository_crud_roundtrip(pool: PgPool) {
    with_secret();
    let app = router(pool.clone());
    let project_id = seed_project(&pool, "REPO-1").await;

    let (status, created) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/repositories"),
                json!({
                    "name": "主仓库",
                    "url": "git@gitlab.local:team/world-map.git",
                    "role": "main",
                    "default_branch": "main",
                    "description": "游戏客户端"
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {created}");
    assert_eq!(created["name"], "主仓库");
    assert_eq!(created["role"], "main");
    assert_eq!(created["revision"], 1);
    let repo_id = created["id"].as_str().unwrap().to_string();

    let (status, listed) = body_json(
        app.clone()
            .oneshot(get(&format!(
                "/v1/pm/projects/{project_id}/repositories"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["items"][0]["url"], "git@gitlab.local:team/world-map.git");

    let (status, updated) = body_json(
        app.clone()
            .oneshot(json_request(
                "PATCH",
                &format!("/v1/pm/repositories/{repo_id}"),
                json!({
                    "revision": 1,
                    "name": "主仓库（改名）",
                    "url": "git@gitlab.local:team/world-map.git",
                    "role": "docs",
                    "default_branch": "main",
                    "description": null
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {updated}");
    assert_eq!(updated["name"], "主仓库（改名）");
    assert_eq!(updated["role"], "docs");
    assert_eq!(updated["revision"], 2);

    // 旧 revision 再次提交 → 409
    let (status, conflict) = body_json(
        app.clone()
            .oneshot(json_request(
                "PATCH",
                &format!("/v1/pm/repositories/{repo_id}"),
                json!({
                    "revision": 1,
                    "name": "stale",
                    "url": "git@gitlab.local:team/world-map.git"
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {conflict}");
    assert_eq!(conflict["message"], "revision conflict");

    let (status, deleted) = body_json(
        app.clone()
            .oneshot(delete(&format!("/v1/pm/repositories/{repo_id}")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {deleted}");
    assert_eq!(deleted["id"], repo_id);

    let (_, listed) = body_json(
        app.clone()
            .oneshot(get(&format!(
                "/v1/pm/projects/{project_id}/repositories"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(listed["total"], 0);

    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND target_type = 'project_repository'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 3, "created + updated + deleted");
}

#[sqlx::test(migrations = "../../migrations")]
async fn repository_rejects_bad_urls_and_duplicates(pool: PgPool) {
    with_secret();
    let app = router(pool.clone());
    let project_id = seed_project(&pool, "REPO-2").await;
    let uri = format!("/v1/pm/projects/{project_id}/repositories");

    for bad in [
        "https://github.com/team/repo.git",
        "file:///etc/passwd",
        "ext::sh -c rm",
        "git@host:../escape",
    ] {
        let (status, body) = body_json(
            app.clone()
                .oneshot(json_request(
                    "POST",
                    &uri,
                    json!({ "name": "bad", "url": bad }),
                ))
                .await
            .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "url {bad} body {body}");
        assert_eq!(body["message"], "invalid repository url");
    }

    let good = json!({
        "name": "主仓库",
        "url": "git@gitlab.local:team/repo.git",
        "role": "main"
    });
    let (status, _) = body_json(
        app.clone()
            .oneshot(json_request("POST", &uri, good.clone()))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = body_json(
        app.clone()
            .oneshot(json_request("POST", &uri, good))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
    assert_eq!(
        body["message"],
        "repository url already bound to this project"
    );

    let (status, body) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &uri,
                json!({ "name": "x", "url": "git@host:team/repo.git", "role": "nope" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert_eq!(body["message"], "invalid repository role");
}

#[sqlx::test(migrations = "../../migrations")]
async fn ssh_key_never_leaks_private_material(pool: PgPool) {
    with_secret();
    let app = router(pool.clone());
    let project_id = seed_project(&pool, "REPO-3").await;
    let uri = format!("/v1/pm/projects/{project_id}/ssh-key");

    // 未生成时 404
    let (status, _) = body_json(app.clone().oneshot(get(&uri)).await.unwrap()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, key) = body_json(
        app.clone()
            .oneshot(json_request("POST", &uri, json!({})))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {key}");
    let public_key = key["public_key"].as_str().unwrap();
    assert!(public_key.starts_with("ssh-ed25519 "), "{public_key}");
    assert!(key["fingerprint"].as_str().unwrap().starts_with("SHA256:"));
    let serialized = key.to_string();
    assert!(!serialized.contains("private"), "{serialized}");
    assert!(!serialized.contains("PRIVATE KEY"), "{serialized}");

    // 库里只有密文，且不等于明文私钥
    let (ciphertext,): (Vec<u8>,) = sqlx::query_as(
        "SELECT private_key_ciphertext FROM integration.project_ssh_key WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!ciphertext.is_empty());
    let as_text = String::from_utf8_lossy(&ciphertext);
    assert!(!as_text.contains("BEGIN OPENSSH PRIVATE KEY"));

    let (status, fetched) = body_json(app.clone().oneshot(get(&uri)).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["public_key"], public_key);
    assert!(!fetched.to_string().contains("private"));

    let (status, _) = body_json(
        app.clone()
            .oneshot(delete(&uri))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = body_json(app.clone().oneshot(get(&uri)).await.unwrap()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn unknown_project_is_404(pool: PgPool) {
    with_secret();
    let app = router(pool);
    let unknown = Uuid::new_v4();
    let (status, body) = body_json(
        app.clone()
            .oneshot(get(&format!("/v1/pm/projects/{unknown}/repositories")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["message"], "project not found");

    let (status, body) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{unknown}/ssh-key"),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["message"], "project not found");
}
fn init_local_repo() -> (std::path::PathBuf, String) {
    let root = std::env::temp_dir().join(format!("ai-pm-api-repo-{}", Uuid::new_v4()));
    let path = root.join("repo");
    std::fs::create_dir_all(&path).unwrap();
    let run = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(&path)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    };
    run(&["init", "-q", "-b", "main"]);
    std::fs::write(path.join("README.md"), "# demo\n").unwrap();
    run(&["add", "."]);
    run(&["-c", "commit.gpgsign=false", "commit", "-q", "-m", "init"]);
    let url = format!("file://{}", path.to_string_lossy().replace('\\', "/"));
    (root, url)
}

#[sqlx::test(migrations = "../../migrations")]
async fn ssh_key_create_is_idempotent_and_revoke_blocks_verified_reference(pool: PgPool) {
    with_secret();
    let app = router(pool.clone());
    let project_id = seed_project(&pool, "REPO-4").await;
    let keys_uri = format!("/v1/pm/projects/{project_id}/ssh-keys");

    let (status, first) = body_json(
        app.clone()
            .oneshot(json_request("POST", &keys_uri, json!({})))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {first}");
    let (status, second) = body_json(
        app.clone()
            .oneshot(json_request("POST", &keys_uri, json!({})))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["fingerprint"], second["fingerprint"], "幂等复用同一把 key");

    let (root, local_url) = init_local_repo();
    let (status, repo) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/repositories"),
                json!({ "name": "本地夹具", "url": "git@example.invalid:team/demo.git", "role": "main" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {repo}");
    let repo_id = repo["id"].as_str().unwrap().to_string();
    sqlx::query("UPDATE integration.project_repository SET url = $2 WHERE id = $1")
        .bind(Uuid::parse_str(&repo_id).unwrap())
        .bind(&local_url)
        .execute(&pool)
        .await
        .unwrap();

    let (status, verified) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/repositories/{repo_id}/verify"),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let _ = std::fs::remove_dir_all(root);
    assert_eq!(status, StatusCode::OK, "body: {verified}");
    assert_eq!(verified["status"], "verified");
    assert_eq!(verified["default_branch"], "main");
    assert_eq!(verified["head_commit_sha"].as_str().unwrap().len(), 40);

    let (status, body) = body_json(
        app.clone()
            .oneshot(delete(&keys_uri))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
    assert_eq!(
        body["message"],
        "ssh key is still referenced by a verified repository"
    );

    let (status, listed) = body_json(
        app.clone()
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/ssh-keys")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["items"][0]["status"], "active");
}
