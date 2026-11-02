//! rmcp server shell (Task 16): declares the three pinned MCP tools and
//! binds them to the function layer in [`crate::tools`], plus the streamable
//! HTTP service factory pm-api mounts under `/mcp`.
//!
//! The shell stays thin on purpose: every call goes through
//! [`PmMcpServer::tool_result`], which serializes the function layer's JSON
//! value as structured content and renders a [`tools::ToolError`] as a
//! caller-visible `isError` tool result (never a panic, never an unwrap).
//!
//! Transport (M1 decision): streamable HTTP in stateless JSON mode —
//! `legacy_session_mode(false)` + `json_response(true)` — so each POST gets a
//! plain JSON-RPC response and no session store is kept; `allowed_hosts`
//! stays at the loopback default because this server is local-only by
//! design. No MCP OAuth, no SSE notifications (M1 scope cut).

use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    service::RequestContext,
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;

use crate::tools::{
    self, ClaimTaskArgs, ConfirmDocumentProposalArgs, DiscoveryCompleteArgs, DiscoveryGetArgs,
    DiscoverySubmitAnswersArgs, DiscoverySubmitFindingsArgs, DiscoverySubmitProposalArgs,
    ProposeDocumentArgs, ReportProgressArgs, RunContextArgs, SubmissionStatusArgs,
    SubmitDocumentArgs, SubmitResultArgs, SubmitTechnicalDesignArgs, TaskContextArgs, ToolError,
    VetoDocumentRevisionArgs,
};

/// The MCP server over the shared pool: owns the rmcp tool router generated
/// from the `#[tool]` methods below.
#[derive(Clone)]
pub struct PmMcpServer {
    pool: PgPool,
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl PmMcpServer {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            tool_router: Self::tool_router(),
        }
    }

    /// One mapping for all tools: JSON value → structured tool result; tool
    /// error → `isError` result carrying the message text (the caller's MCP
    /// client renders it), with the detail in the log.
    fn tool_result(result: Result<Value, ToolError>) -> Result<CallToolResult, ErrorData> {
        match result {
            Ok(value) => Ok(CallToolResult::structured(value)),
            Err(err) => {
                tracing::warn!(error = %err, "mcp tool call failed");
                Ok(CallToolResult::error(vec![ContentBlock::text(
                    err.to_string(),
                )]))
            }
        }
    }

    #[tool(
        name = "pm_submit_technical_design",
        description = "Submit a technical design for one task. Persists an idempotent \
                       'technical_design_submitted' event and queues AI analysis; answers \
                       {event_id, processing_status}. The same (project_id, idempotency_key, \
                       payload) replays the original event."
    )]
    pub async fn submit_technical_design(
        &self,
        Parameters(args): Parameters<SubmitTechnicalDesignArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::submit_technical_design(&self.pool, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_get_submission_status",
        description = "Read the processing status of one submitted event: the event status, \
                       the proposals its analysis runs produced (id/status/command_type, ordered \
                       by proposal_seq) and the latest analysis run's status."
    )]
    pub async fn get_submission_status(
        &self,
        Parameters(args): Parameters<SubmissionStatusArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::get_submission_status(&self.pool, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_get_task_context",
        description = "Read the context snapshot of one task within a project: the task fields, \
                       the latest linked requirement revision and the milestone. A task of \
                       another project reads as not found."
    )]
    pub async fn get_task_context(
        &self,
        Parameters(args): Parameters<TaskContextArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::get_task_context(&self.pool, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_submit_document",
        description = "Create one Git-backed knowledge document (Markdown body + commit + \
                       revision row). Requires PM_MCP_DOCUMENT_WRITE. The project must already \
                       have an ai_pm principal; the same doc_key cannot be created twice."
    )]
    pub async fn submit_document(
        &self,
        Parameters(args): Parameters<SubmitDocumentArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let root = pm_application::knowledge::KnowledgeRoot::from_env();
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::submit_document(&self.pool, &root, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_propose_document",
        description = "Submit a Markdown proposal through the knowledge quality gates. A clean \
                       proposal is auto-committed to Git; a red-line one (external_commitment / \
                       delete_history / rewrite_delivered) is held pending human confirmation \
                       and is NOT written to Git. Requires PM_MCP_DOCUMENT_WRITE."
    )]
    pub async fn propose_document(
        &self,
        Parameters(args): Parameters<ProposeDocumentArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let root = pm_application::knowledge::KnowledgeRoot::from_env();
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::propose_document(&self.pool, &root, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_confirm_document_proposal",
        description = "Human confirmation of a red-line knowledge proposal; commits it to Git \
                       and records redline_confirmed_by. Requires PM_MCP_DOCUMENT_WRITE."
    )]
    pub async fn confirm_document_proposal(
        &self,
        Parameters(args): Parameters<ConfirmDocumentProposalArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let root = pm_application::knowledge::KnowledgeRoot::from_env();
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::confirm_document_proposal(&self.pool, &root, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_veto_document_revision",
        description = "Veto a published knowledge revision within the veto window: Git revert + \
                       revision rolled_back + pointer fallback + index re-follow + audit. Requires \
                       PM_MCP_DOCUMENT_WRITE."
    )]
    pub async fn veto_document_revision(
        &self,
        Parameters(args): Parameters<VetoDocumentRevisionArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let root = pm_application::knowledge::KnowledgeRoot::from_env();
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::veto_document_revision(&self.pool, &root, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_discovery_get",
        description = "Read one project discovery session (findings / questions / proposals). \
                       Omit session_id to resolve the project's live session. Answers not-found \
                       when the project has no session yet."
    )]
    pub async fn discovery_get(
        &self,
        Parameters(args): Parameters<DiscoveryGetArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::discovery_get(&self.pool, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_discovery_get_context",
        description = "Gather /pm-learn working context for one project: the discovery session \
                       summary, existing knowledge documents, linked repositories and the open \
                       questions waiting for user clarification."
    )]
    pub async fn discovery_get_context(
        &self,
        Parameters(args): Parameters<DiscoveryGetArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::discovery_get_context(&self.pool, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_discovery_submit_findings",
        description = "Submit AI analysis findings (observed / user_confirmed / speculation / \
                       contradicted) with evidence and optional repository links. Collection \
                       only: nothing is written to the official knowledge base."
    )]
    pub async fn discovery_submit_findings(
        &self,
        Parameters(args): Parameters<DiscoverySubmitFindingsArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::discovery_submit_findings(&self.pool, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_discovery_submit_answers",
        description = "Submit AI clarification questions and/or relay user answers into the \
                       discovery session. Collection only; final knowledge stays propose-only."
    )]
    pub async fn discovery_submit_answers(
        &self,
        Parameters(args): Parameters<DiscoverySubmitAnswersArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::discovery_submit_answers(&self.pool, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_discovery_submit_proposal",
        description = "Submit a knowledge-base change proposal from a discovery session. Routes \
                       through the existing propose_document chain (quality gates, red-line hold, \
                       base_revision_id optimistic concurrency) and mirrors the outcome onto a \
                       discovery_proposal row. Requires PM_MCP_DOCUMENT_WRITE."
    )]
    pub async fn discovery_submit_proposal(
        &self,
        Parameters(args): Parameters<DiscoverySubmitProposalArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let root = pm_application::knowledge::KnowledgeRoot::from_env();
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::discovery_submit_proposal(&self.pool, &root, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_discovery_complete",
        description = "Mark one discovery round as COMPLETED or REVIEW_REQUIRED. The user still \
                       approves proposals in the Web console; this only closes the collection \
                       round."
    )]
    pub async fn discovery_complete(
        &self,
        Parameters(args): Parameters<DiscoveryCompleteArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::discovery_complete(&self.pool, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_claim_task",
        description = "Claim the run of one specific task as an executor (Model A). Signs a \
                       one-time lease token (plaintext returned exactly once; only its sha256 \
                       is stored) and locks the per-repository baselines. Works regardless of \
                       AI_PM_RUN_DISPATCH (that switch only gates the unattended worker). A task \
                       with no runnable Run answers not-found; a non-ready task or an existing \
                       active run answers conflict."
    )]
    pub async fn claim_task(
        &self,
        Parameters(args): Parameters<ClaimTaskArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::claim_task(&self.pool, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_get_run_context",
        description = "Read the full execution contract of the run you hold: goal, constraints, \
                       acceptance criteria, deliverables, per-repository baselines, knowledge \
                       references, git branch and policy snapshot. Requires the lease token from \
                       pm_claim_task; it does not renew the lease. A wrong or expired token is \
                       rejected without echoing the stored hash."
    )]
    pub async fn get_run_context(
        &self,
        Parameters(args): Parameters<RunContextArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::get_run_context(&self.pool, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_report_progress",
        description = "Heartbeat and progress report for the run you hold: renews the lease \
                       (strictly forward), and on the first stage='started' also advances the \
                       task to running via the guarded state machine. Lease renewal is the only \
                       heartbeat; skipping reports lets the lease expire and the run be reclaimed. \
                       blocked=true records a run_blocked audit without changing task status."
    )]
    pub async fn report_progress(
        &self,
        Parameters(args): Parameters<ReportProgressArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::report_progress(&self.pool, args, &caller).await
            }
            .await,
        )
    }

    #[tool(
        name = "pm_submit_result",
        description = "Submit the delivery of one run: per-repository commits, changed-file \
                       counts and test evidence. The platform verifies each declared commit \
                       against the actual git workspace (it must equal the workspace HEAD and \
                       descend from the locked baseline), records the delivery manifest, and \
                       advances the task to submitted. A business rejection is answered with \
                       outcome='rejected' plus a machine-readable code, not a tool failure."
    )]
    pub async fn submit_result(
        &self,
        Parameters(args): Parameters<SubmitResultArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(
            async {
                let caller = crate::caller::caller_from_ctx(&ctx)?;
                tools::submit_result(&self.pool, args, &caller).await
            }
            .await,
        )
    }
}

#[tool_handler(router = self.tool_router, name = "ai-pm-mcp", version = "0.1.0")]
impl ServerHandler for PmMcpServer {}

impl PmMcpServer {
    /// The streamable HTTP service for pm-api's `/mcp` route: stateless JSON
    /// mode (no session store), loopback-only hosts — the M1 local server.
    pub fn streamable_http_service(
        pool: PgPool,
    ) -> StreamableHttpService<PmMcpServer, LocalSessionManager> {
        StreamableHttpService::new(
            move || Ok(PmMcpServer::new(pool.clone())),
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default()
                .with_legacy_session_mode(false)
                .with_json_response(true),
        )
    }
}
