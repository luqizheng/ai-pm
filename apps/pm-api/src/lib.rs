//! pm-api library target: exposes the axum [`routes::router`] so both the
//! binary (`main.rs`) and the integration tests (`tests/routes.rs`) drive the
//! exact same route table.

pub mod routes;

