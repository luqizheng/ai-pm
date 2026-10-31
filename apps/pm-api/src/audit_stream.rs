//! Audit SSE stream: PG LISTEN/NOTIFY (`pm_audit`) -> in-process broadcast ->
//! `text/event-stream`.
//!
//! `governance.audit_log` fires `pg_notify` on every INSERT (migration 0005),
//! so this feed covers every writer without touching business code. The
//! listener owns one dedicated connection and reconnects with exponential
//! backoff; SSE subscribers only depend on the broadcast channel, so a PG blip
//! never drops the HTTP stream.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use tokio::sync::broadcast;
use tokio_stream::Stream;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::routes::AppState;

/// Notification channel shared with `governance.notify_audit_insert()`.
pub const CHANNEL: &str = "pm_audit";
const CHANNEL_CAPACITY: usize = 256;
const KEEPALIVE: Duration = Duration::from_secs(15);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Wire shape consumed by `frontend/src/api/types.ts::AuditLog`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditLog {
    pub id: String,
    pub project_id: String,
    pub actor_id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub created_at: String,
}

/// Process-wide fan-out handle. Cloneable; subscribers come and go freely.
#[derive(Clone)]
pub struct AuditFeed {
    tx: broadcast::Sender<Arc<AuditLog>>,
}

impl Default for AuditFeed {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditFeed {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<AuditLog>> {
        self.tx.subscribe()
    }

    pub fn publish(&self, log: AuditLog) {
        // Err only means "no subscribers right now" — normal, not a failure.
        let _ = self.tx.send(Arc::new(log));
    }
}

/// Parses one `pg_notify` payload. Malformed rows are logged and dropped so a
/// single bad writer can never kill the stream.
pub fn parse_notification(payload: &str) -> Option<AuditLog> {
    match serde_json::from_str::<AuditLog>(payload) {
        Ok(log) => Some(log),
        Err(err) => {
            tracing::warn!(error = %err, "dropping malformed pm_audit payload");
            None
        }
    }
}

/// Dedicated LISTEN loop. Reconnects with 1s -> 30s exponential backoff and
/// re-subscribes on every successful connect.
pub async fn run_listener(pool: PgPool, feed: AuditFeed) {
    let mut backoff = Duration::from_secs(1);
    loop {
        match PgListener::connect_with(&pool).await {
            Ok(mut listener) => {
                if let Err(err) = listener.listen(CHANNEL).await {
                    tracing::warn!(error = %err, "pm_audit LISTEN failed; retrying");
                } else {
                    tracing::info!(channel = CHANNEL, "pm_audit listener connected");
                    backoff = Duration::from_secs(1);
                    loop {
                        match listener.recv().await {
                            Ok(notification) => {
                                if let Some(log) = parse_notification(notification.payload()) {
                                    feed.publish(log);
                                }
                            }
                            Err(err) => {
                                tracing::warn!(error = %err, "pm_audit listener dropped");
                                break;
                            }
                        }
                    }
                }
            }
            Err(err) => tracing::warn!(error = %err, "pm_audit connect failed"),
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

/// `GET /v1/pm/events/stream` — one `event: audit` frame per new audit row.
pub(crate) async fn stream_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.audit_feed.subscribe();
    let stream = BroadcastStream::new(receiver).filter_map(|item| match item {
        Ok(log) => Event::default()
            .event("audit")
            .json_data(&*log)
            .ok()
            .map(|event| Ok::<_, Infallible>(event)),
        Err(err) => {
            // A lagging client skips frames; the stream itself stays open.
            tracing::warn!(error = %err, "sse subscriber lagged; frame skipped");
            None
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(KEEPALIVE).text("keepalive"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(target_id: Option<&str>) -> String {
        serde_json::json!({
            "id": "11111111-1111-1111-1111-111111111111",
            "project_id": "22222222-2222-2222-2222-222222222222",
            "actor_id": "33333333-3333-3333-3333-333333333333",
            "action": "task.create",
            "target_type": "task",
            "target_id": target_id,
            "created_at": "2026-10-31T03:00:00.000000Z"
        })
        .to_string()
    }

    #[test]
    fn parses_full_payload() {
        let log = parse_notification(&sample(Some("44444444-4444-4444-4444-444444444444"))).unwrap();
        assert_eq!(log.action, "task.create");
        assert_eq!(log.target_type, "task");
        assert_eq!(
            log.target_id.as_deref(),
            Some("44444444-4444-4444-4444-444444444444")
        );
    }

    #[test]
    fn parses_null_target_id() {
        let log = parse_notification(&sample(None)).unwrap();
        assert_eq!(log.target_id, None);
    }

    #[test]
    fn malformed_payload_is_dropped() {
        assert!(parse_notification("{\"id\":").is_none());
        assert!(parse_notification("not json").is_none());
    }

    #[tokio::test]
    async fn feed_fans_out_to_subscribers() {
        let feed = AuditFeed::new();
        let mut rx = feed.subscribe();
        feed.publish(AuditLog {
            id: "1".into(),
            project_id: "2".into(),
            actor_id: "3".into(),
            action: "task.create".into(),
            target_type: "task".into(),
            target_id: None,
            created_at: "2026-10-31T03:00:00.000000Z".into(),
        });
        let got = rx.recv().await.unwrap();
        assert_eq!(got.action, "task.create");
    }

    #[tokio::test]
    async fn publish_without_subscribers_does_not_panic() {
        AuditFeed::new().publish(AuditLog {
            id: "1".into(),
            project_id: "2".into(),
            actor_id: "3".into(),
            action: "noop".into(),
            target_type: "task".into(),
            target_id: None,
            created_at: "2026-10-31T03:00:00.000000Z".into(),
        });
    }
}