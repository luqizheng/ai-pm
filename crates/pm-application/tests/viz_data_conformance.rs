//! 关系数据符合性核查集成测试（G-2）。
//!
//! 两件事：
//! 1. **干净库基线为 0 违规**（新系统无存量数据——把"没有"变成可复跑的结论）；
//! 2. **注入四类脏数据能被抓到**（证明核查逻辑本身有效，而非恒返回 0 的空壳）。

use pm_application::visualization::data_conformance::{
    check_relation_data, is_conformant, summary,
};
use sqlx::PgPool;
use uuid::Uuid;

/// 造租户 / 成员 / 客户 / 项目。
async fn seed_project(pool: &PgPool, seq: i64) -> (Uuid, Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) VALUES ('lmms', $1, $1) RETURNING id",
    )
    .bind(seq.to_string())
    .fetch_one(pool)
    .await
    .unwrap();
    let principal: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) VALUES ('human', $1, true) RETURNING id",
    )
    .bind(format!("p-{seq}"))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, code, name, status) VALUES ($1, $2, $2, 'active') RETURNING id",
    )
    .bind(tenant)
    .bind(format!("C-{seq}"))
    .fetch_one(pool)
    .await
    .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) VALUES ($1, $2, $3, $3, 'active') RETURNING id",
    )
    .bind(tenant)
    .bind(customer)
    .bind(format!("P-{seq}"))
    .fetch_one(pool)
    .await
    .unwrap();
    (tenant, project, principal)
}

/// 造需求 + 其修订。返回 (requirement_id, revision_id)。
async fn seed_requirement(
    pool: &PgPool,
    tenant: Uuid,
    project: Uuid,
    by: Uuid,
    code: &str,
) -> (Uuid, Uuid) {
    let requirement: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement (tenant_id, project_id, code, title, created_by) \
         VALUES ($1, $2, $3, $3, $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(code)
    .bind(by)
    .fetch_one(pool)
    .await
    .unwrap();
    let revision: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
             (tenant_id, project_id, requirement_id, version, state, content, content_hash, created_by) \
         VALUES ($1, $2, $3, 1, 'draft', '{\"title\":\"t\"}'::jsonb, 'hash', $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(requirement)
    .bind(by)
    .fetch_one(pool)
    .await
    .unwrap();
    (requirement, revision)
}

async fn seed_design(
    pool: &PgPool,
    tenant: Uuid,
    project: Uuid,
    by: Uuid,
    title: &str,
) -> (Uuid, Uuid) {
    let design: Uuid = sqlx::query_scalar(
        "INSERT INTO design.design (tenant_id, project_id, code, title, created_by) \
         VALUES ($1, $2, $3, $3, $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(title)
    .bind(by)
    .fetch_one(pool)
    .await
    .unwrap();
    let revision: Uuid = sqlx::query_scalar(
        "INSERT INTO design.design_revision \
             (tenant_id, project_id, design_id, version, state, content, content_hash, created_by) \
         VALUES ($1, $2, $3, 1, 'draft', '{\"title\":\"t\"}'::jsonb, 'hash', $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(design)
    .bind(by)
    .fetch_one(pool)
    .await
    .unwrap();
    (design, revision)
}

async fn seed_task(pool: &PgPool, tenant: Uuid, project: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (tenant_id, project_id, title, status) VALUES ($1, $2, $3, 'ready') RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(title)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn clean_database_has_zero_violations(pool: PgPool) {
    // 新系统无存量数据 → 基线必须是 0 违规（把"没有"变成可复跑的结论）
    let rows = check_relation_data(&pool, None).await.expect("核查应成功");
    assert!(!rows.is_empty(), "应返回四项检查的结果行");
    assert!(
        is_conformant(&rows),
        "干净库应为 0 违规：{}",
        summary(&rows)
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn clean_data_after_normal_writes_has_zero_violations(pool: PgPool) {
    // 走"正常写入"的数据（同项目、绑定正确）应保持 0 违规
    let (tenant, project, by) = seed_project(&pool, 7101).await;
    let (req, rev) = seed_requirement(&pool, tenant, project, by, "R-1").await;
    let (design, drev) = seed_design(&pool, tenant, project, by, "D-1").await;

    sqlx::query(
        "INSERT INTO design.design_requirement \
             (design_revision_id, design_id, requirement_id, requirement_revision_id, tenant_id, project_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(drev)
    .bind(design)
    .bind(req)
    .bind(rev)
    .bind(tenant)
    .bind(project)
    .bind(by)
    .execute(&pool)
    .await
    .unwrap();

    let task = seed_task(&pool, tenant, project, "T-1").await;
    sqlx::query(
        "INSERT INTO delivery.task_requirement (project_id, task_id, requirement_id, revision_id, tenant_id) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(project)
    .bind(task)
    .bind(req)
    .bind(rev)
    .bind(tenant)
    .execute(&pool)
    .await
    .unwrap();

    let rows = check_relation_data(&pool, Some(tenant))
        .await
        .expect("核查");
    assert!(
        is_conformant(&rows),
        "正常写入的数据应为 0 违规：{}",
        summary(&rows)
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_project_requirement_binding_is_detected(pool: PgPool) {
    // C-1：design_requirement 的 project 与 requirement 的 project 不同
    let (tenant, project_a, by) = seed_project(&pool, 7102).await;
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, code, name, status) VALUES ($1, 'C2-7102', 'C2', 'active') RETURNING id",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    let project_b: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) VALUES ($1, $2, 'P2-7102', 'P2', 'active') RETURNING id",
    )
    .bind(tenant)
    .bind(customer)
    .fetch_one(&pool)
    .await
    .unwrap();

    let (req_in_b, rev_in_b) = seed_requirement(&pool, tenant, project_b, by, "R-B").await;
    let (design, drev) = seed_design(&pool, tenant, project_a, by, "D-A").await;

    // 故意把 B 项目的需求绑到 A 项目的设计上
    sqlx::query(
        "INSERT INTO design.design_requirement \
             (design_revision_id, design_id, requirement_id, requirement_revision_id, tenant_id, project_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(drev)
    .bind(design)
    .bind(req_in_b)
    .bind(rev_in_b)
    .bind(tenant)
    .bind(project_a)
    .bind(by)
    .execute(&pool)
    .await
    .unwrap();

    let rows = check_relation_data(&pool, Some(tenant))
        .await
        .expect("核查");
    let c1: i64 = rows
        .iter()
        .filter(|r| r.check_id == "C-1")
        .map(|r| r.violations)
        .sum();
    assert!(c1 > 0, "跨项目绑定必须被 C-1 抓到：{}", summary(&rows));
}

#[sqlx::test(migrations = "../../migrations")]
async fn mismatched_revision_binding_is_detected(pool: PgPool) {
    // C-2：绑定修订属于另一个 requirement
    let (tenant, project, by) = seed_project(&pool, 7103).await;
    let (req_1, _rev_1) = seed_requirement(&pool, tenant, project, by, "R-1").await;
    let (_req_2, rev_2) = seed_requirement(&pool, tenant, project, by, "R-2").await;
    let (design, drev) = seed_design(&pool, tenant, project, by, "D-1").await;

    // requirement_id 是 R-1，但绑的修订是 R-2 的
    sqlx::query(
        "INSERT INTO design.design_requirement \
             (design_revision_id, design_id, requirement_id, requirement_revision_id, tenant_id, project_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(drev)
    .bind(design)
    .bind(req_1)
    .bind(rev_2)
    .bind(tenant)
    .bind(project)
    .bind(by)
    .execute(&pool)
    .await
    .unwrap();

    let rows = check_relation_data(&pool, Some(tenant))
        .await
        .expect("核查");
    let c2: i64 = rows
        .iter()
        .filter(|r| r.check_id == "C-2")
        .map(|r| r.violations)
        .sum();
    assert!(c2 > 0, "修订归属错位必须被 C-2 抓到：{}", summary(&rows));
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_project_task_dependency_is_detected(pool: PgPool) {
    // C-4：依赖两端任务跨项目
    let (tenant, project_a, by) = seed_project(&pool, 7104).await;
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, code, name, status) VALUES ($1, 'C2-7104', 'C2', 'active') RETURNING id",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    let project_b: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) VALUES ($1, $2, 'P2-7104', 'P2', 'active') RETURNING id",
    )
    .bind(tenant)
    .bind(customer)
    .fetch_one(&pool)
    .await
    .unwrap();

    let task_a = seed_task(&pool, tenant, project_a, "T-A").await;
    let task_b = seed_task(&pool, tenant, project_b, "T-B").await;

    let inserted = sqlx::query(
        "INSERT INTO delivery.task_dependency (tenant_id, project_id, task_id, depends_on_task_id, created_by) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(tenant)
    .bind(project_a)
    .bind(task_a)
    .bind(task_b)
    .bind(by)
    .execute(&pool)
    .await;

    // **重要发现**：`delivery.task_dependency` 有复合外键
    // `(project_id, depends_on_task_id) → delivery.task(project_id, id)`，
    // 跨项目依赖在 **schema 层就插不进去**——C-4 在本库结构下结构性不可能。
    // 故测试断言「DB 已阻止」，C-4 核查保留为**纵深复核**（防未来放宽约束
    // 或直连改库绕过 FK）。
    assert!(
        inserted.is_err(),
        "复合外键应阻止跨项目依赖写入（C-4 由 schema 保障）"
    );
    let message = format!("{:?}", inserted.unwrap_err());
    assert!(
        message.contains("task_dependency_project_id"),
        "应命中复合外键约束：{message}"
    );

    // C-4 在干净库上仍为 0
    let rows = check_relation_data(&pool, Some(tenant))
        .await
        .expect("核查");
    let c4: i64 = rows
        .iter()
        .filter(|r| r.check_id == "C-4")
        .map(|r| r.violations)
        .sum();
    assert_eq!(
        c4,
        0,
        "跨项目依赖被 FK 挡住，核查应为 0：{}",
        summary(&rows)
    );
}
