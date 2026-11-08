//! S02-I5 · 需求级验收端点集成测试（S02-04 V1.2 §11 Q-11~Q-14 回归面）：
//! criteria CRUD（治理授权=Lead 闸）、verification_kind 扩枚举校验（Q-13）、
//! params 形状校验、需求级裁决入口（Q-14）、确认漂移提示（Q-12）、
//! 非成员 404 / AI 主体 403（§7.2 A*）。

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router_with_feed_and_knowledge;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
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

fn delete(uri: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .body(Body::empty())
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
    let app = router_with_feed_and_knowledge(
        pool.clone(),
        pm_api::audit_stream::AuditFeed::new(),
        pm_api::mcp_hosts::McpHosts::default(),
        common::knowledge_root(),
    );
    let response = app.oneshot(request).await.unwrap();
    body_json(response).await
}

/// 种子：项目 + dev-off 成员（owner = Lead，治理授权默认满足）。
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

async fn create_requirement(pool: &PgPool, project_id: Uuid, code: &str) -> Uuid {
    let (status, body) = send(
        pool,
        post_json(
            &format!("/v1/pm/projects/{project_id}/requirements"),
            serde_json::json!({ "code": code, "title": "NFR 响应时间" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    body["requirement"]["id"].as_str().unwrap().parse().unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn criteria_crud_and_validation(pool: PgPool) {
    let project_id = seed(&pool, "REQ-ACC").await;
    let requirement_id = create_requirement(&pool, project_id, "REQ-1").await;
    let base = format!("/v1/pm/requirements/{requirement_id}/acceptance-criteria");

    // ── 空列表。
    let (status, body) = send(&pool, get(&base)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 0);

    // ── 新增：缺省 kind=manual；established_in_revision_id 留痕（无正式版本 = null）。
    let (status, body) = send(
        &pool,
        post_json(
            &base,
            serde_json::json!({ "statement": "P95 接口响应 ≤ 300ms" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let criterion = body["criterion"].clone();
    assert_eq!(criterion["verification_kind"], "manual");
    assert_eq!(criterion["verdict"], "pending");
    assert!(criterion["established_in_revision_id"].is_null());
    let first_id: Uuid = criterion["id"].as_str().unwrap().parse().unwrap();

    // ── 新增：扩枚举 performance + params JSONB（Q-13），position 递推。
    let (status, body) = send(
        &pool,
        post_json(
            &base,
            serde_json::json!({
                "statement": "压测 500 并发下无错误",
                "verification_kind": "performance",
                "params": { "metric": "error_rate", "threshold": 0, "load": 500 }
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert_eq!(body["criterion"]["position"], 2);
    let second_id: Uuid = body["criterion"]["id"].as_str().unwrap().parse().unwrap();

    // ── 非法 kind / 非法 params / 空 statement → 400。
    for invalid in [
        serde_json::json!({ "statement": "s", "verification_kind": "chaos" }),
        serde_json::json!({ "statement": "s", "params": [1, 2] }),
        serde_json::json!({ "statement": "   " }),
    ] {
        let (status, _) = send(&pool, post_json(&base, invalid)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    // ── 修改（Q-12：独立受控命令，不产生新修订）。
    let (status, body) = send(
        &pool,
        patch_json(
            &format!("{base}/{first_id}"),
            serde_json::json!({
                "statement": "P95 接口响应 ≤ 200ms",
                "verification_kind": "performance",
                "params": { "metric": "p95_ms", "threshold": 200 }
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["criterion"]["verification_kind"], "performance");
    assert_eq!(body["criterion"]["params"]["threshold"], 200);

    // ── 裁决入口（Q-14）：passed + note；再裁决覆盖为 failed。
    let (status, body) = send(
        &pool,
        post_json(
            &format!("{base}/{first_id}/verdict"),
            serde_json::json!({ "verdict": "passed", "note": "压测报告 PR-9" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["criterion"]["verdict"], "passed");
    assert!(body["criterion"]["verdict_by"].is_string());
    assert_eq!(body["criterion"]["verdict_note"], "压测报告 PR-9");

    let (status, body) = send(
        &pool,
        post_json(
            &format!("{base}/{first_id}/verdict"),
            serde_json::json!({ "verdict": "failed", "note": "回归复测未过" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["criterion"]["verdict"], "failed");

    // 非法裁决值 → 400。
    let (status, _) = send(
        &pool,
        post_json(
            &format!("{base}/{second_id}/verdict"),
            serde_json::json!({ "verdict": "maybe" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // ── 列表有序回显 + 移除。
    let (status, body) = send(&pool, get(&base)).await;
    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"].as_str().unwrap(), first_id.to_string());

    let (status, _) = send(&pool, delete(&format!("{base}/{second_id}"))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = send(&pool, get(&base)).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    // ── 不存在的 criterion → 404。
    let (status, _) = send(&pool, delete(&format!("{base}/{}", Uuid::new_v4()))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn non_member_governance_and_ai_gates(pool: PgPool) {
    // ── 非成员 404：项目 B 不入 dev-off 成员，需求用服务层种入（绕开 REST
    //    成员闸——成员闸归 REST 层，本用例锁 criteria 端点的同一闸）。
    let outsider = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "REQ-OUT2".to_string(),
            project_name: "Outsider 2".to_string(),
            tenant_id: pm_domain::SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;
    let actor = pm_application::actor::AuditActor::principal(pm_api::auth::DEV_OFF_PRINCIPAL);
    // principal 行须先在（created_by FK），但不入项目 B 成员——保持非成员语境。
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'dev-off', true) ON CONFLICT (id) DO NOTHING",
    )
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap();
    let entry = pm_application::requirement::create_requirement(
        &pool,
        pm_domain::SYSTEM_TENANT,
        outsider,
        pm_application::requirement::CreateRequirementRequest {
            code: "REQ-X".to_string(),
            title: "nope".to_string(),
            description: None,
            requirement_type: None,
            priority: None,
            markdown: None,
        },
        &actor,
    )
    .await
    .unwrap();
    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{}/acceptance-criteria", entry.id),
            serde_json::json!({ "statement": "绕过成员闸" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "非成员写 criteria 必须 404");

    // ── 成员但非治理授权 → 403（Lead OR 显式 Approver，复用 0040 闸）。
    let project_id = seed(&pool, "REQ-ACC2").await;
    let requirement_id = create_requirement(&pool, project_id, "REQ-1").await;
    let base = format!("/v1/pm/requirements/{requirement_id}/acceptance-criteria");
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
            &base,
            serde_json::json!({ "statement": "普通成员不得立验收" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "criteria 写须治理授权");
    sqlx::query(
        "UPDATE core.project_member SET role = 'owner' \
         WHERE project_id = $1 AND principal_id = $2",
    )
    .bind(project_id)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap();

    // ── AI 主体 → 403（§7.2 A*：服务面 require_human，与 confirm 同型）。
    let agent_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES (gen_random_uuid(), 'ai_pm', 'AI-PM', true) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let ai_actor = pm_application::actor::AuditActor {
        principal_id: agent_id,
        identity_id: None,
        client: Some("executor".to_string()),
        auth_mode: Some("executor".to_string()),
        claimed_device_id: None,
    };
    let err = pm_application::requirement_acceptance::add_criterion(
        &pool,
        pm_domain::SYSTEM_TENANT,
        requirement_id,
        pm_application::requirement_acceptance::AddCriterionRequest {
            statement: "AI 不得设立验收".to_string(),
            verification_kind: None,
            params: None,
            position: None,
        },
        &ai_actor,
    )
    .await
    .expect_err("AI principal must not establish criteria");
    assert!(matches!(
        err,
        pm_application::pm_write::PmWriteError::Forbidden(_)
    ));
}

#[sqlx::test(migrations = "../../migrations")]
async fn confirm_reports_criteria_drift(pool: PgPool) {
    let project_id = seed(&pool, "REQ-ACC3").await;
    let requirement_id = create_requirement(&pool, project_id, "REQ-1").await;
    let base = format!("/v1/pm/requirements/{requirement_id}/acceptance-criteria");

    // 先立验收项，再走 创建→提交。
    let (status, _) = send(
        &pool,
        post_json(&base, serde_json::json!({ "statement": "P95 ≤ 300ms" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (_, body) = send(&pool, get(&format!("/v1/pm/requirements/{requirement_id}"))).await;
    let revision_id: Uuid = body["revisions"][0]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let hash = body["revisions"][0]["content_hash"]
        .as_str()
        .unwrap()
        .to_string();
    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/submit"),
            serde_json::json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // ── 提交后未改验收项 → 确认无漂移提示。
    let (status, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/confirm"),
            serde_json::json!({ "expected_content_hash": hash }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["criteria_drift"], false);

    // ── 二轮：提交后再改验收项 → 确认响应 criteria_drift=true（Q-12 一致性提示）。
    let (_, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions"),
            serde_json::json!({ "markdown": "# updated\n- v2 body", "summary": "v2" }),
        ),
    )
    .await;
    assert_eq!(body["revision"]["version"], 2);
    let revision_v2: Uuid = body["revision"]["id"].as_str().unwrap().parse().unwrap();
    let hash_v2 = body["revision"]["content_hash"]
        .as_str()
        .unwrap()
        .to_string();
    let (status, _) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_v2}/submit"),
            serde_json::json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // 提交之后修改验收项 → 漂移。
    let (status, body) = send(&pool, get(&base)).await;
    assert_eq!(status, StatusCode::OK);
    let criterion_id: Uuid = body["items"][0]["id"].as_str().unwrap().parse().unwrap();
    let (status, _) = send(
        &pool,
        patch_json(
            &format!("{base}/{criterion_id}"),
            serde_json::json!({ "statement": "P95 ≤ 500ms（提交后放宽）" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/requirements/{requirement_id}/revisions/{revision_v2}/confirm"),
            serde_json::json!({ "expected_content_hash": hash_v2 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["criteria_drift"], true);
}
