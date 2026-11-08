//! Best-effort `ai.llm_call_record` persistence for real (non-fake) model
//! calls (Task 17; migration 0041 §3, DEC-LLM-004.7: failed calls are
//! recorded too, never only the successes).
//!
//! Contract: recording must never take the calling chain down — an insert
//! failure is logged and dropped. Usage/token columns are left NULL (the
//! adapters do not read the chat-completions `usage` object yet; the Phase
//! 6 LLM Gateway owns the fuller schema). Latency is wall-clock around the
//! whole `analyze`/`draft` call.

/// `purpose` values written by this crate. The column is free-form TEXT
/// today (Phase 3 policy resolution will own the vocabulary); the constants
/// keep the worker-side writers from drifting apart from
/// `connection_test` (pm-application::team_llm).
pub const PURPOSE_AI_ANALYSIS: &str = "ai_analysis";
pub const PURPOSE_REPO_IMPORT: &str = "repo_import";
/// Batch G: assistant conversation replies. `analysis_run_id` is always NULL
/// for this purpose (no analysis run backs a chat turn — the subquery in
/// [`record_call`] yields NULL for a NULL event, which the column allows).
pub const PURPOSE_CONVERSATION_REPLY: &str = "conversation_reply";

/// Maps an analysis-chain [`ModelError`] onto the §11 error taxonomy
/// (migration 0041 comment: authentication / connection / timeout /
/// rate_limit / provider_error / model_unavailable / configuration).
/// `Unavailable` folds 5xx and transport failures, so the coarse
/// `model_unavailable` kind — the same word the orchestrator stores on
/// `ai.analysis_run.failure_code` — keeps both observability surfaces
/// aligned; `provider_error` covers unusable upstream output.
pub(crate) fn analysis_error_kind(err: &crate::model::ModelError) -> &'static str {
    match err {
        crate::model::ModelError::Timeout => "timeout",
        crate::model::ModelError::Unavailable(_) => "model_unavailable",
        crate::model::ModelError::BadOutput(_) => "provider_error",
    }
}

/// Same taxonomy for the repo-import chain's [`RepoModelError`]; the enum
/// shape is identical to the analysis-side one.
pub(crate) fn repo_error_kind(
    err: &pm_application::repo_facts::pm_ai_port::ModelError,
) -> &'static str {
    match err {
        pm_application::repo_facts::pm_ai_port::ModelError::Timeout => "timeout",
        pm_application::repo_facts::pm_ai_port::ModelError::Unavailable(_) => "model_unavailable",
        pm_application::repo_facts::pm_ai_port::ModelError::BadOutput(_) => "provider_error",
    }
}

/// Milliseconds elapsed, clamped for a `BIGINT` column (a clock bug must
/// not fail the record write).
pub(crate) fn elapsed_ms(started: std::time::Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX)
}

/// One `ai.llm_call_record` row for a real model call (ok or failed).
/// Best-effort by contract: an insert failure is logged, never propagated.
/// `event_id` (analysis chain only) links the row to the event's current
/// analysis run via subquery — no run row (or a NULL event) simply leaves
/// `analysis_run_id` NULL. Tenant/project/provider FKs come from the
/// resolved provider context, so fake-model calls are never recorded here
/// (they have no provider row).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn record_call(
    pool: &sqlx::PgPool,
    project_id: uuid::Uuid,
    provider_id: uuid::Uuid,
    model: &str,
    purpose: &str,
    status: &str,
    error_kind: Option<&str>,
    error_message: Option<&str>,
    latency_ms: i64,
    event_id: Option<uuid::Uuid>,
) {
    let result = sqlx::query(
        "INSERT INTO ai.llm_call_record \
         (tenant_id, project_id, provider_id, model, purpose, status, \
          error_kind, error_message, latency_ms, analysis_run_id) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, \
                 $3, $4, $5, $6, $7, $8, \
                 (SELECT id FROM ai.analysis_run WHERE event_id = $9 \
                  ORDER BY created_at DESC LIMIT 1))",
    )
    .bind(project_id)
    .bind(provider_id)
    .bind(model)
    .bind(purpose)
    .bind(status)
    .bind(error_kind)
    .bind(error_message)
    .bind(latency_ms)
    .bind(event_id)
    .execute(pool)
    .await;
    if let Err(e) = result {
        tracing::warn!(error = %e, purpose, "llm_call_record insert failed (ignored)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_error_kind_uses_the_seven_kind_taxonomy() {
        assert_eq!(
            analysis_error_kind(&crate::model::ModelError::Timeout),
            "timeout"
        );
        assert_eq!(
            analysis_error_kind(&crate::model::ModelError::Unavailable("boom".to_string())),
            "model_unavailable"
        );
        assert_eq!(
            analysis_error_kind(&crate::model::ModelError::BadOutput("not json".to_string())),
            "provider_error"
        );
    }
}
