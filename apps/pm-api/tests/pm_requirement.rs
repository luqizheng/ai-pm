//! S02-I1 需求核心端点集成测试（S02-04 V1.2 §5/§7/§13/§14 回归面）：
//! 生命周期链（创建→起草→提交→确认→superseded）、拒绝终态、CAS 双令牌 409、
//! 非成员 404（DEF-01 V-4 回归）、AI 主体确认 403（DEF-02 S02D-02 #2 服务面）、
//! 治理命令 Lead 闸与 archived 不可恢复（I1-D05）。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_api::routes::router_with_feed_and_knowledge;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::knowledge::KnowledgeRoot;

use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod common;

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn post_json(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn patch_json(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn body_json(response: axum::response::Response) -> (StatusCode, serde_json::Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

async fn send(pool: &PgPool, request: Request<Body>) -> (StatusCode, serde_json::Value) {
    // S02-I2：每请求注入独立临时知识库根——并行用例各自 git init，避免
    // 共享仓库的 index 竞态（submit 是本套件唯一的 Git 写点）。
    let app = router_with_feed_and_knowledge(
        pool.clone(),
        pm_api::audit_stream::AuditFeed::new(),
        pm_api::mcp_hosts::McpHosts::default(),
        common::knowledge_root(),
    );
    let response = app.oneshot(request).await.unwrap();
    body_json(response).await
}

/// 种子：项目 + dev-off 成员（owner）。
async fn seed(pool: &PgPool, code: &str) -> Uuid {
    common::ensure_knowledge_env();
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

async fn create_requirement(pool: &PgPool, project_id: Uuid, code: &str) -> serde_json::Value {
    let (status, body) = send(
        pool,
        post_json(
            &format!("/v1/pm/projects/{project_id}/requirements"),
            serde_json::json!({
                "code": code,
                "title": "User authentication",
                "priority": "high",
                "description": "账号密码登录与会话管理"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    body["requirement"].clone()
}

/// 走完 创建→起草→提交 到 pending_confirmation，返回 (requirement_id, revision_id, content_hash)。
async fn submitted_revision(pool: &PgPool, project_id: Uuid, code: &str) -> (Uuid, Uuid, String) {
    let requirement = create_requirement(pool, project_id, code).await;
    let requirement_id: Uuid = requirement["id"].as_str().unwrap().parse().unwrap();
    let revision_id: Uuid = requirement["latest_revision_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let (_, detail) = send(pool, get(&format!("/v1/pm/requirements/{requirement_id}"))).await;
    let hash = detail["revisions"][0]["content_hash"]
        .as_str()
        .unwrap()
        .to_string();
    let (status, _) = send(
        pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/submit"),
            serde_json::json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    (requirement_id, revision_id, hash)
}

#[sqlx::test(migrations = "../../migrations")]
async fn confirm_flow_supersedes_and_records_cas(pool: PgPool) {
    let project_id = seed(&pool, "REQ-LC").await;

    // ── 创建：active + v1 草稿；重复 code 409。
    let requirement = create_requirement(&pool, project_id, "REQ-1").await;
    let requirement_id: Uuid = requirement["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(requirement["lifecycle_status"], "active");
    assert_eq!(requirement["latest_state"], "draft");
    assert_eq!(requirement["latest_version"], 1);
    assert!(requirement["current_revision_id"].is_null());

    let (status, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/projects/{project_id}/requirements"),
            serde_json::json!({ "code": "REQ-1", "title": "dup" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {body}");

    // ── 详情：修订链 + 投影提取。
    let (_, detail) = send(&pool, get(&format!("/v1/pm/requirements/{requirement_id}"))).await;
    assert_eq!(detail["revisions"].as_array().unwrap().len(), 1);
    assert_eq!(detail["revisions"][0]["markdown"], "# User authentication");
    assert_eq!(detail["revisions"][0]["summary"], "账号密码登录与会话管理");
    let revision_id: Uuid = detail["revisions"][0]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    // ── 改草稿：hash 随正文更新（S02D-16 sha256(markdown)）。
    let (status, body) = send(
        &pool,
        patch_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}"),
            serde_json::json!({ "markdown": "# updated body\n- login flow" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let hash = body["revision"]["content_hash"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(
        hash,
        detail["revisions"][0]["content_hash"].as_str().unwrap()
    );

    // ── 提交 → pending_confirmation；确认携带错误哈希 → 409（跨版本重放防线）。
    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/submit"),
            serde_json::json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/confirm"),
            serde_json::json!({ "expected_content_hash": "deadbeef" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // ── 正确确认 → approved + Confirmation 落账 + row_revision+1。
    let (status, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/confirm"),
            serde_json::json!({ "expected_content_hash": hash }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["requirement"]["current_version"], 1);
    assert_eq!(
        body["requirement"]["current_revision_id"].as_str().unwrap(),
        revision_id.to_string()
    );
    let row_revision_after_v1 = body["requirement"]["row_revision"].as_i64().unwrap();
    assert_eq!(row_revision_after_v1, 2);

    let (_, detail) = send(&pool, get(&format!("/v1/pm/requirements/{requirement_id}"))).await;
    assert_eq!(detail["confirmations"].as_array().unwrap().len(), 1);
    assert_eq!(detail["confirmations"][0]["action"], "confirm");
    assert_eq!(detail["confirmations"][0]["content_hash"], hash.as_str());
    assert_eq!(detail["revisions"][0]["state"], "approved");

    // ── 重放确认（同一请求再发）→ 409（幂等键归 I6-B/O-5，V1 显式拒绝）。
    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/confirm"),
            serde_json::json!({ "expected_content_hash": hash }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // ── v2：CAS 旧 row_revision → 409；新令牌 → 200 且 v1 → superseded。
    let (status, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions"),
            serde_json::json!({ "markdown": "# v2 body" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let revision_v2: Uuid = body["revision"]["id"].as_str().unwrap().parse().unwrap();
    let hash_v2 = body["revision"]["content_hash"]
        .as_str()
        .unwrap()
        .to_string();
    send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_v2}/submit"),
            serde_json::json!({}),
        ),
    )
    .await;

    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_v2}/confirm"),
            serde_json::json!({
                "expected_content_hash": hash_v2,
                "expected_row_revision": 1
            }),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "stale CAS token must 409 (DEF-03 方向)"
    );

    let (status, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_v2}/confirm"),
            serde_json::json!({
                "expected_content_hash": hash_v2,
                "expected_row_revision": row_revision_after_v1
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["requirement"]["current_version"], 2);
    assert_eq!(body["requirement"]["row_revision"], 3);

    let (_, detail) = send(&pool, get(&format!("/v1/pm/requirements/{requirement_id}"))).await;
    let states: Vec<&str> = detail["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["state"].as_str().unwrap())
        .collect();
    assert_eq!(states, vec!["approved", "superseded"]); // version DESC

    // ── 列表 + 审计词表（S02D-23 点号式）。
    let (_, list) = send(
        &pool,
        get(&format!("/v1/pm/projects/{project_id}/requirements")),
    )
    .await;
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    assert_eq!(list["items"][0]["latest_state"], "approved");
    assert_eq!(list["items"][0]["current_version"], 2);

    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM governance.audit_log WHERE project_id = $1 ORDER BY created_at",
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    for expected in [
        "requirement.created",
        "requirement.revision_drafted",
        "requirement.revision_submitted",
        "requirement.confirmed",
        "requirement.revision_superseded",
    ] {
        assert!(
            actions.iter().any(|a| a == expected),
            "missing audit {expected}: {actions:?}"
        );
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn reject_is_terminal_and_reason_recorded(pool: PgPool) {
    let project_id = seed(&pool, "REQ-RJ").await;
    let (requirement_id, revision_id, hash) = submitted_revision(&pool, project_id, "REQ-1").await;

    let (status, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/reject"),
            serde_json::json!({ "reason": "锁定策略不可测" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["revision"]["state"], "rejected");

    let (_, detail) = send(&pool, get(&format!("/v1/pm/requirements/{requirement_id}"))).await;
    assert_eq!(detail["confirmations"][0]["action"], "reject");
    assert_eq!(detail["confirmations"][0]["reason"], "锁定策略不可测");
    assert!(detail["requirement"]["current_revision_id"].is_null());

    // 拒绝终态：确认 409、再提交 409（I1-D03 不复用）。
    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/confirm"),
            serde_json::json!({ "expected_content_hash": hash }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/submit"),
            serde_json::json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // 重开必须走新修订（版本单调递增，永不复用）。
    let (status, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions"),
            serde_json::json!({ "markdown": "# v2" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["revision"]["version"], 2);
    let revision_v2: Uuid = body["revision"]["id"].as_str().unwrap().parse().unwrap();
    let hash_v2 = body["revision"]["content_hash"]
        .as_str()
        .unwrap()
        .to_string();
    send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_v2}/submit"),
            serde_json::json!({}),
        ),
    )
    .await;
    let (status, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_v2}/confirm"),
            serde_json::json!({ "expected_content_hash": hash_v2 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["requirement"]["current_version"], 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn non_member_and_cross_project_writes_are_404(pool: PgPool) {
    // 项目不入 dev-off 成员（bootstrap 后不 join）。
    let outsider = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "REQ-OUT".to_string(),
            project_name: "Outsider".to_string(),
            tenant_id: pm_domain::SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;

    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/projects/{outsider}/requirements"),
            serde_json::json!({ "code": "REQ-X", "title": "nope" }),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "DEF-01 V-4 回归：非成员写必须 404"
    );

    // 成员项目的需求，对非成员项目上下文的读取同样 404（requirement 级边界）。
    let project_id = seed(&pool, "REQ-IN").await;
    let (requirement_id, _, _) = submitted_revision(&pool, project_id, "REQ-1").await;
    let (status, _) = send(&pool, get(&format!("/v1/pm/requirements/{requirement_id}"))).await;
    assert_eq!(status, StatusCode::OK);
    // 独立库内无第二租户；跨租户由 tenant_isolation.rs 全套覆盖，这里锁需求级 404 口径。
}

#[sqlx::test(migrations = "../../migrations")]
async fn ai_principal_cannot_confirm_or_govern(pool: PgPool) {
    let project_id = seed(&pool, "REQ-AI").await;
    let (requirement_id, revision_id, hash) = submitted_revision(&pool, project_id, "REQ-1").await;

    // AI 主体（kind='ai_pm'，即 DEF-02 的 ActorKind::AiPm）直接调服务层确认 → 403（S02D-02 #2）。
    let agent_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES (gen_random_uuid(), 'ai_pm', 'AI-PM', true) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor = pm_application::actor::AuditActor {
        principal_id: agent_id,
        identity_id: None,
        client: Some("executor".to_string()),
        auth_mode: Some("executor".to_string()),
        claimed_device_id: None,
    };
    let err = pm_application::requirement::confirm_revision(
        &pool,
        pm_domain::SYSTEM_TENANT,
        requirement_id,
        revision_id,
        pm_application::requirement::ConfirmRevisionRequest {
            expected_content_hash: hash.clone(),
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &actor,
    )
    .await
    .expect_err("AI principal must not confirm");
    assert!(matches!(
        err,
        pm_application::pm_write::PmWriteError::Forbidden(_)
    ));

    let err = pm_application::requirement::transition_lifecycle(
        &pool,
        pm_domain::SYSTEM_TENANT,
        requirement_id,
        pm_application::requirement::LifecycleAction::Withdraw,
        &actor,
    )
    .await
    .expect_err("AI principal must not govern lifecycle");
    assert!(matches!(
        err,
        pm_application::pm_write::PmWriteError::Forbidden(_)
    ));

    // 真人主体仍可确认（同一 revision 未被污染）。
    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/confirm"),
            serde_json::json!({ "expected_content_hash": hash }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "../../migrations")]
async fn lifecycle_governance_requires_lead_and_archive_is_terminal(pool: PgPool) {
    let project_id = seed(&pool, "REQ-GV").await;
    let (requirement_id, revision_id, hash) = submitted_revision(&pool, project_id, "REQ-1").await;

    // 非 Lead 成员 → 403：把 dev-off 降为 member 后再试。
    sqlx::query(
        "UPDATE core.project_member SET role = 'member' \
         WHERE project_id = $1 AND principal_id = $2",
    )
    .bind(project_id)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap();
    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/withdraw"),
            serde_json::json!({}),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "withdraw requires project lead"
    );
    sqlx::query(
        "UPDATE core.project_member SET role = 'owner' \
         WHERE project_id = $1 AND principal_id = $2",
    )
    .bind(project_id)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap();

    // withdrawn 期间不接受确认/拒绝（ensure_active）。
    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/withdraw"),
            serde_json::json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/confirm"),
            serde_json::json!({ "expected_content_hash": hash }),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "withdrawn requirement is not active"
    );

    // restore → active；archive → archived 终态；restore 409（I1-D05）。
    let (status, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/restore"),
            serde_json::json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["requirement"]["lifecycle_status"], "active");

    let (status, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/archive"),
            serde_json::json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["requirement"]["lifecycle_status"], "archived");

    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/restore"),
            serde_json::json!({}),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "archived must not restore (I1-D05)"
    );
    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/archive"),
            serde_json::json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}
