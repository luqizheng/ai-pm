//! 持久化进程清理账本（S01 · I1-E，设计稿 §9.2/§9.3，迁移 `0035`）。
//!
//! 原则（§9.1）：**Run 终态与进程清理是两个独立生命周期**——Run 取消成功不等于
//! 进程已退出；进程退出失败不允许恢复 Run 的执行凭据。
//!
//! 旧路径 `cleanup_finished_processes` 的「1 小时窗口 + LIMIT 64 + 尽力而为」
//! 不能作为完整清理保障（§9.4）：超过窗口的残留进程会永远丢失。本模块把清理
//! 状态落库 `delivery.task_run_cleanup`：
//!
//! ```text
//! pending → cleaning → completed
//!              ↓（失败：attempts+1、退避重试）
//!           needs_human（达到 max_attempts：持续可查询、可告警，I1-040）
//! ```
//!
//! PID 复用防护（§9.3）：进程身份 = session + pid 文件中的 `identity_hint` 锚点
//! （执行器侧已实现比对），本账本只持久化锚点引用事实，不依赖裸 PID。

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// 清理账本状态。
pub const STATUS_PENDING: &str = "pending";
pub const STATUS_CLEANING: &str = "cleaning";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_NEEDS_HUMAN: &str = "needs_human";

/// 单轮领取的行数上限（§9.3：分批处理；剩余行由下一轮继续，I1-032）。
pub const CLEANUP_BATCH_LIMIT: i64 = 64;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct CleanupEntry {
    pub run_id: Uuid,
    pub session: String,
    pub executor_kind: Option<String>,
    pub identity_hint: Option<String>,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub next_retry_at: DateTime<Utc>,
    pub last_error: Option<String>,
    /// Internal fencing token; never expose it in serialized cleanup summaries.
    #[serde(skip_serializing)]
    pub claim_token: Option<Uuid>,
}

/// 登记一条待清理记录（撤凭据时调用；幂等——同 Run 已在账本则不覆盖）。
pub async fn enqueue(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_id: Uuid,
    session: &str,
    executor_kind: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO delivery.task_run_cleanup (run_id, session, executor_kind) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (run_id) DO NOTHING",
    )
    .bind(run_id)
    .bind(session)
    .bind(executor_kind)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 领取一批到期待清理记录（`FOR UPDATE SKIP LOCKED`：多清理 Worker 安全）。
const CLEANUP_CLAIM_LEASE_SECS: f64 = 300.0;

/// Atomically claim due cleanup rows and fence each claim with a fresh token.
/// `next_retry_at` is the claim lease expiry while status is `cleaning`; a
/// crashed worker becomes reclaimable only after that deadline.
pub async fn claim_batch(pool: &PgPool) -> Result<Vec<CleanupEntry>, sqlx::Error> {
    sqlx::query_as::<_, CleanupEntry>(
        "WITH candidates AS (
             SELECT run_id
               FROM delivery.task_run_cleanup
              WHERE status IN ('pending', 'cleaning')
                AND next_retry_at <= now()
              ORDER BY next_retry_at, created_at
              LIMIT $1
              FOR UPDATE SKIP LOCKED
         )
         UPDATE delivery.task_run_cleanup AS cleanup
            SET status = 'cleaning',
                claim_token = gen_random_uuid(),
                next_retry_at = now() + make_interval(secs => $2::float8),
                updated_at = now()
           FROM candidates
          WHERE cleanup.run_id = candidates.run_id
         RETURNING cleanup.run_id, cleanup.session, cleanup.executor_kind,
                   cleanup.identity_hint, cleanup.status, cleanup.attempts,
                   cleanup.max_attempts, cleanup.next_retry_at,
                   cleanup.last_error, cleanup.claim_token",
    )
    .bind(CLEANUP_BATCH_LIMIT)
    .bind(CLEANUP_CLAIM_LEASE_SECS)
    .fetch_all(pool)
    .await
}

pub async fn mark_completed(
    pool: &PgPool,
    run_id: Uuid,
    claim_token: Uuid,
) -> Result<bool, sqlx::Error> {
    let rows = sqlx::query(
        "UPDATE delivery.task_run_cleanup
            SET status = 'completed', completed_at = now(), claim_token = NULL, updated_at = now()
          WHERE run_id = $1 AND status = 'cleaning' AND claim_token = $2",
    )
    .bind(run_id)
    .bind(claim_token)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(rows == 1)
}

pub async fn mark_failed(
    pool: &PgPool,
    run_id: Uuid,
    claim_token: Uuid,
    error: &str,
    backoff_secs: i64,
) -> Result<bool, sqlx::Error> {
    let rows = sqlx::query(
        "UPDATE delivery.task_run_cleanup
            SET attempts = attempts + 1,
                status = CASE WHEN attempts + 1 >= max_attempts THEN 'needs_human' ELSE 'pending' END,
                next_retry_at = now() + make_interval(secs => $3::float8),
                last_error = $4,
                claim_token = NULL,
                updated_at = now()
          WHERE run_id = $1 AND status = 'cleaning' AND claim_token = $2",
    )
    .bind(run_id)
    .bind(claim_token)
    .bind(backoff_secs as f64)
    .bind(truncate_error(error))
    .execute(pool)
    .await?
    .rows_affected();
    Ok(rows == 1)
}

pub async fn requeue(pool: &PgPool, run_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE delivery.task_run_cleanup \
            SET status = 'pending', attempts = 0, next_retry_at = now(), \
                last_error = NULL, claim_token = NULL, completed_at = NULL, updated_at = now() \
          WHERE run_id = $1",
    )
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn truncate_error(error: &str) -> String {
    let trimmed = error.trim();
    if trimmed.chars().count() <= 480 {
        return trimmed.to_string();
    }
    let mut out: String = trimmed.chars().take(480).collect();
    out.push('…');
    out
}

/// 清理 Worker 单轮（§9.3）：领一批 → 逐条 terminate → 成功置 completed /
/// 失败退避。返回 (成功数, 失败数)。
pub async fn run_cleanup_round(
    pool: &PgPool,
    executor: &dyn crate::run_dispatch::RunExecutor,
) -> Result<(usize, usize), sqlx::Error> {
    let batch = claim_batch(pool).await?;
    let mut ok = 0usize;
    let mut failed = 0usize;
    for entry in batch {
        let session = crate::run_dispatch::ExecutorSession {
            session: entry.session.clone(),
            run_id: entry.run_id,
            workspace_root: crate::run_dispatch::workspace_root_from_env(),
        };
        let claim_token = entry.claim_token.ok_or_else(|| {
            sqlx::Error::Protocol("claimed cleanup row is missing its fencing token".into())
        })?;
        match executor
            .terminate(&session, crate::run_dispatch::TerminateReason::Cancelled)
            .await
        {
            Ok(()) => {
                if mark_completed(pool, entry.run_id, claim_token).await? {
                    ok += 1;
                } else {
                    tracing::warn!(run = %entry.run_id, "stale cleanup claim could not mark completion");
                    failed += 1;
                }
            }
            Err(error) => {
                tracing::warn!(
                    run = %entry.run_id,
                    attempt = entry.attempts,
                    error = %error,
                    "cleanup attempt failed; backoff retry scheduled"
                );
                if !mark_failed(pool, entry.run_id, claim_token, &error.to_string(), 60).await? {
                    tracing::warn!(run = %entry.run_id, "stale cleanup claim could not record failure");
                }
                failed += 1;
            }
        }
    }
    Ok((ok, failed))
}
