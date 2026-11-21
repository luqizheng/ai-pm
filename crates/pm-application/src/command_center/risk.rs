//! 风险面：风险汇总块（§4.5）。

use super::*;

// ---------------------------------------------------------------------------
// §4.5 实现
// ---------------------------------------------------------------------------

pub(super) async fn load_risk_summary(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<RiskSummary, sqlx::Error> {
    let mut items: Vec<RiskItem> = Vec::new();

    // 失败 / 超时的 Run
    type Row = (Uuid, Uuid, String, String, Option<DateTime<Utc>>);
    let failed_runs: Vec<Row> = sqlx::query_as(
        "SELECT r.id, r.task_id, t.title, r.status, r.finished_at \
         FROM delivery.task_run r JOIN delivery.task t ON t.id = r.task_id \
         WHERE r.project_id = $1 AND r.tenant_id = $2 AND r.status IN ('failed','timed_out') \
         ORDER BY r.created_at DESC, r.id DESC LIMIT 50",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    for (run_id, _task_id, title, status, finished_at) in failed_runs {
        items.push(RiskItem {
            kind: "run_failed".to_string(),
            domain: "run".to_string(),
            source_id: run_id.to_string(),
            project_id,
            title: format!("执行失败（{title}）"),
            source_state: status,
            severity: "high".to_string(),
            occurred_at: finished_at,
            suggestion: "查看 Run 日志并决定重跑".to_string(),
            link: ActionLink {
                kind: "task_run".to_string(),
                params: BTreeMap::from([("run_id".to_string(), run_id.to_string())]),
            },
        });
    }

    // 阻塞中的 Task
    let blocked_tasks: Vec<(Uuid, String, String, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT id, title, status, updated_at FROM delivery.task \
         WHERE project_id = $1 AND tenant_id = $2 AND status = 'blocked' \
         ORDER BY updated_at DESC NULLS LAST, id DESC LIMIT 50",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    for (id, title, status, updated_at) in blocked_tasks {
        items.push(RiskItem {
            kind: "task_blocked".to_string(),
            domain: "task".to_string(),
            source_id: id.to_string(),
            project_id,
            title: format!("任务阻塞（{title}）"),
            source_state: status,
            severity: "medium".to_string(),
            occurred_at: updated_at,
            suggestion: "查看阻塞原因并解除".to_string(),
            link: ActionLink {
                kind: "task".to_string(),
                params: BTreeMap::from([("task_id".to_string(), id.to_string())]),
            },
        });
    }

    // 不可达仓库
    type Row2 = (Uuid, String, Option<String>, Option<DateTime<Utc>>);
    let repos: Vec<Row2> = sqlx::query_as(
        "SELECT id, name, last_error_code, last_verified_at \
         FROM integration.project_repository \
         WHERE project_id = $1 AND tenant_id = $2 AND status = 'unreachable' \
         ORDER BY name, id LIMIT 50",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    for (id, name, error_code, verified_at) in repos {
        let severity = if error_code.as_deref() == Some("auth_failed") {
            "high"
        } else {
            "medium"
        };
        items.push(RiskItem {
            kind: "repository_unreachable".to_string(),
            domain: "repository".to_string(),
            source_id: id.to_string(),
            project_id,
            title: format!("仓库不可达（{name}）"),
            source_state: error_code.unwrap_or_else(|| "unreachable".to_string()),
            severity: severity.to_string(),
            occurred_at: verified_at,
            suggestion: "检查 SSH 密钥与仓库地址后重新校验".to_string(),
            link: ActionLink {
                kind: "repository".to_string(),
                params: BTreeMap::from([("repository_id".to_string(), id.to_string())]),
            },
        });
    }

    // 分析失败（以最新导入作业为准：失败且无更新的成功）
    let latest_import: Option<(Uuid, String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, status, error_code, created_at FROM integration.repo_import \
         WHERE project_id = $1 AND tenant_id = $2 ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?;
    if let Some((import_id, status, error_code, created_at)) = latest_import {
        if status == "failed" || status == "rejected" {
            items.push(RiskItem {
                kind: "analysis_failed".to_string(),
                domain: "import".to_string(),
                source_id: import_id.to_string(),
                project_id,
                title: "代码分析失败".to_string(),
                source_state: status,
                severity: "high".to_string(),
                occurred_at: Some(created_at),
                suggestion: error_code.unwrap_or_else(|| "查看导入作业详情".to_string()),
                link: ActionLink {
                    kind: "repo_import".to_string(),
                    params: BTreeMap::from([("import_id".to_string(), import_id.to_string())]),
                },
            });
        }
    }

    // 归档后残留活跃 Run（I1 P3 事实）
    let archived: bool = sqlx::query_scalar(
        "SELECT status = 'closed' FROM core.project WHERE id = $1 AND tenant_id = $2",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    if archived {
        let residual: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM delivery.task_run \
             WHERE project_id = $1 AND tenant_id = $2 AND status IN ('claimed','running')",
        )
        .bind(project_id)
        .bind(tenant_id)
        .fetch_one(pool)
        .await?;
        if residual > 0 {
            items.push(RiskItem {
                kind: "archive_cleanup".to_string(),
                domain: "project".to_string(),
                source_id: project_id.to_string(),
                project_id,
                title: "归档后仍有执行中的 Run".to_string(),
                source_state: "closed".to_string(),
                severity: "medium".to_string(),
                occurred_at: None,
                suggestion: "终止残留进程并确认清理完成".to_string(),
                link: ActionLink {
                    kind: "project".to_string(),
                    params: BTreeMap::from([("project_id".to_string(), project_id.to_string())]),
                },
            });
        }
    }

    let total = items.len() as i64;
    Ok(RiskSummary { total, items })
}
