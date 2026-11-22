//! Optional loopback Prometheus exporter for the Runner process.
//! The listener is disabled unless `AI_PM_RUNNER_METRICS_ADDR` is set.
//! Labels are a closed vocabulary; no IDs, URLs, tokens, or raw errors are exposed.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const OPERATIONS: [&str; 8] = [
    "claim",
    "discover",
    "heartbeat",
    "command_poll",
    "command_ack",
    "lease_reacquire",
    "interaction",
    "other",
];

pub struct RunnerMetrics {
    started_at: u64,
    operations: [[AtomicU64; 2]; 8],
    supervision_errors: AtomicU64,
    cleanup_successes: AtomicU64,
    cleanup_errors: AtomicU64,
    scrapes: AtomicU64,
}

impl RunnerMetrics {
    fn new() -> Self {
        Self {
            started_at: unix_seconds(),
            operations: [
                [AtomicU64::new(0), AtomicU64::new(0)],
                [AtomicU64::new(0), AtomicU64::new(0)],
                [AtomicU64::new(0), AtomicU64::new(0)],
                [AtomicU64::new(0), AtomicU64::new(0)],
                [AtomicU64::new(0), AtomicU64::new(0)],
                [AtomicU64::new(0), AtomicU64::new(0)],
                [AtomicU64::new(0), AtomicU64::new(0)],
                [AtomicU64::new(0), AtomicU64::new(0)],
            ],
            supervision_errors: AtomicU64::new(0),
            cleanup_successes: AtomicU64::new(0),
            cleanup_errors: AtomicU64::new(0),
            scrapes: AtomicU64::new(0),
        }
    }

    fn record_operation(&self, operation: &str, success: bool) {
        let index = OPERATIONS.iter().position(|candidate| *candidate == operation).unwrap_or(6);
        self.operations[index][usize::from(!success)].fetch_add(1, Ordering::Relaxed);
    }

    fn render(&self) -> String {
        let mut out = String::with_capacity(2048);
        metric(&mut out, "ai_pm_runner_uptime_seconds", "gauge", "Runner process uptime in seconds.", unix_seconds().saturating_sub(self.started_at));
        let _ = writeln!(out, "# HELP ai_pm_runner_mcp_operations_total Runner MCP tool calls by bounded operation and outcome.");
        let _ = writeln!(out, "# TYPE ai_pm_runner_mcp_operations_total counter");
        for (index, operation) in OPERATIONS.iter().enumerate() {
            for (outcome_index, outcome) in ["success", "error"].iter().enumerate() {
                let value = self.operations[index][outcome_index].load(Ordering::Relaxed);
                sample(&mut out, "ai_pm_runner_mcp_operations_total", &[ ("operation", operation), ("outcome", outcome) ], value);
            }
        }
        metric(&mut out, "ai_pm_runner_supervision_errors_total", "counter", "Runner supervision errors observed by this process.", self.supervision_errors.load(Ordering::Relaxed));
        metric(&mut out, "ai_pm_runner_cleanup_terminations_total", "counter", "Orphan and lease-loss Agent termination attempts completed successfully.", self.cleanup_successes.load(Ordering::Relaxed));
        metric(&mut out, "ai_pm_runner_cleanup_errors_total", "counter", "Orphan and lease-loss Agent termination attempts that could not be confirmed.", self.cleanup_errors.load(Ordering::Relaxed));
        metric(&mut out, "ai_pm_runner_metrics_scrapes_total", "counter", "Successful metrics responses served by this Runner process.", self.scrapes.load(Ordering::Relaxed));
        out
    }
}

static METRICS: OnceLock<RunnerMetrics> = OnceLock::new();

fn metrics() -> &'static RunnerMetrics {
    METRICS.get_or_init(RunnerMetrics::new)
}

pub fn record_mcp_operation(operation: &str, success: bool) {
    metrics().record_operation(operation, success);
}

pub fn record_supervision_error() {
    metrics().supervision_errors.fetch_add(1, Ordering::Relaxed);
}

pub fn record_cleanup_attempt(success: bool) {
    let counter = if success { &metrics().cleanup_successes } else { &metrics().cleanup_errors };
    counter.fetch_add(1, Ordering::Relaxed);
}

/// Start the optional HTTP exporter inside the active Tokio runtime.
pub fn start_from_env() {
    let Ok(address) = std::env::var("AI_PM_RUNNER_METRICS_ADDR") else {
        return;
    };
    let allow_remote = std::env::var("AI_PM_RUNNER_METRICS_ALLOW_REMOTE")
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let parsed: SocketAddr = match address.parse() {
        Ok(address) => address,
        Err(error) => {
            tracing::warn!(%error, "invalid AI_PM_RUNNER_METRICS_ADDR; metrics exporter disabled");
            return;
        }
    };
    if !parsed.ip().is_loopback() && !allow_remote {
        tracing::warn!(%parsed, "Runner metrics exporter must bind loopback unless AI_PM_RUNNER_METRICS_ALLOW_REMOTE=true");
        return;
    }
    tokio::spawn(async move {
        match TcpListener::bind(parsed).await {
            Ok(listener) => {
                tracing::info!(address = %parsed, "Runner metrics exporter listening");
                if let Err(error) = serve_listener(listener).await {
                    tracing::error!(%error, "Runner metrics exporter stopped");
                }
            }
            Err(error) => tracing::error!(%error, address = %parsed, "Runner metrics exporter failed to bind"),
        }
    });
}

async fn serve_listener(listener: TcpListener) -> std::io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(error) = serve_connection(stream).await {
                tracing::debug!(%error, "Runner metrics request failed");
            }
        });
    }
}

async fn serve_connection(mut stream: TcpStream) -> std::io::Result<()> {
    let mut request = [0_u8; 1024];
    let read = stream.read(&mut request).await?;
    let first_line = std::str::from_utf8(&request[..read])
        .ok()
        .and_then(|request| request.lines().next())
        .unwrap_or_default();
    let (status, body) = if first_line.starts_with("GET /metrics ") {
        metrics().scrapes.fetch_add(1, Ordering::Relaxed);
        ("200 OK", metrics().render())
    } else {
        ("404 Not Found", "not found\n".to_string())
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; version=0.0.4; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await
}

fn metric(out: &mut String, name: &str, kind: &str, help: &str, value: u64) {
    use std::fmt::Write;
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} {kind}");
    let _ = writeln!(out, "{name} {value}");
}

fn sample(out: &mut String, name: &str, labels: &[(&str, &str)], value: u64) {
    use std::fmt::Write;
    let _ = write!(out, "{name}{{");
    for (index, (key, label_value)) in labels.iter().enumerate() {
        if index > 0 { let _ = write!(out, ","); }
        let _ = write!(out, "{key}=\"{label_value}\"");
    }
    let _ = writeln!(out, "}} {value}");
}

fn unix_seconds() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{RunnerMetrics, OPERATIONS};

    #[tokio::test]
    async fn optional_exporter_serves_prometheus_http_and_rejects_other_paths() {
        use super::serve_listener;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(serve_listener(listener));

        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n").await.unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Content-Type: text/plain; version=0.0.4"));
        assert!(response.contains("# TYPE ai_pm_runner_mcp_operations_total counter"));

        let mut missing = TcpStream::connect(address).await.unwrap();
        missing.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await.unwrap();
        let mut missing_response = String::new();
        missing.read_to_string(&mut missing_response).await.unwrap();
        assert!(missing_response.starts_with("HTTP/1.1 404 Not Found"));
        server.abort();
    }

    #[test]
    fn exposition_has_bounded_operation_labels_and_no_entity_ids() {
        let metrics = RunnerMetrics::new();
        metrics.record_operation("claim", true);
        metrics.record_operation("command_ack", false);
        metrics.supervision_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let output = metrics.render();
        assert!(output.contains("ai_pm_runner_mcp_operations_total{operation=\"claim\",outcome=\"success\"} 1"));
        assert!(output.contains("ai_pm_runner_mcp_operations_total{operation=\"command_ack\",outcome=\"error\"} 1"));
        assert!(output.contains("ai_pm_runner_supervision_errors_total 1"));
        assert!(output.ends_with('\n'));
        assert!(!output.contains("project_id"));
        assert!(!output.contains("run_id"));
        assert!(!output.contains("lease_token"));
        assert_eq!(OPERATIONS.len(), 8);
    }
}
