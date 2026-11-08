//! 0061_ai_executor_capability 迁移验证（V2.0-D Capability Registry V1 DDL 断言，
//! 照 migration_0048.rs 样式；真实数据库，不 Mock）。
//!
//! 断言面：① 表与列齐；② source CHECK 两值（'agent_definition'/'manual'）；
//! ③ UNIQUE (tenant_id, principal_id, capability)（含 revoked 行也占位——
//! 重复键必须显式复活既有行而非另插一行）；④ 活跃部分索引
//! idx_ai_executor_capability_active 存在（partial，(tenant_id, capability)
//! WHERE revoked_at IS NULL）；⑤ DEFAULT source='agent_definition'。

use sqlx::PgPool;
use uuid::Uuid;

async fn seed_tenant(pool: &PgPool, tag: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("cap61-{tag}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_agent(pool: &PgPool, tag: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('dev_agent', $1, true) RETURNING id",
    )
    .bind(format!("cap61-agent-{tag}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn executor_capability_columns_checks_and_partial_index(pool: PgPool) {
    let tenant = seed_tenant(&pool, "a").await;
    let agent = seed_agent(&pool, "a").await;

    // ① 列存在性（SELECT 直查即断言）。
    let row = sqlx::query(
        "SELECT id, tenant_id, principal_id, capability, source, granted_at, \
                revoked_at, created_at, updated_at \
           FROM ai.ai_executor_capability LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("ai_executor_capability must carry the 0061 columns");
    assert_eq!(row.len(), 0);

    // ⑤ source 默认 'agent_definition'。
    let row: (Uuid, String) = sqlx::query_as(
        "INSERT INTO ai.ai_executor_capability (tenant_id, principal_id, capability) \
         VALUES ($1, $2, 'developer') RETURNING id, source",
    )
    .bind(tenant)
    .bind(agent)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row.1, "agent_definition",
        "default source must be agent_definition"
    );
    let _ = row.0;

    // ② 非法 source 拒绝。
    let other = seed_agent(&pool, "b").await;
    let bad = sqlx::query(
        "INSERT INTO ai.ai_executor_capability (tenant_id, principal_id, capability, source) \
         VALUES ($1, $2, 'tester', 'no_such_source')",
    )
    .bind(tenant)
    .bind(other)
    .execute(&pool)
    .await;
    assert!(bad.is_err(), "unknown source must violate CHECK");

    // ③ UNIQUE (tenant_id, principal_id, capability)：重复键拒绝（含 revoked
    // 行占位——重新授予须 UPDATE 复活既有行，语义同 0040 approver_grant）。
    let dup = sqlx::query(
        "INSERT INTO ai.ai_executor_capability (tenant_id, principal_id, capability) \
         VALUES ($1, $2, 'developer')",
    )
    .bind(tenant)
    .bind(agent)
    .execute(&pool)
    .await;
    assert!(
        dup.is_err(),
        "duplicate (tenant, principal, capability) must fail"
    );

    // ④ 活跃部分索引存在：(tenant_id, capability) 且 indpred 非空（partial）。
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_index i \
           JOIN pg_class c ON c.oid = i.indrelid \
           JOIN pg_namespace ns ON ns.oid = c.relnamespace \
          WHERE ns.nspname = 'ai' AND c.relname = 'ai_executor_capability' \
            AND i.indpred IS NOT NULL \
            AND (SELECT array_agg(a.attname ORDER BY a.attnum) \
                   FROM pg_attribute a \
                  WHERE a.attrelid = c.oid AND a.attnum = ANY(i.indkey::smallint[])) \
                @> ARRAY['tenant_id'::name, 'capability'::name]",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        n > 0,
        "idx_ai_executor_capability_active partial index must exist"
    );

    // ④' 谓词行为级：revoked 行不在活跃集合（部分索引谓词语义的行为投影——
    // 活跃行唯一可查、撤销后查不到）。
    let tenant2 = seed_tenant(&pool, "c").await;
    let agent2 = seed_agent(&pool, "c").await;
    sqlx::query(
        "INSERT INTO ai.ai_executor_capability (tenant_id, principal_id, capability) \
         VALUES ($1, $2, 'architect')",
    )
    .bind(tenant2)
    .bind(agent2)
    .execute(&pool)
    .await
    .unwrap();
    let active_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.ai_executor_capability \
          WHERE tenant_id = $1 AND capability = 'architect' AND revoked_at IS NULL",
    )
    .bind(tenant2)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active_before, 1);
    sqlx::query(
        "UPDATE ai.ai_executor_capability SET revoked_at = now() \
          WHERE tenant_id = $1 AND principal_id = $2 AND capability = 'architect'",
    )
    .bind(tenant2)
    .bind(agent2)
    .execute(&pool)
    .await
    .unwrap();
    let active_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.ai_executor_capability \
          WHERE tenant_id = $1 AND capability = 'architect' AND revoked_at IS NULL",
    )
    .bind(tenant2)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        active_after, 0,
        "revoked_at IS NOT NULL must leave the active set"
    );

    // FK 兜底：未知 tenant 拒绝。
    let ghost = sqlx::query(
        "INSERT INTO ai.ai_executor_capability (tenant_id, principal_id, capability) \
         VALUES ($1, $2, 'developer')",
    )
    .bind(Uuid::new_v4())
    .bind(agent2)
    .execute(&pool)
    .await;
    assert!(ghost.is_err(), "unknown tenant must violate FK");
}
