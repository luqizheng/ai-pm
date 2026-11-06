//! S02-I3 Source & Traceability 集成测试（迁移 0044 / docs/s02-i3-source-traceability-implementation-v1.md）：
//! 来源登记/关联/解除全链 + Finding 支撑关联 + 跨项目 finding 404（negative_4 同语义）+
//! 重复关联 409 + AI 主体 403（§7.4 职责表）+ 非成员 404 + archived 需求拒绝关联 + 审计断言。

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::actor::AuditActor;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::pm_write::PmWriteError;
use pm_application::requirement::{self, CreateRequirementRequest};
use pm_application::requirement_traceability::{self, RegisterSourceInputRequest};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod common;

fn request(method: &str, uri: &str, body: Option<serde_json::Value>) -> Request<Body> {
    let builder = Request::builder().method(method).uri(uri);
    match body {
        Some(v) => builder
            .header("content-type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn send(pool: &PgPool, req: Request<Body>) -> (StatusCode, serde_json::Value) {
    let response = router(pool.clone()).oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

async fn seed(pool: &PgPool, code: &str) -> Uuid {
    let outcome = bootstrap_project(
        &pool,
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

/// 不把 dev-off 加入成员（模拟非成员上下文）。
async fn seed_without_member(pool: &PgPool, code: &str) -> Uuid {
    bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("Project {code}"),
            tenant_id: pm_domain::SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id
}

fn actor_of(principal_id: Uuid) -> AuditActor {
    AuditActor {
        principal_id,
        identity_id: None,
        client: Some("test".to_string()),
        auth_mode: Some("test".to_string()),
        claimed_device_id: None,
    }
}

async fn create_requirement(pool: &PgPool, project_id: Uuid, code: &str) -> Uuid {
    let actor = actor_of(pm_api::auth::DEV_OFF_PRINCIPAL);
    requirement::create_requirement(
        &pool,
        pm_domain::SYSTEM_TENANT,
        project_id,
        CreateRequirementRequest {
            code: code.to_string(),
            title: format!("Req {code}"),
            description: None,
            requirement_type: None,
            priority: None,
            markdown: Some(format!("# {code}")),
        },
        &actor,
    )
    .await
    .unwrap()
    .id
}

/// 经 REST 建 discovery finding（session → finding），返回 finding_id。
async fn create_finding_via_rest(pool: &PgPool, project_id: Uuid) -> Uuid {
    let (status, body) = send(
        &pool,
        request(
            "POST",
            &format!("/v1/pm/projects/{project_id}/discovery/sessions"),
            Some(serde_json::json!({"source": "WEB", "kind": "INITIAL_ONBOARDING"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let session_id = Uuid::parse_str(body["session"]["id"].as_str().unwrap()).unwrap();

    let (status, body) = send(
        &pool,
        request(
            "POST",
            &format!(
                "/v1/pm/projects/{project_id}/discovery/sessions/{session_id}/findings"
            ),
            Some(serde_json::json!({
                "kind": "observed",
                "content": "qa traceability finding",
                "evidence": [],
                "repository_ids": []
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

async fn audit_count(pool: &PgPool, action: &str, target_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE action = $1 AND target_id = $2",
    )
    .bind(action)
    .bind(target_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn t_source_flow_registers_links_and_lists(pool: PgPool) {
    let project_id = seed(&pool, "I3-A").await;
    let rid = create_requirement(&pool, project_id, "REQ-I3-1").await;

    let (status, body) = send(
        &pool,
        request(
            "POST",
            &format!("/v1/pm/projects/{project_id}/source-inputs"),
            Some(serde_json::json!({
                "input_kind": "human_input",
                "title": "原始用户请求",
                "content": "# 原始诉求\n\n登录能力",
                "source_ref": "ticket-42"
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let input_id = Uuid::parse_str(body["source_input"]["id"].as_str().unwrap()).unwrap();
    assert_eq!(audit_count(&pool, "source.registered", input_id).await, 1);

    let (status, body) = send(
        &pool,
        request(
            "POST",
            &format!("/v1/pm/requirements/{rid}/source-inputs"),
            Some(serde_json::json!({"input_id": input_id})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert_eq!(body["relation"], "source");
    assert_eq!(audit_count(&pool, "requirement.source_linked", rid).await, 1);

    // 重复关联 → 409
    let (status, _) = send(
        &pool,
        request(
            "POST",
            &format!("/v1/pm/requirements/{rid}/source-inputs"),
            Some(serde_json::json!({"input_id": input_id})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, body) = send(
        &pool,
        request("GET", &format!("/v1/pm/requirements/{rid}/source-inputs"), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["input_kind"], "human_input");

    // 解除 → 204 → 清单空
    let (status, _) = send(
        &pool,
        request(
            "DELETE",
            &format!("/v1/pm/requirements/{rid}/source-inputs/{input_id}"),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(audit_count(&pool, "requirement.source_unlinked", rid).await, 1);
    let (status, body) = send(
        &pool,
        request("GET", &format!("/v1/pm/requirements/{rid}/source-inputs"), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn t_finding_link_flow_and_audit(pool: PgPool) {
    let project_id = seed(&pool, "I3-B").await;
    let rid = create_requirement(&pool, project_id, "REQ-I3-2").await;
    let finding_id = create_finding_via_rest(&pool, project_id).await;

    let (status, body) = send(
        &pool,
        request(
            "POST",
            &format!("/v1/pm/requirements/{rid}/findings"),
            Some(serde_json::json!({"finding_id": finding_id})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert_eq!(body["relation"], "supports");
    assert_eq!(audit_count(&pool, "requirement.finding_linked", rid).await, 1);

    let (status, body) = send(
        &pool,
        request("GET", &format!("/v1/pm/requirements/{rid}/findings"), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["finding_id"], serde_json::json!(finding_id));
    assert_eq!(items[0]["kind"], "observed");

    let (status, _) = send(
        &pool,
        request(
            "DELETE",
            &format!("/v1/pm/requirements/{rid}/findings/{finding_id}"),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(audit_count(&pool, "requirement.finding_unlinked", rid).await, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn t_cross_project_finding_is_404(pool: PgPool) {
    let project_a = seed(&pool, "I3-C1").await;
    let project_b = seed(&pool, "I3-C2").await;
    let rid = create_requirement(&pool, project_a, "REQ-I3-3").await;
    let finding_b = create_finding_via_rest(&pool, project_b).await;

    let (status, _) = send(
        &pool,
        request(
            "POST",
            &format!("/v1/pm/requirements/{rid}/findings"),
            Some(serde_json::json!({"finding_id": finding_b})),
        ),
    )
    .await;
    // 跨项目 finding 按不存在处理（不泄露存在性）。
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn t_ai_principal_cannot_register_or_link(pool: PgPool) {
    let project_id = seed(&pool, "I3-D").await;
    let rid = create_requirement(&pool, project_id, "REQ-I3-4").await;
    let ai_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES (gen_random_uuid(), 'ai_pm', 'AI-PM', true) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let ai_actor = actor_of(ai_id);

    let err = requirement_traceability::register_source_input(
        &pool,
        pm_domain::SYSTEM_TENANT,
        project_id,
        RegisterSourceInputRequest {
            input_kind: "human_input".to_string(),
            title: "AI probe".to_string(),
            content: None,
            source_ref: None,
        },
        &ai_actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, PmWriteError::Forbidden(_)));

    let err = requirement_traceability::link_finding(
        &pool,
        pm_domain::SYSTEM_TENANT,
        rid,
        Uuid::new_v4(),
        &ai_actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, PmWriteError::Forbidden(_)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn t_non_member_source_inputs_404(pool: PgPool) {
    let outsider = seed(&pool, "I3-E1").await;
    let project_id = seed_without_member(&pool, "I3-E2").await;
    let rid = create_requirement(&pool, project_id, "REQ-I3-5").await;

    // dev-off 是 outsider 项目成员、不是 project_id 项目成员 → 需求端点 404。
    let (status, _) = send(
        &pool,
        request("GET", &format!("/v1/pm/requirements/{rid}/source-inputs"), None),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(
        &pool,
        request(
            "POST",
            &format!("/v1/pm/requirements/{rid}/findings"),
            Some(serde_json::json!({"finding_id": Uuid::new_v4()})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let _ = outsider;
}

#[sqlx::test(migrations = "../../migrations")]
async fn t_archived_requirement_rejects_link(pool: PgPool) {
    let project_id = seed(&pool, "I3-F").await;
    let rid = create_requirement(&pool, project_id, "REQ-I3-6").await;
    let actor = actor_of(pm_api::auth::DEV_OFF_PRINCIPAL);
    requirement::transition_lifecycle(
        &pool,
        pm_domain::SYSTEM_TENANT,
        rid,
        pm_application::requirement::LifecycleAction::Archive,
        &actor,
    )
    .await
    .unwrap();

    let err = requirement_traceability::link_source_input(
        &pool,
        pm_domain::SYSTEM_TENANT,
        rid,
        Uuid::new_v4(),
        &actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, PmWriteError::Conflict(_)));
}