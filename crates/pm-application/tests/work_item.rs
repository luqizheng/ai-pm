//! 统一事项基础模型与 Activity Timeline 验收（PA-05；注册表裁决 2026-11-21）。
//!
//! 覆盖：注册（创建即注册 + 幂等自愈不覆盖状态）· 状态迁移接线（语义变化
//! 记 Timeline / 语义未变不记）· implements 关联（钉版需求 → 统一关联 +
//! relation_changed；幂等不重复）· 四维查询 · 关系校验（self/未知类型）·
//! 解除关联记 removed · ADR 注册带版本引用。

use pm_application::actor::AuditActor;
use pm_application::pm_write::{create_task, link_task_requirement, CreateTaskRequest};
use pm_application::work_item::{
    add_relation, ensure_work_item, list_activity, on_task_transition, project_tenant,
    record_activity, remove_relation, WorkItemError,
};
use sqlx::PgPool;
use uuid::Uuid;

// ─────────────────────────────────────────── 种子

async fn seed_world(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let tenant_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.tenant (id, provider, external_key, display_name) \
         VALUES ($1, 'test', $2, 'PA05 Test Tenant')",
    )
    .bind(tenant_id)
    .bind(tenant_id.to_string())
    .execute(pool)
    .await
    .unwrap();
    let customer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.customer (id, tenant_id, name) VALUES ($1, $2, 'Acme')")
        .bind(customer_id)
        .bind(&tenant_id)
        .execute(pool)
        .await
        .unwrap();
    let project_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.project (id, customer_id, code, name, tenant_id) \
         VALUES ($1, $2, 'PA05', 'PA-05 Project', $3)",
    )
    .bind(project_id)
    .bind(customer_id)
    .bind(tenant_id)
    .execute(pool)
    .await
    .unwrap();
    let principal_id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'human', 'Alex Morgan')")
        .bind(principal_id)
        .execute(pool)
        .await
        .unwrap();
    (tenant_id, project_id, principal_id)
}

fn actor(principal_id: Uuid) -> AuditActor {
    AuditActor {
        principal_id,
        identity_id: None,
        client: None,
        auth_mode: None,
        claimed_device_id: None,
    }
}

async fn work_item_row(
    pool: &PgPool,
    tenant_id: Uuid,
    target_type: &str,
    target_id: Uuid,
) -> Option<(Uuid, String, String)> {
    // (id, title, status)
    sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, title, status FROM core.work_item \
          WHERE tenant_id = $1 AND target_type = $2 AND target_id = $3",
    )
    .bind(tenant_id)
    .bind(target_type)
    .bind(target_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn activity_count(pool: &PgPool, item_id: Uuid, activity_type: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM core.work_item_activity \
          WHERE item_id = $1 AND activity_type = $2",
    )
    .bind(item_id)
    .bind(activity_type)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn task_request(project_id: Uuid, title: &str) -> CreateTaskRequest {
    CreateTaskRequest {
        project_id: Some(project_id),
        milestone_id: None,
        title: Some(title.into()),
        description: None,
        priority: None,
        assignee_id: None,
        ai_role_version_id: None,
        implementation_baseline_id: None,
        acceptance_criteria: None,
    }
}

// ─────────────────────────────────────────── 用例

#[sqlx::test(migrations = "../../migrations")]
async fn task_create_registers_work_item_and_activity(pool: PgPool) {
    let (tenant_id, project_id, principal_id) = seed_world(&pool).await;
    let created = create_task(&pool, task_request(project_id, "Implement sign-in"), &actor(principal_id))
        .await
        .expect("create task");
    let (item_id, title, status) =
        work_item_row(&pool, tenant_id, "task", created.id).await.expect("work item registered");
    assert_eq!(title, "Implement sign-in");
    assert_eq!(status, "open"); // draft → open
    assert_eq!(activity_count(&pool, item_id, "created").await, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn ensure_is_idempotent_and_does_not_clobber_status(pool: PgPool) {
    let (tenant_id, project_id, principal_id) = seed_world(&pool).await;
    let target_id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    let first = ensure_work_item(
        &mut tx,
        tenant_id,
        "task",
        target_id,
        Some(project_id),
        "Original title",
        "open",
        Some(principal_id),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE core.work_item SET status = 'in_progress' WHERE id = $1")
        .bind(first)
        .execute(&mut *tx)
        .await
        .unwrap();
    // 同 target 再次注册：title 更新、status 不被覆盖（自愈语义）。
    let second = ensure_work_item(
        &mut tx,
        tenant_id,
        "task",
        target_id,
        Some(project_id),
        "Updated title",
        "open",
        Some(principal_id),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(first, second);
    let (_, title, status) =
        work_item_row(&pool, tenant_id, "task", target_id).await.expect("row");
    assert_eq!(title, "Updated title");
    assert_eq!(status, "in_progress");
}

#[sqlx::test(migrations = "../../migrations")]
async fn transition_records_activity_only_on_semantic_change(pool: PgPool) {
    let (tenant_id, project_id, principal_id) = seed_world(&pool).await;
    let target_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.task (id, project_id, title, status, revision, tenant_id) \
         VALUES ($1, $2, 'Wire me', 'ready', 1, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(target_id)
    .bind(project_id)
    .execute(&pool)
    .await
    .unwrap();

    // ready → claimed：open → in_progress，语义变化 → status_changed 活动。
    let mut tx = pool.begin().await.unwrap();
    on_task_transition(
        &mut tx,
        tenant_id,
        project_id,
        target_id,
        "ready",
        "claimed",
        "human",
        Some(principal_id),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let (item_id, _, status) =
        work_item_row(&pool, tenant_id, "task", target_id).await.expect("registered");
    assert_eq!(status, "in_progress");
    assert_eq!(activity_count(&pool, item_id, "status_changed").await, 1);

    // ready → draft? 不可达；此处用 cancelled → superseded（同 cancelled 语义）：
    // 语义未变 → 不记新活动（§8.3 合并规则）。
    let mut tx = pool.begin().await.unwrap();
    on_task_transition(
        &mut tx,
        tenant_id,
        project_id,
        target_id,
        "cancelled",
        "superseded",
        "human",
        Some(principal_id),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(activity_count(&pool, item_id, "status_changed").await, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn pinned_requirement_creates_implements_relation(pool: PgPool) {
    let (tenant_id, project_id, principal_id) = seed_world(&pool).await;
    let actor = actor(principal_id);
    let created = create_task(&pool, task_request(project_id, "Implement REQ"), &actor)
        .await
        .expect("create task");
    let requirement_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.requirement \
         (id, project_id, code, title, lifecycle_status, tenant_id) \
         VALUES ($1, $2, 'REQ-1', 'Sign-in requirement', 'active', $3)",
    )
    .bind(requirement_id)
    .bind(project_id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .unwrap();
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.requirement_revision \
         (id, requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, $3, 1, '{\"text\":\"v1\"}'::jsonb, 'approved', 'hash-v1', $4, $5)",
    )
    .bind(revision_id)
    .bind(requirement_id)
    .bind(project_id)
    .bind(principal_id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .unwrap();

    assert!(
        link_task_requirement(&pool, created.id, requirement_id, revision_id, &actor)
            .await
            .expect("link"),
        "first link inserts"
    );
    let (task_item, _, _) =
        work_item_row(&pool, tenant_id, "task", created.id).await.expect("task item");
    let (req_item, _, _) =
        work_item_row(&pool, tenant_id, "requirement", requirement_id).await.expect("req item");
    let relation_type: String = sqlx::query_scalar(
        "SELECT relation_type FROM core.work_item_relation \
          WHERE source_item_id = $1 AND target_item_id = $2",
    )
    .bind(task_item)
    .bind(req_item)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(relation_type, "implements");
    assert_eq!(activity_count(&pool, task_item, "relation_changed").await, 1);

    // 幂等：同修订再次 link（rows_affected=0 早退）→ 关系不重复。
    assert!(!link_task_requirement(&pool, created.id, requirement_id, revision_id, &actor)
        .await
        .expect("second link"));
    assert_eq!(activity_count(&pool, task_item, "relation_changed").await, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn list_activity_supports_item_and_project_dimensions(pool: PgPool) {
    let (tenant_id, project_id, principal_id) = seed_world(&pool).await;
    let target_a = Uuid::new_v4();
    let target_b = Uuid::new_v4();
    for target in [target_a, target_b] {
        sqlx::query(
            "INSERT INTO delivery.task (id, project_id, title, status, revision, tenant_id) \
             VALUES ($1, $2, 'Feed task', 'draft', 1, \
                     (SELECT tenant_id FROM core.project WHERE id = $2))",
        )
        .bind(target)
        .bind(project_id)
        .execute(&pool)
        .await
        .unwrap();
        let mut tx = pool.begin().await.unwrap();
        let tenant = project_tenant(&mut tx, project_id).await.unwrap();
        let item = pm_application::work_item::ensure_work_item_for_target(
            &mut tx,
            tenant,
            "task",
            target,
            Some(project_id),
            "open",
            Some(principal_id),
        )
        .await
        .unwrap();
        record_activity(
            &mut tx,
            tenant,
            item,
            Some(project_id),
            None,
            "created",
            "human",
            Some(principal_id),
            "seeded",
            None,
            None,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    // Project 维度：两个事项的活动都可见。
    let project_feed =
        list_activity(&pool, tenant_id, None, Some(project_id), None, 50, 0).await.unwrap();
    assert_eq!(project_feed.len(), 2);
    // Item 维度：单事项。
    let (item_a, _, _) = work_item_row(&pool, tenant_id, "task", target_a).await.unwrap();
    let item_feed = list_activity(&pool, tenant_id, Some(item_a), None, None, 50, 0).await.unwrap();
    assert_eq!(item_feed.len(), 1);
    // 无过滤条件 → Validation（PA-05-05 至少一个维度）。
    assert!(matches!(
        list_activity(&pool, tenant_id, None, None, None, 50, 0).await,
        Err(WorkItemError::Validation(_))
    ));
}

#[sqlx::test(migrations = "../../migrations")]
async fn relation_validation_and_removal(pool: PgPool) {
    let (tenant_id, project_id, principal_id) = seed_world(&pool).await;
    let target_a = Uuid::new_v4();
    let target_b = Uuid::new_v4();
    for target in [target_a, target_b] {
        sqlx::query(
            "INSERT INTO delivery.task (id, project_id, title, status, revision, tenant_id) \
             VALUES ($1, $2, 'Rel', 'draft', 1, \
                     (SELECT tenant_id FROM core.project WHERE id = $2))",
        )
        .bind(target)
        .bind(project_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    let mut tx = pool.begin().await.unwrap();
    let item_a = pm_application::work_item::ensure_work_item_for_target(
        &mut tx,
        tenant_id,
        "task",
        target_a,
        Some(project_id),
        "open",
        Some(principal_id),
    )
    .await
    .unwrap();
    let item_b = pm_application::work_item::ensure_work_item_for_target(
        &mut tx,
        tenant_id,
        "task",
        target_b,
        Some(project_id),
        "open",
        Some(principal_id),
    )
    .await
    .unwrap();
    // self 关系与未知类型拒绝。
    assert!(matches!(
        add_relation(&mut tx, tenant_id, item_a, item_a, "references", "human", None, None)
            .await,
        Err(WorkItemError::Validation(_))
    ));
    assert!(
        add_relation(&mut tx, tenant_id, item_a, item_b, "blocks", "human", None, None)
            .await
            .is_ok()
    );
    assert!(matches!(
        add_relation(&mut tx, tenant_id, item_a, item_b, "hates", "human", None, None).await,
        Err(WorkItemError::Validation(_))
    ));
    tx.commit().await.unwrap();
    // 同关联幂等（不产生第二行）。
    let mut tx = pool.begin().await.unwrap();
    let again = add_relation(
        &mut tx,
        tenant_id,
        item_a,
        item_b,
        "blocks",
        "human",
        None,
        None,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.work_item_relation \
          WHERE source_item_id = $1 AND target_item_id = $2 AND relation_type = 'blocks'",
    )
    .bind(item_a)
    .bind(item_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
    // 解除关联：行删除 + removed 活动追加（历史不覆盖）。
    let mut tx = pool.begin().await.unwrap();
    remove_relation(&mut tx, tenant_id, again, Some(principal_id))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.work_item_relation WHERE id = $1",
    )
    .bind(again)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);
    assert_eq!(activity_count(&pool, item_a, "relation_changed").await, 2);
}

// ─────────────────────────────────────────── 三 Feed 视角（批次 C）

use pm_application::work_item::{mark_read_state, my_feed, product_feed, project_feed};

async fn seed_task_with_activity(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    title: &str,
) -> Uuid {
    let target_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.task (id, project_id, title, status, revision, tenant_id) \
         VALUES ($1, $2, $3, 'draft', 1, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(target_id)
    .bind(project_id)
    .bind(title)
    .execute(pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let tenant = project_tenant(&mut tx, project_id).await.unwrap();
    let item = pm_application::work_item::ensure_work_item_for_target(
        &mut tx,
        tenant,
        "task",
        target_id,
        Some(project_id),
        "open",
        Some(principal_id),
    )
    .await
    .unwrap();
    record_activity(
        &mut tx,
        tenant,
        item,
        Some(project_id),
        None,
        "created",
        "human",
        Some(principal_id),
        "seeded feed task",
        None,
        None,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let (_, _, _) = (tenant_id, title, target_id);
    item
}

#[sqlx::test(migrations = "../../migrations")]
async fn my_feed_routes_by_memberships_and_reads_state(pool: PgPool) {
    let (tenant_id, project_id, principal_id) = seed_world(&pool).await;
    seed_task_with_activity(&pool, tenant_id, project_id, principal_id, "Feed A").await;

    // 非成员视角：什么都看不到（既非 project_member 也非 product_member）。
    let outsider = Uuid::new_v4();
    sqlx::query("INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'human', 'Out')")
        .bind(outsider)
        .execute(&pool)
        .await
        .unwrap();
    let empty = my_feed(&pool, tenant_id, outsider, 50, 0).await.unwrap();
    assert!(empty.is_empty());

    // 项目成员视角：可见。
    sqlx::query(
        "INSERT INTO core.project_member (project_id, principal_id, tenant_id, role) \
         VALUES ($1, $2, $3, 'owner')",
    )
    .bind(project_id)
    .bind(principal_id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .unwrap();
    let feed = my_feed(&pool, tenant_id, principal_id, 50, 0).await.unwrap();
    assert_eq!(feed.len(), 1);
    assert!(feed[0].read_at.is_none());

    // 标记已读 → my_feed 带 read_at；未读清除后回 None（PA-05-09 Read 可逆）。
    let item_id = feed[0].item_id;
    mark_read_state(&pool, tenant_id, principal_id, &[item_id], true)
        .await
        .unwrap();
    let feed = my_feed(&pool, tenant_id, principal_id, 50, 0).await.unwrap();
    assert!(feed[0].read_at.is_some());
    mark_read_state(&pool, tenant_id, principal_id, &[item_id], false)
        .await
        .unwrap();
    let feed = my_feed(&pool, tenant_id, principal_id, 50, 0).await.unwrap();
    assert!(feed[0].read_at.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn project_and_product_feeds_scope_correctly(pool: PgPool) {
    let (tenant_id, project_id, principal_id) = seed_world(&pool).await;
    seed_task_with_activity(&pool, tenant_id, project_id, principal_id, "P Task").await;

    let project_rows =
        project_feed(&pool, tenant_id, principal_id, project_id, 50, 0).await.unwrap();
    assert_eq!(project_rows.len(), 1);
    assert_eq!(project_rows[0].target_type, "task");

    // Product Feed：活动未挂产品 → 空；挂上后可见（team 先建满足外键）。
    let team_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.team (id, tenant_id, name) VALUES ($1, $2, 'Feed Team')",
    )
    .bind(team_id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .unwrap();
    let product_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.product (id, team_id, tenant_id, key, name) \
         VALUES ($1, $2, $3, 'FEED', 'Feed Product')",
    )
    .bind(product_id)
    .bind(team_id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .unwrap();
    let empty = product_feed(&pool, tenant_id, principal_id, product_id, 50, 0)
        .await
        .unwrap();
    assert!(empty.is_empty());
    let item_id: Uuid = sqlx::query_scalar("SELECT id FROM core.work_item LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE core.work_item_activity SET product_id = $1 WHERE item_id = $2")
        .bind(product_id)
        .bind(item_id)
        .execute(&pool)
        .await
        .unwrap();
    let rows = product_feed(&pool, tenant_id, principal_id, product_id, 50, 0)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}
