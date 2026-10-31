//! `pm-worker`: claims outbox events and dispatches them to registered
//! handlers. First version registers no handlers — every claimed event
//! dead-letters with `no handler registered for <type>` and stays
//! visible for manual `replay` (decision D4); real handlers arrive with
//! Slice C/D (MCP scheduling, knowledge write-back).

use pm_infrastructure::outbox_worker::OutboxWorker;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("database pool");

    let worker_id = format!("pm-worker-{}", uuid::Uuid::new_v4());
    // 首版不注册任何 handler：事件一律死信并保留 last_error 供 replay，
    // 真实 handler（MCP 调度 / 知识回写）在 Slice C/D 经 with_handler 注册。
    let worker = OutboxWorker::new(pool, worker_id.clone());
    tracing::info!(service = "pm-worker", %worker_id, "started");
    worker.run().await;
}
