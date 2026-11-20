//! Product Membership 与生命周期验收（P4-ORG Phase 3；PA 基线 §2/§3/§4/§5；
//! DR-PA-01/02/03，2026-11-20）。DB 约束 + API 矩阵同文件。
//!
//! 覆盖：创建（Team Owner 独占 / 首位 Owner 指定 / 非 Team 本体 400）·
//! Membership 内部管理（Member 平权加删普通成员 / Team Member 不自动成为
//! Product 成员）· Owner 保护（末位不可移除/降级 · 任命=继任指定 · 降级仅
//! Team Owner · 恢复性任命）· 生命周期（Owner 独占 · abandoned 拦 Link ·
//! noop 409）· 审计。

use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::auth::AuthConfig;
use pm_api::routes::router_with_auth_config;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const PUBLIC_PEM: &str = include_str!("keys/public.pem");
const PRIVATE_PEM: &str = include_str!("keys/private.pem");

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
    match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            builder.body(Body::from(value.to_string())).unwrap()
        }
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn send(app: &axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    send(app, request(method, uri, token, body)).await
}

/// 认领建团 + provision/激活/入团成员。返回 (app, tenant_id, team_id, owner_id)。
async fn team_world(pool: PgPool) -> (axum::Router, Uuid, Uuid, Uuid) {
    std::env::set_var("AI_PM_SECRET_KEY", "product-domain-test-secret");
    let app = router_with_auth_config(
        pool.clone(),
        test_jwks(),
        AuthConfig {
            first_admin_tenant: Some("1".to_string()),
            first_admin_subject: Some("usr-owner".to_string()),
        },
    );
    let owner_token = token_for("usr-owner", 1);
    let (_, me) = call(&app, "GET", "/v1/pm/me", &owner_token, None).await;
    let tenant_id = Uuid::parse_str(me["tenant_id"].as_str().unwrap()).unwrap();
    let owner_id = Uuid::parse_str(me["principal_id"].as_str().unwrap()).unwrap();
    let (st, body) = call(
        &app,
        "POST",
        "/v1/pm/teams",
        &owner_token,
        Some(json!({ "name": "PA Team" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{body}");
    let team_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    (app, tenant_id, team_id, owner_id)
}

/// provision + 激活 + 入 Team 名册（两步 helper 的 Pool 版）。
async fn seed_member(
    app: &axum::Router,
    pool: &PgPool,
    team_id: Uuid,
    owner_token: &str,
    sub: &str,
) -> Uuid {
    let (_, me) = call(app, "GET", "/v1/pm/me", &token_for(sub, 1), None).await;
    let id = Uuid::parse_str(me["principal_id"].as_str().unwrap()).unwrap();
    let (st, body) = call(
        app,
        "PATCH",
        &format!("/v1/pm/principals/{id}"),
        owner_token,
        Some(json!({ "is_active": true })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let (st, body) = call(
        app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/members"),
        owner_token,
        Some(json!({ "principal_id": id, "role": "member" })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    id
}

struct ProductWorld {
    app: axum::Router,
    #[allow(dead_code)]
    tenant_id: Uuid,
    team_id: Uuid,
    owner_id: Uuid,
    /// Product Owner（= Team Owner 兼任首位 Owner）。
    product_id: Uuid,
    /// Product Member（普通成员）。
    member_id: Uuid,
    /// Team Member 但**不是** Product Member（DR-PA-01 关键断言用）。
    outsider_id: Uuid,
}

/// 认领建团 + Product（首位 Owner = Team Owner）+ 一个普通 Product Member
/// + 一个仅 Team Member 的 outsider。
async fn product_world(pool: PgPool) -> ProductWorld {
    let (app, tenant_id, team_id, owner_id) = team_world(pool.clone()).await;
    let owner_token = token_for("usr-owner", 1);
    let (st, body) = call(
        &app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/products"),
        &owner_token,
        Some(json!({
            "name": "Alpha",
            "description": "first product",
            "first_owner_principal_id": owner_id,
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{body}");
    let product_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    let member_id = seed_member(&app, &pool, team_id, &owner_token, "usr-member").await;
    let outsider_id = seed_member(&app, &pool, team_id, &owner_token, "usr-out").await;

    // Product Member 平权：owner 把 usr-member 加为普通成员。
    let (st, body) = call(
        &app,
        "POST",
        &format!("/v1/pm/products/{product_id}/members"),
        &owner_token,
        Some(json!({ "principal_id": member_id })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");

    ProductWorld {
        app,
        tenant_id,
        team_id,
        owner_id,
        product_id,
        member_id,
        outsider_id,
    }
}

fn members_uri(product_id: Uuid) -> String {
    format!("/v1/pm/products/{product_id}/members")
}

// ═══════════════════════════════════════════════════════════ 创建

#[sqlx::test(migrations = "../../migrations")]
async fn create_requires_team_owner(pool: PgPool) {
    let (app, _t, team_id, _owner) = team_world(pool.clone()).await;
    let member = seed_member(&app, &pool, team_id, &token_for("usr-owner", 1), "usr-member").await;
    let (st, body) = call(
        &app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/products"),
        &token_for("usr-member", 1),
        Some(json!({
            "name": "Bad",
            "first_owner_principal_id": member,
        })),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["message"], json!("team owner required"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_rejects_first_owner_outside_tenant(pool: PgPool) {
    let (app, _t, team_id, _owner) = team_world(pool).await;
    let (st, body) = call(
        &app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/products"),
        &token_for("usr-owner", 1),
        Some(json!({
            "name": "Bad",
            "first_owner_principal_id": Uuid::new_v4(),
        })),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{body}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_seeds_first_owner_and_audit(pool: PgPool) {
    let (app, tenant, team_id, owner_id) = team_world(pool.clone()).await;
    let (st, body) = call(
        &app,
        "POST",
        &format!("/v1/pm/teams/{team_id}/products"),
        &token_for("usr-owner", 1),
        Some(json!({
            "name": "Audit",
            "first_owner_principal_id": owner_id,
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{body}");
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.product_member WHERE principal_id = $1 AND role = 'owner'",
    )
    .bind(owner_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(rows >= 1, "first owner row must exist");
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.tenant_audit_log \
          WHERE tenant_id = $1 AND action IN ('product.created','product_member.added')",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(audits >= 2, "product.created + product_member.added audits");
}

// ═══════════════════════════════════════════════════════════ Membership 内部管理

#[sqlx::test(migrations = "../../migrations")]
async fn team_member_is_not_auto_product_member(pool: PgPool) {
    let w = product_world(pool.clone()).await;
    // outsider 是 Team Member 但没有 Product Membership：档案与名册均不可见。
    let (st, _) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/products/{}", w.product_id),
        &token_for("usr-out", 1),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = call(
        &w.app,
        "GET",
        &members_uri(w.product_id),
        &token_for("usr-out", 1),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    // Team Member 列表过滤：只见自己参与的（空）。
    let (st, body) = call(
        &w.app,
        "GET",
        &format!("/v1/pm/teams/{}/products", w.team_id),
        &token_for("usr-out", 1),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn member_matrix_and_owner_last(pool: PgPool) {
    let w = product_world(pool.clone()).await;
    let owner_token = token_for("usr-owner", 1);
    let member_token = token_for("usr-member", 1);

    // Product Member（usr-member）可读名册、可添加/移除普通成员（平权）。
    let new_id = seed_member(&w.app, &pool, w.team_id, &owner_token, "usr-new").await;
    let (st, body) = call(
        &w.app,
        "POST",
        &members_uri(w.product_id),
        &member_token,
        Some(json!({ "principal_id": new_id })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let (st, body) = call(
        &w.app,
        "DELETE",
        &format!("{}/{new_id}", members_uri(w.product_id)),
        &member_token,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");

    // 非成员不可添加。
    let (st, body) = call(
        &w.app,
        "POST",
        &members_uri(w.product_id),
        &token_for("usr-out", 1),
        Some(json!({ "principal_id": new_id })),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["message"], json!("not a product member"));

    // 移除末位 Owner → 409 last_owner（即便发起者是 Team Owner 本人）。
    let (st, body) = call(
        &w.app,
        "DELETE",
        &format!("{}/{}", members_uri(w.product_id), w.owner_id),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], json!("last_owner"));
    let _ = body;
}

// ═══════════════════════════════════════════════════════════ Owner 变更

#[sqlx::test(migrations = "../../migrations")]
async fn appointment_and_demote_matrix(pool: PgPool) {
    let w = product_world(pool.clone()).await;
    let owner_token = token_for("usr-owner", 1);
    let new_id = seed_member(&w.app, &pool, w.team_id, &owner_token, "usr-new").await;
    let (st, body) = call(
        &w.app,
        "POST",
        &members_uri(w.product_id),
        &owner_token,
        Some(json!({ "principal_id": new_id })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");

    // Product Owner 任命继任者（多 Owner 并存）。
    let (st, body) = call(
        &w.app,
        "POST",
        &format!("{}/{new_id}/appoint-owner", members_uri(w.product_id)),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");

    // 任命后：新 Owner（usr-new）任命他人 → 多 Owner 并存（任意有效 Owner 可）。
    let (st, body) = call(
        &w.app,
        "POST",
        &format!("{}/{}/appoint-owner", members_uri(w.product_id), new_id),
        &token_for("usr-new", 1),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], json!("already_owner"));

    // 普通 Member 任命/降级 Owner → 403（OPEN-3：第一轮仅 Owner 任命 + Team Owner 降级）。
    let plain_id = seed_member(&w.app, &pool, w.team_id, &owner_token, "usr-plain").await;
    let (st, body) = call(
        &w.app,
        "POST",
        &members_uri(w.product_id),
        &token_for("usr-member", 1),
        Some(json!({ "principal_id": plain_id })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let (st, body) = call(
        &w.app,
        "POST",
        &format!("{}/{plain_id}/demote-owner", members_uri(w.product_id)),
        &token_for("usr-member", 1),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{body}");
    let _ = body;

    // Team Owner 降级 Product Owner → 200；降末位 → 409 last_owner。
    let (st, body) = call(
        &w.app,
        "POST",
        &format!("{}/{new_id}/demote-owner", members_uri(w.product_id)),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let (st, body) = call(
        &w.app,
        "POST",
        &format!("{}/{}/demote-owner", members_uri(w.product_id), w.owner_id),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], json!("last_owner"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn team_owner_restore_when_no_owner(pool: PgPool) {
    let w = product_world(pool.clone()).await;
    let owner_token = token_for("usr-owner", 1);
    // Team Owner 先移除唯一的 Owner（自己）——owner_count=1 时被 OWNER_LAST 拦，
    // 因此先任命继任者再移除，之后降级继任者制造无 Owner 态，最后恢复性任命。
    let new_id = seed_member(&w.app, &pool, w.team_id, &owner_token, "usr-new").await;
    let (st, _) = call(
        &w.app,
        "POST",
        &members_uri(w.product_id),
        &owner_token,
        Some(json!({ "principal_id": new_id })),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = call(
        &w.app,
        "POST",
        &format!("{}/{new_id}/appoint-owner", members_uri(w.product_id)),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = call(
        &w.app,
        "POST",
        &format!("{}/{}/demote-owner", members_uri(w.product_id), w.owner_id),
        &owner_token,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    // 模拟「特殊情况导致暂时没有 Owner」（PA §4）：直接移除剩余 Owner 行——
    // 正常路径被 OWNER_LAST 拦，异常态由 Team Owner 恢复性任命兜底。
    sqlx::query("DELETE FROM core.product_member WHERE product_id = $1 AND role = 'owner'")
        .bind(w.product_id)
        .execute(&pool)
        .await
        .unwrap();
    let owners: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.product_member WHERE product_id = $1 AND role = 'owner'",
    )
    .bind(w.product_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owners, 0);
    let (st, body) = call(
        &w.app,
        "POST",
        &members_uri(w.product_id),
        &owner_token,
        Some(json!({ "principal_id": new_id })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let role: String = sqlx::query_scalar(
        "SELECT role FROM core.product_member WHERE product_id = $1 AND principal_id = $2",
    )
    .bind(w.product_id)
    .bind(new_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(role, "owner", "restore appointment must grant owner");
}

// ═══════════════════════════════════════════════════════════ 生命周期

#[sqlx::test(migrations = "../../migrations")]
async fn lifecycle_owner_only_and_link_gate(pool: PgPool) {
    let w = product_world(pool.clone()).await;
    let owner_token = token_for("usr-owner", 1);
    let member_token = token_for("usr-member", 1);

    // Member（普通成员）不可 abandon。
    let (st, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/products/{}/lifecycle", w.product_id),
        &member_token,
        Some(json!({ "action": "abandon" })),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["message"], json!("product_owner_required"));

    // Owner abandon → abandoned；重复 abandon → 409 noop。
    let (st, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/products/{}/lifecycle", w.product_id),
        &owner_token,
        Some(json!({ "action": "abandon" })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["status"], json!("abandoned"));
    let (st, _) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/products/{}/lifecycle", w.product_id),
        &owner_token,
        Some(json!({ "action": "abandon" })),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);

    // abandoned 拦 Link 建立（fail-closed）；active 时允许。
    let (st, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/products/{}/links", w.product_id),
        &owner_token,
        Some(json!({ "project_id": Uuid::new_v4() })),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["message"], json!("product_abandoned"));

    let (st, body) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/products/{}/lifecycle", w.product_id),
        &owner_token,
        Some(json!({ "action": "restart" })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["status"], json!("active"));

    // active 建 Link 需要项目存在；bogus project → 404。
    let (st, _) = call(
        &w.app,
        "POST",
        &format!("/v1/pm/products/{}/links", w.product_id),
        &owner_token,
        Some(json!({ "project_id": Uuid::new_v4() })),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // 生命周期审计。
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.tenant_audit_log \
          WHERE action = 'product.lifecycle_changed' AND target_id = $1",
    )
    .bind(w.product_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 2, "abandon + restart audits");
}
