//! `/v1/pm/*` 写端点契约测试（Slice A）：decide / tasks create+update。
//! 契约源：`frontend/src/api/pm.ts` 注释与 `frontend/src/api/mock/fixtures.ts`
//! （校验顺序 404 → 409 → 400 decision → 400 reason；错误体 `{message}`）。
//! 需求修订起草/定稿的契约测试已随旧 approve 直跳面退役（S02D-04，0039 批），
//! 新协议（submit/confirm/reject + CAS + 六要素）由 `pm_requirement.rs` 全量覆盖。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

fn request(method: &str, uri: String, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(body) = &body {
        builder = builder.header("content-type", "application/json");
        builder.body(Body::from(body.to_string())).unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    }
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// 审批决策链种子：task → event → analysis_run → proposal(proposed) →
/// approval_request(pending)，proposed_data 为可执行的 UpdateTaskStatus 命令。
async fn seed_pending_approval(pool: &PgPool, project_id: Uuid, actor_id: Uuid) -> Uuid {
    let task_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.task (id, project_id, title, status, revision, tenant_id) \
         VALUES ($1, $2, 'DECIDE-TASK', 'running', 2, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(project_id)
    .execute(pool)
    .await
    .unwrap();

    let event_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO integration.inbound_event \
         (id, project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash, tenant_id) \
         VALUES ($1, $2, $3, 'test', 'bug_report', $4, '{}', 'hash', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(event_id)
    .bind(project_id)
    .bind(actor_id)
    .bind(Uuid::new_v4().to_string())
    .execute(pool)
    .await
    .unwrap();
    let run_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO ai.analysis_run \
         (id, project_id, event_id, model_name, policy_version, config_snapshot, tenant_id) \
         VALUES ($1, $2, $3, 'fake-model', 1, '{}', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(event_id)
    .execute(pool)
    .await
    .unwrap();

    let proposal_id = Uuid::new_v4();
    let proposed_data = json!({
        "kind": "update_task_status",
        "data": { "task_id": task_id, "expected_revision": 2, "new_status": "submitted" }
    });
    sqlx::query(
        "INSERT INTO ai.change_proposal \
         (id, project_id, analysis_run_id, proposal_seq, target_type, target_id, command_type, \
          proposed_data, rationale, risk_level, status, tenant_id) \
         VALUES ($1, $2, $3, 0, 'task', $4, 'update_task_status', $5, 'because', 'low', 'proposed', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(proposal_id)
    .bind(project_id)
    .bind(run_id)
    .bind(task_id)
    .bind(&proposed_data)
    .execute(pool)
    .await
    .unwrap();

    let approval_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO governance.approval_request \
         (id, project_id, proposal_id, status, required_role, tenant_id) \
         VALUES ($1, $2, $3, 'pending', 'project_owner', \
                 (SELECT tenant_id FROM ai.change_proposal WHERE id = $3))",
    )
    .bind(approval_id)
    .bind(project_id)
    .bind(proposal_id)
    .execute(pool)
    .await
    .unwrap();
    approval_id
}

async fn proposal_status(pool: &PgPool, proposal_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM ai.change_proposal WHERE id = $1")
        .bind(proposal_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn approval_proposal_id(pool: &PgPool, approval_id: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT proposal_id FROM governance.approval_request WHERE id = $1")
        .bind(approval_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ------------------------------------------------------------------ decide

#[sqlx::test(migrations = "../../migrations")]
async fn decide_unknown_approval_is_404_even_with_invalid_body(pool: PgPool) {
    let app = router(pool.clone());
    // mock 校验顺序：存在性先于 body 校验。
    let response = app
        .oneshot(request(
            "POST",
            format!("/v1/pm/approvals/{}/decide", Uuid::new_v4()),
            Some(json!({ "decision": "nonsense", "reason": "" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_json(response).await;
    assert_eq!(body["message"], json!("not found"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn decide_rejects_invalid_decision_and_empty_reason_with_400(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "DEC-1".into(),
            project_name: "d".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let approval_id = seed_pending_approval(&pool, boot.project_id, boot.human_id).await;
    let app = router(pool.clone());

    let response = app
        .clone()
        .oneshot(request(
            "POST",
            format!("/v1/pm/approvals/{approval_id}/decide"),
            Some(json!({ "decision": "maybe", "reason": "ok" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["message"], json!("invalid decision"));

    let response = app
        .oneshot(request(
            "POST",
            format!("/v1/pm/approvals/{approval_id}/decide"),
            Some(json!({ "decision": "approved", "reason": "   " })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["message"], json!("reason is required"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn decide_approve_executes_proposal_and_answers_contract_body(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "DEC-2".into(),
            project_name: "d".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let approval_id = seed_pending_approval(&pool, boot.project_id, boot.human_id).await;
    let app = router(pool.clone());

    let response = app
        .oneshot(request(
            "POST",
            format!("/v1/pm/approvals/{approval_id}/decide"),
            Some(json!({ "decision": "approved", "reason": "lgtm" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["id"], json!(approval_id.to_string()));
    assert_eq!(body["status"], json!("approved"));
    assert!(
        body["decided_at"].is_string(),
        "decided_at must be an ISO timestamp"
    );

    let proposal_id = approval_proposal_id(&pool, approval_id).await;
    assert_eq!(proposal_status(&pool, proposal_id).await, "executed");

    // 红线执行的审计行存在。
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND source_kind = 'redline_approval'",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 1, "approve must leave a redline audit row");
}

#[sqlx::test(migrations = "../../migrations")]
async fn decide_reject_vetoes_proposal_and_answers_contract_body(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "DEC-3".into(),
            project_name: "d".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let approval_id = seed_pending_approval(&pool, boot.project_id, boot.human_id).await;
    let app = router(pool.clone());

    let response = app
        .oneshot(request(
            "POST",
            format!("/v1/pm/approvals/{approval_id}/decide"),
            Some(json!({ "decision": "rejected", "reason": "risky" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["status"], json!("rejected"));
    assert!(body["decided_at"].is_string());

    let proposal_id = approval_proposal_id(&pool, approval_id).await;
    assert_eq!(proposal_status(&pool, proposal_id).await, "vetoed");
}

#[sqlx::test(migrations = "../../migrations")]
async fn decide_twice_conflicts_with_409(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "DEC-4".into(),
            project_name: "d".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let approval_id = seed_pending_approval(&pool, boot.project_id, boot.human_id).await;
    let app = router(pool.clone());
    let body = json!({ "decision": "rejected", "reason": "first" });

    let first = app
        .clone()
        .oneshot(request(
            "POST",
            format!("/v1/pm/approvals/{approval_id}/decide"),
            Some(body.clone()),
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .oneshot(request(
            "POST",
            format!("/v1/pm/approvals/{approval_id}/decide"),
            Some(body),
        ))
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CONFLICT);
    let body = body_json(second).await;
    assert_eq!(body["message"], json!("already decided"));
}

// -------------------------------------------------------------- tasks CRUD

#[sqlx::test(migrations = "../../migrations")]
async fn create_task_defaults_and_contract_shape(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "TSK-1".into(),
            project_name: "t".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let app = router(pool.clone());

    let response = app
        .oneshot(request(
            "POST",
            "/v1/pm/tasks".into(),
            Some(json!({ "project_id": boot.project_id, "title": "  new task  " })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let task = body_json(response).await;
    assert_eq!(task["status"], json!("draft"));
    assert_eq!(task["priority"], json!("normal"));
    assert_eq!(task["revision"], json!(1));
    assert_eq!(task["title"], json!("new task"), "title must be trimmed");
    assert!(task["milestone_id"].is_null());
    assert!(task["assignee_id"].is_null());

    // 审计行 task.create 已落。
    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE action = 'task.create' AND project_id = $1",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_task_falls_back_to_global_human_when_project_has_no_members(pool: PgPool) {
    // 兼容无 project_member 行的种子/导入数据：actor 回退到全局第一个 human。
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "TSK-0".into(),
            project_name: "t".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let bare_project = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.project (id, customer_id, code, name, tenant_id) \
         VALUES ($1, $2, 'BARE-1', 'no members', $3)",
    )
    .bind(bare_project)
    .bind(boot.customer_id)
    .bind(SYSTEM_TENANT)
    .execute(&pool)
    .await
    .unwrap();
    let app = router(pool.clone());

    let response = app
        .oneshot(request(
            "POST",
            "/v1/pm/tasks".into(),
            Some(json!({ "project_id": bare_project, "title": "orphan project task" })),
        ))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "memberless project must still accept creates"
    );
    let task = body_json(response).await;
    assert_eq!(task["status"], json!("draft"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_task_validation_errors(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "TSK-2".into(),
            project_name: "t".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let app = router(pool.clone());

    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/v1/pm/tasks".into(),
            Some(json!({ "project_id": boot.project_id, "title": "   " })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["message"], json!("title is required"));

    let response = app
        .oneshot(request(
            "POST",
            "/v1/pm/tasks".into(),
            Some(json!({ "project_id": Uuid::new_v4(), "title": "ghost" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_json(response).await;
    assert_eq!(body["message"], json!("unknown project"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_task_full_fields_with_optimistic_lock(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "TSK-3".into(),
            project_name: "t".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let app = router(pool.clone());

    let created = app
        .clone()
        .oneshot(request(
            "POST",
            "/v1/pm/tasks".into(),
            Some(json!({ "project_id": boot.project_id, "title": "draft task" })),
        ))
        .await
        .unwrap();
    let task = body_json(created).await;
    let task_id = task["id"].as_str().unwrap().to_string();

    let response = app
        .clone()
        .oneshot(request(
            "PATCH",
            format!("/v1/pm/tasks/{task_id}"),
            Some(json!({
                "revision": task["revision"],
                "title": "renamed",
                "description": "updated",
                "status": "ready",
                "priority": "high",
                "assignee_id": boot.dev_agent_id
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let updated = body_json(response).await;
    assert_eq!(updated["title"], json!("renamed"));
    assert_eq!(updated["status"], json!("ready"));
    assert_eq!(updated["priority"], json!("high"));
    assert_eq!(updated["assignee_id"], json!(boot.dev_agent_id.to_string()));
    assert_eq!(updated["revision"], json!(2), "revision must bump");
    assert_ne!(
        updated["updated_at"], task["updated_at"],
        "updated_at must move"
    );

    // 错 revision → 409。
    let response = app
        .clone()
        .oneshot(request(
            "PATCH",
            format!("/v1/pm/tasks/{task_id}"),
            Some(json!({ "revision": 1, "title": "stale", "status": "completed", "priority": "low" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = body_json(response).await;
    assert_eq!(body["message"], json!("revision conflict"));

    // 非法 status / priority / title → 400。
    for bad in [
        json!({ "revision": 2, "title": "x", "status": "doing", "priority": "low" }),
        json!({ "revision": 2, "title": "x", "status": "completed", "priority": "urgentest" }),
        json!({ "revision": 2, "title": " ", "status": "completed", "priority": "low" }),
    ] {
        let response = app
            .clone()
            .oneshot(request(
                "PATCH",
                format!("/v1/pm/tasks/{task_id}"),
                Some(bad),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    let response = app
        .oneshot(request(
            "PATCH",
            format!("/v1/pm/tasks/{}", Uuid::new_v4()),
            Some(json!({
                "revision": 1, "title": "ghost", "status": "completed", "priority": "low"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------- D-2=A：ai_role_version_id 锚

/// bootstrap 项目里某 role 的当前版本 id（种子 v1.0）。
async fn ai_role_version_of(pool: &PgPool, project_id: Uuid, role_key: &str) -> Uuid {
    sqlx::query_scalar(
        "SELECT v.id \
           FROM ai.ai_role_version v \
           JOIN ai.project_ai_role pr ON pr.id = v.project_role_id \
           JOIN ai.ai_role_template t ON t.id = pr.role_id \
          WHERE pr.project_id = $1 AND t.role_key = $2 AND v.version = '1.0'",
    )
    .bind(project_id)
    .bind(role_key)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// POST /v1/pm/tasks 带可选 `ai_role_version_id` → 201 且锚落库
/// （D-1 遗留①：Task 创建时的 Role 版本引用，稿 §17.2）。
#[sqlx::test(migrations = "../../migrations")]
async fn create_task_with_ai_role_version_persists_anchor(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "TSK-R1".into(),
            project_name: "t".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let version = ai_role_version_of(&pool, boot.project_id, "developer").await;
    let app = router(pool.clone());

    let response = app
        .oneshot(request(
            "POST",
            "/v1/pm/tasks".into(),
            Some(json!({
                "project_id": boot.project_id,
                "title": "role-anchored",
                "ai_role_version_id": version,
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let task = body_json(response).await;
    let stored: Option<Uuid> = sqlx::query_scalar(
        "SELECT ai_role_version_id FROM delivery.task WHERE id = $1",
    )
    .bind(task["id"].as_str().unwrap().parse::<Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, Some(version), "the role anchor must be persisted");

    // 不带锚 = 历史 Task 兼容形态，仍 201 且列 NULL。
    let response = router(pool.clone())
        .oneshot(request(
            "POST",
            "/v1/pm/tasks".into(),
            Some(json!({ "project_id": boot.project_id, "title": "unanchored" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let task = body_json(response).await;
    let stored: Option<Uuid> = sqlx::query_scalar(
        "SELECT ai_role_version_id FROM delivery.task WHERE id = $1",
    )
    .bind(task["id"].as_str().unwrap().parse::<Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, None);
}

/// 跨项目的 Role 版本锚 → 400（版本必须属于目标 project）。
#[sqlx::test(migrations = "../../migrations")]
async fn create_task_rejects_cross_project_ai_role_version(pool: PgPool) {
    let boot_a = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "TSK-R2A".into(),
            project_name: "t".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let boot_b = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "TSK-R2B".into(),
            project_name: "t".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let version_a = ai_role_version_of(&pool, boot_a.project_id, "developer").await;
    let app = router(pool.clone());

    let response = app
        .oneshot(request(
            "POST",
            "/v1/pm/tasks".into(),
            Some(json!({
                "project_id": boot_b.project_id,
                "title": "stolen anchor",
                "ai_role_version_id": version_a,
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    // 未知版本 id 同样 400（不泄露存在性）。
    let response = router(pool.clone())
        .oneshot(request(
            "POST",
            "/v1/pm/tasks".into(),
            Some(json!({
                "project_id": boot_b.project_id,
                "title": "ghost anchor",
                "ai_role_version_id": Uuid::new_v4(),
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// D-2 遗留补读面（Batch E）：带锚创建的任务在 GET /v1/pm/tasks 列表行里
/// 携带 `ai_role_version_id`；无锚任务该列为 null。
#[sqlx::test(migrations = "../../migrations")]
async fn task_list_rows_carry_ai_role_version_id(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "TSK-R3".into(),
            project_name: "t".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let version = ai_role_version_of(&pool, boot.project_id, "developer").await;
    // off 模式注入 dev-off principal；/v1/pm 读端点按 project_member 过滤，
    // 先把它加为项目成员（同 tests/common::join_dev；本文件未引 common 故内联）。
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'dev-off', true) ON CONFLICT (id) DO NOTHING",
    )
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'owner')",
    )
    .bind(boot.project_id)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap();
    let app = router(pool.clone());
    let response = app
        .oneshot(request(
            "POST",
            "/v1/pm/tasks".into(),
            Some(json!({
                "project_id": boot.project_id,
                "title": "anchored-list",
                "ai_role_version_id": version,
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let response = router(pool.clone())
        .oneshot(request(
            "POST",
            "/v1/pm/tasks".into(),
            Some(json!({ "project_id": boot.project_id, "title": "plain-list" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = router(pool.clone())
        .oneshot(request(
            "GET",
            format!("/v1/pm/tasks?project_id={}", boot.project_id),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "both created tasks must appear in the list");
    let anchored = items
        .iter()
        .find(|item| item["title"] == "anchored-list")
        .expect("anchored task row");
    assert_eq!(
        anchored["ai_role_version_id"]
            .as_str()
            .unwrap()
            .parse::<Uuid>()
            .unwrap(),
        version,
        "the list row must carry the anchored ai_role_version_id"
    );
    let plain = items
        .iter()
        .find(|item| item["title"] == "plain-list")
        .expect("unanchored task row");
    assert!(
        plain["ai_role_version_id"].is_null(),
        "an unanchored task row must expose ai_role_version_id as null"
    );
}
