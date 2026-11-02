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
    /// Idempotency key: replaying the same key produces no second revision.
    pub operation_id: String,
}

/// Arguments of `pm_propose_document`: submit a proposal through the quality gates.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProposeDocumentArgs {
    pub project_id: Uuid,
    /// Stable document key (`[A-Za-z0-9._-]`), unique per project.
    pub doc_key: String,
    /// One of requirement / decision / architecture / knowledge.
    pub doc_type: String,
    /// Proposed Markdown body (LF); must carry `source_events:` or a doc link.
    pub markdown: String,
    /// Required when rewriting an official document.
    pub change_summary: Option<String>,
    /// Provenance: outbox event id (either this or `analysis_run_id`/`source_ref`).
    pub source_event_id: Option<Uuid>,
    /// Provenance: analysis run id.
    pub analysis_run_id: Option<Uuid>,
    /// Provenance: explicit source marker (e.g. `requirement_revision:<id>`).
    pub source_ref: Option<String>,
    /// Red-line declaration: external_commitment | delete_history | rewrite_delivered.
    pub redline_kind: Option<String>,
    /// Optimistic concurrency: the published revision this proposal is based on.
    pub base_revision_id: Option<Uuid>,
    /// Idempotency key: replaying the same key produces no second revision.
    pub operation_id: String,
}

/// Arguments of `pm_confirm_document_proposal`: human confirmation of a red-line proposal.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConfirmDocumentProposalArgs {
    pub project_id: Uuid,
    pub proposal_id: Uuid,
    /// Must be a human principal.
    pub confirmed_by: Uuid,
    /// Idempotency key: replaying the same key produces no second revision.
    pub operation_id: String,
}

/// Arguments of `pm_veto_document_revision`: roll back a published revision.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VetoDocumentRevisionArgs {
    pub project_id: Uuid,
    pub revision_id: Uuid,
    /// Must be a human principal.
    pub vetoed_by: Uuid,
    pub reason: String,
    /// Idempotency key: replaying the same key produces no second rollback.
    pub operation_id: String,
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
    /// D2：run 级凭据不匹配 / 缺失 / 非活跃。绝不回显 hash 或正确 owner。
    #[error("forbidden: {0}")]
    Forbidden(String),
    /// D2：租约已过期——正确 token 也必须重新领取。
    #[error("lease expired: {0}")]
    LeaseExpired(String),
    /// D2 shape-first：D4 才实现的动作，不触库。
    #[error("unsupported: {0}")]
    Unsupported(String),
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
            operation_id: args.operation_id,
            base_revision_id: None,
        },
    )
    .await
    .map_err(map_knowledge_error)?;

    Ok(json!({
        "document_id": created.document_id,
        "revision_id": created.revision_id,
        "revision_no": created.revision_no,
        "git_commit_sha": created.git_commit_sha,
        "content_hash": created.content_hash,
        "canonical_path": created.canonical_path,
    }))
}

fn map_knowledge_error(err: KnowledgeError) -> ToolError {
    match err {
        KnowledgeError::NotFound => {
            ToolError::NotFound("knowledge document or project not found".to_string())
        }
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
        KnowledgeError::Conflict(message) => ToolError::Conflict(message),
        // T-060：写锁排队超时（可重试）；base_revision 过期（不产生修订）。
        KnowledgeError::ProjectWriteBusy(message) => ToolError::Conflict(message),
        KnowledgeError::RevisionConflict(message) => ToolError::Conflict(message),
        KnowledgeError::IndexUnavailable(message) => ToolError::Conflict(message),
        KnowledgeError::Config(message)
        | KnowledgeError::Io(message)
        | KnowledgeError::Git(message) => ToolError::InvalidRequest(message),
        KnowledgeError::Db(err) => ToolError::Db(err),
    }
}

async fn ai_pm_principal(pool: &PgPool, project_id: Uuid) -> Result<Uuid, ToolError> {
    sqlx::query_scalar(
        "SELECT pm.principal_id FROM core.project_member pm \
         JOIN core.principal p ON p.id = pm.principal_id \
         WHERE pm.project_id = $1 AND p.kind = 'ai_pm' \
         ORDER BY pm.principal_id LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(ToolError::Db)?
    .ok_or_else(|| {
        ToolError::NotFound(format!("project {project_id} has no ai_pm principal"))
    })
}

/// 知识空间 V2：提交提案（质量门 → 自动提交 / 红线挂起 / 拒绝）。
pub async fn propose_document(
    pool: &PgPool,
    root: &KnowledgeRoot,
    args: ProposeDocumentArgs,
) -> Result<Value, ToolError> {
    let created_by = ai_pm_principal(pool, args.project_id).await?;
    let outcome = pm_application::knowledge_proposal::submit_proposal(
        pool,
        root,
        pm_application::knowledge_proposal::ProposalRequest {
            project_id: args.project_id,
            doc_key: args.doc_key,
            doc_type: args.doc_type,
            markdown: args.markdown,
            change_summary: args.change_summary,
            source_event_id: args.source_event_id,
            analysis_run_id: args.analysis_run_id,
            source_ref: args.source_ref,
            redline_kind: args.redline_kind,
            base_revision_id: args.base_revision_id,
            operation_id: args.operation_id,
            created_by,
        },
    )
    .await
    .map_err(map_knowledge_error)?;
    Ok(serde_json::to_value(outcome).unwrap_or_default())
}

/// 人类确认红线提案。
pub async fn confirm_document_proposal(
    pool: &PgPool,
    root: &KnowledgeRoot,
    args: ConfirmDocumentProposalArgs,
) -> Result<Value, ToolError> {
    let outcome = pm_application::knowledge_proposal::confirm_redline_proposal(
        pool,
        root,
        args.proposal_id,
        args.project_id,
        args.confirmed_by,
        args.operation_id,
    )
    .await
    .map_err(map_knowledge_error)?;
    Ok(serde_json::to_value(outcome).unwrap_or_default())
}

/// 否决窗口：回滚已发布修订。
pub async fn veto_document_revision(
    pool: &PgPool,
    root: &KnowledgeRoot,
    args: VetoDocumentRevisionArgs,
) -> Result<Value, ToolError> {
    let outcome = pm_application::knowledge_proposal::veto_revision(
        pool,
        root,
        args.project_id,
        args.revision_id,
        args.vetoed_by,
        &args.reason,
        args.operation_id,
    )
    .await
    .map_err(map_knowledge_error)?;
    Ok(serde_json::to_value(outcome).unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Slice D2：执行端三工具 + submit_result shape-first
// ---------------------------------------------------------------------------

/// Arguments of `pm_claim_task`：执行端按 task 显式领取（Model A）。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClaimTaskArgs {
    /// Project scope; a task of another project reads as not found.
    pub project_id: Uuid,
    /// The task to claim (`delivery.task.id`).
    pub task_id: Uuid,
    /// Executor identity, recorded on the Run (`client_name`).
    pub client_name: String,
    /// Executor build/version, recorded but not used for tiering in V1.
    pub client_version: Option<String>,
}

/// Arguments of `pm_get_run_context`：凭租约读取本 Run 的执行契约。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RunContextArgs {
    pub project_id: Uuid,
    pub run_id: Uuid,
    /// The one-time plaintext token `pm_claim_task` returned.
    pub lease_token: String,
    /// T-032 T2：执行权世代（`pm_claim_task` 返回）。服务端与 token 双验证；
    /// 租约重领后旧 generation 永不重新获得授权。
    pub lease_generation: i32,
}

/// Arguments of `pm_report_progress`：续租 + 阶段推进 + 审计（单事务）。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReportProgressArgs {
    pub project_id: Uuid,
    pub run_id: Uuid,
    pub lease_token: String,
    /// T-032 T2：执行权世代（与 token 双验证）。
    pub lease_generation: i32,
    /// Report the durable stage; `started` is the only one that advances state.
    pub stage: String,
    /// Free-form progress note (audited, never stored in the contract).
    pub note: Option<String>,
    /// Mark the execution as blocked (audited; task status stays unchanged in D2).
    #[serde(default)]
    pub blocked: bool,
}

/// One repository result inside the frozen `pm_submit_result` shape.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubmittedRepository {
    pub repository_id: Uuid,
    pub base_commit: String,
    pub commit_sha: String,
    pub changed_files: i32,
    pub tests: Vec<SubmittedTest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubmittedTest {
    pub name: String,
    pub passed: bool,
    pub evidence_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct KnowledgeProposalInput {
    pub document_id: Uuid,
    pub change_summary: String,
}

/// Arguments of `pm_submit_result`：**D4 定稿**（在 D2 draft 形状上只加
/// `integration_status`）。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubmitResultArgs {
    pub project_id: Uuid,
    pub run_id: Uuid,
    pub lease_token: String,
    /// T-032 T2：执行权世代（D4 落库时经 `accept_executor_result` 双验证）。
    pub lease_generation: i32,
    /// Run 级集成事实（`passed` / `failed` / `not_run`）；缺省 `not_run`。
    #[serde(default)]
    pub integration_status: Option<String>,
    pub repositories: Vec<SubmittedRepository>,
    pub summary: String,
    pub knowledge_proposals: Vec<KnowledgeProposalInput>,
}

fn map_dispatch_error_v2(err: pm_application::run_dispatch::RunDispatchError) -> ToolError {
    use pm_application::run_dispatch::RunDispatchError as E;
    match err {
        E::TaskNotFound => ToolError::NotFound("task not found in this project".to_string()),
        E::NoRunnableRun => ToolError::NotFound(
            "no runnable run for the task (wait for the TaskReady event)".to_string(),
        ),
        E::TaskNotReady(state) => {
            ToolError::Conflict(format!("task is not ready (current status: {state})"))
        }
        E::RunAlreadyClaimed => {
            ToolError::Conflict("the task already has an active run".to_string())
        }
        E::Transition(message) => ToolError::Conflict(message),
        E::NoActor | E::InvalidOption(_) | E::Random => {
            ToolError::InvalidRequest(err.to_string())
        }
        E::Db(e) => ToolError::Db(e),
    }
}

fn map_lease_error_v2(err: pm_application::run_context::LeaseError) -> ToolError {
    use pm_application::run_context::LeaseError as E;
    match err {
        E::NotFound => ToolError::NotFound("run not found in this project".to_string()),
        E::Forbidden => ToolError::Forbidden("lease credential rejected".to_string()),
        E::Expired => {
            ToolError::LeaseExpired("lease expired; claim the task again".to_string())
        }
        E::Conflict(message) => ToolError::Conflict(message),
        E::Db(e) => ToolError::Db(e),
        E::Random => ToolError::InvalidRequest("lease token generation failed".to_string()),
    }
}

/// Model A 领取：一次调用拿到 Run 与一次性租约明文。
pub async fn claim_task(pool: &PgPool, args: ClaimTaskArgs) -> Result<Value, ToolError> {
    let cfg = pm_application::run_dispatch::RunDispatchConfig::from_env();
    let claimed = pm_application::run_dispatch::claim_task_run(
        pool,
        args.project_id,
        args.task_id,
        &args.client_name,
        args.client_version.as_deref(),
        &cfg,
    )
    .await
    .map_err(map_dispatch_error_v2)?;
    Ok(json!({
        "run_id": claimed.run_id,
        "task_id": claimed.task_id,
        "attempt_no": claimed.attempt_no,
        "lease_token": claimed.lease_token,
        "lease_generation": claimed.lease_generation,
        "lease_expires_at": claimed.lease_expires_at,
        "git_branch": claimed.git_branch,
        "stage": claimed.stage,
    }))
}

/// 读取本 Run 的执行契约（只读、不续租）。
pub async fn get_run_context(pool: &PgPool, args: RunContextArgs) -> Result<Value, ToolError> {
    let contract = pm_application::run_context::get_run_context(
        pool,
        args.project_id,
        args.run_id,
        &args.lease_token,
        args.lease_generation,
    )
    .await
    .map_err(map_lease_error_v2)?;
    serde_json::to_value(contract).map_err(|e| ToolError::InvalidRequest(e.to_string()))
}

/// 续租（= 心跳）+ 首次 `started` 阶段推进 + 审计，单事务完成。
pub async fn report_progress(pool: &PgPool, args: ReportProgressArgs) -> Result<Value, ToolError> {
    let cfg = pm_application::run_dispatch::RunDispatchConfig::from_env();
    let renewed = pm_application::run_context::report_progress(
        pool,
        args.project_id,
        args.run_id,
        &args.lease_token,
        pm_application::run_context::ProgressReport {
            stage: &args.stage,
            note: args.note.as_deref(),
            blocked: args.blocked,
        },
        &cfg,
        args.lease_generation,
    )
    .await
    .map_err(map_lease_error_v2)?;
    Ok(json!({
        "run_id": renewed.run_id,
        "stage": renewed.stage,
        "status": renewed.status,
        "task_status": renewed.task_status,
        "lease_expires_at": renewed.lease_expires_at,
    }))
}

/// D4 定稿实现：真实落库（交付段落两表 + Run 级集成事实 + 任务状态迁移）。
///
/// 错误映射（设计稿 §4.1 九项错误码）：
/// - `lease_invalid` / `lease_expired` → `Forbidden` / `LeaseExpired`；
/// - 其余业务驳回（`readonly_repo_commit` / `baseline_mismatch` /
///   `commit_not_descendant` / `repo_not_in_contract` / `submit_conflict` /
///   `integration_status_invalid` / `invalid_request`）→ `Conflict`（409 语义），
///   错误文本带机器可读错误码。
///
/// **业务驳回不是异常**：管线把驳回作为 `SubmitOutcome::Rejected` 返回（审计已
/// 持久），工具层照样回 200 并带上 `outcome: "rejected"` 与驳回原因——调用方
/// 据此知道「没有交付」而不是「调用失败」。
pub async fn submit_result(pool: &PgPool, args: SubmitResultArgs) -> Result<Value, ToolError> {
    submit_result_with(pool, args, &pm_application::delivery::DeliveryOptions::from_env()).await
}

/// 同 [`submit_result`]，但可注入工作区根与 git 超时（测试用——避免靠环境变量
/// 传路径，那会与同进程内的并行用例抢同一个全局变量）。
pub async fn submit_result_with(
    pool: &PgPool,
    args: SubmitResultArgs,
    options: &pm_application::delivery::DeliveryOptions,
) -> Result<Value, ToolError> {
    let request = pm_application::delivery::SubmitResultRequest {
        project_id: args.project_id,
        run_id: args.run_id,
        lease_token: args.lease_token,
        lease_generation: args.lease_generation,
        integration_status: args.integration_status,
        repositories: args
            .repositories
            .into_iter()
            .map(|repo| pm_application::delivery::SubmittedRepository {
                repository_id: repo.repository_id,
                base_commit: repo.base_commit,
                commit_sha: repo.commit_sha,
                changed_files: repo.changed_files,
                tests: repo
                    .tests
                    .into_iter()
                    .map(|test| pm_application::delivery::SubmittedTest {
                        name: test.name,
                        passed: test.passed,
                        evidence_ref: test.evidence_ref,
                    })
                    .collect(),
            })
            .collect(),
        summary: args.summary,
        knowledge_proposals: args
            .knowledge_proposals
            .into_iter()
            .map(|proposal| pm_application::delivery::KnowledgeProposalInput {
                document_id: proposal.document_id,
                change_summary: proposal.change_summary,
            })
            .collect(),
    };

    let outcome = pm_application::delivery::submit_delivery_with(pool, &request, options)
        .await
        .map_err(map_delivery_error)?;

    Ok(match outcome {
        pm_application::delivery::SubmitOutcome::Accepted { manifest } => json!({
            "outcome": "accepted",
            "run_id": manifest.run_id,
            "task_id": manifest.task_id,
            "integration_status": manifest.integration_status,
            "commit_count": manifest.commit_count,
            "knowledge_proposal_count": manifest.knowledge_proposal_count,
            "repositories": manifest.repositories,
        }),
        pm_application::delivery::SubmitOutcome::Rejected { rejection } => json!({
            "outcome": "rejected",
            "code": rejection.code,
            "message": rejection.message,
            "submitted_repository_count": rejection.submitted_repository_count,
            "knowledge_proposal_count": rejection.knowledge_proposal_count,
        }),
    })
}

/// D4 错误码 → MCP 工具错误。
fn map_delivery_error(err: pm_application::delivery::DeliveryError) -> ToolError {
    use pm_application::delivery::DeliveryError as E;
    match err {
        E::LeaseInvalid(message) => ToolError::Forbidden(message),
        E::LeaseExpired(message) => ToolError::LeaseExpired(message),
        E::Db(error) => ToolError::Db(error),
        other => ToolError::Conflict(format!("{}: {other}", other.code())),
    }
}
