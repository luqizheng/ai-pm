//! Task 10 acceptance: lease-based claim / complete / fail, the freeze
//! filter at claim time, and exponential backoff on failure.
//!
//! Holding claims use 3600s leases: with claim-time reclaim, a short lease
//! would let a later assertion run into an expired-lease reclaim of a job
//! another step in the same test still "holds" under load.

use pm_domain::SYSTEM_TENANT;
use pm_infrastructure::job_queue;
use uuid::Uuid;

const JOB_TYPE: &str = "requirement_analysis";

struct Seed {
    project_id: Uuid,
    principal_id: Uuid,
}

/// Minimal bootstrap-equivalent rows: customer, project, AI principal and
/// its ai_lead-enabled config. Bare SQL on purpose — the queue is testable
/// without the bootstrap service.
async fn seed_project(pool: &sqlx::PgPool) -> Seed {
    let customer_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let principal_id = Uuid::new_v4();

    sqlx::query("INSERT INTO core.customer (id, name, tenant_id) VALUES ($1, 'Acme', $2)")
        .bind(customer_id)
        .bind(SYSTEM_TENANT)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.project (id, customer_id, code, name, tenant_id) \
         VALUES ($1, $2, 'P1', 'Queue Project', $3)",
    )
    .bind(project_id)
    .bind(customer_id)
    .bind(SYSTEM_TENANT)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'ai_pm', 'AI PM')",
    )
    .bind(principal_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai.project_ai_config \
         (project_id, ai_principal_id, model_provider, model_name, automation_mode, enabled, tenant_id) \
         VALUES ($1, $2, 'test', 'fake-model', 'ai_lead', true, \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(principal_id)
    .execute(pool)
    .await
    .unwrap();

    Seed {
        project_id,
        principal_id,
    }
}

/// One queued job on a fresh inbound event — `UNIQUE (event_id, job_type)`
/// forbids two jobs of the same type on one event. Returns (job_id, event_id).
async fn seed_job(pool: &sqlx::PgPool, seed: &Seed) -> (Uuid, Uuid) {
    let job_id = Uuid::new_v4();
    let event_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO integration.inbound_event \
         (id, project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash, tenant_id) \
         VALUES ($1, $2, $3, 'test', 'requirement.created', $4, '{}', 'hash', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(event_id)
    .bind(seed.project_id)
    .bind(seed.principal_id)
    .bind(event_id.to_string())
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO integration.job (id, project_id, event_id, job_type, tenant_id) \
         VALUES ($1, $2, $3, $4, (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(job_id)
    .bind(seed.project_id)
    .bind(event_id)
    .bind(JOB_TYPE)
    .execute(pool)
    .await
    .unwrap();

    (job_id, event_id)
}

/// (status, attempts, lease_in_future, available_in_future, lease_owner, last_error)
type JobState = (String, i32, bool, bool, Option<String>, Option<String>);

async fn job_state(pool: &sqlx::PgPool, job_id: Uuid) -> JobState {
    sqlx::query_as::<_, JobState>(
        "SELECT status, attempts, \
                COALESCE(lease_until > now(), false), available_at > now(), \
                lease_owner, last_error \
         FROM integration.job WHERE id = $1",
    )
    .bind(job_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Whether `available_at` lies more than `secs` in the future. Thresholds
/// in the tests leave wide slack because the six suite members share one
/// Postgres and the server-side gap between two statements can stretch
/// under load.
async fn available_at_beyond(pool: &sqlx::PgPool, job_id: Uuid, secs: i64) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT available_at > now() + make_interval(secs => $1::float8) \
         FROM integration.job WHERE id = $2",
    )
    .bind(secs)
    .bind(job_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn claim_hands_out_distinct_jobs(pool: sqlx::PgPool) {
    let seed = seed_project(&pool).await;
    let (job_a, event_a) = seed_job(&pool, &seed).await;
    let (job_b, event_b) = seed_job(&pool, &seed).await;

    let first = job_queue::claim(&pool, JOB_TYPE, "worker-1", 3600)
        .await
        .expect("first claim must get a job");
    let second = job_queue::claim(&pool, JOB_TYPE, "worker-2", 3600)
        .await
        .expect("second claim must get the other job");
    assert_ne!(first.job_id, second.job_id);
    for claimed in [&first, &second] {
        assert!(claimed.job_id == job_a || claimed.job_id == job_b);
        let event_id = if claimed.job_id == job_a {
            event_a
        } else {
            event_b
        };
        assert_eq!(claimed.event_id, event_id);
        assert_eq!(claimed.project_id, seed.project_id);
    }

    assert!(
        job_queue::claim(&pool, JOB_TYPE, "worker-3", 3600)
            .await
            .is_none(),
        "exhausted queue must claim nothing"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn leased_job_is_not_reclaimable_within_lease(pool: sqlx::PgPool) {
    let seed = seed_project(&pool).await;
    let (job_id, _) = seed_job(&pool, &seed).await;

    let claimed = job_queue::claim(&pool, JOB_TYPE, "worker-1", 3600)
        .await
        .expect("claim");
    assert_eq!(claimed.job_id, job_id);

    let (status, attempts, lease_future, _, owner, _) = job_state(&pool, job_id).await;
    assert_eq!(status, "running");
    assert_eq!(attempts, 0);
    assert!(
        lease_future,
        "lease_until must lie in the future after claim"
    );
    assert_eq!(owner.as_deref(), Some("worker-1"));

    assert!(
        job_queue::claim(&pool, JOB_TYPE, "worker-2", 3600)
            .await
            .is_none(),
        "a job inside its lease must not be handed out again"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_expired_lease_reclaimable(pool: sqlx::PgPool) {
    let seed = seed_project(&pool).await;
    let (job_id, _) = seed_job(&pool, &seed).await;

    // Real crash semantics: a worker holds the job (running, live lease)
    // and then dies — the lease runs out while status stays 'running'.
    // Claiming is the reclaim path, so the next claim must hand the job to
    // a new worker (M1 needs no separate reaper process).
    let claimed = job_queue::claim(&pool, JOB_TYPE, "worker-1", 3600)
        .await
        .expect("claim");
    assert_eq!(claimed.job_id, job_id);

    sqlx::query(
        "UPDATE integration.job SET lease_until = now() - interval '5 seconds' WHERE id = $1",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();

    let reclaimed = job_queue::claim(&pool, JOB_TYPE, "worker-2", 3600)
        .await
        .expect("expired lease on a running job must be reclaimable");
    assert_eq!(reclaimed.job_id, job_id);
    let (status, _, lease_future, _, owner, _) = job_state(&pool, job_id).await;
    assert_eq!(status, "running");
    assert!(lease_future);
    assert_eq!(
        owner.as_deref(),
        Some("worker-2"),
        "lease must change hands"
    );

    // Reaper fallback shape: a queued row with a stale lease in the past
    // is equally claimable.
    sqlx::query(
        "UPDATE integration.job \
         SET status = 'queued', lease_until = now() - interval '5 seconds' \
         WHERE id = $1",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();
    let fallback = job_queue::claim(&pool, JOB_TYPE, "worker-3", 3600)
        .await
        .expect("queued row with a stale lease must be claimable");
    assert_eq!(fallback.job_id, job_id);
}

#[sqlx::test(migrations = "../../migrations")]
async fn complete_marks_job_done(pool: sqlx::PgPool) {
    let seed = seed_project(&pool).await;
    let (job_id, _) = seed_job(&pool, &seed).await;

    let claimed = job_queue::claim(&pool, JOB_TYPE, "worker-1", 3600)
        .await
        .expect("claim");
    assert_eq!(claimed.job_id, job_id);
    job_queue::complete(&pool, job_id).await.unwrap();

    let (status, _, lease_future, _, owner, _) = job_state(&pool, job_id).await;
    assert_eq!(status, "completed");
    assert!(!lease_future);
    assert_eq!(owner, None);
    assert!(
        job_queue::claim(&pool, JOB_TYPE, "worker-2", 3600)
            .await
            .is_none(),
        "completed job must not be handed out again"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn fail_backs_off_then_marks_failed(pool: sqlx::PgPool) {
    let seed = seed_project(&pool).await;
    let (job_id, _) = seed_job(&pool, &seed).await;
    let max_attempts = 3;
    let base = 60;

    job_queue::fail(&pool, job_id, "boom-1", max_attempts, base)
        .await
        .unwrap();
    let (status, attempts, _, _, _, last_error) = job_state(&pool, job_id).await;
    assert_eq!(status, "queued", "first failure retries");
    assert_eq!(attempts, 1);
    assert_eq!(last_error.as_deref(), Some("boom-1"));
    // base * 2^1 = 120s; the 30s floor tolerates server-side statement gaps.
    assert!(
        available_at_beyond(&pool, job_id, 30).await,
        "first retry must wait ~120s (well beyond 30s)"
    );
    assert!(!available_at_beyond(&pool, job_id, 150).await);

    job_queue::fail(&pool, job_id, "boom-2", max_attempts, base)
        .await
        .unwrap();
    let (status, attempts, _, _, _, last_error) = job_state(&pool, job_id).await;
    assert_eq!(status, "queued");
    assert_eq!(attempts, 2);
    assert_eq!(last_error.as_deref(), Some("boom-2"));
    // base * 2^2 = 240s — the wait doubles per attempt (beyond the first
    // retry's 120s ceiling of 150s).
    assert!(
        available_at_beyond(&pool, job_id, 150).await,
        "second retry must wait ~240s (beyond the first retry's ceiling)"
    );

    assert!(
        job_queue::claim(&pool, JOB_TYPE, "worker-1", 3600)
            .await
            .is_none(),
        "a job backing off must not be claimable"
    );

    job_queue::fail(&pool, job_id, "boom-3", max_attempts, base)
        .await
        .unwrap();
    let (status, attempts, _, _, owner, last_error) = job_state(&pool, job_id).await;
    assert_eq!(
        status, "failed",
        "third failure is terminal at max_attempts"
    );
    assert_eq!(attempts, 3);
    assert_eq!(last_error.as_deref(), Some("boom-3"));
    assert_eq!(owner, None, "terminal job holds no lease");
}

#[sqlx::test(migrations = "../../migrations")]
async fn controlled_mode_jobs_are_not_claimable(pool: sqlx::PgPool) {
    let seed = seed_project(&pool).await;
    let (job_a, _) = seed_job(&pool, &seed).await;

    // Freeze: controlled mode filters the project out at claim time.
    sqlx::query(
        "UPDATE ai.project_ai_config \
         SET automation_mode = 'controlled' WHERE project_id = $1",
    )
    .bind(seed.project_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        job_queue::claim(&pool, JOB_TYPE, "worker-1", 3600)
            .await
            .is_none(),
        "controlled-mode project must yield no candidates"
    );

    // Unfreeze: the same job becomes claimable again.
    sqlx::query(
        "UPDATE ai.project_ai_config \
         SET automation_mode = 'ai_lead' WHERE project_id = $1",
    )
    .bind(seed.project_id)
    .execute(&pool)
    .await
    .unwrap();
    let claimed = job_queue::claim(&pool, JOB_TYPE, "worker-1", 3600)
        .await
        .expect("ai_lead project must be claimable again");
    assert_eq!(claimed.job_id, job_a);

    // The other half of the freeze filter: a disabled config.
    let (job_b, _) = seed_job(&pool, &seed).await;
    sqlx::query("UPDATE ai.project_ai_config SET enabled = false WHERE project_id = $1")
        .bind(seed.project_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        job_queue::claim(&pool, JOB_TYPE, "worker-2", 3600)
            .await
            .is_none(),
        "disabled config must yield no candidates"
    );
    sqlx::query("UPDATE ai.project_ai_config SET enabled = true WHERE project_id = $1")
        .bind(seed.project_id)
        .execute(&pool)
        .await
        .unwrap();
    let claimed = job_queue::claim(&pool, JOB_TYPE, "worker-2", 3600)
        .await
        .expect("enabled config claims again");
    assert_eq!(claimed.job_id, job_b);
}
