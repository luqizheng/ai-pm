//! Team Onboarding 验收（设计稿 `docs/team-onboarding/`，产品裁决 2026-11-06；
//! 映射 team-domain-model 数据库设计稿 §15 TDM-001~018 的可落地子集）。
//!
//! **P4-ORG Phase 2 改写登记（2026-11-20）**：Team 面锚切换 `core.team`/team_id——
//! /me `tenant_role`→`team_role`（值域 owner|member）、团队档案/邀请/链接/成员
//! 全部改 `/v1/pm/teams/{team_id}` 路径（原 /v1/pm/tenants/{tid} 路径删除）、
//! admin 角色取消（邀请/添加只发 member，Owner 经 PATCH role 任命）、
//! owner-transfer 路由删除（404）、first-admin provision 只落 tenant_member
//! 预登记行不再是 Team Owner（governed_world 改为 POST /v1/pm/teams 建团）、
//! 审计 team.created/updated 的 target_type="team"；解散（dissolve）API 取消，
//! 其用例删除；新增成员管理契约面用例（add/role 值域、ALREADY_MEMBER、
//! member not found、同值幂等、owner-transfer 404）。
//!
//! 覆盖：建团认领（creator=owner / 已治理租户 409 / slug 冲突）、邮箱邀请全
//! 生命周期（建/接受/重复接受/已成员幂等/过期/撤销/PENDING 防重/email 比对）、
//! 邀请链接（重复使用幂等/名额消耗/触顶 EXHAUSTED/撤销）、权限矩阵（member
//! 只读 403）、跨租户 404/403、/me onboarding_required 跃迁、审计四列落地、
//! 成员管理契约（400 role must be member / 409 ALREADY_MEMBER / 404 member
//! not found）。
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

/// 已治理租户（owner=usr-owner 经建团认领产生）。first-admin provision 只落
/// core.tenant_member 预登记行（principal inactive、非 Team Owner），故 owner
/// 必须先 POST /v1/pm/teams 认领建团；team_id 从 /me memberships[0].team_id 取。
async fn governed_world(pool: PgPool) -> (Router, PgPool, Uuid, Uuid, String) {
    std::env::set_var("AI_PM_SECRET_KEY", "team-onboarding-test-secret");
    let config = AuthConfig {
        first_admin_tenant: Some("1".to_string()),
        first_admin_subject: Some("usr-owner".to_string()),
    };
    let app = router_with_auth_config(pool.clone(), test_jwks(), config);
    let owner_token = token_for("usr-owner", 1);
    let _ = me(&app, &owner_token).await; // provision（inactive，无 Team）
    let (status, team) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &owner_token,
        Some(json!({ "name": "Acme Team" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{team}");
    let team_id = Uuid::parse_str(team["id"].as_str().unwrap()).unwrap();
    let owner = me(&app, &owner_token).await;
    let tenant_id = Uuid::parse_str(owner["tenant_id"].as_str().unwrap()).unwrap();
    assert_eq!(
        owner["memberships"][0]["team_id"],
        json!(team_id.to_string()),
        "memberships 锚 = core.team.id"
    );
    (app, pool, tenant_id, team_id, owner_token)
}

/// 「激活并加成员」两步（P4-ORG Phase 2）：POST members（owner token）→
/// PATCH principals 激活（owner token）。返回成员 token 与 principal_id。
async fn add_active_member(
    app: &Router,
    team_id: Uuid,
    owner_token: &str,
    sub: &str,
) -> (String, Uuid) {
    let token = token_for(sub, 1);
    let id = Uuid::parse_str(me(app, &token).await["principal_id"].as_str().unwrap()).unwrap();
    let (status, body) = call(
        app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/members"),
        owner_token,
        Some(json!({ "principal_id": id, "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "add {sub}: {body}");
    let (status, body) = call(
        app,
        "PATCH",
        &format!("/v1/pm/principals/{id}"),
        owner_token,
        Some(json!({ "is_active": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "activate {sub}: {body}");
    (token, id)
}

// ────────────────────────────────────────────────────── 建团认领（TDM-001/002/016）

#[sqlx::test(migrations = "../../migrations")]
async fn fresh_principal_is_onboarding_required(pool: PgPool) {
    let app = fresh_app(pool).await;
    let me_body = me(&app, &token_for("usr-new", 1)).await;
    assert_eq!(me_body["onboarding_required"], json!(true));
    assert_eq!(me_body["is_active"], json!(false));
    assert_eq!(me_body["team_role"], json!("member"), "无成员行 = 最小权限");
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
    let team_id = Uuid::parse_str(team["id"].as_str().unwrap()).unwrap();

    let me_body = me(&app, &t).await;
    assert_eq!(me_body["onboarding_required"], json!(false));
    assert_eq!(me_body["is_active"], json!(true));
    assert_eq!(
        me_body["team_role"],
        json!("owner"),
        "Q2：创建者自动 Owner"
    );
    assert_eq!(me_body["is_admin"], json!(true));
    assert_eq!(me_body["team"]["id"], json!(team_id.to_string()));
    assert_eq!(me_body["team"]["name"], json!("Acme R&D"));
    assert_eq!(
        me_body["memberships"][0]["team_id"],
        json!(team_id.to_string())
    );
    assert_eq!(me_body["memberships"][0]["role"], json!("owner"));

    // 认领建团落审计（team.created，target = core.team 行）。
    let audits: Vec<(String, String, Uuid)> = sqlx::query_as(
        "SELECT action, target_type, target_id FROM governance.tenant_audit_log \
          WHERE action = 'team.created'",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(audits.len(), 1, "{audits:?}");
    assert_eq!(audits[0].1, "team");
    assert_eq!(audits[0].2, team_id);
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_team_rejected_when_tenant_already_governed(pool: PgPool) {
    let (app, _pool, _tenant, _team, _owner_token) = governed_world(pool).await;

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
    let (app, db, tenant_id, team_id, owner_token) = governed_world(pool).await;
    let base = format!("/v1/pm/teams/{team_id}/invitations");

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

    // 邀请只发 member（DR-ORG-02：Owner 只能经 PATCH role 任命产生）。
    for role in ["admin", "owner"] {
        let (status, body) = call(
            &app,
            "POST",
            &base,
            &owner_token,
            Some(json!({ "email": "boss@example.com", "role": role })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["message"], json!("invitation role must be member"));
    }

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
    assert_eq!(bob["team_role"], json!("member"));
    assert_eq!(bob["memberships"][0]["team_id"], json!(team_id.to_string()));
    assert_eq!(bob["memberships"][0]["role"], json!("member"));

    // 接受落 core.team_member（member 行，租户锚 = team 的租户）。
    let bob_id = Uuid::parse_str(bob["principal_id"].as_str().unwrap()).unwrap();
    let (row_role,): (String,) = sqlx::query_as(
        "SELECT tm.role FROM core.team_member tm \
          JOIN core.team t ON t.id = tm.team_id \
         WHERE t.tenant_id = $1 AND tm.principal_id = $2",
    )
    .bind(tenant_id)
    .bind(bob_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(row_role, "member");

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
    let (app, _pool, _tenant, team_id, owner_token) = governed_world(pool).await;
    let (status, inv) = call(
        &app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/invitations"),
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
    let (app, db, _tenant, _team, owner_token) = governed_world(pool).await;
    let (status, inv) = call(
        &app,
        "POST",
        &format!("/v1/pm/teams/{_team}/invitations"),
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
    let (app, _pool, _tenant, team_id, owner_token) = governed_world(pool).await;
    let base = format!("/v1/pm/teams/{team_id}/invitations");
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
    let (app, _pool, _tenant, team_id, owner_token) = governed_world(pool).await;
    let base = format!("/v1/pm/teams/{team_id}/invitation-links");

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
    let (app, _pool, _tenant, team_id, owner_token) = governed_world(pool).await;
    let base = format!("/v1/pm/teams/{team_id}/invitation-links");
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
    let (app, _pool, _tenant, team_id, owner_token) = governed_world(pool).await;
    let base = format!("/v1/pm/teams/{team_id}");

    // member 经两步加团激活（POST members → PATCH principals）。
    let (member_token, _member_id) =
        add_active_member(&app, team_id, &owner_token, "usr-m").await;

    // member 读档案/名册 OK，写 403（Owner 独占）。
    let (status, _) = call(&app, "GET", &base, &member_token, None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&app, "GET", &format!("{base}/members"), &member_token, None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = call(
        &app,
        "PATCH",
        &base,
        &member_token,
        Some(json!({ "name": "Renamed" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["message"], json!("team owner required"));
    for (m, uri, body) in [
        (
            "POST",
            format!("{base}/invitations"),
            Some(json!({ "email": "x@example.com", "role": "member" })),
        ),
        ("GET", format!("{base}/invitations"), None),
        ("POST", format!("{base}/invitation-links"), Some(json!({}))),
        ("GET", format!("{base}/invitation-links"), None),
        ("GET", format!("{base}/members/{_member_id}"), None),
    ] {
        let (status, resp) = call(&app, m, &uri, &member_token, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "member {m} {uri}: {resp}");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_tenant_access_is_404_and_accept_is_403(pool: PgPool) {
    let (app, _pool, _tenant, _team, owner_token) = governed_world(pool).await;

    // 跨 team id 一律 404（guard_team 口径，不泄露存在性）。
    let (status, body) = call(
        &app,
        "GET",
        &format!("/v1/pm/teams/{}", Uuid::new_v4()),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["message"], json!("team not found"));

    // tid=2 租户：先认领建团，再以其 owner 在自己 team 建邀请。
    let foreign_token = token_for("usr-foreign-owner", 2);
    let (status, _) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &foreign_token,
        Some(json!({ "name": "Team Two", "slug": "team-two" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let team2 = Uuid::parse_str(
        me(&app, &foreign_token).await["memberships"][0]["team_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let (status, inv) = call(
        &app,
        "POST",
        &format!("/v1/pm/teams/{team2}/invitations"),
        &foreign_token,
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
}

#[sqlx::test(migrations = "../../migrations")]
async fn team_profile_update_and_audit_rows(pool: PgPool) {
    let (app, db, tenant_id, team_id, owner_token) = governed_world(pool).await;
    let (status, team) = call(
        &app,
        "PATCH",
        &format!("/v1/pm/teams/{team_id}"),
        &owner_token,
        Some(json!({ "name": "Acme Renamed", "slug": "acme" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{team}");
    assert_eq!(team["name"], json!("Acme Renamed"));
    assert_eq!(team["slug"], json!("acme"));

    // PATCH 更新 core.team.name/description + tenant.slug。
    let (name,): (String,) = sqlx::query_as("SELECT name FROM core.team WHERE id = $1")
        .bind(team_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(name, "Acme Renamed");
    let (slug,): (Option<String>,) =
        sqlx::query_as("SELECT slug FROM core.tenant WHERE id = $1")
            .bind(tenant_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(slug.as_deref(), Some("acme"));

    // 审计四列口径：team.created / team.updated 的 target = team 行
    // （governed_world 走认领建团，两条都在）。
    type Row = (String, String, Uuid, Uuid, Option<Uuid>, Option<String>);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT action, target_type, target_id, actor_id, actor_identity_id, actor_auth_mode \
           FROM governance.tenant_audit_log WHERE tenant_id = $1 ORDER BY created_at",
    )
    .bind(tenant_id)
    .fetch_all(&db)
    .await
    .unwrap();
    let actions: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    assert!(actions.contains(&"team.updated"), "{actions:?}");
    assert!(actions.contains(&"team.created"), "{actions:?}");
    for row in rows
        .iter()
        .filter(|r| r.0 == "team.created" || r.0 == "team.updated")
    {
        assert_eq!(row.1, "team", "target_type = team");
        assert_eq!(row.2, team_id, "target_id = core.team.id");
        assert!(!row.3.is_nil(), "actor_id must be present");
        assert!(row.5.is_some(), "actor_auth_mode must be present");
    }
}

// ────────────────────────────────────────────── 成员管理契约面（P4-ORG Phase 2 新增）

#[sqlx::test(migrations = "../../migrations")]
async fn member_admin_surface_contract(pool: PgPool) {
    let (app, _pool, _tenant, team_id, owner_token) = governed_world(pool).await;
    let base = format!("/v1/pm/teams/{team_id}/members");
    let m = me(&app, &token_for("usr-contract", 1)).await;
    let member_id = Uuid::parse_str(m["principal_id"].as_str().unwrap()).unwrap();

    // 添加值域：role 只接受 member（owner 经任命产生）。
    for role in ["admin", "owner"] {
        let (status, body) = call(
            &app,
            "POST",
            &base,
            &owner_token,
            Some(json!({ "principal_id": member_id, "role": role })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["message"], json!("team role must be member"));
    }
    let (status, body) = call(
        &app,
        "POST",
        &base,
        &owner_token,
        Some(json!({ "principal_id": member_id, "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["role"], json!("member"));

    // 已是成员 → 409 ALREADY_MEMBER。
    let (status, body) = call(
        &app,
        "POST",
        &base,
        &owner_token,
        Some(json!({ "principal_id": member_id, "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], json!("ALREADY_MEMBER"));

    // PATCH role 值域 owner|member；admin → 400。
    let role_uri = format!("{base}/{member_id}/role");
    let (status, body) = call(
        &app,
        "PATCH",
        &role_uri,
        &owner_token,
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["message"], json!("team role must be owner or member"));

    // 同值幂等 → 200。
    let (status, _) = call(
        &app,
        "PATCH",
        &role_uri,
        &owner_token,
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // 目标非成员 → 404 member not found（读/写皆然）。
    let stranger = Uuid::new_v4();
    let (status, body) = call(&app, "GET", &format!("{base}/{stranger}"), &owner_token, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["message"], json!("member not found"));
    let (status, body) = call(
        &app,
        "PATCH",
        &format!("{base}/{stranger}/role"),
        &owner_token,
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["message"], json!("member not found"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn owner_transfer_route_is_removed(pool: PgPool) {
    let (app, _pool, tenant_id, team_id, owner_token) = governed_world(pool).await;
    // P4-ORG：owner-transfer 取消（多 Owner 任命制替代），原路由 404。
    let (status, _) = call(
        &app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/owner-transfer"),
        &owner_token,
        Some(json!({ "new_owner_principal_id": Uuid::new_v4() })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/owner-transfer"),
        &owner_token,
        Some(json!({ "new_owner_principal_id": Uuid::new_v4() })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
