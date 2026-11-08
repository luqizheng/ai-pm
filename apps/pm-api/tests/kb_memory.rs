//! KP-3 `/v1/kb/*` REST 契约测试：K4/K5 状态码矩阵、来源登记/管理面、真人审核闭环
//! （讨论沉淀 → 提案 → K4 批准 → 注入面数据就绪）与越权负向。
//!
//! off 模式（`router()`）调用方 = `dev-off`（真人）：提案与 K4 审核同主体可走通
//! （真人校验只拒绝 AI 主体，见 pm-application 侧 kb_service 测试）。

mod common;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use pm_domain::SYSTEM_TENANT;

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .expect("bootstrap project");
    outcome.project_id
}

fn post_json(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(uri: &str) -> Request<Body> {
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

async fn register_source(pool: &PgPool, code: &str) -> (Uuid, Value) {
    let project = seed_project(pool, code).await;
    common::join_dev(pool, project).await;
    let app = router(pool.clone());
    let response = app
        .oneshot(post_json(
            &format!("/v1/kb/sources?project_id={project}"),
            json!({
                "source_kind": "meeting_notes",
                "title": "架构评审会纪要",
                "inline_content": "否决方案 X：延迟不可控。",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED, "first registration");
    (project, body_json(response).await)
}

/// 提案一条记忆（evidence=来源 + subject=task），返回 (memory_id, revision_id)。
async fn propose(pool: &PgPool, project: Uuid, source_id: Uuid) -> (Uuid, Uuid) {
    let app = router(pool.clone());
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, task_type, tenant_id) \
         VALUES ($1, 'terrain', 'ready', 1, 'architecture', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project)
    .fetch_one(pool)
    .await
    .unwrap();
    let response = app
        .oneshot(post_json(
            &format!("/v1/kb/memories?project_id={project}"),
            json!({
                "memory_type": "rejected_option",
                "title": "方案 X 被否决",
                "summary": "因延迟不可控",
                "change_reason": "沉淀评审会结论",
                "refs": [
                    {"ref_role": "evidence", "target_domain": "knowledge_base",
                     "target_type": "source", "target_id": source_id.to_string()},
                    {"ref_role": "subject", "target_domain": "requirement",
                     "target_type": "task", "target_id": task_id.to_string()},
                ],
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED, "proposal");
    let body = body_json(response).await;
    (
        body["memory_id"].as_str().unwrap().parse().unwrap(),
        body["revision_id"].as_str().unwrap().parse().unwrap(),
    )
}

#[sqlx::test(migrations = "../../migrations")]
async fn source_registration_and_k5_listing(pool: PgPool) {
    let (project, first) = register_source(&pool, "KB-REST-SRC").await;
    let source_id: Uuid = first["source_id"].as_str().unwrap().parse().unwrap();
    assert_eq!(first["deduplicated"], false);
    assert_eq!(first["newly_linked"], true);
    assert_eq!(
        first["content_hash"],
        pm_application::knowledge::content_hash("否决方案 X：延迟不可控。")
    );

    // 同项目重放：200（幂等零副作用）；另一项目：201（新关联）。
    let app = router(pool.clone());
    let replay = app
        .clone()
        .oneshot(post_json(
            &format!("/v1/kb/sources?project_id={project}"),
            json!({
                "source_kind": "meeting_notes",
                "title": "架构评审会纪要",
                "inline_content": "否决方案 X：延迟不可控。",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(body_json(replay).await["source_id"], first["source_id"]);

    let other = seed_project(&pool, "KB-REST-SRC-B").await;
    common::join_dev(&pool, other).await;
    let shared = app
        .clone()
        .oneshot(post_json(
            &format!("/v1/kb/sources?project_id={other}"),
            json!({
                "source_kind": "meeting_notes",
                "title": "架构评审会纪要（别项目标题不改团队池）",
                "inline_content": "否决方案 X：延迟不可控。",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(shared.status(), StatusCode::CREATED);
    assert_eq!(body_json(shared).await["source_id"], first["source_id"]);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM kb.source")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1, "team pool keeps one identity per content");

    // K5：项目 A 一条；项目 C（未关联）零条（不泄露团队池）。
    let app = router(pool.clone());
    let response = app
        .clone()
        .oneshot(get(&format!("/v1/kb/sources?project_id={project}")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let items = body_json(response).await["items"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["source_id"], Value::from(source_id.to_string()));
    assert_eq!(items[0]["content_hash"], first["content_hash"]);

    let third = seed_project(&pool, "KB-REST-SRC-C").await;
    common::join_dev(&pool, third).await;
    let response = app
        .clone()
        .oneshot(get(&format!("/v1/kb/sources?project_id={third}")))
        .await
        .unwrap();
    assert_eq!(
        body_json(response).await["items"].as_array().unwrap().len(),
        0
    );

    // 详情含内联正文；非 UUID 路径 id 与未知 id 同口径 404。
    let response = app
        .clone()
        .oneshot(get(&format!(
            "/v1/kb/sources/{source_id}?project_id={project}"
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let detail = body_json(response).await;
    assert_eq!(detail["inline_content"], "否决方案 X：延迟不可控。");
    let response = app
        .clone()
        .oneshot(get(&format!(
            "/v1/kb/sources/not-a-uuid?project_id={project}"
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // 400 矩阵：双 locator / 无 locator / 内联 hash 不一致 / 未知类别。
    for (body, why) in [
        (
            json!({"source_kind": "pdf", "title": "t", "uri": "s3://a", "inline_content": "x",
                   "content_hash": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}),
            "both locators",
        ),
        (json!({"source_kind": "pdf", "title": "t"}), "no locator"),
        (
            json!({"source_kind": "markdown", "title": "t", "inline_content": "x",
                   "content_hash": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}),
            "hash mismatch",
        ),
        (
            json!({"source_kind": "tweet", "title": "t", "inline_content": "x"}),
            "unknown kind",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(post_json(
                &format!("/v1/kb/sources?project_id={project}"),
                body,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{why}");
    }
    // 无凭据由全局中间件拒（off 模式不适用）；缺 project_id → 400。
    let response = app.clone().oneshot(get("/v1/kb/sources")).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn k4_review_lifecycle_and_contract_shapes(pool: PgPool) {
    let (project, first) = register_source(&pool, "KB-REST-K4").await;
    let source_id: Uuid = first["source_id"].as_str().unwrap().parse().unwrap();
    let (memory_id, revision_id) = propose(&pool, project, source_id).await;
    let app = router(pool.clone());
    let review_uri = format!("/v1/kb/memories/{memory_id}/review?project_id={project}");

    // 400 矩阵：未知动作 / base 非 UUID / body 非 JSON 契约。
    for (body, why) in [
        (
            json!({"action": "merge", "base_memory_revision_id": revision_id.to_string()}),
            "unknown action",
        ),
        (
            json!({"action": "approve", "base_memory_revision_id": "nope"}),
            "non-uuid base",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(post_json(&review_uri, body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{why}");
    }

    // 过期 CAS（base 不是待审提案）→ 409。
    let response = app
        .clone()
        .oneshot(post_json(
            &review_uri,
            json!({"action": "approve", "note": "ok",
                   "base_memory_revision_id": Uuid::new_v4().to_string()}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);

    // 批准：200 + §12.2 形状 { memory_id, review_status } + revision_ref 四元组。
    let response = app
        .clone()
        .oneshot(post_json(
            &review_uri,
            json!({"action": "approve", "note": "结论与纪要一致",
                   "base_memory_revision_id": revision_id.to_string()}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["memory_id"], Value::from(memory_id.to_string()));
    assert_eq!(body["review_status"], "APPROVED");
    let revision_ref = &body["revision_ref"];
    assert_eq!(revision_ref["target_domain"], "knowledge_base");
    assert_eq!(revision_ref["target_type"], "memory_revision");
    assert_eq!(
        revision_ref["target_id"],
        Value::from(revision_id.to_string())
    );

    // 已裁决再审 → 409（无待审）。
    let response = app
        .clone()
        .oneshot(post_json(
            &review_uri,
            json!({"action": "approve", "base_memory_revision_id": revision_id.to_string()}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);

    // 批准后：K5 evidence_of（memory 维度）命中来源；详情状态 active。
    let evidence_uri = format!(
        "/v1/kb/sources?project_id={project}&evidence_of=memory:{}",
        memory_id
    );
    let response = app.clone().oneshot(get(&evidence_uri)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let items = body_json(response).await["items"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["source_id"], first["source_id"]);
    // 非法 evidence_of 形态 → 400。
    let response = app
        .clone()
        .oneshot(get(&format!(
            "/v1/kb/sources?project_id={project}&evidence_of=garbage"
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    // 事件与审计面：批准同事务一条 kb.memory.approved + KB 域审计四身份列齐。
    let events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.outbox_event \
          WHERE project_id = $1 AND event_type = 'kb.memory.approved'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(events, 1);
    let audit: (Uuid, Option<Uuid>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT actor_id, actor_identity_id, actor_client, actor_auth_mode \
           FROM governance.audit_log \
          WHERE source_kind = 'kb_service' AND action = 'kb.memory.change_approved'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit.0, pm_api::auth::DEV_OFF_PRINCIPAL);
    // off 模式调用方 identity/client 为 NULL、auth_mode='off'（T-088 口径；真人三列
    // 齐全形态已在 pm-application 侧 kb_service 测试断言）。
    assert!(audit.1.is_none() && audit.2.is_none());
    assert_eq!(audit.3.as_deref(), Some("off"));

    // 记忆列表 / 详情（管理面 D-KP3-5）。
    let response = app
        .clone()
        .oneshot(get(&format!(
            "/v1/kb/memories?project_id={project}&state=active"
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let items = body_json(response).await["items"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["state"], "active");
    assert_eq!(items[0]["headline"]["version_status"], "CURRENT");
    let response = app
        .clone()
        .oneshot(get(&format!(
            "/v1/kb/memories/{memory_id}?project_id={project}"
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let detail = body_json(response).await;
    assert_eq!(detail["state"], "active");
    assert_eq!(detail["revisions"].as_array().unwrap().len(), 1);
    assert_eq!(detail["revisions"][0]["refs"].as_array().unwrap().len(), 2);
    // 非 UUID / 跨项目记忆 → 404。
    let response = app
        .clone()
        .oneshot(get(&format!("/v1/kb/memories/zzz?project_id={project}")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = app
        .oneshot(get(&format!(
            "/v1/kb/memories/{memory_id}?project_id={}",
            Uuid::new_v4()
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn proposal_rest_negatives_and_withdraw_flow(pool: PgPool) {
    let (project, first) = register_source(&pool, "KB-REST-NEG").await;
    let source_id: Uuid = first["source_id"].as_str().unwrap().parse().unwrap();
    let app = router(pool.clone());
    let uri = format!("/v1/kb/memories?project_id={project}");

    // 400 矩阵：无 evidence / 未知类型 / 非本项目 task / 不存在的会话消息 / 未知 ref 形状。
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, task_type, tenant_id) \
         VALUES ($1, 't', 'ready', 1, 'architecture', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    let foreign_project = seed_project(&pool, "KB-REST-NEG-B").await;
    let foreign_task: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, task_type, tenant_id) \
         VALUES ($1, 't', 'ready', 1, 'architecture', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(foreign_project)
    .fetch_one(&pool)
    .await
    .unwrap();
    for (refs, why) in [
        (
            json!([{"ref_role": "subject", "target_domain": "requirement",
                    "target_type": "task", "target_id": task_id.to_string()}]),
            "no evidence",
        ),
        (
            json!([{"ref_role": "evidence", "target_domain": "knowledge_base",
                    "target_type": "document", "target_id": source_id.to_string()}]),
            "unsupported kb type",
        ),
        (
            json!([{"ref_role": "evidence", "target_domain": "knowledge_base",
                    "target_type": "source", "target_id": source_id.to_string()},
                   {"ref_role": "subject", "target_domain": "requirement",
                    "target_type": "task", "target_id": foreign_task.to_string()}]),
            "foreign task subject",
        ),
        (
            json!([{"ref_role": "evidence", "target_domain": "requirement",
                    "target_type": "conversation_message",
                    "target_id": Uuid::new_v4().to_string()}]),
            "message not found",
        ),
        (
            json!([{"ref_role": "evidence", "target_domain": "chat",
                    "target_type": "message", "target_id": Uuid::new_v4().to_string()}]),
            "unknown domain",
        ),
        (
            json!([{"ref_role": "evidence", "target_domain": "knowledge_base",
                    "target_type": "source", "target_id": source_id.to_string()},
                   {"ref_role": "evidence", "target_domain": "knowledge_base",
                    "target_type": "source", "target_id": source_id.to_string()}]),
            "duplicate ref",
        ),
    ] {
        let body = json!({
            "memory_type": "rejected_option",
            "title": "t", "summary": "s",
            "change_reason": "r", "refs": refs,
        });
        let response = app.clone().oneshot(post_json(&uri, body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{why}");
    }
    let kb_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM kb.memory")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kb_rows, 0);

    // 建一条（真人提案）→ 批准为 v1 → 修改提案 → 批准 v2 → 撤回提案 → 批准撤回。
    let (memory_id, create_revision) = propose(&pool, project, source_id).await;
    let approve = |app: &axum::Router, base: Uuid, note: &'static str| {
        let app = app.clone();
        let uri = format!("/v1/kb/memories/{memory_id}/review?project_id={project}");
        Box::pin(async move {
            let response = app
                .oneshot(post_json(
                    &uri,
                    json!({"action": "approve", "note": note,
                           "base_memory_revision_id": base.to_string()}),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        })
    };
    approve(&app, create_revision, "首版批准").await;
    let response = app
        .clone()
        .oneshot(post_json(
            &format!("/v1/kb/memories/{memory_id}/changes?project_id={project}"),
            json!({"action": "update", "memory_type": "rejected_option",
                   "title": "方案 X 被否决", "summary": "因延迟不可控且成本高",
                   "change_reason": "补充理由",
                   "refs": [{"ref_role": "evidence", "target_domain": "knowledge_base",
                             "target_type": "source", "target_id": source_id.to_string()}]}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let pending = body_json(response).await;
    // 在途修改期间再提任何变更 → 409（单 pending）。
    let response = app
        .clone()
        .oneshot(post_json(
            &format!("/v1/kb/memories/{memory_id}/changes?project_id={project}"),
            json!({"action": "withdraw", "change_reason": "并发撤回"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    // 裁决在途修改（批准 → v2 CURRENT），随后撤回提案批准 → WITHDRAWN。
    let app2 = router(pool.clone());
    let response = app2
        .clone()
        .clone()
        .oneshot(post_json(
            &format!("/v1/kb/memories/{memory_id}/review?project_id={project}"),
            json!({"action": "approve", "base_memory_revision_id":
                   pending["revision_id"].to_string().trim_matches('"').to_string()}),
        ))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        body_json(response).await
    );
    let response = app2
        .clone()
        .oneshot(post_json(
            &format!("/v1/kb/memories/{memory_id}/changes?project_id={project}"),
            json!({"action": "withdraw", "change_reason": "结论已过时",
                   "title": "不允许的内容", "summary": "s"}),
        ))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "withdraw carries no content"
    );
    let response = app2
        .clone()
        .oneshot(post_json(
            &format!("/v1/kb/memories/{memory_id}/changes?project_id={project}"),
            json!({"action": "withdraw", "change_reason": "结论已过时"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let wd = body_json(response).await;
    let response = app2
        .clone()
        .oneshot(post_json(
            &format!("/v1/kb/memories/{memory_id}/review?project_id={project}"),
            json!({"action": "approve", "base_memory_revision_id":
                   wd["revision_id"].to_string().trim_matches('"').to_string()}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app2
        .clone()
        .oneshot(get(&format!(
            "/v1/kb/memories/{memory_id}?project_id={project}"
        )))
        .await
        .unwrap();
    let detail = body_json(response).await;
    assert_eq!(detail["state"], "withdrawn");
    assert_eq!(detail["current_revision_id"], Value::Null);
}
