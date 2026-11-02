//! pm-mcp: MCP server tools exposed to dev agents (Task 16).
//!
//! Two layers:
//!
//! - [`tools`] is the function layer — plain async functions over
//!   `&PgPool` returning JSON values or a [`tools::ToolError`], directly
//!   testable against the database without any MCP machinery. All three tools
//!   reuse the application services: `ingestion::submit_event`,
//!   `read::task_context` and plain SQL over the `ai` schema.
//! - [`server`] is the rmcp shell — a `ServerHandler` whose `#[tool_router]`
//!   registers the three pinned tool names (`pm_submit_technical_design`,
//!   `pm_get_submission_status`, `pm_get_task_context`) and binds them to the
//!   function layer, plus the streamable HTTP service factory that pm-api
//!   mounts under `/mcp`.

pub mod caller;
pub mod server;
pub mod tools;

pub use server::PmMcpServer;
