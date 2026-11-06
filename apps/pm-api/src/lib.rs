//! pm-api library target: exposes the axum [`routes::router`] so both the
//! binary (`main.rs`) and the integration tests (`tests/routes.rs`) drive the
//! exact same route table.

pub mod audit_stream;
pub mod auth;
pub mod command_center;
pub mod customers;
pub mod device_claim;
pub mod documents;
pub mod team_llm;
pub mod team_llm_guard;
pub mod team_onboarding;
pub mod mcp_hosts;
pub mod outbox_ops;
pub mod onboarding;
pub mod pm_acceptance;
pub mod pm_artifacts;
pub mod pm_approvers;
pub mod pm_baseline;
pub mod pm_delivery;
pub mod pm_devices;
pub mod pm_discovery;
pub mod pm_imports;
pub mod pm_cognition;
pub mod pm_requirements;
pub mod pm_lists;
pub mod pm_repositories;
pub mod pm_reviews;
pub mod pm_runs;
pub mod pm_writes;
pub mod principals;
pub mod project_lifecycle;
pub mod project_members;
pub mod routes;
pub mod skills_package;
pub mod support;
pub mod task_dependencies;
pub mod tenant_members;

