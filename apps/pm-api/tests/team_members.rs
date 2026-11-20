//! Team 成员与角色管理验收（P4-ORG Phase 2，原 `tenant_members.rs` 改写）。
//!
//! 改写说明：路由 `/v1/pm/tenants/{tenant_id}/members` → `/v1/pm/teams/{team_id}/members`；
//! 角色收敛 owner|member 两值（admin 取消）；owner-transfer 删除改多 Owner 任命制
//! （PATCH role 双向）；新增自助退出 `POST /v1/pm/teams/{team_id}/leave`；
//! 事实源 `core.team_member.role`（每请求查库，撤权对同一 JWT 即时生效）。
//! 12 个用例**名沿用旧名**便于与旧清单对照，语义全部按新契约重写（各用例 doc 注明）。
//!
//! world 口径：first_admin provision 只落 `core.tenant_member` 预登记行（不再是
//! Team Owner）→ `POST /v1/pm/teams` 建团（创建者 = 首位 Owner）→ 加成员 +
//! 激活。审计在 `governance.tenant_audit_log`（action `team_member.*`）。

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

async fn me(app: &Router, token: &str) -> Value {
    let (status, body) = call(app, "GET", "/v1/pm/me", token, None).await;
    assert_eq!(status, StatusCode::OK, "me must stay reachable: {body}");
    body
}

/// Owner（`usr-owner`，首位 Team Owner）+ 两个 member（`usr-admin` / `usr-member`）。
struct World {
    app: Router,
    tenant_id: Uuid,
    team_id: Uuid,
    owner_id: Uuid,
    admin_id: Uuid,
    member_id: Uuid,
}

/// Owner 把某人加为 member（`POST /v1/pm/teams/{team_id}/members`）。
async fn add_member(
    app: &Router,
    team_id: Uuid,
    token: &str,
    principal_id: Uuid,
) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/members"),
        token,
        Some(json!({ "principal_id": principal_id, "role": "member" })),
    )
    .await
}

/// Owner 激活主体（`PATCH /v1/pm/principals/{pid}`）。
async fn activate(app: &Router, token: &str, principal_id: Uuid) {
    let (status, body) = call(
        app,
        "PATCH",
        &format!("/v1/pm/principals/{principal_id}"),
        token,
        Some(json!({ "is_active": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "activate {principal_id}: {body}");
}

/// Owner 任命第二 Owner（`PATCH .../members/{pid}/role {"role":"owner"}`）。
async fn appoint_owner(app: &Router, team_id: Uuid, token: &str, principal_id: Uuid) {
    let (status, body) = call(
        app,
        "PATCH",
        &format!("/v1/pm/teams/{team_id}/members/{principal_id}/role"),
        token,
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "appoint {principal_id}: {body}");
}

/// 搭一个「1 owner + 2 member」的 Team。
///
/// owner 走 provision（预登记行，还不是 Team Owner）→ 建团成为首位 Owner；
/// 另两把 token 首访 provision 成未激活主体，由 owner 加团并激活（member）。
async fn world(pool: &PgPool) -> World {
    let app = router_with_auth_config(pool.clone(), test_jwks(), first_admin("usr-owner"));
    let owner_token = token("usr-owner", json!([]));

    let owner = me(&app, &owner_token).await;
    let tenant_id = Uuid::parse_str(owner["tenant_id"].as_str().unwrap()).unwrap();
    let owner_id = Uuid::parse_str(owner["principal_id"].as_str().unwrap()).unwrap();
    assert_eq!(owner["is_active"], json!(true));
    assert_eq!(
        owner["is_admin"], json!(false),
        "provision 预登记行不再是 Team Owner"
    );
    assert_eq!(owner["team_role"], json!("member"));

    // 建团：创建者 = 首位 Team Owner；team_id 从 /me memberships[0].team_id 取。
    let (status, team) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &owner_token,
        Some(json!({ "name": "Alpha Team" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{team}");
    let after = me(&app, &owner_token).await;
    assert_eq!(after["is_admin"], json!(true), "建团后 owner 即刻进入管理面");
    assert_eq!(after["team_role"], json!("owner"));
    assert_eq!(after["memberships"][0]["role"], json!("owner"));
    assert_eq!(after["onboarding_required"], json!(false));
    let team_id = Uuid::parse_str(after["memberships"][0]["team_id"].as_str().unwrap()).unwrap();
    assert_eq!(
        team["id"].as_str().unwrap(),
        team_id.to_string(),
        "建团响应与 /me memberships 一致"
    );

    // 成员：首访 provision（inactive）→ owner 加团 → owner 激活。
    let admin = me(&app, &token("usr-admin", json!([]))).await;
    let admin_id = Uuid::parse_str(admin["principal_id"].as_str().unwrap()).unwrap();
    let member = me(&app, &token("usr-member", json!([]))).await;
    let member_id = Uuid::parse_str(member["principal_id"].as_str().unwrap()).unwrap();
    for id in [admin_id, member_id] {
        let (status, body) = add_member(&app, team_id, &owner_token, id).await;
        assert_eq!(status, StatusCode::OK, "add {id}: {body}");
        activate(&app, &owner_token, id).await;
    }

    World {
        app,
        tenant_id,
        team_id,
        owner_id,
        admin_id,
        member_id,
    }
}

/// 成员角色（契约 §8：`team_member JOIN team` 按租户定位）。
async fn member_role(pool: &PgPool, tenant_id: Uuid, principal_id: Uuid) -> Option<String> {
    sqlx::query_scalar(
        "SELECT tm.role FROM core.team_member tm \
          JOIN core.team t ON t.id = tm.team_id \
         WHERE t.tenant_id = $1 AND tm.principal_id = $2",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// 本 Team 的 owner 数。
async fn owner_count(pool: &PgPool, team_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM core.team_member WHERE team_id = $1 AND role = 'owner'")
        .bind(team_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ─────────────────────────────────────────────────────── TN-4 / TN-5

/// 任命第二 Owner（PATCH role=owner）一步完成：角色落库 + 新 Owner 即刻进管理面
/// + 审计 before member → after owner；同值重复 PATCH 200 幂等不落审计；
/// owner-transfer 路由已删除（404）。
#[sqlx::test(migrations = "../../migrations")]
async fn tn4_owner_transfer_swaps_roles_atomically_and_audits_twice(pool: PgPool) {
    let w = world(&pool).await;
    let owner_token = token("usr-owner", json!([]));

    // owner-transfer 已取消（多 Owner 任命制替代）。
    let (status, _) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/owner-transfer", w.tenant_id),
        &owner_token,
        Some(json!({ "new_owner_principal_id": w.admin_id })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // 任命：member → owner 单次 PATCH。
    appoint_owner(&w.app, w.team_id, &owner_token, w.admin_id).await;
    assert_eq!(
        member_role(&pool, w.tenant_id, w.admin_id).await.as_deref(),
        Some("owner")
    );
    assert_eq!(owner_count(&pool, w.team_id).await, 2);

    // 新 Owner 即刻获得管理面（/me is_admin 每请求查库）。
    let now = me(&w.app, &token("usr-admin", json!([]))).await;
    assert_eq!(now["is_admin"], json!(true));
    assert_eq!(now["team_role"], json!("owner"));
    assert_eq!(now["memberships"][0]["role"], json!("owner"));

    // 审计：team_member.role_changed，before member → after owner。
    let (action, before, after): (String, Value, Value) = sqlx::query_as(
        "SELECT action, before_data, after_data FROM governance.tenant_audit_log \
          WHERE tenant_id = $1 AND target_principal_id = $2 \
            AND action = 'team_member.role_changed' \
          ORDER BY created_at DESC LIMIT 1",
    )
    .bind(w.tenant_id)
    .bind(w.admin_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(action, "team_member.role_changed");
    assert_eq!(before["role"], json!("member"));
    assert_eq!(after["role"], json!("owner"));

    // 同值重复 PATCH → 200 幂等，不产生新审计。
    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/teams/{}/members/{}/role", w.team_id, w.admin_id),
        &owner_token,
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["role"], json!("owner"));
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.tenant_audit_log \
          WHERE tenant_id = $1 AND target_principal_id = $2 \
            AND action = 'team_member.role_changed'",
    )
    .bind(w.tenant_id)
    .bind(w.admin_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1, "同值幂等不落审计");
}

/// member 对任命/加团零权限（403 "team owner required"）；owner-transfer 404。
#[sqlx::test(migrations = "../../migrations")]
async fn tn5_admin_cannot_transfer_ownership(pool: PgPool) {
    let w = world(&pool).await;
    let member_token = token("usr-member", json!(["tenant_admin"]));

    // member 想任命自己为 owner → 403（token 角色不参与授权）。
    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/teams/{}/members/{}/role", w.team_id, w.member_id),
        &member_token,
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["message"], json!("team owner required"));

    // member 调 owner-transfer → 路由不存在。
    let (status, _) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/tenants/{}/owner-transfer", w.tenant_id),
        &member_token,
        Some(json!({ "new_owner_principal_id": w.member_id })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // 什么都没发生。
    assert_eq!(owner_count(&pool, w.team_id).await, 1);
    assert_eq!(
        member_role(&pool, w.tenant_id, w.member_id).await.as_deref(),
        Some("member")
    );
}

// ────────────────────────────────────────────── RV-1 / RV-2

/// 任命第二 Owner → 降级其一 → 同一 JWT 的下一次管理请求即 403（撤权即时生效）。
#[sqlx::test(migrations = "../../migrations")]
async fn rv1_rv2_admin_demoted_to_member_loses_access_on_the_same_jwt(pool: PgPool) {
    let w = world(&pool).await;
    let owner_token = token("usr-owner", json!([]));
    let admin_token = token("usr-admin", json!(["tenant_admin"]));

    appoint_owner(&w.app, w.team_id, &owner_token, w.admin_id).await;

    // 降级前：第二 Owner 可读成员详情（owner 闸内）。
    let (status, _) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/teams/{}/members/{}", w.team_id, w.member_id),
        &admin_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // RV-1：owner 把第二 owner 降为 member。
    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/teams/{}/members/{}/role", w.team_id, w.admin_id),
        &owner_token,
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    assert_eq!(
        member_role(&pool, w.tenant_id, w.admin_id).await.as_deref(),
        Some("member")
    );
    assert_eq!(owner_count(&pool, w.team_id).await, 1);

    // 审计：before owner → after member。
    let (action, before, after): (String, Value, Value) = sqlx::query_as(
        "SELECT action, before_data, after_data FROM governance.tenant_audit_log \
          WHERE tenant_id = $1 AND target_principal_id = $2 \
            AND action = 'team_member.role_changed' \
          ORDER BY created_at DESC LIMIT 1",
    )
    .bind(w.tenant_id)
    .bind(w.admin_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(action, "team_member.role_changed");
    assert_eq!(before["role"], json!("owner"));
    assert_eq!(after["role"], json!("member"));

    // RV-2：同一张 JWT（token 角色不变）立刻被管理面拒绝——撤权不等 token 过期。
    let (status, body) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/teams/{}/members/{}", w.team_id, w.member_id),
        &admin_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["message"], json!("team owner required"));

    let now = me(&w.app, &admin_token).await;
    assert_eq!(now["is_admin"], json!(false));
    assert_eq!(now["team_role"], json!("member"));

    // 名册只读仍是 active 成员的权利（管理面与成员面分开）。
    let (status, _) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/teams/{}/members", w.team_id),
        &admin_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

// ─────────────────────────────────────────────────────── RV-1b

/// Owner 降级另一 Owner 为 member，再任命回 owner（双向都是 Team Owner 操作）。
#[sqlx::test(migrations = "../../migrations")]
async fn rv1b_admin_can_demote_another_admin_but_cannot_promote_back(pool: PgPool) {
    let w = world(&pool).await;
    let owner_token = token("usr-owner", json!([]));

    appoint_owner(&w.app, w.team_id, &owner_token, w.admin_id).await;

    // 降级另一 Owner（非末位）→ 200。
    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/teams/{}/members/{}/role", w.team_id, w.admin_id),
        &owner_token,
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        member_role(&pool, w.tenant_id, w.admin_id).await.as_deref(),
        Some("member")
    );
    assert_eq!(owner_count(&pool, w.team_id).await, 1);

    // 任命回 owner → 200（任命制下 Owner 可再次任命）。
    appoint_owner(&w.app, w.team_id, &owner_token, w.admin_id).await;
    assert_eq!(
        member_role(&pool, w.tenant_id, w.admin_id).await.as_deref(),
        Some("owner")
    );
    assert_eq!(owner_count(&pool, w.team_id).await, 2);

    // 三次变更各落一条审计（任命 member→owner ×2 + 降级 owner→member ×1，
    // 不依赖行序）。
    let rows: Vec<(Value, Value)> = sqlx::query_as(
        "SELECT before_data, after_data FROM governance.tenant_audit_log \
          WHERE tenant_id = $1 AND target_principal_id = $2 \
            AND action = 'team_member.role_changed'",
    )
    .bind(w.tenant_id)
    .bind(w.admin_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert_eq!(
        rows.iter()
            .filter(|(b, a)| b["role"] == json!("owner") && a["role"] == json!("member"))
            .count(),
        1,
        "{rows:?}"
    );
    assert_eq!(
        rows.iter()
            .filter(|(b, a)| b["role"] == json!("member") && a["role"] == json!("owner"))
            .count(),
        2,
        "{rows:?}"
    );
}

// ─────────────────────────────────────────────────────────── RV-3

/// 移除后同一 JWT 立即失去名册读权（403 "not an active member"）+ removed 审计。
#[sqlx::test(migrations = "../../migrations")]
async fn rv3_removed_member_is_rejected_on_the_same_jwt(pool: PgPool) {
    let w = world(&pool).await;
    let member_token = token("usr-member", json!([]));

    let before = me(&w.app, &member_token).await;
    assert_eq!(before["is_active"], json!(true));
    // 移除前：active 成员可读名册。
    let (status, _) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/teams/{}/members", w.team_id),
        &member_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = call(
        &w.app,
        "DELETE",
        &format!("/v1/pm/teams/{}/members/{}", w.team_id, w.member_id),
        &token("usr-owner", json!([])),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.team_member tm \
          JOIN core.team t ON t.id = tm.team_id \
         WHERE t.tenant_id = $1 AND tm.principal_id = $2",
    )
    .bind(w.tenant_id)
    .bind(w.member_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);

    // 旧 JWT 立刻失去成员面（被移除者回库验证 → 403 "not an active member"）。
    let (status, body) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/teams/{}/members", w.team_id),
        &member_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["message"], json!("not an active member"));

    let now = me(&w.app, &member_token).await;
    assert_eq!(now["memberships"], json!([]));
    assert_eq!(now["onboarding_required"], json!(true));

    // 审计：team_member.removed 恰一条。
    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.tenant_audit_log \
          WHERE tenant_id = $1 AND target_principal_id = $2 AND action = 'team_member.removed'",
    )
    .bind(w.tenant_id)
    .bind(w.member_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);
}

// ─────────────────────────────────────────────────────────── RV-4

/// member 对成员管理零权限（即便 token 自称管理员）；唯一写权是自助退出 leave。
#[sqlx::test(migrations = "../../migrations")]
async fn rv4_member_cannot_touch_member_management(pool: PgPool) {
    let w = world(&pool).await;
    let member_token = token("usr-member", json!(["tenant_admin"]));
    let base = format!("/v1/pm/teams/{}/members", w.team_id);

    // 名册只读对所有 active 成员开放。
    let (status, _) = call(&w.app, "GET", &base, &member_token, None).await;
    assert_eq!(status, StatusCode::OK);

    // 详情（owner 闸）/加人/改角色/移除 全部 403。
    for (method, uri, body) in [
        ("GET", format!("{base}/{}", w.member_id), None),
        (
            "POST",
            base.clone(),
            Some(json!({ "principal_id": w.member_id, "role": "member" })),
        ),
        (
            "PATCH",
            format!("{base}/{}/role", w.member_id),
            Some(json!({ "role": "owner" })),
        ),
        ("DELETE", format!("{base}/{}", w.member_id), None),
    ] {
        let (status, body) = call(&w.app, method, &uri, &member_token, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}: {body}");
        assert_eq!(body["message"], json!("team owner required"), "{method} {uri}");
    }

    // 成员面唯一写权：自助退出。成功后 memberships=[]、onboarding_required=true。
    let (status, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/teams/{}/leave", w.team_id),
        &member_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let now = me(&w.app, &member_token).await;
    assert_eq!(now["memberships"], json!([]));
    assert_eq!(now["onboarding_required"], json!(true));

    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.tenant_audit_log \
          WHERE tenant_id = $1 AND target_principal_id = $2 AND action = 'team_member.left'",
    )
    .bind(w.tenant_id)
    .bind(w.member_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);
}

// ────────────────────────────────────────────── RV-5 / RV-6

/// 任命只能由 Owner 经 PATCH role 发起：member 不可、add 不可造 owner；
/// 角色值/目标校验（400 / 404 / 409）。
#[sqlx::test(migrations = "../../migrations")]
async fn rv5_nobody_but_owner_transfer_creates_an_owner(pool: PgPool) {
    let w = world(&pool).await;
    let owner_token = token("usr-owner", json!([]));
    let member_token = token("usr-member", json!(["tenant_admin"]));

    // member（即便自称管理员）不能任命 owner，也不能做任何角色写。
    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/teams/{}/members/{}/role", w.team_id, w.member_id),
        &member_token,
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["message"], json!("team owner required"));

    let (status, _) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/teams/{}/members/{}/role", w.team_id, w.member_id),
        &member_token,
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Owner 也不能经「加成员」直接造 owner（任命只能走 PATCH role）。
    let (status, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/teams/{}/members", w.team_id),
        &owner_token,
        Some(json!({ "principal_id": w.member_id, "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["message"], json!("team role must be member"));

    // 非法角色值 → 400。
    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/teams/{}/members/{}/role", w.team_id, w.member_id),
        &owner_token,
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["message"], json!("team role must be owner or member"));

    // owner 对自己 PATCH owner → 409 ALREADY_OWNER（无意义）。
    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/teams/{}/members/{}/role", w.team_id, w.owner_id),
        &owner_token,
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], json!("ALREADY_OWNER"));

    // 目标非成员 → 404 "member not found"（PATCH / DELETE 两路）。
    let stranger = Uuid::new_v4();
    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/teams/{}/members/{stranger}/role", w.team_id),
        &owner_token,
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["message"], json!("member not found"));
    let (status, body) = call(
        &w.app,
        "DELETE",
        &format!("/v1/pm/teams/{}/members/{stranger}", w.team_id),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["message"], json!("member not found"));

    // 已是成员重复加 → 409 ALREADY_MEMBER。
    let (status, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/teams/{}/members", w.team_id),
        &owner_token,
        Some(json!({ "principal_id": w.member_id, "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], json!("ALREADY_MEMBER"));

    assert_eq!(owner_count(&pool, w.team_id).await, 1, "全程没有新的 owner");
}

/// 末位 Owner 三路径全 409 OWNER_LAST：降级 / 移除 / 自助退出。
#[sqlx::test(migrations = "../../migrations")]
async fn rv6_the_last_owner_cannot_be_demoted_or_removed(pool: PgPool) {
    let w = world(&pool).await;
    let owner_token = token("usr-owner", json!([]));

    let (status, body) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/teams/{}/members/{}/role", w.team_id, w.owner_id),
        &owner_token,
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], json!("OWNER_LAST"));
    assert_eq!(body["message"], json!("the last owner cannot be demoted"));

    let (status, body) = call(
        &w.app,
        "DELETE",
        &format!("/v1/pm/teams/{}/members/{}", w.team_id, w.owner_id),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], json!("OWNER_LAST"));
    assert_eq!(body["message"], json!("the last owner cannot be removed"));

    let (status, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/teams/{}/leave", w.team_id),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], json!("OWNER_LAST"));
    assert_eq!(body["message"], json!("the last owner cannot leave"));

    assert_eq!(owner_count(&pool, w.team_id).await, 1, "库内必须仍有 owner");
}

// ─────────────────────────────────────────────────────────── RV-7

/// 跨 Team 一律 404 "team not found"（不泄露存在性），写不得生效。
#[sqlx::test(migrations = "../../migrations")]
async fn rv7_cross_tenant_member_calls_are_404(pool: PgPool) {
    let w = world(&pool).await;
    // 另一个租户 + 自己的 Team + owner 行，用 usr-owner 的 token 去打别的 Team。
    let other: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', '2', 'lmms:2') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let other_owner = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'other-owner', true)",
    )
    .bind(other_owner)
    .execute(&pool)
    .await
    .unwrap();
    let other_team: Uuid = sqlx::query_scalar(
        "INSERT INTO core.team (tenant_id, name) VALUES ($1, 'Other Team') RETURNING id",
    )
    .bind(other)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.team_member (team_id, principal_id, tenant_id, role) \
         VALUES ($1, $2, $3, 'owner')",
    )
    .bind(other_team)
    .bind(other_owner)
    .bind(other)
    .execute(&pool)
    .await
    .unwrap();

    let owner_token = token("usr-owner", json!([]));
    for (method, uri, body) in [
        ("GET", format!("/v1/pm/teams/{other_team}/members"), None),
        (
            "POST",
            format!("/v1/pm/teams/{other_team}/members"),
            Some(json!({ "principal_id": other_owner, "role": "member" })),
        ),
        (
            "PATCH",
            format!("/v1/pm/teams/{other_team}/members/{other_owner}/role"),
            Some(json!({ "role": "member" })),
        ),
        (
            "DELETE",
            format!("/v1/pm/teams/{other_team}/members/{other_owner}"),
            None,
        ),
        ("POST", format!("/v1/pm/teams/{other_team}/leave"), None),
    ] {
        let (status, resp) = call(&w.app, method, &uri, &owner_token, body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {uri}: {resp}");
        assert_eq!(resp["message"], json!("team not found"), "{method} {uri}");
    }
    let role: String = sqlx::query_scalar(
        "SELECT role FROM core.team_member WHERE team_id = $1 AND principal_id = $2",
    )
    .bind(other_team)
    .bind(other_owner)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(role, "owner", "跨 Team 写不得生效");
}

// ─────────────────────────────────────────────────────────── RV-8

/// 审计写入失败（注入约束违反）必须把角色变更一起回滚。
#[sqlx::test(migrations = "../../migrations")]
async fn rv8_audit_failure_rolls_the_role_change_back(pool: PgPool) {
    let w = world(&pool).await;
    // 注入：让 `role_changed` 这一动作的审计写入必然违反约束。
    // `NOT VALID` 只对新行生效——world() 只落了 team.created / team_member.added。
    sqlx::query(
        "ALTER TABLE governance.tenant_audit_log \
         ADD CONSTRAINT tmp_reject_role_changed \
         CHECK (action <> 'team_member.role_changed') NOT VALID",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (status, _) = call(
        &w.app,
        "PATCH",
        &format!("/v1/pm/teams/{}/members/{}/role", w.team_id, w.member_id),
        &token("usr-owner", json!([])),
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    assert_eq!(
        member_role(&pool, w.tenant_id, w.member_id).await.as_deref(),
        Some("member"),
        "审计失败必须让角色变更一起回滚"
    );
    assert_eq!(owner_count(&pool, w.team_id).await, 1);
}

// ─────────────────────────────────────────────────────────── RV-9

/// 并发降级两位 owner 之一：锁序在 core.team 行上串行化，最终 owner 数 ≥ 1。
#[sqlx::test(migrations = "../../migrations")]
async fn rv9_concurrent_owner_demotion_never_reaches_zero_owners(pool: PgPool) {
    let w = world(&pool).await;
    let owner_token = token("usr-owner", json!([]));
    // 经 API 任命第二 owner（加团已在 world 里完成）。
    appoint_owner(&w.app, w.team_id, &owner_token, w.admin_id).await;

    // 两个 owner 各自降级对方：并发下必须有一个被拦住。
    let a_uri = format!("/v1/pm/teams/{}/members/{}/role", w.team_id, w.admin_id);
    let b_uri = format!("/v1/pm/teams/{}/members/{}/role", w.team_id, w.owner_id);
    let first = w.app.clone().oneshot(request(
        "PATCH",
        &a_uri,
        &owner_token,
        Some(json!({ "role": "member" })),
    ));
    let second = w.app.clone().oneshot(request(
        "PATCH",
        &b_uri,
        &token("usr-admin", json!([])),
        Some(json!({ "role": "member" })),
    ));
    let (a, b) = tokio::join!(first, second);

    let codes = vec![a.unwrap().status(), b.unwrap().status()];
    assert!(
        codes.contains(&StatusCode::OK),
        "至少一个必须成功，实际 {codes:?}"
    );
    // 另一个要么撞 OWNER_LAST（409），要么因「对方先把自己降成 member」而失去
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

    assert_eq!(owner_count(&pool, w.team_id).await, 1, "并发降级不得让 owner 归零");
}

// ─────────────────────────────────────────────────────────── RV-10

/// token 角色声称任何角色，只要没有 team_member 行，管理面一律 403。
#[sqlx::test(migrations = "../../migrations")]
async fn rv10_token_roles_alone_never_grant_tenant_admin(pool: PgPool) {
    let app = router_with_auth_config(pool.clone(), test_jwks(), first_admin("usr-owner"));
    let owner_token = token("usr-owner", json!([]));
    let owner = me(&app, &owner_token).await;
    let owner_id = Uuid::parse_str(owner["principal_id"].as_str().unwrap()).unwrap();
    let (status, _) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &owner_token,
        Some(json!({ "name": "Claim Probe" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let team_id =
        Uuid::parse_str(me(&app, &owner_token).await["memberships"][0]["team_id"].as_str().unwrap())
            .unwrap();

    // token 自称 tenant_admin，但库里只有 provision 的 member 预登记行。
    let claimant_token = token("usr-claimant", json!(["tenant_admin"]));
    let who = me(&app, &claimant_token).await;
    assert_eq!(who["is_admin"], json!(false), "token roles 不是事实源");
    assert_eq!(who["team_role"], json!("member"));
    assert_eq!(who["memberships"], json!([]));
    let claimant_id = Uuid::parse_str(who["principal_id"].as_str().unwrap()).unwrap();

    // 先激活（排除「未激活」干扰：下面的 403 必须来自成员事实源，不是激活门）。
    activate(&app, &owner_token, claimant_id).await;

    // 名册读：无 team_member 行 → 403 "not an active member"。
    let (status, body) = call(
        &app,
        "GET",
        &format!("/v1/pm/teams/{team_id}/members"),
        &claimant_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["message"], json!("not an active member"));

    // 管理面写（POST / PATCH / DELETE）一律 403 "team owner required"。
    for (method, uri, body) in [
        (
            "POST",
            format!("/v1/pm/teams/{team_id}/members"),
            Some(json!({ "principal_id": claimant_id, "role": "member" })),
        ),
        (
            "PATCH",
            format!("/v1/pm/teams/{team_id}/members/{owner_id}/role"),
            Some(json!({ "role": "member" })),
        ),
        (
            "DELETE",
            format!("/v1/pm/teams/{team_id}/members/{owner_id}"),
            None,
        ),
    ] {
        let (status, resp) = call(&app, method, &uri, &claimant_token, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}: {resp}");
        assert_eq!(resp["message"], json!("team owner required"), "{method} {uri}");
    }
}
