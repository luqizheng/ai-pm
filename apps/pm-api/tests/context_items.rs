//! B2 集成测试：会话上下文条目（AI-02 · CX1/CX3 · SQL-01~SQL-08）。
//!
//! 覆盖：批量附加（对象存在性 / 版本复核 / 分类型矩阵）· 生命周期状态机
//! （白名单 / Pending→Active 真人门 / unavailable 原因同生同灭）· 软移除
//! （detached 留痕不删业务对象）· 属主隔离（跨属主 404 同形）· 唯一约束
//! （同会话同对象不重复附加）· 审计落账。

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

fn sign(claims: Value) -> String {
    sign_token(PRIVATE_PEM, &claims, Duration::from_secs(300)).expect("test token must sign")
}

fn browser_token() -> String {
    sign(json!({
        "sub": "usr-lead", "tid": 1, "username": "usr-lead",
        "roles": ["tenant_admin"], "permissions": [], "aud": "coder-micro-services",
    }))
}

fn req(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {}", browser_token()));
    if let Some(value) = body {
        builder = builder.header("content-type", "application/json");
        builder.body(Body::from(value.to_string())).unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    }
}

async fn send(app: &axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn app(pool: PgPool) -> axum::Router {
    router_with_auth_config(
        pool.clone(),
        Arc::new(JWKSCache::from_pem(PUBLIC_PEM).unwrap()),
        AuthConfig {
            first_admin_tenant: Some("1".to_string()),
            first_admin_subject: Some("usr-lead".to_string()),
        },
    )
}

/// 种子：租户 + 项目 + 真人主体验活 + 项目成员；返回 (app, project, conversation)。
async fn seed(pool: &PgPool, code: &str) -> (axum::Router, Uuid, Uuid) {
    let tenant_id: Uuid = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', '1', 'lmms:1') \
         ON CONFLICT (provider, external_key) DO UPDATE SET display_name = core.tenant.display_name \
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let outcome = pm_application::bootstrap::bootstrap_project(
        pool,
        pm_application::bootstrap::BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("Project {code}"),
            tenant_id,
        },
    )
    .await
    .unwrap();
    let project_id = outcome.project_id;

    let a = app(pool.clone());
    // 首管理员激活（真人主体）并加入项目。
    let me = send(&a, req("GET", "/v1/pm/me", None)).await;
    assert_eq!(me.0, StatusCode::OK, "activate: {}", me.1);
    let human: Uuid = me.1["principal_id"].as_str().unwrap().parse().unwrap();
    // P4-ORG Phase 2：first_admin 只落预登记行，管理操作（激活他人）要求
    // Team Owner——先认领建团。
    let (team_status, team_body) = send(
        &a,
        req("POST", "/v1/pm/teams", Some(json!({ "name": "Ctx Team" }))),
    )
    .await;
    assert_eq!(team_status, StatusCode::CREATED, "{team_body}");
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         SELECT tenant_id, $1, $2, 'owner' FROM core.project WHERE id = $1 \
         ON CONFLICT DO NOTHING",
    )
    .bind(project_id)
    .bind(human)
    .execute(pool)
    .await
    .unwrap();

    // 建会话（属主 = 真人）。
    let created = send(
        &a,
        req(
            "POST",
            &format!("/v1/pm/projects/{project_id}/conversations"),
            Some(json!({ "title": "ctx items" })),
        ),
    )
    .await;
    assert_eq!(
        created.0,
        StatusCode::CREATED,
        "conversation: {}",
        created.1
    );
    let conversation: Uuid = created.1["id"].as_str().unwrap().parse().unwrap();

    (a, project_id, conversation)
}

/// 种一条需求（可附加对象）。
async fn seed_requirement(pool: &PgPool, project_id: Uuid) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO delivery.requirement (project_id, code, title, tenant_id) \
         VALUES ($1, 'REQ-CX1', 'Context item fixture', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

const BASE: fn(Uuid, Uuid) -> String = |project, conversation| {
    format!("/v1/pm/projects/{project}/conversations/{conversation}/context-items")
};

#[sqlx::test(migrations = "../../migrations")]
async fn attach_requirement_and_list(pool: PgPool) {
    let (a, project, conversation) = seed(&pool, "CI-1").await;
    let requirement = seed_requirement(&pool, project).await;

    let (status, body) = send(
        &a,
        req(
            "POST",
            &BASE(project, conversation),
            Some(json!({
                "items": [{
                    "object_type": "requirement",
                    "object_id": requirement,
                    "source": "page",
                    "read_only": false
                }]
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["object_type"], json!("requirement"));
    assert_eq!(items[0]["attachment"], json!("attached"));
    assert_eq!(items[0]["lifecycle"], json!("active"));
    assert_eq!(items[0]["object_code"], json!("REQ-CX1"));

    // 列表可见。
    let (status, listed) = send(&a, req("GET", &BASE(project, conversation), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);

    // 同对象重复附加 → 409。
    let (status, body) = send(
        &a,
        req(
            "POST",
            &BASE(project, conversation),
            Some(json!({
                "items": [{
                    "object_type": "requirement",
                    "object_id": requirement,
                    "source": "page"
                }]
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // 不存在的对象 → 400（校验失败，对象不在本项目）。
    let (status, _) = send(
        &a,
        req(
            "POST",
            &BASE(project, conversation),
            Some(json!({
                "items": [{
                    "object_type": "requirement",
                    "object_id": Uuid::new_v4(),
                    "source": "page"
                }]
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_items_reject_content_version_fields(pool: PgPool) {
    let (a, project, conversation) = seed(&pool, "CI-2").await;
    let task: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, tenant_id) \
         VALUES ($1, 'fixture task', (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 任务没有内容版本：revision_id / version / content_hash 一律拒绝（SQL-01）。
    let (status, body) = send(
        &a,
        req(
            "POST",
            &BASE(project, conversation),
            Some(json!({
                "items": [{
                    "object_type": "task",
                    "object_id": task,
                    "version": 3,
                    "source": "page"
                }]
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // row_version 快照合法（与实际 revision 一致）。
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM delivery.task WHERE id = $1")
        .bind(task)
        .fetch_one(&pool)
        .await
        .unwrap();
    let (status, body) = send(
        &a,
        req(
            "POST",
            &BASE(project, conversation),
            Some(json!({
                "items": [{
                    "object_type": "task",
                    "object_id": task,
                    "row_version": revision,
                    "source": "page"
                }]
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["items"][0]["row_version"], json!(revision));

    // 错误的 row_version 快照 → 400（写入即复核）。
    let (status, _) = send(
        &a,
        req(
            "POST",
            &BASE(project, conversation),
            Some(json!({
                "items": [{
                    "object_type": "task",
                    "object_id": task,
                    "row_version": revision + 100,
                    "source": "page"
                }]
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn lifecycle_state_machine_and_human_gate(pool: PgPool) {
    let (a, project, conversation) = seed(&pool, "CI-3").await;
    let requirement = seed_requirement(&pool, project).await;
    let base = BASE(project, conversation);

    let (status, body) = send(
        &a,
        req(
            "POST",
            &base,
            Some(json!({
                "items": [{
                    "object_type": "requirement",
                    "object_id": requirement,
                    "initial_lifecycle": "pending",
                    "source": "page"
                }]
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item: Uuid = body["items"][0]["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(body["items"][0]["lifecycle"], json!("pending"));

    let lifecycle_uri = format!("{base}/{item}/lifecycle");

    // pending → inactive：状态机不允许 → 409。
    let (status, body) = send(
        &a,
        req(
            "PATCH",
            &lifecycle_uri,
            Some(json!({ "lifecycle": "inactive" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // pending → active：真人交互会话 → 200。
    let (status, body) = send(
        &a,
        req(
            "PATCH",
            &lifecycle_uri,
            Some(json!({ "lifecycle": "active" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["lifecycle"], json!("active"));

    // active → inactive（Disable）。
    let (status, _) = send(
        &a,
        req(
            "PATCH",
            &lifecycle_uri,
            Some(json!({ "lifecycle": "inactive" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // inactive → active：重新校验（对象仍在 → 通过）。
    let (status, _) = send(
        &a,
        req(
            "PATCH",
            &lifecycle_uri,
            Some(json!({ "lifecycle": "active" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // 任意 → unavailable：缺原因 → 400；给原因 → 200。
    let (status, _) = send(
        &a,
        req(
            "PATCH",
            &lifecycle_uri,
            Some(json!({ "lifecycle": "unavailable" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body) = send(
        &a,
        req(
            "PATCH",
            &lifecycle_uri,
            Some(json!({
                "lifecycle": "unavailable",
                "unavailable_reason": "source document removed"
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["unavailable_reason"], json!("source document removed"));

    // unavailable → active：原因被清空（不得留误导性当前原因）。
    let (status, body) = send(
        &a,
        req(
            "PATCH",
            &lifecycle_uri,
            Some(json!({ "lifecycle": "active" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["unavailable_reason"], Value::Null);
}

#[sqlx::test(migrations = "../../migrations")]
async fn soft_remove_keeps_object_and_history(pool: PgPool) {
    let (a, project, conversation) = seed(&pool, "CI-4").await;
    let requirement = seed_requirement(&pool, project).await;
    let base = BASE(project, conversation);

    let (status, body) = send(
        &a,
        req(
            "POST",
            &base,
            Some(json!({
                "items": [{
                    "object_type": "requirement",
                    "object_id": requirement,
                    "source": "manual"
                }]
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item: Uuid = body["items"][0]["id"].as_str().unwrap().parse().unwrap();

    // 软移除 → 200，attachment=detached。
    let (status, body) = send(&a, req("DELETE", &format!("{base}/{item}"), None)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["attachment"], json!("detached"));

    // 业务对象未被删除。
    let still_there: Option<Uuid> =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM delivery.requirement WHERE id = $1")
            .bind(requirement)
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert_eq!(still_there, Some(requirement), "business object must stay");

    // 默认列表不再出现；include_detached=true 可见（History）。
    let (_, listed) = send(&a, req("GET", &base, None)).await;
    assert_eq!(listed["items"].as_array().unwrap().len(), 0);
    let (_, listed_all) = send(
        &a,
        req("GET", &format!("{base}?include_detached=true"), None),
    )
    .await;
    assert_eq!(listed_all["items"].as_array().unwrap().len(), 1);

    // 重复移除 → 409。
    let (status, _) = send(&a, req("DELETE", &format!("{base}/{item}"), None)).await;
    assert_eq!(status, StatusCode::CONFLICT);

    // detached 条目不能 PATCH 生命周期（须重新附加）。
    let (status, _) = send(
        &a,
        req(
            "PATCH",
            &format!("{base}/{item}/lifecycle"),
            Some(json!({ "lifecycle": "active" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // 审计：attach + remove 各一条。
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND action IN ('context_item.attach', 'context_item.remove')",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 2, "attach and remove must both be audited");
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_owner_access_is_404_same_shape(pool: PgPool) {
    let (a, project, conversation) = seed(&pool, "CI-5").await;
    let base = BASE(project, conversation);

    let (status, body) = send(
        &a,
        req(
            "POST",
            &base,
            Some(json!({
                "items": [{
                    "object_type": "requirement",
                    "object_id": seed_requirement(&pool, project).await,
                    "source": "page"
                }]
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item: Uuid = body["items"][0]["id"].as_str().unwrap().parse().unwrap();

    // 另一真人**激活**（成为项目成员，但不是会话属主）→ 读写均 404 同形。
    let other_token = sign(json!({
        "sub": "usr-peer", "tid": 1, "username": "usr-peer",
        "roles": ["tenant_admin"], "permissions": [], "aud": "coder-micro-services",
    }));
    let other_req = |method: &str, uri: &str, body: Option<Value>| {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {other_token}"));
        if let Some(value) = body {
            builder = builder.header("content-type", "application/json");
            builder.body(Body::from(value.to_string())).unwrap()
        } else {
            builder.body(Body::empty()).unwrap()
        }
    };
    let me = send(&a, other_req("GET", "/v1/pm/me", None)).await;
    assert_eq!(me.0, StatusCode::OK, "peer self-inspection");
    let peer: Uuid = me.1["principal_id"].as_str().unwrap().parse().unwrap();
    // 由管理员激活（同 principal_identity 测试的激活路径）。
    let activated = send(
        &a,
        req(
            "PATCH",
            &format!("/v1/pm/principals/{peer}"),
            Some(json!({ "is_active": true })),
        ),
    )
    .await;
    assert_eq!(
        activated.0,
        StatusCode::OK,
        "peer activation: {}",
        activated.1
    );
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role)          SELECT tenant_id, $1, $2, 'member' FROM core.project WHERE id = $1          ON CONFLICT DO NOTHING",
    )
    .bind(project)
    .bind(peer)
    .execute(&pool)
    .await
    .unwrap();

    // 项目成员但非会话属主：列表 / PATCH / DELETE 全部 404（不泄露存在性）。
    let (status, body) = send(&a, other_req("GET", &base, None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "peer list must 404: {body}");

    let (status, _) = send(
        &a,
        other_req(
            "PATCH",
            &format!("{base}/{item}/lifecycle"),
            Some(json!({ "lifecycle": "inactive" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "peer patch must 404");

    let (status, _) = send(&a, other_req("DELETE", &format!("{base}/{item}"), None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "peer delete must 404");

    // 属主自己仍可见（404 同形不误伤）。
    let (status, listed) = send(&a, req("GET", &base, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);
}
