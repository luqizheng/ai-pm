//! Outbox dispatch loop: a handler registry on top of the `outbox`
//! state machine (task state machine spec §20.3, §20.6).
//!
//! `dispatch_once` is the single step: claim one due event, route it
//! through the registry, and settle it. An event type with no
//! registered handler is **not** silently marked delivered — it goes
//! straight to the dead letter bin with a `no handler registered for
//! <type>` reason, so first-version workers stay honest about what was
//! and was not processed (decision D4) and the event stays replayable
//! once a real handler ships.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::outbox::{self, ClaimedOutboxEvent, OutboxFailOutcome};

/// Idle sleep between empty claims.
pub const OUTBOX_IDLE_POLL_MS: u64 = 100;

/// A port that consumes one outbox event. Handlers must be idempotent —
/// delivery is at-least-once, so a redelivered event may arrive twice.
#[async_trait::async_trait]
pub trait OutboxHandler: Send + Sync {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String>;
}

/// What one `dispatch_once` step did.
#[derive(Debug, PartialEq, Eq)]
pub enum DispatchOutcome {
    Delivered,
    Retried,
    DeadLettered,
    Idle,
}

/// Registry-based outbox dispatcher.
pub struct OutboxWorker {
    pool: sqlx::PgPool,
    worker_id: String,
    handlers: HashMap<&'static str, Arc<dyn OutboxHandler>>,
    lease_secs: i64,
    max_attempts: i32,
    backoff_base_secs: i64,
}

impl OutboxWorker {
    /// A worker with the crate-default retry budget
    /// (`OUTBOX_LEASE_SECS` / `OUTBOX_MAX_ATTEMPTS` /
    /// `OUTBOX_BACKOFF_BASE_SECS`) and no handlers.
    pub fn new(pool: sqlx::PgPool, worker_id: String) -> Self {
        Self {
            pool,
            worker_id,
            handlers: HashMap::new(),
            lease_secs: outbox::OUTBOX_LEASE_SECS,
            max_attempts: outbox::OUTBOX_MAX_ATTEMPTS,
            backoff_base_secs: outbox::OUTBOX_BACKOFF_BASE_SECS,
        }
    }

    /// Register `handler` for `event_type`. Later registrations for the
    /// same type replace earlier ones.
    pub fn with_handler(
        mut self,
        event_type: &'static str,
        handler: Arc<dyn OutboxHandler>,
    ) -> Self {
        self.handlers.insert(event_type, handler);
        self
    }

    /// Override the retry budget. Production wiring keeps the crate
    /// defaults; tests shrink the ceiling to exercise the dead-letter
    /// path without waiting out five backoff cycles.
    pub fn with_retry_limits(mut self, max_attempts: i32, backoff_base_secs: i64) -> Self {
        self.max_attempts = max_attempts;
        self.backoff_base_secs = backoff_base_secs;
        self
    }

    /// Claim and settle at most one event.
    pub async fn dispatch_once(&self) -> Result<DispatchOutcome, sqlx::Error> {
        let Some(event) = outbox::claim(&self.pool, &self.worker_id, self.lease_secs).await
        else {
            return Ok(DispatchOutcome::Idle);
        };
        match self.handlers.get(event.event_type.as_str()) {
            Some(handler) => match handler.handle(&event).await {
                Ok(()) => {
                    outbox::complete(&self.pool, event.id).await?;
                    Ok(DispatchOutcome::Delivered)
                }
                Err(msg) => {
                    let outcome = outbox::fail(
                        &self.pool,
                        event.id,
                        &msg,
                        self.max_attempts,
                        self.backoff_base_secs,
                    )
                    .await?;
                    Ok(match outcome {
                        OutboxFailOutcome::Retried => DispatchOutcome::Retried,
                        OutboxFailOutcome::DeadLettered => DispatchOutcome::DeadLettered,
                    })
                }
            },
            None => {
                outbox::dead_letter(
                    &self.pool,
                    event.id,
                    &format!("no handler registered for {}", event.event_type),
                )
                .await?;
                Ok(DispatchOutcome::DeadLettered)
            }
        }
    }

    /// Run the dispatch loop forever: busy when events flow, a short
    /// sleep when idle or on dispatch errors (which are logged and
    /// retried — a transient DB blip must not kill the worker).
    pub async fn run(self) -> std::convert::Infallible {
        loop {
            match self.dispatch_once().await {
                Ok(DispatchOutcome::Idle) => {
                    tokio::time::sleep(Duration::from_millis(OUTBOX_IDLE_POLL_MS)).await
                }
                Ok(_) => continue,
                Err(e) => {
                    tracing::warn!(
                        worker = %self.worker_id,
                        error = %e,
                        "outbox dispatch failed; backing off"
                    );
                    tokio::time::sleep(Duration::from_millis(OUTBOX_IDLE_POLL_MS)).await
                }
            }
        }
    }
}