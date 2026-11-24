//! T-084 项目认知访谈（Discovery Web 入口）HTTP 验收：
//! session 创建（source=WEB + import unknowns → questions）、详情读取、
//! 状态机推进、发现卡片确认（answer + verification + user_confirmed）、
//! question 回答/暂缓、T-022 可见性与跨租户隔离。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn seed_project(pool: &PgPool) -> Uuid {
    let app = router(pool.clone());
    let (status, body) = body_json(
        app.oneshot(json_request(
            "POST",
            "/v1/pm/projects",
            json!({ "code": format!("DISC-{}", Uuid::new_v4().simple()), "name": "发现" }),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

/// 种子一个 draft_ready 导入，草案带 2 条 unknowns（Fake 模型产物形态）。
///
/// 草案落 `integration.repo_import_draft_version`（I6-B D-1a：版本化，不再是
/// `repo_import` 上的单列），`repo_import.draft_revision` 指向该版本。
async fn seed_import(pool: &PgPool, project_id: Uuid) -> Uuid {
    let import_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.repo_import \
         (tenant_id, project_id, requested_by, status, draft_revision) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'draft_ready', 1) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO integration.repo_import_draft_version \
         (tenant_id, import_id, revision, draft, generation_status, generated_at) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $2, 1, $3, 'succeeded', now())",
    )
    .bind(project_id)
    .bind(import_id)
    .bind(json!({
        "schema_version": 1,
        "unknowns": [
            { "question": "支付模块由谁实现？", "why_blocking": "阻塞架构认知" },
            { "question": "部署形态是单机还是集群？", "why_blocking": "阻塞运维规划" }
        ]
    }))
    .execute(pool)
    .await
    .unwrap();
    import_id
}

async fn start_session(
    app: &axum::Router,
    project_id: Uuid,
    import_id: Option<Uuid>,
) -> (StatusCode, Value) {
    let payload = match import_id {
        Some(id) => json!({ "import_id": id }),
        None => json!({}),
    };
    body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/pm/projects/{project_id}/discovery/sessions"),
                payload,
            ))
            .await
            .unwrap(),
    )
    .await
}

#[sqlx::test(migrations = "../../migrations")]
async fn web_session_is_created_with_unknowns_as_questions(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let import_id = seed_import(&pool, project_id).await;
    let app = router(pool.clone());

    let (status, body) = start_session(&app, project_id, Some(import_id)).await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let session = &body["session"];
    assert_eq!(session["source"], "WEB");
    assert_eq!(session["kind"], "INITIAL_ONBOARDING");
    assert_eq!(session["status"], "IN_PROGRESS");
    assert_eq!(body["questions_added"], 2);
    assert_eq!(body["linked_import_id"], json!(import_id));

    let session_id = Uuid::parse_str(session["id"].as_str().unwrap()).unwrap();
    let linked: Option<Uuid> = sqlx::query_scalar(
        "SELECT discovery_session_id FROM integration.repo_import WHERE id = $1",
    )
    .bind(import_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        linked,
        Some(session_id),
        "repo_import must link forward (Q1)"
    );

    let (status, detail) = body_json(
        app.oneshot(get(&format!(
            "/v1/pm/projects/{project_id}/discovery/sessions/{session_id}"
        )))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["questions"].as_array().unwrap().len(), 2);
    assert_eq!(detail["questions"][0]["status"], "open");
    assert_eq!(detail["findings"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn session_status_machine_is_enforced_over_http(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let app = router(pool.clone());
    let (status, body) = start_session(&app, project_id, None).await;
    assert_eq!(status, StatusCode::CREATED);
    let session_id = body["session"]["id"].as_str().unwrap().to_string();
    let base = format!("/v1/pm/projects/{project_id}/discovery/sessions/{session_id}");

    let (status, moved) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("{base}/status"),
                json!({ "status": "PAUSED" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(moved["status"], "PAUSED");

    let (status, err) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("{base}/status"),
                json!({ "status": "COMPLETED" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "PAUSED → COMPLETED must be allowed: {err}"
    );
    assert_eq!(err["status"], "COMPLETED");

    let (status, err) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("{base}/status"),
                json!({ "status": "IN_PROGRESS" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "invalid_state_transition");
}

#[sqlx::test(migrations = "../../migrations")]
async fn discovery_card_verification_writes_answer_and_confirmed_finding(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let app = router(pool.clone());
    let (_, body) = start_session(&app, project_id, None).await;
    let session_id = body["session"]["id"].as_str().unwrap().to_string();
    let base = format!("/v1/pm/projects/{project_id}/discovery/sessions/{session_id}");

    let (status, finding) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("{base}/findings"),
                json!({
                    "kind": "speculation",
                    "content": "支付模块可能尚未完成",
                    "confidence": 0.78,
                    "evidence": [
                        { "repository": "backend", "file": "src/payment/mod.rs", "description": "存在未实现的支付接口" }
                    ]
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {finding}");
    let finding_id = finding["id"].as_str().unwrap().to_string();

    let (status, outcome) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("{base}/findings/{finding_id}/verification"),
                json!({
                    "status": "CONFIRMED",
                    "answer": "确实尚未完成",
                    "confirmed_finding": "支付模块确未完成，计划外部实现"
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {outcome}");
    assert_eq!(outcome["finding"]["verification_status"], "CONFIRMED");
    assert_eq!(outcome["question"]["finding_id"], json!(finding_id));
    assert_eq!(outcome["answer"]["answer"], "确实尚未完成");
    assert_eq!(
        outcome["confirmed_finding"]["kind"], "user_confirmed",
        "选项对应外部实现时必须产生 user_confirmed finding"
    );

    let (_, detail) = body_json(app.clone().oneshot(get(&base)).await.unwrap()).await;
    assert_eq!(detail["findings"].as_array().unwrap().len(), 2);
    assert_eq!(detail["questions"].as_array().unwrap().len(), 1);
    assert_eq!(
        detail["questions"][0]["answers"].as_array().unwrap().len(),
        1
    );

    let (status, err) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("{base}/findings"),
                json!({ "kind": "rumor", "content": "x" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err["error"], "validation_error");
}

#[sqlx::test(migrations = "../../migrations")]
async fn questions_can_be_answered_and_deferred(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let app = router(pool.clone());
    let (status, body) = start_session(&app, project_id, None).await;
    assert_eq!(status, StatusCode::CREATED);
    let session_id = body["session"]["id"].as_str().unwrap().to_string();
    let base = format!("/v1/pm/projects/{project_id}/discovery/sessions/{session_id}");

    let (status, question) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("{base}/questions"),
                json!({ "question": "目标用户是谁？", "why_blocking": "阻塞需求梳理" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {question}");
    let question_id = question["id"].as_str().unwrap().to_string();

    let (status, answer) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("{base}/questions/{question_id}/answers"),
                json!({ "answer": "内部运营团队" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {answer}");
    assert_eq!(answer["answer"], "内部运营团队");

    let (_, detail) = body_json(app.clone().oneshot(get(&base)).await.unwrap()).await;
    assert_eq!(detail["questions"][0]["status"], "answered");

    let (status, _err) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("{base}/questions/{question_id}/answers"),
                json!({ "answer": "再答一次" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "answered 不可再答"
    );

    let (status, question2) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("{base}/questions"),
                json!({ "question": "部署形态？" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let question2_id = question2["id"].as_str().unwrap().to_string();
    let (status, deferred) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("{base}/questions/{question2_id}/defer"),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deferred["status"], "deferred");
}

#[sqlx::test(migrations = "../../migrations")]
async fn non_member_and_cross_tenant_projects_return_404(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let app = router(pool.clone());
    let (_, body) = start_session(&app, project_id, None).await;
    let session_id = body["session"]["id"].as_str().unwrap().to_string();

    let other_tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'disc-web-other', 'other') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let other_customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, 'c') RETURNING id",
    )
    .bind(other_tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    let other_principal: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'other') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let other_project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name) \
         VALUES ($1, $2, 'DISC-WEB-OTHER', 'other') RETURNING id",
    )
    .bind(other_tenant)
    .bind(other_customer)
    .fetch_one(&pool)
    .await
    .unwrap();

    let (status, err) = body_json(
        app.clone()
            .oneshot(get(&format!(
                "/v1/pm/projects/{other_project}/discovery/sessions/{session_id}"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "跨租户项目必须按不存在处理: {err}"
    );

    let member_only: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name) \
         VALUES ((SELECT tenant_id FROM core.customer WHERE id = $1 LIMIT 1), $1, 'MEMBER-ONLY', 'x') \
         RETURNING id",
    )
    .bind(other_customer)
    .fetch_one(&pool)
    .await
    .unwrap();
    let _ = other_principal;
    let (status, _) = body_json(
        app.clone()
            .oneshot(get(&format!(
                "/v1/pm/projects/{member_only}/discovery/sessions"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "非成员项目的读端点必须 404");
}

#[sqlx::test(migrations = "../../migrations")]
async fn session_listing_returns_created_sessions(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let app = router(pool.clone());
    for _ in 0..2 {
        let (status, _) = start_session(&app, project_id, None).await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let (status, list) = body_json(
        app.oneshot(get(&format!(
            "/v1/pm/projects/{project_id}/discovery/sessions"
        )))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["total"], 2);
    assert_eq!(list["items"].as_array().unwrap().len(), 2);
}

/// B03-01（R1）：人工处理动作（完结/退回复核）留审计痕——跃迁与审计同事务，
/// `after_data` 记录 from→to；会话详情如实暴露失败标记列（初始为 null）。
#[sqlx::test(migrations = "../../migrations")]
async fn human_transition_writes_audit_and_exposes_round_error_fields(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let app = router(pool.clone());
    let (status, body) = start_session(&app, project_id, None).await;
    assert_eq!(status, StatusCode::CREATED);
    let session_id = body["session"]["id"].as_str().unwrap().to_string();
    // 新列出现在会话视图（失败可见性契约）。
    assert!(body["session"]["last_round_error"].is_null());
    assert!(body["session"]["last_round_error_at"].is_null());
    let base = format!("/v1/pm/projects/{project_id}/discovery/sessions/{session_id}");

    let (status, moved) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("{base}/status"),
                json!({ "status": "NEEDS_USER_INPUT" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(moved["status"], "NEEDS_USER_INPUT");

    // 复核通过并完结（B03-01 的人工处理动作）→ 审计行记录 from→to。
    let (status, moved) = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("{base}/status"),
                json!({ "status": "COMPLETED" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{moved}");

    let (action, after): (String, Value) = sqlx::query_as(
        "SELECT action, after_data FROM governance.audit_log \
         WHERE target_type = 'discovery_session' AND target_id = $1 \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(Uuid::parse_str(&session_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(action, "discovery_session_transitioned");
    assert_eq!(after["from"], "NEEDS_USER_INPUT");
    assert_eq!(after["to"], "COMPLETED");
}
