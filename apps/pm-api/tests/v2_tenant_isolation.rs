//! 阶段 C · V-2 旧 ai 栈租户隔离负向测试（V2-T01~T10）。
//!
//! 口径（交付物 05 V1.1 §3.2 + 任务书 §6.3）：
//! - `regression_*` = Security Regression，计入 G2；
//! - `char_*` = Characterization（SEC-P2 现状基线），**不计入通过**。
//!
//! V2-T06（删除）登记为 UNKNOWN：旧 ai 栈 `ai.project_knowledge` /
//! `ai.cognition_item` 无 DELETE 端点（insert-only + 状态机），没有可执行的
//! 跨租户删除路径可测——结论与证据见 D04。

use axum::http::StatusCode;
use pm_application::actor::AuditActor;
use pm_application::kb_service::memory::{propose_create, MemoryContent, RefRole, ResolvedRef};
use pm_application::pc::{register_source, SourceInput};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod l2_security;

use l2_security::*;

fn decision_uri(project_id: Uuid, item_id: Uuid) -> String {
    format!("/v1/pm/projects/{project_id}/cognition/items/{item_id}/decision")
}

/// 以 ownerB 确认一条 B 认知条目，返回 (条目 id, 知识正文)——作为跨租户读标记。
async fn seed_confirmed_b_knowledge(pool: &PgPool, b: &TenantCtx) -> (Uuid, Uuid, String) {
    let items = start_cognition_items(&b.app, &b.token, b.project_id).await;
    let item = items[0];
    let response = b
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &decision_uri(b.project_id, item),
            Some(&b.token),
            Some(json!({ "action": "confirm" })),
        ))
        .await
        .unwrap();
    let (status, body) = status_body(response).await;
    assert_eq!(status, StatusCode::OK, "ownerB confirm must pass: {body}");
    let knowledge_id = Uuid::parse_str(body["knowledge_id"].as_str().unwrap()).unwrap();
    let content: String =
        sqlx::query_scalar("SELECT content FROM ai.project_knowledge WHERE id = $1")
            .bind(knowledge_id)
            .fetch_one(pool)
            .await
            .unwrap();
    (item, knowledge_id, content)
}

// ───────────────────────────────────────────────────────────── V2-T01

/// V2-T01（Regression）：Team A 访问自己的项目 → 按权限允许（读 + 写）。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t01_tenant_reads_own_project(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let items = start_cognition_items(&a.app, &a.token, a.project_id).await;

    for uri in [
        format!("/v1/pm/projects/{}/cognition", a.project_id),
        format!("/v1/pm/projects/{}/cognition/knowledge", a.project_id),
    ] {
        let response = a
            .app
            .clone()
            .oneshot(req_json("GET", &uri, Some(&a.token), None))
            .await
            .unwrap();
        let (status, body) = status_body(response).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "own project must be readable: {body}"
        );
    }

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &decision_uri(a.project_id, items[0]),
            Some(&a.token),
            Some(json!({ "action": "confirm" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "own project write");
}

// ───────────────────────────────────────────────────────────── V2-T02

/// V2-T02（Regression）：Team A 访问 Team B 项目认知 → 404；
/// 响应体不含任何 B 侧认知内容（逐字段/逐标记断言）。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t02_cross_tenant_project_cognition_denied(pool: PgPool) {
    let (a, b) = dual_tenants(&pool).await;
    let (_b_item, _b_knowledge, b_content) = seed_confirmed_b_knowledge(&pool, &b).await;

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "GET",
            &format!("/v1/pm/projects/{}/cognition", b.project_id),
            Some(&a.token),
            None,
        ))
        .await
        .unwrap();
    expect_cross_tenant_read_denied(
        response,
        &[StatusCode::NOT_FOUND],
        &[&b_content, "l2 tenant b"],
    )
    .await;
}

// ───────────────────────────────────────────────────────────── V2-T03

/// V2-T03（Regression）：Team A 用 Team B project_id 查 Knowledge → 404，
/// 响应体不含 B 侧知识正文。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t03_cross_tenant_knowledge_read_denied(pool: PgPool) {
    let (a, b) = dual_tenants(&pool).await;
    let (_b_item, b_knowledge, b_content) = seed_confirmed_b_knowledge(&pool, &b).await;

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "GET",
            &format!("/v1/pm/projects/{}/cognition/knowledge", b.project_id),
            Some(&a.token),
            None,
        ))
        .await
        .unwrap();
    expect_cross_tenant_read_denied(
        response,
        &[StatusCode::NOT_FOUND],
        &[&b_content, &b_knowledge.to_string()],
    )
    .await;
}

// ───────────────────────────────────────────────────────────── V2-T04

/// V2-T04（Regression）：Team A 用 Team B 的对象 ID（cognition item）在**自己的**
/// 项目路径下查/写 → 404；B 侧 DB 快照无变化（对象 ID 替换不可绕过租户过滤）。
///
/// 现状实现：`PgCognitionRepo` 全部查询带 `tenant_id` 条件；handler 先过项目可见性，
/// 因此 item 的租户不匹配落入 NotFound。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t04_cross_tenant_object_id_substitution_denied(pool: PgPool) {
    let (a, b) = dual_tenants(&pool).await;
    let (b_item, _b_knowledge, _b_content) = seed_confirmed_b_knowledge(&pool, &b).await;
    let before_b = ProjectSnapshot::capture(&pool, b.project_id).await;

    // 先给 A 准备一条 proposed 条目，防止「项目无条目」掩盖对象替换语义。
    let a_items = start_cognition_items(&a.app, &a.token, a.project_id).await;
    assert!(!a_items.is_empty());

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &decision_uri(a.project_id, b_item),
            Some(&a.token),
            Some(json!({ "action": "confirm" })),
        ))
        .await
        .unwrap();
    expect_cross_tenant_denied(
        &pool,
        b.project_id,
        &before_b,
        response,
        &[StatusCode::NOT_FOUND],
    )
    .await;
}

// ───────────────────────────────────────────────────────────── V2-T05

/// V2-T05（Regression）：Team A 修改 Team B Knowledge（确认 B 条目）→ 404；
/// B 侧状态与知识零变化。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t05_cross_tenant_knowledge_write_denied(pool: PgPool) {
    let (a, b) = dual_tenants(&pool).await;
    let items = start_cognition_items(&b.app, &b.token, b.project_id).await;
    let item = items[0];
    let before_b = ProjectSnapshot::capture(&pool, b.project_id).await;

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &decision_uri(b.project_id, item),
            Some(&a.token),
            Some(json!({ "action": "confirm" })),
        ))
        .await
        .unwrap();
    let body = expect_cross_tenant_denied(
        &pool,
        b.project_id,
        &before_b,
        response,
        &[StatusCode::NOT_FOUND],
    )
    .await;
    assert_eq!(body["message"], json!("project not found"));
}

// ───────────────────────────────────────────────────────────── V2-T07

/// V2-T07（Regression）：批量请求混入 Team B 任务 → 整批拒绝（404），
/// 合法子项也不得被写入（当前实现为全有或全无）；合法单批仍 200。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t07_mixed_batch_is_rejected_atomically(pool: PgPool) {
    let (a, b) = dual_tenants(&pool).await;
    let task_a = seed_task(&pool, a.project_id, "L2-A-DRAFT", "draft").await;
    let task_b = seed_task(&pool, b.project_id, "L2-B-DRAFT", "draft").await;

    let before_a = ProjectSnapshot::capture(&pool, a.project_id).await;
    let before_b = ProjectSnapshot::capture(&pool, b.project_id).await;

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &format!("/v1/pm/projects/{}/tasks/mark-ready", a.project_id),
            Some(&a.token),
            Some(json!({ "task_ids": [task_a, task_b] })),
        ))
        .await
        .unwrap();
    let (status, body) = status_body(response).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "mixed batch must be rejected: {body}"
    );
    assert_eq!(
        ProjectSnapshot::capture(&pool, a.project_id).await,
        before_a
    );
    assert_eq!(
        ProjectSnapshot::capture(&pool, b.project_id).await,
        before_b
    );

    // 合法单批（只含自己的任务）仍成功，防误伤。
    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &format!("/v1/pm/projects/{}/tasks/mark-ready", a.project_id),
            Some(&a.token),
            Some(json!({ "task_ids": [task_a] })),
        ))
        .await
        .unwrap();
    let (status, body) = status_body(response).await;
    assert_eq!(status, StatusCode::OK, "legal batch must pass: {body}");
    let task_a_status: String =
        sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
            .bind(task_a)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(task_a_status, "ready");
}

// ───────────────────────────────────────────────────────────── V2-T08-C

/// V2-T08-C（**Characterization**，不计入通过）：跨租户引用目标当前被接受。
///
/// SEC-P2-1：`kb.memory_ref` 非 KB 目标仅结构校验（不验存在性/归属）；
/// SEC-P2-2：`pc.source.target_*` 同样不做存在性/同租户校验。
/// 本用例以 Tenant A 的 project 引用 Tenant B 的 cognition item，固化现状基线；
/// SEC-P2 修复落地后应改为拒绝断言（V2-T08-R）。
#[sqlx::test(migrations = "../../migrations")]
async fn char_t08c_cross_tenant_reference_targets_are_accepted(pool: PgPool) {
    let (a, b) = dual_tenants(&pool).await;
    let (b_item, _b_knowledge, _b_content) = seed_confirmed_b_knowledge(&pool, &b).await;
    let actor = AuditActor::principal(a.human_id);

    // ① pc.source：A 的来源登记指向 B 的认知条目 → 当前成功。
    let source_id = register_source(
        &pool,
        a.project_id,
        &actor,
        SourceInput {
            source_kind: "project_cognition".to_string(),
            target_domain: "project_cognition".to_string(),
            target_type: "cognition_item".to_string(),
            target_id: b_item.to_string(),
            target_revision_id: None,
            revision_missing_reason: Some("l2-char".to_string()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: Some("characterization: cross-tenant target".to_string()),
            source_content_hash: None,
        },
    )
    .await
    .expect("Characterization baseline：pc.source 不校验目标租户");
    let (stored_tenant, stored_target, stored_domain): (Uuid, String, String) =
        sqlx::query_as("SELECT tenant_id, target_id, target_domain FROM pc.source WHERE id = $1")
            .bind(source_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored_tenant, a.tenant_id);
    assert_eq!(stored_target, b_item.to_string());
    assert_eq!(stored_domain, "project_cognition");

    // ② kb.memory_ref：A 的记忆提案引用 B 的认知条目 → 当前成功。
    let receipt = propose_create(
        &pool,
        a.tenant_id,
        a.project_id,
        &actor,
        MemoryContent {
            memory_type: "discussion".to_string(),
            title: "l2 char T08-C".to_string(),
            summary: "cross-tenant reference baseline".to_string(),
            body: None,
        },
        "characterization baseline (SEC-P2-1)",
        vec![
            ResolvedRef {
                role: RefRole::Subject,
                target_domain: "project_cognition".to_string(),
                target_type: "cognition_item".to_string(),
                target_id: b_item.to_string(),
                target_revision_id: None,
                revision_missing_reason: Some("l2-char".to_string()),
                locator: None,
            },
            ResolvedRef {
                role: RefRole::Evidence,
                target_domain: "project_cognition".to_string(),
                target_type: "cognition_item".to_string(),
                target_id: b_item.to_string(),
                target_revision_id: None,
                revision_missing_reason: Some("l2-char".to_string()),
                locator: None,
            },
        ],
    )
    .await
    .expect("Characterization baseline：kb.memory_ref 不校验目标归属");

    let refs: Vec<(String, String)> = sqlx::query_as(
        "SELECT target_domain, target_id FROM kb.memory_ref WHERE revision_id = $1 \
          ORDER BY ref_role",
    )
    .bind(receipt.revision_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(refs.len(), 2);
    assert!(refs.iter().all(|(_, target)| target == &b_item.to_string()));
}

// ───────────────────────────────────────────────────────────── V2-T09

/// V2-T09（Regression）：同租户、无项目成员资格的真人（userA2）访问 A 项目
/// → 404；确认写请求同样 404 且无写入。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t09_tenant_member_without_project_is_denied(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let items = start_cognition_items(&a.app, &a.token, a.project_id).await;
    let (_a2, a2_token) = tenant_member_without_project(&pool, &a, "a2-l2").await;
    let before = ProjectSnapshot::capture(&pool, a.project_id).await;

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "GET",
            &format!("/v1/pm/projects/{}/cognition", a.project_id),
            Some(&a2_token),
            None,
        ))
        .await
        .unwrap();
    expect_cross_tenant_read_denied(response, &[StatusCode::NOT_FOUND], &["l2 tenant a"]).await;

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &decision_uri(a.project_id, items[0]),
            Some(&a2_token),
            Some(json!({ "action": "confirm" })),
        ))
        .await
        .unwrap();
    expect_cross_tenant_denied(
        &pool,
        a.project_id,
        &before,
        response,
        &[StatusCode::NOT_FOUND],
    )
    .await;
}

// ───────────────────────────────────────────────────────────── V2-T10

/// V2-T10（Regression）：Project 与 Team 归属不匹配（tenantA 的 JWT 指向
/// tenantB 的 project_id）→ legacy 端点 404，不泄露 B 侧通知 payload。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t10_tenant_project_mismatch_denied(pool: PgPool) {
    let (a, b) = dual_tenants(&pool).await;
    let secret = "l2-tenant-b-notification-secret";
    sqlx::query(
        "INSERT INTO integration.notification (project_id, kind, payload, tenant_id) \
         VALUES ($1, 'veto_applied', $2, (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(b.project_id)
    .bind(json!({ "secret": secret }))
    .execute(&pool)
    .await
    .unwrap();

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "GET",
            &format!("/api/v1/projects/{}/notifications?limit=10", b.project_id),
            Some(&a.token),
            None,
        ))
        .await
        .unwrap();
    expect_cross_tenant_read_denied(response, &[StatusCode::NOT_FOUND], &[secret]).await;
}
