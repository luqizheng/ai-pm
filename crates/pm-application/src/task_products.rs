//! Task→Product 业务服务归属（PA07-IMP-004；PA-07 V1.0 §4.3 / §6.1）。
//!
//! 表达「该 Task 服务哪些 Product 的业务受益归属」，与 Product 门禁的技术依赖
//! 分开建模（§6.1：服务归属 ≠ 依赖某 Product 的代码/接口）。撤回是软删除
//! （withdrawn_by/at），历史保留——§6.4 需求撤回后专属范围经正式处理退出有效
//! 范围，验收与追溯仍可见。新建关联要求 Product 处于 `normal`（业务写入闸，
//! [`crate::product::assert_product_active`]）；执行门禁的判定在 run_dispatch。

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::product::ProductError;

/// `GET /v1/pm/tasks/{task_id}/products` 的元素（含已撤回的历史行）。
#[derive(Debug, Clone, Serialize)]
pub struct TaskProductLink {
    pub product_id: Uuid,
    pub product_name: String,
    pub product_status: String,
    pub scope: Option<String>,
    pub withdrawn: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub withdrawn_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug)]
pub enum TaskProductError {
    TaskNotFound,
    ProductNotFound,
    /// Product 存在但不在可承接状态（`normal`）。
    ProductNotActive(&'static str),
    /// 同一对 (task, product) 已有有效关联。
    Duplicate,
    /// 关联不存在或已撤回（幂等删除时也返回该值区分）。
    LinkNotFound,
    Db(sqlx::Error),
}

impl From<ProductError> for TaskProductError {
    fn from(e: ProductError) -> Self {
        match e {
            ProductError::NotFound => TaskProductError::ProductNotFound,
            ProductError::Conflict(code) => TaskProductError::ProductNotActive(code),
            ProductError::Db(err) => TaskProductError::Db(err),
            ProductError::Forbidden(_) | ProductError::Validation(_) => {
                TaskProductError::ProductNotActive("product_forbidden")
            }
            ProductError::RuleRejected(_, _) => {
                TaskProductError::ProductNotActive("product_rule_rejected")
            }
        }
    }
}

impl From<sqlx::Error> for TaskProductError {
    fn from(e: sqlx::Error) -> Self {
        TaskProductError::Db(e)
    }
}

pub async fn list_task_products(
    pool: &PgPool,
    tenant_id: Uuid,
    task_id: Uuid,
) -> Result<Vec<TaskProductLink>, TaskProductError> {
    let exists: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM delivery.task WHERE id = $1 AND tenant_id = $2",
    )
    .bind(task_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?;
    if exists.is_none() {
        return Err(TaskProductError::TaskNotFound);
    }
    let rows: Vec<(Uuid, String, String, Option<String>, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        "SELECT p.id, p.name, p.status, tp.scope, tp.withdrawn_at, tp.created_at, tp.withdrawn_at \
           FROM delivery.task_product tp \
           JOIN core.product p ON p.id = tp.product_id \
          WHERE tp.task_id = $1 AND tp.tenant_id = $2 \
          ORDER BY tp.created_at, p.id",
    )
    .bind(task_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(product_id, product_name, product_status, scope, withdrawn_at, created_at, _)| {
                TaskProductLink {
                    product_id,
                    product_name,
                    product_status,
                    scope,
                    withdrawn: withdrawn_at.is_some(),
                    created_at,
                    withdrawn_at,
                }
            },
        )
        .collect())
}

/// 建立归属（同事务：Product 业务写入闸 FOR UPDATE → 插行 → 审计）。
pub async fn add_task_product(
    pool: &PgPool,
    tenant_id: Uuid,
    task_id: Uuid,
    product_id: Uuid,
    scope: Option<&str>,
    actor: &AuditActor,
) -> Result<(), TaskProductError> {
    let mut tx = pool.begin().await?;
    let (project_id,): (Uuid,) = sqlx::query_as(
        "SELECT project_id FROM delivery.task WHERE id = $1 AND tenant_id = $2",
    )
    .bind(task_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(TaskProductError::TaskNotFound)?;

    // Product 必须存在且在业务活跃态（含 FOR UPDATE，闸内已锁行）。
    crate::product::assert_product_active(&mut tx, product_id).await?;

    let inserted = sqlx::query(
        "INSERT INTO delivery.task_product \
         (tenant_id, project_id, task_id, product_id, scope, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (task_id, product_id) WHERE withdrawn_at IS NULL DO NOTHING",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(task_id)
    .bind(product_id)
    .bind(scope)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(TaskProductError::Duplicate);
    }

    crate::pm_write::append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "task.product_linked",
        "task",
        Some(task_id),
        serde_json::json!({ "product_id": product_id, "scope": scope }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 撤回归属（软删除 + 原因入审计，审查82 同款纪律）。幂等：已撤回/不存在
/// 返回 `Ok(false)`，不重复写审计。
pub async fn withdraw_task_product(
    pool: &PgPool,
    tenant_id: Uuid,
    task_id: Uuid,
    product_id: Uuid,
    reason: &str,
    actor: &AuditActor,
) -> Result<bool, TaskProductError> {
    let mut tx = pool.begin().await?;
    let (project_id,): (Uuid,) = sqlx::query_as(
        "SELECT project_id FROM delivery.task WHERE id = $1 AND tenant_id = $2",
    )
    .bind(task_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(TaskProductError::TaskNotFound)?;

    let updated = sqlx::query(
        "UPDATE delivery.task_product \
            SET withdrawn_by = $3, withdrawn_at = now() \
          WHERE task_id = $1 AND product_id = $2 AND withdrawn_at IS NULL",
    )
    .bind(task_id)
    .bind(product_id)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        tx.commit().await?;
        return Ok(false);
    }

    crate::pm_write::append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "task.product_withdrawn",
        "task",
        Some(task_id),
        serde_json::json!({ "product_id": product_id, "reason": reason }),
    )
    .await?;
    tx.commit().await?;
    Ok(true)
}

// ───────────────────────────────────── Run 启动门禁（PA07-IMP-004，PA-07 §5）

/// Product 执行门禁（PA-07 §5.1 门禁 2）：返回第一个阻塞执行的有效关联
/// Product `(product_id, deny_reason)`——`suspended`/`archived` 一律阻塞；
/// `normal`/`resuming` 放行（Resuming 的逐步开放按 Portfolio Review 结论
/// 由 AI-PM 控制，V1 门禁不细分）。行锁 FOR UPDATE 与 lifecycle 写路径
/// 串行化：暂停事务与领取事务不可能交错通过（§16 竞态用行锁/最终检查解决）。
///
/// 锁序说明：claim 现有锁序 project → run → task；本函数在其后追加 product
/// 行锁。lifecycle 事务只持 product 单行锁，无反向持锁，不成环。
pub(crate) async fn product_gate_denied(
    tx: &mut sqlx::PgConnection,
    task_id: Uuid,
) -> Result<Option<(Uuid, &'static str)>, sqlx::Error> {
    // 锁**全部**有效关联行（不按状态过滤）：normal 行也必须持锁，否则
    // 「评估=放行 → 写快照前被并发暂停插入」的交错会让 claim 成功但证据
    // 记录 blocked——行锁保证 claim 事务内看到的 Product 状态到提交为止
    // 不可变（§16 竞态解决方案）。
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT p.id, p.status \
           FROM delivery.task_product tp \
           JOIN core.product p ON p.id = tp.product_id \
          WHERE tp.task_id = $1 AND tp.withdrawn_at IS NULL \
          ORDER BY p.id \
          FOR UPDATE OF p",
    )
    .bind(task_id)
    .fetch_all(&mut *tx)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(product_id, status)| match status.as_str() {
            "suspended" => Some((product_id, "product_suspended")),
            "archived" => Some((product_id, "product_archived")),
            _ => None,
        })
        .next())
}

/// 领取成功路径的门禁快照落库（同一 claim 事务内调用）：
/// 1. 依赖逐边快照——前置 Task、其当前 Revision、条件、交付物引用、检查结论
///    （成功领取时全部边已通过 [`crate::task_dependency::dependencies_satisfied`]，
///    satisfied 恒 true；行集是「当时为何允许执行」的证据，审查81）；
/// 2. Product 门禁快照——各有效关联 Product 的执行状态与放行结论（jsonb 列，
///    无关联 = 空数组）。
///
/// Probe 模式领取后立即释放，重领会再写一批快照行——按 checked_at 顺序即
/// 评估历史，不做去重。
pub(crate) async fn write_run_gate_snapshots(
    tx: &mut sqlx::PgConnection,
    project_id: Uuid,
    task_id: Uuid,
    run_id: Uuid,
) -> Result<(), sqlx::Error> {
    let edges: Vec<(Uuid, Option<Uuid>, String, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT d.depends_on_task_id, u.current_task_revision_id, \
                d.required_condition, d.required_deliverable_ref \
           FROM delivery.task_dependency d \
           JOIN delivery.task u ON u.project_id = d.project_id \
                AND u.id = d.depends_on_task_id \
          WHERE d.project_id = $1 AND d.task_id = $2",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_all(&mut *tx)
    .await?;
    for (predecessor_task_id, revision_id, condition, deliverable_ref) in &edges {
        sqlx::query(
            "INSERT INTO delivery.task_run_dependency_snapshot \
             (tenant_id, project_id, run_id, task_id, predecessor_task_id, \
              predecessor_revision_id, required_condition, required_deliverable_ref, \
              satisfied, checked_at) \
             SELECT tenant_id, project_id, $2, $3, $4, $5, $6, $7, TRUE, now() \
               FROM delivery.task WHERE id = $3",
        )
        .bind(project_id)
        .bind(run_id)
        .bind(task_id)
        .bind(predecessor_task_id)
        .bind(revision_id)
        .bind(condition)
        .bind(deliverable_ref)
        .execute(&mut *tx)
        .await?;
    }

    let products: Vec<serde_json::Value> = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT p.id, p.status \
           FROM delivery.task_product tp \
           JOIN core.product p ON p.id = tp.product_id \
          WHERE tp.task_id = $1 AND tp.withdrawn_at IS NULL \
          ORDER BY p.id",
    )
    .bind(task_id)
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|(product_id, status)| {
        serde_json::json!({
            "product_id": product_id,
            "execution_state": status,
            "allowed": status == "normal" || status == "resuming",
        })
    })
    .collect();
    sqlx::query(
        "UPDATE delivery.task_run SET product_gate_snapshot = $2, \
                product_gate_snapshot_at = now() WHERE id = $1",
    )
    .bind(run_id)
    .bind(serde_json::Value::Array(products))
    .execute(&mut *tx)
    .await?;
    Ok(())
}
