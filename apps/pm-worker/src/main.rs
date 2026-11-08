//! `pm-worker`: claims outbox events and dispatches them to registered
//! handlers.
//!
//! **统一 Handler 装配**（T-061 §6.2.1）：`pm-worker` 与 `ai-pm-worker` 共享
//! 同一张 outbox 表——任一实例抢到的事件都必须能被正确消费。零 Handler 空壳
//! 参与共享队列领取，会把另一 Worker **可正常处理**的事件经重试误判为死信
//! （**假死信**，非业务失败）。因此两个 Worker 都经同一个入口
//! `register_all_handlers` 装配，注册集合必须覆盖 `EventCatalog` 中
//! `has_handler = true` 的全集——由启动时的目录一致性校验保证。

use pm_infrastructure::outbox_handlers::register_all_handlers;
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
    // P2-13/D-1=A（2026-11-08）：仓库快照分析的 LLM 增强模型（与 ai-pm-worker
    // 同构装配——共享 outbox 表，两 worker 的 handler 语义必须一致）。
    // `AI_PM_SNAPSHOT_ANALYSIS_MODEL=auto|derived`（默认 auto）。
    let snapshot_model: std::sync::Arc<
        dyn pm_application::snapshot_analysis::SnapshotAnalysisModel,
    > = std::sync::Arc::new(pm_ai::snapshot_model::LlmSnapshotAnalysisModel::from_env(
        pool.clone(),
    ));
    let worker = register_all_handlers(
        OutboxWorker::new(pool.clone(), worker_id.clone()),
        &pool,
        Some(snapshot_model),
    );
    // 目录一致性校验：不一致即 panic（fail fast），不带着假死信配置进入消费循环。
    worker.assert_catalog_consistent();
    tracing::info!(
        service = "pm-worker",
        %worker_id,
        handlers = ?worker.handled_event_types(),
        "started"
    );
    worker.run().await;
}
