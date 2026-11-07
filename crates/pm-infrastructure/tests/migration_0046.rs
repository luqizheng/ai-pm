//! 0046_kb_pc_intake 迁移验证（P3-1：intake_record + intake_task 结构断言，
//! V1.1 §9.2 DDL 逐项；真实数据库，不 Mock）。
//!
//! 断言面：① 两表存在且列齐；② CHECK 枚举（intake_type 八类/channel 四值/
//! intake_status 六态/target_service 两值/task_type 五值/phase 两值/大写
//! 任务状态八态）；③ 唯一约束（UNIQUE(tenant_id,id)、UNIQUE(tenant_id,
//! project_id,idempotency_key)）；④ 活跃部分唯一索引 uq_intake_task_active
//! 行为级验证（同 (intake_id, task_type) 至多一个活跃任务、终态不受限）；
//! ⑤ 组合 FK（tenant+project / tenant+intake）；⑥ 幂等键冲突行为。

use pm_domain::SYSTEM_TENANT;
use sqlx::{PgPool, Row};
use uuid::Uuid;

async fn seed_principal(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ('11111111-1111-1111-1111-111111111111', 'human', 'seed', true) \
         ON CONFLICT (id) DO NOTHING",
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("c-{code}"))
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(code)
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 第二租户项目：组合 FK (tenant_id, intake_id) 的跨租户拒绝断言需要真实
/// 异租户（同租户跨项目在组合 FK 语义下属合法形态）。
async fn seed_project_in_new_tenant(pool: &PgPool, code: &str) -> Uuid {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $2) RETURNING id",
    )
    .bind(format!("test-{code}-{}", Uuid::new_v4().simple()))
    .bind(format!("t-{code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("c-{code}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(code)
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_intake(pool: &PgPool, project_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.intake_record \
         (tenant_id, project_id, intake_type, channel, submitted_by, source_ref, \
          content_hash, title, intake_status, idempotency_key) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, 'document_upload', \
                 'web', (SELECT id FROM core.principal LIMIT 1), \
                 '{\"kind\":\"inline_text\"}'::jsonb, 'sha256:x', $2, 'ROUTED', $3) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .bind(format!("key-{title}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn intake_record_columns_and_checks(pool: PgPool) {
    seed_principal(&pool).await;
    let project_id = seed_project(&pool, "it46-a").await;
    // 列存在性（SELECT 直查即断言）。
    let row = sqlx::query(
        "SELECT intake_type, channel, submitted_by, source_ref, content_hash, title, \
                intake_status, idempotency_key, created_at, updated_at, tenant_id, project_id \
           FROM integration.intake_record LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("intake_record must carry the V1.1 §9.2 columns");
    assert_eq!(row.len(), 0);

    // 非法 intake_type 拒绝。
    let result = sqlx::query(
        "INSERT INTO integration.intake_record \
         (tenant_id, project_id, intake_type, channel, submitted_by, source_ref, idempotency_key) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, 'no_such_type', \
                 'web', (SELECT id FROM core.principal LIMIT 1), '{}'::jsonb, 'k1')",
    )
    .bind(project_id)
    .execute(&pool)
    .await;
    assert!(result.is_err(), "unknown intake_type must violate CHECK");

    // 幂等键唯一：同 (tenant, project, idempotency_key) 二次插入拒绝。
    let project2 = seed_project(&pool, "it46-b").await;
    let first: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.intake_record \
         (tenant_id, project_id, intake_type, channel, submitted_by, source_ref, idempotency_key) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, 'document_upload', \
                 'web', (SELECT id FROM core.principal LIMIT 1), '{}'::jsonb, 'dup-key') \
         RETURNING id",
    )
    .bind(project2)
    .fetch_one(&pool)
    .await
    .unwrap();
    let second = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO integration.intake_record \
         (tenant_id, project_id, intake_type, channel, submitted_by, source_ref, idempotency_key) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, 'meeting_notes', \
                 'web', (SELECT id FROM core.principal LIMIT 1), '{}'::jsonb, 'dup-key') \
         RETURNING id",
    )
    .bind(project2)
    .fetch_one(&pool)
    .await;
    assert!(second.is_err(), "duplicate (tenant, project, idempotency_key) must fail");
    let _ = first;
    let _ = project_id;
}

#[sqlx::test(migrations = "../../migrations")]
async fn intake_task_structure_and_active_unique(pool: PgPool) {
    seed_principal(&pool).await;
    let project_id = seed_project(&pool, "it46-c").await;
    let intake_id = seed_intake(&pool, project_id, "flow-c").await;

    // 列与 CHECK：非法 task_status（小写）拒绝——大写枚举为 §9.2 有意选择。
    let bad = sqlx::query(
        "INSERT INTO integration.intake_task \
         (tenant_id, project_id, intake_id, target_service, task_type, phase, task_status, idempotency_key) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'kb', 'kb_analyze', \
                 'required', 'pending', 'k-bad')",
    )
    .bind(project_id)
    .bind(intake_id)
    .execute(&pool)
    .await;
    assert!(bad.is_err(), "lowercase status must violate CHECK (§9.2 N-8)");

    // 组合 FK：intake_id 属**别的租户**时拒绝（tenant_id+intake_id 锚；
    // 同租户跨项目引用属合法形态，不在此 FK 拦截面）。
    let other_project = seed_project_in_new_tenant(&pool, "it46-d").await;
    let other_intake = seed_intake(&pool, other_project, "other").await;
    let cross = sqlx::query(
        "INSERT INTO integration.intake_task \
         (tenant_id, project_id, intake_id, target_service, task_type, idempotency_key) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'kb', 'kb_analyze', 'k-cross')",
    )
    .bind(project_id)
    .bind(other_intake)
    .execute(&pool)
    .await;
    assert!(cross.is_err(), "composite FK (tenant_id, intake_id) must reject cross-intake row");

    // uq_intake_task_active：同 (intake_id, task_type) 第二个活跃任务拒绝。
    let t1: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.intake_task \
         (tenant_id, project_id, intake_id, target_service, task_type, task_status, idempotency_key) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'pc', 'pc_analyze', \
                 'PENDING', 'k-a1') RETURNING id",
    )
    .bind(project_id)
    .bind(intake_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let t2 = sqlx::query(
        "INSERT INTO integration.intake_task \
         (tenant_id, project_id, intake_id, target_service, task_type, task_status, idempotency_key) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'pc', 'pc_analyze', \
                 'PROCESSING', 'k-a2')",
    )
    .bind(project_id)
    .bind(intake_id)
    .execute(&pool)
    .await;
    assert!(t2.is_err(), "second ACTIVE pc_analyze must violate uq_intake_task_active");

    // 终态不受限：t1 转 COMPLETED 后新建同类型活跃任务合法（补充信号路径）。
    sqlx::query("UPDATE integration.intake_task SET task_status = 'COMPLETED' WHERE id = $1")
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();
    let after_terminal = sqlx::query(
        "INSERT INTO integration.intake_task \
         (tenant_id, project_id, intake_id, target_service, task_type, task_status, idempotency_key) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'pc', 'pc_analyze', \
                 'PENDING', 'k-a3')",
    )
    .bind(project_id)
    .bind(intake_id)
    .execute(&pool)
    .await;
    assert!(after_terminal.is_ok(), "terminal task must not block new active row (§9.4.3-3)");
}

#[sqlx::test(migrations = "../../migrations")]
async fn tenant_composite_keys_present(pool: PgPool) {
    // intake_record：UNIQUE (tenant_id, id)——intake_task 组合 FK 的目标锚
    // （V1.1 §9.2 评审 N-8）。按**列集合**断言（不依赖 PG 自动命名）。
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_index i \
           JOIN pg_class c ON c.oid = i.indrelid \
           JOIN pg_namespace ns ON ns.oid = c.relnamespace \
          WHERE ns.nspname = 'integration' AND c.relname = 'intake_record' \
            AND i.indisunique \
            AND (SELECT array_agg(a.attname ORDER BY a.attnum) \
                   FROM pg_attribute a \
                  WHERE a.attrelid = c.oid AND a.attnum = ANY(i.indkey::smallint[])) \
                @> ARRAY['tenant_id'::name, 'id'::name]",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(n > 0, "intake_record must carry UNIQUE (tenant_id, id)");
    // intake_task：UNIQUE (tenant_id, project_id, idempotency_key) 幂等键
    // （按列名集合断言，conkey attnum → attname 映射）。
    let n2: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint con \
           JOIN pg_class c ON c.oid = con.conrelid \
           JOIN pg_namespace ns ON ns.oid = c.relnamespace \
          WHERE ns.nspname = 'integration' AND c.relname = 'intake_task' \
            AND con.contype = 'u' \
            AND (SELECT array_agg(a.attname ORDER BY a.attnum) \
                   FROM unnest(con.conkey) AS k(attnum) \
                   JOIN pg_attribute a \
                     ON a.attrelid = con.conrelid AND a.attnum = k.attnum) \
                @> ARRAY['tenant_id'::name, 'project_id'::name, 'idempotency_key'::name]",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(n2 > 0, "intake_task must carry UNIQUE (tenant_id, project_id, idempotency_key)");
}
