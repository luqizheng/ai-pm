//! S01 · I5 项目总控台验收（I5-C 后端读模型）。
//!
//! 契约：`docs/s01-i5-read-model-contract.md`；事实：`docs/s01-i5-code-facts.md`
//!
//! 覆盖：
//! - I5-001/002 新建项目正常展示；**零仓库合法**（不判初始化失败）
//! - I5-009/010/011 待人工处理（Finding 待确认 / 待补充）；跨项目不泄露
//! - I5-012/013 当前执行（活跃 Run + 执行主体取 `task.assignee_id`）
//! - I5-022/029 无无依据百分比；初始化完成度**不可判定**
//! - I5-025/030/031 最近活动只展示白名单事件且去重
//! - I5-019/036 归档只读

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_domain::SYSTEM_TENANT;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod common;

fn request(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn seed_project(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
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

/// 造一条 PENDING Finding（+ 可选的「要求补充」审计）。
async fn seed_finding(
    pool: &PgPool,
    project_id: Uuid,
    session_id: Uuid,
    revision_requested: bool,
) -> Uuid {
    let finding_id: Uuid = sqlx::query_scalar(
        "INSERT INTO discovery.discovery_finding \
            (project_id, session_id, kind, content, confidence, verification_status, actor_role, tenant_id) \
         VALUES ($1, $2, 'observed', '待人工确认的发现', 0.8, 'PENDING', 'human', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(session_id)
    .fetch_one(pool)
    .await
    .unwrap();
    if revision_requested {
        sqlx::query(
            "INSERT INTO governance.audit_log \
                (project_id, actor_id, action, target_type, target_id, tenant_id) \
             VALUES ($1, (SELECT principal_id FROM core.project_member WHERE project_id = $1 LIMIT 1), \
                     'discovery_finding_revision_requested', 'discovery_finding', $2, \
                     (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(project_id)
        .bind(finding_id)
        .execute(pool)
        .await
        .unwrap();
    }
    finding_id
}

async fn seed_session(pool: &PgPool, project_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO discovery.discovery_session \
            (project_id, status, source, kind, created_by, tenant_id) \
         VALUES ($1, 'IN_PROGRESS', 'SYSTEM', 'INITIAL_ONBOARDING', \
                 (SELECT principal_id FROM core.project_member WHERE project_id = $1 LIMIT 1), \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn get_center(pool: &PgPool, project_id: Uuid) -> (StatusCode, Value) {
    let app = router(pool.clone());
    body_json(
        app.oneshot(request(&format!(
            "/v1/pm/projects/{project_id}/command-center"
        )))
        .await
        .unwrap(),
    )
    .await
}

fn block<'a>(body: &'a Value, name: &str) -> &'a Value {
    &body["blocks"][name]
}

/// I5-001/002：新建项目正常展示；零仓库合法——仓库「未开始」、分析「不适用」，
/// 初始化完成度**不可判定**，且不产出任何百分比。
#[sqlx::test(migrations = "../../migrations")]
async fn new_project_renders_all_blocks_and_zero_repository_is_legal(pool: PgPool) {
    let (project_id, _owner) = seed_project(&pool, "I5-1").await;
    let (status, body) = get_center(&pool, project_id).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    for name in [
        "project_summary",
        "initialization_progress",
        "work_summary",
        "human_action_summary",
        "risk_summary",
        "recent_activity",
    ] {
        assert_eq!(block(&body, name)["status"], json!("ok"), "block {name}: {body}");
        assert!(block(&body, name)["as_of"].is_string(), "block {name} 缺新鲜度");
    }

    let summary = block(&body, "project_summary")["data"].clone();
    assert_eq!(summary["project_id"], json!(project_id.to_string()));
    assert_eq!(summary["archived"], json!(false));
    assert_eq!(summary["repository_count"], json!(0));
    assert_eq!(summary["goal"]["present"], json!(false));

    let init = block(&body, "initialization_progress")["data"].clone();
    assert_eq!(init["completion"], json!("not_determinable"), "不得宣称初始化已完成");
    let items = init["items"].as_array().unwrap();
    let by_key = |key: &str| {
        items
            .iter()
            .find(|item| item["key"] == json!(key))
            .unwrap_or_else(|| panic!("缺少事项 {key}"))
            .clone()
    };
    assert_eq!(by_key("repository_access")["status"], json!("not_started"));
    assert_eq!(by_key("code_analysis")["status"], json!("not_applicable"));
    assert_eq!(by_key("project_basics")["status"], json!("completed"));
    assert_eq!(by_key("project_basics")["required"], json!(true));
    assert_eq!(by_key("repository_access")["required"], json!(false));

    // 无百分比（I5-022）：整块不得出现 percent / percentage 字段。
    let raw = body.to_string().to_lowercase();
    assert!(!raw.contains("percent"), "总控台不得输出百分比");
}

/// I5-009/010/011：待人工处理聚合（待确认 / 待补充）+ 跨项目不泄露。
#[sqlx::test(migrations = "../../migrations")]
async fn pending_findings_appear_as_human_actions_without_cross_project_leak(pool: PgPool) {
    let (project_id, _owner) = seed_project(&pool, "I5-2").await;
    let (other_project, _other_owner) = seed_project(&pool, "I5-2-OTHER").await;
    let session = seed_session(&pool, project_id).await;
    let other_session = seed_session(&pool, other_project).await;
    let pending = seed_finding(&pool, project_id, session, false).await;
    let revision = seed_finding(&pool, project_id, session, true).await;
    let other_finding = seed_finding(&pool, other_project, other_session, false).await;

    let (status, body) = get_center(&pool, project_id).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let actions = block(&body, "human_action_summary")["data"].clone();
    let items = actions["items"].as_array().unwrap();
    let ids: Vec<String> = items
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect();
    assert!(ids.contains(&pending.to_string()), "待确认 Finding 必须在列表中");
    assert!(ids.contains(&revision.to_string()), "待补充 Finding 必须在列表中");
    assert!(
        !ids.contains(&other_finding.to_string()),
        "不得泄露其它 Project 的 Finding"
    );
    // 展示态来自 I2 权威推导（不是 open question）。
    let revision_item = items
        .iter()
        .find(|item| item["id"] == json!(revision.to_string()))
        .unwrap();
    assert_eq!(revision_item["kind"], json!("finding_revision_requested"));
    assert_eq!(revision_item["source_state"], json!("REVISION_REQUESTED"));
    let pending_item = items
        .iter()
        .find(|item| item["id"] == json!(pending.to_string()))
        .unwrap();
    assert_eq!(pending_item["kind"], json!("finding_pending"));
    assert_eq!(pending_item["source_state"], json!("PENDING"));
    // 每条都带项目归属（可证属本项目）。
    for item in items {
        assert_eq!(item["project_id"], json!(project_id.to_string()));
    }
}

/// I5-012/013：当前执行——活跃 Run 计数 + 执行主体取 `task.assignee_id`
/// （`task_run.agent_id` 恒 NULL，不得使用）。
#[sqlx::test(migrations = "../../migrations")]
async fn active_runs_report_the_task_assignee_as_agent(pool: PgPool) {
    let (project_id, _owner) = seed_project(&pool, "I5-3").await;
    let developer: Uuid = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member \
          WHERE project_id = $1 AND role = 'developer' LIMIT 1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, assignee_id, tenant_id) \
         VALUES ($1, '执行中的任务', 'running', $2, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(developer)
    .fetch_one(&pool)
    .await
    .unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, run_kind, status, tenant_id) \
         VALUES ($1, $2, 1, 'delivery', 'running', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let (status, body) = get_center(&pool, project_id).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let work = block(&body, "work_summary")["data"].clone();
    assert_eq!(work["active_run_count"], json!(1));
    assert_eq!(work["task_by_status"]["running"], json!(1));
    let active = work["active_runs"].as_array().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0]["run_id"], json!(run_id.to_string()));
    assert_eq!(active[0]["status"], json!("running"));
    assert_eq!(
        active[0]["agent"]["principal_id"],
        json!(developer.to_string()),
        "执行主体必须是 task.assignee_id"
    );
    assert_eq!(active[0]["agent"]["kind"], json!("dev_agent"));
}

/// I5-025/030/031：最近活动只展示白名单事件；同 dedup_key 只保留一条。
#[sqlx::test(migrations = "../../migrations")]
async fn recent_activity_filters_whitelist_and_dedups(pool: PgPool) {
    let (project_id, _owner) = seed_project(&pool, "I5-4").await;
    let actor: Uuid = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member WHERE project_id = $1 LIMIT 1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let target: Uuid = Uuid::new_v4();
    // 白名单内：两条同 target 同 action（应去重为一条）。
    for _ in 0..2 {
        sqlx::query(
            "INSERT INTO governance.audit_log \
                (project_id, actor_id, action, target_type, target_id, tenant_id) \
             VALUES ($1, $2, 'finding.decided', 'discovery_finding', $3, \
                     (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(project_id)
        .bind(actor)
        .bind(target)
        .execute(&pool)
        .await
        .unwrap();
    }
    // 白名单外：技术性动作不得展示。
    sqlx::query(
        "INSERT INTO governance.audit_log \
            (project_id, actor_id, action, target_type, target_id, tenant_id) \
         VALUES ($1, $2, 'renew_run_lease', 'task_run', $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(actor)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = get_center(&pool, project_id).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let activity = block(&body, "recent_activity")["data"].clone();
    let items = activity["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "同 dedup_key 只保留最新一条：{items:?}");
    assert_eq!(items[0]["activity_type"], json!("finding_decided"));
    assert_eq!(items[0]["source_id"], json!(target.to_string()));
    assert!(!items[0]["summary"].as_str().unwrap().is_empty(), "活动必须有可读摘要");
    // 白名单外的事件不进入展示流。
    assert!(
        !items.iter().any(|item| item["activity_type"] == json!("run_started")),
        "白名单外事件不得展示"
    );
}

/// I5-019/036：归档后表现为只读态。
#[sqlx::test(migrations = "../../migrations")]
async fn archived_project_is_reported_read_only(pool: PgPool) {
    let (project_id, _owner) = seed_project(&pool, "I5-5").await;
    sqlx::query("UPDATE core.project SET status = 'archived' WHERE id = $1")
        .bind(project_id)
        .execute(&pool)
        .await
        .unwrap();

    let (status, body) = get_center(&pool, project_id).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let summary = block(&body, "project_summary")["data"].clone();
    assert_eq!(summary["archived"], json!(true));
    assert_eq!(summary["lifecycle"], json!("archived"));
}

/// I5-017/018：非成员访问 → 404（不泄露存在性）。
#[sqlx::test(migrations = "../../migrations")]
async fn non_member_cannot_read_command_center(pool: PgPool) {
    let (_project_id, _owner) = seed_project(&pool, "I5-6").await;
    // 另一个项目：dev-off 不是其成员 → 404。
    let (other_project, _other_owner) = seed_project(&pool, "I5-6-OTHER").await;
    sqlx::query(
        "DELETE FROM core.project_member \
         WHERE project_id = $1 AND principal_id = $2",
    )
    .bind(other_project)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap();

    let (status, _body) = get_center(&pool, other_project).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "非成员不得读取总控台，也不泄露存在性"
    );
}
