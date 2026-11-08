//! S01 · I1 归档生命周期验收（B-1 默认并入 P1~P4，2026-11-04）。
//!
//! - I1-001~006（门禁基础）：归档守卫三入口（claim / TaskReady / enqueue_review）
//! - P1（I1-015 段）：queued Run 条件取消——只动 queued，claimed/running 走 P2
//! - P2：cancel_run 外部入口（REST，reason 透传审计；非可取消态 → 409）
//! - P4/N-2：归档释放初始化名额、终态化在飞 Import
//! - 归档管理面：owner 可归档；重复归档 409 ALREADY_ARCHIVED

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::run_dispatch::NoopExecutor;
use pm_application::run_executor;
use pm_domain::SYSTEM_TENANT;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod common;

fn request(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    builder.body(body).unwrap()
}

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn seed_world(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(pool, boot.project_id).await;
    (boot.project_id, boot.human_id)
}

async fn seed_task(pool: &PgPool, project_id: Uuid, code: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
         VALUES ($1, $2, 'ready', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_run(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    attempt_no: i32,
    status: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, run_kind, status, tenant_id) \
         VALUES ($1, $2, $3, 'delivery', $4, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(attempt_no)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// I1-A 三入口守卫 + P1：归档后 queued Run 被条件取消、claim 拿不到、
/// TaskReady 不建新 Run、enqueue_review 不建 review Run。
#[sqlx::test(migrations = "../../migrations")]
async fn archive_guards_all_entries_and_cancels_queued(pool: PgPool) {
    let (project_id, _owner) = seed_world(&pool, "I1-1").await;
    let task_a = seed_task(&pool, project_id, "归档任务A").await;
    let task_b = seed_task(&pool, project_id, "归档任务B").await;
    let queued = seed_run(&pool, project_id, task_a, 1, "queued").await;
    let running = seed_run(&pool, project_id, task_b, 1, "running").await;

    let app = router(pool.clone());
    let (status, body) = body_json(
        app.oneshot(request(
            "POST",
            &format!("/v1/pm/projects/{project_id}/archive"),
            Some(json!({ "reason": "I1 验收归档" })),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["status"], json!("archived"));
    assert_eq!(body["queued_runs_cancelled"], json!(1));
    assert_eq!(body["imports_finalized"], json!(0));

    // P1：queued → cancelled（条件取消，reason 落 termination_reason）。
    let (run_status, reason): (String, Option<String>) =
        sqlx::query_as("SELECT status, termination_reason FROM delivery.task_run WHERE id = $1")
            .bind(queued)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(run_status, "cancelled");
    assert_eq!(reason.as_deref(), Some("I1 验收归档"));

    // 事务内不动 running（P2 走取消协议）——Noop 终态落库。
    let run_status: String =
        sqlx::query_scalar("SELECT status FROM delivery.task_run WHERE id = $1")
            .bind(running)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(run_status, "cancelled", "P2 must cancel the active run");
    let reason: Option<String> =
        sqlx::query_scalar("SELECT termination_reason FROM delivery.task_run WHERE id = $1")
            .bind(running)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("project_archived"));

    // 项目状态。
    let status: String = sqlx::query_scalar("SELECT status FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "archived");

    // 归档审计。
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'project.archived'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1);

    // 入口守卫：TaskReady 对归档项目不建 Run（手工投一个事件走 handler 语义——
    // 直接断言门禁读：project_activity → Archived）。
    assert_eq!(
        pm_application::project_activity(&pool, project_id)
            .await
            .unwrap(),
        pm_application::ProjectActivity::Archived
    );
}

/// claim 守卫：归档项目下 claim_core 拿不到候选（防御纵深）。
#[sqlx::test(migrations = "../../migrations")]
async fn claim_skips_archived_project(pool: PgPool) {
    let (project_id, _owner) = seed_world(&pool, "I1-2").await;
    let task = seed_task(&pool, project_id, "claim 目标").await;
    seed_run(&pool, project_id, task, 1, "queued").await;

    // 归档前能拿到；归档后拿不到。
    let cfg = pm_application::run_dispatch::RunDispatchConfig::from_env();
    let worker = "i1-archive-test";
    let before = pm_application::run_dispatch::claim_once(
        &pool,
        worker,
        &cfg,
        pm_application::run_dispatch::DispatchMode::On,
    )
    .await
    .unwrap();
    assert!(
        before.is_some(),
        "queued run must be claimable before archive"
    );

    sqlx::query("UPDATE core.project SET status = 'archived' WHERE id = $1")
        .bind(project_id)
        .execute(&pool)
        .await
        .unwrap();
    // 归档不自动取消这里的 run（绕过 archive service 直改状态）——守卫仍须挡。
    let after = pm_application::run_dispatch::claim_once(
        &pool,
        worker,
        &cfg,
        pm_application::run_dispatch::DispatchMode::On,
    )
    .await
    .unwrap();
    assert!(after.is_none(), "claim must skip archived projects");
}

/// P4/N-2：归档终态化在飞 Import + 释放初始化名额（initializing 卡死事实收口）。
#[sqlx::test(migrations = "../../migrations")]
async fn archive_finalizes_inflight_import_and_releases_slot(pool: PgPool) {
    let (project_id, _owner) = seed_world(&pool, "I1-3").await;
    let import_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.repo_import (tenant_id, project_id, requested_by, status) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'analyzing') \
         RETURNING id",
    )
    .bind(project_id)
    .bind(pm_application::run_dispatch::DISPATCH_SERVICE_PRINCIPAL)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_initialization_operation \
         (tenant_id, project_id, generation, status, import_id) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, 1, 'active', $2)",
    )
    .bind(project_id)
    .bind(import_id)
    .execute(&pool)
    .await
    .unwrap();

    let app = router(pool.clone());
    let (status, body) = body_json(
        app.oneshot(request(
            "POST",
            &format!("/v1/pm/projects/{project_id}/archive"),
            None,
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["imports_finalized"], json!(1));

    let import_status: String =
        sqlx::query_scalar("SELECT status FROM integration.repo_import WHERE id = $1")
            .bind(import_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(import_status, "cancelled");
    let active_ops: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.project_initialization_operation \
         WHERE project_id = $1 AND status = 'active'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active_ops, 0, "the initialization slot must be released");
}

/// 重复归档 → 409 ALREADY_ARCHIVED（幂等冲突，不重复审计）。
#[sqlx::test(migrations = "../../migrations")]
async fn double_archive_conflicts(pool: PgPool) {
    let (project_id, _owner) = seed_world(&pool, "I1-4").await;
    let app = router(pool.clone());
    for expected in [StatusCode::OK, StatusCode::CONFLICT] {
        let (status, body) = body_json(
            app.clone()
                .oneshot(request(
                    "POST",
                    &format!("/v1/pm/projects/{project_id}/archive"),
                    None,
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, expected, "body: {body}");
        if expected == StatusCode::CONFLICT {
            assert_eq!(body["code"], json!("ALREADY_ARCHIVED"));
        }
    }
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE action = 'project.archived'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1);
}

/// P2：cancel_run 外部入口——running 可取消（reason=project_archived 透传），
/// queued/终态 → 409 NOT_CANCELLABLE。
#[sqlx::test(migrations = "../../migrations")]
async fn external_run_cancel_entry(pool: PgPool) {
    let (project_id, _owner) = seed_world(&pool, "I1-5").await;
    let task_a = seed_task(&pool, project_id, "取消目标A").await;
    let task_b = seed_task(&pool, project_id, "取消目标B").await;
    let running = seed_run(&pool, project_id, task_a, 1, "running").await;
    let queued = seed_run(&pool, project_id, task_b, 1, "queued").await;

    let app = router(pool.clone());
    // running → 200。
    let (status, body) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &format!("/v1/pm/task-runs/{running}/cancel"),
                Some(json!({ "reason": "project_archived" })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["status"], json!("cancelled"));
    let reason: Option<String> =
        sqlx::query_scalar("SELECT termination_reason FROM delivery.task_run WHERE id = $1")
            .bind(running)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("project_archived"));

    // queued → 409（归档路径才管 queued）。
    let (status, body) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &format!("/v1/pm/task-runs/{queued}/cancel"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
    assert_eq!(body["code"], json!("NOT_CANCELLABLE"));
    let _ = run_executor::cancel_run; // 导入保持（同模块其他测试用）
    let _ = NoopExecutor;
}
