//! ACP 通信面 · Worker Command 命令查询面（V1.1 §3；基线 V1.3.1 §10）。
//!
//! 四态 `issued → delivered → acked | failed`（基线 §10.3，不改）；V1.1 P0-2
//! 领取租约：`delivered` = 「已领取且处于有界 claim 租约内」，claim 过期后同
//! worker 可用同一 command id 重投（同 id 幂等执行）；claim 过期 ≠ failed。
//!
//! 身份双轨（V1.1 §3.3，R-ACP-3 已裁）：命令面不依赖 Run Lease token——校验 =
//! 凭据（MCP/REST 适配层）+ `worker_id` 绑定（本模块）+ `lease_generation`
//! 只读比对（本模块）。ADR-0002 D2 管执行面写操作，命令收发不是执行写。
//!
//! 情形 6（旧进程退出未确认）：continue 命令在存在未回执 terminate 时不予
//! 下发（[`poll_commands`] 内置过滤）+ [`has_unconfirmed_termination`] 供
//! 执行端启动前自查（V1.3.1 §9.4 检查 8 的操作化）。

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::command_executor::{apply_task_transition, system_context, TransitionFacts};
use crate::pm_write::append_audit_with_after;
use pm_domain::task::TaskStatus;

pub const COMMAND_TERMINATE: &str = "agent.process.terminate";
pub const COMMAND_CONTINUE: &str = "agent.task.continue";

/// 领取租约时长（V1.1 §3.2；默认 300s）。
pub fn command_claim_lease_secs() -> i64 {
    std::env::var("AI_PM_COMMAND_CLAIM_LEASE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300)
}

#[derive(Debug, thiserror::Error)]
pub enum CommandFaceError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("{0}")]
    Validation(String),
    /// 不泄露跨项目存在性。
    #[error("command or run not found")]
    NotFound,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PolledCommand {
    pub command_id: Uuid,
    pub command_type: String,
    pub interaction_id: Option<Uuid>,
    pub payload: Value,
    pub issued_at: DateTime<Utc>,
    /// 第几次领取（重投可见性；claim 过期重投 +1）。
    pub attempt: i32,
}

/// 下发 terminate 命令（V1.3.1 §5.2 报文 = payload）。由
/// [`crate::interaction::report_interaction`] 在 Interaction 持久化事务内调用
/// （D-03：立即终止）。worker_id = run.lease_owner 快照（R-ACP-3）。
pub(crate) async fn issue_terminate_command(
    tx: &mut Transaction<'static, Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    run_id: Uuid,
    interaction_id: Uuid,
    worker_id: &str,
    lease_generation: i32,
    session_id: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    issue_command(
        tx,
        project_id,
        task_id,
        run_id,
        Some(interaction_id),
        None,
        COMMAND_TERMINATE,
        worker_id,
        lease_generation,
        json!({
            "command_type": COMMAND_TERMINATE,
            "task_id": task_id,
            "run_id": run_id,
            "interaction_id": interaction_id,
            "session_id": session_id,
            "reason": "human_intervention_required",
            "issued_at": chrono::Utc::now().to_rfc3339(),
        }),
    )
    .await
}

/// 下发 continue 命令（V1.3.1 §5.4 报文 = payload；回复经 REST/UI 持久化后，
/// 由 ACP-03/05 路径调用）。`continuation_id` 待 ACP-05 落表后必填。
#[allow(clippy::too_many_arguments)]
pub async fn issue_continue_command(
    tx: &mut Transaction<'static, Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    run_id: Uuid,
    interaction_id: Uuid,
    continuation_id: Option<Uuid>,
    worker_id: &str,
    lease_generation: i32,
    response: Value,
) -> Result<Uuid, sqlx::Error> {
    // F-1（复审 2026-11-07）：continue payload 携带 §9.4 前置快照（guard），
    // 执行端启动前校验——避免为前置检查新增只读工具。快照 = 下发时刻事实：
    // continuation_start_frozen（terminate 失败冻结）+ agent_deadline_at
    // （D-02 不冻结顺延，过期须显式上层授权 ACP-07）。执行端拒绝 = 命令
    // failed，回复保留幂等重投/转授权路径（"拒绝得干净"）。
    let (deadline_at, frozen): (Option<chrono::DateTime<chrono::Utc>>, bool) = sqlx::query_as(
        "SELECT r.deadline_at, \
                EXISTS (SELECT 1 FROM delivery.task_interaction i \
                         WHERE i.id = $2 AND i.continuation_start_frozen_at IS NOT NULL) \
           FROM delivery.task_run r WHERE r.id = $1",
    )
    .bind(run_id)
    .bind(interaction_id)
    .fetch_one(&mut **tx)
    .await?;
    issue_command(
        tx,
        project_id,
        task_id,
        run_id,
        Some(interaction_id),
        continuation_id,
        COMMAND_CONTINUE,
        worker_id,
        lease_generation,
        json!({
            "command_type": COMMAND_CONTINUE,
            "task_id": task_id,
            "run_id": run_id,
            "interaction_id": interaction_id,
            "continuation_id": continuation_id,
            "response": response,
            "guard": {
                "continuation_start_frozen": frozen,
                "agent_deadline_at": deadline_at.map(|d| d.to_rfc3339()),
            },
            "issued_at": chrono::Utc::now().to_rfc3339(),
        }),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn issue_command(
    tx: &mut Transaction<'static, Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    run_id: Uuid,
    interaction_id: Option<Uuid>,
    continuation_id: Option<Uuid>,
    command_type: &str,
    worker_id: &str,
    lease_generation: i32,
    payload: Value,
) -> Result<Uuid, sqlx::Error> {
    let command_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.worker_command \
           (id, tenant_id, project_id, command_type, worker_id, task_id, run_id, \
            interaction_id, continuation_id, lease_generation, payload, status) \
         VALUES ($1, (SELECT tenant_id FROM core.project WHERE id = $2), $2, $3, $4, $5, $6, \
                 $7, $8, $9, $10, 'issued')",
    )
    .bind(command_id)
    .bind(project_id)
    .bind(command_type)
    .bind(worker_id)
    .bind(task_id)
    .bind(run_id)
    .bind(interaction_id)
    .bind(continuation_id)
    .bind(lease_generation)
    .bind(&payload)
    .execute(&mut **tx)
    .await?;
    Ok(command_id)
}

/// 轮询领取（V1.1 §3.2）：原子置 delivered + claim 租约；同 worker 可重领
/// 自己未过期的 claim（幂等重启路径）；claim 过期的 delivered 对同 worker
/// 可重投。返回按 issued_at 排序的已领取命令。
///
/// 内置情形 6 过滤：存在未回执 terminate 时，continue 不予下发。
pub async fn poll_commands(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    worker_id: &str,
) -> Result<Vec<PolledCommand>, CommandFaceError> {
    // 路由目标校验前置：run 必须存在且属于项目（不泄露跨项目存在性）。
    let run_generation: Option<i32> = sqlx::query_scalar(
        "SELECT lease_generation FROM delivery.task_run WHERE id = $1 AND project_id = $2",
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or(CommandFaceError::NotFound)?;

    let rows = sqlx::query(
        "UPDATE delivery.worker_command c \
            SET status = 'delivered', \
                claimed_by = $3, claimed_at = now(), \
                claim_expires_at = now() + ($4 * interval '1 second'), \
                claim_count = claim_count + 1, \
                delivered_at = COALESCE(delivered_at, now()) \
          WHERE c.run_id = $1 AND c.project_id = $2 AND c.worker_id = $3 \
            AND c.lease_generation = $5 \
            AND (c.status = 'issued' \
                 OR (c.status = 'delivered' \
                     AND (c.claim_expires_at < now() OR c.claimed_by = $3))) \
            AND NOT (c.command_type = 'agent.task.continue' \
                     AND EXISTS (SELECT 1 FROM delivery.worker_command t \
                                  WHERE t.run_id = c.run_id \
                                    AND t.command_type = 'agent.process.terminate' \
                                    AND t.status IN ('issued', 'delivered'))) \
         RETURNING c.id, c.command_type, c.interaction_id, c.payload, \
                   c.issued_at, c.claim_count",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(worker_id)
    .bind(command_claim_lease_secs())
    .bind(run_generation.unwrap_or(0))
    .fetch_all(pool)
    .await?;

    // UPDATE..RETURNING 不支持 ORDER BY：领取顺序在内存里排（terminate 先于
    // continue 仅作展示约定，执行顺序由执行端按类型自行保证）。
    let mut polled: Vec<PolledCommand> = rows
        .into_iter()
        .map(|row| PolledCommand {
            command_id: row.get("id"),
            command_type: row.get("command_type"),
            interaction_id: row.get("interaction_id"),
            payload: row.get("payload"),
            issued_at: row.get("issued_at"),
            attempt: row.get("claim_count"),
        })
        .collect();
    polled.sort_by_key(|c| c.issued_at);
    Ok(polled)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckOutcome {
    Acked,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CommandAckResult {
    pub command_id: Uuid,
    pub command_type: String,
    pub status: String,
    /// 幂等重放：命令已处于终态（acked/failed）时为 true。
    pub replayed: bool,
}

/// 命令回执（V1.1 §3.2/§3.5）。
///
/// acked 从严判据（R-ACP-5/P1-1）：terminate 必须携带 `exit_outcome`；
/// continue 必须同时携带 `session_created` 与 `input_injected`（注入失败时
/// 不得因为 Session 已创建就把人工回复当成交付成功，ACP-441）。
///
/// 副作用：continue acked → 调度保护 start_wait → none（屏蔽释放点，§4.1）；
/// terminate failed → `continuation_start_frozen_at` 落值 + Task → blocked
/// （§6.5 / R-ACP-7：只冻结启动，不冻结回复提交）。
pub async fn ack_command(
    pool: &PgPool,
    project_id: Uuid,
    command_id: Uuid,
    worker_id: &str,
    outcome: AckOutcome,
    receipt: Option<Value>,
    last_error: Option<String>,
) -> Result<CommandAckResult, CommandFaceError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "SELECT id, command_type, status, task_id, run_id, interaction_id, payload \
           FROM delivery.worker_command \
          WHERE id = $1 AND project_id = $2 AND worker_id = $3 \
            FOR UPDATE",
    )
    .bind(command_id)
    .bind(project_id)
    .bind(worker_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(CommandFaceError::NotFound)?;
    let command_type: String = row.get("command_type");
    let status: String = row.get("status");
    let task_id: Uuid = row.get("task_id");
    let run_id: Uuid = row.get("run_id");
    let interaction_id: Option<Uuid> = row.get("interaction_id");

    // 幂等：终态命令重复回执原样返回（ACP-437/440 的重投路径）。
    if matches!(status.as_str(), "acked" | "failed") {
        tx.commit().await?;
        return Ok(CommandAckResult {
            command_id,
            command_type,
            status,
            replayed: true,
        });
    }
    if status != "delivered" {
        return Err(CommandFaceError::Validation(
            "command is not in delivered state (claim it first)".into(),
        ));
    }

    let receipt = receipt.unwrap_or(Value::Null);
    match (outcome, command_type.as_str()) {
        (AckOutcome::Acked, COMMAND_TERMINATE) => {
            if receipt.get("exit_outcome").and_then(|v| v.as_str()).is_none() {
                return Err(CommandFaceError::Validation(
                    "terminate ack requires receipt.exit_outcome (ExitOutcome five-value semantics)"
                        .into(),
                ));
            }
        }
        (AckOutcome::Acked, COMMAND_CONTINUE) => {
            let session_created = receipt
                .get("session_created")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let input_injected = receipt
                .get("input_injected")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !(session_created && input_injected) {
                return Err(CommandFaceError::Validation(
                    "continue ack requires receipt.session_created AND receipt.input_injected \
                     (injection failure must not be reported as delivered)"
                        .into(),
                ));
            }
        }
        _ => {}
    }

    match outcome {
        AckOutcome::Acked => {
            sqlx::query(
                "UPDATE delivery.worker_command \
                    SET status = 'acked', acked_at = now(), \
                        payload = payload || jsonb_build_object('receipt', $2::jsonb) \
                  WHERE id = $1",
            )
            .bind(command_id)
            .bind(&receipt)
            .execute(&mut *tx)
            .await?;
            if command_type == COMMAND_CONTINUE {
                // 屏蔽释放点：INPUT_INJECTED 已确认（V1.1 §4.1）。
                sqlx::query(
                    "UPDATE delivery.task_run SET scheduling_shield = 'none' \
                      WHERE id = $1 AND scheduling_shield = 'start_wait'",
                )
                .bind(run_id)
                .execute(&mut *tx)
                .await?;
                // R-4（V1.2 §5.3 / ACP-450/451）：INPUT_INJECTED 是执行恢复的
                // 业务确认边界——同事务链关闭 + task_waiting resolve + Task
                // awaiting_decision→running；consumed 降为 Interaction 级观测
                // 事实（ACP-C2）。迟到/重复 ack 在上方终态幂等分支已拦截。
                if let Some(interaction_id) = interaction_id {
                    crate::interaction::resume_execution_after_continue_tx(
                        &mut tx,
                        project_id,
                        task_id,
                        interaction_id,
                        &AuditActor::dispatch(),
                    )
                    .await
                    .map_err(|e| CommandFaceError::Validation(e.to_string()))?;
                }
            }
            if command_type == COMMAND_TERMINATE {
                // F-10（审计 2026-11-07）：终止回执落进程清理账本（§12.6 优先
                // 扩展既有账本；Runner 载体 spawn 时无账本行，此处直接以
                // completed 终态入账——终止生命周期已闭环，无需 cleanup 重试）。
                sqlx::query(
                    "INSERT INTO delivery.task_run_cleanup \
                       (run_id, session, executor_kind, status, completed_at) \
                     SELECT r.id, COALESCE(r.executor_session, 'unknown'), \
                            COALESCE(r.executor_kind, 'runner'), 'completed', now() \
                       FROM delivery.task_run r WHERE r.id = $1 \
                     ON CONFLICT (run_id) DO UPDATE \
                        SET status = 'completed', completed_at = now(), updated_at = now()",
                )
                .bind(run_id)
                .execute(&mut *tx)
                .await?;
            }
        }
        AckOutcome::Failed => {
            sqlx::query(
                "UPDATE delivery.worker_command \
                    SET status = 'failed', failed_at = now(), last_error = $2, \
                        payload = payload || jsonb_build_object('receipt', $3::jsonb) \
                  WHERE id = $1",
            )
            .bind(command_id)
            .bind(last_error.clone())
            .bind(&receipt)
            .execute(&mut *tx)
            .await?;
            if command_type == COMMAND_TERMINATE {
                // §11.4：重试耗尽 → 冻结新 Session 启动（不冻结回复提交），
                // Interaction 保持 PENDING，Task 进合法阻塞路径（尽力而为）。
                if let Some(interaction_id) = interaction_id {
                    sqlx::query(
                        "UPDATE delivery.task_interaction \
                            SET continuation_start_frozen_at = now() \
                          WHERE id = $1 AND continuation_start_frozen_at IS NULL",
                    )
                    .bind(interaction_id)
                    .execute(&mut *tx)
                    .await?;
                }
                // F-10：终止失败（旧进程退出未确认）→ 账本 needs_human
                //（清理未闭环转人工；completed_at 保持 NULL 满足 CHECK）。
                sqlx::query(
                    "INSERT INTO delivery.task_run_cleanup \
                       (run_id, session, executor_kind, status, last_error) \
                     SELECT r.id, COALESCE(r.executor_session, 'unknown'), \
                            COALESCE(r.executor_kind, 'runner'), 'needs_human', $2 \
                       FROM delivery.task_run r WHERE r.id = $1 \
                     ON CONFLICT (run_id) DO UPDATE \
                        SET status = 'needs_human', completed_at = NULL, \
                            updated_at = now(), last_error = $2",
                )
                .bind(run_id)
                .bind(last_error.clone())
                .execute(&mut *tx)
                .await?;
                let task_status: Option<String> = sqlx::query_scalar(
                    "SELECT status FROM delivery.task WHERE id = $1 AND project_id = $2 FOR UPDATE",
                )
                .bind(task_id)
                .bind(project_id)
                .fetch_optional(&mut *tx)
                .await?;
                match task_status.as_deref() {
                    // 阻塞处理路径（§11.4「现有合法的阻塞处理路径」）：
                    // Task 在 Running → 合法边转 Blocked；
                    // Task 在 awaiting_decision（人工等待本就是 Task 级等待态，
                    // 且 AwaitingDecision→Blocked 非合法边）→ 保持不动——
                    // 启动冻结（continuation_start_frozen_at）+ 命令 failed
                    // + 告警已承载失败语义，不得为表达阻塞而绕开 13 态。
                    Some("running") => {
                        let ctx = system_context(&mut tx, project_id)
                            .await
                            .map_err(|e| CommandFaceError::Validation(e.to_string()))?;
                        apply_task_transition(
                            &mut tx,
                            task_id,
                            None,
                            TaskStatus::Blocked,
                            &TransitionFacts::for_dispatch(),
                            &ctx,
                        )
                        .await
                        .map_err(|e| CommandFaceError::Validation(e.to_string()))?;
                    }
                    _ => {}
                }
            }
        }
    }

    append_audit_with_after(
        &mut tx,
        project_id,
        &AuditActor::dispatch(),
        match outcome {
            AckOutcome::Acked => "command.acked",
            AckOutcome::Failed => "command.failed",
        },
        "worker_command",
        Some(command_id),
        json!({
            "command_type": command_type,
            "outcome": match outcome { AckOutcome::Acked => "acked", AckOutcome::Failed => "failed" },
            "receipt": receipt,
            "last_error": last_error,
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(CommandAckResult {
        command_id,
        command_type,
        status: match outcome {
            AckOutcome::Acked => "acked".into(),
            AckOutcome::Failed => "failed".into(),
        },
        replayed: false,
    })
}

/// 情形 6 自查（V1.3.1 §9.4 检查 8 操作化）：Run 存在未收到终止回执的
/// terminate 命令（issued/delivered）→ 禁止启动新 Session。continue 的
/// 执行端启动前检查与 Platform 侧防驭均消费本判定。
pub async fn has_unconfirmed_termination(
    pool: &PgPool,
    run_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM delivery.worker_command \
          WHERE run_id = $1 AND command_type = 'agent.process.terminate' \
            AND status IN ('issued', 'delivered'))",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}
