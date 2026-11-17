//! `/v1/pm/projects/{id}/repositories` 与 `/v1/pm/projects/{id}/ssh-key`
//! 的 HTTP 契约（P0 CRUD）。全程用真实 router + `#[sqlx::test]` 临时库，
//! 断言两件关键事：CRUD 语义正确，且私钥永不回显。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod common;

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
    let project_id = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("{code} project"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;
    common::join_dev(pool, project_id).await;
    project_id
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
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/repositories")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["total"], 1);
    assert_eq!(
        listed["items"][0]["url"],
        "git@gitlab.local:team/world-map.git"
    );

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
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/repositories")))
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

/// 基线 §5.1 + C-4：被历史执行记录引用的仓库**不可物理删除** → 409。
#[sqlx::test(migrations = "../../migrations")]
async fn repository_with_history_cannot_be_deleted(pool: PgPool) {
    let project_id = seed_project(&pool, "REPO-HISTORY").await;
    let app = router(pool.clone());
    let (status, created) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/repositories"),
                json!({
                    "name": "主仓库",
                    "url": "git@gitlab.local:team/hist.git",
                    "role": "main",
                    "default_branch": "main"
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {created}");
    let repo_id = created["id"].as_str().unwrap().to_string();

    // 制造一条历史引用：Import 的 Source 行指向该仓库（RESTRICT 生效条件）。
    sqlx::query(
        "INSERT INTO integration.repo_import \
         (tenant_id, project_id, requested_by, status) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, \
                 (SELECT principal_id FROM core.project_member WHERE project_id = $1 LIMIT 1), \
                 'analyzing')",
    )
    .bind(project_id)
    .execute(&pool)
    .await
    .unwrap();
    let import_id: Uuid =
        sqlx::query_scalar("SELECT id FROM integration.repo_import WHERE project_id = $1 LIMIT 1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO integration.repo_import_source \
         (tenant_id, project_id, import_id, repository_id) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3::uuid)",
    )
    .bind(project_id)
    .bind(import_id)
    .bind(&repo_id)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = body_json(
        app.oneshot(delete(&format!("/v1/pm/repositories/{repo_id}")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("history"),
        "the error must explain the history protection: {body}"
    );
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
    let keys_uri = format!("/v1/pm/projects/{project_id}/ssh-keys");

    // 未生成时列表为空
    let (status, listed) = body_json(app.clone().oneshot(get(&keys_uri)).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["total"], 0);

    let (status, key) = body_json(
        app.clone()
            .oneshot(json_request("POST", &keys_uri, json!({})))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {key}");
    assert_eq!(key["label"], "default");
    assert_eq!(key["is_default"], true);
    let key_id = key["id"].as_str().unwrap().to_string();
    let public_key = key["public_key"].as_str().unwrap();
    assert!(public_key.starts_with("ssh-ed25519 "), "{public_key}");
    assert!(key["fingerprint"].as_str().unwrap().starts_with("SHA256:"));
    let serialized = key.to_string();
    assert!(!serialized.contains("private"), "{serialized}");
    assert!(!serialized.contains("PRIVATE KEY"), "{serialized}");

    // 库里只有密文，且不等于明文私钥
    let (ciphertext,): (Vec<u8>,) = sqlx::query_as(
        "SELECT private_key_ciphertext FROM integration.project_ssh_key WHERE id = $1",
    )
    .bind(Uuid::parse_str(&key_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!ciphertext.is_empty());
    let as_text = String::from_utf8_lossy(&ciphertext);
    assert!(!as_text.contains("BEGIN OPENSSH PRIVATE KEY"));

    let (status, listed) = body_json(app.clone().oneshot(get(&keys_uri)).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["items"][0]["public_key"], public_key);
    assert!(!listed.to_string().contains("private"));

    let (status, _) = body_json(
        app.clone()
            .oneshot(delete(&format!("/v1/pm/ssh-keys/{key_id}")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // 删除 = 吊销（保留行，永不回显私钥材料）
    let (_, listed) = body_json(app.clone().oneshot(get(&keys_uri)).await.unwrap()).await;
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["items"][0]["status"], "revoked");
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
                &format!("/v1/pm/projects/{unknown}/ssh-keys"),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["message"], "project not found");

    let (status, body) = body_json(
        app.oneshot(delete(&format!("/v1/pm/ssh-keys/{}", Uuid::new_v4())))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["message"], "ssh key not found");
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
    assert_eq!(
        first["fingerprint"], second["fingerprint"],
        "幂等复用同一把 key"
    );

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

    // 迁移 0022：绑定默认把到 verified 仓库 → 吊销被拒；解绑后放行。
    let key_id = first["id"].as_str().unwrap().to_string();
    let bind_uri = format!("/v1/pm/repositories/{repo_id}/ssh-key");
    let (status, bound) = body_json(
        app.clone()
            .oneshot(json_request(
                "PUT",
                &bind_uri,
                json!({ "ssh_key_id": key_id }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {bound}");
    assert_eq!(bound["ssh_key_id"], key_id);

    let (status, body) = body_json(
        app.clone()
            .oneshot(delete(&format!("/v1/pm/ssh-keys/{key_id}")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
    assert_eq!(
        body["message"],
        "ssh key is still referenced by a verified repository"
    );

    let (status, unbound) = body_json(
        app.clone()
            .oneshot(json_request(
                "PUT",
                &bind_uri,
                json!({ "ssh_key_id": null }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {unbound}");
    assert_eq!(unbound["ssh_key_id"], serde_json::Value::Null);

    let (status, _) = body_json(
        app.clone()
            .oneshot(delete(&format!("/v1/pm/ssh-keys/{key_id}")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, listed) = body_json(
        app.clone()
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/ssh-keys")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["items"][0]["status"], "revoked");
}

/// S2（2026-11-14）独立 SSH 私钥设置页：重命名契约。
/// label 即名称；重命名后列表可见；与既有 label 冲突 → 409 label_taken；
/// 未知 key → 404；每次成功写审计 ssh_key.renamed。
/// （删除被引用拒绝已有用例：ssh_key_create_is_idempotent_and_revoke_blocks_verified_reference。）
#[sqlx::test(migrations = "../../migrations")]
async fn ssh_key_rename_updates_label_and_rejects_conflict(pool: PgPool) {
    with_secret();
    let app = router(pool.clone());
    let project_id = seed_project(&pool, "REPO-5").await;
    let keys_uri = format!("/v1/pm/projects/{project_id}/ssh-keys");

    let (status, key) = body_json(
        app.clone()
            .oneshot(json_request("POST", &keys_uri, json!({})))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {key}");
    let key_id = key["id"].as_str().unwrap().to_string();

    // 重命名成功：响应带新 label，列表可见。
    let rename_uri = format!("/v1/pm/ssh-keys/{key_id}");
    let (status, renamed) = body_json(
        app.clone()
            .oneshot(json_request(
                "PATCH",
                &rename_uri,
                json!({ "label": "github-main" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {renamed}");
    assert_eq!(renamed["label"], "github-main");
    let (status, listed) = body_json(app.clone().oneshot(get(&keys_uri)).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["items"][0]["label"], "github-main");

    // 同项目内撞既有 label → 409 label_taken（稳定 error 码供前端兜底）。
    sqlx::query(
        "INSERT INTO integration.project_ssh_key \
         (project_id, label, public_key, private_key_ciphertext, fingerprint, tenant_id) \
         VALUES ($1, 'gitlab', 'ssh-ed25519 AAAA test', '\\x00', 'SHA256:test', $2)",
    )
    .bind(project_id)
    .bind(SYSTEM_TENANT)
    .execute(&pool)
    .await
    .unwrap();
    let (status, conflict) = body_json(
        app.clone()
            .oneshot(json_request(
                "PATCH",
                &rename_uri,
                json!({ "label": "gitlab" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {conflict}");
    assert_eq!(conflict["error"], "label_taken");
    assert_eq!(conflict["message"], "ssh key label already in use");

    // 未知 key → 404。
    let (status, missing) = body_json(
        app.clone()
            .oneshot(json_request(
                "PATCH",
                &format!("/v1/pm/ssh-keys/{}", Uuid::new_v4()),
                json!({ "label": "whatever" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {missing}");

    // 仅首次成功写审计。
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND action = 'ssh_key.renamed'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1);
}

fn ssh_keygen_available() -> bool {
    std::process::Command::new("ssh-keygen")
        .arg("-V")
        .output()
        .is_ok()
}

/// 粘贴导入：合法 ed25519 → 200，公钥/指纹为推导值，私钥不回显、库里只有密文；
/// CRLF/空白粘贴形态也能导入；每次导入写审计 ssh_key.imported。
#[sqlx::test(migrations = "../../migrations")]
async fn ssh_key_import_derives_public_material_and_never_echoes_private(pool: PgPool) {
    if !ssh_keygen_available() {
        eprintln!("ssh-keygen unavailable; skipping");
        return;
    }
    with_secret();
    let app = router(pool.clone());
    let project_id = seed_project(&pool, "REPO-IMPORT-1").await;
    let uri = format!("/v1/pm/projects/{project_id}/ssh-keys/import");

    let generated = pm_application::secret::generate_ed25519("ai-pm:IMPORT-TEST").unwrap();

    let (status, imported) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &uri,
                json!({ "private_key": generated.private_key }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {imported}");
    assert_eq!(imported["public_key"], generated.public_key);
    assert_eq!(imported["fingerprint"], generated.fingerprint);
    assert_eq!(imported["label"], "imported");
    assert_eq!(imported["is_default"], true, "项目第一把自动成为默认");
    let serialized = imported.to_string();
    assert!(!serialized.contains("PRIVATE KEY"), "{serialized}");
    assert!(!serialized.contains("private_key"), "{serialized}");

    // 粘贴形态（首尾空白 + CRLF）同样可导入；同名 = 替换该把（is_default 保持）
    let crlf = format!("  {}\r\n", generated.private_key.replace('\n', "\r\n"));
    let (status, replaced) = body_json(
        app.clone()
            .oneshot(json_request("POST", &uri, json!({ "private_key": crlf })))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {replaced}");
    assert_eq!(replaced["fingerprint"], generated.fingerprint);
    assert_eq!(replaced["is_default"], true);
    let (count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM integration.project_ssh_key WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "一项目一把，覆盖不新增");

    // 库里只有密文
    let (ciphertext,): (Vec<u8>,) = sqlx::query_as(
        "SELECT private_key_ciphertext FROM integration.project_ssh_key WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let as_text = String::from_utf8_lossy(&ciphertext);
    assert!(!as_text.contains("BEGIN OPENSSH PRIVATE KEY"));

    let (audits,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND action = 'ssh_key.imported'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 2, "每次导入写审计");
}

/// 无效私钥（垃圾文本 / 带口令）→ 400 Validation，且不落库；越权项目 → 404。
#[sqlx::test(migrations = "../../migrations")]
async fn ssh_key_import_rejects_invalid_private_key(pool: PgPool) {
    if !ssh_keygen_available() {
        eprintln!("ssh-keygen unavailable; skipping");
        return;
    }
    with_secret();
    let app = router(pool.clone());
    let project_id = seed_project(&pool, "REPO-IMPORT-2").await;
    let uri = format!("/v1/pm/projects/{project_id}/ssh-keys/import");

    let (status, body) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &uri,
                json!({ "private_key": "definitely not a key" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert_eq!(
        body["message"],
        "private key is invalid or passphrase-protected"
    );

    // 带口令的私钥：ssh-keygen -y 无法推导（不接受交互）→ 400
    let dir = std::env::temp_dir().join(format!("ai-pm-api-pp-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let key_path = dir.join("id_encrypted");
    let gen = std::process::Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "secret-pass", "-C", "pp", "-f"])
        .arg(&key_path)
        .output()
        .unwrap();
    assert!(gen.status.success());
    let pem = std::fs::read_to_string(&key_path).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    let (status, body) = body_json(
        app.clone()
            .oneshot(json_request("POST", &uri, json!({ "private_key": pem })))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert_eq!(
        body["message"],
        "private key is invalid or passphrase-protected"
    );

    let (count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM integration.project_ssh_key WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0, "失败导入不落库");

    // 越权（不存在项目）→ 404 project not found
    let unknown = Uuid::new_v4();
    let (status, body) = body_json(
        app.oneshot(json_request(
            "POST",
            &format!("/v1/pm/projects/{unknown}/ssh-keys/import"),
            json!({ "private_key": "x" }),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    assert_eq!(body["message"], "project not found");
}

/// 多把共存（迁移 0022）：不同 label 各成一把，首把默认；同名替换不动其它把。
#[sqlx::test(migrations = "../../migrations")]
async fn ssh_key_multi_labels_coexist_and_replace(pool: PgPool) {
    if !ssh_keygen_available() {
        eprintln!("ssh-keygen unavailable; skipping");
        return;
    }
    with_secret();
    let app = router(pool.clone());
    let project_id = seed_project(&pool, "REPO-MULTI-KEY").await;
    let uri = format!("/v1/pm/projects/{project_id}/ssh-keys/import");

    let key_a = pm_application::secret::generate_ed25519("ai-pm:A").unwrap();
    let key_b = pm_application::secret::generate_ed25519("ai-pm:B").unwrap();

    let (status, a) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &uri,
                json!({ "private_key": key_a.private_key, "label": "github" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {a}");
    assert_eq!(a["label"], "github");
    assert_eq!(a["is_default"], true);

    let (status, b) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &uri,
                json!({ "private_key": key_b.private_key, "label": "gitlab" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {b}");
    assert_eq!(b["is_default"], false, "第二把不夺默认");

    // 同名替换：github 换钥，gitlab 不受影响，is_default 保持 false
    let key_a2 = pm_application::secret::generate_ed25519("ai-pm:A2").unwrap();
    let (status, replaced) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &uri,
                json!({ "private_key": key_a2.private_key, "label": "github" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {replaced}");
    assert_eq!(replaced["fingerprint"], key_a2.fingerprint);
    assert_eq!(replaced["is_default"], true, "同名替换保持默认地位");

    let (status, listed) = body_json(
        app.clone()
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/ssh-keys")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["total"], 2, "同名替换不新增行");
    // 默认把排最前（github 保持首把的默认地位）
    assert_eq!(listed["items"][0]["label"], "github");
    assert_eq!(listed["items"][0]["is_default"], true);
    assert_eq!(listed["items"][1]["label"], "gitlab");
    assert_eq!(listed["items"][1]["is_default"], false);
}

/// 仓库绑定校验：绑定/切换/解绑；越权 key 400、未知 key 404。
#[sqlx::test(migrations = "../../migrations")]
async fn repository_bind_key_validates_project_and_existence(pool: PgPool) {
    if !ssh_keygen_available() {
        eprintln!("ssh-keygen unavailable; skipping");
        return;
    }
    with_secret();
    let app = router(pool.clone());
    let project_id = seed_project(&pool, "REPO-BIND-1").await;
    let other_project = seed_project(&pool, "REPO-BIND-2").await;

    let (status, created) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/repositories"),
                json!({ "name": "主仓库", "url": "git@gitlab.local:team/bind.git", "role": "main" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {created}");
    let repo_id = created["id"].as_str().unwrap().to_string();
    let bind_uri = format!("/v1/pm/repositories/{repo_id}/ssh-key");

    // 两把 key：默认（ensure）+ 导入
    let (status, default_key) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/ssh-keys"),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {default_key}");
    let (status, imported_key) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/ssh-keys/import"),
                json!({
                    "private_key":
                        pm_application::secret::generate_ed25519("ai-pm:BIND").unwrap().private_key,
                    "label": "github"
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {imported_key}");
    let default_key_id = default_key["id"].as_str().unwrap().to_string();
    let imported_key_id = imported_key["id"].as_str().unwrap().to_string();

    // 绑定导入把 → 回读
    let (status, bound) = body_json(
        app.clone()
            .oneshot(json_request(
                "PUT",
                &bind_uri,
                json!({ "ssh_key_id": imported_key_id }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {bound}");
    assert_eq!(bound["ssh_key_id"], imported_key_id);

    // 切换到默认把 → 解绑（null = 跟随项目默认）
    let (status, switched) = body_json(
        app.clone()
            .oneshot(json_request(
                "PUT",
                &bind_uri,
                json!({ "ssh_key_id": default_key_id }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {switched}");
    assert_eq!(switched["ssh_key_id"], default_key_id);
    let (status, unbound) = body_json(
        app.clone()
            .oneshot(json_request(
                "PUT",
                &bind_uri,
                json!({ "ssh_key_id": null }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {unbound}");
    assert_eq!(unbound["ssh_key_id"], serde_json::Value::Null);

    // 未知 key → 404
    let (status, body) = body_json(
        app.clone()
            .oneshot(json_request(
                "PUT",
                &bind_uri,
                json!({ "ssh_key_id": Uuid::new_v4().to_string() }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");

    // 他项目 key → 400 越权
    let (status, foreign) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{other_project}/ssh-keys/import"),
                json!({
                    "private_key":
                        pm_application::secret::generate_ed25519("ai-pm:FOREIGN").unwrap().private_key,
                    "label": "github"
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {foreign}");
    let foreign_key_id = foreign["id"].as_str().unwrap().to_string();
    let (status, body) = body_json(
        app.clone()
            .oneshot(json_request(
                "PUT",
                &bind_uri,
                json!({ "ssh_key_id": foreign_key_id }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert_eq!(body["message"], "ssh key belongs to another project");
}

/// S1（2026-11-13）：POST /v1/pm/repositories/{id}/analyze 的 HTTP 契约。
/// 未核验 → 422；202 受理 + 同键幂等重放；已有活跃分析 → 409。
#[sqlx::test(migrations = "../../migrations")]
async fn manual_analyze_http_contract(pool: PgPool) {
    with_secret();
    let app = router(pool.clone());
    let project_id = seed_project(&pool, "REPO-AN").await;

    let (status, created) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/repositories"),
                json!({
                    "name": "svc",
                    "url": "git@gitlab.local:team/svc.git",
                    "role": "main",
                    "default_branch": "main"
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {created}");
    let repo_id = created["id"].as_str().unwrap().to_string();
    let analyze_uri = format!("/v1/pm/repositories/{repo_id}/analyze");

    // 未核验 / 无锚点 → 422。
    let (status, body) = body_json(
        app.clone()
            .oneshot(json_request("POST", &analyze_uri, json!({})))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    assert_eq!(body["error"], "analysis_no_anchor");

    sqlx::query(
        "UPDATE integration.project_repository \
            SET status = 'verified', head_commit_sha = 'abc1234', selected_commit = 'abc1234' \
          WHERE id = $1",
    )
    .bind(Uuid::parse_str(&repo_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();

    let (status, first) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &analyze_uri,
                json!({ "idempotency_key": "fixed-key" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "body: {first}");
    assert_eq!(first["replayed"], false);
    assert_eq!(first["commit"], "abc1234");

    // D-5 报告门先于幂等重放（c50bc1c 起）：首次 analyze 已建 analyzing 报告，
    // 报告 open 期间任何 analyze（含同键重放）→ 409 report_open（带 report_id）。
    let (status, gated) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &analyze_uri,
                json!({ "idempotency_key": "fixed-key" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {gated}");
    assert_eq!(gated["error"], "report_open");
    assert_eq!(gated["report_id"], first["report_id"]);

    // 报告 failed（终态不占门）释放门 → 同键重放：202 + replayed=true
    // （活跃任务检查排除本次幂等键自身）。
    sqlx::query(
        "UPDATE integration.repo_analysis_report \
            SET status = 'failed', error_code = 'fixture', error_detail = 'fixture' \
          WHERE id = $1",
    )
    .bind(Uuid::parse_str(gated["report_id"].as_str().unwrap()).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    let (status, replay) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &analyze_uri,
                json!({ "idempotency_key": "fixed-key" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "body: {replay}");
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["intake_id"], first["intake_id"]);

    // 不同触发时刻但已有活跃任务 → 409（进行中 / 限流）。
    let (status, conflict) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &analyze_uri,
                json!({ "idempotency_key": "other-key" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {conflict}");
    assert_eq!(
        conflict["message"],
        "repository analysis is already in progress"
    );
    // 稳定错误码供前端本地化兜底（S4 详情面板）。
    assert_eq!(conflict["error"], "analysis_in_progress");
}
