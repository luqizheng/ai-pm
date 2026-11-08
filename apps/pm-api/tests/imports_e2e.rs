//! 冷启动导入端到端验收（plan Task 14，Step 1–3）：真 HTTP router（无 socket）+
//! 真 PostgreSQL + 真本地 Git 仓库，一次跑通 Done When 主路径——
//! HTTP 建项目（5 人编制）→ 建密钥（只回公钥）→ 绑仓库 → start_import →
//! 跑 Fake 分析到 draft_ready → apply 出 4 条提案且 `delivery.*` 零写 →
//! HTTP 4 次整批审批 → 导入 `applied`、项目 `planning`、需求 `draft`、任务有承接人、
//! 知识文档与 git commit、审计 `project_import_applied`；随后 AI PM 用同一套批量
//! 命令继续运营；全链路断言私钥零泄露。
//!
//! 偏差留痕：plan 字面「绑本地 `file://` 仓库」与 Task 3 的 URL 硬化（拒绝
//! `file://`）冲突，故仓库经 HTTP 以合法 URL 建行，再用 SQL 指向本地 `file://`
//! 并置 `verified` 以驱动 clone；`file://` 被拒与 HTTP verify 的正向契约由
//! `tests/repositories.rs` 覆盖，此处不再重复网络等待。

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_ai::repo_model::FakeRepoAnalysisModel;
use pm_api::routes::router;
use pm_application::command_executor::{
    ActorKind, CommandSource, ExecutionContext, PgCommandExecutor,
};
use pm_application::project_import::run_import_job;
use pm_application::PolicyEngine;
use pm_domain::import::{DocumentDraft, MilestoneDraft, RequirementDraft};
use pm_domain::proposal::Confidence;
use pm_domain::ProjectCommand;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

fn with_env() {
    std::env::set_var("AI_PM_SECRET_KEY", "e2e-secret-key-not-for-production");
    let cache = std::env::temp_dir().join(format!("ai-pm-e2e-cache-{}", Uuid::new_v4()));
    std::env::set_var("AI_PM_REPO_CACHE_ROOT", &cache);
    let knowledge = std::env::temp_dir().join(format!("ai-pm-e2e-knowledge-{}", Uuid::new_v4()));
    std::env::set_var("AI_PM_KNOWLEDGE_ROOT", &knowledge);
    std::env::set_var("AI_PM_REPO_ALLOWED_HOSTS", "gitlab.local");
}

fn knowledge_root() -> PathBuf {
    PathBuf::from(std::env::var("AI_PM_KNOWLEDGE_ROOT").unwrap())
}

struct LocalRepo {
    root: PathBuf,
    url: String,
    head: String,
}

impl Drop for LocalRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// 造一个真实的本地仓库：README + `docs/design.md` + 2 次提交（feat / fix）。
fn init_repo() -> LocalRepo {
    let root = std::env::temp_dir().join(format!("ai-pm-e2e-repo-{}", Uuid::new_v4()));
    let path = root.join("repo");
    std::fs::create_dir_all(&path).unwrap();
    let run = |args: &[&str]| {
        let status = Command::new("git")
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
    std::fs::write(path.join("README.md"), "# 世界地图编辑器\n\nTODO: docs\n").unwrap();
    std::fs::create_dir_all(path.join("docs")).unwrap();
    std::fs::write(path.join("docs/design.md"), "# 地形设计\n").unwrap();
    run(&["add", "-A"]);
    run(&[
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-q",
        "-m",
        "feat(core): terrain",
    ]);
    std::fs::write(path.join("docs/design.md"), "# 地形设计 v2\n").unwrap();
    run(&["add", "-A"]);
    run(&[
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-q",
        "-m",
        "fix: docs",
    ]);
    let head = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&path)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let url = format!("file://{}", path.to_string_lossy().replace('\\', "/"));
    LocalRepo { root, url, head }
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

fn keys_of(draft: &Value, section: &str) -> Vec<String> {
    draft[section]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item["key"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[sqlx::test(migrations = "../../migrations")]
async fn cold_start_import_runs_end_to_end(pool: PgPool) {
    with_env();
    let app = router(pool.clone());
    let mut leaked: Vec<String> = Vec::new();

    // 1. HTTP 建项目 → 201 + 恰 5 人编制。
    let (status, project) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/pm/projects",
                json!({ "code": "E2E-IMP", "name": "端到端导入", "description": "冷启动验收" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    leaked.push(project.to_string());
    assert_eq!(status, StatusCode::CREATED, "body: {project}");
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();
    let roles: Vec<String> = sqlx::query_scalar(
        "SELECT role FROM core.project_member WHERE project_id = $1 ORDER BY role",
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        roles.len(),
        5,
        "new project must start with a five-role org chart"
    );
    for role in ["owner", "ai_pm", "architect", "developer", "tester"] {
        assert!(roles.iter().any(|r| r == role), "missing role: {role}");
    }

    // 2. HTTP 建 SSH 密钥 → 200，只回公钥与指纹。
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
    leaked.push(key.to_string());
    assert_eq!(status, StatusCode::OK, "body: {key}");
    assert!(
        key["public_key"]
            .as_str()
            .unwrap()
            .starts_with("ssh-ed25519 "),
        "public key must be an ed25519 key"
    );
    assert!(
        key["fingerprint"].as_str().unwrap().starts_with("SHA256:"),
        "fingerprint must be SHA256"
    );
    assert!(
        key.get("private_key").is_none(),
        "the response must not carry a private key"
    );

    // 3. HTTP 绑仓库（合法 URL）→ 201，初始未 verified。
    let repo = init_repo();
    let (status, repository) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/repositories"),
                json!({
                    "name": "主仓库",
                    "url": "git@gitlab.local:team/world-map.git",
                    "role": "main",
                    "default_branch": "main",
                    "description": "世界地图客户端"
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    leaked.push(repository.to_string());
    assert_eq!(status, StatusCode::CREATED, "body: {repository}");
    assert_ne!(
        repository["status"],
        json!("verified"),
        "a freshly bound repository must not be verified before reachability check"
    );
    let repo_id = Uuid::parse_str(repository["id"].as_str().unwrap()).unwrap();

    // 4. 偏差：URL 硬化拒绝 file://（Task 3），故经 SQL 指向本地仓库并置 verified。
    sqlx::query(
        "UPDATE integration.project_repository \
         SET url = $2, status = 'verified', default_branch = 'main', head_commit_sha = $3 \
         WHERE id = $1",
    )
    .bind(repo_id)
    .bind(&repo.url)
    .bind(&repo.head)
    .execute(&pool)
    .await
    .unwrap();

    // 5. HTTP 启动导入 → 202 + queued（N-3：`repository_ids` 必填）。
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
    leaked.push(started.to_string());
    assert_eq!(status, StatusCode::ACCEPTED, "body: {started}");
    assert_eq!(started["status"], json!("queued"));
    let import_id = Uuid::parse_str(started["import_id"].as_str().unwrap()).unwrap();

    // 6. 跑 worker job（Fake 模型）→ draft_ready；GET 轮询拿到草案。
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
    leaked.push(detail.to_string());
    assert_eq!(status, StatusCode::OK, "body: {detail}");
    assert_eq!(detail["status"], json!("draft_ready"), "body: {detail}");
    assert!(
        !detail["draft"].is_null(),
        "draft must be present once ready"
    );
    assert!(detail["progress"].as_i64().unwrap() > 0);
    let revision = detail["draft_revision"].as_i64().unwrap();
    let draft = detail["draft"].clone();

    // 7. HTTP apply → outcome=proposed，恰 4 条提案，且 delivery.* 仍零写入。
    let (status, proposed) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/imports/{import_id}/apply"),
                json!({
                    "draft_revision": revision,
                    "project": { "code": "E2E-IMP", "name": "端到端导入", "description": "冷启动验收" },
                    "milestone_keys": keys_of(&draft, "milestones"),
                    "requirement_keys": keys_of(&draft, "requirements"),
                    "task_keys": keys_of(&draft, "tasks"),
                    "write_knowledge_documents": true
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    leaked.push(proposed.to_string());
    assert_eq!(status, StatusCode::OK, "body: {proposed}");
    assert_eq!(proposed["outcome"], json!("proposed"));
    let proposals = proposed["proposals"].as_array().unwrap();
    assert_eq!(
        proposals.len(),
        4,
        "one import yields four proposals: {proposed}"
    );

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
    let docs_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.project_document WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        docs_before, 0,
        "documents must not be written before approval"
    );

    // 8. HTTP 整批审批：4 次 decide(approved)。
    for proposal in proposals {
        let approval_id = proposal["approval_id"].as_str().unwrap();
        let (status, decided) = body_json(
            app.clone()
                .oneshot(json_request(
                    "POST",
                    &format!("/v1/pm/approvals/{approval_id}/decide"),
                    json!({ "decision": "approved", "reason": "端到端验收批准" }),
                ))
                .await
                .unwrap(),
        )
        .await;
        leaked.push(decided.to_string());
        assert_eq!(status, StatusCode::OK, "body: {decided}");
        assert_eq!(decided["status"], json!("approved"));
    }

    // 9. 断言落地：导入 applied、项目 planning。
    let import_status: String =
        sqlx::query_scalar("SELECT status FROM integration.repo_import WHERE id = $1")
            .bind(import_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(import_status, "applied");
    let project_status: String =
        sqlx::query_scalar("SELECT status FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(project_status, "planning");

    let expected_requirements = draft["requirements"].as_array().unwrap().len() as i64;
    let expected_milestones = draft["milestones"].as_array().unwrap().len() as i64;
    let requirement_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.requirement WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(requirement_count, expected_requirements);
    // 0039/S02D-04：requirement 级 draft 态废除——「不自动批准」的新等价断言
    // = 各需求 v1 修订仍为 draft（无伪造 approved）。
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
    let milestone_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.milestone WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(milestone_count, expected_milestones);
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

    // 知识文档：每需求一篇 + 项目总览一篇，且知识根确有 git 提交。
    let docs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.project_document WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(docs, expected_requirements + 1);
    let git_log = Command::new("git")
        .args(["log", "--oneline"])
        .current_dir(knowledge_root())
        .output()
        .unwrap();
    assert!(
        git_log.status.success(),
        "knowledge root must be a git repository"
    );
    assert!(
        String::from_utf8_lossy(&git_log.stdout).lines().count() >= docs as usize,
        "each document must land a git commit"
    );

    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND action = 'project_import_applied'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);

    // 10. 运营闭环：导入完成后 AI PM 用同一套批量命令继续建里程碑 / 需求 / 文档。
    let owner: Uuid = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member WHERE project_id = $1 AND role = 'owner'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let executor = PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()));
    let context = || ExecutionContext {
        project_id,
        actor: pm_application::actor::AuditActor::principal(owner),
        actor_kind: ActorKind::Human,
        source: CommandSource::RedlineApproval {
            approval_id: Uuid::new_v4(),
        },
    };
    executor
        .execute(
            ProjectCommand::CreateMilestones {
                milestones: vec![MilestoneDraft {
                    key: "M-ops".to_string(),
                    name: "导入后运营里程碑".to_string(),
                    planned_at: None,
                }],
            },
            context(),
        )
        .await
        .unwrap();
    executor
        .execute(
            ProjectCommand::CreateRequirements {
                requirements: vec![RequirementDraft {
                    key: "R-ops".to_string(),
                    code: "REQ-OPS-1".to_string(),
                    title: "导入后追加需求".to_string(),
                    markdown: "# 追加\n".to_string(),
                    priority: "normal".to_string(),
                    confidence: Confidence::High,
                    evidence: Vec::new(),
                }],
            },
            context(),
        )
        .await
        .unwrap();
    executor
        .execute(
            ProjectCommand::WriteDocuments {
                documents: vec![DocumentDraft {
                    doc_key: "OPS-DOC-1".to_string(),
                    doc_type: "knowledge".to_string(),
                    markdown: "# 运营手记\n".to_string(),
                    change_summary: Some("导入后补记".to_string()),
                }],
            },
            context(),
        )
        .await
        .unwrap();
    let after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.requirement WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        after,
        expected_requirements + 1,
        "the AI PM keeps operating with the same batch commands after import"
    );

    // 11. 私钥零泄露：所有 HTTP 响应体、草案 JSON、审计 after_data 与落库密文列。
    leaked.push(draft.to_string());
    let audits: Vec<Value> = sqlx::query_scalar(
        "SELECT after_data FROM governance.audit_log \
         WHERE project_id = $1 AND after_data IS NOT NULL",
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    for row in audits {
        leaked.push(row.to_string());
    }
    let ciphertext: Vec<u8> = sqlx::query_scalar(
        "SELECT private_key_ciphertext FROM integration.project_ssh_key WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let ciphertext_text = String::from_utf8_lossy(&ciphertext).into_owned();
    leaked.push(ciphertext_text.clone());

    for haystack in &leaked {
        assert!(
            !haystack.contains("PRIVATE KEY"),
            "a private key leaked into: {haystack}"
        );
        assert!(
            !haystack.contains("private_key"),
            "the private_key field leaked into: {haystack}"
        );
    }
    assert!(
        !ciphertext_text.contains("BEGIN"),
        "the stored secret must be sealed, not raw PEM"
    );

    let _ = std::fs::remove_dir_all(knowledge_root());
    let _ = repo;
}
