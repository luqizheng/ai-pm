//! B3 集成测试：注入源投影（CX2 注入矩阵 · SQL-07 重读）。
//!
//! 覆盖：approved 修订注入正文 · 未批准只给元数据（不注入草稿正文）·
//! knowledge 注入正文 · task 仅元数据且行版本漂移如实标注 ·
//! 非 attached/active 条目一律不出现。

use pm_application::ai_context_items::load_injection_sources;
use sqlx::PgPool;
use uuid::Uuid;

/// 种子：租户 / 项目 / 真人 / 会话。返回 (pool 内已就绪, tenant, project, conversation)。
async fn seed(pool: &PgPool, code: &str) -> (Uuid, Uuid, Uuid) {
    let tenant_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', '1', 'lmms:1') \
         ON CONFLICT (provider, external_key) DO UPDATE SET display_name = core.tenant.display_name \
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let outcome = pm_application::bootstrap::bootstrap_project(
        pool,
        pm_application::bootstrap::BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("Project {code}"),
            tenant_id,
        },
    )
    .await
    .unwrap();
    let project_id = outcome.project_id;
    let conversation_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.conversation (tenant_id, project_id, principal_id, title) \
         VALUES ($1, $2, $3, 'injection fixture') RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(outcome.ai_principal_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (tenant_id, project_id, conversation_id)
}

/// 直插一条 context item（绕过 API，聚焦投影逻辑）。
async fn insert_item(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    conversation_id: Uuid,
    object_type: &str,
    object_id: Uuid,
    revision_id: Option<Uuid>,
    row_version: Option<i64>,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO ai.conversation_context_item \
         (tenant_id, project_id, conversation_id, object_type, object_id, revision_id, \
          row_version, attachment, lifecycle, source, added_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'attached', 'active', 'page', \
                 (SELECT principal_id FROM ai.conversation WHERE id = $3)) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(conversation_id)
    .bind(object_type)
    .bind(object_id)
    .bind(revision_id)
    .bind(row_version)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn approved_revision_injects_body_draft_is_metadata_only(pool: PgPool) {
    let (tenant_id, project_id, conversation_id) = seed(&pool, "CX2-A").await;

    // 需求 + 两版修订：v1 approved、v2 draft（当前）。
    let requirement: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement (project_id, code, title, tenant_id) \
         VALUES ($1, 'REQ-CX2', 'Injection fixture', $2) RETURNING id",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let approved_rev: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
         (requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, 1, '{\"markdown\": \"## Approved body\\n被批准的验收口径。\"}', \
                 'approved', 'hash-a', $3, $4) RETURNING id",
    )
    .bind(requirement)
    .bind(project_id)
    .bind(
        sqlx::query_scalar::<_, Uuid>("SELECT principal_id FROM ai.conversation WHERE id = $1")
            .bind(conversation_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let draft_rev: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
         (requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, 2, '{\"markdown\": \"## Draft body\\n草稿正文不得注入。\"}', \
                 'draft', 'hash-b', $3, $4) RETURNING id",
    )
    .bind(requirement)
    .bind(project_id)
    .bind(
        sqlx::query_scalar::<_, Uuid>("SELECT principal_id FROM ai.conversation WHERE id = $1")
            .bind(conversation_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 条目钉 approved 修订 → 注入正文。
    insert_item(
        &pool,
        tenant_id,
        project_id,
        conversation_id,
        "requirement",
        requirement,
        Some(approved_rev),
        None,
    )
    .await;
    let sources = load_injection_sources(&pool, tenant_id, project_id, conversation_id)
        .await
        .unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(
        sources[0].section_key,
        format!("object:requirement:{requirement}")
    );
    assert!(
        sources[0]
            .body
            .as_deref()
            .unwrap_or_default()
            .contains("被批准的验收口径"),
        "approved revision body must be injected"
    );

    // 换钉 draft 修订 → 只给元数据（草稿正文不得注入）。
    sqlx::query("UPDATE ai.conversation_context_item SET revision_id = $1")
        .bind(draft_rev)
        .execute(&pool)
        .await
        .unwrap();
    let sources = load_injection_sources(&pool, tenant_id, project_id, conversation_id)
        .await
        .unwrap();
    assert_eq!(sources.len(), 1);
    assert!(sources[0].body.is_none(), "draft body must NOT be injected");
    assert!(
        sources[0].meta.contains("draft"),
        "meta must state the draft status: {}",
        sources[0].meta
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn knowledge_and_task_shapes_and_inactive_filtering(pool: PgPool) {
    let (tenant_id, project_id, conversation_id) = seed(&pool, "CX2-B").await;

    // knowledge：注入正文。
    let knowledge: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.project_knowledge \
         (tenant_id, project_id, domain, content, confirmed_by) \
         VALUES ($1, $2, 'identity', 'Confirmed identity fact.', $3) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(
        sqlx::query_scalar::<_, Uuid>("SELECT principal_id FROM ai.conversation WHERE id = $1")
            .bind(conversation_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    insert_item(
        &pool,
        tenant_id,
        project_id,
        conversation_id,
        "knowledge",
        knowledge,
        None,
        None,
    )
    .await;

    // task：仅元数据；行版本漂移标注。
    let task: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id) \
         VALUES ($1, 'fixture task', 'ready', 4, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    insert_item(
        &pool,
        tenant_id,
        project_id,
        conversation_id,
        "task",
        task,
        None,
        Some(2), // 快照停在 2，实际已到 4 → 漂移
    )
    .await;

    // inactive 条目：不得出现。
    let inactive_req: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement (project_id, code, title, tenant_id) \
         VALUES ($1, 'REQ-OFF', 'Inactive fixture', $2) RETURNING id",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let inactive_item = insert_item(
        &pool,
        tenant_id,
        project_id,
        conversation_id,
        "requirement",
        inactive_req,
        None,
        None,
    )
    .await;
    sqlx::query("UPDATE ai.conversation_context_item SET lifecycle = 'inactive' WHERE id = $1")
        .bind(inactive_item)
        .execute(&pool)
        .await
        .unwrap();

    let sources = load_injection_sources(&pool, tenant_id, project_id, conversation_id)
        .await
        .unwrap();
    assert_eq!(sources.len(), 2, "inactive item must be filtered out");

    let knowledge_source = sources
        .iter()
        .find(|s| s.section_key.starts_with("object:knowledge:"))
        .expect("knowledge source");
    assert!(
        knowledge_source
            .body
            .as_deref()
            .unwrap_or_default()
            .contains("Confirmed identity fact."),
        "knowledge content must be injected"
    );

    let task_source = sources
        .iter()
        .find(|s| s.section_key.starts_with("object:task:"))
        .expect("task source");
    assert!(task_source.body.is_none(), "task injects metadata only");
    assert!(
        task_source.meta.contains("row version advanced"),
        "row-version drift must be stated honestly: {}",
        task_source.meta
    );
}
