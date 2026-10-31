//! pm-api binary: axum REST facade over the idempotent ingestion service.
//!
//! Reads `DATABASE_URL` for the Postgres pool and `PM_API_PORT` (default 8090)
//! for the bind port. Bearer auth follows `AI_PM_AUTH_MODE`: `jwks` (default,
//! fail-closed) verifies tokens against the lmms token-service JWKS at
//! `AI_PM_JWKS_URL`; `off` serves the legacy unauthenticated router (local
//! development only). Tracing keeps the standard env-filter setup.

use std::sync::Arc;
use std::time::Duration;

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

    let mode = std::env::var("AI_PM_AUTH_MODE").unwrap_or_else(|_| "jwks".to_string());
    let app = if mode == "off" {
        tracing::warn!("AI_PM_AUTH_MODE=off — bearer auth disabled (local development only)");
        pm_api::routes::router(pool)
    } else {
        let jwks_url = std::env::var("AI_PM_JWKS_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8082/.well-known/jwks.json".to_string());
        let jwks = Arc::new(digua_common_jwt::JWKSCache::new(
            digua_common_jwt::JwksConfig {
                url: jwks_url,
                ..Default::default()
            },
            reqwest::Client::new(),
        ));
        // Fail-closed startup: wait briefly for the first JWKS fetch before
        // serving. If the fetch never lands, the cache has no keys and every
        // request 401s — never a silent pass-through.
        Arc::clone(&jwks)
            .init_with_refresh_loop(Duration::from_secs(30))
            .await;
        tracing::info!(mode = "jwks", "pm-api bearer auth enabled");
        pm_api::routes::router_with_auth(pool, jwks)
    };
    tracing::info!(port, "pm-api listening");

    axum::serve(listener, app).await?;
    Ok(())
}
