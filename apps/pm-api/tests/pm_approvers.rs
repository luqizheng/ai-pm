//! S02-I6-A Approver 显式授权端点与服务闸集成测试（迁移 0040 / O-6 裁决）：
//! Lead 授予 → 非 Lead 成员获得确认权 → 撤销回落 403；授予面守卫
//! （非 Lead 403 / 非成员 400 / 自授与授 Lead 400）；AI 主体恒拒。

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::actor::AuditActor;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::requirement::{self, ConfirmRevisionRequest, CreateRequirementRequest};
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

/// 第二成员（role='member'，非 Lead）。
async fn add_member(pool: &PgPool, project_id: Uuid) -> Uuid {
    let member_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES (gen_random_uuid(), 'human', 'member-wang', true) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'member')",
    )
    .bind(project_id)
    .bind(member_id)
    .execute(pool)
    .await
    .unwrap();
    member_id
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

/// 走到 pending_confirmation 的需求（Lead=dev-off 起草提交）。
async fn submitted(pool: &PgPool, project_id: Uuid, code: &str) -> (Uuid, Uuid, String) {
    let lead = pm_api::auth::DEV_OFF_PRINCIPAL;
    let actor = actor_of(lead);
    let entry = requirement::create_requirement(
        pool,
        pm_domain::SYSTEM_TENANT,
        project_id,
        CreateRequirementRequest {
            code: code.to_string(),
            title: "approver flow".to_string(),
            description: None,
            requirement_type: None,
            priority: None,
            markdown: None,
        },
        &actor,
    )
    .await
    .unwrap();
    // 创建即带 v1 草稿——直接取它提交，不再另造修订（确认后 current_version=1）。
    let detail = requirement::requirement_detail(pool, pm_domain::SYSTEM_TENANT, entry.id)
        .await
        .unwrap();
    let revision = &detail.revisions[0];
    let (revision_id, hash) = (revision.id, revision.content_hash.clone());
    requirement::submit_revision(
        pool,
        pm_domain::SYSTEM_TENANT,
        entry.id,
        revision_id,
        &actor,
        &common::knowledge_root(),
    )
        .await
        .unwrap();
    (entry.id, revision_id, hash)
}

#[sqlx::test(migrations = "../../migrations")]
async fn grant_confirms_and_revoke_revokes(pool: PgPool) {
    let project_id = seed(&pool, "APV-1").await;
    let member = add_member(&pool, project_id).await;
    let (requirement_id, revision_id, hash) = submitted(&pool, project_id, "REQ-1").await;

    // 撤销前：非 Lead 成员确认 → 403（无授权行）。
    let err = requirement::confirm_revision(
        &pool,
        pm_domain::SYSTEM_TENANT,
        requirement_id,
        revision_id,
        ConfirmRevisionRequest { expected_content_hash: hash.clone(), expected_current_revision_id: None, expected_row_revision: None },
        &actor_of(member),
    )
    .await
    .expect_err("non-lead member without grant must not confirm");
    assert!(matches!(err, pm_application::pm_write::PmWriteError::Forbidden(_)));

    // Lead 经 REST 授予 → 201；列表可见。
    let (status, body) = send(
        &pool,
        request("POST", &format!("/v1/pm/projects/{project_id}/approvers"),
            Some(serde_json::json!({ "principal_id": member }))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert_eq!(body["grant"]["display_name"], "member-wang");

    let (status, body) = send(&pool, request("GET", &format!("/v1/pm/projects/{project_id}/approvers"), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    // 授权后：成员可确认（Lead-or-Grant）。
    let entry = requirement::confirm_revision(
        &pool,
        pm_domain::SYSTEM_TENANT,
        requirement_id,
        revision_id,
        ConfirmRevisionRequest { expected_content_hash: hash, expected_current_revision_id: None, expected_row_revision: None },
        &actor_of(member),
    )
    .await
    .unwrap();
    assert_eq!(entry.current_version, Some(1));

    // Lead 撤销（REST）→ 成员治理权回落 403；重复撤销 404。
    let (status, _) = send(
        &pool,
        request("DELETE", &format!("/v1/pm/projects/{project_id}/approvers/{member}"), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (requirement_id2, revision_id2, hash2) = submitted(&pool, project_id, "REQ-2").await;
    let err = requirement::confirm_revision(
        &pool,
        pm_domain::SYSTEM_TENANT,
        requirement_id2,
        revision_id2,
        ConfirmRevisionRequest { expected_content_hash: hash2, expected_current_revision_id: None, expected_row_revision: None },
        &actor_of(member),
    )
    .await
    .expect_err("revoked approver must not confirm");
    assert!(matches!(err, pm_application::pm_write::PmWriteError::Forbidden(_)));

    let (status, _) = send(
        &pool,
        request("DELETE", &format!("/v1/pm/projects/{project_id}/approvers/{member}"), None),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "double revoke → 404");
}

#[sqlx::test(migrations = "../../migrations")]
async fn grant_guards(pool: PgPool) {
    let project_id = seed(&pool, "APV-2").await;
    let member = add_member(&pool, project_id).await;

    // 非 Lead 调授予 → 403（降级 dev-off 后再试）。
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
        request("POST", &format!("/v1/pm/projects/{project_id}/approvers"),
            Some(serde_json::json!({ "principal_id": member }))),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    sqlx::query(
        "UPDATE core.project_member SET role = 'owner' \
         WHERE project_id = $1 AND principal_id = $2",
    )
    .bind(project_id)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap();

    // 授予非成员 → 400。
    let outsider: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES (gen_random_uuid(), 'human', 'outsider', true) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (status, body) = send(
        &pool,
        request("POST", &format!("/v1/pm/projects/{project_id}/approvers"),
            Some(serde_json::json!({ "principal_id": outsider }))),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");

    // Lead 自授 → 400（Lead 已持权）。
    let (status, _) = send(
        &pool,
        request("POST", &format!("/v1/pm/projects/{project_id}/approvers"),
            Some(serde_json::json!({ "principal_id": pm_api::auth::DEV_OFF_PRINCIPAL }))),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // AI 主体（服务层）→ 恒 Forbidden（非成员 → 非 Lead → 无授权行）。
    let ai: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES (gen_random_uuid(), 'ai_pm', 'AI-PM', true) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let err = pm_application::approver::ensure_governance_authorized(&pool, project_id, ai)
        .await
        .expect_err("AI subject must not pass the governance gate");
    assert!(matches!(err, pm_application::pm_write::PmWriteError::Forbidden(_)));

    // 审计词表：grant/revoke 各一条（本用例只有 grant 失败路径——用 APV-1 的语义在
    // 这里补一条成功授予以验证审计；先授后撤由上一用例覆盖，此处只验 grant 行）。
    let (status, _) = send(
        &pool,
        request("POST", &format!("/v1/pm/projects/{project_id}/approvers"),
            Some(serde_json::json!({ "principal_id": member }))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM governance.audit_log WHERE project_id = $1 AND action LIKE 'approver.%'",
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(actions.iter().any(|a| a == "approver.granted"), "{actions:?}");
}
