//! Team Onboarding QA 补强套件（`test-case/team-qa-test-cases-v1.md` 的代码级证据）。
//!
//! **P4-ORG Phase 2 改写登记（2026-11-20）**：Team 面锚切换 `core.team`/team_id，
//! 全部路径改 `/v1/pm/teams/{team_id}`；admin 角色取消后——B02-12（admin 邀请）
//! 改写为「member 邀请落 team_member 行 + team_member.activated 审计」、
//! B02-18（admin 授 admin，原待裁决）改写为「PATCH role 任命第二 Owner +
//! role_changed 审计 + 第二 Owner 同享管理面」；B02-13/14 的提权面随角色值域
//! 收敛为 member 单值（B02-14 由待裁决改为常规断言）；B02-19 改写为现状契约
//! （EXPIRED 二次接受 = invitation_already_accepted）；B04-01 admin 矩阵改写为
//! Owner 正向矩阵 + owner-transfer 404；成员角色查询改 core.team_member。
//! 新增覆盖：member 自助退出（leave 成功 + 可再入团）、任命第二 Owner 后原
//! Owner 可降级、移除/退出级联撤销 project_member、项目 owner 依赖
//! （PROJECT_OWNER_DEPENDENCY 拦截移除与退出）。
//!
//! 与 `team_onboarding.rs`（16 例）、`team_members.rs` 互补，只覆盖既有用例
//! 未触达的面：并发（TDM-011/认领竞争/同邀请竞争）、错误契约未覆盖分支、
//! member 权限矩阵、跨租户写面、被移除成员的残留访问、DB 终态（token 只存
//! 哈希、TTL、名额计数）。
//!
//! 用例名前缀 = 用例 ID（`qa_b03_02_*` ↔ TEAM-QA-B03-02）。
//! harness 与 `team_onboarding.rs` 同款（自签 JWT + `router_with_auth_config`）。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::auth::AuthConfig;
use pm_api::routes::router_with_auth_config;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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

fn request(method: &str, uri: &str, token: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
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
        .oneshot(request(method, uri, Some(token), body))
        .await
        .unwrap();
    let status = response.status();
    (status, json_of(response).await)
}

async fn call_anon(app: &Router, method: &str, uri: &str, body: Option<Value>) -> StatusCode {
    app.clone()
        .oneshot(request(method, uri, None, body))
        .await
        .unwrap()
        .status()
}

async fn fresh_app(pool: PgPool) -> Router {
    router_with_auth_config(pool, test_jwks(), AuthConfig::default())
}

async fn me(app: &Router, token: &str) -> Value {
    let (status, body) = call(app, "GET", "/v1/pm/me", token, None).await;
    assert_eq!(status, StatusCode::OK, "me must stay reachable: {body}");
    body
}

fn uuid_of(v: &Value) -> Uuid {
    Uuid::parse_str(v.as_str().expect("uuid string")).unwrap()
}

/// 已治理租户（owner=usr-owner 经建团认领产生），同 `team_onboarding.rs` 口径。
/// 返回 tenant_id 与 team_id（= /me memberships[0].team_id）。
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
    let tenant_id = uuid_of(&owner["tenant_id"]);
    (app, pool, tenant_id, team_id, owner_token)
}

/// 「加成员并激活」两步（Phase 2）：POST members（owner）→ PATCH principals
/// 激活（owner）。返回成员 token 与 principal_id。
async fn add_active_member(
    app: &Router,
    team_id: Uuid,
    owner_token: &str,
    sub: &str,
) -> (String, Uuid) {
    let token = token_for(sub, 1);
    let id = uuid_of(&me(app, &token).await["principal_id"]);
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

/// 任命 Owner（PATCH members/{pid}/role = owner，多 Owner 任命制）。
async fn promote_owner(app: &Router, team_id: Uuid, owner_token: &str, principal_id: Uuid) {
    let (status, body) = call(
        app,
        "PATCH",
        &format!("/v1/pm/teams/{team_id}/members/{principal_id}/role"),
        owner_token,
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "promote {principal_id}: {body}");
}

async fn invite(app: &Router, team_id: Uuid, token: &str, email: &str) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/invitations"),
        token,
        Some(json!({ "email": email, "role": "member" })),
    )
    .await
}

async fn db_role(db: &PgPool, team_id: Uuid, principal_id: Uuid) -> Option<String> {
    sqlx::query_scalar(
        "SELECT role FROM core.team_member WHERE team_id = $1 AND principal_id = $2",
    )
    .bind(team_id)
    .bind(principal_id)
    .fetch_optional(db)
    .await
    .unwrap()
}

// ═══════════════════════════════════════════════════════════ B01 建团/认领

/// TEAM-QA-B01-07：同租户两人并发认领 → 恰好一个 201，另一个 409；库内只有 1 个 owner。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b01_07_concurrent_claim_has_exactly_one_winner(pool: PgPool) {
    let db = pool.clone();
    let app = fresh_app(pool).await;
    let (ta, tb) = (token_for("usr-race-a", 1), token_for("usr-race-b", 1));
    // 先 provision 两个主体，避免并发落在 provision 本身。
    me(&app, &ta).await;
    me(&app, &tb).await;

    let fa = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &ta,
        Some(json!({ "name": "Race A", "slug": "race-a" })),
    );
    let fb = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &tb,
        Some(json!({ "name": "Race B", "slug": "race-b" })),
    );
    let ((sa, ba), (sb, bb)) = tokio::join!(fa, fb);
    let mut statuses = vec![sa, sb];
    statuses.sort();
    assert_eq!(
        statuses,
        vec![StatusCode::CREATED, StatusCode::CONFLICT],
        "a={ba} b={bb}"
    );
    let loser = if sa == StatusCode::CONFLICT { ba } else { bb };
    assert_eq!(loser["message"], json!("team_already_exists"));

    let tenant_id = uuid_of(&me(&app, &ta).await["tenant_id"]);
    let owners: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.team_member tm \
           JOIN core.team t ON t.id = tm.team_id \
          WHERE t.tenant_id = $1 AND tm.role = 'owner'",
    )
    .bind(tenant_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(owners, 1, "并发认领不得产生两个 owner");
    let created: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.tenant_audit_log WHERE action = 'team.created'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(created, 1, "输家不得留下审计/档案");
}

/// TEAM-QA-B01-08：名称/slug 边界（名称 1..=120 字符、slug 小写字母数字连字符、首字符非连字符）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b01_08_name_and_slug_boundaries(pool: PgPool) {
    let app = fresh_app(pool).await;
    let cases: Vec<(i64, Value, StatusCode)> = vec![
        (
            11,
            json!({ "name": "x".repeat(121) }),
            StatusCode::BAD_REQUEST,
        ),
        (
            12,
            json!({ "name": "A", "slug": "Upper" }),
            StatusCode::BAD_REQUEST,
        ),
        (
            13,
            json!({ "name": "A", "slug": "-lead" }),
            StatusCode::BAD_REQUEST,
        ),
        (
            14,
            json!({ "name": "A", "slug": "has_underscore" }),
            StatusCode::BAD_REQUEST,
        ),
        (
            15,
            json!({ "name": "A", "slug": "a".repeat(101) }),
            StatusCode::BAD_REQUEST,
        ),
        (
            16,
            json!({ "name": "中文团队名", "slug": "" }),
            StatusCode::CREATED,
        ),
        (
            17,
            json!({ "name": "y".repeat(120), "slug": "0-ok-slug" }),
            StatusCode::CREATED,
        ),
        (
            18,
            json!({ "name": "Z", "slug": "b".repeat(99) }),
            StatusCode::CREATED,
        ),
    ];
    for (tid, body, expected) in cases {
        let (status, resp) = call(
            &app,
            "POST",
            "/v1/pm/teams",
            &token_for(&format!("usr-b{tid}"), tid),
            Some(body.clone()),
        )
        .await;
        assert_eq!(status, expected, "tid={tid} body={body} resp={resp}");
    }
    // 空串 slug 落库为 NULL（未认领语义不受影响）。
    let t = token_for("usr-b16", 16);
    assert_eq!(me(&app, &t).await["team"]["slug"], Value::Null);
}

/// TEAM-QA-B01-09：建团必须携带 bearer（自助路径不等于公开路径）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b01_09_create_team_requires_bearer(pool: PgPool) {
    let app = fresh_app(pool).await;
    let status = call_anon(
        &app,
        "POST",
        "/v1/pm/teams",
        Some(json!({ "name": "Anon" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    for path in [
        "/v1/pm/invitations/preview?token=x",
        "/v1/pm/invitation-links/preview?token=x",
    ] {
        assert_eq!(
            call_anon(&app, "GET", path, None).await,
            StatusCode::UNAUTHORIZED,
            "{path}"
        );
    }
}

// ═══════════════════════════════════════════════════════════ B02 邮箱邀请

/// TEAM-QA-B02-09：邮箱格式校验。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_09_invalid_email_rejected(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    for email in [
        "no-at-sign",
        "a@nodot",
        "@example.com",
        "has space@example.com",
        "",
    ] {
        let (status, body) = invite(&app, team_id, &owner, email).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "email={email:?} {body}");
    }
}

/// TEAM-QA-B02-10：明文 token 不落库（只存 SHA-256），默认有效期 7 天。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_10_token_hashed_and_default_ttl(pool: PgPool) {
    let (app, db, _tenant, team_id, owner) = governed_world(pool).await;
    let (status, inv) = invite(&app, team_id, &owner, "hash@example.com").await;
    assert_eq!(status, StatusCode::CREATED, "{inv}");
    let token = inv["token"].as_str().unwrap();
    assert_eq!(token.len(), 64, "256-bit hex token");

    let (stored, ttl_hours): (String, f64) = sqlx::query_as(
        "SELECT token_hash, EXTRACT(EPOCH FROM (expires_at - created_at))::float8 / 3600 \
           FROM core.tenant_invitation WHERE email_normalized = 'hash@example.com'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    let expected: String = Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(stored, expected, "库内只存 SHA-256");
    assert_ne!(stored, token);
    assert!(
        (ttl_hours - 168.0).abs() < 0.1,
        "默认 TTL 7 天，实际 {ttl_hours}h"
    );

    // 列表不回显 token。
    let (_, list) = call(
        &app,
        "GET",
        &format!("/v1/pm/teams/{team_id}/invitations"),
        &owner,
        None,
    )
    .await;
    assert!(!list.to_string().contains(token), "列表泄露明文 token");
}

/// TEAM-QA-B02-11：未知 token 的 preview / accept → 404 invitation_not_found。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_11_unknown_token_is_404(pool: PgPool) {
    let (app, _db, _tenant, _team, _owner) = governed_world(pool).await;
    let t = token_for("usr-stranger", 1);
    let (status, body) = call(
        &app,
        "GET",
        "/v1/pm/invitations/preview?token=deadbeef",
        &t,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &t,
        Some(json!({ "token": "deadbeef" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["message"], json!("invitation_not_found"));
}

/// TEAM-QA-B02-12（Phase 2 改写，原 admin 邀请面随 admin 角色取消）：
/// owner 邀请 member → 接受后落 core.team_member member 行，邀请单
/// ACCEPTED + accepted_principal_id，审计 team_member.activated
/// （target_type="principal"）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_12_owner_invites_member_lands_team_member_row(pool: PgPool) {
    let (app, db, _tenant, team_id, owner) = governed_world(pool).await;
    let (status, inv) = invite(&app, team_id, &owner, "ada@example.com").await;
    assert_eq!(status, StatusCode::CREATED, "{inv}");
    let ada = token_for("usr-ada", 1);
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &ada,
        Some(json!({ "token": inv["token"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ada_id = uuid_of(&me(&app, &ada).await["principal_id"]);
    assert_eq!(db_role(&db, team_id, ada_id).await.as_deref(), Some("member"));
    let (st, accepted_by): (String, Option<Uuid>) = sqlx::query_as(
        "SELECT status, accepted_principal_id FROM core.tenant_invitation \
          WHERE email_normalized = 'ada@example.com'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(st, "ACCEPTED");
    assert_eq!(accepted_by, Some(ada_id));

    // 审计：接受 = team_member.activated，target = principal 行。
    let (action, target_type): (String, String) = sqlx::query_as(
        "SELECT action, target_type FROM governance.tenant_audit_log \
          WHERE action = 'team_member.activated' AND target_id = $1",
    )
    .bind(ada_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(action, "team_member.activated");
    assert_eq!(target_type, "principal");
}

/// TEAM-QA-B02-13（Phase 2 改写：角色值域收敛 member，无「更高角色邀请」面）：
/// 已是成员者接受新邀请 → already_member，不产生第二行（无重复成员关系）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_13_already_member_accept_does_not_change_role(pool: PgPool) {
    let (app, db, _tenant, team_id, owner) = governed_world(pool).await;
    let (mem, mem_id) = add_active_member(&app, team_id, &owner, "usr-mem").await;
    let (_, inv) = invite(&app, team_id, &owner, "mem@example.com").await;
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &mem,
        Some(json!({ "token": inv["token"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], json!("already_member"));
    assert_eq!(db_role(&db, team_id, mem_id).await.as_deref(), Some("member"));
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.team_member WHERE team_id = $1 AND principal_id = $2",
    )
    .bind(team_id)
    .bind(mem_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(rows, 1, "不重复建成员关系");
}

/// TEAM-QA-B02-14（Phase 2 改写为常规断言：角色值域单值 member，邀请单角色
/// 恒等于库内实际角色，原「回显不一致」待裁决项失效）：already_member 分支
/// 响应 role = member。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_14_already_member_response_role_reflects_actual_role(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    let (mem, _) = add_active_member(&app, team_id, &owner, "usr-mem").await;
    let (_, inv) = invite(&app, team_id, &owner, "mem@example.com").await;
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &mem,
        Some(json!({ "token": inv["token"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["role"], json!("member"), "{body}");
}

/// TEAM-QA-B02-15：团队休眠（is_active=false）时接受邀请 → 403 team_suspended。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_15_suspended_team_rejects_accept(pool: PgPool) {
    let (app, db, tenant_id, _team, owner) = governed_world(pool).await;
    let (_, inv) = invite(&app, _team, &owner, "sus@example.com").await;
    sqlx::query("UPDATE core.tenant SET is_active = false WHERE id = $1")
        .bind(tenant_id)
        .execute(&db)
        .await
        .unwrap();
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &token_for("usr-sus", 1),
        Some(json!({ "token": inv["token"] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["message"], json!("team_suspended"));
}

/// TEAM-QA-B02-16：撤销面——未知 id 404、member 403（Owner 独占）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_16_revoke_guards(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    let base = format!("/v1/pm/teams/{team_id}/invitations");
    let (status, _) = call(
        &app,
        "POST",
        &format!("{base}/{}/revoke", Uuid::new_v4()),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, inv) = invite(&app, team_id, &owner, "rv@example.com").await;
    let (mem, _) = add_active_member(&app, team_id, &owner, "usr-mem").await;
    let (status, body) = call(
        &app,
        "POST",
        &format!("{base}/{}/revoke", inv["id"].as_str().unwrap()),
        &mem,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["message"], json!("team owner required"));
}

/// TEAM-QA-B02-17：同一张邀请两人并发接受 → 恰好一人 joined，另一人 409。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_17_concurrent_accept_same_invitation(pool: PgPool) {
    let (app, db, _tenant, team_id, owner) = governed_world(pool).await;
    let (_, inv) = invite(&app, team_id, &owner, "dup@example.com").await;
    let (t1, t2) = (token_for("usr-dup1", 1), token_for("usr-dup2", 1));
    me(&app, &t1).await;
    me(&app, &t2).await;
    let body = json!({ "token": inv["token"] });
    let ((s1, b1), (s2, b2)) = tokio::join!(
        call(
            &app,
            "POST",
            "/v1/pm/invitations/accept",
            &t1,
            Some(body.clone())
        ),
        call(
            &app,
            "POST",
            "/v1/pm/invitations/accept",
            &t2,
            Some(body.clone())
        ),
    );
    let mut statuses = vec![s1, s2];
    statuses.sort();
    assert_eq!(
        statuses,
        vec![StatusCode::OK, StatusCode::CONFLICT],
        "b1={b1} b2={b2}"
    );
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.team_member tm \
           JOIN core.principal p ON p.id = tm.principal_id \
           JOIN core.principal_identity pi ON pi.principal_id = p.id \
          WHERE tm.team_id = $1 AND pi.subject IN ('usr-dup1','usr-dup2') AND p.is_active",
    )
    .bind(team_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(active, 1, "一张邀请只能激活一人");
}

/// TEAM-QA-B02-18（Phase 2 改写，原「admin 不得授 admin」随 admin 角色取消）：
/// Owner 经 PATCH role=owner 任命第二 Owner（role_changed 审计 before/after），
/// 第二 Owner 同享管理面（可邀请、可撤销）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_18_second_owner_appointment_shares_owner_surface(pool: PgPool) {
    let (app, db, _tenant, team_id, owner) = governed_world(pool).await;
    let (co_token, co_id) = add_active_member(&app, team_id, &owner, "usr-co").await;

    // 任命第二 Owner（member → owner 同端点）。
    promote_owner(&app, team_id, &owner, co_id).await;
    let owners: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.team_member WHERE team_id = $1 AND role = 'owner'",
    )
    .bind(team_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(owners, 2, "任命后应有两位 Owner");

    // 审计：role_changed（before=member / after=owner）。
    let (before, after): (Value, Value) = sqlx::query_as(
        "SELECT before_data, after_data FROM governance.tenant_audit_log \
          WHERE action = 'team_member.role_changed' AND target_principal_id = $1 \
          ORDER BY created_at DESC LIMIT 1",
    )
    .bind(co_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(before["role"], json!("member"));
    assert_eq!(after["role"], json!("owner"));

    // 第二 Owner 同享管理面。
    let (status, _) = call(
        &app,
        "GET",
        &format!("/v1/pm/teams/{team_id}/invitations"),
        &co_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, inv) = invite(&app, team_id, &co_token, "peer@example.com").await;
    assert_eq!(status, StatusCode::CREATED, "{inv}");
    let (status, _) = call(
        &app,
        "POST",
        &format!(
            "/v1/pm/teams/{team_id}/invitations/{}/revoke",
            inv["id"].as_str().unwrap()
        ),
        &co_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

/// TEAM-QA-B02-19（Phase 2 改写为现状契约）：EXPIRED 邀请首次接受报
/// invitation_expired，二次接受命中状态闸（EXPIRED 与 ACCEPTED 同路终结）
/// → 409 invitation_already_accepted。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_19_expired_reaccept_reports_already_accepted(pool: PgPool) {
    let (app, db, _tenant, team_id, owner) = governed_world(pool).await;
    let (_, inv) = invite(&app, team_id, &owner, "late@example.com").await;
    sqlx::query(
        "UPDATE core.tenant_invitation SET expires_at = now() - interval '1 hour', \
                created_at = now() - interval '25 hours'",
    )
    .execute(&db)
    .await
    .unwrap();
    let t = token_for("usr-late", 1);
    let body = json!({ "token": inv["token"] });
    let (status, first) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &t,
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{first}");
    assert_eq!(first["message"], json!("invitation_expired"));
    let (status, second) = call(&app, "POST", "/v1/pm/invitations/accept", &t, Some(body)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{second}");
    assert_eq!(second["message"], json!("invitation_already_accepted"));
}

// ═══════════════════════════════════════════════════════════ B03 邀请链接

async fn create_link(app: &Router, team_id: Uuid, token: &str, body: Value) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/invitation-links"),
        token,
        Some(body),
    )
    .await
}

/// TEAM-QA-B03-06（TDM-011）：max_uses=3，10 人并发领用 → 恰好 3 joined、7 exhausted，计数 = 3。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b03_06_concurrent_redeem_never_exceeds_quota(pool: PgPool) {
    let (app, db, _tenant, team_id, owner) = governed_world(pool).await;
    let (status, link) = create_link(&app, team_id, &owner, json!({ "max_uses": 3 })).await;
    assert_eq!(status, StatusCode::CREATED, "{link}");
    let tokens: Vec<String> = (0..10)
        .map(|i| token_for(&format!("usr-rush-{i}"), 1))
        .collect();
    for t in &tokens {
        me(&app, t).await; // 先 provision，竞争只落在名额上
    }
    let body = json!({ "token": link["token"] });
    let mut set = tokio::task::JoinSet::new();
    for t in tokens {
        let (app, body) = (app.clone(), body.clone());
        set.spawn(async move {
            call(
                &app,
                "POST",
                "/v1/pm/invitation-links/accept",
                &t,
                Some(body),
            )
            .await
        });
    }
    let results: Vec<(StatusCode, Value)> = set.join_all().await;
    let joined = results
        .iter()
        .filter(|(s, b)| *s == StatusCode::OK && b["outcome"] == "joined")
        .count();
    let exhausted = results
        .iter()
        .filter(|(s, b)| *s == StatusCode::CONFLICT && b["message"] == "invitation_link_exhausted")
        .count();
    assert_eq!((joined, exhausted), (3, 7), "{results:?}");

    let (used, st, redemptions): (i32, String, i64) = sqlx::query_as(
        "SELECT used_count, status, (SELECT count(*) FROM core.tenant_invitation_link_redemption r \
                                       WHERE r.link_id = l.id) \
           FROM core.tenant_invitation_link l WHERE team_id = $1",
    )
    .bind(team_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!((used, st.as_str(), redemptions), (3, "EXHAUSTED", 3));
}

/// TEAM-QA-B03-07：链接参数校验（max_uses ≤0、过去的 expires_at）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b03_07_link_validation(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    for body in [
        json!({ "max_uses": 0 }),
        json!({ "max_uses": -1 }),
        json!({ "expires_at": "2000-01-01T00:00:00Z" }),
        json!({ "default_role": "owner" }),
    ] {
        let (status, resp) = create_link(&app, team_id, &owner, body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body} → {resp}");
    }
}

/// TEAM-QA-B03-08：链接过期惰性翻转 → 409 invitation_expired，库内 EXPIRED。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b03_08_link_expiry_is_lazy(pool: PgPool) {
    let (app, db, _tenant, team_id, owner) = governed_world(pool).await;
    let (_, link) = create_link(&app, team_id, &owner, json!({})).await;
    sqlx::query(
        "UPDATE core.tenant_invitation_link SET expires_at = now() - interval '1 hour', \
                created_at = now() - interval '25 hours'",
    )
    .execute(&db)
    .await
    .unwrap();
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitation-links/accept",
        &token_for("usr-tardy", 1),
        Some(json!({ "token": link["token"] })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["message"], json!("invitation_expired"));
    let st: String = sqlx::query_scalar("SELECT status FROM core.tenant_invitation_link")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(st, "EXPIRED");
}

/// TEAM-QA-B03-09：不限次数链接 + preview 余量。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b03_09_unlimited_link_and_preview(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    let (_, unlimited) = create_link(&app, team_id, &owner, json!({})).await;
    for i in 0..5 {
        let (status, body) = call(
            &app,
            "POST",
            "/v1/pm/invitation-links/accept",
            &token_for(&format!("usr-u{i}"), 1),
            Some(json!({ "token": unlimited["token"] })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let (_, capped) = create_link(&app, team_id, &owner, json!({ "max_uses": 2 })).await;
    let viewer = token_for("usr-viewer", 1);
    let preview_uri = format!(
        "/v1/pm/invitation-links/preview?token={}",
        capped["token"].as_str().unwrap()
    );
    let (status, p) = call(&app, "GET", &preview_uri, &viewer, None).await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["remaining_uses"], json!(2));
    call(
        &app,
        "POST",
        "/v1/pm/invitation-links/accept",
        &viewer,
        Some(json!({ "token": capped["token"] })),
    )
    .await;
    let (_, p) = call(&app, "GET", &preview_uri, &viewer, None).await;
    assert_eq!(p["remaining_uses"], json!(1), "{p}");
    let (status, _) = call(
        &app,
        "GET",
        "/v1/pm/invitation-links/preview?token=nope",
        &viewer,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// TEAM-QA-B03-10：链接撤销守卫——二次撤销 409 link_not_active、member 403。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b03_10_link_revoke_guards(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    let base = format!("/v1/pm/teams/{team_id}/invitation-links");
    let (_, link) = create_link(&app, team_id, &owner, json!({})).await;
    let id = link["id"].as_str().unwrap();
    let (mem, _) = add_active_member(&app, team_id, &owner, "usr-mem").await;
    let (status, _) = call(&app, "POST", &format!("{base}/{id}/revoke"), &mem, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(&app, "POST", &format!("{base}/{id}/revoke"), &owner, None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = call(&app, "POST", &format!("{base}/{id}/revoke"), &owner, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["message"], json!("link_not_active"));
}

/// TEAM-QA-B03-11：跨租户领用链接 → 403 invitation_tenant_mismatch。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b03_11_cross_tenant_link_redeem_is_403(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    let (_, link) = create_link(&app, team_id, &owner, json!({})).await;
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitation-links/accept",
        &token_for("usr-outsider", 2),
        Some(json!({ "token": link["token"] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["message"], json!("invitation_tenant_mismatch"));
}

/// TEAM-QA-B03-12：owner 领用自己团队的链接 → already_member，库内仍 owner，名额不消耗。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b03_12_owner_redeeming_link_keeps_owner(pool: PgPool) {
    let (app, db, _tenant, team_id, owner) = governed_world(pool).await;
    let owner_id = uuid_of(&me(&app, &owner).await["principal_id"]);
    let (_, link) = create_link(&app, team_id, &owner, json!({ "max_uses": 1 })).await;
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/invitation-links/accept",
        &owner,
        Some(json!({ "token": link["token"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], json!("already_member"));
    assert_eq!(
        db_role(&db, team_id, owner_id).await.as_deref(),
        Some("owner")
    );
    let used: i32 = sqlx::query_scalar("SELECT used_count FROM core.tenant_invitation_link")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(used, 0);
}

// ═══════════════════════════════════════════════════════════ B04 权限矩阵

/// TEAM-QA-B04-01（Phase 2 改写，原 admin 矩阵随 admin 角色取消）：Team Owner
/// 管理面全绿（档案/邀请/链接/名册/详情）；owner-transfer 路由已删除 → 404。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b04_01_owner_management_surface(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    let (_, mem_id) = add_active_member(&app, team_id, &owner, "usr-mem").await;
    let t = format!("/v1/pm/teams/{team_id}");

    let allowed: Vec<(&str, String, Option<Value>, StatusCode)> = vec![
        (
            "PATCH",
            t.clone(),
            Some(json!({ "description": "by owner" })),
            StatusCode::OK,
        ),
        ("GET", format!("{t}/invitations"), None, StatusCode::OK),
        (
            "POST",
            format!("{t}/invitations"),
            Some(json!({ "email": "n@example.com", "role": "member" })),
            StatusCode::CREATED,
        ),
        ("GET", format!("{t}/invitation-links"), None, StatusCode::OK),
        (
            "POST",
            format!("{t}/invitation-links"),
            Some(json!({})),
            StatusCode::CREATED,
        ),
        ("GET", format!("{t}/members"), None, StatusCode::OK),
        ("GET", format!("{t}/members/{mem_id}"), None, StatusCode::OK),
    ];
    for (m, uri, body, expected) in allowed {
        let (status, resp) = call(&app, m, &uri, &owner, body).await;
        assert_eq!(status, expected, "owner {m} {uri}: {resp}");
    }
    let (status, _) = call(
        &app,
        "POST",
        &format!("{t}/owner-transfer"),
        &owner,
        Some(json!({ "new_owner_principal_id": mem_id })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "owner-transfer 已取消");
}

/// TEAM-QA-B04-02：member 只读——名册/档案可读，其余管理面一律 403。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b04_02_member_matrix(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    let (mem, _) = add_active_member(&app, team_id, &owner, "usr-mem").await;
    let (_, other_id) = add_active_member(&app, team_id, &owner, "usr-other").await;
    let t = format!("/v1/pm/teams/{team_id}");

    for uri in [t.clone(), format!("{t}/members")] {
        let (status, _) = call(&app, "GET", &uri, &mem, None).await;
        assert_eq!(status, StatusCode::OK, "member GET {uri}");
    }
    let denied: Vec<(&str, String, Option<Value>)> = vec![
        ("GET", format!("{t}/members/{other_id}"), None),
        ("GET", format!("{t}/invitations"), None),
        (
            "POST",
            format!("{t}/invitations"),
            Some(json!({ "email": "n@example.com", "role": "member" })),
        ),
        ("GET", format!("{t}/invitation-links"), None),
        ("POST", format!("{t}/invitation-links"), Some(json!({}))),
        ("DELETE", format!("{t}/members/{other_id}"), None),
        (
            "PATCH",
            format!("{t}/members/{other_id}/role"),
            Some(json!({ "role": "owner" })),
        ),
        (
            "POST",
            format!("{t}/members"),
            Some(json!({ "principal_id": other_id, "role": "member" })),
        ),
    ];
    for (m, uri, body) in denied {
        let (status, resp) = call(&app, m, &uri, &mem, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "member {m} {uri}: {resp}");
    }
    // 角色值域校验先于权限闸：member PATCH role="admin" → 400。
    let (status, body) = call(
        &app,
        "PATCH",
        &format!("{t}/members/{other_id}/role"),
        &mem,
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["message"], json!("team role must be owner or member"));
}

/// TEAM-QA-B04-03：未激活主体只能走自助路径；团队面 403。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b04_03_inactive_principal_limited_to_self_service(pool: PgPool) {
    let (app, _db, _tenant, team_id, _owner) = governed_world(pool).await;
    let idle = token_for("usr-idle", 1);
    let m = me(&app, &idle).await;
    assert_eq!(m["is_active"], json!(false));
    for uri in [
        format!("/v1/pm/teams/{team_id}"),
        format!("/v1/pm/teams/{team_id}/members"),
    ] {
        let (status, body) = call(&app, "GET", &uri, &idle, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}: {body}");
    }
}

// ═══════════════════════════════════════════════════════════ B05 名册与成员

/// TEAM-QA-B05-01：唯一 owner 不可被移除/降级/退出（OWNER_LAST 三闸），
/// 自我任命 → ALREADY_OWNER。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b05_01_sole_owner_cannot_be_removed_demoted_or_leave(pool: PgPool) {
    let (app, db, _tenant, team_id, owner) = governed_world(pool).await;
    let owner_id = uuid_of(&me(&app, &owner).await["principal_id"]);
    let mbase = format!("/v1/pm/teams/{team_id}/members/{owner_id}");

    let (status, body) = call(&app, "DELETE", &mbase, &owner, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], json!("OWNER_LAST"));
    assert_eq!(body["message"], json!("the last owner cannot be removed"));

    let (status, body) = call(
        &app,
        "PATCH",
        &format!("{mbase}/role"),
        &owner,
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], json!("OWNER_LAST"));
    assert_eq!(body["message"], json!("the last owner cannot be demoted"));

    // 自助退出同样被 OWNER_LAST 拦截。
    let (status, body) = call(
        &app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/leave"),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], json!("OWNER_LAST"));
    assert_eq!(body["message"], json!("the last owner cannot leave"));

    // 给自己升 owner → 409 ALREADY_OWNER（已是 owner，无意义）。
    let (status, body) = call(
        &app,
        "PATCH",
        &format!("{mbase}/role"),
        &owner,
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], json!("ALREADY_OWNER"));

    let owners: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.team_member WHERE team_id = $1 AND role = 'owner'",
    )
    .bind(team_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(owners, 1, "库内必须仍有 owner");
}

/// TEAM-QA-B05-02：被移除的成员必须失去团队可见性（设计稿 §6.3 Verify Active Membership）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b05_02_removed_member_loses_team_visibility(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    let (mem, mem_id) = add_active_member(&app, team_id, &owner, "usr-gone").await;
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/v1/pm/teams/{team_id}/members/{mem_id}"),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = call(
        &app,
        "GET",
        &format!("/v1/pm/teams/{team_id}/members"),
        &mem,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "被移除者仍可读名册：{status} {body}");
    assert_eq!(body["message"], json!("not an active member"));
}

/// TEAM-QA-B05-03：被移除成员的 /me 必须自洽——无 membership 时应回到 onboarding 门。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b05_03_removed_member_me_is_consistent(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    let (mem, mem_id) = add_active_member(&app, team_id, &owner, "usr-gone").await;
    call(
        &app,
        "DELETE",
        &format!("/v1/pm/teams/{team_id}/members/{mem_id}"),
        &owner,
        None,
    )
    .await;
    let m = me(&app, &mem).await;
    assert_eq!(m["memberships"], json!([]), "{m}");
    assert_eq!(
        m["onboarding_required"],
        json!(true),
        "无成员关系却不进 onboarding：{m}"
    );
}

/// TEAM-QA-B05-04（Phase 2 新增）：member 自助退出 → 200 left=true，回到
/// onboarding 门，审计 team_member.left；之后可被 owner 再次添加。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b05_04_member_leave_succeeds_and_can_be_readded(pool: PgPool) {
    let (app, db, _tenant, team_id, owner) = governed_world(pool).await;
    let (mem, mem_id) = add_active_member(&app, team_id, &owner, "usr-leaver").await;

    let (status, body) = call(
        &app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/leave"),
        &mem,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["left"], json!(true));

    let m = me(&app, &mem).await;
    assert_eq!(m["memberships"], json!([]), "{m}");
    assert_eq!(m["onboarding_required"], json!(true));
    assert_eq!(m["team_role"], json!("member"));

    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.team_member WHERE team_id = $1 AND principal_id = $2",
    )
    .bind(team_id)
    .bind(mem_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(rows, 0, "退出即删 team_member 行");
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.tenant_audit_log \
          WHERE action = 'team_member.left' AND target_principal_id = $1",
    )
    .bind(mem_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(audits, 1, "退出落 team_member.left 审计");

    // owner 可再次添加（自助路径重新入团）。
    let (status, _) = call(
        &app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/members"),
        &owner,
        Some(json!({ "principal_id": mem_id, "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

/// TEAM-QA-B05-05（Phase 2 新增，owner-transfer 的多 Owner 任命制替代）：
/// 任命第二 Owner 后，原 Owner 可被降级为 member；降权即时生效（旧 JWT 下一次
/// 请求即失管理面），第二 Owner 保持完整管理面。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b05_05_original_owner_demotable_after_second_owner_appointed(pool: PgPool) {
    let (app, db, _tenant, team_id, owner) = governed_world(pool).await;
    let owner_id = uuid_of(&me(&app, &owner).await["principal_id"]);
    let (co_token, co_id) = add_active_member(&app, team_id, &owner, "usr-co").await;

    // 任命第二 Owner。
    promote_owner(&app, team_id, &owner, co_id).await;

    // 原 Owner 自降为 member（另一位 Owner 在场）→ 200。
    let (status, body) = call(
        &app,
        "PATCH",
        &format!("/v1/pm/teams/{team_id}/members/{owner_id}/role"),
        &owner,
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let owners: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.team_member WHERE team_id = $1 AND role = 'owner'",
    )
    .bind(team_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(owners, 1, "降级后剩一位 Owner");

    // 撤权即时生效：原 Owner 旧 JWT 下一次请求即失管理面。
    let now = me(&app, &owner).await;
    assert_eq!(now["is_admin"], json!(false));
    assert_eq!(now["team_role"], json!("member"));
    let (status, _) = call(
        &app,
        "GET",
        &format!("/v1/pm/teams/{team_id}/invitations"),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // 第二 Owner 保持完整管理面。
    let (status, _) = call(
        &app,
        "GET",
        &format!("/v1/pm/teams/{team_id}/invitations"),
        &co_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

/// TEAM-QA-B05-06（Phase 2 新增）：移除成员级联撤销其本租户 project_member 行
/// （审计 after_data 并入 project_memberships_revoked 计数）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b05_06_remove_member_cascades_project_memberships(pool: PgPool) {
    let (app, db, _tenant, team_id, owner) = governed_world(pool).await;
    let (_, mem_id) = add_active_member(&app, team_id, &owner, "usr-cascade").await;

    // 项目经 bootstrap 建成（T-022：调用者自动占 owner 槽）。
    let (status, project) = call(
        &app,
        "POST",
        "/v1/pm/projects",
        &owner,
        Some(json!({ "code": "CASCADE", "name": "Cascade" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{project}");
    let pid = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();
    let (status, body) = call(
        &app,
        "POST",
        &format!("/v1/pm/projects/{pid}/members"),
        &owner,
        Some(json!({ "principal_id": mem_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/v1/pm/teams/{team_id}/members/{mem_id}"),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let pms: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.project_member \
          WHERE tenant_id = (SELECT tenant_id FROM core.team WHERE id = $1) \
            AND principal_id = $2",
    )
    .bind(team_id)
    .bind(mem_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(pms, 0, "真人项目成员关系必须级联撤销");

    let after: Value = sqlx::query_scalar(
        "SELECT after_data FROM governance.tenant_audit_log \
          WHERE action = 'team_member.removed' AND target_principal_id = $1",
    )
    .bind(mem_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(after["project_memberships_revoked"], json!(1), "{after}");
}

/// TEAM-QA-B05-07（Phase 2 新增）：成员是某项目 owner 时，移除/退出被
/// PROJECT_OWNER_DEPENDENCY 拦截（须先转移项目所有权）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b05_07_project_owner_cannot_be_removed_or_leave(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    let owner_id = uuid_of(&me(&app, &owner).await["principal_id"]);
    let (status, project) = call(
        &app,
        "POST",
        "/v1/pm/projects",
        &owner,
        Some(json!({ "code": "OWNED", "name": "Owned" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{project}");
    let _pid = project["id"].as_str().unwrap().to_string();

    // 任命第二 Owner，避免 OWNER_LAST 先拦。
    let (_, co_id) = add_active_member(&app, team_id, &owner, "usr-co").await;
    promote_owner(&app, team_id, &owner, co_id).await;

    let (status, body) = call(
        &app,
        "DELETE",
        &format!("/v1/pm/teams/{team_id}/members/{owner_id}"),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], json!("PROJECT_OWNER_DEPENDENCY"));

    let (status, body) = call(
        &app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/leave"),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], json!("PROJECT_OWNER_DEPENDENCY"));
}

// ═══════════════════════════════════════════════════════════ B07 跨租户

/// TEAM-QA-B07-01：跨租户读写团队面一律 404，且对方数据不变。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b07_01_cross_tenant_team_surface_is_404(pool: PgPool) {
    let (app, db, tenant_id, team_id, _owner) = governed_world(pool).await;
    let foreign = token_for("usr-foreign", 2);
    let (status, _) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &foreign,
        Some(json!({ "name": "Foreign" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let t = format!("/v1/pm/teams/{team_id}");
    let probes: Vec<(&str, String, Option<Value>)> = vec![
        ("GET", t.clone(), None),
        ("PATCH", t.clone(), Some(json!({ "name": "Pwned" }))),
        ("GET", format!("{t}/members"), None),
        (
            "POST",
            format!("{t}/members"),
            Some(json!({ "principal_id": Uuid::new_v4(), "role": "member" })),
        ),
        (
            "POST",
            format!("{t}/invitations"),
            Some(json!({ "email": "x@example.com", "role": "member" })),
        ),
        ("POST", format!("{t}/invitation-links"), Some(json!({}))),
        ("POST", format!("{t}/leave"), None),
    ];
    for (m, uri, body) in probes {
        let (status, resp) = call(&app, m, &uri, &foreign, body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{m} {uri}: {resp}");
    }
    let name: String = sqlx::query_scalar("SELECT name FROM core.team WHERE id = $1")
        .bind(team_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_ne!(name, "Pwned");
    let invites: i64 =
        sqlx::query_scalar("SELECT count(*) FROM core.tenant_invitation WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(invites, 0);
}

// ═══════════════════════════════════════════════════════════ B08 /me

/// TEAM-QA-B08-01：/me 契约 shape（team/onboarding_required/memberships[]）随入团跃迁；
/// team.id 与 memberships[].team_id = core.team.id（不再是 tenant_id）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b08_01_me_contract_after_joining_by_link(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    let (_, link) = create_link(&app, team_id, &owner, json!({})).await;
    let t = token_for("usr-joiner", 1);
    let before = me(&app, &t).await;
    assert_eq!(before["onboarding_required"], json!(true));
    call(
        &app,
        "POST",
        "/v1/pm/invitation-links/accept",
        &t,
        Some(json!({ "token": link["token"] })),
    )
    .await;
    let after = me(&app, &t).await;
    assert_eq!(after["onboarding_required"], json!(false), "{after}");
    assert_eq!(after["team"]["id"], json!(team_id.to_string()));
    let ms = after["memberships"].as_array().unwrap();
    assert_eq!(ms.len(), 1);
    assert_eq!(ms[0]["team_id"], json!(team_id.to_string()));
    assert_eq!(ms[0]["role"], json!("member"));
    assert_eq!(ms[0]["status"], json!("active"));
}

/// TEAM-QA-B08-02：被停用（is_active=false）的成员回到 onboarding 门，管理面失效。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b08_02_deactivated_member_returns_to_onboarding(pool: PgPool) {
    let (app, _db, _tenant, team_id, owner) = governed_world(pool).await;
    let (mem, mem_id) = add_active_member(&app, team_id, &owner, "usr-paused").await;
    let (status, _) = call(
        &app,
        "PATCH",
        &format!("/v1/pm/principals/{mem_id}"),
        &owner,
        Some(json!({ "is_active": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let m = me(&app, &mem).await;
    assert_eq!(m["onboarding_required"], json!(true), "{m}");
    assert_eq!(m["memberships"], json!([]));
    let (status, _) = call(
        &app,
        "GET",
        &format!("/v1/pm/teams/{team_id}/members"),
        &mem,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
