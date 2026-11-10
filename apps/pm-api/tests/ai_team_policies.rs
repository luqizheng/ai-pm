//! Execution Policies REST 契约（UI-3，U-2=A；零迁移，0013/0010 既有表）。
//!
//! 覆盖：读面结构（空态默认值 + 四层全有数据的映射）、PUT project upsert /
//! 清除（null）/ 非管理者 403、PUT 任务覆盖 upsert / DELETE / 全 null 400 /
//! 非管理者 403、审计行落库断言（action + after_data 快照）、组合规则文档
//! 字段存在（defaults 照 policy_composition 代码事实）。
//!
//! 夹具沿用 `ai_roles.rs` 的种子 harness（tenant external_key = JWT tid）。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::auth::AuthConfig;
use pm_api::routes::router_with_auth_config;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

const PRIVATE_PEM: &str = include_str!("keys/private.pem");
const PUBLIC_PEM: &str = include_str!("keys/public.pem");

fn test_jwks() -> Arc<JWKSCache> {
    Arc::new(JWKSCache::from_pem(PUBLIC_PEM).expect("test public key must parse"))
}

fn token_for(sub: &str, tid: i64) -> String {
    sign_token(
        PRIVATE_PEM,
        &json!({
            "sub": sub, "tid": tid, "username": sub, "roles": [],
            "permissions": [], "aud": "coder-micro-services",
        }),
        // 1h：本机 sqlx::test 迁移慢，长链路用例会超过 300s 默认（ai_roles 同款）。
        Duration::from_secs(3600),
    )
    .expect("test token must sign")
}

fn request(method: &str, uri: &str, token: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"));
    let body = match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    builder.body(body).unwrap()
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(request(method, uri, token, body))
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn app(pool: PgPool) -> Router {
    router_with_auth_config(pool, test_jwks(), AuthConfig::default())
}

async fn seed_tenant(pool: &PgPool, external_key: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', $1, $1) RETURNING id",
    )
    .bind(external_key)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// principal + identity + tenant_member 一条龙（role = 租户角色）。
async fn seed_tenant_member(pool: &PgPool, tenant_id: Uuid, sub: &str, role: &str) -> Uuid {
    let principal: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', $1, true) RETURNING id",
    )
    .bind(sub)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal_identity (provider, tenant_id, subject, principal_id) \
         VALUES ('lmms', $1, $2, $3)",
    )
    .bind(tenant_id)
    .bind(sub)
    .bind(principal)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) \
         VALUES ($1, $2, $3)",
    )
    .bind(tenant_id)
    .bind(principal)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    principal
}

async fn seed_project_member(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    role: &str,
) {
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
}

/// 手工造项目（策略读写不依赖 bootstrap 种子链）。
async fn seed_project(pool: &PgPool, tenant_id: Uuid, code: &str) -> Uuid {
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, 'c') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, $3, $3, 'planning') RETURNING id",
    )
    .bind(tenant_id)
    .bind(customer_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_task(pool: &PgPool, tenant_id: Uuid, project_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, tenant_id, title) \
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(title)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn policies_of(app: &Router, token: &str, project_id: Uuid) -> Value {
    let (status, body) = call(
        app,
        "GET",
        &format!("/v1/pm/projects/{project_id}/ai-team/execution-policies"),
        token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn audit_rows(pool: &PgPool, project_id: Uuid, action: &str) -> Vec<(Value, Uuid)> {
    sqlx::query_as::<_, (serde_json::Value, Uuid)>(
        "SELECT after_data, actor_id FROM governance.audit_log \
          WHERE project_id = $1 AND action = $2 ORDER BY created_at, id",
    )
    .bind(project_id)
    .bind(action)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// 空态读面：org/project 行缺失 = null、task_types/overrides 空、composition
/// 默认值照 `policy_composition::compose` 代码事实（能力默认允许、审批默认
/// 不要求、额外校验空）。
#[sqlx::test(migrations = "../../migrations")]
async fn read_empty_state_composition_defaults(pool: PgPool) {
    let tenant_id = seed_tenant(&pool, "61").await;
    let owner = seed_tenant_member(&pool, tenant_id, "usr-owner", "owner").await;
    let project_id = seed_project(&pool, tenant_id, "EP-1").await;
    seed_project_member(&pool, tenant_id, project_id, owner, "owner").await;

    let a = app(pool.clone());
    let token = token_for("usr-owner", 61);
    let view = policies_of(&a, &token, project_id).await;

    assert_eq!(view["organization"], Value::Null, "{view}");
    assert_eq!(view["project"], Value::Null);
    assert_eq!(view["taskTypes"], json!([]));
    assert_eq!(view["taskOverrides"], json!([]));

    let composition = &view["composition"];
    assert_eq!(
        composition["layer_order"],
        json!(["organization", "project", "task_type", "task"])
    );
    assert_eq!(composition["defaults"]["auto_accept"], true);
    assert_eq!(composition["defaults"]["auto_merge"], true);
    assert_eq!(composition["defaults"]["auto_publish_knowledge"], true);
    assert_eq!(composition["defaults"]["require_human_approval"], false);
    assert_eq!(composition["defaults"]["additional_checks"], json!([]));
    let rules = composition["rules"].as_array().unwrap();
    assert!(rules.len() >= 4, "composition rules must exist: {rules:?}");
    assert!(
        rules.iter().any(|r| r["key"] == "constraints"),
        "constraints rule (高层强制审批低层不得放宽) must be present"
    );
}

/// 全层数据读面：org 行 / project 行 / task_type 行（全局口径）/ 任务覆盖行
/// （JOIN 出 title、NULL 字段保持 null）各归各位。
#[sqlx::test(migrations = "../../migrations")]
async fn read_with_all_layers_maps_rows(pool: PgPool) {
    let tenant_id = seed_tenant(&pool, "62").await;
    let owner = seed_tenant_member(&pool, tenant_id, "usr-owner", "owner").await;
    let project_id = seed_project(&pool, tenant_id, "EP-2").await;
    seed_project_member(&pool, tenant_id, project_id, owner, "owner").await;

    sqlx::query(
        "INSERT INTO delivery.policy_scope \
             (tenant_id, scope, scope_key, require_human_approval, updated_by) \
         VALUES ($1, 'organization', '*', true, $2), \
                 ($1, 'project', $3, false, $2), \
                 ($1, 'task_type', 'backend', true, $2)",
    )
    .bind(tenant_id)
    .bind(owner)
    .bind(project_id.to_string())
    .execute(&pool)
    .await
    .unwrap();
    let task_id = seed_task(&pool, tenant_id, project_id, "Implement login").await;
    sqlx::query(
        "INSERT INTO delivery.task_policy_override \
             (task_id, project_id, tenant_id, auto_accept, require_human_approval, updated_by) \
         VALUES ($1, $2, $3, false, true, $4)",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(tenant_id)
    .bind(owner)
    .execute(&pool)
    .await
    .unwrap();

    let a = app(pool.clone());
    let token = token_for("usr-owner", 62);
    let view = policies_of(&a, &token, project_id).await;

    assert_eq!(
        view["organization"]["require_human_approval"], true,
        "{view}"
    );
    assert_eq!(view["project"]["require_human_approval"], false);
    assert_eq!(view["project"]["updated_by"], json!(owner));
    let task_types = view["taskTypes"].as_array().unwrap();
    assert_eq!(task_types.len(), 1, "{view}");
    assert_eq!(task_types[0]["task_type"], "backend");
    assert_eq!(task_types[0]["require_human_approval"], true);

    let overrides = view["taskOverrides"].as_array().unwrap();
    assert_eq!(overrides.len(), 1, "{view}");
    assert_eq!(overrides[0]["task_id"], json!(task_id));
    assert_eq!(overrides[0]["task_title"], "Implement login");
    assert_eq!(overrides[0]["require_human_approval"], true);
    assert_eq!(overrides[0]["auto_accept"], false);
    // 未写的字段 = NULL = 继承
    assert_eq!(overrides[0]["auto_merge"], Value::Null);
    assert_eq!(overrides[0]["auto_publish_knowledge"], Value::Null);
}

/// PUT project：upsert true → 行落库 + 审计快照；null → 删行恢复继承 +
/// 审计快照 null；响应回写后行状态（UI 回读语义）。
#[sqlx::test(migrations = "../../migrations")]
async fn put_project_upsert_clear_and_audit(pool: PgPool) {
    let tenant_id = seed_tenant(&pool, "63").await;
    let owner = seed_tenant_member(&pool, tenant_id, "usr-owner", "owner").await;
    let project_id = seed_project(&pool, tenant_id, "EP-3").await;
    seed_project_member(&pool, tenant_id, project_id, owner, "owner").await;

    let a = app(pool.clone());
    let token = token_for("usr-owner", 63);

    // upsert（行不存在 → 建行）
    let (status, body) = call(
        &a,
        "PUT",
        &format!("/v1/pm/projects/{project_id}/ai-team/execution-policies/project"),
        &token,
        Some(json!({ "require_human_approval": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["require_human_approval"], true, "{body}");
    let stored: bool = sqlx::query_scalar(
        "SELECT require_human_approval FROM delivery.policy_scope \
          WHERE scope = 'project' AND scope_key = $1",
    )
    .bind(project_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(stored);
    let audits = audit_rows(&pool, project_id, "execution_policy.project_updated").await;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].0["require_human_approval"], true);
    assert_eq!(audits[0].0["scope"], "project");
    assert_eq!(audits[0].1, owner, "updated_by = caller");

    // 覆盖写（行已存在 → 改值）
    let (status, body) = call(
        &a,
        "PUT",
        &format!("/v1/pm/projects/{project_id}/ai-team/execution-policies/project"),
        &token,
        Some(json!({ "require_human_approval": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["require_human_approval"], false);

    // null = 清除（删行恢复继承组织）
    let (status, body) = call(
        &a,
        "PUT",
        &format!("/v1/pm/projects/{project_id}/ai-team/execution-policies/project"),
        &token,
        Some(json!({ "require_human_approval": null })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, Value::Null, "cleared row reads back as null");
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.policy_scope \
          WHERE scope = 'project' AND scope_key = $1",
    )
    .bind(project_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);

    let audits = audit_rows(&pool, project_id, "execution_policy.project_updated").await;
    assert_eq!(audits.len(), 3, "upsert/overwrite/clear each audited");
    assert_eq!(audits[2].0["require_human_approval"], Value::Null);

    // GET 回读：project 层已恢复继承
    let view = policies_of(&a, &token, project_id).await;
    assert_eq!(view["project"], Value::Null, "{view}");
}

/// 写守卫：项目内普通成员（非 owner，非租户 admin）→ 403，不落行不落审计；
/// 跨租户项目 → 404（不泄露存在性）。
#[sqlx::test(migrations = "../../migrations")]
async fn put_project_forbidden_and_cross_tenant(pool: PgPool) {
    let tenant_id = seed_tenant(&pool, "64").await;
    let owner = seed_tenant_member(&pool, tenant_id, "usr-owner", "owner").await;
    let member = seed_tenant_member(&pool, tenant_id, "usr-member", "member").await;
    let project_id = seed_project(&pool, tenant_id, "EP-4").await;
    seed_project_member(&pool, tenant_id, project_id, owner, "owner").await;
    seed_project_member(&pool, tenant_id, project_id, member, "member").await;

    // 跨租户项目（tenant 65）
    let other_tenant = seed_tenant(&pool, "65").await;
    let other_project = seed_project(&pool, other_tenant, "EP-4X").await;

    let a = app(pool.clone());
    let member_token = token_for("usr-member", 64);

    let (status, body) = call(
        &a,
        "PUT",
        &format!("/v1/pm/projects/{project_id}/ai-team/execution-policies/project"),
        &member_token,
        Some(json!({ "require_human_approval": true })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    // 读仍允许（项目可见）
    let (status, _) = call(
        &a,
        "GET",
        &format!("/v1/pm/projects/{project_id}/ai-team/execution-policies"),
        &member_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let audits = audit_rows(&pool, project_id, "execution_policy.project_updated").await;
    assert!(audits.is_empty(), "403 must not audit");

    // 跨租户 → 404
    let (status, _) = call(
        &a,
        "PUT",
        &format!("/v1/pm/projects/{other_project}/ai-team/execution-policies/project"),
        &member_token,
        Some(json!({ "require_human_approval": true })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// 任务覆盖：upsert（至少一个非 null）→ 行落库 + 审计；DELETE → 删行 +
/// cleared 审计；全 null body → 400；普通成员 → 403。
#[sqlx::test(migrations = "../../migrations")]
async fn task_override_upsert_delete_forbidden_and_validation(pool: PgPool) {
    let tenant_id = seed_tenant(&pool, "66").await;
    let owner = seed_tenant_member(&pool, tenant_id, "usr-owner", "owner").await;
    let member = seed_tenant_member(&pool, tenant_id, "usr-member", "member").await;
    let project_id = seed_project(&pool, tenant_id, "EP-5").await;
    seed_project_member(&pool, tenant_id, project_id, owner, "owner").await;
    seed_project_member(&pool, tenant_id, project_id, member, "member").await;
    let task_id = seed_task(&pool, tenant_id, project_id, "Polish onboarding").await;

    let a = app(pool.clone());
    let owner_token = token_for("usr-owner", 66);
    let member_token = token_for("usr-member", 66);
    let uri = format!("/v1/pm/tasks/{task_id}/policy-override");

    // 全 null = 无差异可写 → 400
    let (status, body) = call(&a, "PUT", &uri, &owner_token, Some(json!({}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // 普通成员 → 403（行不落、审计不落）
    let (status, body) = call(
        &a,
        "PUT",
        &uri,
        &member_token,
        Some(json!({ "require_human_approval": true })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let overrides: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.task_policy_override")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(overrides, 0);
    assert!(
        audit_rows(&pool, project_id, "execution_policy.task_override_updated")
            .await
            .is_empty()
    );

    // owner upsert：写的字段落值、未写字段 = NULL 继承
    let (status, body) = call(
        &a,
        "PUT",
        &uri,
        &owner_token,
        Some(json!({ "auto_accept": false, "require_human_approval": true })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let row: (Option<bool>, Option<bool>, Option<bool>, Option<bool>) = sqlx::query_as(
        "SELECT auto_accept, auto_merge, auto_publish_knowledge, require_human_approval \
           FROM delivery.task_policy_override WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (Some(false), None, None, Some(true)), "{row:?}");
    let audits = audit_rows(&pool, project_id, "execution_policy.task_override_updated").await;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].0["task_id"], json!(task_id));
    assert_eq!(audits[0].0["require_human_approval"], true);
    assert_eq!(audits[0].0["auto_merge"], Value::Null);

    // 覆盖读面可见（JOIN title）
    let view = policies_of(&a, &owner_token, project_id).await;
    let overrides = view["taskOverrides"].as_array().unwrap();
    assert_eq!(overrides.len(), 1, "{view}");
    assert_eq!(overrides[0]["task_title"], "Polish onboarding");

    // DELETE → 恢复全继承 + cleared 审计
    let (status, _) = call(&a, "DELETE", &uri, &owner_token, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let overrides: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.task_policy_override")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(overrides, 0);
    let cleared = audit_rows(&pool, project_id, "execution_policy.task_override_cleared").await;
    assert_eq!(cleared.len(), 1);
    assert_eq!(cleared[0].0["task_id"], json!(task_id));

    // 再 DELETE（行已不存在）= 幂等成功、不重复审计
    let (status, _) = call(&a, "DELETE", &uri, &owner_token, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let cleared = audit_rows(&pool, project_id, "execution_policy.task_override_cleared").await;
    assert_eq!(cleared.len(), 1);
}
