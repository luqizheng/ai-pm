//! S05-02 执行上下文注入闭环（批 3）集成测试。
//!
//! 覆盖四条链路：
//! ① **锚点自动生成**：任务绑定需求 → 同事务生成需求文档锚点（`is_required=true`）；
//!    需求投影完成 → 回填该需求已关联任务（覆盖「先绑任务后提交需求」时序）。
//! ② **人工确认**：`update_task(knowledge_refs)` 全量替换（增删 / 改 required / 钉版）。
//! ③ **内容注入**：开关 on 时 `knowledge_sections` 携带章节正文（来自
//!    `ai.document_section_index`）；钉版优先于最新 published。
//! ④ **开关纪律**：off 时契约不含 `knowledge_sections`（逐字节兼容既有形态）。

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::discussion_memory::DiscussionMemoryInjectionMode;
use pm_application::pm_write::{
    link_task_requirement, update_task, KnowledgeRefInput, UpdateTaskRequest,
};
use pm_application::run_context::get_run_context_with_injection;
use pm_application::task_knowledge::{KnowledgeInjectionMode, MAX_SECTION_CHARS};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    task_id: Uuid,
    #[allow(dead_code)] // 播种用，断言不读
    agent_id: Uuid,
    author: Uuid,
}

async fn seed(pool: &PgPool, code: &str) -> Fx {
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
    let agent_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', 'executor') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'author') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, task_type, tenant_id) \
         VALUES ($1, '注入闭环任务', 'ready', 1, $2, 'architecture', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(boot.project_id)
    .bind(agent_id)
    .fetch_one(pool)
    .await
    .unwrap();
    Fx {
        project_id: boot.project_id,
        task_id,
        agent_id,
        author,
    }
}

/// 建需求（approved 修订）+ 其知识文档（含 published 修订与章节索引）。
/// 返回 (requirement_id, revision_id, document_id)。
async fn seed_requirement(pool: &PgPool, fx: &Fx, code: &str) -> (Uuid, Uuid) {
    let requirement_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement (project_id, code, title, lifecycle_status, created_by, tenant_id) \
         VALUES ($1, $2, '需求标题', 'active', $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(fx.project_id)
    .bind(code)
    .bind(fx.author)
    .fetch_one(pool)
    .await
    .unwrap();
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
         (requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, 1, '{\"markdown\":\"# 需求\"}'::jsonb, 'approved', 'hash-1', $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $2)) RETURNING id",
    )
    .bind(requirement_id)
    .bind(fx.project_id)
    .bind(fx.author)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.requirement SET current_revision_id = $2 WHERE id = $1")
        .bind(requirement_id)
        .bind(revision_id)
        .execute(pool)
        .await
        .unwrap();

    (requirement_id, revision_id)
}

/// 建需求的知识文档（身份 + published 修订 + 章节索引）。
/// 文档身份规则：`doc_key = req-{requirement_id}`（投影唯一绑定点规则）。
async fn seed_document(pool: &PgPool, fx: &Fx, requirement_id: Uuid, revisions: &[i32]) -> Uuid {
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.project_document (project_id, doc_key, doc_type, canonical_path, tenant_id) \
         VALUES ($1, $2, 'requirement', 'requirements/req.md', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(fx.project_id)
    .bind(format!("req-{requirement_id}"))
    .fetch_one(pool)
    .await
    .unwrap();

    for &revision_no in revisions {
        let revision_id: Uuid = sqlx::query_scalar(
            "INSERT INTO ai.document_revision \
             (project_id, document_id, revision_no, git_commit_sha, content_hash, status, created_by, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, 'published', $6, \
                     (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
        )
        .bind(fx.project_id)
        .bind(document_id)
        .bind(revision_no)
        .bind(format!("sha-{revision_no}"))
        .bind(format!("hash-{revision_no}"))
        .bind(fx.author)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO ai.document_section_index \
             (tenant_id, project_id, document_revision_id, provider, section_no, level, \
              title, anchor, content, start_line) \
             VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'db-section', \
                     1, 1, $3, $4, $5, 1)",
        )
        .bind(fx.project_id)
        .bind(revision_id)
        .bind(format!("标题 v{revision_no}"))
        .bind(format!("#sec-{revision_no}"))
        .bind(format!("第 {revision_no} 版的章节正文"))
        .execute(pool)
        .await
        .unwrap();
    }
    document_id
}

/// 只改知识锚点的更新请求（`UpdateTaskRequest` 无 Default：其余字段一律不动）。
/// 乐观锁：自动读取当前 `revision`（成功一次 +1）。
async fn knowledge_update(
    pool: &PgPool,
    task_id: Uuid,
    refs: Vec<KnowledgeRefInput>,
) -> UpdateTaskRequest {
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap();
    UpdateTaskRequest {
        revision: Some(revision),
        title: Some("注入闭环任务".to_string()),
        description: None,
        status: Some("ready".to_string()),
        priority: Some("normal".to_string()),
        assignee_id: None,
        goal: None,
        constraints: None,
        deliverables: None,
        acceptance_criteria: None,
        knowledge_refs: Some(refs),
    }
}

/// 锚点内容（任务 → 文档）。
async fn anchors(pool: &PgPool, task_id: Uuid) -> Vec<(Uuid, bool, Option<i32>)> {
    sqlx::query_as(
        "SELECT document_id, is_required, pinned_revision_no \
           FROM delivery.task_knowledge_ref WHERE task_id = $1 ORDER BY document_id",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn linking_a_requirement_auto_creates_the_knowledge_anchor(pool: PgPool) {
    let fx = seed(&pool, "KI-01").await;
    let (requirement_id, revision_id) = seed_requirement(&pool, &fx, "KI-01-R").await;
    let document_id = seed_document(&pool, &fx, requirement_id, &[1, 2]).await;

    // 绑定前：无锚点（表恒空的事实）。
    assert!(anchors(&pool, fx.task_id).await.is_empty());

    let linked = link_task_requirement(
        &pool,
        fx.task_id,
        requirement_id,
        revision_id,
        &AuditActor::principal(fx.author),
    )
    .await
    .unwrap();
    assert!(linked);

    // 绑定后：需求文档成为 required 锚点（未钉版）。
    let rows = anchors(&pool, fx.task_id).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, document_id);
    assert!(rows[0].1, "自动生成的锚点为 required");
    assert_eq!(rows[0].2, None, "自动生成的锚点不钉版");

    // 幂等：重复绑定不产生第二行。
    link_task_requirement(
        &pool,
        fx.task_id,
        requirement_id,
        revision_id,
        &AuditActor::principal(fx.author),
    )
    .await
    .unwrap();
    assert_eq!(anchors(&pool, fx.task_id).await.len(), 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn human_confirmation_replaces_anchors_and_can_pin_a_revision(pool: PgPool) {
    let fx = seed(&pool, "KI-02").await;
    let (req, _rev) = seed_requirement(&pool, &fx, "KI-02-R").await;
    let document_id = seed_document(&pool, &fx, req, &[1, 2]).await;

    // 人工确认：全量替换（增删 + 改 required + 钉版）。
    update_task(
        &pool,
        fx.task_id,
        knowledge_update(
            &pool,
            fx.task_id,
            vec![KnowledgeRefInput {
                document_id,
                is_required: false,
                pinned_revision_no: Some(1),
            }],
        )
        .await,
        &AuditActor::principal(fx.author),
    )
    .await
    .unwrap();

    let rows = anchors(&pool, fx.task_id).await;
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].1, "人工可把锚点降为可选");
    assert_eq!(rows[0].2, Some(1), "人工可钉版修订号");

    // 人工移除 → 锚点清空（人工确认是权威）。
    update_task(
        &pool,
        fx.task_id,
        knowledge_update(&pool, fx.task_id, vec![]).await,
        &AuditActor::principal(fx.author),
    )
    .await
    .unwrap();
    assert!(anchors(&pool, fx.task_id).await.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn sections_are_injected_when_enabled_and_absent_when_off(pool: PgPool) {
    let fx = seed(&pool, "KI-03").await;
    let (req, _rev) = seed_requirement(&pool, &fx, "KI-03-R").await;
    let document_id = seed_document(&pool, &fx, req, &[1, 2]).await;

    update_task(
        &pool,
        fx.task_id,
        knowledge_update(
            &pool,
            fx.task_id,
            vec![KnowledgeRefInput {
                document_id,
                is_required: true,
                pinned_revision_no: None,
            }],
        )
        .await,
        &AuditActor::principal(fx.author),
    )
    .await
    .unwrap();

    // 领取一个 run（契约读取需有效租约）。
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'claimed', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(fx.project_id)
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let token = pm_application::run_context::generate_lease_token().unwrap();
    let hash = pm_application::run_context::hash_lease_token(&token);
    sqlx::query(
        "UPDATE delivery.task_run SET lease_token_hash = $2, lease_generation = 1, \
                lease_expires_at = now() + interval '10 minutes' WHERE id = $1",
    )
    .bind(run_id)
    .bind(hash)
    .execute(&pool)
    .await
    .unwrap();

    // ① 开关 off → 契约不含 knowledge_sections。
    let contract = get_run_context_with_injection(
        &pool,
        fx.project_id,
        run_id,
        &token,
        1,
        DiscussionMemoryInjectionMode::Off,
        KnowledgeInjectionMode::Off,
    )
    .await
    .unwrap();
    assert!(contract.knowledge_sections.is_empty());
    let json = serde_json::to_value(&contract).unwrap();
    assert!(
        json.get("knowledge_sections").is_none(),
        "开关 off 时字段不得序列化：{json}"
    );

    // ② 开关 on → 章节正文注入，且取最新 published 修订。
    let contract = get_run_context_with_injection(
        &pool,
        fx.project_id,
        run_id,
        &token,
        1,
        DiscussionMemoryInjectionMode::Off,
        KnowledgeInjectionMode::On,
    )
    .await
    .unwrap();
    assert_eq!(contract.knowledge_sections.len(), 1);
    let section = &contract.knowledge_sections[0];
    assert_eq!(section.document_id, document_id);
    assert_eq!(section.revision_no, 2, "默认取最新 published 修订");
    assert_eq!(section.content, "第 2 版的章节正文");
    assert!(section.is_required);
    assert!(section.content.chars().count() <= MAX_SECTION_CHARS);

    // ③ 人工钉版 → 以钉版为准（可复现）。
    update_task(
        &pool,
        fx.task_id,
        knowledge_update(
            &pool,
            fx.task_id,
            vec![KnowledgeRefInput {
                document_id,
                is_required: true,
                pinned_revision_no: Some(1),
            }],
        )
        .await,
        &AuditActor::principal(fx.author),
    )
    .await
    .unwrap();
    let contract = get_run_context_with_injection(
        &pool,
        fx.project_id,
        run_id,
        &token,
        1,
        DiscussionMemoryInjectionMode::Off,
        KnowledgeInjectionMode::On,
    )
    .await
    .unwrap();
    assert_eq!(contract.knowledge_sections[0].revision_no, 1);
    assert_eq!(contract.knowledge_sections[0].content, "第 1 版的章节正文");
    let anchor = contract
        .knowledge_refs
        .iter()
        .find(|r| r.document_id == document_id)
        .expect("锚点仍在契约中");
    assert_eq!(anchor.revision_no, Some(1), "锚点修订号随钉版");
    assert_eq!(anchor.pinned_revision_no, Some(1));
}

#[sqlx::test(migrations = "../../migrations")]
async fn backfill_covers_the_bind_before_submit_order(pool: PgPool) {
    let fx = seed(&pool, "KI-04").await;

    // 需求先建（**尚无文档**）→ 绑任务（此时无法生成锚点）。
    let (requirement_id, revision_id) = seed_requirement(&pool, &fx, "KI-04-R").await;
    link_task_requirement(
        &pool,
        fx.task_id,
        requirement_id,
        revision_id,
        &AuditActor::principal(fx.author),
    )
    .await
    .unwrap();
    assert!(
        anchors(&pool, fx.task_id).await.is_empty(),
        "文档不存在时不生成锚点"
    );

    // 需求提交（投影）→ 回填锚点。
    let document_id = seed_document(&pool, &fx, requirement_id, &[1]).await;
    let mut tx = pool.begin().await.unwrap();
    let n = pm_application::task_knowledge::backfill_document_anchor(
        &mut tx,
        fx.project_id,
        requirement_id,
        document_id,
    )
    .await
    .unwrap();
    assert_eq!(n, 1, "回填一条锚点");
    tx.commit().await.unwrap();

    let rows = anchors(&pool, fx.task_id).await;
    assert_eq!(rows.len(), 1);
    assert!(rows[0].1);
}
