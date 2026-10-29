//! pm-api library target: exposes the axum [`routes::router`] so both the
//! binary (`main.rs`) and the integration tests (`tests/routes.rs`) drive the
//! exact same route table.

pub mod routes;

/// 前端壳与业务读端点（独立聚合，由 main.rs merge；见 shell_routes.rs 头注释）。
pub mod shell_routes;

