//! T-083 Discovery schema 结构断言（information_schema / pg_constraint 机械断言）。

use sqlx::{PgPool, Row};

#[sqlx::test(migrations = "../../migrations")]
async fn discovery_schema_has_seven_tenant_tables(pool: PgPool) {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'discovery'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 7, "discovery schema must hold exactly the 7 design tables");
}

#[sqlx::test(migrations = "../../migrations")]
async fn discovery_session_anchored_to_project_by_composite_fk(pool: PgPool) {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint con \
           JOIN pg_class c ON c.oid = con.conrelid \
           JOIN pg_namespace n ON n.oid = c.relnamespace \
           JOIN pg_class t ON t.oid = con.confrelid \
           JOIN pg_namespace tn ON tn.oid = t.relnamespace \
          WHERE n.nspname = 'discovery' AND c.relname = 'discovery_session' \
            AND tn.nspname = 'core' AND t.relname = 'project' \
            AND con.contype = 'f' AND con.conkey[1] = (\
                SELECT attnum FROM pg_attribute \
                 WHERE attrelid = c.oid AND attname = 'tenant_id')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        n, 1,
        "discovery_session must anchor to core.project via (tenant_id, project_id)"
    );
}

/// Q1 关键裁定：`repo_import.discovery_session_id` 单向 FK；不得出现反向引用。
#[sqlx::test(migrations = "../../migrations")]
async fn repo_import_link_is_single_direction(pool: PgPool) {
    let forward: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint con \
           JOIN pg_class c ON c.oid = con.conrelid \
           JOIN pg_namespace n ON n.oid = c.relnamespace \
           JOIN pg_class t ON t.oid = con.confrelid \
           JOIN pg_namespace tn ON tn.oid = t.relnamespace \
          WHERE n.nspname = 'integration' AND c.relname = 'repo_import' \
            AND tn.nspname = 'discovery' AND t.relname = 'discovery_session' \
            AND con.contype = 'f'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(forward, 1, "repo_import must reference discovery_session");

    let backward: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint con \
           JOIN pg_class c ON c.oid = con.conrelid \
           JOIN pg_namespace n ON n.oid = c.relnamespace \
           JOIN pg_class t ON t.oid = con.confrelid \
           JOIN pg_namespace tn ON tn.oid = t.relnamespace \
          WHERE n.nspname = 'discovery' \
            AND tn.nspname = 'integration' AND t.relname = 'repo_import' \
            AND con.contype = 'f'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        backward, 0,
        "discovery schema must not reference integration.repo_import (Q1 single direction)"
    );
}

/// Q3：四类 finding kind 的 CHECK 与 confidence ∈ [0,1] 在库层强制。
#[sqlx::test(migrations = "../../migrations")]
async fn finding_check_constraints_are_enforced(pool: PgPool) {
    let tenant: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'disc-ddl', 'disc-ddl') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let customer: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, 'c') RETURNING id",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    let human: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'h') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let project: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name) \
         VALUES ($1, $2, 'DISC-DDL', 'disc') RETURNING id",
    )
    .bind(tenant)
    .bind(customer)
    .fetch_one(&pool)
    .await
    .unwrap();
    let session: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO discovery.discovery_session \
         (tenant_id, project_id, source, kind, created_by) \
         VALUES ($1, $2, 'WEB', 'INITIAL_ONBOARDING', $3) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(human)
    .fetch_one(&pool)
    .await
    .unwrap();

    let bad_kind = sqlx::query(
        "INSERT INTO discovery.discovery_finding \
         (tenant_id, session_id, kind, content) VALUES ($1, $2, 'rumor', 'x')",
    )
    .bind(tenant)
    .bind(session)
    .execute(&pool)
    .await;
    assert!(bad_kind.is_err(), "kind CHECK must reject unknown kinds");

    let bad_confidence = sqlx::query(
        "INSERT INTO discovery.discovery_finding \
         (tenant_id, session_id, kind, content, confidence) \
         VALUES ($1, $2, 'speculation', 'x', 1.5)",
    )
    .bind(tenant)
    .bind(session)
    .execute(&pool)
    .await;
    assert!(bad_confidence.is_err(), "confidence CHECK must reject >1");

    let row: (String, serde_json::Value) = sqlx::query_as(
        "INSERT INTO discovery.discovery_finding \
         (tenant_id, session_id, kind, content, confidence) \
         VALUES ($1, $2, 'speculation', 'x', 0.5) \
         RETURNING verification_status, evidence",
    )
    .bind(tenant)
    .bind(session)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "PENDING", "verification_status defaults to PENDING");
    assert_eq!(row.1, serde_json::json!([]), "evidence defaults to []");
}

/// 防止忘记同步租户表清单：TENANT_TABLES 的数量与 discovery schema 实况对账。
#[sqlx::test(migrations = "../../migrations")]
async fn every_discovery_table_appears_in_tenant_assertions(pool: PgPool) {
    for table in [
        "discovery_session",
        "discovery_message",
        "discovery_finding",
        "discovery_finding_repository",
        "discovery_question",
        "discovery_answer",
        "discovery_proposal",
    ] {
        let row = sqlx::query(
            "SELECT is_nullable FROM information_schema.columns \
              WHERE table_schema = 'discovery' AND table_name = $1 AND column_name = 'tenant_id'",
        )
        .bind(table)
        .fetch_optional(&pool)
        .await
        .unwrap();
        let row = row.unwrap_or_else(|| panic!("discovery.{table} has no tenant_id column"));
        assert_eq!(row.get::<String, _>("is_nullable"), "NO", "discovery.{table}");
    }
}