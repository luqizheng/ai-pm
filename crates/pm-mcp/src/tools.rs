//! MCP tool function layer (Task 16): three DB-backed functions the rmcp
//! server binds to, each returning a JSON value or a [`ToolError`].
//!
//! - [`submit_technical_design`] reuses the idempotent
//!   `ingestion::submit_event` with the pinned channel identity
//!   (`source = 'mcp'`, `event_type = 'technical_design_submitted'`) and the
//!   payload `{task_ref, fixture, summary}`; the acting principal is the
//!   project's `dev_agent` (MCP callers are dev agents). The task is resolved
//!   up front so a dangling or cross-project `task_ref` is a tool error, not
//!   a deferred pipeline failure.
//! - [`get_submission_status`] aggregates the event status, every proposal
//!   produced by the event's analysis runs (`id`/`status`/`command_type`,
//!   ordered by `proposal_seq`) and the latest analysis run's status.
//! - [`get_task_context`] reuses `read::task_context`; the read service's
//!   project-scoping turns a cross-project task into `NotFound`, which this
//!   layer maps to [`ToolError::NotFound`] — the brief's "MCP 工具错误，非
//!   panic" contract.

use pm_application::ingestion::{IngestionError, SubmitEventRequest, submit_event};
use pm_application::knowledge::{
    self, CreateDocumentRequest, KnowledgeError, KnowledgeRoot,
};
use pm_application::read::{ReadError, task_context};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

/// Channel identity pinned for every event the MCP tools submit.
pub const MCP_SOURCE: &str = "mcp";
/// Event type pinned for technical-design submissions.
pub const SUBMIT_EVENT_TYPE: &str = "technical_design_submitted";

/// Arguments of `pm_submit_technical_design`. The JSON shape is pinned by
/// `tests/tools.rs` (and thus by the generated tool JSON Schema).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubmitTechnicalDesignArgs {
    /// Project the submission belongs to; scopes idempotency and visibility.
    pub project_id: Uuid,
    /// Reference to the task the design belongs to (UUID string of
    /// `delivery.task.id`), matching the pipeline's `payload["task_ref"]`.
    pub task_ref: String,
    /// Caller-chosen idempotency key; the same key with the same payload
    /// replays the original event instead of creating a new one.
    pub idempotency_key: String,
    /// Test-fixture selector forwarded to the analysis pipeline
    /// (`payload["fixture"]`).
    pub fixture: String,
    /// Human-readable summary of the technical design.
    pub summary: String,
}

/// Arguments of `pm_get_submission_status`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubmissionStatusArgs {
    /// The inbound event id `pm_submit_technical_design` returned.
    pub event_id: Uuid,
}

/// Arguments of `pm_get_task_context`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskContextArgs {
    /// Project scope: a task of another project reads as not found.
    pub project_id: Uuid,
    /// The task to read (UUID string of `delivery.task.id`).
    pub task_ref: String,
}

/// Arguments of `pm_submit_document`: creates one Git-backed knowledge document.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubmitDocumentArgs {
    /// Project the document belongs to.
    pub project_id: Uuid,
    /// Stable document key (`[A-Za-z0-9._-]`), unique per project.
    pub doc_key: String,
    /// One of requirement / decision / architecture / knowledge.
    pub doc_type: String,
    /// Authoritative Markdown body (LF).
    pub markdown: String,
    /// Optional human-readable reason for this revision.
    pub change_summary: Option<String>,
}

/// Everything a tool failure can be, kept coarse on purpose: the MCP layer
/// renders only the message text (`isError` result), so the variants exist to
/// make the failure modes explicit and matchable in tests.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

fn parse_task_ref(task_ref: &str) -> Result<Uuid, ToolError> {
    Uuid::parse_str(task_ref)
        .map_err(|_| ToolError::InvalidRequest(format!("task_ref `{task_ref}` is not a UUID string")))
}

/// Submits one technical design: resolves the acting `dev_agent` principal,
/// verifies the referenced task is visible in the project, then reuses the
/// idempotent `submit_event`. Answers `{event_id, processing_status}` where
/// `processing_status` is `"queued"` (fresh event) or `"duplicate"` (idempotent
/// replay).
pub async fn submit_technical_design(
    pool: &PgPool,
    args: SubmitTechnicalDesignArgs,
) -> Result<Value, ToolError> {
    let task_id = parse_task_ref(&args.task_ref)?;

    // Task visibility first: a dangling or cross-project task_ref must fail
    // here with a tool error instead of poisoning the analysis pipeline later.
    task_context(pool, args.project_id, task_id)
        .await
        .map_err(|err| match err {
            ReadError::NotFound => ToolError::NotFound(format!(
                "task {} not found in project {} (or not visible)",
                args.task_ref, args.project_id
            )),
            ReadError::Db(e) => ToolError::Db(e),
        })?;

    // MCP callers are dev agents: act as the project's `dev_agent` principal.
    let actor_id: Uuid = sqlx::query_scalar(
        "SELECT pm.principal_id FROM core.project_member pm \
         JOIN core.principal p ON p.id = pm.principal_id \
         WHERE pm.project_id = $1 AND p.kind = 'dev_agent' \
         ORDER BY pm.principal_id LIMIT 1",
    )
    .bind(args.project_id)
    .fetch_optional(pool)
    .await
    .map_err(ToolError::Db)?
    .ok_or_else(|| {
        ToolError::NotFound(format!(
            "project {} has no dev agent principal",
            args.project_id
        ))
    })?;

    let outcome = submit_event(
        pool,
        SubmitEventRequest {
            project_id: args.project_id,
            actor_id,
            source: MCP_SOURCE.to_string(),
            event_type: SUBMIT_EVENT_TYPE.to_string(),
            idempotency_key: args.idempotency_key,
            payload: json!({
                "task_ref": args.task_ref,
                "fixture": args.fixture,
                "summary": args.summary,
            }),
        },
    )
    .await
    .map_err(|err| match err {
        IngestionError::IdempotencyConflict { existing_event_id } => ToolError::Conflict(format!(
            "idempotency_key already used with a different payload (existing event {existing_event_id})"
        )),
        IngestionError::UnknownProject => ToolError::NotFound(format!(
            "unknown project {} (or missing dev agent principal)",
            args.project_id
        )),
        IngestionError::Db(e) => ToolError::Db(e),
    })?;

    Ok(match outcome {
        pm_application::SubmitOutcome::Accepted { event_id } => json!({
            "event_id": event_id,
            "processing_status": "queued",
        }),
        pm_application::SubmitOutcome::Duplicate { event_id, .. } => json!({
            "event_id": event_id,
            "processing_status": "duplicate",
        }),
    })
}

/// Aggregates one event's processing status: the event row's status, the
/// proposals of all its analysis runs (`id`, `proposal_seq`, `status`,
/// `command_type`, ordered by `proposal_seq`) and the latest analysis run's
/// `id`/`status` (`null` before the pipeline produced one).
pub async fn get_submission_status(
    pool: &PgPool,
    args: SubmissionStatusArgs,
) -> Result<Value, ToolError> {
    let event_status: String = sqlx::query_scalar(
        "SELECT status FROM integration.inbound_event WHERE id = $1",
    )
    .bind(args.event_id)
    .fetch_optional(pool)
    .await
    .map_err(ToolError::Db)?
    .ok_or_else(|| ToolError::NotFound(format!("event {} not found", args.event_id)))?;

    let proposals: Vec<(Uuid, i32, String, String)> = sqlx::query_as(
        "SELECT p.id, p.proposal_seq, p.status, p.command_type \
         FROM ai.change_proposal p \
         JOIN ai.analysis_run r ON r.id = p.analysis_run_id \
         WHERE r.event_id = $1 \
         ORDER BY p.proposal_seq ASC",
    )
    .bind(args.event_id)
    .fetch_all(pool)
    .await
    .map_err(ToolError::Db)?;

    let analysis_run: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, status FROM ai.analysis_run WHERE event_id = $1 \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(args.event_id)
    .fetch_optional(pool)
    .await
    .map_err(ToolError::Db)?;

    Ok(json!({
        "event_id": args.event_id,
        "status": event_status,
        "analysis_run": analysis_run
            .map(|(id, status)| json!({"id": id, "status": status})),
        "proposals": proposals
            .into_iter()
            .map(|(id, proposal_seq, status, command_type)| json!({
                "id": id,
                "proposal_seq": proposal_seq,
                "status": status,
                "command_type": command_type,
            }))
            .collect::<Vec<_>>(),
    }))
}

/// Reads one task's context via `read::task_context` (project-scoped: a task
/// of another project reads as `NotFound`, the same as a missing task).
pub async fn get_task_context(pool: &PgPool, args: TaskContextArgs) -> Result<Value, ToolError> {
    let task_id = parse_task_ref(&args.task_ref)?;
    let row = task_context(pool, args.project_id, task_id).await.map_err(|err| match err {
        ReadError::NotFound => ToolError::NotFound(format!(
            "task {} not found in project {} (or not visible)",
            args.task_ref, args.project_id
        )),
        ReadError::Db(e) => ToolError::Db(e),
    })?;
    Ok(json!({
        "task_id": row.task_id,
        "title": row.title,
        "status": row.status,
        "revision": row.revision,
        "requirement": row.requirement,
        "milestone": row.milestone,
    }))
}
/// Creates one knowledge document: Markdown body + Git commit + identity/revision
/// rows. The acting principal is the project's `ai_pm` (knowledge maintainer per
/// design §四); MCP callers are dev agents and must hold
/// `PM_MCP_DOCUMENT_WRITE` (enforced by the pm-api middleware).
pub async fn submit_document(
    pool: &PgPool,
    root: &KnowledgeRoot,
    args: SubmitDocumentArgs,
) -> Result<Value, ToolError> {
    let actor_id: Uuid = sqlx::query_scalar(
        "SELECT pm.principal_id FROM core.project_member pm \
         JOIN core.principal p ON p.id = pm.principal_id \
         WHERE pm.project_id = $1 AND p.kind = 'ai_pm' \
         ORDER BY pm.principal_id LIMIT 1",
    )
    .bind(args.project_id)
    .fetch_optional(pool)
    .await
    .map_err(ToolError::Db)?
    .ok_or_else(|| {
        ToolError::NotFound(format!(
            "project {} has no ai_pm principal",
            args.project_id
        ))
    })?;

    let created = knowledge::create_document(
        pool,
        root,
        args.project_id,
        actor_id,
        CreateDocumentRequest {
            doc_key: args.doc_key,
            doc_type: args.doc_type,
            markdown: args.markdown,
            change_summary: args.change_summary,
        },
    )
    .await
    .map_err(|err| match err {
        KnowledgeError::NotFound => ToolError::NotFound(format!(
            "project {} not found or has no document",
            args.project_id
        )),
        KnowledgeError::Exists(key) => {
            ToolError::Conflict(format!("document {key} already exists"))
        }
        KnowledgeError::InvalidKey(key) => {
            ToolError::InvalidRequest(format!("invalid doc_key {key}"))
        }
        KnowledgeError::InvalidType(doc_type) => {
            ToolError::InvalidRequest(format!("invalid doc_type {doc_type}"))
        }
        KnowledgeError::Stale => {
            ToolError::Conflict("document content drifted from its recorded hash".into())
        }
        KnowledgeError::Config(message)
        | KnowledgeError::Io(message)
        | KnowledgeError::Git(message) => ToolError::InvalidRequest(message),
        KnowledgeError::Db(err) => ToolError::Db(err),
    })?;

    Ok(json!({
        "document_id": created.document_id,
        "revision_no": created.revision_no,
        "git_commit_sha": created.git_commit_sha,
        "content_hash": created.content_hash,
        "canonical_path": created.canonical_path,
    }))
}
