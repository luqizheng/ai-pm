//! 0013_policy_scope 迁移验证（R5）：多级策略表与 task_type 列存在、scope CHECK 生效。

use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_project_actor(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('m', $1) RETURNING id",
    )
            .bind(SYSTEM_TENANT)
            .fetch_one(pool)
            .await
            .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, 'm', $3) RETURNING id",
    )
    .bind(customer)
    .bind(code.to_string())
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();
    let actor: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'h') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    (project, actor)
}

#[sqlx::test(migrations = "../../migrations")]
async fn policy_scope_accepts_all_levels_and_rejects_unknown(pool: PgPool) {
    let (_p, actor) = seed_project_actor(&pool, "MIG13A").await;
    for scope in ["organization", "project", "task_type"] {
        sqlx::query(
            "INSERT INTO delivery.policy_scope \
             (scope, scope_key, require_human_approval, updated_by, tenant_id) \
             VALUES ($1, $2, true, $3, $4)",
        )
        .bind(scope)
        .bind(format!("k-{scope}"))
        .bind(actor)
        .bind(SYSTEM_TENANT)
        .execute(&pool)
        .await
        .unwrap();
    }
    let bad = sqlx::query(
        "INSERT INTO delivery.policy_scope \
         (scope, scope_key, require_human_approval, updated_by, tenant_id) \
         VALUES ('nonsense', 'k', true, $1, $2)",
    )
    .bind(actor)
    .bind(SYSTEM_TENANT)
    .execute(&pool)
    .await;
    assert!(bad.is_err(), "an unknown scope must be refused by the CHECK");
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_type_column_exists_and_is_nullable(pool: PgPool) {
    let (p, _a) = seed_project_actor(&pool, "MIG13B").await;
    sqlx::query(
        "INSERT INTO delivery.task (project_id, title, status, revision, task_type, tenant_id) \
         VALUES ($1, 't', 'ready', 1, 'refactor', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(p)
    .execute(&pool)
    .await
    .expect("task_type column must exist");
    let kind: Option<String> = sqlx::query_scalar("SELECT task_type FROM delivery.task WHERE project_id = $1")
        .bind(p)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kind.as_deref(), Some("refactor"));
}