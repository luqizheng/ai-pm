use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use sqlx::types::uuid::Uuid;
use sqlx::PgPool;

/// customer → project → principal seed; returns (project_id, principal_id).
async fn seed_project(pool: &PgPool, project_code: &str) -> (Uuid, Uuid) {
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("customer {project_code}"))
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();

    let principal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', $1) RETURNING id",
    )
    .bind(format!("owner {project_code}"))
    .fetch_one(pool)
    .await
    .unwrap();

    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(customer_id)
    .bind(project_code)
    .bind(format!("project {project_code}"))
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();

    (project_id, principal_id)
}

async fn insert_requirement(pool: &PgPool, project_id: Uuid, code: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.requirement (project_id, code, title, tenant_id) \
         VALUES ($1, $2, $3, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(code)
    .bind(format!("title {code}"))
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn requirement_revision_roundtrip(pool: sqlx::PgPool) {
    // Build customer → project → requirement, insert revisions v1 and v2.
    let (project_id, author_id) = seed_project(&pool, "REV-ROUNDTRIP").await;
    let requirement_id = insert_requirement(&pool, project_id, "REQ-001").await;

    let contents = [
        (1_i32, json!({"business_goal": "v1 goal"}), "hash-v1"),
        (2_i32, json!({"business_goal": "v2 goal"}), "hash-v2"),
    ];
    let mut revision_ids = Vec::new();
    for (version, content, content_hash) in contents {
        let revision_id: Uuid = sqlx::query_scalar(
            "INSERT INTO delivery.requirement_revision \
             (requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
             VALUES ($1, $2, $3, $4, 'draft', $5, $6, \
                     (SELECT tenant_id FROM core.project WHERE id = $2)) RETURNING id",
        )
        .bind(requirement_id)
        .bind(project_id)
        .bind(version)
        .bind(content)
        .bind(content_hash)
        .bind(author_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        revision_ids.push(revision_id);
    }

    // Inserting a duplicate (requirement_id, version) must fail with a unique violation.
    let duplicate = sqlx::query(
        "INSERT INTO delivery.requirement_revision \
         (requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, 2, $3, 'draft', 'hash-duplicate', $4, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(requirement_id)
    .bind(project_id)
    .bind(json!({"business_goal": "duplicate v2"}))
    .bind(author_id)
    .execute(&pool)
    .await;
    let err = duplicate.expect_err("duplicate revision version must be rejected");
    assert!(
        err.as_database_error()
            .expect("expected a database error")
            .is_unique_violation(),
        "expected unique violation, got: {err}"
    );

    // Point current_revision_id at v2, then read content_hash back through it.
    let revision_v2_id = revision_ids[1];
    sqlx::query("UPDATE delivery.requirement SET current_revision_id = $1 WHERE id = $2")
        .bind(revision_v2_id)
        .bind(requirement_id)
        .execute(&pool)
        .await
        .unwrap();

    let content_hash: String = sqlx::query_scalar(
        "SELECT r.content_hash \
         FROM delivery.requirement req \
         JOIN delivery.requirement_revision r ON r.id = req.current_revision_id \
         WHERE req.id = $1",
    )
    .bind(requirement_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(content_hash, "hash-v2");
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_project_revision_rejected(pool: sqlx::PgPool) {
    // Two projects; a revision for A's requirement carrying B's project_id
    // must be rejected by the composite foreign key.
    let (project_a_id, author_id) = seed_project(&pool, "CROSS-A").await;
    let (project_b_id, _) = seed_project(&pool, "CROSS-B").await;

    let requirement_id = insert_requirement(&pool, project_a_id, "REQ-A1").await;

    let cross_project = sqlx::query(
        "INSERT INTO delivery.requirement_revision \
         (requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, 1, $3, 'draft', 'hash-cross', $4, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(requirement_id)
    .bind(project_b_id)
    .bind(json!({"business_goal": "cross project"}))
    .bind(author_id)
    .execute(&pool)
    .await;
    let err = cross_project.expect_err("revision must not point at a foreign project");
    assert!(
        err.as_database_error()
            .expect("expected a database error")
            .is_foreign_key_violation(),
        "expected foreign key violation, got: {err}"
    );
}
