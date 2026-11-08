//! Team Onboarding 验收（设计稿 `docs/team-onboarding/`，产品裁决 2026-11-06；
//! 映射 team-domain-model 数据库设计稿 §15 TDM-001~018 的可落地子集）。
//!
//! 覆盖：建团认领（creator=owner / 已治理租户 409 / slug 冲突）、邮箱邀请全
//! 生命周期（建/接受/重复接受/已成员幂等/过期/撤销/PENDING 防重/email 比对）、
//! 邀请链接（重复使用幂等/名额消耗/触顶 EXHAUSTED/撤销）、权限矩阵（member
//! 只读 403）、跨租户 404/403、/me onboarding_required 跃迁、审计四列落地。
//!
//! harness 与 `team_llm.rs` 同款：`keys/*.pem` 自签 JWT + `router_with_auth_config`。
//! 「首登无团队」= 无 first-admin 配置下未知 subject 首次请求被 provision 成
//! inactive member（auth.rs resolve_or_provision）。

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

async fn json_of(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
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

/// 无 first-admin 配置：任何未知 subject 都落成 inactive member（首登无团队态）。
async fn fresh_app(pool: PgPool) -> Router {
    router_with_auth_config(pool.clone(), test_jwks(), AuthConfig::default())
}

async fn me(app: &Router, token: &str) -> Value {
    let (status, body) = call(app, "GET", "/v1/pm/me", token, None).await;
    assert_eq!(status, StatusCode::OK, "me must stay reachable: {body}");
    body
}

/// 已治理租户（owner=usr-owner，同 team_llm world 口径），另含 usr-bob 未激活主体。
async fn governed_world(pool: PgPool) -> (Router, PgPool, Uuid, String) {
    std::env::set_var("AI_PM_SECRET_KEY", "team-onboarding-test-secret");
    let config = AuthConfig {
        first_admin_tenant: Some("1".to_string()),
        first_admin_subject: Some("usr-owner".to_string()),
    };
    let app = router_with_auth_config(pool.clone(), test_jwks(), config);
    let owner_token = token_for("usr-owner", 1);
    let owner = me(&app, &owner_token).await;
    let tenant_id = Uuid::parse_str(owner["tenant_id"].as_str().unwrap()).unwrap();
    (app, pool, tenant_id, owner_token)
}

// ────────────────────────────────────────────────────── 建团认领（TDM-001/002/016）

#[sqlx::test(migrations = "../../migrations")]
async fn fresh_principal_is_onboarding_required(pool: PgPool) {
    let app = fresh_app(pool).await;
    let me_body = me(&app, &token_for("usr-new", 1)).await;
    assert_eq!(me_body["onboarding_required"], json!(true));
    assert_eq!(me_body["is_active"], json!(false));
    assert_eq!(me_body["memberships"], json!([]), "provision 行不算已加入");
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_team_claims_tenant_and_creator_becomes_owner(pool: PgPool) {
    let db = pool.clone();
    let app = fresh_app(pool).await;
    let t = token_for("usr-founder", 1);

    let (status, team) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &t,
        Some(json!({ "name": "Acme R&D", "slug": "acme", "description": "demo" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{team}");
    assert_eq!(team["name"], json!("Acme R&D"));
    assert_eq!(team["slug"], json!("acme"));

    let me_body = me(&app, &t).await;
    assert_eq!(me_body["onboarding_required"], json!(false));
    assert_eq!(me_body["is_active"], json!(true));
    assert_eq!(
        me_body["tenant_role"],
        json!("owner"),
        "Q2：创建者自动 Owner"
    );
    assert_eq!(me_body["team"]["name"], json!("Acme R&D"));
    assert_eq!(me_body["memberships"][0]["role"], json!("owner"));

    // 认领建团落审计（team.created，四列身份齐）。
    let audits: Vec<(String, String)> = sqlx::query_as(
        "SELECT action, target_type FROM governance.tenant_audit_log \
          WHERE action = 'team.created'",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(audits.len(), 1, "{audits:?}");
    assert_eq!(audits[0].1, "tenant");
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_team_rejected_when_tenant_already_governed(pool: PgPool) {
    let (app, _pool, _tenant, _owner_token) = governed_world(pool).await;

    // 新 subject 落进同一个已治理租户 → 认领必须被拒。
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &token_for("usr-newcomer", 1),
        Some(json!({ "name": "Hijack" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["message"], json!("team_already_exists"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_team_rejects_duplicate_slug_across_tenants(pool: PgPool) {
    let app = fresh_app(pool).await;
    let (_, _) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &token_for("usr-a", 1),
        Some(json!({ "name": "Team A", "slug": "acme" })),
    )
    .await;
    // tid=2 → 另一个租户；slug 全局唯一 → 冲突。
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &token_for("usr-b", 2),
        Some(json!({ "name": "Team B", "slug": "acme" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["message"], json!("slug_already_taken"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_team_rejects_invalid_slug_and_name(pool: PgPool) {
    let app = fresh_app(pool).await;
    let t = token_for("usr-c", 1);
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &t,
        Some(json!({ "name": "X", "slug": "Bad Slug!" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, _) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &t,
        Some(json!({ "name": "  " })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ────────────────────────────────────────────────────── 邀请生命周期（TDM-006~009）

#[sqlx::test(migrations = "../../migrations")]
async fn invitation_full_lifecycle(pool: PgPool) {
    let (app, _pool, tenant_id, owner_token) = governed_world(pool).await;
    let base = format!("/v1/pm/tenants/{tenant_id}/invitations");

    // PENDING 防重（设计稿 §3.3 部分唯一）。
    let (status, inv) = call(
        &app,
        "POST",
        &base,
        &owner_token,
        Some(json!({ "email": "Bob@Example.com", "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{inv}");
    let token = inv["token"].as_str().unwrap().to_string();
    assert!(!token.is_empty());
    let (status, body) = call(
        &app,
        "POST",
        &base,
        &owner_token,
        Some(json!({ "email": "bob@example.com", "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["message"], json!("pending_invitation_exists"));

    // OWNER 不可经邀请产生（权限矩阵）。
    let (status, body) = call(
        &app,
        "POST",
        &base,
        &owner_token,
        Some(json!({ "email": "boss@example.com", "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // 预览：任何持 token 者可见团队名/角色（无密钥回显）。
    let (status, preview) = call(
        &app,
        "GET",
        &format!("/v1/pm/invitations/preview?token={token}"),
        &token_for("usr-bob", 1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["role"], json!("member"));
    assert_eq!(preview["email"], json!("bob@example.com"));

    // 接受：usr-bob（本租户未激活主体）→ joined + 激活。
    let bob_token = token_for("usr-bob", 1);
    let (status, accepted) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &bob_token,
        Some(json!({ "token": token, "email": "BOB@example.com" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{accepted}");
    assert_eq!(accepted["outcome"], json!("joined"));
    assert_eq!(accepted["role"], json!("member"));

    let bob = me(&app, &bob_token).await;
    assert_eq!(bob["is_active"], json!(true));
    assert_eq!(bob["tenant_role"], json!("member"));
    assert_eq!(bob["memberships"][0]["role"], json!("member"));

    // 重复接受同一张 → already_accepted。
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &bob_token,
        Some(json!({ "token": token })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["message"], json!("invitation_already_accepted"));

    // 已是成员再收新邀请 → 幂等 already_member 且邀请单终结。
    let (status, inv2) = call(
        &app,
        "POST",
        &base,
        &owner_token,
        Some(json!({ "email": "bob@example.com", "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{inv2}");
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &bob_token,
        Some(json!({ "token": inv2["token"].as_str().unwrap() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], json!("already_member"));

    // 列表可见两张 ACCEPTED。
    let (status, list) = call(&app, "GET", &base, &owner_token, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn invitation_email_mismatch_rejected(pool: PgPool) {
    let (app, _pool, tenant_id, owner_token) = governed_world(pool).await;
    let (status, inv) = call(
        &app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/invitations"),
        &owner_token,
        Some(json!({ "email": "carol@example.com", "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &token_for("usr-carol", 1),
        Some(json!({ "token": inv["token"], "email": "other@example.com" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["message"], json!("email_mismatch"));
    // 未携带 email → V1 尽力而为放行（JWT 无 email claim）。
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &token_for("usr-carol", 1),
        Some(json!({ "token": inv["token"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn invitation_expiry_is_lazy_and_rejected(pool: PgPool) {
    let (app, db, tenant_id, owner_token) = governed_world(pool).await;
    let (status, inv) = call(
        &app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/invitations"),
        &owner_token,
        Some(json!({ "email": "dave@example.com", "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let token_text = inv["token"].as_str().unwrap();
    // 回拨有效期 → accept 命中惰性翻转。（CHECK(expires_at>created_at) 对
    // UPDATE 同样生效，故 created_at 一并回拨。）
    sqlx::query(
        "UPDATE core.tenant_invitation \
            SET expires_at = now() - interval '1 hour', \
                created_at = now() - interval '25 hours'",
    )
    .execute(&db)
    .await
    .unwrap();
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &token_for("usr-dave", 1),
        Some(json!({ "token": token_text })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["message"], json!("invitation_expired"));
    let flipped: (String,) = sqlx::query_as(
        "SELECT status FROM core.tenant_invitation WHERE email = 'dave@example.com'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(flipped.0, "EXPIRED");
}

#[sqlx::test(migrations = "../../migrations")]
async fn revoked_invitation_rejected(pool: PgPool) {
    let (app, _pool, tenant_id, owner_token) = governed_world(pool).await;
    let base = format!("/v1/pm/tenants/{tenant_id}/invitations");
    let (status, inv) = call(
        &app,
        "POST",
        &base,
        &owner_token,
        Some(json!({ "email": "erin@example.com", "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let inv_id = inv["id"].as_str().unwrap();
    let (status, _) = call(
        &app,
        "POST",
        &format!("{base}/{inv_id}/revoke"),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // 撤销后再撤销 → 409 not_pending。
    let (status, body) = call(
        &app,
        "POST",
        &format!("{base}/{inv_id}/revoke"),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    // 接受已撤销邀请 → 409 invitation_revoked。
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &token_for("usr-erin", 1),
        Some(json!({ "token": inv["token"] })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["message"], json!("invitation_revoked"));
}

// ────────────────────────────────────────────────────── 邀请链接（TDM-010/011）

#[sqlx::test(migrations = "../../migrations")]
async fn invitation_link_quota_and_idempotency(pool: PgPool) {
    let (app, _pool, tenant_id, owner_token) = governed_world(pool).await;
    let base = format!("/v1/pm/tenants/{tenant_id}/invitation-links");

    let (status, link) = call(
        &app,
        "POST",
        &base,
        &owner_token,
        Some(json!({ "max_uses": 2, "expires_at": null })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{link}");
    let token_text = link["token"].as_str().unwrap().to_string();

    // 两个新用户依次接受 → 两次 joined（V1 链接只发 member）。
    for sub in ["usr-l1", "usr-l2"] {
        let (status, body) = call(
            &app,
            "POST",
            "/v1/pm/invitation-links/accept",
            &token_for(sub, 1),
            Some(json!({ "token": token_text })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["outcome"], json!("joined"));
        assert_eq!(body["role"], json!("member"));
    }

    // 已加入用户重复提交 → 幂等 already_member，不消耗名额。
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitation-links/accept",
        &token_for("usr-l1", 1),
        Some(json!({ "token": token_text })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], json!("already_member"));

    // 第三个用户 → 名额耗尽。
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitation-links/accept",
        &token_for("usr-l3", 1),
        Some(json!({ "token": token_text })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["message"], json!("invitation_link_exhausted"));

    // 列表显示 EXHAUSTED 计算态。
    let (status, list) = call(&app, "GET", &base, &owner_token, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list[0]["status"], json!("EXHAUSTED"));
    assert_eq!(list[0]["used_count"], json!(2));

    // default_role 只允许 member（禁止经链接产生 ADMIN/OWNER）。
    let (status, _) = call(
        &app,
        "POST",
        &base,
        &owner_token,
        Some(json!({ "default_role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn revoked_link_rejected(pool: PgPool) {
    let (app, _pool, tenant_id, owner_token) = governed_world(pool).await;
    let base = format!("/v1/pm/tenants/{tenant_id}/invitation-links");
    let (status, link) = call(&app, "POST", &base, &owner_token, Some(json!({}))).await;
    assert_eq!(status, StatusCode::CREATED, "{link}");
    let link_id = link["id"].as_str().unwrap();
    let (status, _) = call(
        &app,
        "POST",
        &format!("{base}/{link_id}/revoke"),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitation-links/accept",
        &token_for("usr-lr", 1),
        Some(json!({ "token": link["token"] })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["message"], json!("invitation_revoked"));
}

// ────────────────────────────────────────────────────── 权限矩阵与跨租户（TDM-015）

#[sqlx::test(migrations = "../../migrations")]
async fn member_is_readonly_on_team_admin_surface(pool: PgPool) {
    let (app, _pool, tenant_id, owner_token) = governed_world(pool).await;

    // member（激活）读档案 OK，写 403。
    let member_token = token_for("usr-m", 1);
    let _ = call(&app, "GET", "/v1/pm/me", &member_token, None).await; // provision
                                                                       // 激活 member：owner 直接授权（既有端点）。
    let m = me(&app, &member_token).await;
    let member_id = Uuid::parse_str(m["principal_id"].as_str().unwrap()).unwrap();
    let (status, _) = call(
        &app,
        "PATCH",
        &format!("/v1/pm/principals/{member_id}"),
        &owner_token,
        Some(json!({ "is_active": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = call(
        &app,
        "GET",
        &format!("/v1/pm/tenants/{tenant_id}"),
        &member_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &app,
        "PATCH",
        &format!("/v1/pm/tenants/{tenant_id}"),
        &member_token,
        Some(json!({ "name": "Renamed" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/invitations"),
        &member_token,
        Some(json!({ "email": "x@example.com", "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &app,
        "GET",
        &format!("/v1/pm/tenants/{tenant_id}/invitations"),
        &member_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_tenant_access_is_404_and_accept_is_403(pool: PgPool) {
    let (app, _pool, tenant_id, owner_token) = governed_world(pool).await;

    // 跨租户 id 一律 404（guard_tenant 口径）。
    let (status, _) = call(
        &app,
        "GET",
        &format!("/v1/pm/tenants/{}/invitations", Uuid::new_v4()),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // tid=2 租户：先认领建团，再以其 owner 在自己租户建邀请。
    let (status, _) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &token_for("usr-foreign-owner", 2),
        Some(json!({ "name": "Team Two", "slug": "team-two" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let owner2 = me(&app, &token_for("usr-foreign-owner", 2)).await;
    let tenant2 = Uuid::parse_str(owner2["tenant_id"].as_str().unwrap()).unwrap();
    let (status, inv) = call(
        &app,
        "POST",
        &format!("/v1/pm/tenants/{tenant2}/invitations"),
        &token_for("usr-foreign-owner", 2),
        Some(json!({ "email": "zz@example.com", "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{inv}");

    // tid=1 的账号尝试接受 tid=2 的邀请 → 403 mismatch。
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &token_for("usr-bob", 1),
        Some(json!({ "token": inv["token"] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["message"], json!("invitation_tenant_mismatch"));
    let _ = tenant_id; // 主租户未被触碰
}

#[sqlx::test(migrations = "../../migrations")]
async fn team_profile_update_and_audit_rows(pool: PgPool) {
    let (app, db, tenant_id, owner_token) = governed_world(pool).await;
    let (status, _) = call(
        &app,
        "PATCH",
        &format!("/v1/pm/tenants/{tenant_id}"),
        &owner_token,
        Some(json!({ "name": "Acme Renamed", "slug": "acme" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (name,): (String,) = sqlx::query_as("SELECT display_name FROM core.tenant WHERE id = $1")
        .bind(tenant_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(name, "Acme Renamed");

    // 审计四列口径：team.updated（first-admin 路径的租户是镜像而来，无
    // team.created——认领建团的审计在 create_team 用例里断言）。
    let rows: Vec<(String, String, Uuid, Option<Uuid>, Option<String>)> = sqlx::query_as(
        "SELECT action, target_type, actor_id, actor_identity_id, actor_auth_mode \
           FROM governance.tenant_audit_log WHERE tenant_id = $1 ORDER BY created_at",
    )
    .bind(tenant_id)
    .fetch_all(&db)
    .await
    .unwrap();
    let actions: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    assert!(actions.contains(&"team.updated"), "{actions:?}");
    for row in rows.iter().filter(|r| r.0 == "team.updated") {
        assert_eq!(row.1, "tenant");
        assert!(!row.2.is_nil(), "actor_id must be present");
        assert!(row.4.is_some(), "actor_auth_mode must be present");
    }
}

// ────────────────────────────────────────────────── 解散团队（Owner 独占）

/// 已治理租户 + 一个 demo 项目，供解散级联断言。
async fn world_with_project(pool: PgPool) -> (Router, PgPool, Uuid, Uuid, String, Uuid, String) {
    let (app, db, tenant_id, owner_token) = governed_world(pool).await;
    // 项目经 bootstrap 建成（T-022：调用者自动占 owner 槽）。
    let owner = me(&app, &owner_token).await;
    let owner_id = Uuid::parse_str(owner["principal_id"].as_str().unwrap()).unwrap();
    let (status, project) = call(
        &app,
        "POST",
        "/v1/pm/projects",
        &owner_token,
        Some(json!({ "code": "DOOMED", "name": "Doomed" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{project}");
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();
    (
        app,
        db,
        tenant_id,
        project_id,
        owner_token,
        owner_id,
        project["code"].as_str().unwrap().to_string(),
    )
}

#[sqlx::test(migrations = "../../migrations")]
async fn dissolve_is_owner_exclusive(pool: PgPool) {
    let (app, _db, tenant_id, _project, owner_token, owner_id, _code) =
        world_with_project(pool).await;

    // 激活一个 admin 与一个 member。
    for sub in ["usr-admin", "usr-member"] {
        let m = me(&app, &token_for(sub, 1)).await;
        let id = Uuid::parse_str(m["principal_id"].as_str().unwrap()).unwrap();
        let (status, _) = call(
            &app,
            "PATCH",
            &format!("/v1/pm/principals/{id}"),
            &owner_token,
            Some(json!({ "is_active": true })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    let admin = me(&app, &token_for("usr-admin", 1)).await;
    let admin_id = Uuid::parse_str(admin["principal_id"].as_str().unwrap()).unwrap();
    let (status, _) = call(
        &app,
        "PATCH",
        &format!("/v1/pm/tenants/{tenant_id}/members/{admin_id}/role"),
        &owner_token,
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // admin 与 member 均不可解散（Owner 独占，实时查库）。
    let (status, body) = call(
        &app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/dissolve"),
        &token_for("usr-admin", 1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["message"], json!("team owner required"));
    let (status, _) = call(
        &app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/dissolve"),
        &token_for("usr-member", 1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Owner 解散 → 200，返回级联计数。
    let (status, body) = call(
        &app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/dissolve"),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["projects_deleted"], json!(1));
    let _ = owner_id;
}

#[sqlx::test(migrations = "../../migrations")]
async fn dissolve_cascades_and_revives_on_reclaim(pool: PgPool) {
    let (app, db, tenant_id, project_id, owner_token, _owner_id, code) =
        world_with_project(pool).await;

    let (status, _) = call(
        &app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/dissolve"),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // 项目终态软删 + 成员行清空。
    let (pstatus, pmembers): (String, i64) = sqlx::query_as(
        "SELECT status, (SELECT count(*) FROM core.project_member m WHERE m.project_id = p.id) \
           FROM core.project p WHERE p.id = $1",
    )
    .bind(project_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(pstatus, "deleted");
    assert_eq!(pmembers, 0);

    // Owner 自己的项目列表与项目可见性均失。
    let (status, list) = call(&app, "GET", "/v1/pm/projects", &owner_token, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["items"].as_array().unwrap().len(), 0, "{list}");
    let (status, _) = call(
        &app,
        "GET",
        &format!("/v1/pm/projects/{project_id}/members"),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "软删项目对任何人 404");

    // 团队面清算 + 租户休眠；/me 回到 onboarding 门。
    let invites: i64 =
        sqlx::query_scalar("SELECT count(*) FROM core.tenant_invitation WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(invites, 0);
    let members_left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM core.tenant_member WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(members_left, 0);
    let now = me(&app, &owner_token).await;
    assert_eq!(now["team"], serde_json::Value::Null);
    assert_eq!(now["onboarding_required"], json!(true));
    assert_eq!(now["memberships"], json!([]));

    // 重复解散 → 409 team_already_dissolved。
    let (status, body) = call(
        &app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/dissolve"),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["message"], json!("team_already_dissolved"));

    // 原 member（principal 仍 active）认领重建 → 新 Owner、租户复活、
    // slug 已释放可复用；旧项目保持 deleted 不可见。
    let (status, team) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &token_for("usr-reclaimer", 1),
        Some(json!({ "name": "Phoenix Team", "slug": code.to_lowercase() + "-reborn" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{team}");
    let reborn = me(&app, &token_for("usr-reclaimer", 1)).await;
    assert_eq!(reborn["tenant_role"], json!("owner"));
    assert_eq!(reborn["onboarding_required"], json!(false));
    let (status, list) = call(
        &app,
        "GET",
        "/v1/pm/projects",
        &token_for("usr-reclaimer", 1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        list["items"].as_array().unwrap().len(),
        0,
        "旧软删项目不可见"
    );

    // 审计：team.dissolved 落行。
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.tenant_audit_log WHERE action = 'team.dissolved'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(audits, 1);
}
