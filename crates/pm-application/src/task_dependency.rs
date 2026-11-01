//! Task 前置依赖与解锁（T-024 / `docs/ai-pm-task-dependency-v1.md`）。
//!
//! 载体是独立边表 `delivery.task_dependency`：`task_id` 是下游（依赖方），
//! `depends_on_task_id` 是上游（被依赖方）。四项已裁定事项（D1–D4）与三项本稿
//! 决定（D5–D8）见设计稿 §2；本模块只实现，不重新裁量。
//!
//! 三个硬不变量：
//! 1. **依赖满足只有一套谓词**（[`dependencies_satisfied`]）：调度领取、建边/删边后
//!    的解锁、终态触发的级联解锁共用它，禁止各写一套。
//! 2. **状态变更仍走唯一受守卫入口**（`apply_task_transition`）：解锁把
//!    `blocked → ready` 也交给它校验 + 审计，不旁路更新状态。
//! 3. **事件与状态同事务**：解锁产生的 `TaskReady` 与迁移在同一个事务里写入，
//!    幂等键沿用命令通道约定 `task:<id>:TaskReady:rev<revision>`。

use serde::Serialize;
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use pm_domain::TaskStatus;

use crate::actor::AuditActor;
use crate::command_executor::{apply_task_transition, system_context, TransitionFacts, Tx};
use crate::outbox::{event_is_enabled, OutboxAppend, OutboxEvent, PgOutboxAppend};

/// D5：前置「已满足」的终态集合。
///
/// `failed` 不在其中——失败的上游需要重试或被取消/取代，否则下游保持阻塞，
/// 不能因为一次临时失败就放行下游。
pub const SATISFIED_STATUSES: [&str; 3] = ["completed", "cancelled", "superseded"];

/// `SQL IN (...)` 片段，供判定与解锁共用（集合只能有一处定义）。
pub const SATISFIED_STATUSES_SQL: &str = "'completed', 'cancelled', 'superseded'";

/// 一次解锁迁移允许的下游状态：只有 `blocked` 需要被解锁；`ready` 及之后的状态
/// 已有自己的事件与 Run，重复投递只会制造第二个语义键（设计稿 §5.3）。
pub fn unlocks_downstream(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Completed | TaskStatus::Cancelled | TaskStatus::Superseded
    )
}

/// 依赖图里的一个端点（GET 响应元素）。
#[derive(Debug, Clone, Serialize)]
pub struct DependencyEdge {
    pub id: Uuid,
    pub title: String,
    pub status: String,
}

/// `GET /v1/pm/tasks/{task_id}/dependencies` 的响应体。
#[derive(Debug, Clone, Serialize)]
pub struct DependencyGraph {
    pub task_id: Uuid,
    /// 本任务依赖的前置（`depends_on_task_id`）。
    pub upstream: Vec<DependencyEdge>,
    /// 依赖本任务的下游（`task_id`）。
    pub downstream: Vec<DependencyEdge>,
}

#[derive(Debug)]
pub enum DependencyError {
    /// 任务不存在（或不属于调用方可见范围）。
    TaskNotFound,
    /// `task_id == depends_on_task_id`。
    SelfDependency,
    /// 下游状态不在 `draft / blocked`（`Ready → Blocked` 不是合法边，D6）。
    TaskNotPending(String),
    /// 同一对端点已有边。
    Duplicate,
    /// 两端不同项目（D3）；库层两条复合外键是第二道保险。
    CrossProject,
    /// 新上游沿现有边能回到下游。
    Cycle,
    Db(sqlx::Error),
}

impl From<sqlx::Error> for DependencyError {
    fn from(e: sqlx::Error) -> Self {
        DependencyError::Db(e)
    }
}

/// 依赖满足的唯一判定（D5）：全部前置状态 ∈ [`SATISFIED_STATUSES`]。
pub(crate) async fn dependencies_satisfied(
    tx: &mut Tx,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT NOT EXISTS ( \
             SELECT 1 FROM delivery.task_dependency d \
               JOIN delivery.task u ON u.project_id = d.project_id AND u.id = d.depends_on_task_id \
              WHERE d.project_id = $1 AND d.task_id = $2 \
                AND u.status NOT IN ({SATISFIED_STATUSES_SQL}))",
    ))
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await
}

/// 任务的前置/后置关系（GET）。调用方负责可见性（T-022）与租户前置校验。
pub async fn list_dependencies(
    pool: &PgPool,
    task_id: Uuid,
) -> Result<DependencyGraph, DependencyError> {
    let _ = sqlx::query("SELECT 1 FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_optional(pool)
        .await?
        .ok_or(DependencyError::TaskNotFound)?;

    let upstream = sqlx::query(
        "SELECT t.id, t.title, t.status \
           FROM delivery.task_dependency d \
           JOIN delivery.task t ON t.project_id = d.project_id AND t.id = d.depends_on_task_id \
          WHERE d.task_id = $1 \
          ORDER BY t.created_at, t.id",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;
    let downstream = sqlx::query(
        "SELECT t.id, t.title, t.status \
           FROM delivery.task_dependency d \
           JOIN delivery.task t ON t.project_id = d.project_id AND t.id = d.task_id \
          WHERE d.depends_on_task_id = $1 \
          ORDER BY t.created_at, t.id",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    Ok(DependencyGraph {
        task_id,
        upstream: upstream.iter().map(edge_of).collect(),
        downstream: downstream.iter().map(edge_of).collect(),
    })
}

fn edge_of(row: &sqlx::postgres::PgRow) -> DependencyEdge {
    DependencyEdge {
        id: row.get("id"),
        title: row.get("title"),
        status: row.get("status"),
    }
}

/// 建边（POST）：同一事务内完成校验 → 插边 → 审计 → 解锁判定（D7）。
pub async fn add_dependency(
    pool: &PgPool,
    task_id: Uuid,
    depends_on_task_id: Uuid,
    actor: &AuditActor,
) -> Result<DependencyGraph, DependencyError> {
    let mut tx = pool.begin().await?;

    // ① 锁下游任务行（与解锁路径同一加锁顺序：先被依赖方，后依赖方）。
    let downstream = sqlx::query(
        "SELECT project_id, status FROM delivery.task WHERE id = $1 FOR UPDATE",
    )
    .bind(task_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(DependencyError::TaskNotFound)?;
    let project_id: Uuid = downstream.get("project_id");
    let status: String = downstream.get("status");

    // ② 上游必须存在且同项目（D3）。
    let upstream = sqlx::query("SELECT project_id FROM delivery.task WHERE id = $1")
        .bind(depends_on_task_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DependencyError::TaskNotFound)?;
    let upstream_project: Uuid = upstream.get("project_id");
    if upstream_project != project_id {
        return Err(DependencyError::CrossProject);
    }

    if depends_on_task_id == task_id {
        return Err(DependencyError::SelfDependency);
    }

    // ③ D6：只允许给尚未进入就绪队列的任务加边。
    if !matches!(status.as_str(), "draft" | "blocked") {
        return Err(DependencyError::TaskNotPending(status));
    }

    // ④ 重复边（唯一键预检；并发插入由 ON CONFLICT 兜底）。
    let duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM delivery.task_dependency \
                          WHERE task_id = $1 AND depends_on_task_id = $2)",
    )
    .bind(task_id)
    .bind(depends_on_task_id)
    .fetch_one(&mut *tx)
    .await?;
    if duplicate {
        return Err(DependencyError::Duplicate);
    }

    // ⑤ 环检测（D2）：从新上游沿 `depends_on_task_id` 上溯，回到下游即有环。
    let cycle: bool = sqlx::query_scalar(
        "WITH RECURSIVE ancestors AS ( \
             SELECT $1::uuid AS id \
             UNION \
             SELECT d.depends_on_task_id \
               FROM delivery.task_dependency d \
               JOIN ancestors a ON d.task_id = a.id \
              WHERE d.project_id = $2) \
         SELECT EXISTS (SELECT 1 FROM ancestors WHERE id = $3)",
    )
    .bind(depends_on_task_id)
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&mut *tx)
    .await?;
    if cycle {
        return Err(DependencyError::Cycle);
    }

    let inserted = sqlx::query(
        "INSERT INTO delivery.task_dependency \
         (tenant_id, project_id, task_id, depends_on_task_id, created_by) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4) \
         ON CONFLICT (task_id, depends_on_task_id) DO NOTHING",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(depends_on_task_id)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(DependencyError::Duplicate);
    }

    crate::pm_write::append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "task.dependency.add",
        "task",
        Some(task_id),
        json!({ "depends_on_task_id": depends_on_task_id }),
    )
    .await?;

    // ⑥ D7：已满足的前置立即解锁 `blocked` 下游（`draft` 保持原状）。
    unlock_task_if_ready(&mut tx, project_id, task_id).await?;

    tx.commit().await?;
    list_dependencies(pool, task_id).await
}

/// 删边（DELETE）：幂等；删除成功后做一次解锁判定（D8）。
pub async fn remove_dependency(
    pool: &PgPool,
    task_id: Uuid,
    depends_on_task_id: Uuid,
    actor: &AuditActor,
) -> Result<bool, DependencyError> {
    let mut tx = pool.begin().await?;
    let downstream =
        sqlx::query("SELECT project_id FROM delivery.task WHERE id = $1 FOR UPDATE")
            .bind(task_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(DependencyError::TaskNotFound)?;
    let project_id: Uuid = downstream.get("project_id");

    let deleted = sqlx::query(
        "DELETE FROM delivery.task_dependency \
          WHERE task_id = $1 AND depends_on_task_id = $2",
    )
    .bind(task_id)
    .bind(depends_on_task_id)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        > 0;

    if deleted {
        crate::pm_write::append_audit_with_after(
            &mut tx,
            project_id,
            actor,
            "task.dependency.remove",
            "task",
            Some(task_id),
            json!({ "depends_on_task_id": depends_on_task_id }),
        )
        .await?;
        unlock_task_if_ready(&mut tx, project_id, task_id).await?;
    }

    tx.commit().await?;
    Ok(deleted)
}

/// 上游进入终态后的级联解锁（D4）：逐个把满足依赖的 `blocked` 下游迁到 `ready`。
///
/// 下游按 `id` 升序逐个加锁（固定顺序防死锁）；每个下游都重新读取状态再判定，
/// 因此并发解锁同一批下游是幂等的。
pub(crate) async fn unlock_downstream(
    tx: &mut Tx,
    project_id: Uuid,
    upstream_task_id: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT t.id \
           FROM delivery.task_dependency d \
           JOIN delivery.task t ON t.project_id = d.project_id AND t.id = d.task_id \
          WHERE d.project_id = $1 AND d.depends_on_task_id = $2 \
          ORDER BY t.id",
    )
    .bind(project_id)
    .bind(upstream_task_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut unlocked = Vec::new();
    for row in rows {
        let downstream_id: Uuid = row.get("id");
        if unlock_task_if_ready(tx, project_id, downstream_id).await? {
            unlocked.push(downstream_id);
        }
    }
    Ok(unlocked)
}

/// 单个任务的解锁判定：`blocked` 且全部前置满足 → `ready` + `TaskReady`（同事务）。
///
/// 返回是否真的发生迁移。`draft` / `ready` / 执行中 / 终态一律不动——`draft`
/// 是否进 `ready` 由用户或命令通道决定（D7）。
async fn unlock_task_if_ready(
    tx: &mut Tx,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        "SELECT status FROM delivery.task WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let status: String = row.get("status");
    if status != "blocked" {
        return Ok(false);
    }
    if !dependencies_satisfied(tx, project_id, task_id).await? {
        return Ok(false);
    }

    // R1：状态推进仍经唯一受守卫入口（含 revision guard 与 update_task_status 审计）。
    let ctx = system_context(tx, project_id).await.map_err(transition_err)?;
    let (_, revision) = apply_task_transition(
        tx,
        task_id,
        None,
        TaskStatus::Ready,
        &TransitionFacts::for_dispatch(),
        &ctx,
    )
    .await
    .map_err(transition_err)?;

    // 事件与迁移同事务；幂等键与命令通道一致（设计稿 §5.3）。
    if event_is_enabled("TaskReady") {
        let aggregate_id = task_id.to_string();
        let idempotency_key = format!("task:{task_id}:TaskReady:rev{revision}");
        PgOutboxAppend
            .append(
                tx,
                OutboxEvent {
                    project_id,
                    event_type: "TaskReady",
                    aggregate_type: "Task",
                    aggregate_id: &aggregate_id,
                    payload: json!({
                        "task_id": task_id,
                        "to": "ready",
                        "revision": revision,
                        "unlocked_by": "task_dependency",
                    }),
                    idempotency_key: &idempotency_key,
                },
            )
            .await?;
    }
    Ok(true)
}

/// 解锁路径只可能遇到数据库错误或状态机拒绝；后者在数据一致时不会出现，
/// 归并成 `Protocol` 保留上下文，不吞掉。
fn transition_err(e: pm_domain::CommandError) -> sqlx::Error {
    match e {
        pm_domain::CommandError::Db(inner) => inner,
        other => sqlx::Error::Protocol(other.to_string()),
    }
}