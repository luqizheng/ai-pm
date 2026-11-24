//! R2（B09-01）chat 注入的项目正式记录投影验收。
//!
//! 覆盖任务书 §五 的关键要求：
//! - AI 能引用/定位记录（code + version + 状态进投影）；
//! - 区分正式与草案/待确认（未经人工确认的显式标记，不得当正式决策）；
//! - 项目隔离（跨项目记录不可见）；
//! - 空域显式「（无）」（不允许凭空生成项目状态）；
//! - 变更后取最新版本（实时投影，无缓存）；
//! - 选择与长度控制（预算内整行裁剪，头部语义与「待人工处理」不可裁）。

use pm_ai::chat_context::build_chat_context;
use pm_ai::project_records::{build_project_records_body, PROJECT_RECORDS_CAP_CHARS};
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

/// 种项目 + 真实人工 principal（`created_by` 外键目标）。
async fn seed_project(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("pr-{code}"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    (boot.project_id, boot.human_id)
}

/// 直接种需求 + 当前修订（state 决定它是正式还是草案）。
async fn seed_requirement(
    pool: &PgPool,
    project_id: Uuid,
    created_by: Uuid,
    code: &str,
    title: &str,
    version: i32,
    state: &str,
) -> Uuid {
    let requirement_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement (id, project_id, code, title, lifecycle_status, tenant_id) \
         VALUES ($1, $2, $3, $4, 'active', $5) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(code)
    .bind(title)
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
         (id, requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, 'hash', $7, $8) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(requirement_id)
    .bind(project_id)
    .bind(version)
    .bind(json!({"summary": title, "markdown": format!("# {title}")}))
    .bind(state)
    .bind(created_by)
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.requirement SET current_revision_id = $1 WHERE id = $2")
        .bind(revision_id)
        .bind(requirement_id)
        .execute(pool)
        .await
        .unwrap();
    requirement_id
}

async fn seed_adr(
    pool: &PgPool,
    project_id: Uuid,
    created_by: Uuid,
    code: &str,
    status: &str,
    blocking: bool,
) {
    sqlx::query(
        "INSERT INTO design.adr (id, tenant_id, project_id, code, title, status, blockers_implementation, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(Uuid::new_v4())
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(code)
    .bind(format!("决策 {code}"))
    .bind(status)
    .bind(blocking)
    .bind(created_by)
    .execute(pool)
    .await
    .unwrap();
}

/// 正式需求与草案分列；草案行带「未经人工确认」标记；行带可定位元数据。
#[sqlx::test(migrations = "../../migrations")]
async fn formal_and_draft_requirements_are_projected_separately(pool: sqlx::PgPool) {
    let (project_id, human) = seed_project(&pool, "R2A").await;
    seed_requirement(
        &pool,
        project_id,
        human,
        "REQ-001",
        "多租户隔离",
        2,
        "approved",
    )
    .await;
    seed_requirement(
        &pool,
        project_id,
        human,
        "REQ-002",
        "手机号登录",
        3,
        "pending_confirmation",
    )
    .await;
    seed_adr(&pool, project_id, human, "ADR-001", "accepted", false).await;
    seed_adr(&pool, project_id, human, "ADR-002", "proposed", true).await;

    let body = build_project_records_body(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();

    // 正式节：approved + 可定位元数据。
    assert!(
        body.contains("REQ-001 · v2 · approved · 多租户隔离"),
        "{body}"
    );
    // 草案节：pending_confirmation 显式标注未经人工确认。
    assert!(
        body.contains("REQ-002 · v3 · pending_confirmation"),
        "{body}"
    );
    assert!(body.contains("未经人工确认"), "{body}");
    // ADR 状态 + 阻塞标记。
    assert!(body.contains("ADR-001 · accepted"), "{body}");
    assert!(body.contains("ADR-002 · proposed"), "{body}");
    assert!(body.contains("阻塞实施"), "{body}");
    // 语义头部在场（定义「未列出即不存在」）。
    assert!(body.contains("未列出的记录即不存在"), "{body}");
}

/// 项目隔离：同租户另一项目的记录不可见。
#[sqlx::test(migrations = "../../migrations")]
async fn projection_is_project_isolated(pool: sqlx::PgPool) {
    let (project_a, _human_a) = seed_project(&pool, "R2B").await;
    let (project_b, _human_b) = seed_project(&pool, "R2C").await;
    seed_requirement(
        &pool,
        project_b,
        _human_b,
        "REQ-SECRET",
        "他项目需求",
        1,
        "approved",
    )
    .await;

    let body_a = build_project_records_body(&pool, SYSTEM_TENANT, project_a)
        .await
        .unwrap();
    assert!(
        !body_a.contains("REQ-SECRET"),
        "跨项目记录不得进入投影: {body_a}"
    );
    assert!(
        !body_a.contains("他项目需求"),
        "跨项目记录不得进入投影: {body_a}"
    );
    // 自己项目的空态显式。
    assert!(body_a.contains("（无）"), "{body_a}");
}

/// 变更后取最新版本：修订状态推进 → 下一次投影即反映（无缓存）。
#[sqlx::test(migrations = "../../migrations")]
async fn projection_reflects_latest_state_without_cache(pool: sqlx::PgPool) {
    let (project_id, _human) = seed_project(&pool, "R2D").await;
    seed_requirement(
        &pool,
        project_id,
        _human,
        "REQ-001",
        "手机号登录",
        1,
        "pending_confirmation",
    )
    .await;

    let before = build_project_records_body(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();
    assert!(before.contains("pending_confirmation"), "{before}");
    assert!(!before.contains("· approved ·"), "{before}");

    // 人工确认（state → approved）。
    sqlx::query(
        "UPDATE delivery.requirement_revision SET state = 'approved' \
         WHERE requirement_id = (SELECT id FROM delivery.requirement WHERE project_id = $1 \
                                 AND code = 'REQ-001')",
    )
    .bind(project_id)
    .execute(&pool)
    .await
    .unwrap();

    let after = build_project_records_body(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();
    assert!(
        after.contains("REQ-001 · v1 · approved · 手机号登录"),
        "最新状态必须立即可见: {after}"
    );
}

/// 空项目：各域显式「（无）」；待人工计数为零——不允许凭空生成状态。
#[sqlx::test(migrations = "../../migrations")]
async fn empty_project_gets_explicit_empty_markers(pool: sqlx::PgPool) {
    let (project_id, _human) = seed_project(&pool, "R2E").await;
    let body = build_project_records_body(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();
    let empty_count = body.matches("（无）").count();
    assert!(
        empty_count >= 6,
        "需求正式/草案/设计/ADR/任务/风险/阻塞均应显式空态: {body}"
    );
    assert!(body.contains("访谈待人工 0"), "{body}");
}

/// 长度控制：记录远超预算时整行裁剪、总长不超节上限、头部与待人工保留。
///
/// 投影按 `updated_at DESC` 排（最新优先），行裁剪自尾部（最旧）弹出——
/// REQ-039 最新必保留，REQ-000 最旧必被裁。
#[sqlx::test(migrations = "../../migrations")]
async fn oversized_projection_is_line_trimmed_within_budget(pool: sqlx::PgPool) {
    let (project_id, human) = seed_project(&pool, "R2F").await;
    for i in 0..60 {
        let title = format!(
            "需求 {} 的很长标题填充以占据投影空间 {}",
            i,
            "补".repeat(100)
        );
        seed_requirement(
            &pool,
            project_id,
            human,
            &format!("REQ-{i:03}"),
            &title,
            1,
            "approved",
        )
        .await;
    }
    // 附带一条审计事件（裁剪优先级最高的一节）。
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (id, project_id, actor_id, action, target_type, tenant_id) \
         VALUES ($1, $2, $3, 'requirement.confirmed', 'requirement', $4)",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(human)
    .bind(SYSTEM_TENANT)
    .execute(&pool)
    .await
    .unwrap();

    let body = build_project_records_body(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();
    assert!(
        body.chars().count() <= PROJECT_RECORDS_CAP_CHARS,
        "投影必须受节预算约束: {} > {}",
        body.chars().count(),
        PROJECT_RECORDS_CAP_CHARS
    );
    // 头部语义不可裁。
    assert!(body.contains("未列出的记录即不存在"), "{body}");
    // 「待人工处理」（priority=0）不可整段裁。
    assert!(body.contains("待人工处理"), "{body}");
    // 最新行保留、最旧行被裁（整行裁剪，不产生半行）。
    assert!(body.contains("REQ-059"), "{body}");
    assert!(!body.contains("REQ-000"), "预算外的最旧行应被裁掉: {body}");
}

/// build_chat_context 装配：records 节在注入产物中（记账可见）。
#[sqlx::test(migrations = "../../migrations")]
async fn chat_context_includes_project_records_section(pool: sqlx::PgPool) {
    let (project_id, human) = seed_project(&pool, "R2G").await;
    seed_requirement(
        &pool,
        project_id,
        human,
        "REQ-001",
        "多租户隔离",
        1,
        "approved",
    )
    .await;

    let context = build_chat_context(&pool, SYSTEM_TENANT, project_id, None)
        .await
        .unwrap();
    let keys: Vec<&str> = context.sections.iter().map(|s| s.key.as_str()).collect();
    assert!(
        keys.contains(&"project_records"),
        "records 节必须进入注入: {keys:?}"
    );
    let records = context
        .sections
        .iter()
        .find(|s| s.key == "project_records")
        .unwrap();
    assert!(records.body.contains("REQ-001"), "{}", records.body);
    assert!(context.metadata()["sections_included"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "project_records"));
}
