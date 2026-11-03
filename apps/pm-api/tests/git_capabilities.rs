//! S01 · I3-B Git 能力授权验收（I3-001~006 段 + §11 管理面）。
//!
//! - I3-001：六能力枚举与依赖链由服务端校验（push 缺 commit → 422）
//! - I3-002：默认最小权限——无授权 = 全拒绝（含 read）
//! - I3-003：授予 / 替换 / 撤销 + 审计（granted / revoked 两类 action）
//! - I3-004：判定层依赖闭包（effective_capabilities_for）
//! - 管理面：owner / 租户 admin 可管，普通成员 404（不可见即不存在）
//! - 数据：capabilities CHECK 拒绝未知值（23514）

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::git_capability::effective_capabilities_for;
use pm_application::repository::{
    CreateRepositoryRequest, create_repository,
};
use pm_application::actor::AuditActor;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;
use pm_domain::SYSTEM_TENANT;

mod common;

fn request(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    builder.body(body).unwrap()
}

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn seed_world(pool: &PgPool, code: &str) -> (Uuid, Uuid, Uuid) {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(pool, boot.project_id).await;
    let repo = create_repository(
        pool,
        boot.project_id,
        &AuditActor::principal(boot.human_id),
        CreateRepositoryRequest {
            name: "主仓库".to_string(),
            url: "git@example.invalid:team/demo.git".to_string(),
            role: Some("main".to_string()),
            default_branch: Some("main".to_string()),
            description: None,
        },
    )
    .await
    .unwrap();
    (boot.project_id, repo.id, boot.human_id)
}

fn caps_uri(project_id: Uuid, repository_id: Uuid, principal_id: Uuid) -> String {
    format!(
        "/v1/pm/projects/{project_id}/repositories/{repository_id}/capabilities/{principal_id}"
    )
}

/// I3-003 + I3-004：授予全链 → 有效能力闭包正确；替换与撤销即时生效；落审计。
#[sqlx::test(migrations = "../../migrations")]
async fn grant_replace_revoke_with_closure_and_audit(pool: PgPool) {
    let (project_id, repo_id, owner_id) = seed_world(&pool, "GCAP-1").await;
    let app = router(pool.clone());
    let uri = caps_uri(project_id, repo_id, owner_id);

    // I3-001：push 缺 commit → 422（依赖链是服务端校验规则）。
    let (status, body) = body_json(
        app.clone()
            .oneshot(request(
                "PUT",
                &uri,
                Some(json!({ "capabilities": ["read", "modify", "push"] })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    assert_eq!(body["error"], json!("prerequisite_missing"));

    // 未识别值 → 422（§3.2 未识别即拒绝）。
    let (status, body) = body_json(
        app.clone()
            .oneshot(request("PUT", &uri, Some(json!({ "capabilities": ["write"] }))))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    assert_eq!(body["error"], json!("unknown_capability"));

    // 授予全链 → 200，判定层闭包输出全部能力。
    let (status, body) = body_json(
        app.clone()
            .oneshot(request(
                "PUT",
                &uri,
                Some(json!({ "capabilities": ["read", "modify", "commit", "push", "merge"] })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let granted = body["capabilities"].as_array().unwrap().len();
    assert_eq!(granted, 5);

    let effective =
        effective_capabilities_for(&pool, SYSTEM_TENANT, project_id, repo_id, owner_id)
            .await
            .unwrap();
    assert!(effective.contains(&pm_domain::GitCapability::Push));
    assert!(effective.contains(&pm_domain::GitCapability::Merge));

    // 替换（缩权到只读）→ 判定层立即只剩 read。
    let (status, _) = body_json(
        app.clone()
            .oneshot(request("PUT", &uri, Some(json!({ "capabilities": ["read"] }))))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effective =
        effective_capabilities_for(&pool, SYSTEM_TENANT, project_id, repo_id, owner_id)
            .await
            .unwrap();
    assert_eq!(effective.len(), 1);
    assert!(effective.contains(&pm_domain::GitCapability::Read));

    // 撤销 → 空集（全拒绝）。
    let (status, body) = body_json(
        app.clone()
            .oneshot(request("DELETE", &uri, None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let effective =
        effective_capabilities_for(&pool, SYSTEM_TENANT, project_id, repo_id, owner_id)
            .await
            .unwrap();
    assert!(effective.is_empty(), "revoke must deny everything");

    // 审计：granted 与 revoked 各有行。
    let granted_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE action = 'git_capability.granted'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(granted_audits, 2, "two PUT rounds");
    let revoked_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE action = 'git_capability.revoked'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revoked_audits, 1);
}

/// I3-002 默认最小权限：无授权行的主体 → 有效能力为空（含 read）。
#[sqlx::test(migrations = "../../migrations")]
async fn no_grant_denies_everything_including_read(pool: PgPool) {
    let (project_id, repo_id, _owner) = seed_world(&pool, "GCAP-2").await;
    let outsider: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', 'no-grant', true) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let effective =
        effective_capabilities_for(&pool, SYSTEM_TENANT, project_id, repo_id, outsider)
            .await
            .unwrap();
    assert!(effective.is_empty(), "default is deny-all");
}

/// 管理面判定（应用层）：owner / 租户 admin = true；普通成员与非成员 = false。
///
/// （HTTP 层负向在 off 模式不可测——dev-off 写请求物化为租户 owner，
/// `auth.rs:304-329`；管理面守卫的 HTTP 404 行为由 JWKS 模式测试覆盖口径。）
#[sqlx::test(migrations = "../../migrations")]
async fn manager_check_owner_yes_member_and_outsider_no(pool: PgPool) {
    let (project_id, _repo_id, owner_id) = seed_world(&pool, "GCAP-3").await;

    // owner → true。
    assert!(
        pm_application::project_member::is_project_manager(
            &pool, SYSTEM_TENANT, project_id, owner_id
        )
        .await
        .unwrap()
    );

    // 普通项目成员（非 owner）→ false。
    let member: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', 'gcap-member', true) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(SYSTEM_TENANT)
    .bind(member)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'member')",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(member)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        !pm_application::project_member::is_project_manager(
            &pool, SYSTEM_TENANT, project_id, member
        )
        .await
        .unwrap()
    );

    // 非成员 → false。
    let outsider: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', 'gcap-outsider', true) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !pm_application::project_member::is_project_manager(
            &pool, SYSTEM_TENANT, project_id, outsider
        )
        .await
        .unwrap()
    );
}

/// 数据：capabilities 数组出现未知值 → CHECK 违例（23514）。
#[sqlx::test(migrations = "../../migrations")]
async fn unknown_capability_value_violates_check(pool: PgPool) {
    let (project_id, repo_id, owner_id) = seed_world(&pool, "GCAP-4").await;
    let err = sqlx::query(
        "INSERT INTO integration.git_capability_grant \
         (tenant_id, project_id, repository_id, principal_id, capabilities) \
         VALUES ($1, $2, $3, $4, ARRAY['release']::text[])",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(repo_id)
    .bind(owner_id)
    .execute(&pool)
    .await;
    assert!(err.is_err(), "unknown capability must violate the CHECK");
    let code = err
        .unwrap_err()
        .as_database_error()
        .unwrap()
        .code()
        .expect("sqlstate")
        .to_string();
    assert_eq!(code, "23514");
}
