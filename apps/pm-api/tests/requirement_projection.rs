//! S02-I2 需求投影协议集成测试（Markdown 权威正文，S02-04 V1.2 §6）：
//! submit 绑定（Git 提交 + document_revision + markdown_version_ref/hash 刷新）
//! → 按 ref 读 Git 一致 → rebuild_projection 重建投影 → 漂移显式拒绝 →
//! 绑定幂等（不产生重复 Git 提交）→ 未绑定 confirm 拒绝（六要素）。

use axum::body::{to_bytes, Body};
use axum::http::StatusCode;
use pm_api::audit_stream::AuditFeed;
use pm_api::mcp_hosts::McpHosts;
use pm_api::routes::router_with_feed_and_knowledge;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::knowledge::KnowledgeRoot;
use pm_application::requirement::{
    self, ConfirmRevisionRequest, CreateRequirementRequest, DraftRevisionRequest,
};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod common;

fn request(method: &str, uri: &str, body: Option<serde_json::Value>) -> axum::http::Request<Body> {
    let builder = axum::http::Request::builder().method(method).uri(uri);
    match body {
        Some(v) => builder
            .header("content-type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn send(
    pool: &PgPool,
    root: &KnowledgeRoot,
    req: axum::http::Request<Body>,
) -> (StatusCode, serde_json::Value) {
    let app = router_with_feed_and_knowledge(
        pool.clone(),
        AuditFeed::new(),
        McpHosts::default(),
        root.clone(),
    );
    let response = app.oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

fn actor_of(principal_id: Uuid) -> pm_application::actor::AuditActor {
    pm_application::actor::AuditActor {
        principal_id,
        identity_id: None,
        client: Some("test".to_string()),
        auth_mode: Some("test".to_string()),
        claimed_device_id: None,
    }
}

async fn seed(pool: &PgPool, code: &str) -> Uuid {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("Project {code}"),
            tenant_id: pm_domain::SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(pool, outcome.project_id).await;
    outcome.project_id
}

/// draft → submit（绑定）→ (requirement_id, revision_id, sha, markdown)
async fn submitted(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    code: &str,
    markdown: &str,
) -> (Uuid, Uuid, String, String) {
    let actor = actor_of(pm_api::auth::DEV_OFF_PRINCIPAL);
    let entry = requirement::create_requirement(
        pool,
        pm_domain::SYSTEM_TENANT,
        project_id,
        CreateRequirementRequest {
            code: code.to_string(),
            title: "projection".to_string(),
            description: None,
            requirement_type: None,
            priority: None,
            markdown: Some(markdown.to_string()),
        },
        &actor,
    )
    .await
    .unwrap();
    let revision = requirement::create_draft(
        pool,
        pm_domain::SYSTEM_TENANT,
        entry.id,
        DraftRevisionRequest {
            markdown: markdown.to_string(),
            summary: Some("投影验证".to_string()),
        },
        &actor,
    )
    .await
    .unwrap();
    requirement::submit_revision(
        pool,
        pm_domain::SYSTEM_TENANT,
        entry.id,
        revision.id,
        &actor,
        root,
    )
    .await
    .unwrap();
    let detail = requirement::requirement_detail(pool, pm_domain::SYSTEM_TENANT, entry.id)
        .await
        .unwrap();
    let rev = detail
        .revisions
        .iter()
        .find(|r| r.id == revision.id)
        .unwrap();
    let sha = rev.markdown_version_ref.clone().expect("submit must bind");
    (entry.id, revision.id, sha, markdown.to_string())
}

#[sqlx::test(migrations = "../../migrations")]
async fn submit_binds_projection_and_read_at_ref_matches(pool: PgPool) {
    let project_id = seed(&pool, "PRJ-1").await;
    let root = common::knowledge_root();
    let markdown = "# 投影验证\n\n- 登录流程权威正文";
    let (requirement_id, revision_id, sha, _) =
        submitted(&pool, &root, project_id, "REQ-1", markdown).await;

    // 绑定行：doc_key/doc_type/revision_no/哈希。
    let (doc_key, doc_type, rev_no, git_sha, hash): (String, String, i32, String, String) =
        sqlx::query_as(
            "SELECT d.doc_key, d.doc_type, r.revision_no, r.git_commit_sha, r.content_hash \
             FROM ai.project_document d \
             JOIN ai.document_revision r ON r.document_id = d.id \
             WHERE d.project_id = $1",
        )
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(doc_key, format!("req-{requirement_id}"));
    assert_eq!(doc_type, "requirement");
    assert_eq!(rev_no, 1);
    assert_eq!(git_sha, sha);

    // revision 绑定列 + Git 侧哈希与正文同源（S02D-16）。
    let (bound_ref, bound_hash): (String, String) = sqlx::query_as(
        "SELECT markdown_version_ref, content_hash FROM delivery.requirement_revision WHERE id = $1",
    )
    .bind(revision_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(bound_ref, sha);
    assert_eq!(
        bound_hash,
        pm_application::knowledge::content_hash(markdown)
    );

    // 按 ref 读 Git = 起草正文（§6.1 A=B）。
    let body = requirement_projection_read(&pool, &root, project_id, requirement_id, &sha).await;
    assert_eq!(body, markdown);
}

async fn requirement_projection_read(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    requirement_id: Uuid,
    sha: &str,
) -> String {
    pm_application::requirement_projection::read_at_ref(pool, root, project_id, requirement_id, sha)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn rebuild_projection_restores_content_and_detects_drift(pool: PgPool) {
    let project_id = seed(&pool, "PRJ-2").await;
    let root = common::knowledge_root();
    let markdown = "# 漂移检测\n\n- 正文权威在 Git";
    let (requirement_id, revision_id, sha, _) =
        submitted(&pool, &root, project_id, "REQ-1", markdown).await;

    // 破坏投影列（模拟投影损坏）→ rebuild 从 Git 恢复（summary 穿透保留）。
    sqlx::query("UPDATE delivery.requirement_revision SET content = jsonb_set(content, '{markdown}', to_jsonb('corrupted'::text)) WHERE id = $1")
        .bind(revision_id)
        .execute(&pool)
        .await
        .unwrap();
    let content = pm_application::requirement_projection::rebuild_projection(
        &pool,
        &root,
        requirement_id,
        revision_id,
    )
    .await
    .unwrap();
    assert_eq!(content["markdown"], serde_json::json!(markdown));
    assert_eq!(content["summary"], serde_json::json!("投影验证"));
    let restored: String = sqlx::query_scalar(
        "SELECT content->>'markdown' FROM delivery.requirement_revision WHERE id = $1",
    )
    .bind(revision_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(restored, markdown);

    // 漂移：账面哈希被篡改 → rebuild 显式拒绝（A=B 不变量，§6.1）。
    sqlx::query("UPDATE delivery.requirement_revision SET content_hash = 'deadbeef' WHERE id = $1")
        .bind(revision_id)
        .execute(&pool)
        .await
        .unwrap();
    let err = pm_application::requirement_projection::rebuild_projection(
        &pool,
        &root,
        requirement_id,
        revision_id,
    )
    .await
    .expect_err("hash drift must be observable");
    assert!(matches!(
        err,
        pm_application::pm_write::PmWriteError::Conflict(_)
    ));
    let _ = sha;
}

#[sqlx::test(migrations = "../../migrations")]
async fn bind_is_idempotent_and_unbound_confirm_is_refused(pool: PgPool) {
    let project_id = seed(&pool, "PRJ-3").await;
    let root = common::knowledge_root();

    // 未绑定（导入形态的 draft）→ confirm 拒绝（六要素，I2 §6.4-2）。
    let actor = actor_of(pm_api::auth::DEV_OFF_PRINCIPAL);
    let entry = requirement::create_requirement(
        &pool,
        pm_domain::SYSTEM_TENANT,
        project_id,
        CreateRequirementRequest {
            code: "REQ-RAW".to_string(),
            title: "no submit".to_string(),
            description: None,
            requirement_type: None,
            priority: None,
            markdown: None,
        },
        &actor,
    )
    .await
    .unwrap();
    let draft = requirement::create_draft(
        &pool,
        pm_domain::SYSTEM_TENANT,
        entry.id,
        DraftRevisionRequest {
            markdown: "# raw".to_string(),
            summary: None,
        },
        &actor,
    )
    .await
    .unwrap();
    // draft 状态下 confirm → 非 pending 409（已有语义）；此处构造 pending-未绑定：
    // 直接置状态绕过 submit（模拟 0039 之前的存量行/崩溃窗口）。
    sqlx::query(
        "UPDATE delivery.requirement_revision SET state = 'pending_confirmation', submitted_at = now() \
         WHERE id = $1",
    )
    .bind(draft.id)
    .execute(&pool)
    .await
    .unwrap();
    let hash = draft.content_hash.clone();
    let err = requirement::confirm_revision(
        &pool,
        pm_domain::SYSTEM_TENANT,
        entry.id,
        draft.id,
        ConfirmRevisionRequest {
            expected_content_hash: hash,
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &actor,
    )
    .await
    .expect_err("unbound revision must not confirm");
    assert!(matches!(
        err,
        pm_application::pm_write::PmWriteError::Conflict(_)
    ));

    // 绑定幂等：同一修订重复 bind → 同一 sha、Git 提交数不增长。
    let (requirement_id, revision_id, sha, markdown) =
        submitted(&pool, &root, project_id, "REQ-1", "# 幂等绑定").await;
    let commits_before = commit_count(&root);
    let sha2 = pm_application::requirement_projection::bind_projection(
        &pool,
        &root,
        project_id,
        requirement_id,
        revision_id,
        &markdown,
        actor_of(pm_api::auth::DEV_OFF_PRINCIPAL).principal_id,
    )
    .await
    .unwrap();
    assert_eq!(sha, sha2);
    assert_eq!(
        commit_count(&root),
        commits_before,
        "rebind must not create a commit"
    );

    // 已绑定 + pending 重入 submit → 409（不可重复提交）。
    let e2 = requirement::create_requirement(
        &pool,
        pm_domain::SYSTEM_TENANT,
        project_id,
        CreateRequirementRequest {
            code: "REQ-2".to_string(),
            title: "reentry".to_string(),
            description: None,
            requirement_type: None,
            priority: None,
            markdown: None,
        },
        &actor,
    )
    .await
    .unwrap();
    let r2 = requirement::create_draft(
        &pool,
        pm_domain::SYSTEM_TENANT,
        e2.id,
        DraftRevisionRequest {
            markdown: "# reentry".to_string(),
            summary: None,
        },
        &actor,
    )
    .await
    .unwrap();
    requirement::submit_revision(&pool, pm_domain::SYSTEM_TENANT, e2.id, r2.id, &actor, &root)
        .await
        .unwrap();
    let err =
        requirement::submit_revision(&pool, pm_domain::SYSTEM_TENANT, e2.id, r2.id, &actor, &root)
            .await
            .expect_err("bound pending revision must not re-submit");
    assert!(matches!(
        err,
        pm_application::pm_write::PmWriteError::Conflict(_)
    ));
}

fn commit_count(root: &KnowledgeRoot) -> usize {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root.path())
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap()
}
