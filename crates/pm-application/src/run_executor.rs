//! Slice D3：执行器编排（`docs/ai-pm-slice-d3-executor-adapter-v1.md` §3）。
//!
//! 职责边界：本模块只做「领取 → 启动 → 监控 → 终止 → 收尾」的**编排与落库**，
//! 不实现进程管理（真实执行器在 `pm-infrastructure`，测试用 `FakeExecutor`）。
//! 三条硬规则沿用 D1/D2：
//! 1. Run 状态回写一律条件 UPDATE（`WHERE status IN ('claimed','running')`）
//!    先到先得，与执行端 `submit_result` / 失联回收竞态时绝不覆盖；
//! 2. 任务状态不经本模块改写——`claimed→running` 由执行端首报 `started` 经
//!    D2 `report_progress` 推进；`failed`/`timed_out` 的任务级裁决归 D5；
//! 3. 收尾与审计同事务，actor 走 `audit_run` 的既有回退链。

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::command_executor::{apply_task_transition, system_context, TransitionFacts};
use crate::run_dispatch::{
    audit_run, claim_once, recover_lost_run, renew_lease, ClaimKind, DispatchMode,
    RunDispatchConfig, RunDispatchError, ExecutableRun, ExecutorProgress, ExecutorSession,
    RunExecutor, RunRepositoryBaseline, TerminateReason, workspace_root_from_env,
};
use pm_domain::TaskStatus;

/// 一次「领取并启动」的结果。
#[derive(Debug)]
pub enum StartOutcome {
    /// 没有可领取的候选。
    Idle,
    /// 已启动执行器（`executor_session` 已落库）。
    Started { run_id: Uuid, session: String },
    /// 启动失败：Run 已按基础设施失败退避（或重试耗尽落 `failed`）。
    StartFailed { run_id: Uuid, error: String },
    /// 领取路径的其他结果（probe 释放 / 死 Run 取消），无需启动。
    Skipped { run_id: Uuid },
}

/// 领取一条 Run 并启动执行器（worker 主循环的领取半边）。
///
/// `Claimed` / `Reclaimed` 都会尝试 `start`；`start` 失败按**基础设施失败**处理：
/// Run 退回 `queued` 指数退避 + `executor_start_failed` failure 行；重试达
/// `max_infrastructure_attempts` 后 Run 落 `failed` 不再重试（B3-4 独立计数）。
pub async fn claim_and_start(
    pool: &PgPool,
    executor: &dyn RunExecutor,
    worker_id: &str,
    cfg: &RunDispatchConfig,
) -> Result<StartOutcome, RunDispatchError> {
    let outcome = claim_once(pool, worker_id, cfg, DispatchMode::On).await?;
    let Some(outcome) = outcome else {
        return Ok(StartOutcome::Idle);
    };
    match outcome.kind {
        ClaimKind::Claimed | ClaimKind::Reclaimed => {}
        ClaimKind::Skipped | ClaimKind::Cancelled => {
            return Ok(StartOutcome::Skipped {
                run_id: outcome.run_id,
            });
        }
    }
    let (run_id, task_id) = (outcome.run_id, outcome.task_id);
    let project_id = project_of(pool, run_id).await?;

    let executable = executable_run(pool, run_id, task_id, project_id).await?;
    match executor.start(&executable).await {
        Ok(session) => {
            let mut tx = pool.begin().await?;
            // 先写执行会话（条件 UPDATE 防竞态），再推进状态（D1 §九问题 4：
            // claimed→running 由 D3 在执行端启动成功后经平台写入，不由执行端直写）。
            let updated = sqlx::query(
                "UPDATE delivery.task_run \
                    SET executor_kind = $2, executor_session = $3 \
                  WHERE id = $1 AND status IN ('claimed', 'running')",
            )
            .bind(run_id)
            .bind(executor.kind())
            .bind(&session.session)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if updated == 1 {
                // Run 侧：running + stage=started（与 D2 首报 started 的落点一致）；
                // 任务侧：经唯一受守卫入口 apply_task_transition 推进。
                sqlx::query(
                    "UPDATE delivery.task_run \
                        SET status = 'running', stage = 'started', started_at = now() \
                      WHERE id = $1 AND status = 'claimed'",
                )
                .bind(run_id)
                .execute(&mut *tx)
                .await?;
                let ctx = system_context(&mut tx, project_id)
                    .await
                    .map_err(|e| RunDispatchError::Transition(e.to_string()))?;
                apply_task_transition(
                    &mut tx,
                    task_id,
                    None,
                    TaskStatus::Running,
                    &TransitionFacts::for_dispatch(),
                    &ctx,
                )
                .await
                .map_err(|e| RunDispatchError::Transition(e.to_string()))?;
                audit_run(
                    &mut tx,
                    project_id,
                    "run_started",
                    run_id,
                    serde_json::json!({
                        "executor_kind": executor.kind(),
                        "executor_session": session.session,
                        "workspace_root": session.workspace_root,
                    }),
                )
                .await?;
            }
            tx.commit().await?;
            Ok(StartOutcome::Started {
                run_id,
                session: session.session,
            })
        }
        Err(error) => {
            let detail = error.to_string();
            start_failed(pool, run_id, project_id, cfg, &detail).await?;
            Ok(StartOutcome::StartFailed {
                run_id,
                error: detail,
            })
        }
    }
}

/// 监控一轮本 worker 名下的已启动 Run：超时强杀 / 兜底续租 / 收尾回写。
/// 返回本轮处理的 Run 数。
pub async fn poll_active_runs(
    pool: &PgPool,
    executor: &dyn RunExecutor,
    worker_id: &str,
    cfg: &RunDispatchConfig,
) -> Result<usize, RunDispatchError> {
    let rows = sqlx::query(
        "SELECT r.id, r.task_id, r.project_id, r.status, r.executor_session, \
                r.lease_expires_at, r.timeout_seconds, \
                coalesce(r.started_at, r.claimed_at) AS began_at \
           FROM delivery.task_run r \
          WHERE r.status IN ('claimed', 'running') \
            AND r.executor_session IS NOT NULL \
            AND r.lease_owner = $1 \
          ORDER BY r.id",
    )
    .bind(worker_id)
    .fetch_all(pool)
    .await?;

    let mut handled = 0;
    for row in rows {
        let run_id: Uuid = row.get("id");
        let task_id: Uuid = row.get("task_id");
        let project_id: Uuid = row.get("project_id");
        let session_text: String = row.get("executor_session");
        let timeout_seconds: Option<i32> = row.get("timeout_seconds");
        let lease_expires_at: Option<chrono::DateTime<chrono::Utc>> = row.get("lease_expires_at");
        let began_at: Option<chrono::DateTime<chrono::Utc>> = row.get("began_at");
        let session = ExecutorSession {
            session: session_text,
            run_id,
            workspace_root: workspace_root_from_env(),
        };

        // ① 超时判定：started（或领取）时刻 + timeout_seconds 耗尽 → 统一终止管线。
        if timed_out(began_at, timeout_seconds) {
            let _ = executor.terminate(&session, TerminateReason::TimedOut).await;
            record_termination(pool, run_id, project_id, "timeout").await?;
            handled += 1;
            continue;
        }

        // ② 存活探测 + 兜底续租（Q-D3-4：防执行端长静默被误杀）。
        let progress = match executor.poll(&session).await {
            Ok(progress) => progress,
            Err(error) => {
                tracing::warn!(run = %run_id, error = %error, "executor poll failed; retry next round");
                continue;
            }
        };
        match progress {
            ExecutorProgress::Pending | ExecutorProgress::Running => {
                let expiring_soon = lease_expires_at
                    .map(|at| at < chrono::Utc::now() + chrono::Duration::seconds(cfg.lease_secs / 2))
                    .unwrap_or(true);
                if expiring_soon {
                    let renewed = renew_lease(pool, run_id, worker_id, cfg.lease_secs).await?;
                    if !renewed {
                        // 租约续不上（已过期/已被回收路径接管）：失联处理——
                        // 杀进程 + B3-2 隔离旧 Run 并新建重试 Run（Q-D3-4 语义）。
                        tracing::info!(run = %run_id, "lease unrenewable; reclaiming as lost");
                        let _ = executor
                            .terminate(&session, TerminateReason::LeaseReclaimed)
                            .await;
                        recover_lost(pool, run_id, task_id, project_id, cfg).await?;
                        handled += 1;
                    }
                }
            }
            ExecutorProgress::Done { exit_code } => {
                finish_run(pool, run_id, project_id, exit_code).await?;
                handled += 1;
            }
        }
    }
    Ok(handled)
}

/// 用户取消（Q-D3-5：与超时同一条终止管线，终态不同）。
/// Run 不在执行中 / 无执行会话时返回 `false`。
pub async fn cancel_run(
    pool: &PgPool,
    executor: &dyn RunExecutor,
    run_id: Uuid,
) -> Result<bool, RunDispatchError> {
    let row = sqlx::query(
        "SELECT project_id, status, executor_session FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let project_id: Uuid = row.get("project_id");
    let status: String = row.get("status");
    let session_text: Option<String> = row.get("executor_session");
    if !matches!(status.as_str(), "claimed" | "running") {
        return Ok(false);
    }
    if let Some(session_text) = session_text {
        let session = ExecutorSession {
            session: session_text,
            run_id,
            workspace_root: workspace_root_from_env(),
        };
        // 进程可能已自行退出：terminate 内部先 try_wait，不强杀尸体。
        let _ = executor.terminate(&session, TerminateReason::Cancelled).await;
    }
    record_termination(pool, run_id, project_id, "user_cancel").await?;
    Ok(true)
}

/// 平台重启后的孤儿 reconcile（Q-D3-5 / 设计稿 §3 异常路径②）。
///
/// 孤儿 = `claimed/running` 且 `executor_session` 非空且租约**未过期**的 Run
/// （过期的由 `claim_once` 回收路径按 B3-2 处理，不在此重复）。对每个孤儿
/// `poll` 一次：
/// - `Done{Some(0)}` → 正常收尾 `succeeded`；
/// - `Done{Some(n)}` → 收尾 `failed` + `exit_code` failure 行；
/// - `Done{None}` → 进程被外部/信号终止且退出码不可归因 → 按失联处理
///   （B3-2：旧 Run `timed_out` + 新 Run 重试）；
/// - `Running`/`Pending` → 进程跨重启仍存活，保留：执行端心跳继续续租，
///   租约过期后由回收路径接管（本 worker 不再续，`lease_owner` 不匹配）。
pub async fn reconcile_orphans(
    pool: &PgPool,
    executor: &dyn RunExecutor,
    cfg: &RunDispatchConfig,
) -> Result<usize, RunDispatchError> {
    let rows = sqlx::query(
        "SELECT r.id, r.task_id, r.project_id, r.executor_session, r.lease_owner \
           FROM delivery.task_run r \
          WHERE r.status IN ('claimed', 'running') \
            AND r.executor_session IS NOT NULL \
            AND r.lease_owner IS NOT NULL \
            AND r.lease_expires_at > now()",
    )
    .fetch_all(pool)
    .await?;

    let mut handled = 0;
    for row in rows {
        let run_id: Uuid = row.get("id");
        let task_id: Uuid = row.get("task_id");
        let project_id: Uuid = row.get("project_id");
        let session_text: String = row.get("executor_session");
        let session = ExecutorSession {
            session: session_text,
            run_id,
            workspace_root: workspace_root_from_env(),
        };
        let progress = match executor.poll(&session).await {
            Ok(progress) => progress,
            Err(_) => continue,
        };
        match progress {
            ExecutorProgress::Done { exit_code: Some(code) } => {
                finish_run(pool, run_id, project_id, Some(code)).await?;
                handled += 1;
            }
            ExecutorProgress::Done { exit_code: None } => {
                recover_lost(pool, run_id, task_id, project_id, cfg).await?;
                handled += 1;
            }
            ExecutorProgress::Pending | ExecutorProgress::Running => {}
        }
    }
    Ok(handled)
}

// ------------------------------------------------------------------ internals

fn timed_out(
    began_at: Option<chrono::DateTime<chrono::Utc>>,
    timeout_seconds: Option<i32>,
) -> bool {
    let (Some(began_at), Some(timeout)) = (began_at, timeout_seconds) else {
        return false;
    };
    // 显式 0 = 立即超时（测试注入用）；未配置（None）不判定。
    chrono::Utc::now() - began_at >= chrono::Duration::seconds(timeout.max(0) as i64)
}

async fn project_of(pool: &PgPool, run_id: Uuid) -> Result<Uuid, RunDispatchError> {
    let project_id: Uuid = sqlx::query_scalar(
        "SELECT project_id FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await?;
    Ok(project_id)
}

async fn executable_run(
    pool: &PgPool,
    run_id: Uuid,
    task_id: Uuid,
    project_id: Uuid,
) -> Result<ExecutableRun, RunDispatchError> {
    let row = sqlx::query(
        "SELECT attempt_no, timeout_seconds FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await?;
    let attempt_no: i32 = row.get("attempt_no");
    let timeout_seconds: Option<i32> = row.get("timeout_seconds");
    let baseline_rows = sqlx::query(
        "SELECT tr.repository_id, tr.base_ref, \
                coalesce(tr.base_commit, pr.head_commit_sha) AS base_commit, \
                tr.write_access, tr.workspace_path \
           FROM delivery.task_run_repository tr \
           LEFT JOIN integration.project_repository pr ON pr.id = tr.repository_id \
          WHERE tr.run_id = $1 \
          ORDER BY tr.repository_id",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;
    let repositories = baseline_rows
        .iter()
        .map(|row| RunRepositoryBaseline {
            repository_id: row.get("repository_id"),
            base_ref: row.get("base_ref"),
            base_commit: row.get("base_commit"),
            write_access: row.get("write_access"),
            workspace_path: row.get("workspace_path"),
        })
        .collect();
    Ok(ExecutableRun {
        run_id,
        task_id,
        project_id,
        attempt_no,
        timeout_seconds,
        repositories,
    })
}

/// `start` 失败：退避 + 基础设施计数（B3-4），耗尽后落 `failed`。
async fn start_failed(
    pool: &PgPool,
    run_id: Uuid,
    project_id: Uuid,
    cfg: &RunDispatchConfig,
    detail: &str,
) -> Result<(), RunDispatchError> {
    let mut tx = pool.begin().await?;
    let row: Option<(String, i32)> = sqlx::query_as(
        "UPDATE delivery.task_run \
            SET status = 'queued', lease_owner = NULL, lease_expires_at = NULL, \
                lease_token_hash = NULL, dispatch_attempts = dispatch_attempts + 1, \
                infrastructure_retry_count = infrastructure_retry_count + 1, \
                next_dispatch_at = now() + make_interval(secs => \
                    LEAST($3::float8, power(2, dispatch_attempts))::float8), \
                release_reason = 'executor_start_failed' \
          WHERE id = $1 AND status IN ('claimed', 'running') \
         RETURNING status, infrastructure_retry_count",
    )
    .bind(run_id)
    .bind(detail)
    .bind(cfg.backoff_cap_secs)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((status, retry_count)) = row else {
        // 已被回收路径接管或已终态：不重复落失败。
        tx.commit().await?;
        return Ok(());
    };
    // B3-1 语义：执行器未成功启动，任务回 ready 等待下一次领取。任务状态仍经
    // 唯一受守卫入口（R1）；任务不在 claimed（竞态被回收）时跳过。
    let (task_id, task_state): (Option<Uuid>, Option<String>) =
        sqlx::query_as(
            "SELECT t.id, t.status \
           FROM delivery.task_run r \
           JOIN delivery.task t ON t.project_id = r.project_id AND t.id = r.task_id \
          WHERE r.id = $1",
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or((None, None));
    if let (Some(task_id), Some(task_state)) = (task_id, task_state) {
        if task_state == "claimed" {
            let ctx = system_context(&mut tx, project_id)
                .await
                .map_err(|e| RunDispatchError::Transition(e.to_string()))?;
            apply_task_transition(
                &mut tx,
                task_id,
                None,
                TaskStatus::Ready,
                &TransitionFacts::for_dispatch(),
                &ctx,
            )
            .await
            .map_err(|e| RunDispatchError::Transition(e.to_string()))?;
        }
    }
    // 失败记录不可变且每 Run 至多一条（0010）：首败留痕，重试只递增计数。
    sqlx::query(
        "INSERT INTO delivery.task_run_failure \
         (run_id, project_id, category, retryable, reason_code, tenant_id) \
         SELECT $1, $2, 'infrastructure', true, 'executor_start_failed', tenant_id \
           FROM delivery.task_run WHERE id = $1 \
         ON CONFLICT (run_id) DO NOTHING",
    )
    .bind(run_id)
    .bind(project_id)
    .execute(&mut *tx)
    .await?;
    if status == "queued" && retry_count >= cfg.max_infrastructure_attempts {
        sqlx::query(
            "UPDATE delivery.task_run \
                SET status = 'failed', outcome = 'failed', finished_at = now(), \
                    release_reason = 'infrastructure_exhausted' \
              WHERE id = $1 AND status = 'queued'",
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
    }
    audit_run(
        &mut tx,
        project_id,
        "run_start_failed",
        run_id,
        serde_json::json!({
            "error": detail,
            "infrastructure_retry_count": retry_count,
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 终止收尾（Q-D3-5）：条件 UPDATE 先到先得 + failure 行 + 审计同事务。
/// `reason_code`：`timeout` → Run `timed_out` + execution failure；
/// `user_cancel` → Run `cancelled`；其余终止原因不落此路径（失联走 B3-2）。
async fn record_termination(
    pool: &PgPool,
    run_id: Uuid,
    project_id: Uuid,
    reason_code: &str,
) -> Result<(), RunDispatchError> {
    let terminal = if reason_code == "timeout" { "timed_out" } else { "cancelled" };
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE delivery.task_run \
            SET status = $3, outcome = $3, finished_at = now(), terminated_at = now(), \
                termination_reason = $4, \
                lease_owner = NULL, lease_expires_at = NULL, lease_token_hash = NULL \
          WHERE id = $1 AND status IN ('claimed', 'running') \
        RETURNING task_id",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(terminal)
    .bind(reason_code)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(_task_id): Option<Uuid> = updated.map(|row| row.get("task_id")) else {
        tx.commit().await?;
        return Ok(());
    };
    if reason_code == "timeout" {
        sqlx::query(
            "INSERT INTO delivery.task_run_failure \
             (run_id, project_id, category, retryable, reason_code, timeout_seconds, tenant_id) \
             SELECT $1, $2, 'execution', false, 'timeout', timeout_seconds, tenant_id \
               FROM delivery.task_run WHERE id = $1",
        )
        .bind(run_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
    }
    audit_run(
        &mut tx,
        project_id,
        "terminate_task_run",
        run_id,
        serde_json::json!({ "termination_reason": reason_code, "result": terminal }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 进程自然退出的收尾：退出码决定 `succeeded`/`failed`；任务状态不动（D5 裁决）。
async fn finish_run(
    pool: &PgPool,
    run_id: Uuid,
    project_id: Uuid,
    exit_code: Option<i32>,
) -> Result<(), RunDispatchError> {
    let succeeded = exit_code == Some(0);
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE delivery.task_run \
            SET status = $3, outcome = $3, finished_at = now(), \
                lease_owner = NULL, lease_expires_at = NULL, lease_token_hash = NULL \
          WHERE id = $1 AND status IN ('claimed', 'running') \
        RETURNING task_id",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(if succeeded { "succeeded" } else { "failed" })
    .fetch_optional(&mut *tx)
    .await?;
    let Some(_task_id): Option<Uuid> = updated.map(|row| row.get("task_id")) else {
        tx.commit().await?;
        return Ok(());
    };
    if !succeeded {
        sqlx::query(
            "INSERT INTO delivery.task_run_failure \
             (run_id, project_id, category, retryable, reason_code, exit_code, tenant_id) \
             SELECT $1, $2, 'execution', false, 'exit_code', $3, tenant_id \
               FROM delivery.task_run WHERE id = $1",
        )
        .bind(run_id)
        .bind(project_id)
        .bind(exit_code)
        .execute(&mut *tx)
        .await?;
    }
    audit_run(
        &mut tx,
        project_id,
        "run_finished",
        run_id,
        serde_json::json!({ "exit_code": exit_code, "result": if succeeded { "succeeded" } else { "failed" } }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 失联（退出码不可归因）的孤儿：B3-2 隔离旧 Run 并新建重试 Run。
async fn recover_lost(
    pool: &PgPool,
    run_id: Uuid,
    task_id: Uuid,
    project_id: Uuid,
    cfg: &RunDispatchConfig,
) -> Result<(), RunDispatchError> {
    let mut tx = pool.begin().await?;
    let active: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM delivery.task_run \
          WHERE id = $1 AND status IN ('claimed', 'running') FOR UPDATE",
    )
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await?;
    if active.is_none() {
        tx.commit().await?;
        return Ok(());
    }
    // B2：新 Run 的 INSERT 必须在 task 行锁之后。
    sqlx::query("SELECT id FROM delivery.task WHERE id = $1 AND project_id = $2 FOR UPDATE")
        .bind(task_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?;
    recover_lost_run(&mut tx, run_id, task_id, project_id, cfg).await?;
    tx.commit().await?;
    Ok(())
}