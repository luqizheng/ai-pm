//! Recovery Foundation（联合实施基线批 3）：发现面 + 同主体续领（R-16 方案 a）。
//!
//! 设计：ACP V1.2 §3.6（`pm_discover_runs` / `pm_reacquire_lease`；R-ACP-16
//! 已裁）+ 联合裁决约束 A/B/C + ACP-C3/C4。
//!
//! **闭合的残留（复审 P0-2 残留半边）**：执行端离线超租约 TTL（PC 休眠/崩溃/
//! 断网；Model A 关闭客户端）后——一次性 token 丢失、旧租约两通道皆不可续、
//! B3-2 被 S2 拦截 → continue 永无人消费。本模块给唯一合法恢复路径：
//!
//! ① **发现面**（[`discover_runs`]）：按稳定 Client 身份（`lease_client_id`
//!    严格相等，V-05-1 后；`IS NULL` = legacy → 一律不发现）列出名下
//!    `start_wait` 且有可投递 continue 命令的 Run 摘要。只读、分页、
//!    无状态副作用；**不返回人工回复正文与 Command payload**。
//! ② **续领**（[`reacquire_lease`]）：同事务五前置（shield=start_wait /
//!    同 Client 身份 / Deadline 有效 / 存在 responded Interaction /
//!    **lease_client_id 非空**——legacy 门）+ 四效果（generation+1 新 token、
//!    非终局命令同事务改路由 + `reacquired_generation` 写回、审计）。
//!
//! **ACP-C4**：terminate ACKED **不是**续领前置，而是新 Session 启动前置
//! （决策表情形 6 / poll 内置过滤不因续领改变）。**T3**：deadline 不动——
//! 离线恢复不构成 Deadline 顺延，过期走 ACP-07。**续领本身不自动创建
//! Session**——Session 仍由 continue 命令显式触发。

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::append_audit_with_after;
use crate::run_context::generate_lease_token;
use crate::run_dispatch::RunDispatchError;

#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("database error: {0}")]
    Db(String),
    #[error("task or run not found in this project")]
    NotFound,
    /// 前置不满足（shield/身份/Deadline/无 responded Interaction/legacy NULL）。
    #[error("reacquire rejected: {0}")]
    Rejected(String),
    #[error("random source unavailable")]
    Random,
}

impl From<RunDispatchError> for RecoveryError {
    fn from(e: RunDispatchError) -> Self {
        match e {
            RunDispatchError::Random => RecoveryError::Random,
            other => RecoveryError::Db(other.to_string()),
        }
    }
}

/// 发现面返回的 Run 摘要（专家十字段 + `task_id`——恢复上下文必需：continue
/// spawn 的 env/心跳/提交都挂 task；**不含人工回复正文与 Command payload**）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoveredRun {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub execution_mode: String,
    pub interaction_id: Option<Uuid>,
    pub interaction_status: Option<String>,
    pub deadline_at: Option<chrono::DateTime<chrono::Utc>>,
    pub shield_state: String,
    pub lease_generation: i32,
    /// `live` | `expired`（执行端存活口径；恢复路径的判据输入）。
    pub lease_status: String,
    pub terminate_command_id: Option<Uuid>,
    pub terminate_status: Option<String>,
}

/// 发现面（约束 C：只发现当前认证 Client 有权恢复的 Run——`lease_client_id`
/// 严格相等；`IS NULL` legacy 一律不发现）。只读、分页、重复调用无副作用。
pub async fn discover_runs(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    client_device_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<DiscoveredRun>, RecoveryError> {
    let limit = limit.clamp(1, 100);
    let rows = sqlx::query(
        "SELECT r.id, r.task_id, r.execution_mode, r.scheduling_shield, r.lease_generation, \
                r.lease_expires_at, r.deadline_at, \
                (SELECT i.id FROM delivery.task_interaction i \
                  WHERE i.run_id = r.id AND i.status = 'responded' \
                  ORDER BY i.created_at DESC LIMIT 1) AS interaction_id, \
                (SELECT i.status FROM delivery.task_interaction i \
                  WHERE i.run_id = r.id AND i.status = 'responded' \
                  ORDER BY i.created_at DESC LIMIT 1) AS interaction_status, \
                (SELECT t.id FROM delivery.worker_command t \
                  WHERE t.run_id = r.id AND t.command_type = 'agent.process.terminate' \
                    AND t.status IN ('issued', 'delivered') \
                  ORDER BY t.issued_at DESC LIMIT 1) AS terminate_command_id, \
                (SELECT t.status FROM delivery.worker_command t \
                  WHERE t.run_id = r.id AND t.command_type = 'agent.process.terminate' \
                    AND t.status IN ('issued', 'delivered') \
                  ORDER BY t.issued_at DESC LIMIT 1) AS terminate_status \
           FROM delivery.task_run r \
          WHERE r.tenant_id = $1 AND r.project_id = $2 \
            AND r.lease_client_id = $3 \
            AND r.scheduling_shield = 'start_wait' \
            AND EXISTS (SELECT 1 FROM delivery.worker_command c \
                         WHERE c.run_id = r.id \
                           AND c.command_type = 'agent.task.continue' \
                           AND c.status IN ('issued', 'delivered')) \
          ORDER BY r.deadline_at ASC NULLS LAST, r.id \
          LIMIT $4 OFFSET $5",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(client_device_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| RecoveryError::Db(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|r| DiscoveredRun {
            lease_status: if r
                .get::<Option<chrono::DateTime<chrono::Utc>>, _>("lease_expires_at")
                .map(|e| e > chrono::Utc::now())
                .unwrap_or(false)
            {
                "live".into()
            } else {
                "expired".into()
            },
            run_id: r.get("id"),
            task_id: r.get("task_id"),
            execution_mode: r.get::<Option<String>, _>("execution_mode")
                .unwrap_or_else(|| "unknown".into()),
            interaction_id: r.get("interaction_id"),
            interaction_status: r.get("interaction_status"),
            deadline_at: r.get("deadline_at"),
            shield_state: r.get("scheduling_shield"),
            lease_generation: r.get("lease_generation"),
            terminate_command_id: r.get("terminate_command_id"),
            terminate_status: r.get("terminate_status"),
        })
        .collect())
}

/// 续领结果（执行端据此重建执行面凭据 + 命令路由身份）。
#[derive(Debug, Clone)]
pub struct ReacquiredLease {
    pub lease_token: String,
    pub lease_generation: i32,
    pub worker_id: String,
    pub deadline_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 续领事务内改路由的命令数（审计/可观测）。
    pub rerouted_commands: u64,
}

/// 同主体续领（R-16 方案 a；§3.6 五前置 + 四效果，单事务）。
///
/// 前置：shield='start_wait' ∧ lease_client_id = device（**NULL = legacy 拒绝**）
/// ∧ deadline_at > now() ∧ 存在 responded Interaction。
/// 效果：同 Run 重签租约（generation+1、新 token、lease_expires_at 刷新、
/// **deadline_at 不动**=T3）；非终局命令同事务改路由 + `reacquired_generation`
/// 写回（改路由 ≠ 重建命令：command_id/payload 不变、claim_count 不重置）；
/// 审计 `run.lease_reacquired`。
///
/// **不创建新 Session、不做 terminate 确认**（ACP-C4：terminate ACKED 是
/// 新 Session 启动前置，由既有情形 6 过滤与 §9.4 检查 8 承担）。
#[allow(clippy::too_many_arguments)]
pub async fn reacquire_lease(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    run_id: Uuid,
    client_device_id: Uuid,
    lease_secs: i64,
    actor: &AuditActor,
) -> Result<ReacquiredLease, RecoveryError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| RecoveryError::Db(e.to_string()))?;

    // ① Run 行锁 + 五前置。
    let run = sqlx::query(
        "SELECT scheduling_shield, lease_client_id, deadline_at, lease_generation, \
                lease_owner \
           FROM delivery.task_run WHERE id = $1 AND project_id = $2 AND tenant_id = $3 \
            FOR UPDATE",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| RecoveryError::Db(e.to_string()))?
    .ok_or(RecoveryError::NotFound)?;
    let shield: String = run.get("scheduling_shield");
    let lease_client: Option<Uuid> = run.get("lease_client_id");
    let deadline_at: Option<chrono::DateTime<chrono::Utc>> = run.get("deadline_at");
    let current_generation: i32 = run.get("lease_generation");
    let owner: Option<String> = run.get("lease_owner");

    if shield != "start_wait" {
        return Err(RecoveryError::Rejected(format!(
            "scheduling_shield must be start_wait (got {shield})"
        )));
    }
    // legacy 门（约束 A/B 延伸）：lease_client_id NULL = 历史自报身份，不认定。
    match lease_client {
        Some(d) if d == client_device_id => {}
        Some(_) => {
            return Err(RecoveryError::Rejected(
                "caller is not the owning client of this run".into(),
            ))
        }
        None => {
            return Err(RecoveryError::Rejected(
                "run has no stable client identity (legacy); manual disposition required".into(),
            ))
        }
    }
    let Some(deadline) = deadline_at else {
        return Err(RecoveryError::Rejected("run has no deadline".into()));
    };
    if deadline <= chrono::Utc::now() {
        // T3：离线恢复不是顺延理由——过期转 ACP-07 人工授权。
        return Err(RecoveryError::Rejected(
            "deadline has passed; requires ACP-07 authorization".into(),
        ));
    }
    let has_responded: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM delivery.task_interaction \
          WHERE run_id = $1 AND status = 'responded')",
    )
    .bind(run_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| RecoveryError::Db(e.to_string()))?;
    if !has_responded {
        return Err(RecoveryError::Rejected(
            "no persisted human reply on this run".into(),
        ));
    }

    // ② 同 Run 重签租约（generation+1；T2/T3 字面一致；不是 B3-2 新建 Run）。
    let lease_token = generate_lease_token().map_err(|_| RecoveryError::Random)?;
    let lease_token_hash = crate::run_context::hash_lease_token(&lease_token);
    let new_generation = current_generation + 1;
    let new_owner = format!("client:{client_device_id}");
    sqlx::query(
        "UPDATE delivery.task_run \
            SET lease_generation = $2, lease_token_hash = $3, \
                lease_expires_at = now() + make_interval(secs => $4::float8), \
                lease_owner = $5, heartbeat_at = now() \
          WHERE id = $1",
    )
    .bind(run_id)
    .bind(new_generation)
    .bind(&lease_token_hash)
    .bind(lease_secs as f64)
    .bind(&new_owner)
    .execute(&mut *tx)
    .await
    .map_err(|e| RecoveryError::Db(e.to_string()))?;

    // ③ 非终局命令同事务改路由 + reacquired_generation 写回
    //   （改路由 ≠ 重建：command_id/idempotency_key/payload 不变、claim_count 不重置）。
    let rerouted = sqlx::query(
        "UPDATE delivery.worker_command \
            SET worker_id = $2, reacquired_generation = $3 \
          WHERE run_id = $1 AND status IN ('issued', 'delivered')",
    )
    .bind(run_id)
    .bind(&new_owner)
    .bind(new_generation)
    .execute(&mut *tx)
    .await
    .map_err(|e| RecoveryError::Db(e.to_string()))?
    .rows_affected();

    // ④ 审计（含原/新 generation 与触发身份）。
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "run.lease_reacquired",
        "task_run",
        Some(run_id),
        serde_json::json!({
            "device_id": client_device_id,
            "from_generation": current_generation,
            "to_generation": new_generation,
            "from_owner": owner,
            "to_owner": new_owner,
            "rerouted_commands": rerouted,
        }),
    )
    .await
    .map_err(|e| RecoveryError::Db(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| RecoveryError::Db(e.to_string()))?;
    Ok(ReacquiredLease {
        lease_token,
        lease_generation: new_generation,
        worker_id: new_owner,
        deadline_at,
        rerouted_commands: rerouted,
    })
}
