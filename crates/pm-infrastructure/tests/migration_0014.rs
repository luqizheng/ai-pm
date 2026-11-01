//! 0014_knowledge_v2 迁移验证：三张新表与 document_revision 补列存在、DDL 为
//! forward-only。真实数据库，不 Mock。

use sqlx::PgPool;

#[sqlx::test(migrations = "../../migrations")]
async fn knowledge_v2_tables_and_columns_exist(pool: PgPool) {
    sqlx::query(
        "SELECT source_event_id, analysis_run_id, redline_confirmed_by, published_at \
           FROM ai.document_revision LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("document_revision must gain source/analysis/redline/published columns");

    sqlx::query(
        "SELECT id, document_id, canonical_path, proposed_markdown, proposed_content_hash, \
                quality_gate_result, status, redline_kind, resulting_revision_id \
           FROM ai.document_proposal LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("document_proposal must exist");

    sqlx::query(
        "SELECT document_revision_id, provider, indexed_content_hash, index_version, status \
           FROM ai.document_index_state LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("document_index_state must exist");

    sqlx::query(
        "SELECT document_id, document_revision_id, section_anchor, reference_kind, source_kind \
           FROM ai.knowledge_reference LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("knowledge_reference must exist");
}

#[sqlx::test(migrations = "../../migrations")]
async fn proposal_status_check_rejects_unknown_value(pool: PgPool) {
    let customer: uuid::Uuid =
        sqlx::query_scalar("INSERT INTO core.customer (name) VALUES ('v2') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let project: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name) VALUES ($1, 'V2-MIG', 'v2') RETURNING id",
    )
    .bind(customer)
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('ai_pm', 'agent') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let err = sqlx::query(
        "INSERT INTO ai.document_proposal \
         (project_id, doc_key, doc_type, canonical_path, proposed_markdown, \
          proposed_content_hash, status, created_by) \
         VALUES ($1, 'REQ-1', 'requirement', 'projects/X/requirement/REQ-1.md', \
                 '# x', 'h', 'bogus', $2)",
    )
    .bind(project)
    .bind(actor)
    .execute(&pool)
    .await;
    assert!(err.is_err(), "unknown proposal status must be refused");
}

#[test]
fn migration_file_is_present_and_forward() {
    let sql = include_str!("../../../migrations/0014_knowledge_v2.sql");
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS ai.document_proposal"));
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS ai.document_index_state"));
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS ai.knowledge_reference"));
    assert!(sql.contains("ADD COLUMN IF NOT EXISTS source_event_id"));
    assert!(!sql.contains("DROP COLUMN"));
}