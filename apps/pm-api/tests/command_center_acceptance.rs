//! S01 · I5-E 项目总控台验收专项（I5-026~036）。
//!
//! 契约：`docs/s01-i5-read-model-contract.md`；事实：`docs/s01-i5-code-facts.md`
//!
//! 覆盖：
//! - I5-027 初始化全部完成 → 收缩为简要状态
//! - I5-028 新增初始化事项 → 重新展开
//! - I5-029 初始化必需项未定义 → 不伪造完成状态
//! - I5-030 活动重复来源 → 同一业务动作不重复展示
//! - I5-031 活动分页 → 稳定排序 + 连续分页不重不漏
//! - I5-032 活动来源 → 可追溯至审计
//! - I5-033 Outbox 历史 → 不依赖 Outbox 作为活动来源
//! - I5-034 部分数据缺失 → 区分无数据 / 未接入 / 无权限 / 加载失败
//! - I5-035 多领域待办 → 不混淆不同领域的原始状态
//! - I5-036 归档 Project → 历史活动可查看，但不可创建新业务活动

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::run_dispatch::NoopExecutor;
use pm_domain::SYSTEM_TENANT;
use serde_json::{json, Value};
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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
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

async fn get_center(pool: &PgPool, project_id: Uuid, query: &str) -> (StatusCode, Value) {
    let app = router(pool.clone());
    body_json(
        app.oneshot(request(&format!(
            "/v1/pm/projects/{project_id}/command-center{query}"
        )))
        .await
        .unwrap(),
    )
    .await
}

fn block<'a>(body: &'a Value, name: &str) -> &'a Value {
    &body["blocks"][name]
}

fn items<'a>(body: &'a Value, block_name: &str) -> &'a Vec<Value> {
    block(body, block_name)["data"]["items"].as_array().unwrap()
}

/// 插一条白名单内的审计事件（可指定 created_at 以制造并列排序）。
async fn seed_audit(
    pool: &PgPool,
    project_id: Uuid,
    actor_id: Uuid,
    action: &str,
    target_id: Uuid,
    created_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Uuid {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO governance.audit_log \
            (project_id, actor_id, action, target_type, target_id, tenant_id, created_at) \
         VALUES ($1, $2, $3, 'task_run', $4, \
                 (SELECT tenant_id FROM core.project WHERE id = $1), \
                 COALESCE($5, now())) RETURNING id",
    )
    .bind(project_id)
    .bind(actor_id)
    .bind(action)
    .bind(target_id)
    .bind(created_at)
    .fetch_one(pool)
    .await
    .unwrap();
    id
}

/// I5-027/028/029：全部事项完成 → 收缩；出现新的必需待办 → 重新展开；
/// 必需项未定义 → 完成度恒为 `not_determinable`（不伪造完成）。
#[sqlx::test(migrations = "../../migrations")]
async fn initialization_progress_collapses_and_expands_again(pool: PgPool) {
    let (project_id, owner) = seed_project(&pool, "I5E-1").await;

    let (status, body) = get_center(&pool, project_id, "").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let init = block(&body, "initialization_progress")["data"].clone();
    assert_eq!(
        init["completion"],
        json!("not_determinable"),
        "I5-029 不得伪造完成状态"
    );
    // 必需项「项目目标」尚未建立 → 面板保持展开（不得隐藏未完成项）。
    assert_eq!(init["expanded"], json!(true), "必需项未完成时必须展开");

    // 建立正式 Goal（published 修订）→ 必需项全部完成 → 收缩。
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.project_document (project_id, doc_key, doc_type, canonical_path, tenant_id) \
         VALUES ($1, 'project-goal', 'knowledge', 'docs/project-goal.md', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai.document_revision \
            (project_id, document_id, revision_no, git_commit_sha, content_hash, status, created_by, tenant_id) \
         VALUES ($1, $2, 1, 'sha-1', 'hash-1', 'published', $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(document_id)
    .bind(owner)
    .execute(&pool)
    .await
    .unwrap();

    let (_status, body) = get_center(&pool, project_id, "").await;
    let init = block(&body, "initialization_progress")["data"].clone();
    assert_eq!(
        init["expanded"],
        json!(false),
        "全部必需项完成后应收缩为简要状态"
    );

    // 出现新的必需待办（人工处理有待办）→ 重新展开。
    sqlx::query(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
         VALUES ($1, '待验收', 'submitted', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .execute(&pool)
    .await
    .unwrap();

    let (_status, body) = get_center(&pool, project_id, "").await;
    let init = block(&body, "initialization_progress")["data"].clone();
    assert_eq!(
        init["expanded"],
        json!(true),
        "出现新的必需待办必须重新展开"
    );
}

/// I5-030/031/032：活动去重 + 稳定排序 + 连续分页 + 可追溯到审计。
#[sqlx::test(migrations = "../../migrations")]
async fn activity_is_deduped_stably_paged_and_traceable(pool: PgPool) {
    let (project_id, owner) = seed_project(&pool, "I5E-2").await;
    let stamp = chrono::DateTime::parse_from_rfc3339("2026-11-04T10:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    // 5 条同 created_at 的白名单事件 → 必须按 id DESC 稳定排序。
    let mut ids: Vec<Uuid> = Vec::new();
    for _ in 0..5 {
        ids.push(
            seed_audit(
                &pool,
                project_id,
                owner,
                "run_started",
                Uuid::new_v4(),
                Some(stamp),
            )
            .await,
        );
    }
    // 同 target 同 action 的重复事件 → 去重后只留一条（保留最新）。
    let dup_target = Uuid::new_v4();
    let dup_old = seed_audit(
        &pool,
        project_id,
        owner,
        "finding.decided",
        dup_target,
        Some(stamp),
    )
    .await;
    let dup_new = seed_audit(
        &pool,
        project_id,
        owner,
        "finding.decided",
        dup_target,
        Some(stamp),
    )
    .await;
    ids.push(dup_old);
    ids.push(dup_new);

    // 第 1 页（limit 3）
    let (_status, page1) =
        get_center(&pool, project_id, "?activity_limit=3&activity_offset=0").await;
    let items1 = items(&page1, "recent_activity");
    assert_eq!(items1.len(), 3, "分页必须受限（去重不得造成少行）");
    // 第 2 页
    let (_status, page2) =
        get_center(&pool, project_id, "?activity_limit=3&activity_offset=3").await;
    let items2 = items(&page2, "recent_activity");

    let ids1: Vec<Uuid> = items1
        .iter()
        .map(|item| Uuid::parse_str(item["id"].as_str().unwrap()).unwrap())
        .collect();
    let ids2: Vec<Uuid> = items2
        .iter()
        .map(|item| Uuid::parse_str(item["id"].as_str().unwrap()).unwrap())
        .collect();
    // 不重不漏（I5-031）。
    for id in &ids2 {
        assert!(!ids1.contains(id), "两页不得重复：{id}");
    }

    // 分页 = 完整列表的切分（I5-031）：两页拼起来等于一次性取全量。
    let (_status, full) = get_center(&pool, project_id, "?activity_limit=100").await;
    let full_ids: Vec<Uuid> = items(&full, "recent_activity")
        .iter()
        .map(|item| Uuid::parse_str(item["id"].as_str().unwrap()).unwrap())
        .collect();
    let mut paged = ids1.clone();
    paged.extend(ids2.iter().copied());
    assert_eq!(paged, full_ids, "分页结果必须等于完整列表的切分");

    // 排序保持库内序（I5-031）：以库自身的 `ORDER BY created_at DESC, id DESC`
    // 为准（PostgreSQL 的 uuid 排序是原始字节序，不能用字符串序复刻）。
    let sql_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM governance.audit_log \
          WHERE tenant_id = $1 AND project_id = $2 AND action = ANY($3) \
          ORDER BY created_at DESC, id DESC",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(vec![
        "run_started".to_string(),
        "finding.decided".to_string(),
    ])
    .fetch_all(&pool)
    .await
    .unwrap();
    // 完整列表是库内序的**子序列**（只少了被去重的那条）。
    let mut cursor = 0usize;
    for id in &full_ids {
        let position = sql_ids[cursor..]
            .iter()
            .position(|candidate| candidate == id)
            .expect("活动项必须来自库内有序列表");
        cursor += position + 1;
    }
    assert_eq!(
        full_ids.len(),
        sql_ids.len() - 1,
        "同 target 的重复事件应只保留一条（I5-030）"
    );
    assert!(
        !full_ids.contains(&dup_old) || !full_ids.contains(&dup_new),
        "同 dedup_key 的两条不得同时出现"
    );

    // 去重：同 dedup_key 只留一条（I5-030）。
    let dedup_keys: Vec<String> = items(&full, "recent_activity")
        .iter()
        .map(|item| item["dedup_key"].as_str().unwrap().to_string())
        .collect();
    let mut unique = dedup_keys.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), dedup_keys.len(), "同一业务动作不得重复展示");

    // 可追溯到审计（I5-032）：带 source_type / source_id。
    let first = &items1[0];
    assert!(first["source_type"].is_string());
    assert!(first["source_id"].is_string());
    assert!(!first["summary"].as_str().unwrap().is_empty());
}

/// I5-033：Outbox 事件**不作为**活动来源（活动只来自审计）。
#[sqlx::test(migrations = "../../migrations")]
async fn outbox_events_never_appear_in_activity(pool: PgPool) {
    let (project_id, _owner) = seed_project(&pool, "I5E-3").await;
    sqlx::query(
        "INSERT INTO integration.outbox_event \
            (project_id, event_type, aggregate_type, aggregate_id, payload, status, \
             idempotency_key, schema_version, tenant_id) \
         VALUES ($1, 'DeliverySubmitted', 'task', $2, '{}'::jsonb, 'delivered', \
                 $3, 1, (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(Uuid::new_v4())
    .bind(format!("outbox-{}", Uuid::new_v4()))
    .execute(&pool)
    .await
    .unwrap();

    let (_status, body) = get_center(&pool, project_id, "").await;
    let activity_items = items(&body, "recent_activity");
    assert!(
        activity_items.is_empty(),
        "活动流只认审计；Outbox 不是活动历史（I5-033）：{activity_items:?}"
    );
    let total = block(&body, "recent_activity")["data"]["total"]
        .as_i64()
        .unwrap();
    assert_eq!(total, 0);
}

/// I5-034：区分「无数据」与「未接入」与「无权限」。
#[sqlx::test(migrations = "../../migrations")]
async fn missing_data_is_distinguished_from_not_applicable_and_forbidden(pool: PgPool) {
    let (project_id, _owner) = seed_project(&pool, "I5E-4").await;

    // 无数据：待人工处理为空 → total=0（不是「加载失败」）。
    let (status, body) = get_center(&pool, project_id, "").await;
    assert_eq!(status, StatusCode::OK);
    let actions = block(&body, "human_action_summary");
    assert_eq!(actions["status"], json!("ok"), "数据源可用");
    assert_eq!(actions["data"]["total"], json!(0));

    // 未接入：零仓库 → 仓库接入「未开始」、代码分析「不适用」（合法状态，不是失败）。
    let init = block(&body, "initialization_progress")["data"].clone();
    let items = init["items"].as_array().unwrap();
    let find = |key: &str| {
        items
            .iter()
            .find(|item| item["key"] == json!(key))
            .unwrap()
            .clone()
    };
    assert_eq!(find("repository_access")["status"], json!("not_started"));
    assert_eq!(find("code_analysis")["status"], json!("not_applicable"));

    // 无权限：非成员 → 404（不泄露存在性），且**不是**「加载失败」语义。
    let (other_project, _other) = seed_project(&pool, "I5E-4-OTHER").await;
    sqlx::query("DELETE FROM core.project_member WHERE project_id = $1 AND principal_id = $2")
        .bind(other_project)
        .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
        .execute(&pool)
        .await
        .unwrap();
    let (status, _body) = get_center(&pool, other_project, "").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// I5-035：多领域待办各自保留原始状态，不得混成统一状态。
#[sqlx::test(migrations = "../../migrations")]
async fn multi_domain_backlog_keeps_each_domain_state(pool: PgPool) {
    let (project_id, owner) = seed_project(&pool, "I5E-5").await;

    // 领域一：Finding 待确认（原始状态 PENDING）。
    let session_id: Uuid = sqlx::query_scalar(
        "INSERT INTO discovery.discovery_session \
            (project_id, status, source, kind, created_by, tenant_id) \
         VALUES ($1, 'IN_PROGRESS', 'SYSTEM', 'INITIAL_ONBOARDING', $2, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(owner)
    .fetch_one(&pool)
    .await
    .unwrap();
    let finding_id: Uuid = sqlx::query_scalar(
        "INSERT INTO discovery.discovery_finding \
            (project_id, session_id, kind, content, confidence, verification_status, actor_role, tenant_id) \
         VALUES ($1, $2, 'observed', '发现', 0.9, 'PENDING', 'human', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 领域二：知识提案待确认（原始状态 redline_pending）。
    let proposal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.document_proposal \
            (project_id, doc_key, doc_type, canonical_path, proposed_markdown, \
             proposed_content_hash, status, redline_kind, created_by, tenant_id) \
         VALUES ($1, 'architecture', 'knowledge', 'docs/architecture.md', '# d', 'h1', \
                 'redline_pending', 'rewrite_delivered', $2, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(owner)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 领域三：待验收交付（Task 原始状态 submitted）。
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
         VALUES ($1, '交付任务', 'submitted', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let (_status, body) = get_center(&pool, project_id, "").await;
    let actions = items(&body, "human_action_summary");
    let by_id = |id: Uuid| {
        actions
            .iter()
            .find(|item| item["id"] == json!(id.to_string()))
            .unwrap_or_else(|| panic!("缺少待办 {id}"))
            .clone()
    };
    assert_eq!(by_id(finding_id)["source_state"], json!("PENDING"));
    assert_eq!(by_id(finding_id)["domain"], json!("discovery"));
    assert_eq!(by_id(proposal_id)["source_state"], json!("redline_pending"));
    assert_eq!(by_id(proposal_id)["domain"], json!("knowledge"));
    assert_eq!(by_id(task_id)["source_state"], json!("submitted"));
    assert_eq!(by_id(task_id)["domain"], json!("delivery"));
}

/// I5-036：归档项目历史活动仍可查看，但不再接受新的业务活动。
#[sqlx::test(migrations = "../../migrations")]
async fn archived_project_keeps_history_but_accepts_no_new_activity(pool: PgPool) {
    let (project_id, owner) = seed_project(&pool, "I5E-6").await;
    seed_audit(
        &pool,
        project_id,
        owner,
        "run_started",
        Uuid::new_v4(),
        None,
    )
    .await;

    pm_application::archive::close_project(
        &pool,
        &NoopExecutor,
        &pm_application::actor::AuditActor::principal(owner),
        SYSTEM_TENANT,
        project_id,
        "i5e-archive",
    )
    .await
    .unwrap();

    // 历史活动仍可读（且归档本身也是一条白名单活动）。
    let (_status, body) = get_center(&pool, project_id, "").await;
    assert_eq!(
        block(&body, "project_summary")["data"]["closed"],
        json!(true)
    );
    let activity_items = items(&body, "recent_activity");
    assert!(
        activity_items
            .iter()
            .any(|item| item["activity_type"] == json!("project_closed")),
        "归档事件应出现在活动流：{activity_items:?}"
    );

    // 归档后不得再产生新的业务活动：Goal 提案被归档门禁拒绝（I4-F）。
    let root = pm_application::knowledge::KnowledgeRoot::new(
        std::env::temp_dir().join(format!("ai-pm-i5e-{}", Uuid::new_v4())),
    );
    let err = pm_application::knowledge_proposal::submit_proposal(
        &pool,
        &root,
        pm_application::knowledge_proposal::ProposalRequest {
            project_id,
            doc_key: "project-goal".to_string(),
            doc_type: "knowledge".to_string(),
            markdown: "# Project Goal\n\n归档后\n".to_string(),
            change_summary: Some("i5e".to_string()),
            source_event_id: None,
            analysis_run_id: None,
            source_ref: Some("i5e".to_string()),
            redline_kind: None,
            base_revision_id: None,
            created_by: owner,
            operation_id: format!("i5e-{}", Uuid::new_v4()),
        },
    )
    .await;
    assert!(
        err.is_err(),
        "归档项目不得再产生新的业务活动（Goal 提案应被门禁拒绝）"
    );
}
