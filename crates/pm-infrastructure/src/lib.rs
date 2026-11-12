//! pm-infrastructure: database repositories, external integrations, adapters.

pub mod codex_executor;
pub mod docproc_store;
pub mod docproc_worker;
pub mod job_queue;
pub mod kb_pc_analysis;
pub mod kb_pc_handlers;
pub mod outbox;
pub mod outbox_handlers;
pub mod outbox_worker;
pub mod repo_baseline;
pub mod repo_cognition;
pub mod repo_identity;
pub mod repo_design_guard;
pub mod repo_kb_pc_query;
pub mod repo_runs;
pub mod repo_tenant_members;
pub mod repo_lists;

