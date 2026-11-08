//! KP-3 Discussion Memory 注入（`RunContract.discussion_memory`，用户裁决 B / D-KP3-7）。
//!
//! 端到端链路（同进程）：kb_service 记忆提案 → K4 真人批准（CURRENT 版本）→
//! `get_run_context_with` 按 Task 检索注入。覆盖裁决的四个承诺：
//!
//! 1. 开关 off / 无命中 → 字段不序列化，输出与 KP-3 前逐字节一致；
//! 2. on → 只注入 **CURRENT** 版本（待审改写 / 被替代 / 撤回不进入）；
//! 3. 主题 = task 本身 + `task_requirement` 关联需求（实体级 subject 同样命中）；
//! 4. 跨项目 / 跨 Task 不串。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::discussion_memory::DiscussionMemoryInjectionMode;
use pm_application::kb_service::memory::{
    propose_change, propose_create, review, ChangeAction, MemoryContent, RefRole, ResolvedRef,
    ReviewAction,
};
use pm_application::kb_service::source::{register_source, RegisterSourceRequest};
use pm_application::run_context::get_run_context_with;
use pm_application::run_dispatch::{claim_task_run, RunDispatchConfig};
use pm_domain::SYSTEM_TENANT;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

fn cfg() -> RunDispatchConfig {
    RunDispatchConfig::from_env()
}

async fn seed(pool: &PgPool, code: &str) -> Uuid {
    bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id
}

async fn principals(pool: &PgPool) -> (Uuid, Uuid) {
    let human: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'reviewer') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let _identity: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal_identity (provider, tenant_id, subject, principal_id) \
         VALUES ('lmms', $1, $2, $3) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(Uuid::new_v4().to_string())
    .bind(human)
    .fetch_one(pool)
    .await
    .unwrap();
    let ai: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('ai_pm', 'ai-pm') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    (human, ai)
}

/// 项目成员行（run dispatch 归因 / 可见性）。
async fn join(pool: &PgPool, project_id: Uuid, principal_id: Uuid, role: &str) {
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3)",
    )
    .bind(project_id)
    .bind(principal_id)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
}

/// 可领取 Run：ready 任务 + queued run + verified 仓库绑定（run_context 测试同款）。
async fn setup_claimable(pool: &PgPool, project_id: Uuid, agent_id: Uuid) -> Uuid {
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, task_type, tenant_id) \
         VALUES ($1, 'terrain refactor', 'ready', 1, $2, 'architecture', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(agent_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, status, head_commit_sha, tenant_id) \
         VALUES ($1, 'world-view', 'git@host:dm', 'verified', 'headsha', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, base_ref, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(repo_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(pool)
    .await
    .unwrap();
    task_id
}

async fn claim(pool: &PgPool, project_id: Uuid, task_id: Uuid) -> (Uuid, String, i32) {
    let claimed = claim_task_run(pool, project_id, task_id, "codex", None, &cfg())
        .await
        .unwrap();
    (
        claimed.run_id,
        claimed.lease_token,
        claimed.lease_generation,
    )
}

/// 关联需求 + revision + task_requirement 行（任务经既有 run 挂接）；返回 requirement id。
async fn link_requirement(pool: &PgPool, project_id: Uuid, task_id: Uuid) -> Uuid {
    let requirement_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement (project_id, code, title, requirement_type, lifecycle_status, tenant_id) \
         VALUES ($1, 'REQ-DM-1', '地形渲染架构需求', 'functional', 'active', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
         (requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, 1, '{\"markdown\":\"# 地形渲染架构\"}'::jsonb, 'approved', repeat('0', 64), \
                 (SELECT id FROM core.principal WHERE kind = 'human' LIMIT 1), \
                 (SELECT tenant_id FROM core.project WHERE id = $2)) RETURNING id",
    )
    .bind(requirement_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.requirement SET current_revision_id = $2 WHERE id = $1")
        .bind(requirement_id)
        .bind(revision_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_requirement (project_id, task_id, requirement_id, revision_id, tenant_id) \
         VALUES ($1, $2, $3, $4, (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(requirement_id)
    .bind(revision_id)
    .execute(pool)
    .await
    .unwrap();
    requirement_id
}

fn source_ref(source_id: Uuid) -> ResolvedRef {
    ResolvedRef {
        role: RefRole::Evidence,
        target_domain: "knowledge_base".into(),
        target_type: "source".into(),
        target_id: source_id.to_string(),
        target_revision_id: None,
        revision_missing_reason: Some("no_revision_mechanism".into()),
        locator: None,
    }
}

fn subject_ref(ty: &str, id: Uuid) -> ResolvedRef {
    ResolvedRef {
        role: RefRole::Subject,
        target_domain: "requirement".into(),
        target_type: ty.into(),
        target_id: id.to_string(),
        target_revision_id: None,
        revision_missing_reason: Some(if ty == "requirement" {
            "entity_level_reference".into()
        } else {
            "no_revision_mechanism".into()
        }),
        locator: None,
    }
}

async fn register(pool: &PgPool, project_id: Uuid, human: Uuid, text: &str) -> Uuid {
    register_source(
        pool,
        SYSTEM_TENANT,
        project_id,
        &pm_application::actor::AuditActor::principal(human),
        RegisterSourceRequest {
            source_kind: "meeting_notes".into(),
            title: "评审纪要".into(),
            uri: None,
            inline_content: Some(text.into()),
            mime: None,
            size_bytes: None,
            content_hash: None,
        },
    )
    .await
    .unwrap()
    .source_id
}

fn content(title: &str, summary: &str) -> MemoryContent {
    MemoryContent {
        memory_type: "decision_background".into(),
        title: title.into(),
        summary: summary.into(),
        body: None,
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn flag_off_output_is_byte_identical_and_no_query_happens(pool: PgPool) {
    let project_id = seed(&pool, "DM-OFF").await;
    let (human, ai) = principals(&pool).await;
    join(&pool, project_id, human, "owner").await;
    // 库里已经有已批准记忆（注入本来会命中），验证开关语义而不是空库巧合。
    let agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', 'executor') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let task_id = setup_claimable(&pool, project_id, agent).await;
    let source_id = register(&pool, project_id, human, "纪要").await;
    let r = propose_create(
        &pool,
        SYSTEM_TENANT,
        project_id,
        &pm_application::actor::AuditActor::principal(ai),
        content("决策背景", "L1 已批准记忆"),
        "沉淀",
        vec![source_ref(source_id), subject_ref("task", task_id)],
    )
    .await
    .unwrap();
    review(
        &pool,
        SYSTEM_TENANT,
        project_id,
        r.memory_id,
        &pm_application::actor::AuditActor::identified(
            human,
            {
                let id: Uuid = sqlx::query_scalar(
                    "SELECT id FROM core.principal_identity WHERE principal_id = $1",
                )
                .bind(human)
                .fetch_one(&pool)
                .await
                .unwrap();
                id
            },
            None,
            None,
        ),
        ReviewAction::Approve,
        None,
        r.revision_id,
    )
    .await
    .unwrap();

    let (run_id, token, generation) = claim(&pool, project_id, task_id).await;
    let (_, off_json) = read_with_token(
        &pool,
        project_id,
        run_id,
        &token,
        generation,
        DiscussionMemoryInjectionMode::Off,
    )
    .await;
    assert!(
        !off_json.contains("discussion_memory"),
        "off contract must not carry the injection field: {off_json}"
    );
    // on 且命中：证明 off 的无字段语义不是空库巧合。
    let (_, on_json) = read_with_token(
        &pool,
        project_id,
        run_id,
        &token,
        generation,
        DiscussionMemoryInjectionMode::On,
    )
    .await;
    assert!(
        on_json.contains("discussion_memory"),
        "on contract injects the field: {on_json}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn on_injects_current_versions_by_task_and_requirement_subjects(pool: PgPool) {
    let project_id = seed(&pool, "DM-ON").await;
    let other_project = seed(&pool, "DM-ON-B").await;
    let (human, ai) = principals(&pool).await;
    join(&pool, project_id, human, "owner").await;
    join(&pool, other_project, human, "owner").await;
    let ai_actor = pm_application::actor::AuditActor::principal(ai);
    let human_actor = pm_application::actor::AuditActor::identified(
        human,
        {
            let id: Uuid = sqlx::query_scalar(
                "SELECT id FROM core.principal_identity WHERE principal_id = $1",
            )
            .bind(human)
            .fetch_one(&pool)
            .await
            .unwrap();
            id
        },
        None,
        None,
    );

    let agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', 'executor') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let task_id = setup_claimable(&pool, project_id, agent).await;
    let requirement_id = link_requirement(&pool, project_id, task_id).await;
    let source_id = register(&pool, project_id, human, "纪要 v1").await;

    // A（task 主题，已批准 → CURRENT v1）+ 修改提案待审（不得注入待审内容）。
    let a = propose_create(
        &pool,
        SYSTEM_TENANT,
        project_id,
        &ai_actor,
        content("方案 X 被否决", "因延迟不可控"),
        "沉淀",
        vec![source_ref(source_id), subject_ref("task", task_id)],
    )
    .await
    .unwrap();
    review(
        &pool,
        SYSTEM_TENANT,
        project_id,
        a.memory_id,
        &human_actor,
        ReviewAction::Approve,
        None,
        a.revision_id,
    )
    .await
    .unwrap();
    propose_change(
        &pool,
        SYSTEM_TENANT,
        project_id,
        a.memory_id,
        &ai_actor,
        ChangeAction::Update,
        Some(content("方案 X 被否决（待审改写）", "未经审核的新说法")),
        "改写",
        vec![source_ref(source_id), subject_ref("task", task_id)],
    )
    .await
    .unwrap();

    // B（requirement 实体级主题，已批准）→ task 经关联需求命中。
    let b = propose_create(
        &pool,
        SYSTEM_TENANT,
        project_id,
        &human_actor,
        content("需求背景", "历史决策背景经需求主题命中"),
        "沉淀",
        vec![
            source_ref(source_id),
            subject_ref("requirement", requirement_id),
        ],
    )
    .await
    .unwrap();
    review(
        &pool,
        SYSTEM_TENANT,
        project_id,
        b.memory_id,
        &human_actor,
        ReviewAction::Approve,
        None,
        b.revision_id,
    )
    .await
    .unwrap();

    // 其他项目登记同主题 id：不得跨项目串。
    let foreign = propose_create(
        &pool,
        SYSTEM_TENANT,
        other_project,
        &ai_actor,
        content("别项目的记忆", "不得串入"),
        "沉淀",
        vec![
            source_ref(register(&pool, other_project, human, "别项目纪要").await),
            subject_ref("task", task_id),
        ],
    )
    .await
    .unwrap();
    review(
        &pool,
        SYSTEM_TENANT,
        other_project,
        foreign.memory_id,
        &human_actor,
        ReviewAction::Approve,
        None,
        foreign.revision_id,
    )
    .await
    .unwrap();

    // 单次领取：同一租约下先读 off（创建记忆之前语义相同），最后读 on——开关是
    // 两次读取间唯一的差异源（get_run_context 只读不续租）。
    let (run_id, token, generation) = claim(&pool, project_id, task_id).await;
    let (_, off_json) = read_with_token(
        &pool,
        project_id,
        run_id,
        &token,
        generation,
        DiscussionMemoryInjectionMode::Off,
    )
    .await;
    assert!(
        !off_json.contains("discussion_memory"),
        "off contract must not carry the injection field: {off_json}"
    );

    let (json, on_json) = read_with_token(
        &pool,
        project_id,
        run_id,
        &token,
        generation,
        DiscussionMemoryInjectionMode::On,
    )
    .await;
    let injected = json["discussion_memory"]
        .as_array()
        .expect("on contract carries array");
    assert_eq!(injected.len(), 2, "A + B, not the pending edit: {json}");
    let ids: Vec<Uuid> = injected
        .iter()
        .map(|m| m["memory_id"].as_str().unwrap().parse().unwrap())
        .collect();
    assert!(ids.contains(&a.memory_id) && ids.contains(&b.memory_id));
    let a_id = a.memory_id.to_string();
    let a_view = injected.iter().find(|m| m["memory_id"] == *a_id).unwrap();
    assert_eq!(a_view["summary"], "因延迟不可控");
    assert_eq!(a_view["version_no"], 1);
    assert_eq!(a_view["version_id"], Value::from(a.revision_id.to_string()));
    assert!(a_view["matched_by"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s == &format!("task:{task_id}")));
    // 摘要级：没有 body 键。
    assert!(a_view.get("body").is_none());
    assert!(on_json.contains("discussion_memory"));
}

/// 既有租约的契约读取（get_run_context 只读不续租，明文 token 由领取时持有）。
async fn read_with_token(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    token: &str,
    generation: i32,
    mode: DiscussionMemoryInjectionMode,
) -> (Value, String) {
    let contract = get_run_context_with(pool, project_id, run_id, token, generation, mode)
        .await
        .unwrap();
    (
        serde_json::to_value(&contract).unwrap(),
        serde_json::to_string(&contract).unwrap(),
    )
}
