//! ACP 通信面 · Interaction 领域服务（ACP-01）与 N-1 调度保护（V1.1 §4/§5）。
//!
//! 设计稿：`docs/ui/s03-acp/smark-pm-slice-design-acp01-02-08-v1.md`（V1.1，
//! R-ACP-1~9 已裁 2026-11-07）；基线 = S03-ACP V1.3.1（已冻结）。
//!
//! 纪律（V-6 / §12.10）：创建 Interaction、Task 转 awaiting_decision、
//! task_waiting upsert、调度保护 `human_wait`、terminate 命令创建 = 同一事务。
//! MCP（`pm-mcp`）与服务器 Worker 只是适配层，禁止绕过本模块写业务状态。
//!
//! 状态机（V1.3.1 §7.3）：
//! `pending → responded → consumed`；`pending → timed_out | cancelled`。
//! RESPONDED 未消费为合法稳态（评审 N-6）。
//!
//! 介入链（V1.1 §5.2）：链 = Interaction 及其 `retry_of` 后代；链开放 ⟺
//! 最新行 `chain_open = true`（持久化列，不由状态推导）。唯一活跃约束 =
//! `uq_task_interaction_open_chain`（同 Task 至多一条开放链）。
//!
//! 调度保护（V1.1 §4.1，`delivery.task_run.scheduling_shield`）：
//! none → human_wait →(回复持久化)→ start_wait →(continue acked)→ none；
//! human_wait →(超时)→ timed_out →(retry_of 新建)→ human_wait。
//! CANCELLED（Task 活）→ none；Task 终态 → none；情形 6（旧进程退出未确认）
//! 维持原值，启动禁止由 [`crate::worker_command::has_unconfirmed_termination`]
//! 承载。

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::command_executor::{apply_task_transition, system_context, TransitionFacts};
use crate::pm_write::{append_audit, append_audit_with_after};
use crate::run_dispatch::DISPATCH_SERVICE_PRINCIPAL;
use pm_domain::task::TaskStatus;

/// 人工等待默认期限（V1.3.1 §8.1：120s；Platform 持久化后不重算、不顺延）。
pub fn human_intervention_timeout_secs() -> i64 {
    std::env::var("AI_PM_HUMAN_INTERVENTION_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120)
}

#[derive(Debug, thiserror::Error)]
pub enum InteractionError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    /// 身份/权限不满足（§8.3 检查②③；fail-closed）。
    #[error("{0}")]
    Forbidden(String),
    /// 不泄露跨项目存在性（对齐 RunDispatchError::TaskNotFound 口径）。
    #[error("task or run not found in this project")]
    NotFound,
    #[error("state transition refused: {0}")]
    Transition(String),
}

/// `pm_report_interaction` 入参（MCP 适配层已完成租约三要素校验与人/Client
/// 成员校验；本模块只做业务事实校验）。
#[derive(Debug, Clone)]
pub struct ReportInteractionInput {
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub run_id: Uuid,
    /// Agent 侧幂等键（标记文件 schema `aipm.interaction.v1` 的 event_id）。
    pub event_id: String,
    /// 四值：needs_info / needs_decision / needs_approval / external_dependency。
    pub reason: String,
    pub title: String,
    pub question: String,
    pub options: Option<Value>,
    pub required_response: bool,
    pub checkpoint_path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReportedInteraction {
    pub interaction_id: Uuid,
    pub status: String,
    pub deadline_at: DateTime<Utc>,
    pub task_status: String,
    /// 幂等重放标记：true = 同 event_id 的既有 Interaction，未新建。
    pub replayed: bool,
}

const VALID_REASONS: [&str; 4] = [
    "needs_info",
    "needs_decision",
    "needs_approval",
    "external_dependency",
];

/// 创建 Interaction（V1.1 §1.3 / §5.1）。幂等键 = (run_id, event_id)。
///
/// 同一事务完成：介入链开放检查 → Interaction 行（chain_open=true）→
/// run.scheduling_shield=human_wait → Task 转 awaiting_decision（Claimed 先
/// 转 Running）→ task_waiting 活跃行 → terminate 命令下发（D-03 立即终止）。
pub async fn report_interaction(
    pool: &PgPool,
    input: &ReportInteractionInput,
    actor: &AuditActor,
) -> Result<ReportedInteraction, InteractionError> {
    if !VALID_REASONS.contains(&input.reason.as_str()) {
        return Err(InteractionError::Validation(format!(
            "invalid reason `{}` (expected one of {VALID_REASONS:?})",
            input.reason
        )));
    }
    if input.event_id.trim().is_empty() {
        return Err(InteractionError::Validation("event_id is required".into()));
    }
    if input.question.trim().is_empty() {
        return Err(InteractionError::Validation("question is required".into()));
    }

    let mut tx: Transaction<'static, Postgres> = pool.begin().await?;

    // ① Run 归属与形态校验（ACP 仅适用 Model B spawn，Q-5/D-05）。
    let run = sqlx::query(
        "SELECT r.id, r.status, r.lease_owner, r.lease_generation, r.executor_session, \
                r.execution_mode, t.status AS task_status \
           FROM delivery.task_run r \
           JOIN delivery.task t ON t.id = r.task_id \
          WHERE r.id = $1 AND r.task_id = $2 AND r.project_id = $3 \
            FOR UPDATE OF r",
    )
    .bind(input.run_id)
    .bind(input.task_id)
    .bind(input.project_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(InteractionError::NotFound)?;
    let run_status: String = run.get("status");
    let execution_mode: Option<String> = run.get("execution_mode");
    if execution_mode.as_deref() != Some("spawn") {
        return Err(InteractionError::Validation(
            "interaction reporting requires a spawn-mode run (Model B only; \
             Model A uses awaiting_decision/task_waiting)"
                .into(),
        ));
    }
    if !matches!(run_status.as_str(), "claimed" | "running") {
        return Err(InteractionError::Conflict(format!(
            "run is not active (status: {run_status})"
        )));
    }
    let run_lease_owner: Option<String> = run.get("lease_owner");
    let Some(lease_owner) = run_lease_owner.clone() else {
        return Err(InteractionError::Conflict(
            "run has no lease owner; cannot route terminate command".into(),
        ));
    };
    let lease_generation: i32 = run.get("lease_generation");
    let session_id: Option<String> = run.get("executor_session");
    let mut task_status: String = run.get("task_status");

    // ② 幂等重放：同 (run_id, event_id) 直接返回既有 Interaction。
    let existing = sqlx::query(
        "SELECT id, status, deadline_at FROM delivery.task_interaction \
          WHERE run_id = $1 AND event_id = $2",
    )
    .bind(input.run_id)
    .bind(&input.event_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(row) = existing {
        let id: Uuid = row.get("id");
        let status: String = row.get("status");
        let deadline_at: DateTime<Utc> = row.get("deadline_at");
        append_audit(
            &mut tx,
            input.project_id,
            actor,
            "interaction.report_replayed",
            "task_interaction",
            Some(id),
        )
        .await?;
        tx.commit().await?;
        return Ok(ReportedInteraction {
            interaction_id: id,
            status,
            deadline_at,
            task_status,
            replayed: true,
        });
    }

    // ③ 介入链唯一活跃（V1.1 §5.2）：同 Task 存在开放链 → Conflict（ACP-431）。
    let open_chain: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM delivery.task_interaction \
          WHERE task_id = $1 AND chain_open \
          FOR UPDATE",
    )
    .bind(input.task_id)
    .fetch_optional(&mut *tx)
    .await?;
    if open_chain.is_some() {
        return Err(InteractionError::Conflict(
            "an open intervention chain exists for this task".into(),
        ));
    }

    // ④ Interaction 行（V1.3.1 §12.1 全字段 + V1.1 chain_open/event_id 定案）。
    let interaction_id = Uuid::new_v4();
    let deadline_at =
        chrono::Utc::now() + chrono::Duration::seconds(human_intervention_timeout_secs());
    sqlx::query(
        "INSERT INTO delivery.task_interaction \
           (id, tenant_id, project_id, task_id, run_id, session_id, \
            continuation_generation, event_id, reason, title, question, options, \
            required_response, checkpoint_path, status, chain_open, deadline_at) \
         VALUES ($1, (SELECT tenant_id FROM core.project WHERE id = $2), $2, $3, $4, $5, \
                 $6, $7, $8, $9, $10, $11, $12, $13, 'pending', TRUE, $14)",
    )
    .bind(interaction_id)
    .bind(input.project_id)
    .bind(input.task_id)
    .bind(input.run_id)
    .bind(&session_id)
    .bind(lease_generation)
    .bind(&input.event_id)
    .bind(&input.reason)
    .bind(&input.title)
    .bind(&input.question)
    .bind(&input.options)
    .bind(input.required_response)
    .bind(&input.checkpoint_path)
    .bind(deadline_at)
    .execute(&mut *tx)
    .await?;

    // ⑤ N-1 调度保护：human_wait（§4.1；Run 活跃性上面已校验）。
    let shielded = sqlx::query(
        "UPDATE delivery.task_run SET scheduling_shield = 'human_wait' \
          WHERE id = $1 AND status IN ('claimed', 'running')",
    )
    .bind(input.run_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if shielded == 0 {
        return Err(InteractionError::Conflict(
            "run left the active state while reporting interaction".into(),
        ));
    }

    // ⑥ Task → awaiting_decision（Running→AwaitingDecision 合法边；Claimed 窗口
    //    先补 Claimed→Running——Agent 已在实际执行）。走统一受守卫入口（R1）。
    let ctx = system_context(&mut tx, input.project_id)
        .await
        .map_err(|e| InteractionError::Transition(e.to_string()))?;
    if task_status == "claimed" {
        apply_task_transition(
            &mut tx,
            input.task_id,
            None,
            TaskStatus::Running,
            &TransitionFacts::for_dispatch(),
            &ctx,
        )
        .await
        .map_err(|e| InteractionError::Transition(e.to_string()))?;
        task_status = "running".to_string();
    }
    if task_status == "running" {
        apply_task_transition(
            &mut tx,
            input.task_id,
            None,
            TaskStatus::AwaitingDecision,
            &TransitionFacts::for_dispatch(),
            &ctx,
        )
        .await
        .map_err(|e| InteractionError::Transition(e.to_string()))?;
        task_status = "awaiting_decision".to_string();
    } else {
        return Err(InteractionError::Conflict(format!(
            "task is not running (status: {task_status}); cannot enter awaiting_decision"
        )));
    }

    // ⑦ task_waiting 活跃行（chain_open ⟺ waiting 活跃，V1.1 §5.2 规则 5）。
    //    每任务一条活跃的唯一约束在此兜底其他来源的等待。
    sqlx::query(
        "INSERT INTO delivery.task_waiting \
           (project_id, task_id, reason, questions, deadline, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(input.project_id)
    .bind(input.task_id)
    .bind(&input.reason)
    .bind(json!([{
        "interaction_id": interaction_id,
        "event_id": input.event_id,
        "title": input.title,
        "question": input.question,
        "options": input.options,
        "required_response": input.required_response,
    }]))
    .bind(deadline_at)
    .execute(&mut *tx)
    .await
    .map_err(|e| map_waiting_conflict(e))?;

    // ⑧ D-03：Interaction 持久化同事务内下发 terminate 命令（V1.3.1 §11.3 步骤 3；
    //    §5.2 报文即 payload）。worker_id = lease_owner 快照（R-ACP-3）。
    crate::worker_command::issue_terminate_command(
        &mut tx,
        input.project_id,
        input.task_id,
        input.run_id,
        interaction_id,
        &lease_owner,
        lease_generation,
        session_id.as_deref(),
    )
    .await?;

    append_audit(
        &mut tx,
        input.project_id,
        actor,
        "interaction.created",
        "task_interaction",
        Some(interaction_id),
    )
    .await?;

    tx.commit().await?;
    Ok(ReportedInteraction {
        interaction_id,
        status: "pending".into(),
        deadline_at,
        task_status,
        replayed: false,
    })
}

fn map_waiting_conflict(e: sqlx::Error) -> InteractionError {
    if let sqlx::Error::Database(db) = &e {
        if db.constraint().is_some_and(|c| c.contains("uq_task_waiting_active")) {
            return InteractionError::Conflict(
                "task already has an active waiting row from another flow".into(),
            );
        }
    }
    InteractionError::Db(e)
}

/// 回复持久化回执（ACP-03 调用；V1.1 §4.1）：pending → responded，
/// 调度保护 human_wait → start_wait。回复本身经可信 REST/UI 持久化（A*）。
pub async fn record_response_persisted(
    pool: &PgPool,
    project_id: Uuid,
    interaction_id: Uuid,
    response_ref: Value,
    actor: &AuditActor,
) -> Result<(), InteractionError> {
    let mut tx = pool.begin().await?;
    record_response_persisted_tx(&mut tx, project_id, interaction_id, response_ref, actor).await?;
    tx.commit().await?;
    Ok(())
}

/// 事务核（ACP-03 §6 步骤 6）：pending → responded + guard start_wait + 审计。
pub(crate) async fn record_response_persisted_tx(
    tx: &mut Transaction<'static, Postgres>,
    project_id: Uuid,
    interaction_id: Uuid,
    response_ref: Value,
    actor: &AuditActor,
) -> Result<(), InteractionError> {
    let row = sqlx::query(
        "UPDATE delivery.task_interaction \
            SET status = 'responded', responded_at = now() \
          WHERE id = $1 AND project_id = $2 AND status = 'pending' \
         RETURNING task_id, run_id",
    )
    .bind(interaction_id)
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        InteractionError::Conflict("interaction is not in pending state".into())
    })?;
    let run_id: Uuid = row.get("run_id");
    sqlx::query(
        "UPDATE delivery.task_run SET scheduling_shield = 'start_wait' \
          WHERE id = $1 AND scheduling_shield = 'human_wait'",
    )
    .bind(run_id)
    .execute(&mut **tx)
    .await?;
    append_audit_with_payload(
        tx,
        project_id,
        actor,
        "interaction.responded",
        interaction_id,
        response_ref,
    )
    .await?;
    Ok(())
}

/// 消费回执（V1.1 §1.5 / N-6）：responded → consumed；链关闭；等待 resolve；
/// Task 回 Running（awaiting_decision → Running 需 waiting_resolved，§12.3）。
/// 证据不足（缺 `initial_input_injected` / `process_alive`）→ 拒绝，禁止伪造
/// CONSUMED（V1.3.1 §8.6；ACP-434）。RESPONDED 未消费稳态不受影响。
pub async fn mark_consumed(
    pool: &PgPool,
    project_id: Uuid,
    interaction_id: Uuid,
    delivery_id: Option<Uuid>,
    evidence: &Value,
    actor: &AuditActor,
) -> Result<(), InteractionError> {
    let kinds = evidence
        .get("kinds")
        .and_then(|k| k.as_array())
        .cloned()
        .unwrap_or_default();
    let has = |k: &str| kinds.iter().any(|v| v.as_str() == Some(k));
    if !(has("initial_input_injected") && has("process_alive")) {
        return Err(InteractionError::Conflict(
            "insufficient consumption evidence (requires initial_input_injected + process_alive)"
                .into(),
        ));
    }

    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "UPDATE delivery.task_interaction \
            SET status = 'consumed', consumed_at = now(), chain_open = FALSE \
          WHERE id = $1 AND project_id = $2 AND status = 'responded' \
         RETURNING task_id, run_id",
    )
    .bind(interaction_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        InteractionError::Conflict(
            "interaction is not in responded state (or already consumed)".into(),
        )
    })?;
    let task_id: Uuid = row.get("task_id");
    let run_id: Uuid = row.get("run_id");

    // 防御性释放（ACP-440：continue ack 丢失而消费回执先到的路径）。
    sqlx::query(
        "UPDATE delivery.task_run SET scheduling_shield = 'none' \
          WHERE id = $1 AND scheduling_shield = 'start_wait'",
    )
    .bind(run_id)
    .execute(&mut *tx)
    .await?;

    // 链关闭 → 等待 resolve（§5.3）。
    resolve_waiting(&mut tx, task_id, &format!("interaction {interaction_id} consumed")).await?;

    // Task 回 Running（仅当仍在 awaiting_decision；其他状态（如已取消）跳过）。
    let task_status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM delivery.task WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;
    if task_status.as_deref() == Some("awaiting_decision") {
        let ctx = system_context(&mut tx, project_id)
            .await
            .map_err(|e| InteractionError::Transition(e.to_string()))?;
        let mut facts = TransitionFacts::for_dispatch();
        facts.waiting_resolved = true;
        apply_task_transition(
            &mut tx,
            task_id,
            None,
            TaskStatus::Running,
            &facts,
            &ctx,
        )
        .await
        .map_err(|e| InteractionError::Transition(e.to_string()))?;
    }

    append_audit_with_payload(
        &mut tx,
        project_id,
        actor,
        "interaction.consumed",
        interaction_id,
        json!({ "delivery_id": delivery_id, "evidence": evidence }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// §8.2 超时清扫（Worker 循环调用）：过期 pending → timed_out（不解释为同意），
/// 调度保护 human_wait → timed_out；链保持开放（未处置不关闭，ACP-439）。
pub async fn interaction_timeout_sweep(pool: &PgPool) -> Result<usize, InteractionError> {
    let mut tx = pool.begin().await?;
    let rows = sqlx::query(
        "UPDATE delivery.task_interaction \
            SET status = 'timed_out', timed_out_at = now() \
          WHERE status = 'pending' AND deadline_at < now() \
         RETURNING id, project_id, run_id",
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut handled = 0usize;
    for row in &rows {
        let interaction_id: Uuid = row.get("id");
        let project_id: Uuid = row.get("project_id");
        let run_id: Uuid = row.get("run_id");
        sqlx::query(
            "UPDATE delivery.task_run SET scheduling_shield = 'timed_out' \
              WHERE id = $1 AND scheduling_shield = 'human_wait'",
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        append_audit(
            &mut tx,
            project_id,
            &AuditActor::dispatch(),
            "interaction.timed_out",
            "task_interaction",
            Some(interaction_id),
        )
        .await?;
        handled += 1;
    }
    tx.commit().await?;
    Ok(handled)
}

/// 介入取消（Task 活跃场景，V1.1 §4.1 情形 5）：链关闭 → guard 归 none →
/// 等待 resolve；Run 走既有回收/重领路径（携带原始上下文，无回复可携带）。
pub async fn cancel_open_interaction(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    reason: &str,
    actor: &AuditActor,
) -> Result<bool, InteractionError> {
    let mut tx = pool.begin().await?;
    let cancelled = cancel_open_interaction_tx(&mut tx, project_id, task_id, reason, actor).await?;
    tx.commit().await?;
    Ok(cancelled)
}

/// 介入取消（事务核，F-8）：供 [`cancel_open_interaction`] 与 Task 取消联动
/// （`update_task_status` → Cancelled 钩子，V1.1 §5.1 CANCELLED 触发行 / §14）
/// 共用——调用方负责事务边界。awaiting_decision → Running 的回转仅在 Task
/// 仍处等待态时发生（Task 已终态时跳过：§5.3 cancelled 行"或随 Task 终态"）。
pub(crate) async fn cancel_open_interaction_tx(
    tx: &mut Transaction<'static, Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    reason: &str,
    actor: &AuditActor,
) -> Result<bool, InteractionError> {
    let row = sqlx::query(
        "UPDATE delivery.task_interaction \
            SET status = 'cancelled', cancelled_at = now(), chain_open = FALSE \
          WHERE task_id = $1 AND project_id = $2 AND chain_open \
         RETURNING id, run_id",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let interaction_id: Uuid = row.get("id");
    let run_id: Uuid = row.get("run_id");
    sqlx::query(
        "UPDATE delivery.task_run SET scheduling_shield = 'none' \
          WHERE id = $1 AND scheduling_shield <> 'none'",
    )
    .bind(run_id)
    .execute(&mut **tx)
    .await?;
    resolve_waiting(tx, task_id, &format!("interaction cancelled: {reason}")).await?;
    // §5.3 映射行：cancelled（Task 活）→ 回转前置状态（awaiting_decision →
    // Running，waiting_resolved 事实由本次取消提供）；其他状态跳过。
    let task_status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM delivery.task WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?;
    if task_status.as_deref() == Some("awaiting_decision") {
        let ctx = system_context(tx, project_id)
            .await
            .map_err(|e| InteractionError::Transition(e.to_string()))?;
        let mut facts = TransitionFacts::for_dispatch();
        facts.waiting_resolved = true;
        apply_task_transition(tx, task_id, None, TaskStatus::Running, &facts, &ctx)
            .await
            .map_err(|e| InteractionError::Transition(e.to_string()))?;
    }
    append_audit_with_payload(
        tx,
        project_id,
        actor,
        "interaction.cancelled",
        interaction_id,
        json!({ "reason": reason }),
    )
    .await?;
    Ok(true)
}

/// 超时重试挂链（V1.1 §5.2 规则 2）：timed_out（或 cancelled 之后的合法重试）
/// 创建新 Interaction 行、旧开放行让出 chain_open。新行沿用原 reason 与 run。
/// 返回新 interaction_id。
pub async fn retry_interaction(
    pool: &PgPool,
    project_id: Uuid,
    previous_interaction_id: Uuid,
    actor: &AuditActor,
) -> Result<Uuid, InteractionError> {
    let mut tx = pool.begin().await?;
    let prev = sqlx::query(
        "SELECT id, task_id, run_id, reason, title, question, options, \
                required_response, checkpoint_path, status \
           FROM delivery.task_interaction \
          WHERE id = $1 AND project_id = $2 \
            FOR UPDATE",
    )
    .bind(previous_interaction_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(InteractionError::NotFound)?;
    let prev_status: String = prev.get("status");
    if !matches!(prev_status.as_str(), "timed_out" | "cancelled") {
        return Err(InteractionError::Conflict(
            "only a timed_out or cancelled interaction can be retried".into(),
        ));
    }
    // 旧行让出（superseded）；若此前行已是关闭态则无行可让。
    let superseded = sqlx::query(
        "UPDATE delivery.task_interaction SET chain_open = FALSE \
          WHERE id = $1 AND chain_open",
    )
    .bind(previous_interaction_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if superseded == 0 {
        return Err(InteractionError::Conflict(
            "previous interaction chain is already closed".into(),
        ));
    }
    let task_id: Uuid = prev.get("task_id");
    let run_id: Uuid = prev.get("run_id");

    // 新行：deadline 重置（retry_of 挂链），guard 回 human_wait。
    let run = sqlx::query(
        "SELECT lease_owner, lease_generation, executor_session, status \
           FROM delivery.task_run WHERE id = $1 FOR UPDATE",
    )
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(InteractionError::NotFound)?;
    let run_status: String = run.get("status");
    if !matches!(run_status.as_str(), "claimed" | "running") {
        return Err(InteractionError::Conflict(format!(
            "run is not active (status: {run_status})"
        )));
    }
    let lease_owner: Option<String> = run.get("lease_owner");
    let Some(lease_owner) = lease_owner else {
        return Err(InteractionError::Conflict(
            "run has no lease owner; cannot route terminate command".into(),
        ));
    };
    let lease_generation: i32 = run.get("lease_generation");
    let session_id: Option<String> = run.get("executor_session");

    let interaction_id = Uuid::new_v4();
    // 重试的 event_id 派生自前次（同一 Agent 事件的重试语义，Platform 幂等键独立）。
    let event_id = format!("retry:{previous_interaction_id}");
    let deadline_at =
        chrono::Utc::now() + chrono::Duration::seconds(human_intervention_timeout_secs());
    sqlx::query(
        "INSERT INTO delivery.task_interaction \
           (id, tenant_id, project_id, task_id, run_id, session_id, \
            continuation_generation, event_id, reason, title, question, options, \
            required_response, checkpoint_path, status, chain_open, deadline_at, retry_of) \
         VALUES ($1, (SELECT tenant_id FROM core.project WHERE id = $2), $2, $3, $4, $5, \
                 $6, $7, $8, $9, $10, $11, $12, $13, 'pending', TRUE, $14, $15)",
    )
    .bind(interaction_id)
    .bind(project_id)
    .bind(task_id)
    .bind(run_id)
    .bind(&session_id)
    .bind(lease_generation)
    .bind(&event_id)
    .bind(prev.get::<String, _>("reason"))
    .bind(prev.get::<String, _>("title"))
    .bind(prev.get::<String, _>("question"))
    .bind(prev.get::<Option<Value>, _>("options"))
    .bind(prev.get::<bool, _>("required_response"))
    .bind(prev.get::<Option<String>, _>("checkpoint_path"))
    .bind(deadline_at)
    .bind(previous_interaction_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE delivery.task_run SET scheduling_shield = 'human_wait' \
          WHERE id = $1 AND status IN ('claimed', 'running')",
    )
    .bind(run_id)
    .execute(&mut *tx)
    .await?;

    crate::worker_command::issue_terminate_command(
        &mut tx,
        project_id,
        task_id,
        run_id,
        interaction_id,
        &lease_owner,
        lease_generation,
        session_id.as_deref(),
    )
    .await?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "interaction.retried",
        "task_interaction",
        Some(interaction_id),
    )
    .await?;
    tx.commit().await?;
    Ok(interaction_id)
}

/// task_waiting 活跃行 resolve（对齐 review/decide.rs end_waiting 语义）。
async fn resolve_waiting(
    tx: &mut Transaction<'static, Postgres>,
    task_id: Uuid,
    note: &str,
) -> Result<(), InteractionError> {
    sqlx::query(
        "UPDATE delivery.task_waiting \
            SET ended_at = now(), ended_by = $2, resolution_note = $3 \
          WHERE task_id = $1 AND ended_at IS NULL",
    )
    .bind(task_id)
    .bind(DISPATCH_SERVICE_PRINCIPAL)
    .bind(note)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// audit 包装：`append_audit` 的 after 载荷版（payload 进 after JSON）。
async fn append_audit_with_payload(
    tx: &mut Transaction<'static, Postgres>,
    project_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_id: Uuid,
    after: Value,
) -> Result<(), InteractionError> {
    append_audit_with_after(
        tx,
        project_id,
        actor,
        action,
        "task_interaction",
        Some(target_id),
        after,
    )
    .await?;
    Ok(())
}

// ───────────────────────── ACP-03：Human Response 提交（§4/§6） ─────────────────────────

/// `submit_interaction_response` 入参（REST handler 已过可见性门/auth_mode 门；
/// 服务层对 §8.3 全部六项 fail-closed 二次断言）。
#[derive(Debug, Clone)]
pub struct SubmitResponseInput {
    pub project_id: Uuid,
    pub interaction_id: Uuid,
    /// 回复者主体（检查①③；REST = AuthedPrincipal.principal_id）。
    pub responder_principal_id: Uuid,
    pub idempotency_key: String,
    pub response_payload: Value,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SubmittedResponse {
    pub response_id: Uuid,
    pub interaction_id: Uuid,
    pub status: String,
    /// 幂等重放标记（§8.4）。
    pub replayed: bool,
    /// 同事务下发的 continue 命令（V1 投递事实；delivery 细账随 ACP-04）。
    pub continue_command_id: Option<Uuid>,
}

/// 人类回复提交（V1.3.1 §8.3 六项检查 + §12.2 持久化 + §8.5 持久化→投递，
/// 全部同一事务——V-6）。R-ACP-7：`continuation_start_frozen_at` 不阻提交。
pub async fn submit_interaction_response(
    pool: &PgPool,
    input: &SubmitResponseInput,
    actor: &AuditActor,
) -> Result<SubmittedResponse, InteractionError> {
    if input.idempotency_key.trim().is_empty() {
        return Err(InteractionError::Validation(
            "idempotency_key is required".into(),
        ));
    }
    // 检查②身份半边：回复者必须是真人主体（api_key 门在 REST handler 的
    // auth_mode 层，此处 principal.kind 双保险）。
    let is_human: Option<bool> = sqlx::query_scalar(
        "SELECT kind = 'human' FROM core.principal WHERE id = $1",
    )
    .bind(input.responder_principal_id)
    .fetch_optional(pool)
    .await?
    .flatten();
    if is_human != Some(true) {
        return Err(InteractionError::Forbidden(
            "responses must come from a human principal".into(),
        ));
    }

    let mut tx = pool.begin().await?;

    // 检查①：interaction 归属当前项目（跨项目 → NotFound 不泄露）。
    let interaction = sqlx::query(
        "SELECT id, task_id, run_id, status, reason, deadline_at, options, required_response \
           FROM delivery.task_interaction \
          WHERE id = $1 AND project_id = $2 \
            FOR UPDATE",
    )
    .bind(input.interaction_id)
    .bind(input.project_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(InteractionError::NotFound)?;
    let status: String = interaction.get("status");
    let reason: String = interaction.get("reason");
    let deadline_at: chrono::DateTime<chrono::Utc> = interaction.get("deadline_at");
    let options: Option<Value> = interaction.get("options");
    let required_response: bool = interaction.get("required_response");
    let task_id: Uuid = interaction.get("task_id");
    let run_id: Uuid = interaction.get("run_id");

    // 检查③（R-ACP-10 已裁 a 分级）：needs_approval → 治理级（Lead/Approver）。
    if reason == "needs_approval" {
        crate::approver::ensure_governance_authorized(
            pool,
            input.project_id,
            input.responder_principal_id,
        )
        .await
        .map_err(|_| {
            InteractionError::Forbidden(
                "this interaction requires Project Lead or an authorized Approver".into(),
            )
        })?;
    }

    // 幂等重放（§8.4）：同 (interaction, idempotency_key) 返回原行零副作用。
    // 必须先于检查④⑤⑥——首次提交后 Interaction 即离开 pending，重放若被
    // 状态门槛拦截则幂等语义失效（ACP-452 竞态后置：重放任何时刻可达）。
    let replay: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT id, continue_command_id \
           FROM delivery.interaction_response \
          WHERE interaction_id = $1 AND idempotency_key = $2",
    )
    .bind(input.interaction_id)
    .bind(&input.idempotency_key)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((response_id, continue_command_id)) = replay {
        tx.commit().await?;
        return Ok(SubmittedResponse {
            response_id,
            interaction_id: input.interaction_id,
            status: "responded".into(),
            replayed: true,
            continue_command_id,
        });
    }

    // 检查④：仅 pending 可回复（R-ACP-7：frozen 不阻提交）。
    if status != "pending" {
        return Err(InteractionError::Conflict(format!(
            "interaction is not in pending state (status: {status})"
        )));
    }
    // 检查⑤：deadline（belt-and-braces，§8.2 清扫间隙竞态兜底）。
    if chrono::Utc::now() > deadline_at {
        return Err(InteractionError::Conflict(
            "human intervention deadline has passed".into(),
        ));
    }
    // 检查⑥：响应结构（options 非空时 decision 必须命中枚举；content 自由文本兜底）。
    validate_response_payload(&input.response_payload, options.as_ref(), required_response)?;

    // 运行侧快照（continue 路由目标）：lease_owner/lease_generation。
    let run = sqlx::query(
        "SELECT lease_owner, lease_generation FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(InteractionError::NotFound)?;
    let lease_owner: Option<String> = run.get("lease_owner");
    let Some(worker_id) = lease_owner else {
        return Err(InteractionError::Conflict(
            "run has no lease owner; cannot route continue command".into(),
        ));
    };
    let lease_generation: i32 = run.get("lease_generation");

    // §9.3 并发裁决：同 Interaction 单回复唯一索引，败者 409（无静默覆盖）。
    let response_id = Uuid::new_v4();
    let authority = if reason == "needs_approval" {
        "lead_or_approver"
    } else {
        "project_member"
    };
    let authorization_context = json!({
        "responder": input.responder_principal_id,
        "reason": reason,
        "authority": authority,
        "checks": { "membership": true, "human": true, "authority": true,
                    "status": true, "deadline": true, "payload": true },
    });
    let insert = sqlx::query(
        "INSERT INTO delivery.interaction_response \
           (id, tenant_id, project_id, interaction_id, task_id, run_id, responded_by, \
            response_payload, idempotency_key, authorization_context) \
         VALUES ($1, (SELECT tenant_id FROM core.project WHERE id = $2), $2, $3, $4, $5, $6, \
                 $7, $8, $9)",
    )
    .bind(response_id)
    .bind(input.project_id)
    .bind(input.interaction_id)
    .bind(task_id)
    .bind(run_id)
    .bind(input.responder_principal_id)
    .bind(&input.response_payload)
    .bind(&input.idempotency_key)
    .bind(&authorization_context)
    .execute(&mut *tx)
    .await;
    if let Err(sqlx::Error::Database(db)) = &insert {
        if db
            .constraint()
            .is_some_and(|c| c.contains("uq_interaction_response_single"))
        {
            return Err(InteractionError::Conflict(
                "another response has already been recorded for this interaction".into(),
            ));
        }
    }
    insert?;

    // 步骤 6/7：pending → responded（guard start_wait）+ 同事务下发 continue。
    record_response_persisted_tx(
        &mut tx,
        input.project_id,
        input.interaction_id,
        json!({ "response_id": response_id, "authorization_context": authorization_context }),
        actor,
    )
    .await?;
    let continue_command_id = crate::worker_command::issue_continue_command(
        &mut tx,
        input.project_id,
        task_id,
        run_id,
        input.interaction_id,
        None,
        &worker_id,
        lease_generation,
        input.response_payload.clone(),
    )
    .await?;
    sqlx::query(
        "UPDATE delivery.interaction_response SET continue_command_id = $2 WHERE id = $1",
    )
    .bind(response_id)
    .bind(continue_command_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(SubmittedResponse {
        response_id,
        interaction_id: input.interaction_id,
        status: "responded".into(),
        replayed: false,
        continue_command_id: Some(continue_command_id),
    })
}

/// 检查⑥：`required_response=true` 且 options 非空 → decision 必须命中枚举；
/// 否则 content/decision 至少其一非空。
fn validate_response_payload(
    payload: &Value,
    options: Option<&Value>,
    required_response: bool,
) -> Result<(), InteractionError> {
    if !payload.is_object() {
        return Err(InteractionError::Validation(
            "response_payload must be a JSON object".into(),
        ));
    }
    let decision = payload.get("decision").and_then(|v| v.as_str());
    let content = payload.get("content").and_then(|v| v.as_str());
    let option_values: Option<Vec<&str>> = options.and_then(|o| o.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|o| o.get("value").and_then(|v| v.as_str()))
            .collect()
    });
    if let Some(values) = option_values.filter(|v| !v.is_empty()) {
        match decision {
            Some(d) if values.contains(&d) => {}
            Some(d) => {
                return Err(InteractionError::Validation(format!(
                    "decision `{d}` is not one of the offered options"
                )))
            }
            None => {
                return Err(InteractionError::Validation(
                    "decision is required (offered options present)".into(),
                ))
            }
        }
    } else if required_response
        && decision.is_none()
        && content.is_none_or(|c| c.trim().is_empty())
    {
        return Err(InteractionError::Validation(
            "response_payload requires a decision or non-empty content".into(),
        ));
    }
    Ok(())
}
