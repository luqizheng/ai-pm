//! pm-api library target: exposes the axum [`routes::router`] so both the
//! binary (`main.rs`) and the integration tests (`tests/routes.rs`) drive the
//! exact same route table.

// 本 crate 大量 handler/辅助函数以 axum `Response` 直接作 `Err` 类型
// （提前返回现成 HTTP 响应，IntoResponse 面零转换）。`Response<Body>` 天然
// ≥128B，触发 result_large_err；装箱 Response 属反模式，整层显式豁免。
#![allow(clippy::result_large_err)]

pub mod admin_config;
pub mod ai_conversations;
pub mod ai_context_items;
pub mod ai_roles;
pub mod ai_team_monitor;
pub mod ai_team_policies;
pub mod audit_stream;
pub mod auth;
pub mod command_center;
pub mod customers;
pub mod docproc;
pub mod device_claim;
pub mod documents;
pub mod files;
pub mod intakes;
pub mod kb_memory_api;
pub mod knowledge_services;
pub mod mcp_hosts;
pub mod onboarding;
pub mod outbox_ops;
pub mod pm_acceptance;
pub mod pm_approvers;
pub mod pm_artifacts;
pub mod pm_baseline;
pub mod pm_cognition;
pub mod pm_delivery;
pub mod pm_designs;
pub mod pm_devices;
pub mod pm_discovery;
pub mod pm_execution;
pub mod pm_imports;
pub mod pm_interactions;
pub mod pm_lists;
pub mod pm_plans;
pub mod pm_repositories;
pub mod pm_requirement_read;
pub mod pm_requirements;
pub mod pm_reviews;
pub mod pm_runs;
pub mod pm_tasks;
pub mod pm_writes;
pub mod principals;
pub mod project_lifecycle;
pub mod project_members;
pub mod retrieval;
pub mod routes;
pub mod setup;
pub mod skills_package;
pub mod support;
pub mod task_dependencies;
pub mod task_repositories;
pub mod team_llm;
pub mod team_llm_catalog;
pub mod team_llm_guard;
pub mod team_onboarding;
pub mod tenant_members;
pub mod viz_handlers;
