//! Batch 3A：chat 精简注入（S1 项目画像 + S2 仓库事实）的验收测试。
//!
//! 覆盖整合计划验收项：
//! - A-6 注入记账（`sections_included` / `sections_dropped` / `chars_total`）；
//! - A-7 凭据零注入（D1 关键字扫描断言）；
//! - A-8 chat 预算独立（8k，与访谈 36k 隔离）；
//! - A-9 无锚点线程零注入（向后兼容：行为与注入前逐字节一致）。

use pm_ai::chat_context::{
    anchor_repository, build_chat_context, CHAT_KEY_PROJECT_PROFILE, CHAT_KEY_REPOSITORY_FACTS,
    CHAT_TOTAL_BUDGET_CHARS,
};
use pm_ai::discovery_context::{assemble_chat, SectionDraft, TOTAL_BUDGET_CHARS};
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_project(pool: &PgPool, name: &str, description: &str) -> Uuid {
    let customer_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.customer (id, name, tenant_id) VALUES ($1, 'Acme', $2)")
        .bind(customer_id)
        .bind(SYSTEM_TENANT)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.project (id, customer_id, code, name, description, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(project_id)
    .bind(customer_id)
    .bind(format!("p-{project_id}"))
    .bind(name)
    .bind(description)
    .bind(SYSTEM_TENANT)
    .execute(pool)
    .await
    .unwrap();
    project_id
}

async fn seed_repository(
    pool: &PgPool,
    project_id: Uuid,
    name: &str,
    default_branch: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
             (tenant_id, project_id, name, url, default_branch, status, head_commit_sha) \
         VALUES ($1, $2, $3, $4, $5, 'verified', 'abc123') RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(name)
    .bind(format!("https://example.invalid/{name}.git"))
    .bind(default_branch)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A-6：两节注入 + 记账字段齐备。
#[sqlx::test(migrations = "../../migrations")]
async fn chat_context_injects_two_sections_with_accounting(pool: PgPool) {
    let project = seed_project(&pool, "Atlas Platform", "Acme Research").await;
    seed_repository(&pool, project, "atlas-platform", "main").await;

    let context = build_chat_context(&pool, SYSTEM_TENANT, project, Some("atlas-platform"))
        .await
        .expect("context builds");

    assert!(!context.is_empty());
    let keys: Vec<&str> = context.sections.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(
        keys,
        vec![CHAT_KEY_PROJECT_PROFILE, CHAT_KEY_REPOSITORY_FACTS]
    );

    // 项目画像节含项目名与状态。
    let profile = context
        .sections
        .iter()
        .find(|s| s.key == CHAT_KEY_PROJECT_PROFILE)
        .expect("project profile section");
    assert!(profile.body.contains("Atlas Platform"), "{}", profile.body);
    assert!(profile.body.contains("Acme Research"), "{}", profile.body);

    // 仓库事实节含仓库名/分支/HEAD。
    let repo = context
        .sections
        .iter()
        .find(|s| s.key == CHAT_KEY_REPOSITORY_FACTS)
        .expect("repository facts section");
    assert!(repo.body.contains("atlas-platform"), "{}", repo.body);
    assert!(repo.body.contains("main"), "{}", repo.body);
    assert!(repo.body.contains("abc123"), "{}", repo.body);

    // 记账载荷字段齐备。
    let meta = context.metadata();
    assert_eq!(meta["chars_total"], json!(context.chars_total));
    assert_eq!(meta["sections_dropped"], json!([] as [String; 0]));
    assert!(meta["sections_included"].as_array().is_some());
}

/// A-9：无锚点 → 只注入项目画像（零仓库节），存量线程行为不变。
#[sqlx::test(migrations = "../../migrations")]
async fn chat_context_without_anchor_injects_project_only(pool: PgPool) {
    let project = seed_project(&pool, "Atlas Platform", "Acme Research").await;
    seed_repository(&pool, project, "atlas-platform", "main").await;

    let context = build_chat_context(&pool, SYSTEM_TENANT, project, None)
        .await
        .expect("context builds");

    let keys: Vec<&str> = context.sections.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(keys, vec![CHAT_KEY_PROJECT_PROFILE]);
    assert!(context
        .sections
        .iter()
        .all(|s| !s.body.contains("atlas-platform")));
}

/// 锚点指向不存在的仓库 → 仓库节缺席，**不伪造**、不报错。
#[sqlx::test(migrations = "../../migrations")]
async fn chat_context_anchor_to_missing_repository_yields_no_repo_section(pool: PgPool) {
    let project = seed_project(&pool, "Atlas Platform", "Acme Research").await;

    let context = build_chat_context(&pool, SYSTEM_TENANT, project, Some("no-such-repo"))
        .await
        .expect("context builds");

    let keys: Vec<&str> = context.sections.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(keys, vec![CHAT_KEY_PROJECT_PROFILE]);
}

/// 项目不存在 → 零注入（空 context，调用方完全跳过注入）。
#[sqlx::test(migrations = "../../migrations")]
async fn chat_context_missing_project_is_empty(pool: PgPool) {
    let context = build_chat_context(&pool, SYSTEM_TENANT, Uuid::new_v4(), None)
        .await
        .expect("context builds");

    assert!(context.is_empty());
    assert_eq!(
        context.metadata(),
        serde_json::json!({
            "sections_included": Vec::<String>::new(),
            "sections_dropped": Vec::<String>::new(),
            "chars_total": 0,
        })
    );
}

/// A-8：chat 预算独立于访谈（8k vs 36k），且互不影响。
#[test]
fn chat_budget_is_independent_from_interview_budget() {
    assert_eq!(CHAT_TOTAL_BUDGET_CHARS, 8_000);
    assert_eq!(TOTAL_BUDGET_CHARS, 36_000);

    // 超预算时整节删除（仓库事实先删），记账 dropped。
    let long = "x".repeat(20_000);
    let context = assemble_chat(vec![
        SectionDraft::new(CHAT_KEY_PROJECT_PROFILE, "项目画像", long.clone()),
        SectionDraft::new(CHAT_KEY_REPOSITORY_FACTS, "仓库事实", long),
    ]);
    assert!(context.chars_total <= CHAT_TOTAL_BUDGET_CHARS);
    assert!(context
        .sections_dropped
        .contains(&CHAT_KEY_REPOSITORY_FACTS.to_string()));
    assert_eq!(context.sections.len(), 1);
    assert_eq!(context.sections[0].key, CHAT_KEY_PROJECT_PROFILE);
}

/// 逐节上限生效（项目画像 2,000）。
#[test]
fn chat_sections_respect_per_section_caps() {
    let long = "y".repeat(10_000);
    let context = assemble_chat(vec![SectionDraft::new(
        CHAT_KEY_PROJECT_PROFILE,
        "项目画像",
        long,
    )]);
    assert_eq!(context.sections[0].chars, 2_000);
    assert!(context.sections[0].truncated);
}

/// A-7：注入源不含任何 provider/凭据字段（静态扫描）。
#[test]
fn chat_injection_sql_contains_no_credential_columns() {
    let source = include_str!("../src/chat_context.rs");
    for forbidden in [
        "api_key",
        "api_secret",
        "token",
        "password",
        "secret",
        "team_llm_provider",
        "credential",
    ] {
        // 只扫描 SQL 常量区段（注入投影的查询语句）。
        let sql_region = source
            .split("const SQL_PROJECT_PROFILE")
            .nth(1)
            .expect("sql constants present")
            .split("/// 构造 chat 注入")
            .next()
            .expect("sql region bounded");
        assert!(
            !sql_region.contains(forbidden),
            "chat injection SQL must not reference {forbidden}"
        );
    }
}

/// 锚点读取：只认字符串型 repository_name，空值/非字符串 → None。
#[test]
fn anchor_repository_reads_only_string_values() {
    assert_eq!(
        anchor_repository(Some(&json!({"repository_name": "atlas-platform"}))),
        Some("atlas-platform".to_string())
    );
    assert_eq!(
        anchor_repository(Some(&json!({"repository_name": "  "}))),
        None
    );
    assert_eq!(
        anchor_repository(Some(&json!({"repository_name": 42}))),
        None
    );
    assert_eq!(anchor_repository(Some(&json!({}))), None);
    assert_eq!(anchor_repository(None), None);
}

// ────────────────────────────── B3 / CX2：对象节（S3+）装配与预算记账 ──────────

use pm_ai::chat_context::{build_chat_context_with_objects, CHAT_CAP_OBJECT_CONTEXT};
use pm_application::ai_context_items::InjectionSource;

fn object_source(key: &str, title: &str, meta: &str, body: Option<String>) -> InjectionSource {
    InjectionSource {
        item_id: Uuid::new_v4(),
        section_key: key.to_string(),
        title: title.to_string(),
        meta: meta.to_string(),
        body,
        updated_at: chrono::Utc::now(),
    }
}

/// 对象节按预算装配：装得下的注入、放不下的整节裁掉且**逐对象**记账。
#[sqlx::test(migrations = "../../migrations")]
async fn object_sections_respect_budget_and_drop_per_object(pool: PgPool) {
    let project_id = seed_project(&pool, "CX2", "object section fixture").await;

    // 两个小对象（都装得下）+ 一个大对象（放不下 → 整节裁掉）。
    let small_a = object_source(
        "object:knowledge:00000000-0000-0000-0000-00000000000a",
        "K-1",
        "knowledge · v1 identity",
        Some("confirmed fact a.".repeat(10)),
    );
    let small_b = object_source(
        "object:task:00000000-0000-0000-0000-00000000000b",
        "T-1",
        "task · ready",
        None,
    );
    let oversized = object_source(
        "object:requirement:00000000-0000-0000-0000-00000000000c",
        "REQ-BIG",
        "requirement · v1 approved",
        Some("x".repeat(CHAT_CAP_OBJECT_CONTEXT + 500)),
    );
    // 大对象放前面（updated_at DESC 顺序的模拟）：裁剪记账语义更受考验。
    let objects = [oversized, small_a, small_b];

    let context = build_chat_context_with_objects(&pool, SYSTEM_TENANT, project_id, None, &objects)
        .await
        .unwrap();

    let keys: Vec<&str> = context.sections.iter().map(|s| s.key.as_str()).collect();
    assert!(
        keys.contains(&"object:knowledge:00000000-0000-0000-0000-00000000000a"),
        "small object a must be included: {keys:?}"
    );
    assert!(
        keys.contains(&"object:task:00000000-0000-0000-0000-00000000000b"),
        "small object b must be included: {keys:?}"
    );
    assert!(
        !keys.contains(&"object:requirement:00000000-0000-0000-0000-00000000000c"),
        "oversized object must be dropped: {keys:?}"
    );
    // 逐对象记账：被裁对象的键进 sections_dropped（不是只记总字符）。
    assert!(
        context
            .sections_dropped
            .contains(&"object:requirement:00000000-0000-0000-0000-00000000000c".to_string()),
        "dropped object key must be in sections_dropped: {:?}",
        context.sections_dropped
    );
    // 对象节总字符 ≤ 对象预算；总注入量 ≤ chat 总预算。
    let object_chars: usize = context
        .sections
        .iter()
        .filter(|s| s.key.starts_with("object:"))
        .map(|s| s.chars)
        .sum();
    assert!(object_chars <= CHAT_CAP_OBJECT_CONTEXT);
    assert!(context.chars_total <= CHAT_TOTAL_BUDGET_CHARS);
}

/// 无对象条目 = 行为与 B2 前一致（空对象切片不产生任何对象节）。
#[sqlx::test(migrations = "../../migrations")]
async fn empty_object_slice_keeps_legacy_behavior(pool: PgPool) {
    let project_id = seed_project(&pool, "CX2-EMPTY", "no objects").await;

    let base = build_chat_context(&pool, SYSTEM_TENANT, project_id, Some("repo-cx2"))
        .await
        .unwrap();
    let extended =
        build_chat_context_with_objects(&pool, SYSTEM_TENANT, project_id, Some("repo-cx2"), &[])
            .await
            .unwrap();

    assert_eq!(base.render(), extended.render());
    assert_eq!(base.chars_total, extended.chars_total);
}
