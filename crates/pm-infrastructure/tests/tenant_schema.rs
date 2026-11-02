//! 多租户结构不变量（T-021b 验收 XT-3 / TN-7 / TN-8）。
//!
//! 用 `information_schema` 机械断言，而不是靠代码评审：44 张租户业务表全部有
//! `tenant_id UUID NOT NULL`，除伪根表 `delivery.policy_scope` 外都至少有一条
//! `(tenant_id, *)` 组合外键（ADR-0001 §3.5 I1）。

use sqlx::{PgPool, Row};

/// ADR-0001 §3.4 + 本设计稿的四类划分：44 张（T-024 新增 `task_dependency`；
/// T-041 新增 `task_repository_commit`；T-043 新增 `review_finding` /
/// `acceptance_criterion_verdict` / `review_approval`）。
const TENANT_TABLES: &[(&str, &str)] = &[
    // A 根表（2）
    ("core", "customer"),
    ("core", "project"),
    // B 直挂 project（38）
    ("core", "project_member"),
    ("delivery", "requirement"),
    ("delivery", "requirement_revision"),
    ("delivery", "milestone"),
    ("delivery", "task"),
    ("delivery", "task_requirement"),
    ("delivery", "task_run"),
    ("delivery", "task_supersession"),
    ("delivery", "task_waiting"),
    ("delivery", "task_policy_override"),
    ("delivery", "task_repository"),
    ("delivery", "task_run_repository"),
    ("delivery", "task_repository_commit"),
    ("delivery", "task_acceptance_criterion"),
    ("delivery", "task_knowledge_ref"),
    ("delivery", "task_dependency"),
    ("delivery", "review_finding"),
    ("delivery", "acceptance_criterion_verdict"),
    ("integration", "inbound_event"),
    ("integration", "job"),
    ("integration", "notification"),
    ("integration", "project_ssh_key"),
    ("integration", "project_repository"),
    ("integration", "repo_import"),
    ("integration", "outbox_event"),
    ("ai", "project_ai_config"),
    ("ai", "project_skill_binding"),
    ("ai", "analysis_run"),
    ("ai", "change_proposal"),
    ("ai", "agent_feedback"),
    ("ai", "project_document"),
    ("ai", "document_revision"),
    ("ai", "document_proposal"),
    ("ai", "document_index_state"),
    ("ai", "knowledge_reference"),
    ("governance", "human_intervention"),
    ("governance", "audit_log"),
    ("governance", "review_approval"),
    // C 锚定非 project 父行（3）
    ("governance", "approval_request"),
    ("delivery", "task_run_failure"),
    ("integration", "repo_import_source"),
    // D 伪根表（1）
    ("delivery", "policy_scope"),
];

/// 唯一允许「只有单列 tenant 外键」的表（scope_key 是文本，没有可锚定的 UUID 父键）。
const PSEUDO_ROOT: (&str, &str) = ("delivery", "policy_scope");

#[sqlx::test(migrations = "../../migrations")]
async fn xt3_every_tenant_table_has_a_not_null_tenant_id(pool: PgPool) {
    for (schema, table) in TENANT_TABLES {
        let row = sqlx::query(
            "SELECT is_nullable, data_type FROM information_schema.columns \
              WHERE table_schema = $1 AND table_name = $2 AND column_name = 'tenant_id'",
        )
        .bind(schema)
        .bind(table)
        .fetch_optional(&pool)
        .await
        .unwrap();
        let row = row.unwrap_or_else(|| panic!("{schema}.{table} has no tenant_id column"));
        assert_eq!(row.get::<String, _>("is_nullable"), "NO", "{schema}.{table}");
        assert_eq!(row.get::<String, _>("data_type"), "uuid", "{schema}.{table}");
    }
}

/// 每张表（伪根表除外）必须至少有一条以 `tenant_id` 打头的组合外键。
#[sqlx::test(migrations = "../../migrations")]
async fn xt3_every_tenant_table_is_anchored_by_a_composite_foreign_key(pool: PgPool) {
    for (schema, table) in TENANT_TABLES {
        if (*schema, *table) == PSEUDO_ROOT {
            continue;
        }
        let composite: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_constraint con \
               JOIN pg_class c ON c.oid = con.conrelid \
               JOIN pg_namespace n ON n.oid = c.relnamespace \
              WHERE n.nspname = $1 AND c.relname = $2 AND con.contype = 'f' \
                AND con.conkey[1] = (\
                    SELECT attnum FROM pg_attribute \
                     WHERE attrelid = c.oid AND attname = 'tenant_id')",
        )
        .bind(schema)
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            composite > 0,
            "{schema}.{table} lacks a composite foreign key led by tenant_id"
        );
    }
}

/// I1 的例外必须真的是例外：`policy_scope` 仍有 tenant 外键（单列）。
#[sqlx::test(migrations = "../../migrations")]
async fn xt3_pseudo_root_still_references_the_tenant(pool: PgPool) {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint con \
           JOIN pg_class c ON c.oid = con.conrelid \
           JOIN pg_namespace n ON n.oid = c.relnamespace \
           JOIN pg_class t ON t.oid = con.confrelid \
           JOIN pg_namespace tn ON tn.oid = t.relnamespace \
          WHERE n.nspname = 'delivery' AND c.relname = 'policy_scope' \
            AND con.contype = 'f' AND tn.nspname = 'core' AND t.relname = 'tenant'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 1, "policy_scope must still have a tenant foreign key");
}

/// TN-7：两个租户各自一行 organization 策略（I4-4 改主键的直接证据）。
#[sqlx::test(migrations = "../../migrations")]
async fn tn7_policy_scope_is_unique_per_tenant(pool: PgPool) {
    let a: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'a', 'a') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let b: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'b', 'b') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'scope-actor') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    for tenant in [a, b] {
        sqlx::query(
            "INSERT INTO delivery.policy_scope \
             (tenant_id, scope, scope_key, require_human_approval, updated_by) \
             VALUES ($1, 'organization', '*', false, $2)",
        )
        .bind(tenant)
        .bind(actor)
        .execute(&pool)
        .await
        .unwrap();
    }
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.policy_scope WHERE scope = 'organization' AND scope_key = '*'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(total, 2, "two tenants may each own an organization policy");
}

/// XT-4 / FK-1 / FK-2 / EX-1：库层隔离不是靠代码纪律。
#[sqlx::test(migrations = "../../migrations")]
async fn xt4_fk_and_executor_columns_are_enforced_by_the_database(pool: PgPool) {
    let tenant_a: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'a', 'a') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let tenant_b: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'b', 'b') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let customer_a: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name, status) VALUES ($1, 'ca', 'active') RETURNING id",
    )
    .bind(tenant_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    let customer_b: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name, status) VALUES ($1, 'cb', 'active') RETURNING id",
    )
    .bind(tenant_b)
    .fetch_one(&pool)
    .await
    .unwrap();

    // XT-4：两个租户各自建同 code 的项目都成功（I4-1）。
    let mut projects = Vec::new();
    for (tenant, customer) in [(tenant_a, customer_a), (tenant_b, customer_b)] {
        let project: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO core.project (tenant_id, customer_id, code, name) \
             VALUES ($1, $2, 'SAME-CODE', 'same') RETURNING id",
        )
        .bind(tenant)
        .bind(customer)
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|e| panic!("two tenants must share a project code: {e}"));
        projects.push(project);
    }

    // FK-2：project 挂到别的租户的客户 → 组合外键拒绝。
    let cross_customer = sqlx::query(
        "INSERT INTO core.project (tenant_id, customer_id, code, name) \
         VALUES ($1, $2, 'CROSS', 'cross')",
    )
    .bind(tenant_a)
    .bind(customer_b)
    .execute(&pool)
    .await;
    assert!(
        cross_customer.is_err(),
        "project must not point at another tenant's customer"
    );

    // FK-1：task 的 tenant_id 与 project 所属租户不一致 → 组合外键拒绝。
    let cross_task = sqlx::query(
        "INSERT INTO delivery.task (tenant_id, project_id, title, status) \
         VALUES ($1, $2, 'cross', 'draft')",
    )
    .bind(tenant_b)
    .bind(projects[0])
    .execute(&pool)
    .await;
    assert!(
        cross_task.is_err(),
        "task tenant_id must match its project's tenant"
    );

    // EX-1：executor_id 可空也可有值（Registry 未建，无外键）。
    // 一个 task 只能有一个 active run（uq_task_run_active），故每次用新 task。
    for executor in [None, Some(uuid::Uuid::new_v4())] {
        let task: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO delivery.task (tenant_id, project_id, title, status) \
             VALUES ($1, $2, 'ok', 'draft') RETURNING id",
        )
        .bind(tenant_a)
        .bind(projects[0])
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO delivery.task_run \
             (tenant_id, project_id, task_id, attempt_no, executor_id) \
             VALUES ($1, $2, $3, 1, $4)",
        )
        .bind(tenant_a)
        .bind(projects[0])
        .bind(task)
        .bind(executor)
        .execute(&pool)
        .await
        .unwrap_or_else(|e| panic!("executor_id {executor:?} must be writable: {e}"));
    }
}

/// TN-8：`idempotency_key` 唯一性按租户划分（I4-3）——跨租户可共存，同租户内重复被拒。
#[sqlx::test(migrations = "../../migrations")]
async fn tn8_outbox_idempotency_key_is_scoped_per_tenant(pool: PgPool) {
    let tenant_a: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'a', 'a') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let tenant_b: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'b', 'b') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let mut projects = Vec::new();
    for (tenant, code) in [(tenant_a, "OUT-A"), (tenant_b, "OUT-B")] {
        let customer: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO core.customer (tenant_id, name) VALUES ($1, $2) RETURNING id",
        )
        .bind(tenant)
        .bind(code)
        .fetch_one(&pool)
        .await
        .unwrap();
        let project: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO core.project (tenant_id, customer_id, code, name) \
             VALUES ($1, $2, $3, $3) RETURNING id",
        )
        .bind(tenant)
        .bind(customer)
        .bind(code)
        .fetch_one(&pool)
        .await
        .unwrap();
        projects.push((tenant, project));
    }

    for (tenant, project) in &projects {
        sqlx::query(
            "INSERT INTO integration.outbox_event \
             (tenant_id, project_id, event_type, aggregate_type, aggregate_id, payload, idempotency_key) \
             VALUES ($1, $2, 'TaskReady', 'task', 't-1', '{}'::jsonb, 'same-key')",
        )
        .bind(tenant)
        .bind(project)
        .execute(&pool)
        .await
        .unwrap_or_else(|e| panic!("cross-tenant same key must succeed: {e}"));
    }

    // 同租户内再次使用同一 key → 唯一约束生效。
    let (tenant, project) = projects[0];
    let duplicate = sqlx::query(
        "INSERT INTO integration.outbox_event \
         (tenant_id, project_id, event_type, aggregate_type, aggregate_id, payload, idempotency_key) \
         VALUES ($1, $2, 'TaskReady', 'task', 't-2', '{}'::jsonb, 'same-key')",
    )
    .bind(tenant)
    .bind(project)
    .execute(&pool)
    .await;
    assert!(
        duplicate.is_err(),
        "same tenant + same key must violate the unique constraint"
    );
}
