use sqlx::types::uuid::Uuid;
use sqlx::PgPool;

/// customer → project → principal seed; returns (project_id, principal_id).
async fn seed_project(pool: &PgPool, project_code: &str) -> (Uuid, Uuid) {
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name) VALUES ($1) RETURNING id",
    )
    .bind(format!("customer {project_code}"))
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
        "INSERT INTO core.project (customer_id, code, name) \
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(customer_id)
    .bind(project_code)
    .bind(format!("project {project_code}"))
    .fetch_one(pool)
    .await
    .unwrap();

    (project_id, principal_id)
}

async fn insert_inbound_event(
    pool: &PgPool,
    project_id: Uuid,
    actor_id: Uuid,
    source: &str,
    idempotency_key: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.inbound_event \
         (project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash) \
         VALUES ($1, $2, $3, 'issue_commented', $4, $5, $6) RETURNING id",
    )
    .bind(project_id)
    .bind(actor_id)
    .bind(source)
    .bind(idempotency_key)
    .bind(serde_json::json!({"body": "hello"}))
    .bind(format!("hash-{idempotency_key}"))
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn idempotency_key_unique(pool: sqlx::PgPool) {
    let (project_id, actor_id) = seed_project(&pool, "IDEM-UNIQUE").await;

    let event_id = insert_inbound_event(&pool, project_id, actor_id, "git", "push-001").await;
    let stored: Uuid = sqlx::query_scalar(
        "SELECT id FROM integration.inbound_event \
         WHERE project_id = $1 AND source = 'git' AND idempotency_key = 'push-001'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, event_id);

    // A second insert with the same (project_id, source, idempotency_key)
    // must be rejected by the unique constraint.
    let duplicate = sqlx::query(
        "INSERT INTO integration.inbound_event \
         (project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash) \
         VALUES ($1, $2, 'git', 'issue_commented', 'push-001', $3, 'hash-duplicate')",
    )
    .bind(project_id)
    .bind(actor_id)
    .bind(serde_json::json!({"body": "duplicate"}))
    .execute(&pool)
    .await;
    let err = duplicate.expect_err("duplicate idempotency key must be rejected");
    assert!(
        err.as_database_error()
            .expect("expected a database error")
            .is_unique_violation(),
        "expected unique violation, got: {err}"
    );

    // The same key under a different source is a distinct event and must succeed.
    let other_source_id =
        insert_inbound_event(&pool, project_id, actor_id, "webhook", "push-001").await;
    assert_ne!(other_source_id, event_id);
}

#[sqlx::test(migrations = "../../migrations")]
async fn job_unique_per_type_and_skill_seed(pool: sqlx::PgPool) {
    let (project_id, actor_id) = seed_project(&pool, "JOB-UNIQUE").await;
    let event_id = insert_inbound_event(&pool, project_id, actor_id, "git", "push-002").await;

    let job_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.job (project_id, event_id, job_type) \
         VALUES ($1, $2, 'analyze') RETURNING id",
    )
    .bind(project_id)
    .bind(event_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // A second job of the same (event_id, job_type) must be rejected.
    let duplicate = sqlx::query(
        "INSERT INTO integration.job (project_id, event_id, job_type) \
         VALUES ($1, $2, 'analyze')",
    )
    .bind(project_id)
    .bind(event_id)
    .execute(&pool)
    .await;
    let err = duplicate.expect_err("duplicate (event_id, job_type) must be rejected");
    assert!(
        err.as_database_error()
            .expect("expected a database error")
            .is_unique_violation(),
        "expected unique violation, got: {err}"
    );

    // A different job_type on the same event is allowed.
    let other_type: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.job (project_id, event_id, job_type) \
         VALUES ($1, $2, 'dispatch_notification') RETURNING id",
    )
    .bind(project_id)
    .bind(event_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(other_type, job_id);

    // The platform migration seeds exactly the five V2 skills.
    let skill_count: i64 =
        sqlx::query_scalar("SELECT count(*)::int8 FROM ai.skill_definition")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(skill_count, 5);

    let mut skill_ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM ai.skill_definition")
            .fetch_all(&pool)
            .await
            .unwrap();
    skill_ids.sort();
    assert_eq!(
        skill_ids,
        vec![
            "architecture_consistency",
            "requirement_analysis",
            "risk_detection",
            "scheduling_orchestration",
            "task_analysis",
        ]
    );
}
