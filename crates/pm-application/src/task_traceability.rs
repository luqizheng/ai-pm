//! 任务全链追溯读面（PA07-IMP-007；实施规范 §10 P0-007）。
//!
//! 只读聚合：Revision 链 / 依赖边与领取快照 / Task-Product 归属与状态 /
//! Product 验收 / Run 门禁证据 / 挂任务的 Human Consultation——六类记录均
//! 来自前批落地的权威表与审计，本模块不产生新事实。供审计检索与 PA-07
//! §8「历史记录可追溯」验收。

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct TaskTraceability {
    pub task_id: Uuid,
    pub task_revision: i64,
    pub current_task_revision_id: Option<Uuid>,
    /// 不可变 Revision 快照链（revision_no 升序，含 change_reason/created_by）。
    pub revisions: Vec<RevisionLink>,
    /// 当前有效依赖边 + 每条边的逐边条件。
    pub dependencies: Vec<DependencyLink>,
    /// 领取成功的门禁快照（逐边前置 Revision/条件/结论/检查时间）。
    pub run_gate_snapshots: Vec<RunGateSnapshotLink>,
    /// Task→Product 归属（含已撤回历史行）。
    pub product_links: Vec<ProductLinkTrace>,
    /// Product 级验收历史（追加式）。
    pub acceptances: Vec<AcceptanceLink>,
    /// 关联任务的 Human Consultation（含状态与决定人）。
    pub consultations: Vec<ConsultationLink>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RevisionLink {
    pub id: Uuid,
    pub revision_no: i64,
    pub change_reason: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DependencyLink {
    pub depends_on_task_id: Uuid,
    pub required_condition: String,
    pub required_deliverable_ref: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RunGateSnapshotLink {
    pub run_id: Uuid,
    pub predecessor_task_id: Uuid,
    pub predecessor_revision_id: Option<Uuid>,
    pub required_condition: String,
    pub satisfied: bool,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ProductLinkTrace {
    pub product_id: Uuid,
    pub product_name: String,
    pub product_status: String,
    pub withdrawn: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub withdrawn_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AcceptanceLink {
    pub id: Uuid,
    pub product_id: Uuid,
    pub result: String,
    pub condition_scope: String,
    pub accepted_by: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ConsultationLink {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub decided_by: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 聚合查询：任务不存在（或跨租户）→ Err。
pub async fn task_traceability(
    pool: &PgPool,
    tenant_id: Uuid,
    task_id: Uuid,
) -> Result<TaskTraceability, sqlx::Error> {
    let exists: Option<i32> =
        sqlx::query_scalar("SELECT 1 FROM delivery.task WHERE id = $1 AND tenant_id = $2")
            .bind(task_id)
            .bind(tenant_id)
            .fetch_optional(pool)
            .await?;
    let Some(_) = exists else {
        return Err(sqlx::Error::RowNotFound);
    };

    let (task_revision, current_task_revision_id): (i64, Option<Uuid>) = sqlx::query_as(
        "SELECT revision, current_task_revision_id FROM delivery.task WHERE id = $1",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await?;

    let revisions = sqlx::query_as(
        "SELECT id, revision_no, change_reason, created_by, created_at \
           FROM delivery.task_revision WHERE task_id = $1 ORDER BY revision_no",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    let dependencies = sqlx::query_as(
        "SELECT depends_on_task_id, required_condition, required_deliverable_ref \
           FROM delivery.task_dependency WHERE task_id = $1 ORDER BY depends_on_task_id",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    let run_gate_snapshots = sqlx::query_as(
        "SELECT run_id, predecessor_task_id, predecessor_revision_id, required_condition, \
                satisfied, checked_at \
           FROM delivery.task_run_dependency_snapshot WHERE task_id = $1 \
          ORDER BY checked_at, run_id",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    let product_links = sqlx::query_as(
        "SELECT p.id AS product_id, p.name AS product_name, p.status AS product_status, \
                (tp.withdrawn_at IS NOT NULL) AS withdrawn, tp.created_at, tp.withdrawn_at \
           FROM delivery.task_product tp JOIN core.product p ON p.id = tp.product_id \
          WHERE tp.task_id = $1 ORDER BY tp.created_at, p.id",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    let acceptances = sqlx::query_as(
        "SELECT id, product_id, result, condition_scope, accepted_by, created_at \
           FROM delivery.task_product_acceptance WHERE task_id = $1 ORDER BY created_at, id",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    let consultations = sqlx::query_as(
        "SELECT id, title, status, decided_by, created_at \
           FROM governance.human_consultation WHERE task_id = $1 AND tenant_id = $2 \
          ORDER BY created_at, id",
    )
    .bind(task_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;

    Ok(TaskTraceability {
        task_id,
        task_revision,
        current_task_revision_id,
        revisions,
        dependencies,
        run_gate_snapshots,
        product_links,
        acceptances,
        consultations,
    })
}
