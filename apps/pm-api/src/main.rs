//! pm-api binary: axum REST facade over the idempotent ingestion service.
//!
//! Reads `DATABASE_URL` for the Postgres pool and `PM_API_PORT` (default 8090)
//! for the bind port; tracing keeps the standard env-filter setup.

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set, e.g. postgres://aipm:aipm@localhost:5433/aipm");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("../../migrations").run(&pool).await?;

    let port: u16 = std::env::var("PM_API_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8090);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(port, "pm-api listening");

    axum::serve(listener, pm_api::routes::router(pool)).await?;
    Ok(())
}
