//! 工作面：任务/交付汇总、活跃 Run、近期 Run 简报（§4.3）。

use super::*;

// ---------------------------------------------------------------------------
// §4.3 实现
// ---------------------------------------------------------------------------

pub(super) async fn load_work_summary(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<WorkSummary, sqlx::Error> {
    let task_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, count(*) FROM delivery.task \
         WHERE project_id = $1 AND tenant_id = $2 GROUP BY status",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let task_by_status: BTreeMap<String, i64> = task_rows.into_iter().collect();
    let task_total: i64 = task_by_status.values().sum();

    let active_runs = load_runs(pool, tenant_id, project_id, true, 20).await?;
    let active_run_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run \
         WHERE project_id = $1 AND tenant_id = $2 \
           AND status IN ('queued','claimed','running')",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    let recent_runs = load_recent_run_briefs(pool, tenant_id, project_id, 5).await?;

    // 需求进度：active 需求总数 + 当前修订已 approved（真人 confirm 定稿）数。
    // draft / pending_confirmation / superseded 不计入进度——与 requirement.rs
    // 状态机一致（Revision 2 approved → v1 superseded，进度只看 current）。
    let requirement_total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.requirement \
         WHERE project_id = $1 AND tenant_id = $2 AND lifecycle_status = 'active'",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    let requirement_approved: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.requirement req \
          JOIN delivery.requirement_revision rev ON rev.id = req.current_revision_id \
         WHERE req.project_id = $1 AND req.tenant_id = $2 \
           AND req.lifecycle_status = 'active' AND rev.state = 'approved'",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;

    let delivery_rows = crate::delivery::list_project_delivery(pool, tenant_id, project_id).await?;
    let mut delivery = DeliverySummary {
        delivered: 0,
        partial: 0,
        missing: 0,
        no_contract: 0,
    };
    for item in &delivery_rows {
        match item.delivery_state.as_str() {
            "delivered" => delivery.delivered += 1,
            "partial" => delivery.partial += 1,
            "missing" => delivery.missing += 1,
            _ => delivery.no_contract += 1,
        }
    }

    Ok(WorkSummary {
        task_total,
        task_by_status,
        requirement_total,
        requirement_approved,
        active_run_count,
        active_runs,
        recent_runs,
        delivery,
    })
}

pub(super) async fn load_runs(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    only_active: bool,
    limit: i64,
) -> Result<Vec<ActiveRun>, sqlx::Error> {
    type Row = (
        Uuid,
        Uuid,
        String,
        String,
        i32,
        String,
        Option<String>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<Uuid>,
        Option<String>,
        Option<String>,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT r.id, r.task_id, t.title, r.run_kind, r.attempt_no, r.status, r.stage, \
                r.claimed_at, r.started_at, r.heartbeat_at, r.lease_expires_at, \
                t.assignee_id, p.display_name, p.kind \
         FROM delivery.task_run r \
         JOIN delivery.task t ON t.id = r.task_id \
         LEFT JOIN core.principal p ON p.id = t.assignee_id \
         WHERE r.project_id = $1 AND r.tenant_id = $2 \
           AND ($3 = false OR r.status IN ('queued','claimed','running')) \
         ORDER BY r.created_at DESC, r.id DESC \
         LIMIT $4",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(only_active)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let run_ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
    let mut repos: BTreeMap<Uuid, Vec<RunRepository>> = BTreeMap::new();
    if !run_ids.is_empty() {
        let repo_rows: Vec<(Uuid, Uuid, String, String, String)> = sqlx::query_as(
            "SELECT run_id, repository_id, base_ref, base_commit, workspace_path \
             FROM delivery.task_run_repository \
             WHERE run_id = ANY($1) ORDER BY run_id, repository_id",
        )
        .bind(&run_ids)
        .fetch_all(pool)
        .await?;
        for (run_id, repository_id, base_ref, base_commit, workspace_path) in repo_rows {
            repos.entry(run_id).or_default().push(RunRepository {
                repository_id,
                base_ref,
                base_commit,
                workspace_path,
            });
        }
    }

    Ok(rows
        .into_iter()
        .map(
            |(
                run_id,
                task_id,
                task_title,
                run_kind,
                attempt_no,
                status,
                stage,
                claimed_at,
                started_at,
                heartbeat_at,
                lease_expires_at,
                assignee_id,
                display_name,
                kind,
            )| ActiveRun {
                run_id,
                task_id,
                task_title,
                run_kind,
                attempt_no,
                status,
                stage,
                claimed_at,
                started_at,
                heartbeat_at,
                lease_expires_at,
                agent: assignee_id.map(|principal_id| AgentRef {
                    principal_id,
                    display_name: display_name.unwrap_or_else(|| "未知执行主体".to_string()),
                    kind: kind.unwrap_or_else(|| "unknown".to_string()),
                }),
                repositories: repos.get(&run_id).cloned().unwrap_or_default(),
            },
        )
        .collect())
}

pub(super) async fn load_recent_run_briefs(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    limit: i64,
) -> Result<Vec<RunBrief>, sqlx::Error> {
    type Row = (
        Uuid,
        Uuid,
        String,
        String,
        i32,
        String,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT r.id, r.task_id, t.title, r.run_kind, r.attempt_no, r.status, r.created_at, r.finished_at \
         FROM delivery.task_run r \
         JOIN delivery.task t ON t.id = r.task_id \
         WHERE r.project_id = $1 AND r.tenant_id = $2 \
         ORDER BY r.created_at DESC, r.id DESC LIMIT $3",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                run_id,
                task_id,
                task_title,
                run_kind,
                attempt_no,
                status,
                created_at,
                finished_at,
            )| {
                RunBrief {
                    run_id,
                    task_id,
                    task_title,
                    run_kind,
                    attempt_no,
                    status,
                    created_at,
                    finished_at,
                }
            },
        )
        .collect())
}
