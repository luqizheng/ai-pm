//! Team Onboarding QA 补强套件（`test-case/team-qa-test-cases-v1.md` 的代码级证据）。
//!
//! 与 `team_onboarding.rs`（16 例）、`tenant_members.rs`（rv1~rv10/tn4/tn5）互补，
//! 只覆盖既有用例未触达的面：并发（TDM-011/认领竞争/同邀请竞争）、错误契约
//! 未覆盖分支、admin/member 权限矩阵补全、解散后残留凭证、跨租户写面、
//! 被移除成员的残留访问、DB 终态（token 只存哈希、TTL、名额计数）。
//!
//! 用例名前缀 = 用例 ID（`qa_b03_02_*` ↔ TEAM-QA-B03-02）。标 `#[ignore]` 的是
//! **待产品裁决项**：断言按设计稿口径书写，现状与之不符但尚未裁定为缺陷，
//! 用 `cargo test -- --ignored` 单独复核。
//!
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

/// 已治理租户（owner=usr-owner），同 `team_onboarding.rs` 口径。
async fn governed_world(pool: PgPool) -> (Router, PgPool, Uuid, String) {
    std::env::set_var("AI_PM_SECRET_KEY", "team-onboarding-test-secret");
    let config = AuthConfig {
        first_admin_tenant: Some("1".to_string()),
        first_admin_subject: Some("usr-owner".to_string()),
    };
    let app = router_with_auth_config(pool.clone(), test_jwks(), config);
    let owner_token = token_for("usr-owner", 1);
    let owner = me(&app, &owner_token).await;
    let tenant_id = uuid_of(&owner["tenant_id"]);
    (app, pool, tenant_id, owner_token)
}

/// 以 owner 激活 `sub` 并（可选）设为 admin；返回其 token 与 principal_id。
async fn activate(
    app: &Router,
    tenant_id: Uuid,
    owner_token: &str,
    sub: &str,
    role: &str,
) -> (String, Uuid) {
    let token = token_for(sub, 1);
    let id = uuid_of(&me(app, &token).await["principal_id"]);
    let (status, body) = call(
        app,
        "PATCH",
        &format!("/v1/pm/principals/{id}"),
        owner_token,
        Some(json!({ "is_active": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "activate {sub}: {body}");
    if role != "member" {
        let (status, body) = call(
            app,
            "PATCH",
            &format!("/v1/pm/tenants/{tenant_id}/members/{id}/role"),
            owner_token,
            Some(json!({ "role": role })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "set role {sub}: {body}");
    }
    (token, id)
}

async fn invite(
    app: &Router,
    tenant_id: Uuid,
    token: &str,
    email: &str,
    role: &str,
) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/invitations"),
        token,
        Some(json!({ "email": email, "role": role })),
    )
    .await
}

async fn db_role(db: &PgPool, tenant_id: Uuid, principal_id: Uuid) -> Option<String> {
    sqlx::query_scalar(
        "SELECT role FROM core.tenant_member WHERE tenant_id = $1 AND principal_id = $2",
    )
    .bind(tenant_id)
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
        "SELECT count(*) FROM core.tenant_member WHERE tenant_id = $1 AND role = 'owner'",
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
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    for email in [
        "no-at-sign",
        "a@nodot",
        "@example.com",
        "has space@example.com",
        "",
    ] {
        let (status, body) = invite(&app, tenant_id, &owner, email, "member").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "email={email:?} {body}");
    }
}

/// TEAM-QA-B02-10：明文 token 不落库（只存 SHA-256），默认有效期 7 天。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_10_token_hashed_and_default_ttl(pool: PgPool) {
    let (app, db, tenant_id, owner) = governed_world(pool).await;
    let (status, inv) = invite(&app, tenant_id, &owner, "hash@example.com", "member").await;
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
        &format!("/v1/pm/tenants/{tenant_id}/invitations"),
        &owner,
        None,
    )
    .await;
    assert!(!list.to_string().contains(token), "列表泄露明文 token");
}

/// TEAM-QA-B02-11：未知 token 的 preview / accept → 404 invitation_not_found。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_11_unknown_token_is_404(pool: PgPool) {
    let (app, _db, _tenant, _owner) = governed_world(pool).await;
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

/// TEAM-QA-B02-12：owner 发 admin 邀请 → 接受后库内角色 = admin，邀请单 ACCEPTED。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_12_owner_invites_admin(pool: PgPool) {
    let (app, db, tenant_id, owner) = governed_world(pool).await;
    let (status, inv) = invite(&app, tenant_id, &owner, "ada@example.com", "admin").await;
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
    assert_eq!(
        db_role(&db, tenant_id, ada_id).await.as_deref(),
        Some("admin")
    );
    let (st, accepted_by): (String, Option<Uuid>) = sqlx::query_as(
        "SELECT status, accepted_principal_id FROM core.tenant_invitation WHERE email_normalized = 'ada@example.com'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(st, "ACCEPTED");
    assert_eq!(accepted_by, Some(ada_id));
}

/// TEAM-QA-B02-13：已是成员者接受更高角色的邀请 → already_member，**库内角色不变**（不得静默提权）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_13_already_member_accept_does_not_change_role(pool: PgPool) {
    let (app, db, tenant_id, owner) = governed_world(pool).await;
    let (mem, mem_id) = activate(&app, tenant_id, &owner, "usr-mem", "member").await;
    let (_, inv) = invite(&app, tenant_id, &owner, "mem@example.com", "admin").await;
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
    assert_eq!(
        db_role(&db, tenant_id, mem_id).await.as_deref(),
        Some("member")
    );
}

/// TEAM-QA-B02-14：响应体 role 应反映调用者实际角色（already_member 分支）。
/// 待裁决：现状回显的是邀请单角色（admin），与库内实际角色（member）不一致。
#[sqlx::test(migrations = "../../migrations")]
#[ignore = "待裁决 OBS：already_member 响应 role 回显邀请单角色而非实际角色"]
async fn qa_b02_14_already_member_response_role_reflects_actual_role(pool: PgPool) {
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let (mem, _) = activate(&app, tenant_id, &owner, "usr-mem", "member").await;
    let (_, inv) = invite(&app, tenant_id, &owner, "mem@example.com", "admin").await;
    let (_, body) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &mem,
        Some(json!({ "token": inv["token"] })),
    )
    .await;
    assert_eq!(body["role"], json!("member"), "{body}");
}

/// TEAM-QA-B02-15：团队休眠（is_active=false）时接受邀请 → 403 team_suspended。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_15_suspended_team_rejects_accept(pool: PgPool) {
    let (app, db, tenant_id, owner) = governed_world(pool).await;
    let (_, inv) = invite(&app, tenant_id, &owner, "sus@example.com", "member").await;
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

/// TEAM-QA-B02-16：撤销面——未知 id 404、member 403。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_16_revoke_guards(pool: PgPool) {
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let base = format!("/v1/pm/tenants/{tenant_id}/invitations");
    let (status, _) = call(
        &app,
        "POST",
        &format!("{base}/{}/revoke", Uuid::new_v4()),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, inv) = invite(&app, tenant_id, &owner, "rv@example.com", "member").await;
    let (mem, _) = activate(&app, tenant_id, &owner, "usr-mem", "member").await;
    let (status, _) = call(
        &app,
        "POST",
        &format!("{base}/{}/revoke", inv["id"].as_str().unwrap()),
        &mem,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// TEAM-QA-B02-17：同一张邀请两人并发接受 → 恰好一人 joined，另一人 409。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b02_17_concurrent_accept_same_invitation(pool: PgPool) {
    let (app, db, tenant_id, owner) = governed_world(pool).await;
    let (_, inv) = invite(&app, tenant_id, &owner, "dup@example.com", "member").await;
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
        "SELECT count(*) FROM core.tenant_member tm \
           JOIN core.principal p ON p.id = tm.principal_id \
           JOIN core.principal_identity pi ON pi.principal_id = p.id \
          WHERE tm.tenant_id = $1 AND pi.subject IN ('usr-dup1','usr-dup2') AND p.is_active",
    )
    .bind(tenant_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(active, 1, "一张邀请只能激活一人");
}

/// TEAM-QA-B02-18：admin 不得经邀请授予 admin（设计稿 §6.1 + tenant_members「admin cannot grant admin」）。
/// 待裁决：现状 admin 可创建 role=admin 邀请（201）。
#[sqlx::test(migrations = "../../migrations")]
#[ignore = "待裁决：admin 经邀请授予 admin 与直接授权口径不一致"]
async fn qa_b02_18_admin_cannot_invite_admin(pool: PgPool) {
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let (adm, _) = activate(&app, tenant_id, &owner, "usr-adm", "admin").await;
    let (status, body) = invite(&app, tenant_id, &adm, "peer@example.com", "admin").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}

/// TEAM-QA-B02-19：已持久化为 EXPIRED 的邀请再次接受 → 仍报 invitation_expired。
/// 待裁决：现状返回 invitation_already_accepted（前端会渲染成「已接受」）。
#[sqlx::test(migrations = "../../migrations")]
#[ignore = "待裁决：EXPIRED 二次接受返回 invitation_already_accepted"]
async fn qa_b02_19_expired_reaccept_still_reports_expired(pool: PgPool) {
    let (app, db, tenant_id, owner) = governed_world(pool).await;
    let (_, inv) = invite(&app, tenant_id, &owner, "late@example.com", "member").await;
    sqlx::query(
        "UPDATE core.tenant_invitation SET expires_at = now() - interval '1 hour', \
                created_at = now() - interval '25 hours'",
    )
    .execute(&db)
    .await
    .unwrap();
    let t = token_for("usr-late", 1);
    let body = json!({ "token": inv["token"] });
    let (_, first) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &t,
        Some(body.clone()),
    )
    .await;
    assert_eq!(first["message"], json!("invitation_expired"));
    let (_, second) = call(&app, "POST", "/v1/pm/invitations/accept", &t, Some(body)).await;
    assert_eq!(second["message"], json!("invitation_expired"), "{second}");
}

// ═══════════════════════════════════════════════════════════ B03 邀请链接

async fn create_link(
    app: &Router,
    tenant_id: Uuid,
    token: &str,
    body: Value,
) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/invitation-links"),
        token,
        Some(body),
    )
    .await
}

/// TEAM-QA-B03-06（TDM-011）：max_uses=3，10 人并发领用 → 恰好 3 joined、7 exhausted，计数 = 3。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b03_06_concurrent_redeem_never_exceeds_quota(pool: PgPool) {
    let (app, db, tenant_id, owner) = governed_world(pool).await;
    let (status, link) = create_link(&app, tenant_id, &owner, json!({ "max_uses": 3 })).await;
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
           FROM core.tenant_invitation_link l WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!((used, st.as_str(), redemptions), (3, "EXHAUSTED", 3));
}

/// TEAM-QA-B03-07：链接参数校验（max_uses ≤0、过去的 expires_at）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b03_07_link_validation(pool: PgPool) {
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    for body in [
        json!({ "max_uses": 0 }),
        json!({ "max_uses": -1 }),
        json!({ "expires_at": "2000-01-01T00:00:00Z" }),
        json!({ "default_role": "owner" }),
    ] {
        let (status, resp) = create_link(&app, tenant_id, &owner, body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body} → {resp}");
    }
}

/// TEAM-QA-B03-08：链接过期惰性翻转 → 409 invitation_expired，库内 EXPIRED。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b03_08_link_expiry_is_lazy(pool: PgPool) {
    let (app, db, tenant_id, owner) = governed_world(pool).await;
    let (_, link) = create_link(&app, tenant_id, &owner, json!({})).await;
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
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let (_, unlimited) = create_link(&app, tenant_id, &owner, json!({})).await;
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
    let (_, capped) = create_link(&app, tenant_id, &owner, json!({ "max_uses": 2 })).await;
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
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let base = format!("/v1/pm/tenants/{tenant_id}/invitation-links");
    let (_, link) = create_link(&app, tenant_id, &owner, json!({})).await;
    let id = link["id"].as_str().unwrap();
    let (mem, _) = activate(&app, tenant_id, &owner, "usr-mem", "member").await;
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
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let (_, link) = create_link(&app, tenant_id, &owner, json!({})).await;
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
    let (app, db, tenant_id, owner) = governed_world(pool).await;
    let owner_id = uuid_of(&me(&app, &owner).await["principal_id"]);
    let (_, link) = create_link(&app, tenant_id, &owner, json!({ "max_uses": 1 })).await;
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
        db_role(&db, tenant_id, owner_id).await.as_deref(),
        Some("owner")
    );
    let used: i32 = sqlx::query_scalar("SELECT used_count FROM core.tenant_invitation_link")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(used, 0);
}

// ═══════════════════════════════════════════════════════════ B04 权限矩阵

/// TEAM-QA-B04-01：admin 可管理团队面（档案/邀请/链接），不可解散、不可转让。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b04_01_admin_matrix(pool: PgPool) {
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let (adm, _) = activate(&app, tenant_id, &owner, "usr-adm", "admin").await;
    let (_, mem_id) = activate(&app, tenant_id, &owner, "usr-mem", "member").await;
    let t = format!("/v1/pm/tenants/{tenant_id}");

    let allowed: Vec<(&str, String, Option<Value>, StatusCode)> = vec![
        (
            "PATCH",
            t.clone(),
            Some(json!({ "description": "by admin" })),
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
        let (status, resp) = call(&app, m, &uri, &adm, body).await;
        assert_eq!(status, expected, "admin {m} {uri}: {resp}");
    }
    let (status, _) = call(&app, "POST", &format!("{t}/dissolve"), &adm, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "admin 不可解散");
    let (status, _) = call(
        &app,
        "POST",
        &format!("{t}/owner-transfer"),
        &adm,
        Some(json!({ "new_owner_principal_id": mem_id })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "admin 不可转让 owner");
}

/// TEAM-QA-B04-02：member 只读——名册/档案可读，其余管理面一律 403。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b04_02_member_matrix(pool: PgPool) {
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let (mem, _) = activate(&app, tenant_id, &owner, "usr-mem", "member").await;
    let (_, other_id) = activate(&app, tenant_id, &owner, "usr-other", "member").await;
    let t = format!("/v1/pm/tenants/{tenant_id}");

    for uri in [t.clone(), format!("{t}/members")] {
        let (status, _) = call(&app, "GET", &uri, &mem, None).await;
        assert_eq!(status, StatusCode::OK, "member GET {uri}");
    }
    let denied: Vec<(&str, String, Option<Value>)> = vec![
        ("GET", format!("{t}/members/{other_id}"), None),
        ("GET", format!("{t}/invitation-links"), None),
        ("POST", format!("{t}/invitation-links"), Some(json!({}))),
        ("DELETE", format!("{t}/members/{other_id}"), None),
        (
            "PATCH",
            format!("{t}/members/{other_id}/role"),
            Some(json!({ "role": "admin" })),
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
}

/// TEAM-QA-B04-03：未激活主体只能走自助路径；团队面 403。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b04_03_inactive_principal_limited_to_self_service(pool: PgPool) {
    let (app, _db, tenant_id, _owner) = governed_world(pool).await;
    let idle = token_for("usr-idle", 1);
    let m = me(&app, &idle).await;
    assert_eq!(m["is_active"], json!(false));
    for uri in [
        format!("/v1/pm/tenants/{tenant_id}"),
        format!("/v1/pm/tenants/{tenant_id}/members"),
    ] {
        let (status, body) = call(&app, "GET", &uri, &idle, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}: {body}");
    }
}

// ═══════════════════════════════════════════════════════════ B05 名册与成员

/// TEAM-QA-B05-01：唯一 owner 不能把自己移出团队（须先转让）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b05_01_sole_owner_cannot_remove_self(pool: PgPool) {
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let owner_id = uuid_of(&me(&app, &owner).await["principal_id"]);
    let (status, body) = call(
        &app,
        "DELETE",
        &format!("/v1/pm/tenants/{tenant_id}/members/{owner_id}"),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

/// TEAM-QA-B05-02：被移除的成员必须失去团队可见性（设计稿 §6.3 Verify Active Membership）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b05_02_removed_member_loses_team_visibility(pool: PgPool) {
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let (mem, mem_id) = activate(&app, tenant_id, &owner, "usr-gone", "member").await;
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/v1/pm/tenants/{tenant_id}/members/{mem_id}"),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = call(
        &app,
        "GET",
        &format!("/v1/pm/tenants/{tenant_id}/members"),
        &mem,
        None,
    )
    .await;
    assert!(
        status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND,
        "被移除者仍可读名册：{status} {body}"
    );
}

/// TEAM-QA-B05-03：被移除成员的 /me 必须自洽——无 membership 时应回到 onboarding 门。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b05_03_removed_member_me_is_consistent(pool: PgPool) {
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let (mem, mem_id) = activate(&app, tenant_id, &owner, "usr-gone", "member").await;
    call(
        &app,
        "DELETE",
        &format!("/v1/pm/tenants/{tenant_id}/members/{mem_id}"),
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

// ═══════════════════════════════════════════════════════════ B06 解散与重建

/// TEAM-QA-B06-01：解散后旧邀请/链接 token 全部失效（404），前成员回到 onboarding 门。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b06_01_dissolve_kills_outstanding_tokens(pool: PgPool) {
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let (mem, _) = activate(&app, tenant_id, &owner, "usr-mem", "member").await;
    let (_, inv) = invite(&app, tenant_id, &owner, "pending@example.com", "member").await;
    let (_, link) = create_link(&app, tenant_id, &owner, json!({})).await;

    let (status, body) = call(
        &app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/dissolve"),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["members_removed"], json!(2), "{body}");

    let newcomer = token_for("usr-after", 1);
    let (status, _) = call(
        &app,
        "POST",
        "/v1/pm/invitations/accept",
        &newcomer,
        Some(json!({ "token": inv["token"] })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &app,
        "POST",
        "/v1/pm/invitation-links/accept",
        &newcomer,
        Some(json!({ "token": link["token"] })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let m = me(&app, &mem).await;
    assert_eq!(m["onboarding_required"], json!(true), "{m}");
    assert_eq!(m["team"], Value::Null);
}

/// TEAM-QA-B06-02：解散释放 slug——重建可复用**同一** slug。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b06_02_reclaim_reuses_same_slug(pool: PgPool) {
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let (status, _) = call(
        &app,
        "PATCH",
        &format!("/v1/pm/tenants/{tenant_id}"),
        &owner,
        Some(json!({ "slug": "phoenix" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    call(
        &app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/dissolve"),
        &owner,
        None,
    )
    .await;
    let (status, body) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &owner,
        Some(json!({ "name": "Phoenix", "slug": "phoenix" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(me(&app, &owner).await["tenant_role"], json!("owner"));
}

/// TEAM-QA-B06-03：解散其他租户 → 404（不泄露存在性）。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b06_03_dissolve_foreign_tenant_is_404(pool: PgPool) {
    let (app, _db, tenant_id, _owner) = governed_world(pool).await;
    let foreign = token_for("usr-foreign", 2);
    call(
        &app,
        "POST",
        "/v1/pm/teams",
        &foreign,
        Some(json!({ "name": "Foreign" })),
    )
    .await;
    let (status, _) = call(
        &app,
        "POST",
        &format!("/v1/pm/tenants/{tenant_id}/dissolve"),
        &foreign,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ═══════════════════════════════════════════════════════════ B07 跨租户

/// TEAM-QA-B07-01：跨租户读写团队面一律 404，且对方数据不变。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b07_01_cross_tenant_team_surface_is_404(pool: PgPool) {
    let (app, db, tenant_id, _owner) = governed_world(pool).await;
    let foreign = token_for("usr-foreign", 2);
    call(
        &app,
        "POST",
        "/v1/pm/teams",
        &foreign,
        Some(json!({ "name": "Foreign" })),
    )
    .await;
    let t = format!("/v1/pm/tenants/{tenant_id}");
    let probes: Vec<(&str, String, Option<Value>)> = vec![
        ("GET", t.clone(), None),
        ("PATCH", t.clone(), Some(json!({ "name": "Pwned" }))),
        ("GET", format!("{t}/members"), None),
        (
            "POST",
            format!("{t}/invitations"),
            Some(json!({ "email": "x@example.com", "role": "admin" })),
        ),
        ("POST", format!("{t}/invitation-links"), Some(json!({}))),
    ];
    for (m, uri, body) in probes {
        let (status, resp) = call(&app, m, &uri, &foreign, body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{m} {uri}: {resp}");
    }
    let name: String = sqlx::query_scalar("SELECT display_name FROM core.tenant WHERE id = $1")
        .bind(tenant_id)
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

/// TEAM-QA-B08-01：/me 契约 shape（team/onboarding_required/memberships[]）随入团跃迁。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b08_01_me_contract_after_joining_by_link(pool: PgPool) {
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let (_, link) = create_link(&app, tenant_id, &owner, json!({})).await;
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
    assert_eq!(after["team"]["id"], json!(tenant_id.to_string()));
    let ms = after["memberships"].as_array().unwrap();
    assert_eq!(ms.len(), 1);
    assert_eq!(ms[0]["team_id"], json!(tenant_id.to_string()));
    assert_eq!(ms[0]["role"], json!("member"));
    assert_eq!(ms[0]["status"], json!("active"));
}

/// TEAM-QA-B08-02：被停用（is_active=false）的成员回到 onboarding 门，管理面失效。
#[sqlx::test(migrations = "../../migrations")]
async fn qa_b08_02_deactivated_member_returns_to_onboarding(pool: PgPool) {
    let (app, _db, tenant_id, owner) = governed_world(pool).await;
    let (mem, mem_id) = activate(&app, tenant_id, &owner, "usr-paused", "member").await;
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
        &format!("/v1/pm/tenants/{tenant_id}/members"),
        &mem,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
