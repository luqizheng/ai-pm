//! pm-api library target: exposes the axum [`routes::router`] so both the
//! binary (`main.rs`) and the integration tests (`tests/routes.rs`) drive the
//! exact same route table.

pub mod audit_stream;
pub mod auth;
pub mod customers;
pub mod documents;
pub mod llm_configs;
pub mod pm_artifacts;
pub mod pm_imports;
pub mod pm_lists;
pub mod pm_repositories;
pub mod pm_runs;
pub mod pm_writes;
pub mod principals;
pub mod routes;
pub mod support;

