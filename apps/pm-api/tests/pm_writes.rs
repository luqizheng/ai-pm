//! `/v1/pm/*` 写端点契约测试（Slice A）：decide / revisions create+approve /
//! tasks create+update。契约源：`frontend/src/api/pm.ts` 注释与
//! `frontend/src/api/mock/fixtures.ts`（校验顺序 404 → 409 → 400 decision →
//! 400 reason；错误体 `{message}`）。

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use serde_json::{Value, json};
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
        "INSERT INTO delivery.task (id, project_id, title, status, revision) \
         VALUES ($1, $2, 'DECIDE-TASK', 'in_progress', 2)",
    )
    .bind(task_id)
    .bind(project_id)
    .execute(pool)
    .await
    .unwrap();

    let event_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO integration.inbound_event \
         (id, project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash) \
         VALUES ($1, $2, $3, 'test', 'bug_report', $4, '{}', 'hash')",
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
        "INSERT INTO ai.analysis_run (id, project_id, event_id, model_name, policy_version, config_snapshot) \
         VALUES ($1, $2, $3, 'fake-model', 1, '{}')",
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
        "data": { "task_id": task_id, "expected_revision": 2, "new_status": "done" }
    });
    sqlx::query(
        "INSERT INTO ai.change_proposal \
         (id, project_id, analysis_run_id, proposal_seq, target_type, target_id, command_type, \
          proposed_data, rationale, risk_level, status) \
         VALUES ($1, $2, $3, 0, 'task', $4, 'update_task_status', $5, 'because', 'low', 'proposed')",
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
         (id, project_id, proposal_id, status, required_role) \
         VALUES ($1, $2, $3, 'pending', 'project_owner')",
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
    let boot = bootstrap_project(&pool, BootstrapRequest { project_code: "DEC-1".into(), project_name: "d".into() }).await.unwrap();
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
    let boot = bootstrap_project(&pool, BootstrapRequest { project_code: "DEC-2".into(), project_name: "d".into() }).await.unwrap();
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
    assert!(body["decided_at"].is_string(), "decided_at must be an ISO timestamp");

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
    let boot = bootstrap_project(&pool, BootstrapRequest { project_code: "DEC-3".into(), project_name: "d".into() }).await.unwrap();
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
    let boot = bootstrap_project(&pool, BootstrapRequest { project_code: "DEC-4".into(), project_name: "d".into() }).await.unwrap();
    let approval_id = seed_pending_approval(&pool, boot.project_id, boot.human_id).await;
    let app = router(pool.clone());
    let body = json!({ "decision": "rejected", "reason": "first" });

    let first = app
        .clone()
        .oneshot(request("POST", format!("/v1/pm/approvals/{approval_id}/decide"), Some(body.clone())))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .oneshot(request("POST", format!("/v1/pm/approvals/{approval_id}/decide"), Some(body)))
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CONFLICT);
    let body = body_json(second).await;
    assert_eq!(body["message"], json!("already decided"));
}

// -------------------------------------------------------------- tasks CRUD

#[sqlx::test(migrations = "../../migrations")]
async fn create_task_defaults_and_contract_shape(pool: PgPool) {
    let boot = bootstrap_project(&pool, BootstrapRequest { project_code: "TSK-1".into(), project_name: "t".into() }).await.unwrap();
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
    assert_eq!(task["status"], json!("backlog"));
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
async fn create_task_validation_errors(pool: PgPool) {
    let boot = bootstrap_project(&pool, BootstrapRequest { project_code: "TSK-2".into(), project_name: "t".into() }).await.unwrap();
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
    let boot = bootstrap_project(&pool, BootstrapRequest { project_code: "TSK-3".into(), project_name: "t".into() }).await.unwrap();
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
                "status": "in_progress",
                "priority": "high",
                "assignee_id": boot.dev_agent_id
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let updated = body_json(response).await;
    assert_eq!(updated["title"], json!("renamed"));
    assert_eq!(updated["status"], json!("in_progress"));
    assert_eq!(updated["priority"], json!("high"));
    assert_eq!(updated["assignee_id"], json!(boot.dev_agent_id.to_string()));
    assert_eq!(updated["revision"], json!(2), "revision must bump");
    assert_ne!(updated["updated_at"], task["updated_at"], "updated_at must move");

    // 错 revision → 409。
    let response = app
        .clone()
        .oneshot(request(
            "PATCH",
            format!("/v1/pm/tasks/{task_id}"),
            Some(json!({ "revision": 1, "title": "stale", "status": "done", "priority": "low" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = body_json(response).await;
    assert_eq!(body["message"], json!("revision conflict"));

    // 非法 status / priority / title → 400。
    for bad in [
        json!({ "revision": 2, "title": "x", "status": "doing", "priority": "low" }),
        json!({ "revision": 2, "title": "x", "status": "done", "priority": "urgentest" }),
        json!({ "revision": 2, "title": " ", "status": "done", "priority": "low" }),
    ] {
        let response = app
            .clone()
            .oneshot(request("PATCH", format!("/v1/pm/tasks/{task_id}"), Some(bad)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    let response = app
        .oneshot(request("PATCH", format!("/v1/pm/tasks/{}", Uuid::new_v4()), Some(json!({
            "revision": 1, "title": "ghost", "status": "done", "priority": "low"
        }))))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ----------------------------------------------------- requirement revisions

async fn seed_requirement_with_revisions(
    pool: &PgPool,
    project_id: Uuid,
    created_by: Uuid,
    approved_versions: &[i64],
    draft_version: i64,
) -> (Uuid, Uuid) {
    let requirement_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.requirement (id, project_id, code, title, status) \
         VALUES ($1, $2, 'REQ-9', 'seeded', 'draft')",
    )
    .bind(requirement_id)
    .bind(project_id)
    .execute(pool)
    .await
    .unwrap();
    let mut latest_draft = Uuid::new_v4();
    for version in 1..=3 {
        let state = if approved_versions.contains(&version) {
            "approved"
        } else if version == draft_version {
            "draft"
        } else {
            "superseded"
        };
        let revision_id = Uuid::new_v4();
        if state == "draft" {
            latest_draft = revision_id;
        }
        sqlx::query(
            "INSERT INTO delivery.requirement_revision \
             (id, requirement_id, project_id, version, content, state, content_hash, created_by, approved_by) \
             VALUES ($1, $2, $3, $4, '{}', $5, 'hash', $6, $7)",
        )
        .bind(revision_id)
        .bind(requirement_id)
        .bind(project_id)
        .bind(version)
        .bind(state)
        .bind(created_by)
        .bind(if state == "approved" { Some(created_by) } else { None })
        .execute(pool)
        .await
        .unwrap();
    }
    (requirement_id, latest_draft)
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_revision_appends_draft_with_hash(pool: PgPool) {
    let boot = bootstrap_project(&pool, BootstrapRequest { project_code: "REV-2".into(), project_name: "r".into() }).await.unwrap();
    let (requirement_id, _) = seed_requirement_with_revisions(&pool, boot.project_id, boot.human_id, &[1], 0).await;
    let app = router(pool.clone());

    let response = app
        .oneshot(request(
            "POST",
            format!("/v1/pm/requirements/{requirement_id}/revisions"),
            Some(json!({ "content": { "markdown": "# 新方案" } })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let revision = body_json(response).await;
    assert_eq!(revision["version"], json!(4), "version must be max+1");
    assert_eq!(revision["state"], json!("draft"));
    assert_eq!(revision["created_by"], json!(boot.human_id.to_string()));
    assert!(revision["approved_by"].is_null());
    let hash = revision["content_hash"].as_str().unwrap();
    assert_eq!(hash.len(), 64, "content_hash must be sha256 hex");
    assert_eq!(revision["content"]["markdown"], json!("# 新方案"));

    // 审计行 requirement.propose_revision 已落。
    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE action = 'requirement.propose_revision'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_revision_validation(pool: PgPool) {
    let boot = bootstrap_project(&pool, BootstrapRequest { project_code: "REV-3".into(), project_name: "r".into() }).await.unwrap();
    let (requirement_id, _) = seed_requirement_with_revisions(&pool, boot.project_id, boot.human_id, &[], 0).await;
    let app = router(pool.clone());

    let response = app
        .clone()
        .oneshot(request(
            "POST",
            format!("/v1/pm/requirements/{requirement_id}/revisions"),
            Some(json!({ "content": { "markdown": "  " } })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["message"], json!("content.markdown is required"));

    let response = app
        .oneshot(request(
            "POST",
            format!("/v1/pm/requirements/{}/revisions", Uuid::new_v4()),
            Some(json!({ "content": { "markdown": "x" } })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_json(response).await;
    assert_eq!(body["message"], json!("not found"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn approve_revision_promotes_and_supersedes(pool: PgPool) {
    let boot = bootstrap_project(&pool, BootstrapRequest { project_code: "REV-4".into(), project_name: "r".into() }).await.unwrap();
    // 版本 1 已 approved、版本 3 是 draft：定稿 3 后 1 必须转 superseded。
    let (requirement_id, draft_id) =
        seed_requirement_with_revisions(&pool, boot.project_id, boot.human_id, &[1], 3).await;
    let app = router(pool.clone());

    let response = app
        .oneshot(request(
            "POST",
            format!("/v1/pm/requirements/{requirement_id}/revisions/{draft_id}/approve"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let revision = body_json(response).await;
    assert_eq!(revision["state"], json!("approved"));
    assert_eq!(revision["approved_by"], json!(boot.human_id.to_string()));

    let (req_status, current, row_revision): (String, Option<Uuid>, i64) = sqlx::query_as(
        "SELECT status, current_revision_id, row_revision FROM delivery.requirement WHERE id = $1",
    )
    .bind(requirement_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(req_status, "approved");
    assert_eq!(current, Some(draft_id));
    assert_eq!(row_revision, 2, "row_revision must bump on approve");

    let superseded: Vec<String> = sqlx::query_scalar(
        "SELECT id::text FROM delivery.requirement_revision \
         WHERE requirement_id = $1 AND state = 'superseded' ORDER BY version",
    )
    .bind(requirement_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(superseded.len(), 2, "old approved and superseded rows must be superseded");
}

#[sqlx::test(migrations = "../../migrations")]
async fn approve_revision_conflicts_for_non_draft_or_unknown(pool: PgPool) {
    let boot = bootstrap_project(&pool, BootstrapRequest { project_code: "REV-5".into(), project_name: "r".into() }).await.unwrap();
    let (requirement_id, _) =
        seed_requirement_with_revisions(&pool, boot.project_id, boot.human_id, &[1], 0).await;
    let app = router(pool.clone());

    // 已 approved 的版本 1：非 draft → 409。
    let approved_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM delivery.requirement_revision WHERE requirement_id = $1 AND state = 'approved'",
    )
    .bind(requirement_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            format!("/v1/pm/requirements/{requirement_id}/revisions/{approved_id}/approve"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = body_json(response).await;
    assert_eq!(body["message"], json!("not a draft revision"));

    // 未知 revision → 同样 409（契约：未找到也走 409）。
    let response = app
        .oneshot(request(
            "POST",
            format!("/v1/pm/requirements/{requirement_id}/revisions/{}/approve", Uuid::new_v4()),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = body_json(response).await;
    assert_eq!(body["message"], json!("not a draft revision"));
}
