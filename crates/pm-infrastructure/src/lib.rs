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
pub mod repo_acceptance_read;
pub mod repo_ai_team_monitor;
pub mod repo_platform_ops;
pub mod repo_customer;
pub mod repo_guard_query;
pub mod repo_design_guard;
pub mod repo_kb_pc_query;
pub mod repo_runs;
pub mod repo_team_members;
pub mod repo_lists;
pub mod repo_viz_read;
pub mod viz_render_sandbox;
pub mod viz_seecode_adapter;
