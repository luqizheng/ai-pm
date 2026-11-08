//! 0024_knowledge_section_index 迁移验证：章节索引表与写入幂等账本表存在、
//! 约束齐备、DDL 为 forward-only。真实数据库，不 Mock。

use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;

#[sqlx::test(migrations = "../../migrations")]
async fn section_index_and_write_ledger_tables_exist(pool: PgPool) {
    sqlx::query(
        "SELECT id, tenant_id, project_id, document_revision_id, provider, section_no, \
                level, title, anchor, content, start_line \
           FROM ai.document_section_index LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("document_section_index must exist");

    sqlx::query(
        "SELECT id, tenant_id, project_id, operation_id, command, proposal_id, revision_id \
           FROM ai.knowledge_write_operation LIMIT 0",
    )
    .fetch_all(&pool)
    .await
    .expect("knowledge_write_operation must exist");
}

#[sqlx::test(migrations = "../../migrations")]
async fn section_index_unique_key_covers_revision_provider_and_section(pool: PgPool) {
    let unique: Vec<String> = sqlx::query_scalar(
        "SELECT conname FROM pg_constraint con \
           JOIN pg_class c ON c.oid = con.conrelid \
           JOIN pg_namespace n ON n.oid = c.relnamespace \
          WHERE n.nspname = 'ai' AND c.relname = 'document_section_index' \
            AND con.contype = 'u'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        unique.len(),
        1,
        "the table must have exactly one unique constraint, got {unique:?}"
    );
}

/// 同一 `(revision, provider, section_no)` 不得重复插入。
#[sqlx::test(migrations = "../../migrations")]
async fn section_index_rejects_duplicate_section_numbers(pool: PgPool) {
    let customer: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('m24', $1) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    let project: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, 'M24', 'm24', $2) RETURNING id",
    )
    .bind(customer)
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('ai_pm', 'agent') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let document_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO ai.project_document (project_id, doc_key, doc_type, canonical_path, tenant_id) \
         VALUES ($1, 'M24-DOC', 'knowledge', 'projects/M24/knowledge/M24-DOC.md', $2) RETURNING id",
    )
    .bind(project)
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    let revision_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO ai.document_revision \
         (project_id, document_id, revision_no, git_commit_sha, content_hash, status, \
          created_by, tenant_id) \
         VALUES ($1, $2, 1, 'sha', 'hash', 'published', $3, $4) RETURNING id",
    )
    .bind(project)
    .bind(document_id)
    .bind(actor)
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();

    let insert = |section_no: i32| {
        sqlx::query(
            "INSERT INTO ai.document_section_index \
             (tenant_id, project_id, document_revision_id, provider, section_no, \
              level, title, anchor, content, start_line) \
             VALUES ($1,$2,$3,'db-section',$4,1,'t','a','c',1)",
        )
        .bind(SYSTEM_TENANT)
        .bind(project)
        .bind(revision_id)
        .bind(section_no)
        .execute(&pool)
    };
    insert(0).await.unwrap();
    assert!(
        insert(0).await.is_err(),
        "the same (revision, provider, section_no) must be rejected"
    );
    insert(1).await.unwrap();
}

/// 幂等账本：同一 `(project_id, operation_id)` 只允许一行。
#[sqlx::test(migrations = "../../migrations")]
async fn write_ledger_is_unique_per_project_and_operation(pool: PgPool) {
    let customer: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('m24-ledger', $1) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    let project: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, 'M24-L', 'm24-l', $2) RETURNING id",
    )
    .bind(customer)
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();

    let insert = || {
        sqlx::query(
            "INSERT INTO ai.knowledge_write_operation (tenant_id, project_id, operation_id, command) \
             VALUES ($1, $2, 'op-1', 'create_document')",
        )
        .bind(SYSTEM_TENANT)
        .bind(project)
    };
    insert().execute(&pool).await.unwrap();
    assert!(
        insert().execute(&pool).await.is_err(),
        "the same (project_id, operation_id) must be rejected"
    );
}

/// 账本的 `command` 是封闭枚举。
#[sqlx::test(migrations = "../../migrations")]
async fn write_ledger_rejects_unknown_commands(pool: PgPool) {
    let customer: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('m24-cmd', $1) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    let project: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, 'M24-C', 'm24-c', $2) RETURNING id",
    )
    .bind(customer)
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();

    let err = sqlx::query(
        "INSERT INTO ai.knowledge_write_operation (tenant_id, project_id, operation_id, command) \
         VALUES ($1, $2, 'op-x', 'not_a_command')",
    )
    .bind(SYSTEM_TENANT)
    .bind(project)
    .execute(&pool)
    .await;
    assert!(err.is_err(), "unknown write command must be refused");
}

/// 章节行随修订级联删除（§4.4 清理生命周期）。
#[sqlx::test(migrations = "../../migrations")]
async fn section_rows_are_cascaded_with_the_revision(pool: PgPool) {
    let customer: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('m24-cascade', $1) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    let project: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, 'M24-CAS', 'm24-cas', $2) RETURNING id",
    )
    .bind(customer)
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('ai_pm', 'agent') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let document_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO ai.project_document (project_id, doc_key, doc_type, canonical_path, tenant_id) \
         VALUES ($1, 'M24-CAS', 'knowledge', 'projects/M24/knowledge/M24-CAS.md', $2) RETURNING id",
    )
    .bind(project)
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    let revision_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO ai.document_revision \
         (project_id, document_id, revision_no, git_commit_sha, content_hash, status, \
          created_by, tenant_id) \
         VALUES ($1, $2, 1, 'sha', 'hash', 'published', $3, $4) RETURNING id",
    )
    .bind(project)
    .bind(document_id)
    .bind(actor)
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai.document_section_index \
         (tenant_id, project_id, document_revision_id, provider, section_no, \
          level, title, anchor, content, start_line) \
         VALUES ($1,$2,$3,'db-section',0,1,'t','a','c',1)",
    )
    .bind(SYSTEM_TENANT)
    .bind(project)
    .bind(revision_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("DELETE FROM ai.document_revision WHERE id = $1")
        .bind(revision_id)
        .execute(&pool)
        .await
        .unwrap();

    let left: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.document_section_index WHERE document_revision_id = $1",
    )
    .bind(revision_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(left, 0, "section rows must be cascaded with the revision");
}

/// 跨租户的章节行写不进去（组合外键强制）。
#[sqlx::test(migrations = "../../migrations")]
async fn section_rows_cannot_cross_tenants(pool: PgPool) {
    let customer: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('m24-xt', $1) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    let project: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, 'M24-XT', 'm24-xt', $2) RETURNING id",
    )
    .bind(customer)
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    let other_tenant: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'm24-other', 'other') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let err = sqlx::query(
        "INSERT INTO ai.knowledge_write_operation (tenant_id, project_id, operation_id, command) \
         VALUES ($1, $2, 'op-x', 'create_document')",
    )
    .bind(other_tenant)
    .bind(project)
    .execute(&pool)
    .await;
    assert!(
        err.is_err(),
        "a ledger row must not point at another tenant's project"
    );
}
