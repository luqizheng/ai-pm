//! P3-4 父子聚合读面：派生整体状态（§9.4.2 规则表）、任务清单、聚合视图。

use super::*;

// ── P3-4 父子聚合（查询时派生，§9.4.2 规则表） ─────────────────────────────

/// L3 整体流程状态（派生，不落列）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DerivedIntakeStatus {
    Processing,
    Completed,
    NeedsHumanReview,
    Failed,
    /// 无任务防御值（创建必含任务；不发生）。
    Empty,
}

/// 派生父 Intake 状态（§9.4.2 优先级：NEEDS_HUMAN_REVIEW > 必需 FAILED >
/// 必需非终态 PROCESSING > 全部必需 COMPLETED）。
pub fn aggregate_task_status(tasks: &[(String, String)]) -> DerivedIntakeStatus {
    // tasks: (phase, task_status)
    let mut any_required_failed = false;
    let mut any_needs_human = false;
    let mut any_required_active = false;
    let mut any_required_exists = false;
    for (phase, status) in tasks {
        let required = phase == "required";
        if status == "NEEDS_HUMAN_REVIEW" {
            any_needs_human = true;
        }
        if required {
            any_required_exists = true;
            match status.as_str() {
                "FAILED" => any_required_failed = true,
                "PENDING" | "DISPATCHED" | "PROCESSING" => any_required_active = true,
                _ => {}
            }
        }
    }
    if any_needs_human {
        return DerivedIntakeStatus::NeedsHumanReview;
    }
    if any_required_failed {
        return DerivedIntakeStatus::Failed;
    }
    if any_required_active {
        return DerivedIntakeStatus::Processing;
    }
    if any_required_exists {
        return DerivedIntakeStatus::Completed;
    }
    DerivedIntakeStatus::Empty
}

/// 单个 Intake 的任务列表（状态校正查询与聚合共用）。
pub async fn list_tasks(pool: &PgPool, intake_id: Uuid) -> Result<Vec<TaskRow>, IntakeError> {
    let rows = sqlx::query(
        "SELECT id, target_service, task_type, phase, task_status, attempt, \
                idempotency_key, signal_ref, result_ref, last_error, created_at, updated_at \
           FROM integration.intake_task WHERE intake_id = $1 ORDER BY created_at",
    )
    .bind(intake_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| TaskRow {
            task_id: row.get("id"),
            target_service: row.get("target_service"),
            task_type: row.get("task_type"),
            phase: row.get("phase"),
            task_status: row.get("task_status"),
            attempt: row.get("attempt"),
            idempotency_key: row.get("idempotency_key"),
            signal_ref: row.get("signal_ref"),
            result_ref: row.get("result_ref"),
            last_error: row.get("last_error"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect())
}

/// intake_task 行投影（状态校正查询响应）。
#[derive(Debug, Serialize)]
pub struct TaskRow {
    pub task_id: Uuid,
    pub target_service: String,
    pub task_type: String,
    pub phase: String,
    pub task_status: String,
    pub attempt: i32,
    pub idempotency_key: String,
    pub signal_ref: Option<serde_json::Value>,
    pub result_ref: Option<serde_json::Value>,
    pub last_error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Intake 整体视图（聚合状态 + 任务明细；状态校正查询 §14.6/§17.3）。
#[derive(Debug, Serialize)]
pub struct IntakeState {
    pub intake_id: Uuid,
    pub intake_status: String,
    pub derived_status: DerivedIntakeStatus,
    pub tasks: Vec<TaskRow>,
}

pub async fn intake_state(pool: &PgPool, intake_id: Uuid) -> Result<IntakeState, IntakeError> {
    let record: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, intake_status FROM integration.intake_record WHERE id = $1")
            .bind(intake_id)
            .fetch_optional(pool)
            .await?;
    let (found_id, intake_status) = record.ok_or(IntakeError::NotFound)?;
    let tasks = list_tasks(pool, found_id).await?;
    let derived = aggregate_task_status(
        &tasks
            .iter()
            .map(|t| (t.phase.clone(), t.task_status.clone()))
            .collect::<Vec<_>>(),
    );
    Ok(IntakeState {
        intake_id: found_id,
        intake_status,
        derived_status: derived,
        tasks,
    })
}

// ── P3-5 人工重试（ADR-12：复用原任务行 attempt+1，禁止新建行） ────────────
