//! PA08-06: bounded Prometheus exposition backed by authoritative PostgreSQL aggregates.
//!
//! Only constrained enum dimensions are emitted as labels. Entity IDs, user
//! content, repository URLs, tokens, and diagnostic strings never enter metrics.

use sqlx::PgPool;

fn escape_label(value: &str) -> String {
    value.replace('\\', r"\\").replace('\n', r"\n").replace('"', r#"\""#)
}

pub async fn render(pool: &PgPool) -> Result<String, sqlx::Error> {
    let mut out = String::with_capacity(2048);

    let runs: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT run_kind, status, count(*)::bigint, \
                COALESCE(EXTRACT(EPOCH FROM (now() - MIN(COALESCE(started_at, claimed_at, created_at)) \
                    FILTER (WHERE status IN ('claimed', 'running'))))::bigint, 0) \
           FROM delivery.task_run \
          GROUP BY run_kind, status \
          ORDER BY run_kind, status",
    )
    .fetch_all(pool)
    .await?;
    out.push_str("# HELP ai_pm_run_count Number of Runs by kind and status.\n");
    out.push_str("# TYPE ai_pm_run_count gauge\n");
    out.push_str("# HELP ai_pm_active_run_oldest_age_seconds Age of the oldest active Run by kind and status.\n");
    out.push_str("# TYPE ai_pm_active_run_oldest_age_seconds gauge\n");
    for (kind, status, count, age) in runs {
        let kind = escape_label(&kind);
        let status = escape_label(&status);
        out.push_str(&format!(
            "ai_pm_run_count{{run_kind=\"{kind}\",status=\"{status}\"}} {count}\n"
        ));
        if status == "claimed" || status == "running" {
            out.push_str(&format!(
                "ai_pm_active_run_oldest_age_seconds{{run_kind=\"{kind}\",status=\"{status}\"}} {age}\n"
            ));
        }
    }

    let cleanup: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, count(*)::bigint \
           FROM delivery.task_run_cleanup \
          GROUP BY status ORDER BY status",
    )
    .fetch_all(pool)
    .await?;
    out.push_str("# HELP ai_pm_run_cleanup_backlog Number of process-cleanup records by status.\n");
    out.push_str("# TYPE ai_pm_run_cleanup_backlog gauge\n");
    for (status, count) in cleanup {
        let status = escape_label(&status);
        out.push_str(&format!(
            "ai_pm_run_cleanup_backlog{{status=\"{status}\"}} {count}\n"
        ));
    }

    let reclamation: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT resource_kind, status, count(*)::bigint \\
           FROM delivery.workspace_reclamation \\
          GROUP BY resource_kind, status ORDER BY resource_kind, status",
    )
    .fetch_all(pool)
    .await?;
    out.push_str("# HELP ai_pm_workspace_reclamation_backlog Number of reclamation records by resource kind and status.\\n");
    out.push_str("# TYPE ai_pm_workspace_reclamation_backlog gauge\\n");
    for (kind, status, count) in reclamation {
        let kind = escape_label(&kind);
        let status = escape_label(&status);
        out.push_str(&format!(
            "ai_pm_workspace_reclamation_backlog{{resource_kind=\\\"{kind}\\\",status=\\\"{status}\\\"}} {count}\\n"
        ));
    }

    let reclaimed: Vec<(String, i64)> = sqlx::query_as(
        "SELECT resource_kind, total_reclaimed_bytes \\
           FROM delivery.workspace_reclamation_totals \\
          ORDER BY resource_kind",
    )
    .fetch_all(pool)
    .await?;
    append_reclaimed_bytes_metric(&mut out, &reclaimed);

    let comm = pm_application::comm_overview::load_comm_overview(pool).await?;
    out.push_str("# HELP ai_pm_pending_worker_commands Number of issued or delivered Worker commands.\n");
    out.push_str("# TYPE ai_pm_pending_worker_commands gauge\n");
    out.push_str(&format!(
        "ai_pm_pending_worker_commands {}\n",
        comm.worker_command_pending_total
    ));
    out.push_str("# HELP ai_pm_oldest_pending_worker_command_age_seconds Age of the oldest unacknowledged Worker command.\n");
    out.push_str("# TYPE ai_pm_oldest_pending_worker_command_age_seconds gauge\n");
    out.push_str(&format!(
        "ai_pm_oldest_pending_worker_command_age_seconds {}\n",
        comm.worker_command_pending_age_seconds
    ));
    out.push_str("# HELP ai_pm_interaction_shielded_runs Number of Runs with scheduling shields.\n");
    out.push_str("# TYPE ai_pm_interaction_shielded_runs gauge\n");
    out.push_str(&format!(
        "ai_pm_interaction_shielded_runs {}\n",
        comm.interaction_shielded_runs
    ));
    out.push_str("# HELP ai_pm_interaction_unconsumed_responses Number of interaction responses awaiting consumption.\n");
    out.push_str("# TYPE ai_pm_interaction_unconsumed_responses gauge\n");
    out.push_str(&format!(
        "ai_pm_interaction_unconsumed_responses {}\n",
        comm.interaction_unconsumed_total
    ));
    out.push_str("# HELP ai_pm_interaction_open_chains Number of open interaction chains.\n");
    out.push_str("# TYPE ai_pm_interaction_open_chains gauge\n");
    out.push_str(&format!(
        "ai_pm_interaction_open_chains {}\n",
        comm.interaction_open_chains
    ));

    Ok(out)
}

fn append_reclaimed_bytes_metric(out: &mut String, reclaimed: &[(String, i64)]) {
    out.push_str("# HELP ai_pm_workspace_reclaimed_bytes_total Cumulative logical file bytes removed by workspace reclamation.\\n");
    out.push_str("# TYPE ai_pm_workspace_reclaimed_bytes_total counter\\n");
    for (kind, total_bytes) in reclaimed {
        let kind = escape_label(kind);
        out.push_str(&format!(
            "ai_pm_workspace_reclaimed_bytes_total{{resource_kind=\\\"{kind}\\\"}} {total_bytes}\\n"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{append_reclaimed_bytes_metric, escape_label};

    #[test]
    fn label_escaping_covers_prometheus_special_characters() {
        assert_eq!(escape_label("a\\b\n\"c"), r#"a\\b\n\"c"#);
    }

    #[test]
    fn reclaimed_bytes_counter_uses_only_stable_resource_kind_label() {
        let mut out = String::new();
        append_reclaimed_bytes_metric(
            &mut out,
            &[("run_worktree".to_string(), 12), ("repository_mirror".to_string(), 34)],
        );
        assert!(out.contains("ai_pm_workspace_reclaimed_bytes_total{resource_kind=\"run_worktree\"} 12"));
        assert!(out.contains("ai_pm_workspace_reclaimed_bytes_total{resource_kind=\"repository_mirror\"} 34"));
        assert!(!out.contains("status="));
    }
}
