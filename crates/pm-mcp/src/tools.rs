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

use pm_application::authz::ExecutorKind;
use pm_application::caller::CallerContext;
use pm_application::ingestion::{submit_event, IngestionError, SubmitEventRequest};
use pm_application::knowledge::{self, CreateDocumentRequest, KnowledgeError, KnowledgeRoot};
use pm_application::read::{task_context, ReadError};

use crate::caller::{authorize, ensure_member};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
    Uuid::parse_str(task_ref).map_err(|_| {
        ToolError::InvalidRequest(format!("task_ref `{task_ref}` is not a UUID string"))
    })
}

/// Submits one technical design: resolves the acting `dev_agent` principal,
/// verifies the referenced task is visible in the project, then reuses the
/// idempotent `submit_event`. Answers `{event_id, processing_status}` where
/// `processing_status` is `"queued"` (fresh event) or `"duplicate"` (idempotent
/// replay).
pub async fn submit_technical_design(
    pool: &PgPool,
    args: SubmitTechnicalDesignArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    // T-088 流水线第 2 步：真人成员校验（非成员 404），再取执行主体 dev_agent。
    let actor = authorize(pool, caller, args.project_id, ExecutorKind::DevAgent).await?;
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

    // MCP callers are dev agents: act as the project's `dev_agent` principal
    // (resolved by `authorize` above, after the human membership check).
    let actor_id: Uuid = actor.principal_id;

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
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    // 该工具的 wire 契约只有 event_id（冻结：不改 JSON Schema）→ 由 inbound_event
    // 反查所属项目（id 是 PK，命中唯一），再走与其余工具一致的真人成员校验。
    let (event_status, project_id): (String, Uuid) =
        sqlx::query_as("SELECT status, project_id FROM integration.inbound_event WHERE id = $1")
            .bind(args.event_id)
            .fetch_optional(pool)
            .await
            .map_err(ToolError::Db)?
            .ok_or_else(|| ToolError::NotFound(format!("event {} not found", args.event_id)))?;
    ensure_member(pool, caller, project_id).await?;

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
pub async fn get_task_context(
    pool: &PgPool,
    args: TaskContextArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    ensure_member(pool, caller, args.project_id).await?;
    let task_id = parse_task_ref(&args.task_ref)?;
    let row = task_context(pool, args.project_id, task_id)
        .await
        .map_err(|err| match err {
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

/// Arguments of `pm_list_tasks`（T-082，runbook G3）：项目内 `ready` 任务的只读
/// 列表，附最新 `queued` Run 的 id（执行端据此判断「能不能直接领」）。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListTasksArgs {
    /// Project scope; another project's tasks read as not found.
    pub project_id: Uuid,
    /// Page size (1..=100); defaults to 20.
    #[serde(default)]
    #[schemars(range(min = 1, max = 100))]
    pub limit: Option<u32>,
}

/// 列出项目内 `ready` 任务（T-082）：真人成员门在前（非成员 404），随后按
/// priority（urgent→low）+ created_at 排序；每行附最新 `queued` Run 的 id——
/// 无 queued run 的 ready 任务领了会 `NoRunnableRun`，故以字段提示而非硬过滤。
pub async fn list_tasks(
    pool: &PgPool,
    args: ListTasksArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    ensure_member(pool, caller, args.project_id).await?;
    let limit = args.limit.unwrap_or(20).clamp(1, 100) as i64;
    let rows: Vec<(
        Uuid,
        String,
        String,
        String,
        Option<Uuid>,
        Option<Uuid>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT t.id, t.title, t.priority, t.status, t.assignee_id, \
                q.run_id, t.created_at \
         FROM delivery.task t \
         LEFT JOIN LATERAL ( \
             SELECT r.id AS run_id FROM delivery.task_run r \
              WHERE r.task_id = t.id AND r.status = 'queued' \
              ORDER BY r.attempt_no DESC LIMIT 1 \
         ) q ON TRUE \
         WHERE t.project_id = $1 AND t.status = 'ready' \
         ORDER BY CASE t.priority \
                    WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 \
                    WHEN 'normal' THEN 2 ELSE 3 END, \
                  t.created_at ASC \
         LIMIT $2",
    )
    .bind(args.project_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(ToolError::Db)?;

    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task WHERE project_id = $1 AND status = 'ready'",
    )
    .bind(args.project_id)
    .fetch_one(pool)
    .await
    .map_err(ToolError::Db)?;

    let tasks: Vec<Value> = rows
        .into_iter()
        .map(
            |(id, title, priority, status, assignee_id, queued_run_id, created_at)| {
                json!({
                    "task_id": id,
                    "title": title,
                    "priority": priority,
                    "status": status,
                    "assignee_id": assignee_id,
                    "queued_run_id": queued_run_id,
                    "created_at": created_at,
                })
            },
        )
        .collect();
    Ok(json!({ "tasks": tasks, "total": total }))
}
/// Creates one knowledge document: Markdown body + Git commit + identity/revision
/// rows. The acting principal is the project's `ai_pm` (knowledge maintainer per
/// design §四); MCP callers are dev agents and must hold
/// `PM_MCP_DOCUMENT_WRITE` (enforced by the pm-api middleware).
pub async fn submit_document(
    pool: &PgPool,
    root: &KnowledgeRoot,
    args: SubmitDocumentArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    // 文档类工具的执行主体维持现状口径：项目 `ai_pm`（真人成员校验在前）。
    let actor_id: Uuid = authorize(pool, caller, args.project_id, ExecutorKind::AiPm)
        .await?
        .principal_id;

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

/// 知识空间 V2：提交提案（质量门 → 自动提交 / 红线挂起 / 拒绝）。
pub async fn propose_document(
    pool: &PgPool,
    root: &KnowledgeRoot,
    args: ProposeDocumentArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    let created_by = authorize(pool, caller, args.project_id, ExecutorKind::AiPm)
        .await?
        .principal_id;
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
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    ensure_member(pool, caller, args.project_id).await?;
    // T-088 §4.6：`confirmed_by` 参数仅为 wire 兼容保留；审计依据改为**认证调用人**
    // （真人 Principal 是唯一授权依据，不接受客户端自称的第三人）。
    let _ = args.confirmed_by;
    let outcome = pm_application::knowledge_proposal::confirm_redline_proposal(
        pool,
        root,
        args.proposal_id,
        args.project_id,
        caller.human.principal_id,
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
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    ensure_member(pool, caller, args.project_id).await?;
    // T-088 §4.6：同 confirm——`vetoed_by` 仅 wire 兼容，审计依据为认证调用人。
    let _ = args.vetoed_by;
    let outcome = pm_application::knowledge_proposal::veto_revision(
        pool,
        root,
        args.project_id,
        args.revision_id,
        caller.human.principal_id,
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
        E::NoActor | E::InvalidOption(_) | E::Random => ToolError::InvalidRequest(err.to_string()),
        E::Db(e) => ToolError::Db(e),
    }
}

fn map_lease_error_v2(err: pm_application::run_context::LeaseError) -> ToolError {
    use pm_application::run_context::LeaseError as E;
    match err {
        E::NotFound => ToolError::NotFound("run not found in this project".to_string()),
        E::Forbidden => ToolError::Forbidden("lease credential rejected".to_string()),
        E::Expired => ToolError::LeaseExpired("lease expired; claim the task again".to_string()),
        E::Conflict(message) => ToolError::Conflict(message),
        E::Db(e) => ToolError::Db(e),
        E::Random => ToolError::InvalidRequest("lease token generation failed".to_string()),
    }
}

/// Model A 领取：一次调用拿到 Run 与一次性租约明文。
pub async fn claim_task(
    pool: &PgPool,
    args: ClaimTaskArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    // T-051 关闭点：claim 必须先过真人成员校验（dev_agent 本身就是成员，不能用它校验）。
    let actor = authorize(pool, caller, args.project_id, ExecutorKind::DevAgent).await?;
    let cfg = pm_application::run_dispatch::RunDispatchConfig::from_env();
    // V-05-1：cpk_ 凭据通道 → 稳定 Client 身份（lease_client_id + client:{id}
    // 路由）；真人 JWT 通道维持既有 mcp:{client_name}:{token8} 形态。
    let claimed = match caller.client_channel {
        Some(channel) => pm_application::run_dispatch::claim_task_run_as_client(
            pool,
            args.project_id,
            args.task_id,
            channel.device_id,
            &args.client_name,
            args.client_version.as_deref(),
            &cfg,
            &actor,
        )
        .await
        .map_err(map_dispatch_error_v2)?,
        None => pm_application::run_dispatch::claim_task_run_as(
            pool,
            args.project_id,
            args.task_id,
            &args.client_name,
            args.client_version.as_deref(),
            &cfg,
            &actor,
        )
        .await
        .map_err(map_dispatch_error_v2)?,
    };
    Ok(json!({
        "run_id": claimed.run_id,
        "task_id": claimed.task_id,
        "attempt_no": claimed.attempt_no,
        "lease_token": claimed.lease_token,
        "lease_generation": claimed.lease_generation,
        "lease_expires_at": claimed.lease_expires_at,
        "git_branch": claimed.git_branch,
        "stage": claimed.stage,
        // V-05-1：命令路由身份快照——执行端 poll/ack 必须原样回带。
        "lease_owner": claimed.lease_owner,
    }))
}

/// 读取本 Run 的执行契约（只读、不续租）。
pub async fn get_run_context(
    pool: &PgPool,
    args: RunContextArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    ensure_member(pool, caller, args.project_id).await?;
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
pub async fn report_progress(
    pool: &PgPool,
    args: ReportProgressArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    let actor = authorize(pool, caller, args.project_id, ExecutorKind::DevAgent).await?;
    let cfg = pm_application::run_dispatch::RunDispatchConfig::from_env();
    let renewed = pm_application::run_context::report_progress_as(
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
        &actor,
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
pub async fn submit_result(
    pool: &PgPool,
    args: SubmitResultArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    submit_result_with(
        pool,
        args,
        &pm_application::delivery::DeliveryOptions::from_env(),
        caller,
    )
    .await
}

/// 同 [`submit_result`]，但可注入工作区根与 git 超时（测试用——避免靠环境变量
/// 传路径，那会与同进程内的并行用例抢同一个全局变量）。
pub async fn submit_result_with(
    pool: &PgPool,
    args: SubmitResultArgs,
    options: &pm_application::delivery::DeliveryOptions,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    let actor = authorize(pool, caller, args.project_id, ExecutorKind::DevAgent).await?;
    let request = pm_application::delivery::SubmitResultRequest {
        caller: Some(actor),
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
            .map(
                |proposal| pm_application::delivery::KnowledgeProposalInput {
                    document_id: proposal.document_id,
                    change_summary: proposal.change_summary,
                },
            )
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
// ---------------------------------------------------------------------------
// T-085：Discovery 认知域六工具（propose-only，§4.2 污染红线）
// ---------------------------------------------------------------------------

/// Arguments of `pm_discovery_get` / `pm_discovery_get_context`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiscoveryGetArgs {
    pub project_id: Uuid,
    /// Session id; omit to resolve the project's live session
    /// (an `IN_PROGRESS` one first, else the most recent).
    pub session_id: Option<Uuid>,
}

/// One finding inside `pm_discovery_submit_findings`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiscoveryFindingInput {
    /// One of observed / user_confirmed / speculation / contradicted.
    pub kind: String,
    pub content: String,
    /// Model confidence within [0,1]; auxiliary, never a fact substitute.
    pub confidence: Option<f64>,
    /// Provenance list: [{repository, file, description}].
    #[serde(default)]
    pub evidence: serde_json::Value,
    #[serde(default)]
    pub repository_ids: Vec<Uuid>,
    pub actor_role: Option<String>,
}

/// Arguments of `pm_discovery_submit_findings`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiscoverySubmitFindingsArgs {
    pub project_id: Uuid,
    pub session_id: Uuid,
    pub findings: Vec<DiscoveryFindingInput>,
}

/// Arguments of `pm_submit_evidence`（I6-B-04：证据锚定到某一次分析轮次）。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubmitEvidenceArgs {
    pub project_id: Uuid,
    /// 要挂证据的发现。
    pub finding_id: Uuid,
    /// code / test_result / commit / user_statement / system_observation。
    pub evidence_type: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub code_range: Option<String>,
    /// 代码类证据必填：导入 id。
    #[serde(default)]
    pub import_id: Option<Uuid>,
    /// 代码类证据必填：仓库 id。
    #[serde(default)]
    pub repository_id: Option<Uuid>,
    /// 代码类证据必填：Commit SHA。
    #[serde(default)]
    pub commit_sha: Option<String>,
    /// 代码类证据必填：分析轮次号。
    #[serde(default)]
    pub attempt_no: Option<i32>,
}

/// One AI question inside `pm_discovery_submit_answers`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiscoveryQuestionInput {
    pub question: String,
    pub why_blocking: Option<String>,
    /// Anchor a question to a finding (discovery card flow).
    pub finding_id: Option<Uuid>,
}

/// One user answer relayed inside `pm_discovery_submit_answers`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiscoveryAnswerInput {
    pub question_id: Uuid,
    pub answer: String,
}

/// Arguments of `pm_discovery_submit_answers`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiscoverySubmitAnswersArgs {
    pub project_id: Uuid,
    pub session_id: Uuid,
    #[serde(default)]
    pub questions: Vec<DiscoveryQuestionInput>,
    #[serde(default)]
    pub answers: Vec<DiscoveryAnswerInput>,
}

/// Arguments of `pm_discovery_submit_proposal`: records a `discovery_proposal`
/// row and routes the content through the existing `propose_document` chain
/// (with `base_revision_id` for Q7 optimistic concurrency). Propose-only:
/// never writes `ai.project_document` directly.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiscoverySubmitProposalArgs {
    pub project_id: Uuid,
    pub session_id: Uuid,
    /// add | modify. `modify` requires `document_id` + `base_revision_id`.
    pub action: String,
    /// Target document path as recorded on the discovery proposal.
    pub target_document: String,
    /// Stable document key for the knowledge chain (`[A-Za-z0-9._-]`).
    pub doc_key: String,
    /// One of requirement / decision / architecture / knowledge.
    pub doc_type: String,
    /// Required for modify: the existing `ai.project_document` id.
    pub document_id: Option<Uuid>,
    /// Q7 base version: the published revision this proposal is based on.
    pub base_revision_id: Option<Uuid>,
    /// Proposed Markdown body (LF).
    pub proposed_content: String,
    pub change_summary: Option<String>,
    /// Provenance marker, e.g. `discovery_session:<id>`.
    pub source_ref: Option<String>,
    pub redline_kind: Option<String>,
    /// Idempotency key for the knowledge chain.
    pub operation_id: String,
}

/// Arguments of `pm_discovery_complete`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiscoveryCompleteArgs {
    pub project_id: Uuid,
    pub session_id: Uuid,
    /// COMPLETED or REVIEW_REQUIRED; the user approves in the Web console.
    pub status: String,
}

fn map_discovery_error(err: pm_application::discovery::DiscoveryError) -> ToolError {
    use pm_application::discovery::DiscoveryError as E;
    match err {
        E::SessionNotFound => ToolError::NotFound(
            "discovery session not found; ask the user to start one in the Web console".into(),
        ),
        E::FindingNotFound => ToolError::NotFound("discovery finding not found".into()),
        E::QuestionNotFound => ToolError::NotFound("discovery question not found".into()),
        E::ProposalNotFound => ToolError::NotFound("discovery proposal not found".into()),
        E::ProjectNotFound | E::ImportNotFound => {
            ToolError::NotFound("project or import not found".into())
        }
        E::InvalidTransition { from, to } => ToolError::Conflict(format!(
            "invalid session status transition from {from} to {to}"
        )),
        E::Validation(message) => ToolError::InvalidRequest(message),
        E::Db(err) => ToolError::Db(err),
    }
}

async fn project_tenant(pool: &PgPool, project_id: Uuid) -> Result<Uuid, ToolError> {
    sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await
        .map_err(ToolError::Db)?
        .ok_or_else(|| ToolError::NotFound(format!("project {project_id} not found")))
}

/// Session id resolution: explicit id must belong to the project; otherwise
/// pick the project's live session (`IN_PROGRESS` first, else most recent).
async fn resolve_session(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    session_id: Option<Uuid>,
) -> Result<Uuid, ToolError> {
    if let Some(id) = session_id {
        let ok: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM discovery.discovery_session \
             WHERE tenant_id = $1 AND id = $2 AND project_id = $3",
        )
        .bind(tenant_id)
        .bind(id)
        .bind(project_id)
        .fetch_optional(pool)
        .await
        .map_err(ToolError::Db)?;
        return ok.ok_or_else(|| {
            ToolError::NotFound(format!("discovery session {id} not found in project"))
        });
    }
    let rows: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM discovery.discovery_session \
         WHERE tenant_id = $1 AND project_id = $2 \
         ORDER BY (status = 'IN_PROGRESS') DESC, created_at DESC, id LIMIT 1",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(ToolError::Db)?;
    rows.into_iter().next().ok_or_else(|| {
        ToolError::NotFound(
            "project has no discovery session; ask the user to start one in the Web console".into(),
        )
    })
}

/// `pm_discovery_get`：读项目认知 session（findings / questions / proposals）。
pub async fn discovery_get(
    pool: &PgPool,
    args: DiscoveryGetArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    ensure_member(pool, caller, args.project_id).await?;
    let tenant_id = project_tenant(pool, args.project_id).await?;
    let session_id = resolve_session(pool, tenant_id, args.project_id, args.session_id).await?;
    let detail = pm_application::discovery::get_session_detail(pool, tenant_id, session_id)
        .await
        .map_err(map_discovery_error)?;
    serde_json::to_value(detail).map_err(|err| ToolError::InvalidRequest(err.to_string()))
}

/// `pm_discovery_get_context`：已有知识 + 仓库 + 待确认问题（/pm-learn 第一步）。
pub async fn discovery_get_context(
    pool: &PgPool,
    args: DiscoveryGetArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    ensure_member(pool, caller, args.project_id).await?;
    let tenant_id = project_tenant(pool, args.project_id).await?;
    let session_id = resolve_session(pool, tenant_id, args.project_id, args.session_id).await?;
    let detail = pm_application::discovery::get_session_detail(pool, tenant_id, session_id)
        .await
        .map_err(map_discovery_error)?;
    let documents = pm_application::knowledge::list_documents(pool, args.project_id)
        .await
        .map_err(map_knowledge_error)?;
    let repositories: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        "SELECT id, name, url, status FROM integration.project_repository \
         WHERE tenant_id = $1 AND project_id = $2 AND status <> 'archived' \
         ORDER BY created_at",
    )
    .bind(tenant_id)
    .bind(args.project_id)
    .fetch_all(pool)
    .await
    .map_err(ToolError::Db)?;
    let open_questions: Vec<Value> = detail
        .questions
        .iter()
        .filter(|question| question.status == "open")
        .map(|question| {
            json!({
                "id": question.id,
                "question": question.question,
                "why_blocking": question.why_blocking,
                "finding_id": question.finding_id,
            })
        })
        .collect();
    Ok(json!({
        "session": {
            "id": detail.session.id,
            "status": detail.session.status,
            "kind": detail.session.kind,
            "source": detail.session.source,
        },
        "documents": documents,
        "repositories": repositories
            .into_iter()
            .map(|(id, name, url, status)| json!({
                "id": id, "name": name, "url": url, "status": status
            }))
            .collect::<Vec<Value>>(),
        "open_questions": open_questions,
    }))
}

/// `pm_discovery_submit_findings`：采集层写 `discovery_finding`（含仓库关联）。
pub async fn discovery_submit_findings(
    pool: &PgPool,
    args: DiscoverySubmitFindingsArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    ensure_member(pool, caller, args.project_id).await?;
    if args.findings.is_empty() {
        return Err(ToolError::InvalidRequest(
            "findings must not be empty".into(),
        ));
    }
    let tenant_id = project_tenant(pool, args.project_id).await?;
    let mut added = Vec::with_capacity(args.findings.len());
    for finding in args.findings {
        let view = pm_application::discovery::add_finding(
            pool,
            tenant_id,
            args.session_id,
            pm_application::discovery::NewFinding {
                kind: finding.kind,
                content: finding.content,
                confidence: finding.confidence,
                evidence: finding.evidence,
                repository_ids: finding.repository_ids,
                actor_role: finding.actor_role,
            },
            pm_application::discovery::FindingCreator::AgentRelayed,
        )
        .await
        .map_err(map_discovery_error)?;
        added.push(view);
    }
    Ok(json!({ "added": added.len(), "findings": added }))
}

/// `pm_submit_evidence`：向既有发现补一条证据（I6-B-04）。
///
/// 代码类证据必须绑定 `import_id` / `repository_id` / `commit_sha` / `attempt_no`
/// ——服务端会校验这四列与某一次分析轮次一致（DB 级组合 FK 兜底）。
pub async fn submit_evidence(
    pool: &PgPool,
    args: SubmitEvidenceArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    ensure_member(pool, caller, args.project_id).await?;
    let tenant_id = project_tenant(pool, args.project_id).await?;
    let evidence = pm_application::discovery::submit_evidence(
        pool,
        tenant_id,
        args.finding_id,
        pm_application::discovery::EvidenceInput {
            evidence_type: Some(args.evidence_type),
            description: args.description,
            file: None,
            file_path: args.file_path,
            symbol: args.symbol,
            code_range: args.code_range,
            repository_id: args.repository_id,
            import_id: args.import_id,
            commit_sha: args.commit_sha,
            attempt_no: args.attempt_no,
        },
    )
    .await
    .map_err(map_discovery_error)?;
    Ok(json!({ "evidence": evidence }))
}

/// `pm_discovery_submit_answers`：提交 AI 问题与用户回答回传（采集层）。
pub async fn discovery_submit_answers(
    pool: &PgPool,
    args: DiscoverySubmitAnswersArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    ensure_member(pool, caller, args.project_id).await?;
    if args.questions.is_empty() && args.answers.is_empty() {
        return Err(ToolError::InvalidRequest(
            "questions and answers must not both be empty".into(),
        ));
    }
    let tenant_id = project_tenant(pool, args.project_id).await?;
    // 执行主体 = 项目 ai_pm（成员校验已由上方 ensure_member 完成）。
    let actor_id =
        pm_application::authz::resolve_project_executor(pool, args.project_id, ExecutorKind::AiPm)
            .await
            .map_err(|_| {
                ToolError::NotFound(format!(
                    "project {} has no ai_pm principal",
                    args.project_id
                ))
            })?;
    let mut questions_added = 0usize;
    for question in args.questions {
        pm_application::discovery::add_question(
            pool,
            tenant_id,
            args.session_id,
            &question.question,
            question.why_blocking.as_deref(),
            question.finding_id,
        )
        .await
        .map_err(map_discovery_error)?;
        questions_added += 1;
    }
    let mut answers_submitted = 0usize;
    for answer in args.answers {
        pm_application::discovery::answer_question(
            pool,
            tenant_id,
            answer.question_id,
            &answer.answer,
            actor_id,
        )
        .await
        .map_err(map_discovery_error)?;
        answers_submitted += 1;
    }
    Ok(json!({
        "questions_added": questions_added,
        "answers_submitted": answers_submitted,
    }))
}

/// `pm_discovery_submit_proposal`：propose-only——先走既有 `propose_document`
/// 知识提案链，再把结果镜像到 `discovery_proposal`（Q7 base_revision 失效置
/// conflict，质量门拒绝置 rejected，红线/通过回填 document_proposal_id）。
pub async fn discovery_submit_proposal(
    pool: &PgPool,
    root: &KnowledgeRoot,
    args: DiscoverySubmitProposalArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    let tenant_id = project_tenant(pool, args.project_id).await?;
    // 真人成员校验 + 执行主体 ai_pm（维持 M9 现状口径）。
    let created_by = authorize(pool, caller, args.project_id, ExecutorKind::AiPm)
        .await?
        .principal_id;
    let outcome = pm_application::knowledge_proposal::submit_proposal(
        pool,
        root,
        pm_application::knowledge_proposal::ProposalRequest {
            project_id: args.project_id,
            doc_key: args.doc_key.clone(),
            doc_type: args.doc_type,
            markdown: args.proposed_content.clone(),
            change_summary: args.change_summary,
            source_event_id: None,
            analysis_run_id: None,
            source_ref: args.source_ref.clone(),
            redline_kind: args.redline_kind,
            base_revision_id: args.base_revision_id,
            operation_id: args.operation_id,
            created_by,
        },
    )
    .await;

    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(KnowledgeError::Conflict(message)) | Err(KnowledgeError::RevisionConflict(message)) => {
            let proposal = pm_application::discovery::add_proposal(
                pool,
                tenant_id,
                args.session_id,
                pm_application::discovery::NewProposal {
                    action: args.action,
                    target_document: args.target_document,
                    document_id: args.document_id,
                    base_revision_id: args.base_revision_id,
                    proposed_content: args.proposed_content,
                    evidence: json!([]),
                },
            )
            .await
            .map_err(map_discovery_error)?;
            pm_application::discovery::set_proposal_status(
                pool,
                tenant_id,
                proposal.id,
                "conflict",
                None,
            )
            .await
            .map_err(map_discovery_error)?;
            return Err(ToolError::Conflict(format!(
                "base revision drifted (Q7): {message}"
            )));
        }
        Err(err) => return Err(map_knowledge_error(err)),
    };

    let discovery_proposal = pm_application::discovery::add_proposal(
        pool,
        tenant_id,
        args.session_id,
        pm_application::discovery::NewProposal {
            action: args.action,
            target_document: args.target_document,
            document_id: args.document_id,
            base_revision_id: args.base_revision_id,
            proposed_content: args.proposed_content,
            evidence: json!([]),
        },
    )
    .await
    .map_err(map_discovery_error)?;

    Ok(match &outcome {
        pm_application::knowledge_proposal::ProposalOutcome::Rejected {
            proposal_id,
            failures,
        } => {
            pm_application::discovery::set_proposal_status(
                pool,
                tenant_id,
                discovery_proposal.id,
                "rejected",
                None,
            )
            .await
            .map_err(map_discovery_error)?;
            json!({
                "discovery_proposal_id": discovery_proposal.id,
                "outcome": { "outcome": "rejected", "proposal_id": proposal_id, "failures": failures },
            })
        }
        other => {
            let document_proposal_id = match other {
                pm_application::knowledge_proposal::ProposalOutcome::RedlinePending {
                    proposal_id,
                } => *proposal_id,
                pm_application::knowledge_proposal::ProposalOutcome::Committed {
                    proposal_id,
                    ..
                } => *proposal_id,
                pm_application::knowledge_proposal::ProposalOutcome::Duplicate { .. } => {
                    discovery_proposal.id
                }
                pm_application::knowledge_proposal::ProposalOutcome::Rejected { .. } => {
                    unreachable!()
                }
            };
            let linked = pm_application::discovery::link_proposal_document(
                pool,
                tenant_id,
                discovery_proposal.id,
                document_proposal_id,
            )
            .await
            .map_err(map_discovery_error)?;
            json!({
                "discovery_proposal_id": linked.id,
                "status": linked.status,
                "outcome": other,
            })
        }
    })
}

/// `pm_discovery_complete`：置本轮学习完成状态（COMPLETED / REVIEW_REQUIRED）。
pub async fn discovery_complete(
    pool: &PgPool,
    args: DiscoveryCompleteArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    ensure_member(pool, caller, args.project_id).await?;
    if args.status != "COMPLETED" && args.status != "REVIEW_REQUIRED" {
        return Err(ToolError::InvalidRequest(
            "status must be COMPLETED or REVIEW_REQUIRED".into(),
        ));
    }
    let tenant_id = project_tenant(pool, args.project_id).await?;
    let session = pm_application::discovery::transition_session(
        pool,
        tenant_id,
        args.session_id,
        &args.status,
    )
    .await
    .map_err(map_discovery_error)?;
    Ok(json!({
        "session_id": session.id,
        "status": session.status,
    }))
}

// ---------------------------------------------------------------------------
// ACP 通信面（0046 / 设计稿 docs/ui/s03-acp/smark-pm-s03-acp-slice-design-
// acp01-02-08-v1.md V1.1）：interaction.required 上报 / 命令查询面 / 消费回执。
// 四工具为**新增**，现有四工具契约不动（工程纪律红线）。权限码见 pm-api
// auth.rs：PM_MCP_AGENT_REPORT / PM_MCP_COMMAND（P1-5，不再挂 PM_MCP_READ
// 语义；人类正式确认永不入码，A*）。
// ---------------------------------------------------------------------------

fn map_interaction_error(err: pm_application::interaction::InteractionError) -> ToolError {
    use pm_application::interaction::InteractionError as E;
    match err {
        E::NotFound => ToolError::NotFound("task or run not found in this project".to_string()),
        E::Conflict(message) => ToolError::Conflict(message),
        E::Validation(message) => ToolError::InvalidRequest(message),
        E::Forbidden(message) => ToolError::Forbidden(message),
        E::Transition(message) => ToolError::Conflict(message),
        E::Db(e) => ToolError::Db(e),
    }
}

fn map_command_face_error(err: pm_application::worker_command::CommandFaceError) -> ToolError {
    use pm_application::worker_command::CommandFaceError as E;
    match err {
        E::NotFound => ToolError::NotFound("command or run not found".to_string()),
        E::Validation(message) => ToolError::InvalidRequest(message),
        E::Db(e) => ToolError::Db(e),
    }
}

/// Arguments of `pm_report_interaction`：Agent 请求人工介入（N-2 闭合 · §1.3）。
/// 执行面身份：租约三要素（T-032 双校验）；平台同事务落 Interaction +
/// awaiting_decision + 调度保护 human_wait + terminate 命令（D-03）。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReportInteractionArgs {
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub lease_token: String,
    /// T-032 T2：执行权世代（与 token 双验证）。
    pub lease_generation: i32,
    /// Agent 侧幂等键（标记文件 schema `aipm.interaction.v1` 的 event_id）。
    pub event_id: String,
    /// needs_info / needs_decision / needs_approval / external_dependency。
    pub reason: String,
    pub title: String,
    pub question: String,
    /// 可选结构化选项（V1.3.1 §5.1 options）。
    #[serde(default)]
    pub options: Option<Value>,
    #[serde(default = "default_true")]
    pub required_response: bool,
    /// 请求自带 checkpoint 引用（文件在 Workspace，Platform 只登记）。
    #[serde(default)]
    pub checkpoint_path: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Agent → 执行端 → Platform 的 interaction.required 上报（R-ACP-1 已裁：
/// 标记文件侦测 + MCP 上报；人类回复仅 REST/UI，A* 边界不变）。
pub async fn report_interaction(
    pool: &PgPool,
    args: ReportInteractionArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    let actor = authorize(pool, caller, args.project_id, ExecutorKind::DevAgent).await?;
    pm_application::run_context::verify_lease(
        pool,
        args.project_id,
        args.run_id,
        &args.lease_token,
        args.lease_generation,
    )
    .await
    .map_err(map_lease_error_v2)?;
    let input = pm_application::interaction::ReportInteractionInput {
        project_id: args.project_id,
        task_id: args.task_id,
        run_id: args.run_id,
        event_id: args.event_id,
        reason: args.reason,
        title: args.title,
        question: args.question,
        options: args.options,
        required_response: args.required_response,
        checkpoint_path: args.checkpoint_path,
    };
    let reported = pm_application::interaction::report_interaction(pool, &input, &actor)
        .await
        .map_err(map_interaction_error)?;
    serde_json::to_value(reported).map_err(|e| ToolError::InvalidRequest(e.to_string()))
}

/// Arguments of `pm_poll_commands`：命令面领取（V1.1 §3.2/§3.3）。
///
/// 身份双轨（R-ACP-3 已裁）：命令面不要求 lease token——`worker_id` 即调用方
/// 的命令路由身份（= 领取时 run.lease_owner 快照）。V1 过渡形态：凭据校验由
/// `/mcp` 中间件承担（PM_MCP_COMMAND + 成员校验），`worker_id` 显式携带并经
/// worker_id 绑定 + lease_generation 只读比对 fencing；V-05-1 Client Registry
/// 落地后升级为 Client 凭据身份（迁移时仅替换身份来源，本工具契约不变）。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PollCommandsArgs {
    pub project_id: Uuid,
    pub run_id: Uuid,
    /// 命令路由身份（= 该 Run 领取时的 lease_owner 值）。
    pub worker_id: String,
}

/// 领取命令：原子 issued/delivered(过期) → delivered + claim 租约；同 worker
/// 可重领自己的活跃 claim（崩溃恢复幂等，ACP-437）。存在未回执 terminate 时
/// continue 不予下发（情形 6 / §9.4 检查 8 操作化）。
pub async fn poll_commands(
    pool: &PgPool,
    args: PollCommandsArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    ensure_member(pool, caller, args.project_id).await?;
    let commands = pm_application::worker_command::poll_commands(
        pool,
        args.project_id,
        args.run_id,
        &args.worker_id,
    )
    .await
    .map_err(map_command_face_error)?;
    Ok(json!({ "commands": commands }))
}

/// Arguments of `pm_ack_command`：命令回执（acked/failed + receipt）。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AckCommandArgs {
    pub project_id: Uuid,
    pub command_id: Uuid,
    /// 命令路由身份（必须等于领取时的 worker_id）。
    pub worker_id: String,
    /// "acked" | "failed"。
    pub outcome: String,
    /// 回执载荷：terminate 需 `exit_outcome`；continue 需
    /// `session_created` + `input_injected`（P1-1 三阶段，ACP-441）。
    #[serde(default)]
    pub receipt: Option<Value>,
    /// outcome=failed 时的最终错误（有界重试耗尽，§3.5）。
    #[serde(default)]
    pub error: Option<String>,
}

/// 命令回执。acked 从严判据（R-ACP-5）；continue acked 释放调度保护
/// start_wait → none（屏蔽释放点，§4.1）；terminate failed 冻结新 Session
/// 启动 + Task 转阻塞（R-ACP-7：不冻结回复提交）。
pub async fn ack_command(
    pool: &PgPool,
    args: AckCommandArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    ensure_member(pool, caller, args.project_id).await?;
    let outcome = match args.outcome.as_str() {
        "acked" => pm_application::worker_command::AckOutcome::Acked,
        "failed" => pm_application::worker_command::AckOutcome::Failed,
        other => {
            return Err(ToolError::InvalidRequest(format!(
                "outcome must be `acked` or `failed` (got `{other}`)"
            )));
        }
    };
    let result = pm_application::worker_command::ack_command(
        pool,
        args.project_id,
        args.command_id,
        &args.worker_id,
        outcome,
        args.receipt,
        args.error,
    )
    .await
    .map_err(map_command_face_error)?;
    serde_json::to_value(result).map_err(|e| ToolError::InvalidRequest(e.to_string()))
}

/// Arguments of `pm_report_consumption`：消费回执（V1.1 §1.5 / N-6）。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReportConsumptionArgs {
    pub project_id: Uuid,
    pub interaction_id: Uuid,
    pub run_id: Uuid,
    pub lease_token: String,
    pub lease_generation: i32,
    /// 回执投递 id——可缺省：服务端按 interaction 1:1 解析 delivery 账
    /// （联合契约 ②：工具签名不变；0050 起消费必落账，解析是服务端职责）。
    #[serde(default)]
    pub delivery_id: Option<Uuid>,
    /// 证据：`{"kinds": ["initial_input_injected", "process_alive"], ...}`。
    /// 证据不足 → 拒绝（禁止伪造 CONSUMED，§8.6；ACP-434）。
    pub evidence: Value,
}

/// 消费回执：responded → consumed、链关闭、等待 resolve、Task 回 Running。
/// N-6：无法提供可靠证据的 Adapter 保持 RESPONDED 未消费合法稳态（不伪造）。
pub async fn report_consumption(
    pool: &PgPool,
    args: ReportConsumptionArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    let actor = authorize(pool, caller, args.project_id, ExecutorKind::DevAgent).await?;
    pm_application::run_context::verify_lease(
        pool,
        args.project_id,
        args.run_id,
        &args.lease_token,
        args.lease_generation,
    )
    .await
    .map_err(map_lease_error_v2)?;
    pm_application::interaction::mark_consumed(
        pool,
        args.project_id,
        args.interaction_id,
        args.delivery_id,
        &args.evidence,
        &actor,
    )
    .await
    .map_err(map_interaction_error)?;
    Ok(json!({
        "interaction_id": args.interaction_id,
        "interaction_status": "consumed",
    }))
}

/// Arguments of `pm_discover_runs`：发现名下待恢复 Run（R-16 方案 a 发现面）。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiscoverRunsArgs {
    pub project_id: Uuid,
    /// 分页（1~100，默认 20）。只读、重复调用无状态副作用。
    #[serde(default = "default_discover_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_discover_limit() -> i64 {
    20
}

fn map_recovery_error(e: pm_application::recovery::RecoveryError) -> ToolError {
    use pm_application::recovery::RecoveryError;
    match e {
        RecoveryError::NotFound => {
            ToolError::NotFound(String::from("task or run not found in this project"))
        }
        RecoveryError::Rejected(msg) => ToolError::InvalidRequest(msg),
        other => ToolError::InvalidRequest(other.to_string()),
    }
}

/// 发现面（R-11/§3.6）：按稳定 Client 身份列出名下 `start_wait` 且有可投递
/// continue 命令的 Run 摘要（十字段；不含人工回复正文与 Command payload）。
/// 仅凭据通道可用（约束 A：发现绑定在 Client 身份上）；只读、分页、无副作用。
pub async fn discover_runs(
    pool: &PgPool,
    args: DiscoverRunsArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    let Some(channel) = caller.client_channel else {
        return Err(ToolError::Forbidden(
            "pm_discover_runs requires a client credential channel".into(),
        ));
    };
    ensure_member(pool, caller, args.project_id).await?;
    let runs = pm_application::recovery::discover_runs(
        pool,
        caller.human.tenant_id,
        args.project_id,
        channel.device_id,
        args.limit,
        args.offset,
    )
    .await
    .map_err(map_recovery_error)?;
    Ok(json!({ "runs": runs }))
}

/// Arguments of `pm_reacquire_lease`：同主体续领（R-16 方案 a；§3.6）。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReacquireLeaseArgs {
    pub project_id: Uuid,
    pub run_id: Uuid,
}

/// 同主体续领：同 Run 重签租约（generation+1 + 新 token + deadline 不动=T3）
/// + 非终局命令同事务改路由（reacquired_generation 写回）。不创建新
/// Session——continue 仍受情形 6 过滤（ACP-C4：terminate ACKED 是新
/// Session 启动前置）。
pub async fn reacquire_lease(
    pool: &PgPool,
    args: ReacquireLeaseArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    let Some(channel) = caller.client_channel else {
        return Err(ToolError::Forbidden(
            "pm_reacquire_lease requires a client credential channel".into(),
        ));
    };
    let actor = authorize(pool, caller, args.project_id, ExecutorKind::DevAgent).await?;
    let cfg = pm_application::run_dispatch::RunDispatchConfig::from_env();
    let lease = pm_application::recovery::reacquire_lease(
        pool,
        caller.human.tenant_id,
        args.project_id,
        args.run_id,
        channel.device_id,
        cfg.lease_secs,
        &actor,
    )
    .await
    .map_err(map_recovery_error)?;
    Ok(json!({
        "run_id": args.run_id,
        "lease_token": lease.lease_token,
        "lease_generation": lease.lease_generation,
        "worker_id": lease.worker_id,
        "deadline_at": lease.deadline_at,
        "rerouted_commands": lease.rerouted_commands,
        "note": "continue will surface via pm_poll_commands once terminate is confirmed (situation 6)",
    }))
}

/// Arguments of `pm_report_checkpoint`：Checkpoint 登记元数据上报（ACP-06；
/// 正文永远在 Workspace，Platform 仅登记；重复 (run_id, content_hash) 幂等）。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReportCheckpointArgs {
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub run_id: Uuid,
    /// Workspace 相对路径（惯例 `.aipm/checkpoint.json`）。
    pub file_path: String,
    pub content_hash: String,
    pub file_size: i64,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Checkpoint 登记（ACP-06 §1.3 executor_report 通道；Agent 侧事实，永无
/// 确认语义；无 Checkpoint 不阻止介入——本工具缺席时零影响）。
pub async fn report_checkpoint(
    pool: &PgPool,
    args: ReportCheckpointArgs,
    caller: &CallerContext,
) -> Result<Value, ToolError> {
    let actor = authorize(pool, caller, args.project_id, ExecutorKind::DevAgent).await?;
    let device = caller.claimed_device_id();
    let id = pm_application::acp06_07::register_checkpoint(
        pool,
        caller.human.tenant_id,
        args.project_id,
        args.task_id,
        args.run_id,
        None,
        &args.file_path,
        &args.content_hash,
        args.file_size,
        args.session_id.as_deref(),
        "executor_report",
        &actor,
    )
    .await
    .map_err(|e| ToolError::InvalidRequest(e.to_string()))?;
    let _ = device;
    Ok(json!({ "checkpoint_id": id, "replayed": false }))
}
