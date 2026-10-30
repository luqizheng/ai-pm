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
    ErrorData, ServerHandler,
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;

use crate::tools::{
    self, SubmissionStatusArgs, SubmitTechnicalDesignArgs, TaskContextArgs, ToolError,
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
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(tools::submit_technical_design(&self.pool, args).await)
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
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(tools::get_submission_status(&self.pool, args).await)
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
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_result(tools::get_task_context(&self.pool, args).await)
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
