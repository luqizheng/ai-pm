//! Project 成员授权验收（S01-W13 / `docs/ai-pm-s01-project-member-authorization-v1.md`）。
//!
//! 覆盖 A001~A016：
//! - 正向：授权后成员立刻可访问项目（A001）、撤销后立刻失权（A007）
//! - 负向：跨项目/跨租户不可见（A002、A004）、授权前提（A003）、非管理者（A005）、
//!   Agent 行（A008 附加）、唯一 owner（A009）
//! - 幂等：重复授权只留一行（A010）
//! - 并发：并发授权只产生一行（A012）、租户撤权与项目授权并发（A013）
//! - 数据：role CHECK 与 PK 违例（A015、A016）
//! - 审计：授权/撤销/级联撤销均可追溯（A011、A014）

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

fn token(sub: &str) -> String {
    sign_token(
        PRIVATE_PEM,
        &json!({
            "sub": sub, "tid": 1, "username": sub, "roles": [],
            "permissions": [], "aud": "coder-micro-services",
        }),
        Duration::from_secs(300),
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

async fn json_of(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

fn first_admin(sub: &str) -> AuthConfig {
    AuthConfig {
        first_admin_tenant: Some("1".to_string()),
        first_admin_subject: Some(sub.to_string()),
    }
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
    (status, json_of(response).await)
}

/// 一个租户 + 一个项目 + 若干真人。
struct World {
    app: Router,
    tenant_id: Uuid,
    /// 租户 owner（= first admin 建团后的 Team Owner），同时是项目 owner。
    owner_id: Uuid,
    /// Team 锚（P4-ORG Phase 2：成员管理路由挂 core.team.id）。
    team_id: Uuid,
    /// 租户普通成员（已激活），项目成员的授予目标。
    member_id: Uuid,
    /// 另一个租户普通成员（已激活）。
    other_id: Uuid,
    project_id: Uuid,
}

async fn world(pool: &PgPool) -> World {
    let app = router_with_auth_config(pool.clone(), test_jwks(), first_admin("usr-owner"));

    let (_, owner) = call(&app, "GET", "/v1/pm/me", &token("usr-owner"), None).await;
    let tenant_id = Uuid::parse_str(owner["tenant_id"].as_str().unwrap()).unwrap();
    let owner_id = Uuid::parse_str(owner["principal_id"].as_str().unwrap()).unwrap();
    // P4-ORG Phase 2：first_admin 只落预登记行——先认领建团成为 Team Owner。
    let (team_st, team_body) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &token("usr-owner"),
        Some(json!({ "name": "PM Team" })),
    )
    .await;
    assert_eq!(team_st, StatusCode::CREATED, "{team_body}");
    let (_, owner) = call(&app, "GET", "/v1/pm/me", &token("usr-owner"), None).await;
    assert_eq!(
        owner["is_admin"],
        json!(true),
        "first admin must be team owner"
    );
    let team_id = Uuid::parse_str(owner["memberships"][0]["team_id"].as_str().unwrap()).unwrap();

    // 两个普通成员：首次访问会被 provision 成未激活的租户成员，需 owner 激活。
    let mut ids = Vec::new();
    for sub in ["usr-member", "usr-other"] {
        let (_, body) = call(&app, "GET", "/v1/pm/me", &token(sub), None).await;
        let id = Uuid::parse_str(body["principal_id"].as_str().unwrap()).unwrap();
        let (status, _) = call(
            &app,
            "PATCH",
            &format!("/v1/pm/principals/{id}"),
            &token("usr-owner"),
            Some(json!({ "is_active": true })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "owner must be able to activate {sub}"
        );
        // P4-ORG：成员名册 = core.team_member——激活后须入团。
        let (add_st, add_body) = call(
            &app,
            "POST",
            &format!("/v1/pm/teams/{team_id}/members"),
            &token("usr-owner"),
            Some(json!({ "principal_id": id, "role": "member" })),
        )
        .await;
        assert_eq!(
            add_st,
            StatusCode::OK,
            "owner must be able to add {sub}: {add_body}"
        );
        ids.push(id);
    }
    let member_id = ids[0];
    let other_id = ids[1];

    let project_id = seed_project(pool, tenant_id, "PM-W13").await;
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'owner')",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(owner_id)
    .execute(pool)
    .await
    .unwrap();

    World {
        app,
        tenant_id,
        team_id,
        owner_id,
        member_id,
        other_id,
        project_id,
    }
}

async fn seed_project(pool: &PgPool, tenant_id: Uuid, code: &str) -> Uuid {
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, $2) RETURNING id",
    )
    .bind(tenant_id)
    .bind(format!("customer-{code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(tenant_id)
    .bind(customer_id)
    .bind(code)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 造一个「本租户、已激活、但既不是租户成员也不是项目成员」的真人。
///
/// 他能通过鉴权层（token → principal 映射），但对项目不可见 —— 用来验证
/// 「不可见即 404」。
async fn seed_outsider(pool: &PgPool, tenant_id: Uuid, sub: &str) -> Uuid {
    let principal_id: Uuid = sqlx::query_scalar(
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
    .bind(principal_id)
    .execute(pool)
    .await
    .unwrap();
    principal_id
}

fn members_uri(project_id: Uuid) -> String {
    format!("/v1/pm/projects/{project_id}/members")
}

async fn member_row_count(pool: &PgPool, project_id: Uuid, principal_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM core.project_member WHERE project_id = $1 AND principal_id = $2",
    )
    .bind(project_id)
    .bind(principal_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 项目列表里是否能看到该项目（成员可见性的端到端口径）。
async fn project_visible(app: &Router, token: &str, project_id: Uuid) -> bool {
    let (status, body) = call(app, "GET", "/v1/pm/projects", token, None).await;
    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().cloned().unwrap_or_default();
    items.iter().any(|item| {
        item["id"].as_str() == Some(project_id.to_string().as_str())
            || item["code"].as_str() == Some("PM-W13")
    })
}

// ───────────────────────────────────────────────────────── A001 正向

/// A001：owner 授予成员 → 成员立刻可访问该项目（列表与成员端点都可见）。
#[sqlx::test(migrations = "../../migrations")]
async fn grant_makes_member_able_to_see_project(pool: PgPool) {
    let w = world(&pool).await;

    let (status, body) = call(
        &w.app,
        "POST",
        &members_uri(w.project_id),
        &token("usr-owner"),
        Some(json!({ "principal_id": w.member_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "grant must succeed: {body}");
    assert_eq!(body["role"], json!("member"));

    let (status, body) = call(
        &w.app,
        "GET",
        &members_uri(w.project_id),
        &token("usr-member"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "member must see the project: {body}"
    );
    let items = body["items"].as_array().cloned().unwrap_or_default();
    assert!(
        items
            .iter()
            .any(|item| item["principal_id"].as_str() == Some(w.member_id.to_string().as_str())),
        "granted member must appear in the member list: {body}"
    );
    assert!(
        project_visible(&w.app, &token("usr-member"), w.project_id).await,
        "granted member must see the project in the project list"
    );
}

// ───────────────────────────────────────────────────────── A002 / A004 负向

/// A002：既不是项目成员也不是租户管理者的调用人 → 404（不可见即不存在）。
#[sqlx::test(migrations = "../../migrations")]
async fn outsider_cannot_see_project_members(pool: PgPool) {
    let w = world(&pool).await;
    let outsider = seed_outsider(&pool, w.tenant_id, "usr-outsider").await;
    assert_ne!(outsider, w.owner_id);

    let (status, body) = call(
        &w.app,
        "GET",
        &members_uri(w.project_id),
        &token("usr-outsider"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "outsider must get 404: {body}"
    );
}

/// A004：跨租户项目 → 404。
#[sqlx::test(migrations = "../../migrations")]
async fn cross_tenant_project_is_not_found(pool: PgPool) {
    let w = world(&pool).await;
    let other_tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'other', 'other') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let other_project = seed_project(&pool, other_tenant, "PM-OTHER").await;

    let (status, body) = call(
        &w.app,
        "GET",
        &members_uri(other_project),
        &token("usr-owner"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "cross tenant must get 404: {body}"
    );
}

// ───────────────────────────────────────────────────────── A003 授权前提

/// A003：目标不是本租户成员 → 409 NOT_TENANT_MEMBER，且零副作用。
#[sqlx::test(migrations = "../../migrations")]
async fn grant_requires_tenant_membership(pool: PgPool) {
    let w = world(&pool).await;
    let outsider = seed_outsider(&pool, w.tenant_id, "usr-outsider").await;

    let (status, body) = call(
        &w.app,
        "POST",
        &members_uri(w.project_id),
        &token("usr-owner"),
        Some(json!({ "principal_id": outsider })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "must conflict: {body}");
    assert_eq!(body["code"], json!("NOT_TENANT_MEMBER"));
    assert_eq!(member_row_count(&pool, w.project_id, outsider).await, 0);
}

/// 附加：Agent principal 不可由此 API 授予 → 409 AGENT_PRINCIPAL。
#[sqlx::test(migrations = "../../migrations")]
async fn grant_rejects_agent_principal(pool: PgPool) {
    let w = world(&pool).await;
    let agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('dev_agent', 'agent-x', true) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let (status, body) = call(
        &w.app,
        "POST",
        &members_uri(w.project_id),
        &token("usr-owner"),
        Some(json!({ "principal_id": agent })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "must conflict: {body}");
    assert_eq!(body["code"], json!("AGENT_PRINCIPAL"));
}

// ───────────────────────────────────────────────────────── A005 非管理者

/// A005：项目普通成员（非 owner、非租户 admin）尝试授权 → 403，且零副作用。
#[sqlx::test(migrations = "../../migrations")]
async fn plain_project_member_cannot_grant(pool: PgPool) {
    let w = world(&pool).await;
    // 先把 usr-member 授成项目成员。
    let (status, _) = call(
        &w.app,
        "POST",
        &members_uri(w.project_id),
        &token("usr-owner"),
        Some(json!({ "principal_id": w.member_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = call(
        &w.app,
        "POST",
        &members_uri(w.project_id),
        &token("usr-member"),
        Some(json!({ "principal_id": w.other_id })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "must be forbidden: {body}");
    assert_eq!(member_row_count(&pool, w.project_id, w.other_id).await, 0);
}

// ───────────────────────────────────────────────────────── A010 幂等

/// A010：重复授权 → 409 ALREADY_MEMBER，成员关系仍只有一行。
#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_grant_keeps_single_row(pool: PgPool) {
    let w = world(&pool).await;

    let (status, _) = call(
        &w.app,
        "POST",
        &members_uri(w.project_id),
        &token("usr-owner"),
        Some(json!({ "principal_id": w.member_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = call(
        &w.app,
        "POST",
        &members_uri(w.project_id),
        &token("usr-owner"),
        Some(json!({ "principal_id": w.member_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "second grant must conflict: {body}"
    );
    assert_eq!(body["code"], json!("ALREADY_MEMBER"));
    assert_eq!(member_row_count(&pool, w.project_id, w.member_id).await, 1);
}

// ───────────────────────────────────────────────────────── A007 撤权即时生效

/// A007：撤销后成员的**下一次请求**即被拒（不等 token 过期）。
#[sqlx::test(migrations = "../../migrations")]
async fn revoke_takes_effect_immediately(pool: PgPool) {
    let w = world(&pool).await;
    let (status, _) = call(
        &w.app,
        "POST",
        &members_uri(w.project_id),
        &token("usr-owner"),
        Some(json!({ "principal_id": w.member_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(project_visible(&w.app, &token("usr-member"), w.project_id).await);

    let (status, body) = call(
        &w.app,
        "DELETE",
        &format!("{}/{}", members_uri(w.project_id), w.member_id),
        &token("usr-owner"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "revoke must succeed: {body}");
    assert_eq!(body["revoked"], json!(true));

    assert!(
        !project_visible(&w.app, &token("usr-member"), w.project_id).await,
        "revoked member must lose project visibility immediately"
    );
    let (status, _) = call(
        &w.app,
        "GET",
        &members_uri(w.project_id),
        &token("usr-member"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A011 审计：撤销落一行 project_member.revoked。
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'project_member.revoked' AND target_id = $2",
    )
    .bind(w.project_id)
    .bind(w.member_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1, "revoke must be audited");
}

/// A009：唯一 owner 不可被撤销 → 409 OWNER_LAST。
#[sqlx::test(migrations = "../../migrations")]
async fn cannot_revoke_project_owner(pool: PgPool) {
    let w = world(&pool).await;
    let (status, body) = call(
        &w.app,
        "DELETE",
        &format!("{}/{}", members_uri(w.project_id), w.owner_id),
        &token("usr-owner"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "must conflict: {body}");
    assert_eq!(body["code"], json!("OWNER_LAST"));
    assert_eq!(member_row_count(&pool, w.project_id, w.owner_id).await, 1);
}

// ───────────────────────────────────────────────────── 所有权转移

/// 所有权转移：原 owner 降为 member，新 owner 升为 owner，并落两条审计。
#[sqlx::test(migrations = "../../migrations")]
async fn owner_transfer_moves_ownership(pool: PgPool) {
    let w = world(&pool).await;
    let (status, _) = call(
        &w.app,
        "POST",
        &members_uri(w.project_id),
        &token("usr-owner"),
        Some(json!({ "principal_id": w.member_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/projects/{}/owner-transfer", w.project_id),
        &token("usr-owner"),
        Some(json!({ "new_owner_principal_id": w.member_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "transfer must succeed: {body}");
    assert_eq!(body["previous_owner"], json!(w.owner_id.to_string()));
    assert_eq!(body["role"], json!("owner"));

    let roles: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT principal_id, role FROM core.project_member WHERE project_id = $1")
            .bind(w.project_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    let role_of = |id: Uuid| {
        roles
            .iter()
            .find(|(pid, _)| *pid == id)
            .map(|(_, role)| role.clone())
            .unwrap_or_default()
    };
    assert_eq!(role_of(w.member_id), "owner");
    assert_eq!(role_of(w.owner_id), "member");
}

// ───────────────────────────────────────────────────── A014 租户撤权级联

/// A014：移除租户成员 → 其全部真人项目成员关系级联撤销，访问即刻失效。
#[sqlx::test(migrations = "../../migrations")]
async fn tenant_removal_cascades_project_memberships(pool: PgPool) {
    let w = world(&pool).await;
    let second = seed_project(&pool, w.tenant_id, "PM-W13-B").await;
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'owner')",
    )
    .bind(w.tenant_id)
    .bind(second)
    .bind(w.owner_id)
    .execute(&pool)
    .await
    .unwrap();

    // 成员在两个项目都有成员身份。
    for project_id in [w.project_id, second] {
        let (status, _) = call(
            &w.app,
            "POST",
            &members_uri(project_id),
            &token("usr-owner"),
            Some(json!({ "principal_id": w.member_id })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    assert!(project_visible(&w.app, &token("usr-member"), w.project_id).await);

    let (status, body) = call(
        &w.app,
        "DELETE",
        &format!("/v1/pm/teams/{}/members/{}", w.team_id, w.member_id),
        &token("usr-owner"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "tenant removal must succeed: {body}"
    );

    assert!(
        !project_visible(&w.app, &token("usr-member"), w.project_id).await,
        "tenant removal must revoke project access immediately"
    );
    assert_eq!(member_row_count(&pool, w.project_id, w.member_id).await, 0);
    assert_eq!(member_row_count(&pool, second, w.member_id).await, 0);

    // 租户审计行带级联计数。
    let after: Option<Value> = sqlx::query_scalar(
        "SELECT after_data FROM governance.tenant_audit_log \
          WHERE tenant_id = $1 AND action = 'team_member.removed' AND target_principal_id = $2",
    )
    .bind(w.tenant_id)
    .bind(w.member_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let after = after.expect("tenant audit row must exist");
    assert_eq!(after["project_memberships_revoked"], json!(2));
}

/// A010 附加：被移除者是项目 owner 时拒绝移除（不静默产生无主项目）。
#[sqlx::test(migrations = "../../migrations")]
async fn tenant_removal_blocked_by_project_ownership(pool: PgPool) {
    let w = world(&pool).await;
    // 把项目所有权转给 usr-other（他是租户普通成员）。
    let (status, _) = call(
        &w.app,
        "POST",
        &members_uri(w.project_id),
        &token("usr-owner"),
        Some(json!({ "principal_id": w.other_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/projects/{}/owner-transfer", w.project_id),
        &token("usr-owner"),
        Some(json!({ "new_owner_principal_id": w.other_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "transfer must succeed: {body}");

    let (status, body) = call(
        &w.app,
        "DELETE",
        &format!("/v1/pm/teams/{}/members/{}", w.team_id, w.other_id),
        &token("usr-owner"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "must conflict: {body}");
    assert_eq!(body["code"], json!("PROJECT_OWNER_DEPENDENCY"));

    // 租户成员行仍在。
    let still: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.tenant_member WHERE tenant_id = $1 AND principal_id = $2",
    )
    .bind(w.tenant_id)
    .bind(w.other_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(still, 1, "tenant membership must survive the rejection");
}

// ───────────────────────────────────────────────────────── A012 并发

/// A012：两个并发授权请求 → 恰好一个成功，成员关系只留一行。
#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_grants_produce_single_row(pool: PgPool) {
    let w = world(&pool).await;
    let uri = members_uri(w.project_id);
    let body = json!({ "principal_id": w.member_id });

    let (a, b) = tokio::join!(
        async {
            call(
                &w.app,
                "POST",
                &uri,
                &token("usr-owner"),
                Some(body.clone()),
            )
            .await
        },
        async {
            call(
                &w.app,
                "POST",
                &uri,
                &token("usr-owner"),
                Some(body.clone()),
            )
            .await
        }
    );

    let mut ok = 0;
    let mut conflict = 0;
    for (status, _) in [a, b] {
        match status {
            StatusCode::OK => ok += 1,
            StatusCode::CONFLICT => conflict += 1,
            other => panic!("unexpected status {other}"),
        }
    }
    assert_eq!(ok, 1, "exactly one grant must win");
    assert_eq!(conflict, 1, "the loser must see ALREADY_MEMBER");
    assert_eq!(member_row_count(&pool, w.project_id, w.member_id).await, 1);
}

/// A013：租户撤权与项目授权并发 → 终态唯一（要么授权被拒，要么授权后被级联撤销）。
#[sqlx::test(migrations = "../../migrations")]
async fn tenant_removal_racing_grant_has_single_final_state(pool: PgPool) {
    let w = world(&pool).await;
    let uri = members_uri(w.project_id);
    let body = json!({ "principal_id": w.member_id });

    let (grant, removal) = tokio::join!(
        async {
            call(
                &w.app,
                "POST",
                &uri,
                &token("usr-owner"),
                Some(body.clone()),
            )
            .await
        },
        async {
            call(
                &w.app,
                "DELETE",
                &format!("/v1/pm/teams/{}/members/{}", w.team_id, w.member_id),
                &token("usr-owner"),
                None,
            )
            .await
        }
    );

    // 无论谁先谁后：成员关系**要么**不存在，**要么**存在且该人仍是租户成员。
    let rows = member_row_count(&pool, w.project_id, w.member_id).await;
    let tenant_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.team_member WHERE team_id = $1 AND principal_id = $2",
    )
    .bind(w.tenant_id)
    .bind(w.member_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 锁序（tenant → project → project_member）保证两种终态：
    //   a) 授予先提交 → 级联撤销把它抹掉 → 授予返回 200，但最终无成员关系；
    //   b) 移除先提交 → 授予发现目标已非租户成员 → 409 NOT_TENANT_MEMBER。
    // 唯一不可接受的是「悬空」：人已不是租户成员，却还留着项目成员行。
    assert_eq!(
        tenant_rows, 0,
        "tenant removal must always win the tenant row"
    );
    assert_eq!(
        rows, 0,
        "no dangling project membership may survive the race"
    );
    assert!(
        matches!(grant.0, StatusCode::OK | StatusCode::CONFLICT),
        "grant outcome must be deterministic, got {}",
        grant.0
    );
    assert_eq!(
        removal.0,
        StatusCode::OK,
        "removal of a plain member must not conflict"
    );
}

// ───────────────────────────────────────────────────────── A015 / A016 数据

/// A015：非法 role 违反 CHECK；A016：重复 (project, principal) 违反 PK。
#[sqlx::test(migrations = "../../migrations")]
async fn data_level_constraints_hold(pool: PgPool) {
    let w = world(&pool).await;

    let illegal = sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'bogus')",
    )
    .bind(w.tenant_id)
    .bind(w.project_id)
    .bind(w.member_id)
    .execute(&pool)
    .await;
    assert!(illegal.is_err(), "illegal role must violate the CHECK");
    let err = illegal.unwrap_err();
    let code = err
        .as_database_error()
        .unwrap()
        .code()
        .expect("sqlstate must be present")
        .to_string();
    assert_eq!(code, "23514", "expected check violation");

    let duplicate = sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'member')",
    )
    .bind(w.tenant_id)
    .bind(w.project_id)
    .bind(w.owner_id)
    .execute(&pool)
    .await;
    assert!(duplicate.is_err(), "duplicate row must violate the PK");
    let err = duplicate.unwrap_err();
    let code = err
        .as_database_error()
        .unwrap()
        .code()
        .expect("sqlstate must be present")
        .to_string();
    assert_eq!(code, "23505", "expected unique violation");
}
