//! PA-07 读面端点契约测试（Figma 179:3603，0050 读面扩展）：
//! - `GET /v1/pm/tasks/{task_id}/revisions`（D 系列：时间线 + 陈旧活跃 Run 警告）
//! - `GET /v1/pm/projects/{project_id}/products`（E 系列 + A 页 RELATED PRODUCTS）
//! - `GET /v1/pm/products/{product_id}/tasks`（E 页 Tasks tab / Impact Summary）
//!
//! 授权：off 模式 dev-off principal；项目面 join_dev、产品面 product_member 直插。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

mod common;
use common::join_dev;

fn get(uri: String) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn seed_task(pool: &PgPool, project_id: uuid::Uuid, title: &str) -> uuid::Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id) \
         VALUES ($1, $2, 'draft', 1, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 建 Product（归属新 Team）+（可选）挂 dev-off 为 Product Owner。
async fn seed_product(
    pool: &PgPool,
    project_id: uuid::Uuid,
    name: &str,
    status: &str,
    with_member: bool,
) -> uuid::Uuid {
    let product_id: uuid::Uuid = sqlx::query_scalar(
        "WITH t AS ( \
             INSERT INTO core.team (tenant_id, name) \
             SELECT tenant_id, $1 || '-team' FROM core.project WHERE id = $2 RETURNING tenant_id, id \
         ), p AS ( \
             INSERT INTO core.product (team_id, tenant_id, key, name, status) \
             SELECT id, tenant_id, 'P-' || upper(substring(gen_random_uuid()::text from 1 for 8)), $1, $3 FROM t RETURNING id \
         ) SELECT id FROM p",
    )
    .bind(name)
    .bind(project_id)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap();
    if with_member {
        sqlx::query(
            "INSERT INTO core.product_member (product_id, principal_id, tenant_id, role) \
             VALUES ($1, $2, (SELECT tenant_id FROM core.product WHERE id = $1), 'owner')",
        )
        .bind(product_id)
        .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
        .execute(pool)
        .await
        .unwrap();
    }
    product_id
}

// ───────────────────────────────────────────────── task revisions（D 系列）

#[sqlx::test(migrations = "../../migrations")]
async fn task_revisions_timeline_marks_active_and_stale_runs(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "PA07-R".into(),
            project_name: "pa07".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    join_dev(&pool, boot.project_id).await;
    let task_id = seed_task(&pool, boot.project_id, "auth api").await;
    // 两个快照：r1（旧）与 r2（current）。
    let r1: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_revision (tenant_id, project_id, task_id, revision_no, goal, change_reason) \
         SELECT tenant_id, project_id, $1, 1, 'baseline', 'initial' FROM delivery.task WHERE id = $1 RETURNING id",
    )
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let r2: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_revision (tenant_id, project_id, task_id, revision_no, goal, change_reason) \
         SELECT tenant_id, project_id, $1, 2, 'expanded', 'cover security' FROM delivery.task WHERE id = $1 RETURNING id",
    )
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task SET current_task_revision_id = $2 WHERE id = $1")
        .bind(task_id)
        .bind(r2)
        .execute(&pool)
        .await
        .unwrap();
    // 陈旧活跃 Run：queued 且绑定 r1（≠ current）。
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, task_revision_id, tenant_id) \
         VALUES ($1, $2, 1, 'queued', $3, (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(boot.project_id)
    .bind(task_id)
    .bind(r1)
    .execute(&pool)
    .await
    .unwrap();

    let app = router(pool.clone());
    let response = app
        .oneshot(get(format!("/v1/pm/tasks/{task_id}/revisions")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "two frozen revisions");
    // 时间线最新在上；展示序号连续（r2/r1），纪元号原样透出。
    assert_eq!(items[0]["revision_no"], json!(2));
    assert_eq!(items[0]["epoch_no"], json!(2));
    assert_eq!(items[0]["active"], json!(true));
    assert_eq!(items[1]["revision_no"], json!(1));
    assert_eq!(items[1]["active"], json!(false));
    assert_eq!(items[1]["change_reason"], json!("initial"));
    // 陈旧活跃 Run 警告：绑定 r1 的 queued run。
    let stale = body["stale_active_runs"].as_array().unwrap();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0]["attempt_no"], json!(1));
    assert_eq!(stale[0]["status"], json!("queued"));
    assert_eq!(stale[0]["revision_no"], json!(1));
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_revisions_unknown_task_404s(pool: PgPool) {
    let app = router(pool.clone());
    let response = app
        .oneshot(get(format!(
            "/v1/pm/tasks/{}/revisions",
            uuid::Uuid::new_v4()
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ───────────────────────────────────── project products（E 系列 + 右栏）

#[sqlx::test(migrations = "../../migrations")]
async fn project_products_lists_linked_products_with_task_counts(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "PA07-P".into(),
            project_name: "pa07".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    join_dev(&pool, boot.project_id).await;
    let task_id = seed_task(&pool, boot.project_id, "auth api").await;
    let linked = seed_product(&pool, boot.project_id, "Auth service", "normal", false).await;
    sqlx::query(
        "INSERT INTO core.product_project_link (product_id, project_id, tenant_id) \
         VALUES ($1, $2, (SELECT tenant_id FROM core.product WHERE id = $1))",
    )
    .bind(linked)
    .bind(boot.project_id)
    .execute(&pool)
    .await
    .unwrap();
    // linked 产品下挂一个活跃任务关联（task_count=1）。
    sqlx::query(
        "INSERT INTO delivery.task_product (tenant_id, project_id, task_id, product_id) \
         SELECT tenant_id, project_id, $1, $2 FROM delivery.task WHERE id = $1",
    )
    .bind(task_id)
    .bind(linked)
    .execute(&pool)
    .await
    .unwrap();

    let app = router(pool.clone());
    let response = app
        .oneshot(get(format!("/v1/pm/projects/{}/products", boot.project_id)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "only linked products are listed");
    let row = &items[0];
    assert_eq!(row["product"]["name"], json!("Auth service"));
    assert_eq!(row["product"]["status"], json!("normal"));
    let key = row["product"]["key"].as_str().unwrap();
    assert!(
        key.starts_with("P-"),
        "product key must carry business key, got {key}"
    );
    assert_eq!(row["task_count"], json!(1));
    assert!(row.get("link_id").is_some());
}

#[sqlx::test(migrations = "../../migrations")]
async fn project_products_invisible_project_404s(pool: PgPool) {
    // 未 join 项目成员 → 可见性闸 404（不泄露存在性）。
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "PA07-P2".into(),
            project_name: "pa07".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let app = router(pool.clone());
    let response = app
        .oneshot(get(format!("/v1/pm/projects/{}/products", boot.project_id)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ─────────────────────────────────────── product tasks（E 页 Tasks tab）

#[sqlx::test(migrations = "../../migrations")]
async fn product_tasks_lists_linked_tasks_with_code_and_deps(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "PA07-T".into(),
            project_name: "pa07".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    join_dev(&pool, boot.project_id).await;
    let task_id = seed_task(&pool, boot.project_id, "implement api").await;
    let product_id = seed_product(&pool, boot.project_id, "Billing API", "suspended", true).await;
    sqlx::query(
        "INSERT INTO delivery.task_product (tenant_id, project_id, task_id, product_id) \
         SELECT tenant_id, project_id, $1, $2 FROM delivery.task WHERE id = $1",
    )
    .bind(task_id)
    .bind(product_id)
    .execute(&pool)
    .await
    .unwrap();
    // 一条未满足依赖边（deps 原料随行）。
    let pred = seed_task(&pool, boot.project_id, "pred").await;
    sqlx::query(
        "INSERT INTO delivery.task_dependency \
         (tenant_id, project_id, task_id, depends_on_task_id, created_by, required_condition) \
         SELECT tenant_id, project_id, $1, $2, $3, 'completed' FROM delivery.task WHERE id = $1",
    )
    .bind(task_id)
    .bind(pred)
    .bind(boot.human_id)
    .execute(&pool)
    .await
    .unwrap();

    let app = router(pool.clone());
    let response = app
        .oneshot(get(format!("/v1/pm/products/{product_id}/tasks")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    let row = &items[0];
    assert_eq!(row["title"], json!("implement api"));
    let code = row["code"].as_str().unwrap();
    assert!(code.starts_with("PM-"), "code must be PM-NNN, got {code}");
    assert_eq!(row["deps_total"], json!(1));
    assert_eq!(row["deps_met"], json!(0));
}

#[sqlx::test(migrations = "../../migrations")]
async fn product_tasks_non_member_404s(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "PA07-T2".into(),
            project_name: "pa07".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let product_id = seed_product(&pool, boot.project_id, "No member", "normal", false).await;
    let app = router(pool.clone());
    let response = app
        .oneshot(get(format!("/v1/pm/products/{product_id}/tasks")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
