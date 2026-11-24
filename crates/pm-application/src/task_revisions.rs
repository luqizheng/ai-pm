//! PA-07 · Task Revision 历史读面（Figma 179:3603 D 系列）。
//!
//! `delivery.task_revision` 由 `pm_write::task_revision::snapshot_and_insert`
//! 写入（实质变更冻结，行不可变——全库无 UPDATE 路径）；`active` =
//! `task.current_task_revision_id` 指针。本模块只做投影，不重算业务事实：
//!
//! - `revision_no`：展示序号（按 created_at 连续 1,2,3…，对应设计稿 r1/r2/r3）；
//!   `epoch_no` = 快照创建时的 `task.revision` CAS 纪元（允许间隙，审计对齐用）。
//! - `run_count`：Run 创建时 `task_run.task_revision_id` 固定引用当时指针
//!   （PA07-IMP-002），据此给每版历史「Runs: …」行。
//! - `stale_active_runs`：仍 queued/claimed/running 但绑定在**非当前** Revision
//!   上的 Run——设计稿「Older active Run warning」（RN-87 continues using r2）。

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// 单条 Revision 时间线行（D 画布「PA-07 / Revision History Timeline」）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct TaskRevisionRow {
    pub id: Uuid,
    /// 展示序号（连续，设计稿 r1/r2/r3）。
    pub revision_no: i64,
    /// 快照创建时的 CAS 纪元（允许间隙）。
    pub epoch_no: i64,
    pub active: bool,
    pub goal: Option<String>,
    pub scope: serde_json::Value,
    pub acceptance_criteria: serde_json::Value,
    pub execution_contract: serde_json::Value,
    pub change_reason: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub run_count: i64,
}

/// 绑定在非当前 Revision 上的仍活跃 Run（D 画布警告卡）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct StaleActiveRunRow {
    pub run_id: Uuid,
    /// task_run.attempt_no 是 INT4。
    pub attempt_no: i32,
    pub status: String,
    pub revision_no: i64,
    pub created_at: DateTime<Utc>,
}

/// `GET /v1/pm/tasks/{task_id}/revisions`：Revision 时间线 + 陈旧活跃 Run。
/// 任务不存在/跨租户 → `None`（HTTP 层映射 404；可见性闸由 require_task_project 负责）。
pub async fn list_task_revisions(
    pool: &PgPool,
    tenant_id: Uuid,
    task_id: Uuid,
) -> Result<Option<(Vec<TaskRevisionRow>, Vec<StaleActiveRunRow>)>, sqlx::Error> {
    let exists: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM delivery.task WHERE id = $1 AND tenant_id = $2")
            .bind(task_id)
            .bind(tenant_id)
            .fetch_optional(pool)
            .await?;
    if exists.is_none() {
        return Ok(None);
    }

    // 时间线最新在上（设计稿：r3 · Active 顶部，r1 · Historical 沉底）。
    let rows: Vec<TaskRevisionRow> = sqlx::query_as(
        "SELECT r.id, \
            row_number() OVER (ORDER BY r.created_at, r.id) AS revision_no, \
            r.revision_no AS epoch_no, \
            (r.id = t.current_task_revision_id) AS active, \
            r.goal, r.scope, r.acceptance_criteria, r.execution_contract, \
            r.change_reason, r.created_by, r.created_at, \
            (SELECT count(*) FROM delivery.task_run ru WHERE ru.task_revision_id = r.id) \
                AS run_count \
       FROM delivery.task_revision r \
       JOIN delivery.task t ON t.id = r.task_id \
      WHERE r.task_id = $1 AND r.tenant_id = $2 \
      ORDER BY r.created_at DESC, r.id DESC",
    )
    .bind(task_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;

    let stale: Vec<StaleActiveRunRow> = sqlx::query_as(
        "SELECT ru.id AS run_id, ru.attempt_no, ru.status, rn.revision_no, ru.created_at \
           FROM delivery.task_run ru \
           JOIN delivery.task_revision rn ON rn.id = ru.task_revision_id \
          WHERE ru.task_id = $1 AND ru.tenant_id = $2 \
            AND ru.status IN ('queued', 'claimed', 'running') \
            AND ru.task_revision_id IS DISTINCT FROM \
                (SELECT current_task_revision_id FROM delivery.task WHERE id = $1) \
          ORDER BY ru.attempt_no DESC",
    )
    .bind(task_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;

    Ok(Some((rows, stale)))
}
