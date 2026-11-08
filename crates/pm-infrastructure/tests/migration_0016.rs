//! 0016_policy_composition 迁移验证：能力类三列与 `additional_checks` 存在、
//! 缺省语义正确、CHECK 拒绝非数组、迁移可重入。真实数据库，不 Mock。

use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "../../migrations")]
async fn policy_scope_gains_the_composition_columns(pool: PgPool) {
    sqlx::query(
        "SELECT auto_accept, auto_merge, auto_publish_knowledge, additional_checks \
           FROM delivery.policy_scope LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("policy_scope must gain the capability columns and additional_checks");
}

#[sqlx::test(migrations = "../../migrations")]
async fn defaults_are_null_capabilities_and_empty_checks(pool: PgPool) {
    let actor: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'owner') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO delivery.policy_scope \
         (scope, scope_key, require_human_approval, updated_by, tenant_id) \
         VALUES ('organization', '*', false, $1, $2)",
    )
    .bind(actor)
    .bind(SYSTEM_TENANT)
    .execute(&pool)
    .await
    .unwrap();

    let (auto_accept, auto_merge, auto_publish_knowledge, additional_checks): (
        Option<bool>,
        Option<bool>,
        Option<bool>,
        serde_json::Value,
    ) = sqlx::query_as(
        "SELECT auto_accept, auto_merge, auto_publish_knowledge, additional_checks \
           FROM delivery.policy_scope WHERE scope = 'organization' AND scope_key = '*'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        auto_accept, None,
        "absent capability column means the layer abstains"
    );
    assert_eq!(auto_merge, None);
    assert_eq!(auto_publish_knowledge, None);
    assert_eq!(additional_checks, serde_json::json!([]));
}

#[sqlx::test(migrations = "../../migrations")]
async fn additional_checks_rejects_non_array_values(pool: PgPool) {
    let actor: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'owner') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO delivery.policy_scope \
         (scope, scope_key, require_human_approval, updated_by, tenant_id) \
         VALUES ('project', 'p', false, $1, $2)",
    )
    .bind(actor)
    .bind(SYSTEM_TENANT)
    .execute(&pool)
    .await
    .unwrap();

    let error = sqlx::query(
        "UPDATE delivery.policy_scope SET additional_checks = '\"not-an-array\"'::jsonb \
          WHERE scope = 'project' AND scope_key = 'p'",
    )
    .execute(&pool)
    .await;
    assert!(
        error.is_err(),
        "additional_checks must stay a JSON array (constraint, not convention)"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn migration_is_reentrant(pool: PgPool) {
    sqlx::raw_sql(include_str!(
        "../../../migrations/0016_policy_composition.sql"
    ))
    .execute(&pool)
    .await
    .expect("re-running 0016 must be idempotent");
}

#[test]
fn migration_file_is_present_and_forward_only() {
    let sql = include_str!("../../../migrations/0016_policy_composition.sql");
    assert!(sql.contains("ADD COLUMN IF NOT EXISTS auto_accept"));
    assert!(sql.contains("ADD COLUMN IF NOT EXISTS auto_merge"));
    assert!(sql.contains("ADD COLUMN IF NOT EXISTS auto_publish_knowledge"));
    assert!(sql.contains("ADD COLUMN IF NOT EXISTS additional_checks"));
    assert!(sql.contains("policy_scope_additional_checks_array"));
    assert!(!sql.contains("DROP COLUMN"));
    assert!(!sql.contains("DROP TABLE"));
}
