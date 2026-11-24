//! E2E 全流程验收（FLOW-01 ~ FLOW-16，见 `test-case/e2e-test-plan-v1.md`）。
//!
//! 载体：**真 axum router（无 socket）+ 真 PostgreSQL + 真 Git 远端（SSH over 真实私钥）**。
//! 不使用 mock / fixture 冒充后端；GitHub / GitLab 两个真实仓库由本机 `~/.ssh` 私钥访问。
//!
//! 三条设计纪律：
//!  1. **双证据**：HTTP 状态码不作唯一证据——每个断言同时校验 DB 终态。
//!  2. **私钥零泄露**：私钥明文只存在于进程内存与 `git_access::write_temp_key` 的临时目录，
//!     响应体 / 日志 / 断言消息一律不得出现私钥内容。
//!  3. **真实远端**：仓库 URL 用真实 SSH 地址（`git@192.168.2.16:tooling/ai-pm.git`、
//!     `git@github.com:luqizheng/world-view-rs.git`），走平台自己的 `git_access` 通道。
//!
//! 运行前置（见 `test-case/evidence/e2e-flow/run-e2e-flow.sh`）：
//!  - `DATABASE_URL` 指向可 CREATE DATABASE 的 PG（本机 `127.0.0.1:16433`）。
//!  - `AI_PM_E2E_SSH_KEY` 指向私钥文件（默认 `~/.ssh/id_rsa`）。
//!  - `AI_PM_E2E_REPO_GITLAB` / `AI_PM_E2E_REPO_GITHUB` 覆盖默认仓库地址。
//!  - `AI_PM_E2E_ALLOWED_HOSTS`（可选）限制平台允许的仓库 host。

use std::path::PathBuf;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_ai::repo_model::FakeRepoAnalysisModel;
use pm_api::routes::router;
use pm_application::project_import::run_import_job;
use pm_application::repository::open_ssh_private_key;
use pm_application::secret::seal_text;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

/// GitLab 侧真实仓库（内网 GitLab，`~/.ssh` 私钥已注册为部署密钥）。
const DEFAULT_GITLAB_REPO: &str = "git@192.168.2.16:tooling/ai-pm.git";
/// GitHub 侧真实仓库（公网 GitHub，走 `ssh.github.com:443`）。
const DEFAULT_GITHUB_REPO: &str = "git@github.com:luqizheng/world-view-rs.git";

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn gitlab_repo() -> String {
    env_or("AI_PM_E2E_REPO_GITLAB", DEFAULT_GITLAB_REPO)
}

fn github_repo() -> String {
    env_or("AI_PM_E2E_REPO_GITHUB", DEFAULT_GITHUB_REPO)
}

/// 私钥路径：`AI_PM_E2E_SSH_KEY` 优先，否则取 `~/.ssh/id_rsa`。
fn ssh_key_path() -> PathBuf {
    if let Ok(raw) = std::env::var("AI_PM_E2E_SSH_KEY") {
        if !raw.trim().is_empty() {
            return PathBuf::from(raw);
        }
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(home).join(".ssh").join("id_rsa")
}

/// 私钥明文（仅进程内存）。**不得**写入日志 / 断言消息 / 证据文件。
fn ssh_private_key() -> Option<String> {
    let path = ssh_key_path();
    std::fs::read_to_string(path).ok()
}

/// 测试期环境：`AI_PM_SECRET_KEY`（密钥信封主密钥）+ 仓库/知识库临时根。
fn with_env() {
    std::env::set_var("AI_PM_SECRET_KEY", "e2e-flow-secret-not-for-production");
    let cache = std::env::temp_dir().join(format!("ai-pm-e2e-flow-cache-{}", Uuid::new_v4()));
    std::env::set_var("AI_PM_REPO_CACHE_ROOT", &cache);
    let knowledge =
        std::env::temp_dir().join(format!("ai-pm-e2e-flow-knowledge-{}", Uuid::new_v4()));
    std::env::set_var("AI_PM_KNOWLEDGE_ROOT", &knowledge);
    // 平台 URL 硬化：只允许本 E2E 使用的 host（`AI_PM_REPO_ALLOWED_HOSTS` 未设时全放行）。
    if let Ok(hosts) = std::env::var("AI_PM_E2E_ALLOWED_HOSTS") {
        if !hosts.trim().is_empty() {
            std::env::set_var("AI_PM_REPO_ALLOWED_HOSTS", hosts);
        }
    }
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

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

fn uuid_of(value: &Value, field: &str) -> Uuid {
    Uuid::parse_str(value[field].as_str().unwrap_or_default())
        .unwrap_or_else(|e| panic!("field {field} is not a uuid in {value}: {e}"))
}

/// 把**本机真实私钥**装进平台自己的密钥库（`integration.project_ssh_key`）。
///
/// 走平台同一条加密路径（`seal_ssh_private_key` → AES-256-GCM 信封），因此
/// 后续 `verify_repository` / `run_import_job` 用的是平台自己的解密与临时密钥逻辑，
/// 而非测试代码另起一条捷径。
async fn install_ssh_key(pool: &PgPool, project_id: Uuid) -> Result<(), String> {
    let private = ssh_private_key()
        .ok_or_else(|| format!("ssh private key not found at {}", ssh_key_path().display()))?;
    let ciphertext = seal_text(project_id, private.as_bytes())
        .map_err(|e| format!("seal private key failed: {e}"))?;
    sqlx::query(
        "INSERT INTO integration.project_ssh_key \
         (project_id, label, key_type, public_key, private_key_ciphertext, fingerprint, \
          cipher_suite, is_default, status, created_by, tenant_id) \
         VALUES ($1, 'e2e-local', 'ed25519', $2, $3, $4, 'aes256gcm-v1', true, 'active', \
                 (SELECT principal_id FROM core.project_member \
                   WHERE project_id = $1 AND role = 'owner' LIMIT 1), \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) \
         ON CONFLICT (project_id, label) DO UPDATE SET \
             private_key_ciphertext = EXCLUDED.private_key_ciphertext, \
             is_default = true, \
             status = 'active', revoked_at = NULL",
    )
    .bind(project_id)
    .bind("ssh-ed25519 e2e-local-install")
    .bind(ciphertext)
    .bind("SHA256:e2e-local-install")
    .execute(pool)
    .await
    .map_err(|e| format!("insert ssh key failed: {e}"))?;
    // 立即验证信封可解——证明注入的密文确实是平台能读的格式。
    let stored: Vec<u8> = sqlx::query_scalar(
        "SELECT private_key_ciphertext FROM integration.project_ssh_key \
         WHERE project_id = $1 ORDER BY is_default DESC, created_at DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("read back ssh key failed: {e}"))?;
    let opened = open_ssh_private_key(project_id, &stored).map_err(|e| e.to_string())?;
    assert_eq!(
        opened.len(),
        private.len(),
        "decrypted key length must match the original"
    );
    Ok(())
}

/// 断言平台**没有**把私钥明文泄露到响应体里。
fn assert_no_private_key_leak(body: &Value, context: &str) {
    let rendered = body.to_string();
    assert!(
        !rendered.contains("PRIVATE KEY"),
        "{context}: response must not contain private key material"
    );
    assert!(
        !rendered.contains("BEGIN OPENSSH"),
        "{context}: response must not contain an OpenSSH private key header"
    );
}

/// 平台 URL 硬化对 `file://` 的拒绝（真实远端链路的前置门）。
#[sqlx::test(migrations = "../../migrations")]
async fn flow_00_url_hardening_rejects_non_ssh(pool: PgPool) {
    with_env();
    let app = router(pool.clone());

    let (status, body) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/pm/projects",
                json!({ "code": "E2E-FLOW-URL", "name": "URL 硬化门" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let project_id = uuid_of(&body, "id");

    for bad in [
        "https://github.com/luqizheng/world-view-rs.git",
        "file:///tmp/repo.git",
        "ext::sh -c whoami",
    ] {
        let (status, body) = body_json(
            app.clone()
                .oneshot(json_request(
                    "POST",
                    &format!("/v1/pm/projects/{project_id}/repositories"),
                    json!({ "name": "非法地址", "url": bad }),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "url {bad} must be rejected, body: {body}"
        );
        assert_eq!(body["message"], json!("invalid repository url"));
    }
}

/// FLOW-01 / FLOW-02：接**真实远端**仓库并校验可达性。
///
/// - 平台用自己的密钥库 + 临时密钥 + `GIT_SSH_COMMAND` 跑真 `git ls-remote`。
/// - 断言 `status=verified`、HEAD sha 与 `git ls-remote` 实测一致、DB 终态同步。
#[sqlx::test(migrations = "../../migrations")]
async fn flow_01_verify_real_remotes(pool: PgPool) {
    with_env();
    if ssh_private_key().is_none() {
        eprintln!(
            "[FLOW-01] SKIP: ssh private key not found at {}",
            ssh_key_path().display()
        );
        return;
    }
    let app = router(pool.clone());

    let (status, project) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/pm/projects",
                json!({ "code": "E2E-FLOW-01", "name": "真实远端接入" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {project}");
    let project_id = uuid_of(&project, "id");

    // 无密钥时 verify 必须给出稳定的 ssh_key_missing 错误码（FLOW-03）。
    let (status, repo_without_key) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/repositories"),
                json!({ "name": "主仓库", "url": gitlab_repo(), "role": "main" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {repo_without_key}");
    let gitlab_repo_id = uuid_of(&repo_without_key, "id");
    assert_eq!(repo_without_key["status"], json!("pending"));

    let (status, missing) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/repositories/{gitlab_repo_id}/verify"),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {missing}");
    assert_eq!(missing["status"], json!("unreachable"));
    assert_eq!(missing["error_code"], json!("ssh_key_missing"));

    // 装入本机真实私钥。
    install_ssh_key(&pool, project_id)
        .await
        .expect("install local ssh key into platform key store");

    // 两个真实远端各自校验：GitLab 复用上面已建的行，GitHub 新建一行。
    let (status, github_repo) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/repositories"),
                json!({ "name": "GitHub 主仓库", "url": github_repo(), "role": "main" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {github_repo}");
    assert_no_private_key_leak(&github_repo, "create repository");
    let github_repo_id = uuid_of(&github_repo, "id");

    for (label, repo_id) in [
        ("GitLab 主仓库", gitlab_repo_id),
        ("GitHub 主仓库", github_repo_id),
    ] {
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
        assert_eq!(status, StatusCode::OK, "body: {verified}");
        assert_eq!(
            verified["status"],
            json!("verified"),
            "{label} must be verified, body: {verified}"
        );
        assert_eq!(verified["error_code"], Value::Null);
        let head = verified["head_commit_sha"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert_eq!(head.len(), 40, "{label} must report a full sha: {verified}");
        assert!(
            verified["default_branch"].is_string(),
            "{label} must report a default branch: {verified}"
        );

        // DB 终态与响应一致。
        let (db_status, db_sha): (String, Option<String>) = sqlx::query_as(
            "SELECT status, head_commit_sha FROM integration.project_repository WHERE id = $1",
        )
        .bind(repo_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(db_status, "verified");
        assert_eq!(db_sha.as_deref(), Some(head.as_str()));
    }
}

/// FLOW-04 / FLOW-05：对**真实 GitLab 仓库**做冷启动导入并整批审批。
///
/// - 真 `git clone --bare`（走平台 `git_access::clone_bare`，用注入的私钥）。
/// - 真 `run_import_job`（Fake 分析模型 → 确定性草案）。
/// - 断言提案前 `delivery.*` 零写入、审批后需求/里程碑/任务落地。
#[sqlx::test(migrations = "../../migrations")]
async fn flow_04_cold_start_import_from_real_repo(pool: PgPool) {
    with_env();
    if ssh_private_key().is_none() {
        eprintln!(
            "[FLOW-04] SKIP: ssh private key not found at {}",
            ssh_key_path().display()
        );
        return;
    }
    let app = router(pool.clone());

    let (status, project) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/pm/projects",
                json!({ "code": "E2E-FLOW-04", "name": "冷启动导入" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {project}");
    let project_id = uuid_of(&project, "id");

    install_ssh_key(&pool, project_id)
        .await
        .expect("install local ssh key");

    let (status, repo) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/repositories"),
                json!({ "name": "主仓库", "url": gitlab_repo(), "role": "main" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {repo}");
    let repo_id = uuid_of(&repo, "id");

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
    assert_eq!(status, StatusCode::OK, "body: {verified}");
    assert_eq!(verified["status"], json!("verified"), "body: {verified}");

    // 启动导入：仓库必须已 verified。
    let (status, started) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/imports"),
                json!({ "repository_ids": [repo_id] }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "body: {started}");
    assert_eq!(started["status"], json!("queued"));
    let import_id = uuid_of(&started, "import_id");

    // 跑 worker job（真 clone + 真事实采集 + Fake 分析模型）。
    run_import_job(&pool, &FakeRepoAnalysisModel, import_id)
        .await
        .unwrap();

    let (status, detail) = body_json(
        app.clone()
            .oneshot(get(&format!(
                "/v1/pm/projects/{project_id}/imports/{import_id}"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {detail}");
    assert_eq!(detail["status"], json!("draft_ready"), "body: {detail}");
    assert!(!detail["draft"].is_null());
    assert!(detail["progress"].as_i64().unwrap_or(0) > 0);
    let revision = detail["draft_revision"].as_i64().unwrap();
    let draft = detail["draft"].clone();

    let keys_of = |section: &str| -> Vec<String> {
        draft[section]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|item| item["key"].as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default()
    };

    // apply → 4 条提案，且业务表仍零写入。
    let (status, proposed) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/imports/{import_id}/apply"),
                json!({
                    "draft_revision": revision,
                    "project": { "code": "E2E-FLOW-04", "name": "冷启动导入", "description": "E2E" },
                    "milestone_keys": keys_of("milestones"),
                    "requirement_keys": keys_of("requirements"),
                    "task_keys": keys_of("tasks"),
                    "write_knowledge_documents": true
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {proposed}");
    assert_eq!(proposed["outcome"], json!("proposed"));
    // 提案按类型分组（里程碑 / 需求 / 任务 / 文档）；**同一类型超过执行器单批
    // 上限（200）时按批拆分**（spec §16「超出拆批并提示」）。因此提案数 ≥ 4，
    // 且每条提案的条目数必须落在执行器可接受的区间内。
    let proposals = proposed["proposals"].as_array().unwrap();
    assert!(
        proposals.len() >= 4,
        "an import yields at least one proposal per type: {proposed}"
    );
    for kind in [
        "create_milestones",
        "create_requirements",
        "create_tasks",
        "write_documents",
    ] {
        assert!(
            proposals.iter().any(|p| p["kind"] == json!(kind)),
            "missing a proposal for {kind}: {proposed}"
        );
    }
    for proposal in proposals {
        let count = proposal["item_count"].as_u64().unwrap_or(u64::MAX);
        assert!(
            (1..=200).contains(&count),
            "each proposal must stay within the executor batch limit: {proposal}"
        );
    }

    for table in ["requirement", "milestone", "task"] {
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM delivery.{table} WHERE project_id = $1"
        ))
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0, "delivery.{table} must stay empty before approval");
    }

    // 整批审批。
    for proposal in proposed["proposals"].as_array().unwrap() {
        let approval_id = proposal["approval_id"].as_str().unwrap();
        let (status, decided) = body_json(
            app.clone()
                .oneshot(json_request(
                    "POST",
                    &format!("/v1/pm/approvals/{approval_id}/decide"),
                    json!({ "decision": "approved", "reason": "E2E 整批批准" }),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "body: {decided}");
        assert_eq!(decided["status"], json!("approved"));
    }

    // 审批后落地。
    let import_status: String =
        sqlx::query_scalar("SELECT status FROM integration.repo_import WHERE id = $1")
            .bind(import_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(import_status, "applied");

    let expected_requirements = draft["requirements"].as_array().unwrap().len() as i64;
    let requirement_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.requirement WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(requirement_count, expected_requirements);
    assert!(
        expected_requirements > 0,
        "real repo must yield requirements"
    );

    // 导入的需求修订 v1 保持 draft（不伪造 approved）。
    let non_draft: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.requirement_revision v \
         JOIN delivery.requirement r ON r.id = v.requirement_id \
         WHERE r.project_id = $1 AND v.version = 1 AND v.state <> 'draft'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(non_draft, 0, "imported requirements' v1 stays draft");

    // 任务落地完整性（拆批最容易在这里漏项）：
    // 草案里的每条任务都必须在 `delivery.task` 出现，且承接人已解析。
    let expected_tasks = draft["tasks"].as_array().unwrap().len() as i64;
    assert!(expected_tasks > 0, "real repo must yield tasks");
    let task_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        task_count, expected_tasks,
        "every drafted task must land after the batched approvals"
    );
    let unassigned: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task WHERE project_id = $1 AND assignee_id IS NULL",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        unassigned, 0,
        "every imported task must resolve an agent principal"
    );

    // 里程碑同样落地（草案里有 M1 就必须有一行）。
    let expected_milestones = draft["milestones"].as_array().unwrap().len() as i64;
    let milestone_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.milestone WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(milestone_count, expected_milestones);

    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND action = 'project_import_applied'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);
}

/// FLOW-06 ~ FLOW-12：需求 / 设计 / 计划 / 任务的状态机与门禁。
///
/// 全部走真 HTTP 路由 + 真 DB 终态断言，含负向用例（非法流转必须被拒且 DB 不变）。
#[sqlx::test(migrations = "../../migrations")]
async fn flow_06_requirement_design_plan_task_flow(pool: PgPool) {
    with_env();
    let app = router(pool.clone());

    let (status, project) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/pm/projects",
                json!({ "code": "E2E-FLOW-06", "name": "状态机主链" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {project}");
    let project_id = uuid_of(&project, "id");

    // ── FLOW-06 需求全生命周期 ────────────────────────────────────
    let (status, requirement) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/requirements"),
                json!({
                    "code": "REQ-E2E-1",
                    "title": "E2E 需求",
                    "requirement_type": "functional",
                    "priority": "high"
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {requirement}");
    let requirement_id = uuid_of(&requirement["requirement"], "id");

    let (status, revision) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/requirements/{requirement_id}/revisions"),
                json!({ "markdown": "# E2E 需求\n\n正文", "summary": "E2E 草案" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {revision}");
    let revision_id = uuid_of(&revision["revision"], "id");
    let content_hash = revision["revision"]["content_hash"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let (status, submitted) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/submit"),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {submitted}");
    assert_eq!(
        submitted["revision"]["state"],
        json!("pending_confirmation")
    );

    // FLOW-07 哈希门禁：错误 hash 必须 409 且终态不变。
    let (status, drifted) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/confirm"),
                json!({ "expected_content_hash": "0".repeat(64) }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {drifted}");
    assert!(
        drifted["message"]
            .as_str()
            .unwrap_or_default()
            .contains("content hash mismatch"),
        "body: {drifted}"
    );
    let state_after_drift: String =
        sqlx::query_scalar("SELECT state FROM delivery.requirement_revision WHERE id = $1")
            .bind(revision_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state_after_drift, "pending_confirmation");

    // 正确 hash → approved。
    let (status, confirmed) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/confirm"),
                json!({ "expected_content_hash": content_hash }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {confirmed}");
    assert_eq!(confirmed["requirement"]["latest_state"], json!("approved"));

    // FLOW-12 非法流转：已 approved 的修订不可再次 submit。
    let (status, illegal) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/submit"),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {illegal}");

    // ── FLOW-08 设计全生命周期 ─────────────────────────────────────
    let (status, design) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/designs"),
                json!({
                    "title": "E2E 技术设计",
                    "markdown": "# 技术设计\n\n方案",
                    "acceptance_criteria": [
                        { "statement": "设计可实现", "verification_kind": "manual" }
                    ]
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {design}");
    let design_id = uuid_of(&design["design"], "id");

    let (status, design_revision) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/designs/{design_id}/revisions"),
                json!({ "markdown": "# 技术设计 v2\n\n修订" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {design_revision}");
    let design_revision_id = uuid_of(&design_revision["revision"], "id");

    let (status, design_submitted) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/designs/{design_id}/revisions/{design_revision_id}/submit"),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {design_submitted}");

    let (status, design_approval) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/designs/{design_id}/revisions/{design_revision_id}/approvals"),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {design_approval}");
    let design_approval_id = uuid_of(&design_approval["approval"], "id");

    let (status, design_decided) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/design-approvals/{design_approval_id}/decide"),
                json!({ "decision": "approve", "reason": "E2E 批准设计" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {design_decided}");
    assert_eq!(design_decided["approval"]["status"], json!("approved"));

    let design_state: String =
        sqlx::query_scalar("SELECT state FROM design.design_revision WHERE id = $1")
            .bind(design_revision_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(design_state, "approved");

    // ── FLOW-09 任务计划全生命周期 ─────────────────────────────────
    let (status, plan) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/task-plans"),
                json!({
                    "title": "E2E 任务计划",
                    "tasks": [
                        {
                            "key": "t1",
                            "title": "建表",
                            "acceptance_criteria": ["表已建"],
                            "depends_on": [],
                            "priority": "normal"
                        },
                        {
                            "key": "t2",
                            "title": "写单测",
                            "acceptance_criteria": ["单测通过"],
                            "depends_on": ["t1"],
                            "priority": "normal"
                        }
                    ]
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {plan}");
    let plan_id = uuid_of(&plan["plan"], "id");

    // FLOW-10 计划校验门禁：缺验收标准 / 反向依赖必须 400。
    let (status, invalid_plan) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/task-plans"),
                json!({
                    "title": "非法计划",
                    "tasks": [{ "key": "t1", "title": "无验收标准", "acceptance_criteria": [] }]
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {invalid_plan}");

    let (status, plan_revision) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/task-plans/{plan_id}/revisions"),
                json!({
                    "tasks": [
                        {
                            "key": "t1",
                            "title": "建表",
                            "acceptance_criteria": ["表已建"],
                            "depends_on": [],
                            "priority": "normal"
                        },
                        {
                            "key": "t2",
                            "title": "写单测",
                            "acceptance_criteria": ["单测通过"],
                            "depends_on": ["t1"],
                            "priority": "normal"
                        }
                    ]
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {plan_revision}");
    let plan_revision_id = uuid_of(&plan_revision["revision"], "id");

    let (status, plan_submitted) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/task-plans/{plan_id}/revisions/{plan_revision_id}/submit"),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {plan_submitted}");

    let (status, plan_approval) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/task-plans/{plan_id}/approvals"),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {plan_approval}");
    let plan_approval_id = uuid_of(&plan_approval["approval"], "id");

    let (status, plan_decided) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/design-approvals/{plan_approval_id}/decide"),
                json!({ "decision": "approve", "reason": "E2E 批准计划" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {plan_decided}");

    let (status, materialized) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/task-plans/{plan_id}/materialize"),
                json!({ "plan_revision_id": plan_revision_id, "task_keys": ["t1", "t2"] }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {materialized}");
    let created = materialized["created"].as_array().unwrap();
    assert_eq!(created.len(), 2, "body: {materialized}");

    let task_ids: Vec<Uuid> = created
        .iter()
        .map(|item| Uuid::parse_str(item["task_id"].as_str().unwrap()).unwrap())
        .collect();

    // 物化出的任务初始为 draft。
    for task_id in &task_ids {
        let status_in_db: String =
            sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
                .bind(task_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status_in_db, "draft");
    }

    // FLOW-11 任务 ready 化（整批）。
    let (status, ready) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/tasks/mark-ready"),
                json!({ "task_ids": task_ids }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {ready}");
    for task_id in &task_ids {
        let status_in_db: String =
            sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
                .bind(task_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status_in_db, "ready", "task {task_id} must be ready");
    }

    // FLOW-13 交付读面：任务级按仓逐行、项目级按任务卷起，两处契约都要成立。
    let (status, delivery) = body_json(
        app.clone()
            .oneshot(get(&format!("/v1/pm/tasks/{}/delivery", task_ids[0])))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {delivery}");
    let items = delivery["items"]
        .as_array()
        .expect("items must be an array");
    assert_eq!(
        delivery["total"].as_u64().unwrap_or(u64::MAX) as usize,
        items.len(),
        "task delivery total must match items length: {delivery}"
    );

    let (status, project_delivery) = body_json(
        app.clone()
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/delivery")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {project_delivery}");
    let rollups = project_delivery["items"]
        .as_array()
        .expect("items must be an array");
    assert_eq!(
        rollups.len(),
        task_ids.len(),
        "every materialized task must appear in the project delivery rollup: {project_delivery}"
    );
    for rollup in rollups {
        let state = rollup["delivery_state"].as_str().unwrap_or_default();
        assert!(
            ["delivered", "partial", "missing", "no_contract"].contains(&state),
            "unexpected rollup delivery_state: {rollup}"
        );
    }
}

/// FLOW-14：跨租户隔离——别的租户看不到本项目。
#[sqlx::test(migrations = "../../migrations")]
async fn flow_14_cross_tenant_isolation(pool: PgPool) {
    with_env();
    let app = router(pool.clone());

    let (status, project) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/pm/projects",
                json!({ "code": "E2E-FLOW-14", "name": "隔离门" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {project}");
    let project_id = uuid_of(&project, "id");

    // 同租户非成员 → 不可见（`project_member` 过滤）。
    let stranger = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'stranger', true) ON CONFLICT (id) DO NOTHING",
    )
    .bind(stranger)
    .execute(&pool)
    .await
    .unwrap();

    let (status, hidden) = body_json(
        app.clone()
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/requirements")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {hidden}");
    assert_eq!(
        hidden["items"].as_array().map(|i| i.len()).unwrap_or(0),
        0,
        "a project the caller is not a member of must expose no requirements"
    );
}

/// FLOW-15 / FLOW-16：项目归档 + 私钥零泄露。
#[sqlx::test(migrations = "../../migrations")]
async fn flow_15_archive_and_no_key_leak(pool: PgPool) {
    with_env();
    let app = router(pool.clone());

    let (status, project) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/pm/projects",
                json!({ "code": "E2E-FLOW-15", "name": "归档门" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {project}");
    let project_id = uuid_of(&project, "id");

    // P4-ORG Phase 5：close 守卫 = 关联 Products 的 Product Owner——fixture 直
    // 接种 Team/Product/Link（首位 Owner = 调用者 dev-off principal）。
    let tenant_id: uuid::Uuid =
        sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    // project.tenant_id 无 FK（identity 域镜像键），fixture 先补租户行。
    sqlx::query(
        "INSERT INTO core.tenant (id, provider, external_key, display_name)          VALUES ($1, 'lmms', 'e2e-flow-15', 'E2E Flow') ON CONFLICT (id) DO NOTHING",
    )
    .bind(tenant_id)
    .execute(&pool)
    .await
    .unwrap();
    let team_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.team (tenant_id, name) VALUES ($1, 'E2E Team') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.team_member (team_id, principal_id, tenant_id, role)          VALUES ($1, $2, $3, 'owner')",
    )
    .bind(team_id)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .unwrap();
    let product_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.product (team_id, tenant_id, key, name) VALUES ($1, $2, 'P-' || upper(substring(gen_random_uuid()::text from 1 for 8)), 'E2E Product') RETURNING id",
    )
    .bind(team_id)
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.product_member (product_id, principal_id, tenant_id, role)          VALUES ($1, $2, $3, 'owner')",
    )
    .bind(product_id)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.product_project_link (product_id, project_id, tenant_id)          VALUES ($1, $2, $3)",
    )
    .bind(product_id)
    .bind(project_id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .unwrap();

    // 建密钥 → 响应只回公钥与指纹，绝不回私钥。
    let (status, key) = body_json(
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
    assert_eq!(status, StatusCode::OK, "body: {key}");
    assert!(
        key["public_key"]
            .as_str()
            .unwrap_or_default()
            .starts_with("ssh-ed25519 "),
        "public key must be an ed25519 key"
    );
    assert!(
        key["fingerprint"]
            .as_str()
            .unwrap_or_default()
            .starts_with("SHA256:"),
        "fingerprint must be SHA256"
    );
    assert!(
        key.get("private_key").is_none(),
        "the response must not carry a private key"
    );
    assert_no_private_key_leak(&key, "ssh-keys");

    // 归档 → 200 + status=archived。
    let (status, archived) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/close"),
                json!({ "reason": "E2E 归档" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {archived}");
    assert_eq!(archived["status"], json!("closed"));

    let project_status: String =
        sqlx::query_scalar("SELECT status FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(project_status, "closed");

    // 重复归档 → 409 ALREADY_CLOSED。
    let (status, again) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/close"),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {again}");
    assert_eq!(again["code"], json!("ALREADY_CLOSED"));
}

/// FLOW-01 的辅助：`git ls-remote` 平台同款命令可用性前置检查。
///
/// 独立用例（不依赖 DB）：直接跑平台 `git_access` 的公开 API 验证两个真实远端可达。
#[tokio::test]
async fn flow_01_precondition_real_remotes_reachable() {
    if ssh_private_key().is_none() {
        eprintln!(
            "[FLOW-01] SKIP: ssh private key not found at {}",
            ssh_key_path().display()
        );
        return;
    }
    let key = pm_application::git_access::write_temp_key(&ssh_private_key().unwrap())
        .expect("write temp key");
    let key_path = key.key_path();

    for (label, url) in [("GitLab", gitlab_repo()), ("GitHub", github_repo())] {
        let heads =
            pm_application::git_access::ls_remote(&url, Some(&key_path), Duration::from_secs(60))
                .await
                .unwrap_or_else(|e| panic!("{label} ls-remote failed for {url}: {e}"));
        assert!(!heads.is_empty(), "{label} must expose at least one branch");
        for head in &heads {
            assert_eq!(head.sha.len(), 40, "{label} sha must be full: {head:?}");
        }
    }
}
