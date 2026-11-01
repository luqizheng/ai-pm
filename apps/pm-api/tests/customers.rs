//! `/v1/pm/customers` 与「建项挂客户」契约测试（0017 客户主数据）。
//!
//! 覆盖：建档 / code 冲突 / 搜索过滤 / 建项携带 customer_id / 未知客户 400 /
//! 归档客户拒绝建项 / 兼容路径落 placeholder / 项目列表返回 customer_name。

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use pm_domain::SYSTEM_TENANT;

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn send(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn create_customer(app: &axum::Router, code: &str, name: &str) -> Value {
    create_customer_with_short(app, code, name, None).await
}

async fn create_customer_with_short(
    app: &axum::Router,
    code: &str,
    name: &str,
    short_name: Option<&str>,
) -> Value {
    let mut payload = json!({ "code": code, "name": name });
    if let Some(short) = short_name {
        payload["short_name"] = json!(short);
    }
    let response = app
        .clone()
        .oneshot(send("POST", "/v1/pm/customers", payload))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await
}

#[sqlx::test(migrations = "../../migrations")]
async fn creates_customer_with_zero_projects(pool: PgPool) {
    let app = router(pool.clone());
    let created = create_customer_with_short(
        &app,
        "BANK-SH",
        "中国某某银行股份有限公司上海分行",
        Some("某某银行上海分行"),
    )
    .await;

    assert_eq!(created["code"], json!("BANK-SH"));
    assert_eq!(created["name"], json!("中国某某银行股份有限公司上海分行"));
    assert_eq!(created["short_name"], json!("某某银行上海分行"));
    assert_eq!(created["status"], json!("active"));
    assert_eq!(created["project_count"], json!(0));
    assert_eq!(created["parent_id"], Value::Null);
}

#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_code_conflicts(pool: PgPool) {
    let app = router(pool.clone());
    create_customer(&app, "BANK-SH", "某银行").await;

    let response = app
        .clone()
        .oneshot(send(
            "POST",
            "/v1/pm/customers",
            json!({ "code": "BANK-SH", "name": "另一家银行" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "../../migrations")]
async fn blank_code_is_rejected(pool: PgPool) {
    let app = router(pool.clone());
    let response = app
        .clone()
        .oneshot(send(
            "POST",
            "/v1/pm/customers",
            json!({ "code": "   ", "name": "某银行" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn searches_by_name_code_and_short_name(pool: PgPool) {
    let app = router(pool.clone());
    create_customer_with_short(&app, "BANK-SH", "中国某某银行上海分行", Some("某某银行")).await;
    create_customer_with_short(&app, "TELECOM", "某某通信集团", Some("某某通信")).await;

    for query in ["BANK", "某某银行", "某某通信"] {
        let response = app
            .clone()
            .oneshot(get(&format!("/v1/pm/customers?q={query}")))
            .await
            .unwrap();
        let body = body_json(response).await;
        assert_eq!(body["total"], json!(1), "q={query} 应只命中一条");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn project_binds_to_existing_customer(pool: PgPool) {
    let app = router(pool.clone());
    let customer = create_customer(&app, "BANK-SH", "中国某某银行上海分行").await;
    let customer_id = customer["id"].as_str().unwrap().to_string();

    for code in ["BANK-CORE", "BANK-MOBILE"] {
        let response = app
            .clone()
            .oneshot(send(
                "POST",
                "/v1/pm/projects",
                json!({ "code": code, "name": code, "customer_id": customer_id }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = body_json(response).await;
        assert_eq!(body["customer_id"], json!(customer_id));
        assert_eq!(body["customer_name"], json!("中国某某银行上海分行"));
        assert_eq!(body["customer_status"], json!("active"));
    }

    // 一个客户挂两个项目：project_count 与 customer_id 过滤都要反映出来。
    let response = app
        .clone()
        .oneshot(get(&format!("/v1/pm/customers/{customer_id}")))
        .await
        .unwrap();
    let detail = body_json(response).await;
    assert_eq!(detail["project_count"], json!(2));

    let response = app
        .clone()
        .oneshot(get(&format!("/v1/pm/projects?customer_id={customer_id}")))
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(body["total"], json!(2));
}

#[sqlx::test(migrations = "../../migrations")]
async fn unknown_customer_is_rejected(pool: PgPool) {
    let app = router(pool.clone());
    let missing = uuid::Uuid::new_v4().to_string();
    let response = app
        .clone()
        .oneshot(send(
            "POST",
            "/v1/pm/projects",
            json!({ "code": "ORPHAN", "name": "孤儿项目", "customer_id": missing }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let projects: i64 = sqlx::query_scalar("SELECT count(*) FROM core.project")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(projects, 0, "失败的事务不得留下项目");
}

#[sqlx::test(migrations = "../../migrations")]
async fn legacy_bootstrap_marks_placeholder_customer(pool: PgPool) {
    let app = router(pool.clone());
    bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "LEGACY-1".into(),
            project_name: "兼容路径".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();

    let response = app
        .clone()
        .oneshot(get("/v1/pm/projects"))
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(body["items"][0]["customer_status"], json!("placeholder"));
    assert_eq!(body["items"][0]["customer_name"], json!("LEGACY-1"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn archived_customer_rejects_new_projects(pool: PgPool) {
    let app = router(pool.clone());
    let customer = create_customer(&app, "BANK-SH", "中国某某银行上海分行").await;
    let customer_id = customer["id"].as_str().unwrap().to_string();

    let response = app
        .clone()
        .oneshot(send(
            "PATCH",
            &format!("/v1/pm/customers/{customer_id}"),
            json!({ "status": "archived", "short_name": "" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let patched = body_json(response).await;
    assert_eq!(patched["status"], json!("archived"));
    assert_eq!(patched["short_name"], Value::Null, "空串即清空");
    assert_eq!(patched["name"], json!("中国某某银行上海分行"), "缺省即不改");

    let response = app
        .clone()
        .oneshot(send(
            "POST",
            "/v1/pm/projects",
            json!({ "code": "AFTER-ARCHIVE", "name": "归档后", "customer_id": customer_id }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
