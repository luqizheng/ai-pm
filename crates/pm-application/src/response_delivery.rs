//! ACP-04 · Response Delivery 持久化（0050）。
//!
//! 联合数据契约：`docs/ui/s03-acp/smark-pm-s03-acp-slice-design-acp04-05-joint-v1.md`
//! V1.0 §2（I-1 接口）+ 联合架构裁决 2026-11-07（五问通过）。
//!
//! 语义锚点：
//! - **一回复一投递账**（UNIQUE(response_id)）：失败重投**复用原行**
//!   （attempt_count+1），不因重投创建新行（V1.2 §1.4 / ACP-471）；
//! - **不参与 Task 完成判定**（裁决③/ACP-C2）：本表是投递/消费的记账域，
//!   只服务重投幂等、unconsumed 告警、§5.3 取证——执行授权仍在租约三要素；
//! - **consumed 从推导态转为持久化事实**（裁决收口方向）：审计不得经
//!   Interaction 状态间接推断消费；
//! - ② generation 表带列、V1 恒 1（R-8）；⑤ continuation_id 先落可空
//!   （ACP-05/0051 回填，不另建 Session 身份）。

use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::append_audit_with_after;

#[derive(Debug, thiserror::Error)]
pub enum DeliveryAccountError {
    #[error("database error: {0}")]
    Db(String),
    #[error("delivery not found")]
    NotFound,
    #[error("{0}")]
    Validation(String),
}

/// 投递账行（GET/查询元素）。
#[derive(Debug, Clone, Serialize)]
pub struct ResponseDeliveryRow {
    pub id: Uuid,
    pub interaction_id: Uuid,
    pub response_id: Uuid,
    pub continue_command_id: Option<Uuid>,
    pub run_id: Uuid,
    pub continuation_id: Option<Uuid>,
    pub continuation_generation: i32,
    pub status: String,
    pub attempt_count: i32,
}

/// 回复持久化事务内的三写第三笔（response + command + delivery，ACP-470）。
/// 幂等：response_id 唯一索引兜底（重放路径不会重复进入——上游重放早退）。
pub async fn create_on_response_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    interaction_id: Uuid,
    response_id: Uuid,
    run_id: Uuid,
    continue_command_id: Uuid,
    actor: &AuditActor,
) -> Result<Uuid, DeliveryAccountError> {
    let delivery_id = sqlx::query_scalar(
        "INSERT INTO delivery.interaction_response_delivery \
           (tenant_id, project_id, interaction_id, response_id, continue_command_id, run_id) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $2), $2, $3, $4, $5, $6) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(project_id)
    .bind(interaction_id)
    .bind(response_id)
    .bind(continue_command_id)
    .bind(run_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DeliveryAccountError::Db(e.to_string()))?;
    append_audit_with_after(
        tx,
        project_id,
        actor,
        "delivery.created",
        "interaction_response_delivery",
        Some(delivery_id),
        serde_json::json!({
            "response_id": response_id,
            "interaction_id": interaction_id,
            "run_id": run_id,
            "continue_command_id": continue_command_id,
        }),
    )
    .await
    .map_err(|e| DeliveryAccountError::Db(e.to_string()))?;
    Ok(delivery_id)
}

/// 命令被领取（poll claim）→ `delivering`（attempt_count+1；failed 重投复用
/// 原行回置）。幂等：仅 pending/failed 行推进。
pub async fn mark_delivering_by_command(
    pool: &PgPool,
    command_ids: &[Uuid],
) -> Result<u64, DeliveryAccountError> {
    if command_ids.is_empty() {
        return Ok(0);
    }
    let result = sqlx::query(
        "UPDATE delivery.interaction_response_delivery \
            SET status = 'delivering', attempt_count = attempt_count + 1, \
                last_attempt_at = now(), updated_at = now() \
          WHERE continue_command_id = ANY($1) AND status IN ('pending', 'delivering', 'failed')",
    )
    .bind(command_ids)
    .execute(pool)
    .await
    .map_err(|e| DeliveryAccountError::Db(e.to_string()))?;
    Ok(result.rows_affected())
}

/// continue acked（INPUT_INJECTED）→ `delivered`。**调用方必须在 F5 恢复事务
/// 内**（R-4：shield 释放/Task 恢复/continuation 推进与投递记账原子）。
/// 返回 delivery_id（供 §5.3 resolution_note 引用）。
pub async fn mark_delivered_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    continue_command_id: Uuid,
    actor: &AuditActor,
) -> Result<Option<Uuid>, DeliveryAccountError> {
    let updated = sqlx::query(
        "UPDATE delivery.interaction_response_delivery \
            SET status = 'delivered', delivered_at = now(), updated_at = now() \
          WHERE continue_command_id = $1 AND project_id = $2 \
            AND status IN ('pending', 'delivering', 'failed') \
         RETURNING id",
    )
    .bind(continue_command_id)
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| DeliveryAccountError::Db(e.to_string()))?;
    let Some(row) = updated else {
        return Ok(None);
    };
    let delivery_id: Uuid = row.get("id");
    append_audit_with_after(
        tx,
        project_id,
        actor,
        "delivery.delivered",
        "interaction_response_delivery",
        Some(delivery_id),
        serde_json::json!({ "continue_command_id": continue_command_id }),
    )
    .await
    .map_err(|e| DeliveryAccountError::Db(e.to_string()))?;
    Ok(Some(delivery_id))
}

/// continue 执行失败（注入失败/重试耗尽）→ `failed` + last_error。
/// 重投路径：命令重领 → [`mark_delivering_by_command`] 回置 + attempt+1。
pub async fn mark_failed_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    continue_command_id: Uuid,
    error: Option<&str>,
    actor: &AuditActor,
) -> Result<(), DeliveryAccountError> {
    let updated = sqlx::query(
        "UPDATE delivery.interaction_response_delivery \
            SET status = 'failed', last_error = $3, updated_at = now() \
          WHERE continue_command_id = $1 AND project_id = $2 \
            AND status <> 'consumed' \
         RETURNING id",
    )
    .bind(continue_command_id)
    .bind(project_id)
    .bind(error)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| DeliveryAccountError::Db(e.to_string()))?;
    if let Some(row) = updated {
        let delivery_id: Uuid = row.get("id");
        append_audit_with_after(
            tx,
            project_id,
            actor,
            "delivery.failed",
            "interaction_response_delivery",
            Some(delivery_id),
            serde_json::json!({ "continue_command_id": continue_command_id, "error": error }),
        )
        .await
        .map_err(|e| DeliveryAccountError::Db(e.to_string()))?;
    }
    Ok(())
}

/// 消费回执达 → `consumed`（**Interaction 级事实的持久化收口**：迟到/重复
/// 回执不得改变已确认的恢复结果——本函数只落 consumed_at/状态，无任何
/// Task/guard/执行面副作用，ACP-C2）。
pub async fn mark_consumed_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    delivery_id: Uuid,
    actor: &AuditActor,
) -> Result<bool, DeliveryAccountError> {
    let updated = sqlx::query(
        "UPDATE delivery.interaction_response_delivery \
            SET status = 'consumed', consumed_at = now(), updated_at = now() \
          WHERE id = $1 AND project_id = $2 AND status IN ('delivering', 'delivered') \
         RETURNING id",
    )
    .bind(delivery_id)
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| DeliveryAccountError::Db(e.to_string()))?;
    let Some(row) = updated else {
        return Ok(false);
    };
    let id: Uuid = row.get("id");
    append_audit_with_after(
        tx,
        project_id,
        actor,
        "delivery.consumed",
        "interaction_response_delivery",
        Some(id),
        serde_json::json!({ "delivery_id": delivery_id }),
    )
    .await
    .map_err(|e| DeliveryAccountError::Db(e.to_string()))?;
    Ok(true)
}

/// 按 response_id 查投递账（④ 只读面：ACP-05 消费入口之一）。
pub async fn get_by_response(
    pool: &PgPool,
    response_id: Uuid,
) -> Result<Option<ResponseDeliveryRow>, DeliveryAccountError> {
    let row = sqlx::query(
        "SELECT id, interaction_id, response_id, continue_command_id, run_id, \
                continuation_id, continuation_generation, status, attempt_count \
           FROM delivery.interaction_response_delivery WHERE response_id = $1",
    )
    .bind(response_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| DeliveryAccountError::Db(e.to_string()))?;
    Ok(row.map(|r| ResponseDeliveryRow {
        id: r.get("id"),
        interaction_id: r.get("interaction_id"),
        response_id: r.get("response_id"),
        continue_command_id: r.get("continue_command_id"),
        run_id: r.get("run_id"),
        continuation_id: r.get("continuation_id"),
        continuation_generation: r.get("continuation_generation"),
        status: r.get("status"),
        attempt_count: r.get("attempt_count"),
    }))
}
