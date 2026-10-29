//! Analysis-run lifecycle and context building over `ai.analysis_run`
//! (architecture doc §7.2).
//!
//! Controller rulings baked into this module (plan Task 12, 2026-10-29):
//!
//! * `start_run` looks at the event's *latest* run
//!   (`ORDER BY created_at DESC LIMIT 1`):
//!   no run, or latest run `status='failed'` → insert a new
//!   `status='analyzing'` row whose `attempt_count` is the old row's
//!   `attempt_count + 1` (0 when there is no old row), with
//!   `started_at = now()` and `model_name` / `policy_version` /
//!   `config_snapshot` copied from `ai.project_ai_config`
//!   (`config_snapshot` ← `model_config`); `project_id` comes from the
//!   inbound event. A missing config row surfaces as
//!   [`ContextError::Db`] — the orchestrator never reaches it because
//!   the job-claim layer already filters on `enabled`.
//!   Latest run `status IN ('queued','analyzing')` with
//!   `finished_at IS NULL` → reuse that run id (crash recovery), the
//!   `status='analyzing'` UPDATE being idempotent.
//!   Latest run `status='completed'` → [`ContextError::AlreadyDone`].
//! * Concurrency is *not* defended here: `integration.job`'s
//!   `UNIQUE (event_id, job_type)` plus the Task 10 lease guarantee at
//!   most one concurrent analysis job per event, and `ai.analysis_run`
//!   deliberately has no `UNIQUE (event_id)` and takes no lock.
//! * `build` loads `payload["task_ref"]` / `["milestone_ref"]` /
//!   `["requirement_ref"]` (optional UUID strings), each query scoped by
//!   `project_id` — cross-project refs must not leak. A ref that is not
//!   a valid UUID, points at no row, or points at another project's row
//!   is uniformly [`ContextError::NotFound`] (no distinction, so
//!   callers cannot probe foreign ids).
//! * `fail_run` is terminal for its row: retries happen as fresh rows
//!   from `start_run` (ruling above), decoupled from job-level backoff.

use crate::model::AnalysisContext;
use pm_domain::{EventId, ProjectId, RunId};

/// Errors surfaced by [`start_run`] and [`build`].
#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    /// Database failure. Also covers a missing `ai.project_ai_config`
    /// row (`RowNotFound`), which the claim layer already excludes.
    #[error("analysis context database error: {0}")]
    Db(#[from] sqlx::Error),
    /// The inbound event does not exist (both start_run and build).
    #[error("inbound event not found: {0}")]
    EventNotFound(EventId),
    /// A delivery object referenced by the payload does not exist —
    /// or is invisible from this project.
    #[error("referenced delivery object not found: {ref}")]
    NotFound {
        /// The offending ref value exactly as it appeared in the payload.
        r#ref: String,
    },
    /// The event already has a run with `status='completed'`.
    #[error("event already has a completed analysis run: {0}")]
    AlreadyDone(EventId),
}

/// Resolve (or reuse) the analysis run for `event_id`, per the module
/// rulings: fresh `analyzing` row after no run or a failed one
/// (inheriting `attempt_count`), idempotent reuse of an open run
/// (crash recovery), [`ContextError::AlreadyDone`] once completed.
pub async fn start_run(pool: &sqlx::PgPool, event_id: EventId) -> Result<RunId, ContextError> {
    // (id, status, attempt_count, finished_at IS NOT NULL) of the latest
    // run for this event, if any.
    let latest: Option<(RunId, String, i32, bool)> = sqlx::query_as(
        "SELECT id, status, attempt_count, finished_at IS NOT NULL \
         FROM ai.analysis_run \
         WHERE event_id = $1 \
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(event_id)
    .fetch_optional(pool)
    .await?;

    if let Some((run_id, status, _, finished)) = &latest {
        match status.as_str() {
            "completed" => return Err(ContextError::AlreadyDone(event_id)),
            "queued" | "analyzing" if !*finished => {
                // Crash recovery: adopt the still-open run. Idempotent.
                sqlx::query("UPDATE ai.analysis_run SET status = 'analyzing' WHERE id = $1")
                    .bind(run_id)
                    .execute(pool)
                    .await?;
                return Ok(*run_id);
            }
            // 'failed' (or a corrupt open-but-finished row): new row below.
            _ => {}
        }
    }

    // The retry count is inherited from the run being retried; a first
    // attempt starts at 0.
    let attempt_count = latest
        .as_ref()
        .map(|(_, _, previous, _)| previous + 1)
        .unwrap_or(0);

    let project_id: ProjectId =
        sqlx::query_scalar("SELECT project_id FROM integration.inbound_event WHERE id = $1")
            .bind(event_id)
            .fetch_optional(pool)
            .await?
            .ok_or(ContextError::EventNotFound(event_id))?;

    // fetch_one: a missing config row becomes Db(RowNotFound). The claim
    // layer already filtered on `enabled`, so the orchestrator cannot
    // legitimately get here.
    let (model_name, config_snapshot, policy_version): (String, serde_json::Value, i32) =
        sqlx::query_as(
            "SELECT model_name, model_config, policy_version \
             FROM ai.project_ai_config WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_one(pool)
        .await?;

    let run_id = RunId::new_v4();
    sqlx::query(
        "INSERT INTO ai.analysis_run \
         (id, project_id, event_id, status, model_name, policy_version, \
          config_snapshot, attempt_count, started_at) \
         VALUES ($1, $2, $3, 'analyzing', $4, $5, $6, $7, now())",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(event_id)
    .bind(model_name)
    .bind(policy_version)
    .bind(config_snapshot)
    .bind(attempt_count)
    .execute(pool)
    .await?;

    Ok(run_id)
}

/// Assemble the [`AnalysisContext`] for `event_id`: the event's own
/// fields, the project-scoped delivery rows its payload references, and
/// the project's current model config / policy version.
pub async fn build(pool: &sqlx::PgPool, event_id: EventId) -> Result<AnalysisContext, ContextError> {
    let (project_id, event_type, payload): (ProjectId, String, serde_json::Value) =
        sqlx::query_as(
            "SELECT project_id, event_type, payload \
             FROM integration.inbound_event WHERE id = $1",
        )
        .bind(event_id)
        .fetch_optional(pool)
        .await?
        .ok_or(ContextError::EventNotFound(event_id))?;

    let task = load_ref(pool, &payload, project_id, "task_ref", "delivery.task").await?;
    let milestone =
        load_ref(pool, &payload, project_id, "milestone_ref", "delivery.milestone").await?;
    let requirement_revision = load_ref(
        pool,
        &payload,
        project_id,
        "requirement_ref",
        "delivery.requirement_revision",
    )
    .await?;

    // Config is read again here so the context reflects the config at
    // build time, not (only) the one frozen into the run row.
    let (config, policy_version): (serde_json::Value, i32) = sqlx::query_as(
        "SELECT model_config, policy_version \
         FROM ai.project_ai_config WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;

    Ok(AnalysisContext {
        project_id,
        event_id,
        event_type,
        payload,
        task,
        requirement_revision,
        milestone,
        config,
        policy_version,
    })
}

/// Resolve one optional payload ref (`"<key>": "<uuid>"`) to the whole
/// referenced row as JSON, scoped to `project_id`.
///
/// `table` is one of the three literals fixed by this module, never
/// input data. An absent (or JSON-null) key yields `None`; anything
/// else — a non-UUID value, no matching row, or a row in another
/// project — is uniformly [`ContextError::NotFound`] so callers learn
/// nothing about foreign data (anti-probing).
async fn load_ref(
    pool: &sqlx::PgPool,
    payload: &serde_json::Value,
    project_id: ProjectId,
    key: &str,
    table: &str,
) -> Result<Option<serde_json::Value>, ContextError> {
    let Some(raw) = payload.get(key).filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let r#ref = raw
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| raw.to_string());
    let id = uuid::Uuid::parse_str(&r#ref)
        .map_err(|_| ContextError::NotFound { r#ref: r#ref.clone() })?;

    let sql = format!("SELECT to_jsonb(t.*) FROM {table} t WHERE t.id = $1 AND t.project_id = $2");
    sqlx::query_scalar::<_, serde_json::Value>(&sql)
        .bind(id)
        .bind(project_id)
        .fetch_optional(pool)
        .await?
        .ok_or(ContextError::NotFound { r#ref })
        .map(Some)
}

/// Persist the assembled context as the run's `context_snapshot` JSONB.
pub async fn save_snapshot(
    pool: &sqlx::PgPool,
    run_id: RunId,
    ctx: &AnalysisContext,
) -> Result<(), sqlx::Error> {
    // Plain data (uuids, strings, numbers, JSON) — serialization cannot
    // fail; model.rs's round-trip test relies on the same assumption.
    let snapshot = serde_json::to_value(ctx).expect("AnalysisContext serializes");
    sqlx::query("UPDATE ai.analysis_run SET context_snapshot = $2 WHERE id = $1")
        .bind(run_id)
        .bind(snapshot)
        .execute(pool)
        .await
        .map(|_| ())
}

/// Complete the run: terminal `status='completed'` with the model's
/// result and self-review, timestamped.
pub async fn finish_run(
    pool: &sqlx::PgPool,
    run_id: RunId,
    result: &serde_json::Value,
    self_review: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE ai.analysis_run \
         SET status = 'completed', result = $2, self_review = $3, finished_at = now() \
         WHERE id = $1",
    )
    .bind(run_id)
    .bind(result)
    .bind(self_review)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Fail the run: terminal `status='failed'` with a coarse failure code,
/// timestamped. Retries are fresh rows from [`start_run`], decoupled
/// from job-level backoff.
pub async fn fail_run(
    pool: &sqlx::PgPool,
    run_id: RunId,
    failure_code: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE ai.analysis_run \
         SET status = 'failed', failure_code = $2, finished_at = now() \
         WHERE id = $1",
    )
    .bind(run_id)
    .bind(failure_code)
    .execute(pool)
    .await
        .map(|_| ())
}
