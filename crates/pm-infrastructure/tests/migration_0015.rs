//! 0015_task_contract 迁移验证（D2 契约，`docs/ai-pm-slice-d2-mcp-context-v1.md` §五）：
//! 契约四列、两张关联表、run 级凭据三列、service principal 种子；约束/外键
//! 与列默认值断言。真实数据库，不 Mock。
//! （迁移重入断言已随 2026-11-09 基线合并退役——0001_baseline.sql 吸收
//! 0001~0065，无迁移链即无重跑场景，见 docs/ai-pm-migration-ledger.md。）

use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

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

async fn seed_task(pool: &PgPool, project_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
         VALUES ($1, $2, 'ready', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn contract_columns_and_tables_exist(pool: PgPool) {
    sqlx::query("SELECT goal, constraints, deliverables, git_branch FROM delivery.task LIMIT 0")
        .fetch_all(&pool)
        .await
        .expect("delivery.task must gain the four contract columns");

    sqlx::query(
        "SELECT lease_token_hash, client_name, client_version FROM delivery.task_run LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("delivery.task_run must gain lease/client identity columns");

    sqlx::query(
        "SELECT id, project_id, task_id, ordinal, text, verification_kind, created_at, updated_at \
           FROM delivery.task_acceptance_criterion LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("delivery.task_acceptance_criterion must exist");

    sqlx::query(
        "SELECT project_id, task_id, document_id, is_required, created_at \
           FROM delivery.task_knowledge_ref LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("delivery.task_knowledge_ref must exist");
}

/// 契约列默认值生效（`constraints={}`、`deliverables=[]`）+ dispatch service
/// principal 种子恰好一条（kind='service'）。
#[sqlx::test(migrations = "../../migrations")]
async fn task_contract_defaults_and_dispatch_seed_present(pool: PgPool) {
    let project_id = seed_project(&pool, "M15-DEFAULTS").await;
    let task_id = seed_task(&pool, project_id, "existing").await;

    let (constraints, deliverables): (serde_json::Value, serde_json::Value) =
        sqlx::query_as("SELECT constraints, deliverables FROM delivery.task WHERE id = $1")
            .bind(task_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(constraints, serde_json::json!({}));
    assert_eq!(deliverables, serde_json::json!([]));

    // 种子不重复插入。
    let service_principals: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.principal WHERE display_name = 'ai-pm-dispatch'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        service_principals, 1,
        "the dispatch service principal is seeded once"
    );

    let service_kind: String = sqlx::query_scalar(
        "SELECT kind FROM core.principal WHERE id = '00000000-0000-0000-0000-000000000001'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(service_kind, "service");
}

#[sqlx::test(migrations = "../../migrations")]
async fn acceptance_criterion_constraints(pool: PgPool) {
    let project_id = seed_project(&pool, "M15-ACC").await;
    let task_id = seed_task(&pool, project_id, "acc").await;

    sqlx::query(
        "INSERT INTO delivery.task_acceptance_criterion \
         (project_id, task_id, ordinal, text, verification_kind, tenant_id) \
         VALUES ($1, $2, 10, 'compare shaders', 'review', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(&pool)
    .await
    .expect("a valid criterion is accepted");

    let duplicate_ordinal = sqlx::query(
        "INSERT INTO delivery.task_acceptance_criterion \
         (project_id, task_id, ordinal, text, tenant_id) \
         VALUES ($1, $2, 10, 'second', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(&pool)
    .await;
    assert!(duplicate_ordinal.is_err(), "ordinal is unique per task");

    let unknown_kind = sqlx::query(
        "INSERT INTO delivery.task_acceptance_criterion \
         (project_id, task_id, ordinal, text, verification_kind, tenant_id) \
         VALUES ($1, $2, 20, 'x', 'magic', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(&pool)
    .await;
    assert!(unknown_kind.is_err(), "verification_kind is constrained");

    let empty_text = sqlx::query(
        "INSERT INTO delivery.task_acceptance_criterion \
         (project_id, task_id, ordinal, text, tenant_id) \
         VALUES ($1, $2, 30, '   ', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(&pool)
    .await;
    assert!(empty_text.is_err(), "blank criterion text is refused");
}

#[sqlx::test(migrations = "../../migrations")]
async fn deleting_task_cascades_criteria_and_refs(pool: PgPool) {
    let project_id = seed_project(&pool, "M15-FK").await;
    let task_id = seed_task(&pool, project_id, "fk").await;
    let actor: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('ai_pm', 'doc') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.project_document (project_id, doc_key, doc_type, canonical_path, tenant_id) \
         VALUES ($1, 'ARCH-1', 'architecture', 'architecture/arch-1.md', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let _ = actor;

    sqlx::query(
        "INSERT INTO delivery.task_acceptance_criterion \
         (project_id, task_id, ordinal, text, tenant_id) \
         VALUES ($1, $2, 1, 'criterion', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_knowledge_ref (project_id, task_id, document_id, tenant_id) \
         VALUES ($1, $2, $3, (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(document_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("DELETE FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();

    let criteria: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_acceptance_criterion")
            .fetch_one(&pool)
            .await
            .unwrap();
    let refs: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.task_knowledge_ref")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(criteria, 0, "criteria cascade with the task");
    assert_eq!(refs, 0, "knowledge refs cascade with the task");
}

#[sqlx::test(migrations = "../../migrations")]
async fn knowledge_ref_rejects_cross_project_document(pool: PgPool) {
    let project_id = seed_project(&pool, "M15-REF-A").await;
    let other_project_id = seed_project(&pool, "M15-REF-B").await;
    let task_id = seed_task(&pool, project_id, "ref").await;
    let foreign_doc: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.project_document (project_id, doc_key, doc_type, canonical_path, tenant_id) \
         VALUES ($1, 'ARCH-1', 'architecture', 'architecture/arch-1.md', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(other_project_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let err = sqlx::query(
        "INSERT INTO delivery.task_knowledge_ref (project_id, task_id, document_id, tenant_id) \
         VALUES ($1, $2, $3, (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(foreign_doc)
    .execute(&pool)
    .await;
    assert!(
        err.is_err(),
        "a document of another project must not be referenced"
    );
}
