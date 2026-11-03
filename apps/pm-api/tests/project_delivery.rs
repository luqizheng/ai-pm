//! 项目级交付聚合验收（S01-W60 / 缺陷 D-13；总体计划 §9.1 W60、§9.2、§15.E）。
//!
//! 覆盖：
//! - 正向：多任务多仓的项目聚合，任务级卷起（delivered / partial / missing / no_contract）
//! - 汇总计数与逐仓明细（基线 commit / 结果 commit / 测试证据）
//! - pending 审批数（Review 状态）
//! - 可见性：非成员 / 跨租户 404（T-022）
//! - 空项目：total=0、summary 全 0

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;
use pm_domain::SYSTEM_TENANT;

mod common;

fn get(uri: &str) -> Request<Body> {
    Request::builder().method("GET").uri(uri).body(Body::empty()).unwrap()
}

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    let project_id = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;
    common::join_dev(pool, project_id).await;
    project_id
}

async fn seed_task(pool: &PgPool, project_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
         VALUES ($1, $2, 'ready', (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_repo(pool: &PgPool, project_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, tenant_id) \
         VALUES ($1, $2, $3, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(name)
    .bind(format!("ssh://git@example.com/{name}.git"))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_task_repo(pool: &PgPool, project_id: Uuid, task_id: Uuid, repo_id: Uuid) {
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, tenant_id) \
         VALUES ($1, $2, $3, true, (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(repo_id)
    .execute(pool)
    .await
    .unwrap();
}

/// 一次已交付的 delivery run：基线 + 结果 commit 都落库。
/// 一次已交付的 delivery run：**一条 run 覆盖多个仓库**（视图 `latest_run` 按
/// task 取最新一条 run，逐仓契约必须在同一 run 内才被一起统计）。
async fn seed_delivered_run(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    attempt_no: i32,
    repos: &[(Uuid, &str, &str)],
) {
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
             (project_id, task_id, attempt_no, run_kind, status, outcome, integration_status, \
              started_at, finished_at, tenant_id) \
         VALUES ($1, $2, $3, 'delivery', 'succeeded', 'succeeded', 'passed', now(), now(), \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(attempt_no)
    .fetch_one(pool)
    .await
    .unwrap();
    for (repo_id, base_sha, result_sha) in repos {
        sqlx::query(
            "INSERT INTO delivery.task_run_repository \
                 (run_id, project_id, repository_id, base_ref, base_commit, write_access, \
                  workspace_path, tenant_id) \
             VALUES ($1, $2, $3, 'main', $4, true, $5, \
                     (SELECT tenant_id FROM core.project WHERE id = $2))",
        )
        .bind(run_id)
        .bind(project_id)
        .bind(repo_id)
        .bind(base_sha)
        .bind(format!("/ws/{run_id}"))
        .execute(pool)
        .await
        .unwrap();
        if !result_sha.is_empty() {
            sqlx::query(
                "INSERT INTO delivery.task_repository_commit \
                     (tenant_id, project_id, task_run_id, repository_id, commit_sha, branch) \
                 VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4, 'feature/x')",
            )
            .bind(project_id)
            .bind(run_id)
            .bind(repo_id)
            .bind(result_sha)
            .execute(pool)
            .await
            .unwrap();
        }
    }
}

/// 一个只有基线、没有交付 commit 的 run（missing 场景）。
async fn seed_contract_only_run(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    attempt_no: i32,
    repo_id: Uuid,
) {
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
             (project_id, task_id, attempt_no, run_kind, status, integration_status, tenant_id) \
         VALUES ($1, $2, $3, 'delivery', 'queued', 'not_run', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(attempt_no)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run_repository \
             (run_id, project_id, repository_id, base_ref, base_commit, write_access, \
              workspace_path, tenant_id) \
         VALUES ($1, $2, $3, 'main', 'base-only', true, $4, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(repo_id)
    .bind(format!("/ws/{run_id}"))
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_pending_approval(pool: &PgPool, project_id: Uuid, task_id: Uuid, run_id: Uuid) {
    sqlx::query(
        "INSERT INTO governance.review_approval \
             (project_id, task_id, review_run_id, delivery_run_id, kind, status, \
              delivery_manifest_hash, tenant_id) \
         VALUES ($1, $2, $3, $3, 'manual_review', 'pending', 'drill-hash', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(run_id)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn project_delivery_aggregates_task_rollups(pool: PgPool) {
    let project_id = seed_project(&pool, "PDEL-1").await;
    let web = seed_repo(&pool, project_id, "shop-web").await;
    let api = seed_repo(&pool, project_id, "shop-api").await;

    // 任务 A：两仓都已交付 → delivered。
    let task_a = seed_task(&pool, project_id, "全交付任务").await;
    seed_task_repo(&pool, project_id, task_a, web).await;
    seed_task_repo(&pool, project_id, task_a, api).await;
    seed_delivered_run(
        &pool,
        project_id,
        task_a,
        1,
        &[(web, "a-base", "a-result"), (api, "b-base", "b-result")],
    )
    .await;

    // 任务 B：一仓交付、一仓只有契约 → partial。
    let task_b = seed_task(&pool, project_id, "部分交付任务").await;
    seed_task_repo(&pool, project_id, task_b, web).await;
    seed_task_repo(&pool, project_id, task_b, api).await;
    seed_delivered_run(
        &pool,
        project_id,
        task_b,
        1,
        &[(web, "a-base", "a2-result"), (api, "b-base", "")],
    )
    .await;

    // 任务 C：只有契约、无任何交付 → missing。
    let task_c = seed_task(&pool, project_id, "未交付任务").await;
    seed_task_repo(&pool, project_id, task_c, web).await;
    seed_contract_only_run(&pool, project_id, task_c, 1, web).await;

    // 任务 D：无契约仓 → no_contract。
    let _task_d = seed_task(&pool, project_id, "无契约任务").await;

    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/delivery")))
            .await
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["total"], 4);
    let summary = &body["summary"];
    println!("DEBUG body = {}", serde_json::to_string_pretty(&body).unwrap());
    assert_eq!(summary["delivered"], 1, "body: {body}");
    assert_eq!(summary["partial"], 1);
    assert_eq!(summary["missing"], 1);
    assert_eq!(summary["no_contract"], 1);

    let items = body["items"].as_array().unwrap();
    let find = |title: &str| {
        items
            .iter()
            .find(|item| item["task_title"] == title)
            .unwrap_or_else(|| panic!("task {title} missing: {body}"))
    };

    let a = find("全交付任务");
    assert_eq!(a["delivery_state"], "delivered");
    assert_eq!(a["expected_delivery_count"], 2);
    assert_eq!(a["actual_delivery_count"], 2);
    let repos_a = a["repositories"].as_array().unwrap();
    assert_eq!(repos_a.len(), 2);
    assert!(
        repos_a
            .iter()
            .all(|repo| repo["delivery_state"] == "delivered"),
        "both repos must be delivered: {a}"
    );
    assert!(
        repos_a.iter().any(|repo| repo["commit_sha"] == "a-result"),
        "result commits must be visible: {a}"
    );
    assert!(
        repos_a.iter().any(|repo| repo["base_commit"] == "a-base"),
        "baseline commits must be visible: {a}"
    );

    let b = find("部分交付任务");
    assert_eq!(b["delivery_state"], "partial");
    assert_eq!(b["expected_delivery_count"], 2);
    assert_eq!(b["actual_delivery_count"], 1);

    let c = find("未交付任务");
    assert_eq!(c["delivery_state"], "missing");

    let d = find("无契约任务");
    assert_eq!(d["delivery_state"], "no_contract");
    assert_eq!(d["repositories"].as_array().unwrap().len(), 0);
}

/// Review 状态：pending 审批数随任务卷起。
#[sqlx::test(migrations = "../../migrations")]
async fn project_delivery_exposes_pending_review_count(pool: PgPool) {
    let project_id = seed_project(&pool, "PDEL-2").await;
    let web = seed_repo(&pool, project_id, "shop-web").await;
    let task = seed_task(&pool, project_id, "待审任务").await;
    seed_task_repo(&pool, project_id, task, web).await;
    seed_delivered_run(&pool, project_id, task, 1, &[(web, "base", "result")]).await;

    let run_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM delivery.task_run WHERE task_id = $1 LIMIT 1",
    )
    .bind(task)
    .fetch_one(&pool)
    .await
    .unwrap();
    seed_pending_approval(&pool, project_id, task, run_id).await;

    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/delivery")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let item = body["items"].as_array().unwrap()[0].clone();
    assert_eq!(item["pending_review_approvals"], 1, "body: {body}");
    assert_eq!(item["delivery_state"], "delivered");
}

/// 空项目：total=0、summary 全 0（不造假数据）。
#[sqlx::test(migrations = "../../migrations")]
async fn empty_project_returns_zero_summary(pool: PgPool) {
    let project_id = seed_project(&pool, "PDEL-3").await;
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!("/v1/pm/projects/{project_id}/delivery")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["total"], 0);
    assert_eq!(body["summary"]["delivered"], 0);
    assert_eq!(body["summary"]["partial"], 0);
    assert_eq!(body["summary"]["missing"], 0);
    assert_eq!(body["summary"]["no_contract"], 0);
}

/// 可见性：非成员 / 跨租户 → 404（T-022，不泄露存在性）。
#[sqlx::test(migrations = "../../migrations")]
async fn invisible_project_is_404(pool: PgPool) {
    let project_id = seed_project(&pool, "PDEL-4").await;

    // 第二个项目（另一个租户上下文不可达——同一调用者非其成员）。
    let other_id = seed_project(&pool, "PDEL-5").await;
    // 移除调用者（dev-off）在 other 项目的成员行，使其不可见。
    sqlx::query("DELETE FROM core.project_member WHERE project_id = $1")
        .bind(other_id)
        .execute(&pool)
        .await
        .unwrap();

    for target in [other_id] {
        let (status, body) = body_json(
            router(pool.clone())
                .oneshot(get(&format!("/v1/pm/projects/{target}/delivery")))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "project {target}: {body}");
    }
    let _ = project_id;
}
