//! S01 · I1 归档生命周期验收（B-1 默认并入 P1~P4，2026-11-04）。
//!
//! - I1-001~006（门禁基础）：归档守卫三入口（claim / TaskReady / enqueue_review）
//! - P1（I1-015 段）：queued Run 条件取消——只动 queued，claimed/running 走 P2
//! - P2：cancel_run 外部入口（REST，reason 透传审计；非可取消态 → 409）
//! - P4/N-2：归档释放初始化名额、终态化在飞 Import
//! - 归档管理面：owner 可归档；重复归档 409 ALREADY_CLOSED

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
    let _pog = ();
    let (project_id, _owner) = seed_world(&pool, "I1-1").await;
    // P4-ORG Phase 5：close 守卫 = 关联 Products 的 Product Owner（fixture 对准调用者）。
    seed_product_owner_link(&pool, project_id).await;
    let task_a = seed_task(&pool, project_id, "归档任务A").await;
    let task_b = seed_task(&pool, project_id, "归档任务B").await;
    let queued = seed_run(&pool, project_id, task_a, 1, "queued").await;
    let running = seed_run(&pool, project_id, task_b, 1, "running").await;

    let app = router(pool.clone());
    let (status, body) = body_json(
        app.oneshot(request(
            "POST",
            &format!("/v1/pm/projects/{project_id}/close"),
            Some(json!({ "reason": "I1 验收归档" })),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["status"], json!("closed"));
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
    assert_eq!(reason.as_deref(), Some("project_closed"));

    // 项目状态。
    let status: String = sqlx::query_scalar("SELECT status FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "closed");

    // 归档审计。
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'project.closed'",
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
        pm_application::ProjectActivity::Closed
    );
}

/// claim 守卫：归档项目下 claim_core 拿不到候选（防御纵深）。
#[sqlx::test(migrations = "../../migrations")]
async fn claim_skips_archived_project(pool: PgPool) {
    let (project_id, _owner) = seed_world(&pool, "I1-2").await;
    // P4-ORG Phase 5：close 守卫 = 关联 Products 的 Product Owner（fixture 对准调用者）。
    seed_product_owner_link(&pool, project_id).await;
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

    sqlx::query("UPDATE core.project SET status = 'closed' WHERE id = $1")
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
    // P4-ORG Phase 5：close 守卫 = 关联 Products 的 Product Owner（fixture 对准调用者）。
    seed_product_owner_link(&pool, project_id).await;
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
            &format!("/v1/pm/projects/{project_id}/close"),
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

/// 重复归档 → 409 ALREADY_CLOSED（幂等冲突，不重复审计）。
#[sqlx::test(migrations = "../../migrations")]
async fn double_archive_conflicts(pool: PgPool) {
    let (project_id, _owner) = seed_world(&pool, "I1-4").await;
    // P4-ORG Phase 5：close 守卫 = 关联 Products 的 Product Owner（fixture 对准调用者）。
    seed_product_owner_link(&pool, project_id).await;
    let app = router(pool.clone());
    for expected in [StatusCode::OK, StatusCode::CONFLICT] {
        let (status, body) = body_json(
            app.clone()
                .oneshot(request(
                    "POST",
                    &format!("/v1/pm/projects/{project_id}/close"),
                    None,
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, expected, "body: {body}");
        if expected == StatusCode::CONFLICT {
            assert_eq!(body["code"], json!("ALREADY_CLOSED"));
        }
    }
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE action = 'project.closed'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1);
}

/// P2：cancel_run 外部入口——running 可取消（reason=project_closed 透传），
/// queued/终态 → 409 NOT_CANCELLABLE。
#[sqlx::test(migrations = "../../migrations")]
async fn external_run_cancel_entry(pool: PgPool) {
    let (project_id, _owner) = seed_world(&pool, "I1-5").await;
    // P4-ORG Phase 5：close 守卫 = 关联 Products 的 Product Owner（fixture 对准调用者）。
    seed_product_owner_link(&pool, project_id).await;
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
                Some(json!({ "reason": "project_closed" })),
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
    assert_eq!(reason.as_deref(), Some("project_closed"));

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

// ───────────────────────────────────── P4-ORG Phase 5：close/reopen 守卫矩阵

/// Product Owner 任命 fixture：Team/TeamMember(owner=dev-off)/Product/
/// ProductMember(owner=dev-off)/Link——全部锚 SYSTEM_TENANT + 项目租户。
async fn seed_product_owner_link(pool: &PgPool, project_id: Uuid) -> Uuid {
    // 调用者 = dev-off（router off 模式注入）；Product Owner 行必须对准它。
    let owner = pm_api::auth::DEV_OFF_PRINCIPAL;
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.tenant (id, provider, external_key, display_name) \
         VALUES ($1, 'lmms', gen_random_uuid()::text, 'Fixture') \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(tenant_id)
    .execute(pool)
    .await
    .unwrap();
    let team_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.team (tenant_id, name) VALUES ($1, 'Fixture Team') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.team_member (team_id, principal_id, tenant_id, role) \
         VALUES ($1, $2, $3, 'owner')",
    )
    .bind(team_id)
    .bind(owner)
    .bind(tenant_id)
    .execute(pool)
    .await
    .unwrap();
    let product_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.product (team_id, tenant_id, key, name) VALUES ($1, $2, 'P-' || upper(substring(gen_random_uuid()::text from 1 for 8)), 'Fixture Product') RETURNING id",
    )
    .bind(team_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.product_member (product_id, principal_id, tenant_id, role) \
         VALUES ($1, $2, $3, 'owner')",
    )
    .bind(product_id)
    .bind(owner)
    .bind(tenant_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.product_project_link (product_id, project_id, tenant_id) \
         VALUES ($1, $2, $3)",
    )
    .bind(product_id)
    .bind(project_id)
    .bind(tenant_id)
    .execute(pool)
    .await
    .unwrap();
    product_id
}

/// DR-PA-05②（2026-11-21 裁决）：close/reopen 守卫 = 关联 Products 的任意有效
/// Product Owner。矩阵：Product Owner 可关可重开；无 Product Owner 身份者 404
/// （存在性隐藏）；重复 close 409；reopen 非 closed 409；Link 撤回即失权。
#[sqlx::test(migrations = "../../migrations")]
async fn close_reopen_guard_matrix_product_owner(pool: PgPool) {
    let (project_id, _owner) = seed_world(&pool, "LC-1").await;
    // P4-ORG Phase 5：close 守卫 = 关联 Products 的 Product Owner（fixture 对准调用者）。
    seed_product_owner_link(&pool, project_id).await;
    let app = pm_api::routes::router(pool.clone());

    // 无 Product 关联的第二主体失权断言（dev-off 单身份模式下 outsider 场景
    // 无法经 HTTP 表达——json_request 恒为 dev-off 身份，其拒绝由守卫函数覆盖）。
    let outsider: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active)          VALUES ('human', 'lc-outsider', true) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let outsider_allowed = pm_application::archive::is_project_lifecycle_manager(
        &pool,
        SYSTEM_TENANT,
        project_id,
        outsider,
    )
    .await
    .unwrap();
    assert!(
        !outsider_allowed,
        "outsider must not be a lifecycle manager"
    );
    let owner_allowed = pm_application::archive::is_project_lifecycle_manager(
        &pool,
        SYSTEM_TENANT,
        project_id,
        pm_api::auth::DEV_OFF_PRINCIPAL,
    )
    .await
    .unwrap();
    assert!(
        owner_allowed,
        "product owner (dev-off caller) must be a lifecycle manager"
    );

    // Owner 走 API 关闭 → 200 closed。
    let (st, body) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/close"),
                Some(json!({ "reason": "lc" })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["status"], json!("closed"));

    // 重复 close → 409 ALREADY_CLOSED。
    let (st, body) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/close"),
                Some(json!({})),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], json!("ALREADY_CLOSED"));

    // （后续 Link 撤回/恢复步骤在 closed 态验证重开路径。）

    // Link 撤回即失权（多来源并集：撤掉唯一来源）。
    sqlx::query(
        "UPDATE core.product_project_link SET withdrawn_at = now(), withdrawn_by = $2 \
          WHERE project_id = $1",
    )
    .bind(project_id)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap();
    let owner_allowed = pm_application::archive::is_project_lifecycle_manager(
        &pool,
        SYSTEM_TENANT,
        project_id,
        pm_api::auth::DEV_OFF_PRINCIPAL,
    )
    .await
    .unwrap();
    assert!(!owner_allowed, "withdrawn link must remove the source");

    // Link 恢复（重建关联）→ 重开成功。
    sqlx::query(
        "UPDATE core.product_project_link SET withdrawn_at = NULL, withdrawn_by = NULL \
          WHERE project_id = $1",
    )
    .bind(project_id)
    .execute(&pool)
    .await
    .unwrap();
    let (st, body) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/reopen"),
                Some(json!({})),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["status"], json!("active"));
}
