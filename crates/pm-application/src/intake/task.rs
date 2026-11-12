//! P3-3 任务状态推进（Platform 唯一写入面，全 CAS）与人工重试。

use super::*;
use super::read::TaskRow;

/// PENDING → DISPATCHED（请求事件被消费时确认；重投递下非 PENDING 时零变更
/// 即幂等）。
pub async fn mark_task_dispatched(pool: &PgPool, task_id: Uuid) -> Result<(), IntakeError> {
    let n = sqlx::query(
        "UPDATE integration.intake_task SET task_status = 'DISPATCHED', updated_at = now() \
          WHERE id = $1 AND task_status = 'PENDING'",
    )
    .bind(task_id)
    .execute(pool)
    .await?
    .rows_affected();
    if n == 0 {
        // 已 DISPATCHED/PROCESSING/终态：确认消费，不报错（至少一次投递）。
        ensure_task_exists(pool, task_id).await?;
    }
    Ok(())
}

/// DISPATCHED → PROCESSING（服务开始处理确认）。
pub async fn mark_task_processing(pool: &PgPool, task_id: Uuid) -> Result<(), IntakeError> {
    let n = sqlx::query(
        "UPDATE integration.intake_task SET task_status = 'PROCESSING', updated_at = now() \
          WHERE id = $1 AND task_status = 'DISPATCHED'",
    )
    .bind(task_id)
    .execute(pool)
    .await?
    .rows_affected();
    if n == 0 {
        ensure_task_exists(pool, task_id).await?;
    }
    Ok(())
}

/// 任务终态回写（服务结果事件经 Platform 消费端；result_ref = DomainRef
/// 四元组，§7.3）。CAS：仅活跃态可进终态——过期/重放事件零副作用。
pub async fn complete_task(
    pool: &PgPool,
    task_id: Uuid,
    result_ref: serde_json::Value,
) -> Result<(), IntakeError> {
    sqlx::query(
        "UPDATE integration.intake_task \
            SET task_status = 'COMPLETED', result_ref = $2, last_error = NULL, updated_at = now() \
          WHERE id = $1 AND task_status IN ('PENDING','DISPATCHED','PROCESSING')",
    )
    .bind(task_id)
    .bind(&result_ref)
    .execute(pool)
    .await?;
    Ok(())
}

/// 失败上报（服务失败事件 / 投递超限）：活跃态 → FAILED，last_error 留痕。
pub async fn fail_task(pool: &PgPool, task_id: Uuid, error: &str) -> Result<(), IntakeError> {
    sqlx::query(
        "UPDATE integration.intake_task \
            SET task_status = 'FAILED', last_error = $2, updated_at = now() \
          WHERE id = $1 AND task_status IN ('PENDING','DISPATCHED','PROCESSING')",
    )
    .bind(task_id)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

async fn ensure_task_exists(pool: &PgPool, task_id: Uuid) -> Result<(), IntakeError> {
    let found: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM integration.intake_task WHERE id = $1")
            .bind(task_id)
            .fetch_optional(pool)
            .await?;
    found.map(|_| ()).ok_or(IntakeError::TaskNotFound(task_id))
}

/// FAILED / NEEDS_HUMAN_REVIEW 任务人工重试：原行 attempt+1 → PENDING →
/// 重写派发事件（新幂等键 retry:{task_id}:{attempt}，服务侧按任务幂等键
/// 去重）。审计留痕由调用方（REST 层）写入。
pub async fn retry_task(
    pool: &PgPool,
    intake_id: Uuid,
    task_id: Uuid,
) -> Result<TaskRow, IntakeError> {
    let mut tx = pool.begin().await?;
    let row: Option<(String, String, String)> = sqlx::query_as(
        "UPDATE integration.intake_task \
            SET task_status = 'PENDING', attempt = attempt + 1, last_error = NULL, \
                updated_at = now() \
          WHERE id = $1 AND intake_id = $2 \
            AND task_status IN ('FAILED','NEEDS_HUMAN_REVIEW') \
         RETURNING target_service, task_type, phase",
    )
    .bind(task_id)
    .bind(intake_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((target_service, task_type, phase)) = row else {
        // 区分「不存在」与「状态不可重试」。
        let exists: Option<String> = sqlx::query_scalar(
            "SELECT task_status FROM integration.intake_task \
              WHERE id = $1 AND intake_id = $2",
        )
        .bind(task_id)
        .bind(intake_id)
        .fetch_optional(&mut *tx)
        .await?;
        return Err(match exists {
            None => IntakeError::TaskNotFound(task_id),
            Some(status) => IntakeError::NotRetryable(task_id, status),
        });
    };
    let attempt: i32 =
        sqlx::query_scalar("SELECT attempt FROM integration.intake_task WHERE id = $1")
            .bind(task_id)
            .fetch_one(&mut *tx)
            .await?;
    let project_id: Uuid =
        sqlx::query_scalar("SELECT project_id FROM integration.intake_record WHERE id = $1")
            .bind(intake_id)
            .fetch_one(&mut *tx)
            .await?;
    let idempotency_key = format!("retry:{task_id}:{attempt}");
    let payload = json!({
        "intake_ref": {
            "target_domain": "requirement",
            "target_type": "intake_record",
            "target_id": intake_id.to_string(),
            "target_revision_id": null,
        },
        "task_ref": {
            "target_domain": "requirement",
            "target_type": "intake_task",
            "target_id": task_id.to_string(),
            "target_revision_id": null,
        },
        "retry": true,
    });
    PgOutboxAppend
        .append(
            &mut tx,
            OutboxEvent {
                project_id,
                event_type: dispatch_event_type(&task_type),
                aggregate_type: "intake_task",
                aggregate_id: &task_id.to_string(),
                payload,
                idempotency_key: &idempotency_key,
            },
        )
        .await?;
    tx.commit().await?;
    Ok(TaskRow {
        task_id,
        target_service,
        task_type,
        phase,
        task_status: "PENDING".to_string(),
        attempt,
        idempotency_key,
        signal_ref: None,
        result_ref: None,
        last_error: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}
