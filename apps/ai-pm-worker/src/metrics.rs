//! PA08-06: loopback-only Prometheus exposition for Worker process counters.
//!
//! The listener binds to localhost by default. Metrics use bounded names and
//! contain no project, task, Run, repository, credential, or error labels.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug)]
pub struct WorkerMetrics {
    started_at_seconds: u64,
    ai_analysis_jobs_claimed: AtomicU64,
    repo_import_jobs_claimed: AtomicU64,
    run_dispatch_claim_attempts: AtomicU64,
    run_dispatch_errors: AtomicU64,
    run_poll_errors: AtomicU64,
    cleanup_errors: AtomicU64,
    scrapes: AtomicU64,
}

impl WorkerMetrics {
    pub fn new() -> Self {
        Self {
            started_at_seconds: unix_seconds(),
            ai_analysis_jobs_claimed: AtomicU64::new(0),
            repo_import_jobs_claimed: AtomicU64::new(0),
            run_dispatch_claim_attempts: AtomicU64::new(0),
            run_dispatch_errors: AtomicU64::new(0),
            run_poll_errors: AtomicU64::new(0),
            cleanup_errors: AtomicU64::new(0),
            scrapes: AtomicU64::new(0),
        }
    }

    pub fn ai_analysis_jobs_claimed(&self) {
        self.ai_analysis_jobs_claimed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn repo_import_jobs_claimed(&self) {
        self.repo_import_jobs_claimed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn run_dispatch_claim_attempt(&self) {
        self.run_dispatch_claim_attempts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn run_dispatch_error(&self) {
        self.run_dispatch_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn run_poll_error(&self) {
        self.run_poll_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn cleanup_error(&self) {
        self.cleanup_errors.fetch_add(1, Ordering::Relaxed);
    }

    fn render(&self) -> String {
        let uptime = unix_seconds().saturating_sub(self.started_at_seconds);
        let mut out = String::with_capacity(1024);
        metric(&mut out, "ai_pm_worker_uptime_seconds", "gauge", "Worker process uptime in seconds.", uptime);
        metric(&mut out, "ai_pm_worker_ai_analysis_jobs_claimed_total", "counter", "AI analysis jobs claimed by this Worker process.", self.ai_analysis_jobs_claimed.load(Ordering::Relaxed));
        metric(&mut out, "ai_pm_worker_repo_import_jobs_claimed_total", "counter", "Repository import jobs claimed by this Worker process.", self.repo_import_jobs_claimed.load(Ordering::Relaxed));
        metric(&mut out, "ai_pm_worker_run_dispatch_claim_attempts_total", "counter", "Run dispatch claim attempts made by this Worker process.", self.run_dispatch_claim_attempts.load(Ordering::Relaxed));
        metric(&mut out, "ai_pm_worker_run_dispatch_errors_total", "counter", "Run dispatch claim errors observed by this Worker process.", self.run_dispatch_errors.load(Ordering::Relaxed));
        metric(&mut out, "ai_pm_worker_run_poll_errors_total", "counter", "Active Run polling errors observed by this Worker process.", self.run_poll_errors.load(Ordering::Relaxed));
        metric(&mut out, "ai_pm_worker_cleanup_errors_total", "counter", "Process cleanup errors observed by this Worker process.", self.cleanup_errors.load(Ordering::Relaxed));
        metric(&mut out, "ai_pm_worker_metrics_scrapes_total", "counter", "Successful metrics responses served by this Worker process.", self.scrapes.load(Ordering::Relaxed));
        out
    }
}

fn metric(out: &mut String, name: &str, kind: &str, help: &str, value: u64) {
    use std::fmt::Write;
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} {kind}");
    let _ = writeln!(out, "{name} {value}");
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Serve a deliberately small HTTP surface on the configured loopback address.
pub async fn serve(
    address: &str,
    metrics: std::sync::Arc<WorkerMetrics>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(address).await?;
    tracing::info!(%address, "Worker loopback metrics listener started");
    loop {
        let (mut stream, _) = listener.accept().await?;
        let metrics = metrics.clone();
        tokio::spawn(async move {
            let mut request = [0_u8; 1024];
            let read = match stream.read(&mut request).await {
                Ok(read) => read,
                Err(error) => {
                    tracing::debug!(%error, "metrics request read failed");
                    return;
                }
            };
            let first_line = std::str::from_utf8(&request[..read])
                .ok()
                .and_then(|request| request.lines().next())
                .unwrap_or_default();
            let (status, body) = if first_line.starts_with("GET /metrics ") {
                metrics.scrapes.fetch_add(1, Ordering::Relaxed);
                ("200 OK", metrics.render())
            } else {
                ("404 Not Found", "not found\n".to_string())
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: text/plain; version=0.0.4; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            if let Err(error) = stream.write_all(response.as_bytes()).await {
                tracing::debug!(%error, "metrics response write failed");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::WorkerMetrics;

    #[test]
    fn exposition_has_bounded_worker_counters_without_entity_labels() {
        let metrics = WorkerMetrics::new();
        metrics.ai_analysis_jobs_claimed();
        metrics.repo_import_jobs_claimed();
        metrics.run_dispatch_claim_attempt();
        metrics.run_dispatch_error();
        metrics.run_poll_error();
        metrics.cleanup_error();

        let output = metrics.render();
        assert!(output.contains("ai_pm_worker_ai_analysis_jobs_claimed_total 1"));
        assert!(output.contains("ai_pm_worker_repo_import_jobs_claimed_total 1"));
        assert!(output.contains("ai_pm_worker_run_dispatch_claim_attempts_total 1"));
        assert!(output.contains("ai_pm_worker_run_dispatch_errors_total 1"));
        assert!(output.contains("ai_pm_worker_run_poll_errors_total 1"));
        assert!(output.contains("ai_pm_worker_cleanup_errors_total 1"));
        assert!(!output.contains("project_id"));
        assert!(!output.contains("run_id"));
        assert!(!output.contains("repository_url"));
    }
}
