//! ACP-05 · Continuation 正式身份与状态机（0051）。
//!
//! 联合数据契约 V1.0 §3（I-2 接口）+ 联合架构裁决：
//! - **身份冻结**：Continuation 身份 = 本表 `id`；Platform Session ID =
//!   `task_run.executor_session`（R-8）；新 Session 标识 =
//!   `worker_command.receipt.session_id`。不允许第四种 Session 身份。
//! - **代次**：`generation` = 同 Run 内第 N 次继续（基线 §3.3，与
//!   lease_generation / attempt_no 完全分离）；B3-2 新 attempt Run 是 Run
//!   维度事件，continuation 不跨 Run 继承代次。
//! - **状态机**（生命周期最小集；06/07 职责不吸收——经接口接入）：
//!   `open →（continue acked，F5 同事务）→ started →（消费回执）→ consumed`
//!   ；`frozen`（terminate 重试耗尽，显式人工处置后随 acked 回 started）；
//!   `closed`（Run 终态 / Task 完成 / 取消钩子驱动，终态不可迁移）。
//! - **ACP-C2**：consumed 是持久化事实（本表），不承担 Task/guard/执行面职责；
//!   迁移只落状态与时间戳。
//! - **Recovery 先行**（裁决排序理由）：续领（recovery.rs）已就位——ack 推进
//!   与 R-4 恢复同事务在本批兑现。

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::append_audit_with_after;

#[derive(Debug, thiserror::Error)]
pub enum ContinuationError {
    #[error("database error: {0}")]
    Db(String),
    #[error("continuation not found")]
    NotFound,
}

/// 回复持久化事务内的第四笔（response + command + delivery + continuation，
/// ACP-474）：generation = 同 Run 内 max+1；同时回填 delivery 行的
/// `continuation_id` / `continuation_generation`（⑤ 前向列兑现）。
pub async fn open_continuation_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    run_id: Uuid,
    origin_interaction_id: Uuid,
    delivery_id: Uuid,
    actor: &AuditActor,
) -> Result<(Uuid, i32), ContinuationError> {
    let generation: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(generation), 0) + 1 FROM delivery.continuation WHERE run_id = $1",
    )
    .bind(run_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| ContinuationError::Db(e.to_string()))?;
    let id = sqlx::query_scalar(
        "INSERT INTO delivery.continuation \
           (tenant_id, project_id, task_id, run_id, origin_interaction_id, generation) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $2), $2, $3, $4, $5, $6) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(project_id)
    .bind(task_id)
    .bind(run_id)
    .bind(origin_interaction_id)
    .bind(generation)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| ContinuationError::Db(e.to_string()))?;
    // ⑤ delivery 回填（前向列兑现；单行 UPDATE）。
    sqlx::query(
        "UPDATE delivery.interaction_response_delivery \
            SET continuation_id = $2, continuation_generation = $3, updated_at = now() \
          WHERE id = $1",
    )
    .bind(delivery_id)
    .bind(id)
    .bind(generation)
    .execute(&mut **tx)
    .await
    .map_err(|e| ContinuationError::Db(e.to_string()))?;
    append_audit_with_after(
        tx,
        project_id,
        actor,
        "continuation.opened",
        "continuation",
        Some(id),
        serde_json::json!({
            "run_id": run_id,
            "origin_interaction_id": origin_interaction_id,
            "generation": generation,
            "delivery_id": delivery_id,
        }),
    )
    .await
    .map_err(|e| ContinuationError::Db(e.to_string()))?;
    Ok((id, generation))
}

/// continue acked（INPUT_INJECTED）→ `started`。**调用方必须在 F5 恢复事务内**
/// （与 mark_delivered / R-4 恢复并列；证据 = delivery 已 delivered）。
pub async fn mark_started_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    run_id: Uuid,
    origin_interaction_id: Uuid,
    actor: &AuditActor,
) -> Result<bool, ContinuationError> {
    let updated = sqlx::query(
        "UPDATE delivery.continuation \
            SET status = 'started', started_at = now() \
          WHERE run_id = $1 AND project_id = $2 AND origin_interaction_id = $3 \
            AND status IN ('open', 'frozen') \
         RETURNING id",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(origin_interaction_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| ContinuationError::Db(e.to_string()))?;
    let Some(row) = updated else {
        return Ok(false);
    };
    let id: Uuid = row.get("id");
    append_audit_with_after(
        tx,
        project_id,
        actor,
        "continuation.started",
        "continuation",
        Some(id),
        serde_json::json!({ "run_id": run_id }),
    )
    .await
    .map_err(|e| ContinuationError::Db(e.to_string()))?;
    Ok(true)
}

/// 消费回执达 → `consumed`（经 delivery 行定位——④ 只读消费方向的反向落账；
/// 仅 `started` 可推进；迟到/重复幂等 no-op）。
pub async fn mark_consumed_by_delivery_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    delivery_id: Uuid,
    actor: &AuditActor,
) -> Result<bool, ContinuationError> {
    let updated = sqlx::query(
        "UPDATE delivery.continuation c \
            SET status = 'consumed' \
           FROM delivery.interaction_response_delivery d \
          WHERE d.id = $1 AND d.continuation_id = c.id \
            AND c.project_id = $2 AND c.status = 'started' \
         RETURNING c.id",
    )
    .bind(delivery_id)
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| ContinuationError::Db(e.to_string()))?;
    let Some(row) = updated else {
        return Ok(false);
    };
    let id: Uuid = row.get("id");
    append_audit_with_after(
        tx,
        project_id,
        actor,
        "continuation.consumed",
        "continuation",
        Some(id),
        serde_json::json!({ "delivery_id": delivery_id }),
    )
    .await
    .map_err(|e| ContinuationError::Db(e.to_string()))?;
    Ok(true)
}

/// terminate 重试耗尽 → `frozen`（启动冻结语义镜像；原
/// `task_interaction.continuation_start_frozen_at` 双写保留过渡）。
pub async fn freeze_by_interaction_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    origin_interaction_id: Uuid,
    actor: &AuditActor,
) -> Result<bool, ContinuationError> {
    let updated = sqlx::query(
        "UPDATE delivery.continuation \
            SET status = 'frozen', frozen_at = now() \
          WHERE origin_interaction_id = $1 AND project_id = $2 \
            AND status IN ('open', 'started') \
         RETURNING id",
    )
    .bind(origin_interaction_id)
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| ContinuationError::Db(e.to_string()))?;
    let Some(row) = updated else {
        return Ok(false);
    };
    let id: Uuid = row.get("id");
    append_audit_with_after(
        tx,
        project_id,
        actor,
        "continuation.frozen",
        "continuation",
        Some(id),
        serde_json::json!({ "origin_interaction_id": origin_interaction_id }),
    )
    .await
    .map_err(|e| ContinuationError::Db(e.to_string()))?;
    Ok(true)
}

/// 终态钩子：Run 终态（finish_run）/ Task 取消 → 非终态 continuation 全部
/// `closed`（终态不可迁移——后续 UPDATE 被 status 守卫拒绝）。
pub async fn close_open_by_run_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    run_id: Uuid,
    reason: &str,
    actor: &AuditActor,
) -> Result<u64, ContinuationError> {
    let updated = sqlx::query(
        "UPDATE delivery.continuation \
            SET status = 'closed', closed_at = now() \
          WHERE run_id = $1 AND project_id = $2 AND status <> 'closed' \
         RETURNING id",
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| ContinuationError::Db(e.to_string()))?;
    for row in &updated {
        let id: Uuid = row.get("id");
        append_audit_with_after(
            tx,
            project_id,
            actor,
            "continuation.closed",
            "continuation",
            Some(id),
            serde_json::json!({ "run_id": run_id, "reason": reason }),
        )
        .await
        .map_err(|e| ContinuationError::Db(e.to_string()))?;
    }
    Ok(updated.len() as u64)
}

/// 终态钩子（Task 维度，DEF-E2E-1）：Task → Cancelled 时该 Task 全部 Run 的
/// 非终态 continuation 一律 `closed`——即使介入链已因 continue acked 关闭
/// （responded，chain_open=FALSE，[`cancel_open_interaction_tx`] 短路不触链，
/// E2E S9 形态）。闭集合按 ACP-477 契约取 `open/started/frozen`（设计稿
/// acp04-05-joint §状态机「Run 终态 / Task 完成 / 取消 → closed」触发行）；
/// `consumed` 是持久化事实（ACP-C2），终态钩子不回写。
pub async fn close_open_by_task_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    reason: &str,
    actor: &AuditActor,
) -> Result<u64, ContinuationError> {
    let updated: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "UPDATE delivery.continuation \
            SET status = 'closed', closed_at = now() \
          WHERE project_id = $1 AND status IN ('open', 'started', 'frozen') \
            AND run_id IN (SELECT id FROM delivery.task_run \
                           WHERE task_id = $2 AND project_id = $1) \
         RETURNING id, run_id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| ContinuationError::Db(e.to_string()))?;
    for (id, run_id) in &updated {
        append_audit_with_after(
            tx,
            project_id,
            actor,
            "continuation.closed",
            "continuation",
            Some(*id),
            serde_json::json!({ "run_id": run_id, "reason": reason }),
        )
        .await
        .map_err(|e| ContinuationError::Db(e.to_string()))?;
    }
    Ok(updated.len() as u64)
}

/// ④ 只读面：Run 当前活跃（非终态）Continuation——06（Checkpoint/Lock）与
/// 07（Deadline）经此接入。
pub async fn get_open_by_run(
    pool: &PgPool,
    run_id: Uuid,
) -> Result<Option<(Uuid, i32, String)>, ContinuationError> {
    let row = sqlx::query(
        "SELECT id, generation, status FROM delivery.continuation \
          WHERE run_id = $1 AND status IN ('open', 'started', 'frozen') \
          ORDER BY generation DESC LIMIT 1",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ContinuationError::Db(e.to_string()))?;
    Ok(row.map(|r| (r.get("id"), r.get("generation"), r.get("status"))))
}
