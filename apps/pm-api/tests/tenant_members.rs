//! 租户成员角色管理验收（T-021b / `docs/ai-pm-tenant-infrastructure-v1.md` §6.2 §6.3）。
//!
//! 覆盖 RV-1..RV-10（用户补充要求 §5 的九条 + ADR-0001 RV-2）与 TN-4/TN-5。
//! 关键不变量：`core.tenant_member.role` 是唯一事实源——每次请求都查库，
//! 所以降级/删除后**旧 JWT 的下一次请求即被拒**，不等 token 过期。

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

fn token(sub: &str, roles: Value) -> String {
    sign_token(
        PRIVATE_PEM,
        &json!({
            "sub": sub, "tid": 1, "username": sub, "roles": roles,
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

/// 首管理员（`usr-owner`）+ 一个待晋升的成员（`usr-admin`）+ 普通成员。
struct World {
    app: Router,
    tenant_id: Uuid,
    owner_id: Uuid,
    admin_id: Uuid,
    member_id: Uuid,
}

async fn me(app: &Router, token: &str) -> Value {
    json_of(
        app.clone()
            .oneshot(request("GET", "/v1/pm/me", token, None))
            .await
            .unwrap(),
    )
    .await
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

/// 搭一个「1 owner + 1 admin + 1 member」的租户。
///
/// owner 走环境对冷启动（首个请求即激活且 `role='owner'`）；
/// 另外两把 token 首访会 provision 成**未激活**的 `member`，需 owner 显式激活。
async fn world(pool: &PgPool) -> World {
    let app = router_with_auth_config(pool.clone(), test_jwks(), first_admin("usr-owner"));
    let owner = me(&app, &token("usr-owner", json!([]))).await;
    let tenant_id = Uuid::parse_str(owner["tenant_id"].as_str().unwrap()).unwrap();
    let owner_id = Uuid::parse_str(owner["principal_id"].as_str().unwrap()).unwrap();
    assert_eq!(owner["is_admin"], json!(true), "first admin must be owner");

    let admin = me(&app, &token("usr-admin", json!([]))).await;
    let admin_id = Uuid::parse_str(admin["principal_id"].as_str().unwrap()).unwrap();
    let member = me(&app, &token("usr-member", json!([]))).await;
    let member_id = Uuid::parse_str(member["principal_id"].as_str().unwrap()).unwrap();

    // 激活两者（PATCH /principals 只在 active 之前由 owner 可调）。
    for id in [admin_id, member_id] {
        let (status, _) = call(
            &app,
            "PATCH",
            &format!("/v1/pm/principals/{id}"),
            &token("usr-owner", json!([])),
            Some(json!({ "is_active": true })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    // 把 usr-admin 升为 admin。
    let (status, _) = call(
        &app,
        "PATCH",
        &format!("/v1/pm/tenants/{tenant_id}/members/{admin_id}/role"),
        &token("usr-owner", json!([])),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "owner may grant admin");

    World {
        app,
        tenant_id,
        owner_id,
        admin_id,
        member_id,
    }
}

// ────────────────────────────────────────────────────────────── RV-1 / RV-2

/// 用户补充要求 §5 第 1 条的字面场景：**Admin 可以把另一个 Admin 降为 Member**
/// ——但不能再把他提回去（第 5 条：不得提升超过自身权限）。
#[sqlx::test(migrations = "../../migrations")]
async fn rv1b_admin_can_demote_another_admin_but_cannot_promote_back(pool: PgPool) {
    let w = world(&pool).await;
    let admin_token = token("usr-admin", json!(["tenant_admin"]));

    // 造第二个 admin（一个从未登录过的 principal，直接绑定成员行）。
    let second_admin: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', 'usr-admin-2', true) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) \
         VALUES ($1, $2, 'admin')",
    )
    .bind(w.tenant_id)
    .bind(second_admin)
    .execute(&pool)
    .await
    .unwrap();

    // admin 降另一个 admin → 允许。
    let (status, _) = call(
        &w.app,
        "PATCH",
        &format!(
            "/v1/pm/tenants/{}/members/{}/role",
            w.tenant_id, second_admin
        ),
        &admin_token,
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "admin 可以降级另一个 admin");
    let role: String = sqlx::query_scalar(
        "SELECT role FROM core.tenant_member WHERE tenant_id = $1 AND principal_id = $2",
    )
    .bind(w.tenant_id)
    .bind(second_admin)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(role, "member");

    // admin 想把他提回 admin → 403（不得提升超过自身权限）。
    let (status, _) = call(
        &w.app,
        "PATCH",
        &format!(
            "/v1/pm/tenants/{}/members/{}/role",
            w.tenant_id, second_admin
        ),
        &admin_token,
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../migrations")]
async fn rv1_rv2_admin_demoted_to_member_loses_access_on_the_same_jwt(pool: PgPool) {
    let w = world(&pool).await;
    let admin_token = token("usr-admin", json!(["tenant_admin"]));

    // 降级前：admin 能读成员列表。
    let (status, _) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/tenants/{}/members", w.tenant_id),
        &admin_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // RV-1：owner 把 admin 降为 member。
    let (status, _) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/tenants/{}/members/{}/role", w.tenant_id, w.admin_id),
        &token("usr-owner", json!([])),
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let role: String = sqlx::query_scalar(
        "SELECT role FROM core.tenant_member WHERE tenant_id = $1 AND principal_id = $2",
    )
    .bind(w.tenant_id)
    .bind(w.admin_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(role, "member");

    // 审计：before/after 正确。
    let (action, before, after): (String, Value, Value) = sqlx::query_as(
        "SELECT action, before_data, after_data FROM governance.tenant_audit_log \
          WHERE tenant_id = $1 AND target_principal_id = $2 \
          ORDER BY created_at DESC LIMIT 1",
    )
    .bind(w.tenant_id)
    .bind(w.admin_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(action, "tenant_member.role_changed");
    assert_eq!(before["role"], json!("admin"));
    assert_eq!(after["role"], json!("member"));

    // RV-2：同一张 JWT（仍带 tenant_admin role）立刻被拒——撤权不等 token 过期。
    // 探针用仍 admin 专用的成员详情端点（列表已对成员只读开放，0044）。
    let (status, _) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/tenants/{}/members/{}", w.tenant_id, w.member_id),
        &admin_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let now = me(&w.app, &admin_token).await;
    assert_eq!(now["is_admin"], json!(false));
}

// ──────────────────────────────────────────────────────────────────── RV-3

#[sqlx::test(migrations = "../../migrations")]
async fn rv3_removed_member_is_rejected_on_the_same_jwt(pool: PgPool) {
    let w = world(&pool).await;
    let member_token = token("usr-member", json!([]));

    let before = me(&w.app, &member_token).await;
    assert_eq!(before["is_active"], json!(true));

    let (status, _) = call(
        &w.app,
        "DELETE",
        &format!("/v1/pm/tenants/{}/members/{}", w.tenant_id, w.member_id),
        &token("usr-owner", json!([])),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.tenant_member WHERE tenant_id = $1 AND principal_id = $2",
    )
    .bind(w.tenant_id)
    .bind(w.member_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);

    // 旧 JWT 立刻失去管理面权限（成员行已不存在 → 最小权限）。
    let (status, _) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/tenants/{}/members/{}", w.tenant_id, w.member_id),
        &member_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.tenant_audit_log \
          WHERE tenant_id = $1 AND target_principal_id = $2 AND action = 'tenant_member.removed'",
    )
    .bind(w.tenant_id)
    .bind(w.member_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);
}

// ──────────────────────────────────────────────────────────────────── RV-4

#[sqlx::test(migrations = "../../migrations")]
async fn rv4_member_cannot_touch_member_management(pool: PgPool) {
    let w = world(&pool).await;
    let member_token = token("usr-member", json!(["tenant_admin"])); // 即便 token 自称管理员
    let base = format!("/v1/pm/tenants/{}/members", w.tenant_id);

    // 名册只读对所有成员开放（0044 口径：member 只读可见，管理零权限）。
    let (status, _) = call(&w.app, "GET", &base, &member_token, None).await;
    assert_eq!(status, StatusCode::OK);

    for (method, uri, body) in [
        ("GET", format!("{base}/{}", w.member_id), None),
        (
            "PATCH",
            format!("{base}/{}/role", w.member_id),
            Some(json!({ "role": "admin" })),
        ),
        ("DELETE", format!("{base}/{}", w.member_id), None),
    ] {
        let (status, _) = call(&w.app, method, &uri, &member_token, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}");
    }
}

// ──────────────────────────────────────────────────────────────── RV-5 / RV-6

#[sqlx::test(migrations = "../../migrations")]
async fn rv5_nobody_but_owner_transfer_creates_an_owner(pool: PgPool) {
    let w = world(&pool).await;
    let admin_token = token("usr-admin", json!(["tenant_admin"]));

    // admin 想把别人设成 owner → 409 USE_OWNER_TRANSFER。
    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!(
            "/v1/pm/tenants/{}/members/{}/role",
            w.tenant_id, w.member_id
        ),
        &admin_token,
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], json!("USE_OWNER_TRANSFER"));

    // admin 想把自己提成 admin（已是 admin）→ 403。
    let (status, _) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/tenants/{}/members/{}/role", w.tenant_id, w.admin_id),
        &admin_token,
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // owner 自己也不能用 PATCH 转让 owner。
    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/tenants/{}/members/{}/role", w.tenant_id, w.owner_id),
        &token("usr-owner", json!([])),
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], json!("USE_OWNER_TRANSFER"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn rv6_the_last_owner_cannot_be_demoted_or_removed(pool: PgPool) {
    let w = world(&pool).await;
    let owner_token = token("usr-owner", json!([]));

    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/tenants/{}/members/{}/role", w.tenant_id, w.owner_id),
        &owner_token,
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], json!("OWNER_LAST"));

    let (status, body) = call(
        &w.app,
        "DELETE",
        &format!("/v1/pm/tenants/{}/members/{}", w.tenant_id, w.owner_id),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], json!("OWNER_LAST"));

    let owners: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.tenant_member WHERE tenant_id = $1 AND role = 'owner'",
    )
    .bind(w.tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owners, 1, "库内必须仍有 owner");
}

// ──────────────────────────────────────────────────────────────────── RV-7

#[sqlx::test(migrations = "../../migrations")]
async fn rv7_cross_tenant_member_calls_are_404(pool: PgPool) {
    let w = world(&pool).await;
    // 另一个租户 + 自己的成员，用 `ons` 的 token 去打 `w.tenant_id`。
    let other: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', '2', 'lmms:2') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let other_owner = Uuid::new_v4();
    sqlx::query("INSERT INTO core.principal (id, kind, display_name, is_active) VALUES ($1, 'human', 'other-owner', true)")
        .bind(other_owner)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(other)
    .bind(other_owner)
    .execute(&pool)
    .await
    .unwrap();

    // 直接调用 service 层不可能绕过；这里用 owner 的 token 打别的租户路径。
    let (status, _) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/tenants/{other}/members"),
        &token("usr-owner", json!([])),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "跨租户读不泄露存在性");

    let (status, _) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/tenants/{other}/members/{}/role", other_owner),
        &token("usr-owner", json!([])),
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let role: String = sqlx::query_scalar(
        "SELECT role FROM core.tenant_member WHERE tenant_id = $1 AND principal_id = $2",
    )
    .bind(other)
    .bind(other_owner)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(role, "owner", "跨租户写不得生效");
}

// ──────────────────────────────────────────────────────────────────── RV-8

#[sqlx::test(migrations = "../../migrations")]
async fn rv8_audit_failure_rolls_the_role_change_back(pool: PgPool) {
    let w = world(&pool).await;
    // 注入：让 `role_changed` 这一动作的审计写入必然违反约束。
    // `NOT VALID` 只对新行生效——表里已经有 world() 造出的 role_changed 行。
    sqlx::query(
        "ALTER TABLE governance.tenant_audit_log \
         ADD CONSTRAINT tmp_reject_role_changed \
         CHECK (action <> 'tenant_member.role_changed') NOT VALID",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (status, _) = call(
        &w.app,
        "PATCH",
        &format!(
            "/v1/pm/tenants/{}/members/{}/role",
            w.tenant_id, w.member_id
        ),
        &token("usr-owner", json!([])),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let role: String = sqlx::query_scalar(
        "SELECT role FROM core.tenant_member WHERE tenant_id = $1 AND principal_id = $2",
    )
    .bind(w.tenant_id)
    .bind(w.member_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(role, "member", "审计失败必须让角色变更一起回滚");
}

// ──────────────────────────────────────────────────────────────────── RV-9

#[sqlx::test(migrations = "../../migrations")]
async fn rv9_concurrent_owner_demotion_never_reaches_zero_owners(pool: PgPool) {
    let w = world(&pool).await;
    // 造两个 owner，然后并发降级。
    sqlx::query(
        "UPDATE core.tenant_member SET role = 'owner' WHERE tenant_id = $1 AND principal_id = $2",
    )
    .bind(w.tenant_id)
    .bind(w.admin_id)
    .execute(&pool)
    .await
    .unwrap();

    // 两个 owner 各自降级对方：并发下必须有一个被 OWNER_LAST 拦住。
    let owner_token = token("usr-owner", json!([]));
    let admin_owner_token = token("usr-admin", json!([]));
    let a_uri = format!("/v1/pm/tenants/{}/members/{}/role", w.tenant_id, w.admin_id);
    let b_uri = format!("/v1/pm/tenants/{}/members/{}/role", w.tenant_id, w.owner_id);

    let first = w.app.clone().oneshot(request(
        "PATCH",
        &a_uri,
        &owner_token,
        Some(json!({ "role": "admin" })),
    ));
    let second = w.app.clone().oneshot(request(
        "PATCH",
        &b_uri,
        &admin_owner_token,
        Some(json!({ "role": "admin" })),
    ));
    let (a, b) = tokio::join!(first, second);

    let mut codes = vec![a.unwrap().status(), b.unwrap().status()];
    codes.sort();
    assert!(
        codes.contains(&StatusCode::OK),
        "至少一个必须成功，实际 {codes:?}"
    );
    // 另一个要么撞 OWNER_LAST（409），要么因「对方先把自己降成 admin」而失去
    // 改 owner 的资格（403）。两种都证明规则没被绕过。
    assert!(
        codes
            .iter()
            .any(|s| *s == StatusCode::CONFLICT || *s == StatusCode::FORBIDDEN),
        "两个请求不得都成功，实际 {codes:?}"
    );
    assert!(
        !codes.iter().all(|s| *s == StatusCode::OK),
        "并发降级不得全部生效，实际 {codes:?}"
    );

    let owners: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.tenant_member WHERE tenant_id = $1 AND role = 'owner'",
    )
    .bind(w.tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owners, 1, "并发降级不得让 owner 归零");
}

// ─────────────────────────────────────────────────────────────────── RV-10

#[sqlx::test(migrations = "../../migrations")]
async fn rv10_token_roles_alone_never_grant_tenant_admin(pool: PgPool) {
    let pool2 = pool.clone();
    let app = router_with_auth_config(pool2, test_jwks(), AuthConfig::default());
    // token 自称 tenant_admin，但库里只有自动 provision 的 member 行。
    let admin_claimant = token("usr-claimant", json!(["tenant_admin"]));
    let who = me(&app, &admin_claimant).await;
    assert_eq!(who["is_admin"], json!(false), "token roles 不是事实源");
    assert_eq!(who["tenant_role"], json!("member"));

    let tenant_id = Uuid::parse_str(who["tenant_id"].as_str().unwrap()).unwrap();
    let (status, _) = call(
        &app,
        "GET",
        &format!("/v1/pm/tenants/{tenant_id}/members"),
        &admin_claimant,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ─────────────────────────────────────────────────────────────────── TN-4/TN-5

#[sqlx::test(migrations = "../../migrations")]
async fn tn4_owner_transfer_swaps_roles_atomically_and_audits_twice(pool: PgPool) {
    let w = world(&pool).await;
    let (status, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/owner-transfer", w.tenant_id),
        &token("usr-owner", json!([])),
        Some(json!({ "new_owner_principal_id": w.admin_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["previous_owner"].as_str().unwrap(),
        w.owner_id.to_string()
    );
    assert_eq!(body["new_owner"].as_str().unwrap(), w.admin_id.to_string());

    let roles: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT principal_id, role FROM core.tenant_member WHERE tenant_id = $1")
            .bind(w.tenant_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    let map: std::collections::HashMap<Uuid, String> = roles.into_iter().collect();
    assert_eq!(map[&w.owner_id], "admin");
    assert_eq!(map[&w.admin_id], "owner");

    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.tenant_audit_log \
          WHERE tenant_id = $1 AND action = 'tenant_member.owner_transferred'",
    )
    .bind(w.tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 2, "旧 owner 降级 + 新 owner 晋升各一条");
}

#[sqlx::test(migrations = "../../migrations")]
async fn tn5_admin_cannot_transfer_ownership(pool: PgPool) {
    let w = world(&pool).await;
    let (status, _) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/owner-transfer", w.tenant_id),
        &token("usr-admin", json!(["tenant_admin"])),
        Some(json!({ "new_owner_principal_id": w.admin_id })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let owners: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.tenant_member WHERE tenant_id = $1 AND role = 'owner'",
    )
    .bind(w.tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owners, 1);
}
