//! Slice D3：执行器编排（`docs/slice-d/ai-pm-slice-d3-executor-adapter-v1.md` §3）。
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
use crate::outbox::OutboxAppend;
use crate::run_dispatch::{
    append_executor_event, audit_run, claim_once, recover_lost_run, renew_lease, run_status,
    workspace_root_from_env, ClaimKind, DispatchMode, ExecutableRun, ExecutorProgress,
    ExecutorSession, ExitOutcome, RunDispatchConfig, RunDispatchError, RunExecutor,
    RunRepositoryBaseline, TerminateReason,
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
    mode: DispatchMode,
) -> Result<StartOutcome, RunDispatchError> {
    let outcome = claim_once(pool, worker_id, cfg, mode).await?;
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

    let mut executable = executable_run(pool, run_id, task_id, project_id).await?;
    executable.lease_token = outcome.lease_token.clone();
    executable.lease_generation = outcome.lease_generation;

    // T-032 T1 终裁：session 由编排器生成，并在 spawn **之前**持久化——
    // `executor_session` 是启动意图，不等于进程已经启动。崩溃窗口收窄为：
    // 「意图已落库，spawn 未执行」→ reconcile 按 stage='dispatch' 识别可安全重领。
    let session_text = format!("{}:{}", executor.kind(), Uuid::new_v4());
    {
        let mut tx = pool.begin().await?;
        let prev_session: Option<String> = sqlx::query_scalar(
            "SELECT executor_session FROM delivery.task_run \
              WHERE id = $1 AND status = 'claimed' AND stage = 'dispatch' FOR UPDATE",
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .flatten();
        let updated = sqlx::query(
            "UPDATE delivery.task_run \
                SET executor_kind = $2, executor_session = $3 \
              WHERE id = $1 AND status = 'claimed' AND stage = 'dispatch'",
        )
        .bind(run_id)
        .bind(executor.kind())
        .bind(&session_text)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if updated != 1 {
            // 已被回收路径接管或状态漂移：本 worker 不再启动。
            tx.commit().await?;
            return Ok(StartOutcome::Skipped { run_id });
        }
        // T1 约束②：旧 session（spawn 失败残留）被覆盖前保留历史——
        // events 文件 + 审计双留痕，库内不另设历史表。
        if let Some(old) = &prev_session {
            append_executor_event(
                &workspace_root_from_env(),
                run_id,
                &serde_json::json!({
                    "event": "session_replaced",
                    "old_session": old,
                    "new_session": session_text,
                    "at": chrono::Utc::now().to_rfc3339(),
                }),
            );
            audit_run(
                &mut tx,
                project_id,
                "executor_session_replaced",
                run_id,
                serde_json::json!({
                    "old_session": old,
                    "new_session": session_text,
                }),
            )
            .await?;
        }
        tx.commit().await?;
    }

    match executor.start(&executable, &session_text).await {
        Ok(()) => {
            // 启动确认（I1-E 阶段 C/D）：条件 UPDATE 带 session 匹配（防竞态）+
            // **先锁 Project 行**——归档与启动确认在同一把行锁上串行：
            //   归档先拿锁 → 本登记失败（updated=0，或 archived 显式拒绝）→
            //   立即终止刚 spawn 的子进程（§8.3：登记失败必须 kill）；
            //   登记先拿锁 → 归档随后看到 running Run 走 P2 取消协议。
            // 这补上 I1-025/026/028 的窗口：「spawn 后、PID 登记前归档」不再能
            // 把子进程推进为 running。
            let mut tx = pool.begin().await?;
            match crate::project_gate::lock_project_activity_tx(&mut tx, project_id).await {
                Ok(crate::project_gate::ProjectActivity::Closed) => {
                    tx.rollback().await?;
                    let session = ExecutorSession {
                        session: session_text.clone(),
                        run_id,
                        workspace_root: workspace_root_from_env(),
                    };
                    if let Err(error) = executor
                        .terminate(&session, TerminateReason::Cancelled)
                        .await
                    {
                        // 终止失败：Run 保持 claimed（未被确认），lease 到期后
                        // reconcile/cleanup 兜底重试——终态不被伪造。
                        tracing::warn!(
                            run = %run_id,
                            error = %error,
                            "post-archive start rejected; terminate failed, cleanup will retry"
                        );
                    }
                    return Ok(StartOutcome::StartFailed {
                        run_id,
                        error: "project archived before start confirmation".to_string(),
                    });
                }
                Ok(crate::project_gate::ProjectActivity::Active) => {}
                Err(err) => {
                    tx.rollback().await?;
                    return Err(RunDispatchError::Db(sqlx::Error::Protocol(err.to_string())));
                }
            }
            crate::run_transition_guard::lock_and_validate_transition(
                &mut tx,
                project_id,
                run_id,
                crate::run_transition_guard::RunKind::Delivery,
                crate::run_transition_guard::RunStatus::Running,
            )
            .await
            .map_err(|e| RunDispatchError::Transition(e.to_string()))?;
            let updated = sqlx::query(
                "UPDATE delivery.task_run \
                    SET status = 'running', stage = 'started', started_at = now() \
                  WHERE id = $1 AND status = 'claimed' AND executor_session = $2 AND project_id = $3",
            )
            .bind(run_id)
            .bind(&session_text)
            .bind(project_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if updated == 1 {
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
                        "executor_session": session_text,
                        "workspace_root": workspace_root_from_env(),
                        "lease_generation": outcome.lease_generation,
                    }),
                )
                .await?;
                // S05 监控事件面（0013）：spawn 启动落账（同事务）。
                crate::run_events::record_run_event(
                    &mut tx,
                    run_id,
                    crate::run_events::RunEvent {
                        event_type: crate::run_events::event_type::RUN_STARTED,
                        generation: Some(outcome.lease_generation),
                        dedupe_tag: "",
                        detail: Some(serde_json::json!({
                            "executor_kind": executor.kind(),
                            "executor_session": session_text,
                        })),
                    },
                )
                .await?;
            }
            tx.commit().await?;
            Ok(StartOutcome::Started {
                run_id,
                session: session_text,
            })
        }
        Err(error) => {
            let detail = error.to_string();
            // T1 终裁：spawn 失败保留 executor_session（启动失败痕迹），stage 仍为
            // dispatch → B3-1 扩展判定可在下一轮同 Run 重领并覆盖 session。
            start_failed(pool, run_id, project_id, cfg, &detail).await?;
            Ok(StartOutcome::StartFailed {
                run_id,
                error: detail,
            })
        }
    }
}

/// 监控一轮本 worker 名下的已启动 Run：四信号分立判定 / 兜底续租 / 收尾回写。
/// 返回本轮处理的 Run 数。
///
/// 判定顺序（T-032 T3/T4 终裁）：
/// ① **绝对截止**（`deadline_at` 固化值，优先于一切信号，续租不影响）；
/// ② **Agent 心跳失效**（进程存活但 `last_agent_heartbeat_at` 停滞超阈值——
///    仅对已上报过心跳的 Run 判定，从未接入 MCP 的 Run 不受此约束）；
/// ③ **长期无进展**（`last_progress_at` 停滞超阈值；心跳 ≠ 进展）；
/// ④ 存量 fallback：`deadline_at` 为 NULL 的行按 `began_at + timeout_seconds` 判定；
/// ⑤ 存活探测 + 兜底续租（deadline 已过时续租天然失败 → B3-2）。
pub async fn poll_active_runs(
    pool: &PgPool,
    executor: &dyn RunExecutor,
    worker_id: &str,
    cfg: &RunDispatchConfig,
) -> Result<usize, RunDispatchError> {
    let rows = sqlx::query(
        "SELECT r.id, r.task_id, r.project_id, r.status, r.executor_session, \
                r.lease_expires_at, r.timeout_seconds, r.deadline_at, \
                r.last_agent_heartbeat_at, r.last_progress_at, \
                coalesce(r.started_at, r.claimed_at) AS began_at \
           FROM delivery.task_run r \
          WHERE r.status IN ('claimed', 'running') \
            AND r.executor_session IS NOT NULL \
            AND r.lease_owner = $1 \
            -- N-1 调度屏蔽（S1，0046 scheduling_shield）：人工等待/回复待投递/
            -- 超时未处置的 Run 对四信号（①deadline ④fallback ②心跳 ③进展）
            -- 一律不可见——ACP-427：等待窗口内 deadline 到期不得触发回收/重领，
            -- Interaction 生命周期不受 poll 信号干扰。设计稿
            -- docs/scenarios/s03/ui/acp/smark-pm-s03-acp-slice-design-acp01-02-08-v1.md §4.2。
            AND r.scheduling_shield = 'none' \
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
        let status: String = row.get("status");
        let session_text: String = row.get("executor_session");
        let timeout_seconds: Option<i32> = row.get("timeout_seconds");
        let lease_expires_at: Option<chrono::DateTime<chrono::Utc>> = row.get("lease_expires_at");
        let deadline_at: Option<chrono::DateTime<chrono::Utc>> = row.get("deadline_at");
        let agent_heartbeat_at: Option<chrono::DateTime<chrono::Utc>> =
            row.get("last_agent_heartbeat_at");
        let last_progress_at: Option<chrono::DateTime<chrono::Utc>> = row.get("last_progress_at");
        let began_at: Option<chrono::DateTime<chrono::Utc>> = row.get("began_at");
        let session = ExecutorSession {
            session: session_text,
            run_id,
            workspace_root: workspace_root_from_env(),
        };

        // ① 绝对截止（T3：固化值，优先于租约与一切其他信号）。
        if deadline_exceeded(deadline_at) {
            let _ = executor
                .terminate(&session, TerminateReason::TimedOut)
                .await;
            record_termination(pool, run_id, project_id, "timeout").await?;
            handled += 1;
            continue;
        }
        // ④ 存量 fallback：无固化 deadline 的行沿用 began_at + timeout_seconds。
        if deadline_at.is_none() && timed_out(began_at, timeout_seconds) {
            let _ = executor
                .terminate(&session, TerminateReason::TimedOut)
                .await;
            record_termination(pool, run_id, project_id, "timeout").await?;
            handled += 1;
            continue;
        }

        // ②③ 心跳 / 进展分离（T4）：running（进程已确认启动）才判定；
        //    心跳停滞 = Agent 死锁；无进展 = 长期停滞。二者独立阈值独立判定，
        //    心跳刷新不视为进展。从未上报过心跳/进展的 Run（spawn 后未接 MCP）
        //    列为 NULL，不受判定。
        if status == run_status::RUNNING {
            if stale_since(agent_heartbeat_at, cfg.agent_heartbeat_stale_secs) {
                tracing::warn!(run = %run_id, "agent heartbeat lost; terminating");
                let _ = executor
                    .terminate(&session, TerminateReason::HeartbeatLost)
                    .await;
                record_termination(pool, run_id, project_id, "agent_heartbeat_lost").await?;
                handled += 1;
                continue;
            }
            if stale_since(last_progress_at, cfg.progress_stale_secs) {
                tracing::warn!(run = %run_id, "progress stalled; terminating");
                let _ = executor
                    .terminate(&session, TerminateReason::ProgressStalled)
                    .await;
                record_termination(pool, run_id, project_id, "progress_stalled").await?;
                handled += 1;
                continue;
            }
        }

        // ⑤ 存活探测 + 兜底续租（Q-D3-4：防执行端长静默被误杀；deadline 已过时
        //    续租天然失败 → B3-2 隔离）。
        let progress = match executor.poll(&session).await {
            Ok(progress) => progress,
            Err(error) => {
                tracing::warn!(run = %run_id, error = %error, "executor poll failed; retry next round");
                continue;
            }
        };
        match progress {
            ExecutorProgress::Uncertain => {
                // T1 终裁：三重证据无法确证进程已死 → 按「可能存活」处理，
                // 告警并保留回收任务，**不得**静默判定回收。
                tracing::warn!(
                    run = %run_id,
                    "orphan liveness uncertain (pid file missing but evidence inconclusive); keeping for next round"
                );
            }
            ExecutorProgress::Pending | ExecutorProgress::Running => {
                let expiring_soon = lease_expires_at
                    .map(|at| {
                        at < chrono::Utc::now() + chrono::Duration::seconds(cfg.lease_secs / 2)
                    })
                    .unwrap_or(true);
                if expiring_soon {
                    let renewed = renew_lease(pool, run_id, worker_id, cfg.lease_secs).await?;
                    if !renewed {
                        // 租约续不上（已过期/已被回收路径接管/deadline 已过）：
                        // 杀进程 + B3-2 隔离旧 Run 并新建重试 Run。
                        tracing::info!(run = %run_id, "lease unrenewable; reclaiming as lost");
                        let _ = executor
                            .terminate(&session, TerminateReason::LeaseReclaimed)
                            .await;
                        recover_lost(pool, run_id, task_id, project_id, cfg).await?;
                        handled += 1;
                    }
                }
            }
            ExecutorProgress::Done { outcome } => {
                finish_run(pool, run_id, project_id, outcome).await?;
                handled += 1;
            }
        }
    }
    Ok(handled)
}

/// 用户取消（Q-D3-5：与超时同一条终止管线，终态不同）。
/// Run 不在执行中 / 无执行会话时返回 `false`。
///
/// T-032 D3 终裁：**先撤销凭据（落终态），进程终止作为后续资源清理**——凭据
/// 撤销是 DB 事务（原子且即时），进程终止存在竞态。即使 terminate 失败，
/// Run 已终态 + token 已撤销；残留进程由 [`cleanup_finished_processes`] 兜底。
pub async fn cancel_run(
    pool: &PgPool,
    executor: &dyn RunExecutor,
    run_id: Uuid,
    reason: &str,
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
    // ① 先落终态 + 撤凭据（原子）+ **登记持久化清理账本**（I1-E，迁移 0035）——
    //    终态与清理是两个生命周期（§9.1）：凭据已撤销，进程清理失败不影响终态，
    //    但必须可重试、可追溯（1 小时窗口兜底之外不再丢失，I1-031）。
    let confirmed = record_termination(pool, run_id, project_id, reason).await?;
    if !confirmed {
        return Ok(false);
    }
    if let Some(session_text) = &session_text {
        let mut tx = pool.begin().await?;
        crate::run_cleanup::enqueue(&mut tx, run_id, session_text, None).await?;
        tx.commit().await?;
    }
    // ② 后终止进程（资源清理；失败由兜底清理路径重试）。
    if let Some(session_text) = session_text {
        let session = ExecutorSession {
            session: session_text,
            run_id,
            workspace_root: workspace_root_from_env(),
        };
        // 进程可能已自行退出：terminate 内部先 try_wait，不强杀尸体。
        if let Err(error) = executor
            .terminate(&session, TerminateReason::Cancelled)
            .await
        {
            tracing::warn!(
                run = %run_id,
                error = %error,
                "post-termination process cleanup failed; finished-run cleanup will retry"
            );
            append_executor_event(
                &workspace_root_from_env(),
                run_id,
                &serde_json::json!({
                    "event": "terminate_cleanup_pending",
                    "at": chrono::Utc::now().to_rfc3339(),
                }),
            );
        }
    }
    Ok(true)
}

/// 平台重启后的孤儿 reconcile（Q-D3-5 / 设计稿 §3 异常路径② + T1 三重证据）。
///
/// 孤儿 = `claimed/running` 且 `executor_session` 非空且租约**未过期**的 Run
/// （过期的由 `claim_once` 回收路径按 B3-2 处理，不在此重复）。对每个孤儿
/// `poll` 一次：
/// - `Done{Success}` → 正常收尾 `succeeded`；
/// - `Done{NonZeroExit}` → 收尾 `failed` + failure 行；
/// - `Done{Signaled|Lost}` → 进程被外部/信号终止且退出码不可归因 → 按失联处理
///   （B3-2：旧 Run `timed_out` + 新 Run 重试）；
/// - `Uncertain` → 三重证据无法确证进程已死（T1 终裁）：告警 + 保留，**不得**
///   静默判定回收；
/// - `Running`/`Pending` → 进程跨重启仍存活，保留：执行端心跳继续续租，
///   租约过期后由回收路径接管（本 worker 不再续，`lease_owner` 不匹配）。
pub async fn reconcile_orphans(
    pool: &PgPool,
    executor: &dyn RunExecutor,
    cfg: &RunDispatchConfig,
) -> Result<usize, RunDispatchError> {
    // P3（I1，B-1 默认并入）：超窗残留进程告警——running 且租约过期超过 1 小时
    // 仍非终态的 Run，落 `run.orphan_suspected` 审计（1 小时去重）+ warn 日志。
    // B06-D3：确认即回收——首次检出的孤儿（租约过期 >1h 且从未被回收）直接
    // 执行回收（Run=timed_out + 任务回 Ready），不再停留于「只告警」。
    let orphans: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT r.id, r.project_id \
           FROM delivery.task_run r \
          WHERE r.status = 'running' AND r.executor_session IS NOT NULL \
            AND r.lease_expires_at < now() - interval '1 hour' \
            AND NOT EXISTS ( \
                 SELECT 1 FROM governance.audit_log a \
                  WHERE a.action = 'run.orphan_reclaimed' AND a.target_id = r.id)",
    )
    .fetch_all(pool)
    .await?;
    for (run_id, project_id) in orphans {
        let Ok(mut tx) = pool.begin().await else {
            continue;
        };
        if crate::pm_write::append_audit(
            &mut tx,
            project_id,
            &crate::actor::AuditActor::dispatch(),
            "run.orphan_suspected",
            "task_run",
            Some(run_id),
        )
        .await
        .is_ok()
        {
            let _ = tx.commit().await;
        }
        tracing::warn!(%run_id, %project_id, "orphan run suspected: lease expired over an hour ago");

        // B06-D3（业务验收 2026-11-24）：此前只告警不回收——Model A（MCP 认领）
        // 的孤儿 Run 没有任何执行体处理，任务永久卡 running。首次确认即回收：
        // Run=timed_out（isolate 语义）+ 任务 Running→Blocked→Ready（域合法边，
        // 可被 runner 重新认领）；`run.orphan_reclaimed` 审计留痕。
        let Ok(mut tx) = pool.begin().await else {
            continue;
        };
        let run_reclaimed = sqlx::query(
            "UPDATE delivery.task_run               SET status = 'timed_out', outcome = 'timed_out', finished_at = now(),                   termination_reason = 'lease_expired',                   lease_owner = NULL, lease_expires_at = NULL, lease_token_hash = NULL             WHERE id = $1 AND project_id = $2 AND status = 'running'",
        )
        .bind(run_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await
        .map(|r| r.rows_affected())
        .unwrap_or(0);
        if run_reclaimed > 0 {
            let task_reclaimed = sqlx::query(
                "WITH stale AS (                     UPDATE delivery.task SET status = 'blocked', updated_at = now()                      WHERE id = (SELECT task_id FROM delivery.task_run WHERE id = $1)                        AND status = 'running' RETURNING id                  ), released AS (                     UPDATE delivery.task SET status = 'ready', updated_at = now()                      WHERE id = (SELECT id FROM stale) AND status = 'blocked' RETURNING id                  )                  SELECT count(*) FROM released",
            )
            .bind(run_id)
            .fetch_one(&mut *tx)
            .await
            .map(|row| row.get::<i64, _>(0))
            .unwrap_or(0);
            let _ = crate::pm_write::append_audit(
                &mut tx,
                project_id,
                &crate::actor::AuditActor::dispatch(),
                "run.orphan_reclaimed",
                "task_run",
                Some(run_id),
            )
            .await;
            let _ = tx.commit().await;
            tracing::info!(
                %run_id, %project_id,
                task_released = task_reclaimed,
                "orphan run reclaimed (lease_expired); task released back to ready"
            );
        } else {
            let _ = tx.rollback().await;
        }
    }

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
            ExecutorProgress::Done { outcome } => {
                if matches!(outcome, ExitOutcome::Success) {
                    finish_run(pool, run_id, project_id, outcome).await?;
                } else {
                    recover_lost(pool, run_id, task_id, project_id, cfg).await?;
                }
                handled += 1;
            }
            ExecutorProgress::Uncertain => {
                tracing::warn!(
                    run = %run_id,
                    "orphan reconcile: liveness uncertain (T1 three-evidence inconclusive); keeping"
                );
            }
            ExecutorProgress::Pending | ExecutorProgress::Running => {}
        }
    }
    Ok(handled)
}

/// T-032 D3 终裁：**终态僵尸进程兜底**。终止顺序反转（先撤凭据后杀进程）引入
/// 的窗口——Run 已终态但 terminate 失败时，`reconcile_orphans`（只扫
/// `claimed/running`）不覆盖终态行。本清理对近期终态且 session 非空的 Run 补
/// terminate；worker 主循环周期性调用。失败仅告警（下一轮重试）。
pub async fn cleanup_finished_processes(
    pool: &PgPool,
    executor: &dyn RunExecutor,
) -> Result<usize, RunDispatchError> {
    let rows = sqlx::query(
        "SELECT id, executor_session FROM delivery.task_run \
          WHERE status IN ('succeeded', 'failed', 'timed_out', 'cancelled') \
            AND executor_session IS NOT NULL \
            AND finished_at > now() - interval '1 hour' \
          ORDER BY finished_at DESC \
          LIMIT 64",
    )
    .fetch_all(pool)
    .await?;

    let mut handled = 0;
    for row in rows {
        let run_id: Uuid = row.get("id");
        let session_text: String = row.get("executor_session");
        let session = ExecutorSession {
            session: session_text,
            run_id,
            workspace_root: workspace_root_from_env(),
        };
        match executor
            .terminate(&session, TerminateReason::LeaseReclaimed)
            .await
        {
            Ok(()) => {
                append_executor_event(
                    &workspace_root_from_env(),
                    run_id,
                    &serde_json::json!({
                        "event": "finished_run_cleanup",
                        "at": chrono::Utc::now().to_rfc3339(),
                    }),
                );
                handled += 1;
            }
            Err(error) => {
                tracing::warn!(
                    run = %run_id,
                    error = %error,
                    "finished-run process cleanup failed; retry next round"
                );
            }
        }
    }
    Ok(handled)
}

// ------------------------------------------------------------------ internals

/// T-032 T3：绝对截止判定——固化 `deadline_at` 已过即超时（NULL 不判定）。
fn deadline_exceeded(deadline_at: Option<chrono::DateTime<chrono::Utc>>) -> bool {
    matches!(deadline_at, Some(at) if at <= chrono::Utc::now())
}

/// T-032 T4：停滞判定——`at` 距今超过 `stale_secs` 即停滞（NULL 不判定）。
fn stale_since(at: Option<chrono::DateTime<chrono::Utc>>, stale_secs: i64) -> bool {
    matches!(at, Some(at) if (chrono::Utc::now() - at) > chrono::Duration::seconds(stale_secs))
}

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
    let project_id: Uuid =
        sqlx::query_scalar("SELECT project_id FROM delivery.task_run WHERE id = $1")
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
        "SELECT attempt_no, timeout_seconds, lease_generation, deadline_at \
           FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await?;
    let attempt_no: i32 = row.get("attempt_no");
    let timeout_seconds: Option<i32> = row.get("timeout_seconds");
    let lease_generation: i32 = row.get("lease_generation");
    let deadline_at: Option<chrono::DateTime<chrono::Utc>> = row.get("deadline_at");
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
    // I3-E 执行期强校验（V1.1 简化裁决）：仓库能否进入执行环境，以**本次 Run 的
    // 定格访问快照**为准——不在快照内的仓（范围外 / 非 verified）不得暴露给执行端。
    let policy_snapshot: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT policy_snapshot FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_optional(pool)
            .await?
            .flatten();
    let repositories = baseline_rows
        .iter()
        .filter(|row| {
            let repository_id: Uuid = row.get("repository_id");
            crate::repo_access::snapshot_allows(
                policy_snapshot.as_ref(),
                repository_id,
                pm_domain::repo_access::RepoAccess::ReadWrite,
            )
        })
        .map(|row| RunRepositoryBaseline {
            repository_id: row.get("repository_id"),
            base_ref: row.get("base_ref"),
            base_commit: row.get("base_commit"),
            write_access: row.get("write_access"),
            workspace_path: row.get("workspace_path"),
        })
        .collect::<Vec<_>>();
    if repositories.is_empty() {
        // 快照缺失 / 无可访问仓库 = deny-all：不得把仓库交给执行端。
        return Err(RunDispatchError::InvalidOption(
            "no accessible repository in the run access snapshot".to_string(),
        ));
    }
    Ok(ExecutableRun {
        run_id,
        task_id,
        project_id,
        attempt_no,
        timeout_seconds,
        lease_generation,
        // 一次性明文不在库内（只存 hash）：由 claim_and_start 从 ClaimOutcome 注入。
        lease_token: None,
        deadline_at,
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
    let from_status = match crate::run_transition_guard::lock_and_validate_transition(
        &mut tx,
        project_id,
        run_id,
        crate::run_transition_guard::RunKind::Delivery,
        crate::run_transition_guard::RunStatus::Queued,
    )
    .await
    {
        Ok(status) => status,
        Err(crate::run_transition_guard::RunTransitionGuardError::NotFound(_))
        | Err(crate::run_transition_guard::RunTransitionGuardError::Invalid(
            crate::run_transition_guard::RunTransitionError::TerminalImmutable(_),
        ))
        | Err(crate::run_transition_guard::RunTransitionGuardError::Invalid(
            crate::run_transition_guard::RunTransitionError::IllegalEdge {
                kind: crate::run_transition_guard::RunKind::Delivery,
                from: crate::run_transition_guard::RunStatus::Queued,
                to: crate::run_transition_guard::RunStatus::Queued,
            },
        )) => {
            tx.commit().await?;
            return Ok(());
        }
        Err(error) => return Err(RunDispatchError::Transition(error.to_string())),
    };
    let row: Option<(String, i32)> = sqlx::query_as(
        "UPDATE delivery.task_run \
            SET status = 'queued', lease_owner = NULL, lease_expires_at = NULL, \
                lease_token_hash = NULL, dispatch_attempts = dispatch_attempts + 1, \
                infrastructure_retry_count = infrastructure_retry_count + 1, \
                next_dispatch_at = now() + make_interval(secs => \
                    LEAST($2::float8, power(2, dispatch_attempts))::float8), \
                release_reason = 'executor_start_failed' \
          WHERE id = $1 AND status = $3 AND project_id = $4 \
         RETURNING status, infrastructure_retry_count",
    )
    .bind(run_id)
    .bind(cfg.backoff_cap_secs)
    .bind(from_status.as_str())
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((status, retry_count)) = row else {
        // 已被回收路径接管或已终态：不重复落失败。
        tx.commit().await?;
        return Ok(());
    };
    // B3-1 语义：执行器未成功启动，任务回 ready 等待下一次领取。任务状态仍经
    // 唯一受守卫入口（R1）；任务不在 claimed（竞态被回收）时跳过。
    let (task_id, task_state): (Option<Uuid>, Option<String>) = sqlx::query_as(
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
        crate::run_transition_guard::lock_and_validate_transition(
            &mut tx,
            project_id,
            run_id,
            crate::run_transition_guard::RunKind::Delivery,
            crate::run_transition_guard::RunStatus::Failed,
        )
        .await
        .map_err(|e| RunDispatchError::Transition(e.to_string()))?;
        let exhausted = sqlx::query(
            "UPDATE delivery.task_run \
                SET status = 'failed', outcome = 'failed', finished_at = now(), \
                    release_reason = 'infrastructure_exhausted' \
              WHERE id = $1 AND project_id = $2 AND status = 'queued'",
        )
        .bind(run_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if exhausted == 1 {
            // S05 监控事件面（0013）：基础设施耗尽终态落账（同事务）。
            crate::run_events::record_run_event(
                &mut tx,
                run_id,
                crate::run_events::RunEvent {
                    event_type: crate::run_events::event_type::RUN_FINISHED,
                    generation: None,
                    dedupe_tag: "",
                    detail: Some(serde_json::json!({
                        "outcome": "failed",
                        "reason": "infrastructure_exhausted",
                    })),
                },
            )
            .await?;
        }
    } else {
        // S05 监控事件面（0013）：启动失败退避回队落账（同 generation 至多一次：
        // 重试必先重领 → generation+1）。
        crate::run_events::record_run_event(
            &mut tx,
            run_id,
            crate::run_events::RunEvent {
                event_type: crate::run_events::event_type::RUN_STAGE_CHANGED,
                generation: None,
                dedupe_tag: "requeued",
                detail: Some(serde_json::json!({ "reason": "executor_start_failed" })),
            },
        )
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

/// 终止收尾（Q-D3-5 + T-032 D3 终裁）：条件 UPDATE 先到先得 + failure 行 +
/// 审计同事务；**本函数即「撤销凭据」动作**（终态 + 清 lease_token_hash 原子
/// 落库）——调用方（cancel_run / poll）必须在调用它**之后**才 terminate 进程。
/// `reason_code`：`timeout` → `timed_out`；`user_cancel` → `cancelled`；
/// `agent_heartbeat_lost` / `progress_stalled` → `failed`（执行类失败）；
/// 失联走 B3-2，不落此路径。返回是否命中（Run 确实处于活跃态并被收尾）。
async fn record_termination(
    pool: &PgPool,
    run_id: Uuid,
    project_id: Uuid,
    reason_code: &str,
) -> Result<bool, RunDispatchError> {
    let terminal = match reason_code {
        "timeout" => "timed_out",
        // 用户取消与关闭收口（I1-D/P2）：同一「人为终止」语义 → cancelled。
        "user_cancel" | "project_closed" => "cancelled",
        // 心跳丢失 / 进展停滞：执行类失败（Agent 死锁 / 长期停滞），终态 failed。
        _ => "failed",
    };
    let run_kind_text: Option<String> = sqlx::query_scalar(
        "SELECT run_kind FROM delivery.task_run WHERE id = $1 AND project_id = $2",
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let Some(run_kind_text) = run_kind_text else {
        return Ok(false);
    };
    let run_kind = crate::run_transition_guard::RunKind::parse(&run_kind_text)
        .map_err(|e| RunDispatchError::Transition(e.to_string()))?;
    if run_kind == crate::run_transition_guard::RunKind::Review && reason_code != "project_closed" {
        return Err(RunDispatchError::Transition(
            "Reviewer Run cancellation is only permitted during Project archival".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    let target_status = crate::run_transition_guard::RunStatus::parse(terminal)
        .map_err(|e| RunDispatchError::Transition(e.to_string()))?;
    let from_status = match crate::run_transition_guard::lock_and_validate_transition(
        &mut tx,
        project_id,
        run_id,
        run_kind,
        target_status,
    )
    .await
    {
        Ok(status) => status,
        Err(crate::run_transition_guard::RunTransitionGuardError::NotFound(_))
        | Err(crate::run_transition_guard::RunTransitionGuardError::Invalid(
            crate::run_transition_guard::RunTransitionError::TerminalImmutable(_),
        ))
        | Err(crate::run_transition_guard::RunTransitionGuardError::Invalid(
            crate::run_transition_guard::RunTransitionError::IllegalEdge {
                kind: crate::run_transition_guard::RunKind::Delivery,
                from: crate::run_transition_guard::RunStatus::Queued,
                ..
            },
        )) => {
            tx.commit().await?;
            return Ok(false);
        }
        Err(error) => return Err(RunDispatchError::Transition(error.to_string())),
    };
    let updated = sqlx::query(
        "UPDATE delivery.task_run \
            SET status = $3, outcome = $3, finished_at = now(), terminated_at = now(), \
                termination_reason = $4, \
                lease_owner = NULL, lease_expires_at = NULL, lease_token_hash = NULL \
          WHERE id = $1 AND project_id = $2 AND status = $5 \
         RETURNING task_id",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(terminal)
    .bind(reason_code)
    .bind(from_status.as_str())
    .fetch_optional(&mut *tx)
    .await?;
    let Some(task_id): Option<Uuid> = updated.map(|row| row.get("task_id")) else {
        tx.commit().await?;
        return Ok(false);
    };
    match reason_code {
        "timeout" => {
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
        "agent_heartbeat_lost" | "progress_stalled" => {
            sqlx::query(
                "INSERT INTO delivery.task_run_failure \
                 (run_id, project_id, category, retryable, reason_code, tenant_id) \
                 SELECT $1, $2, 'execution', false, $3, tenant_id \
                   FROM delivery.task_run WHERE id = $1 \
                 ON CONFLICT (run_id) DO NOTHING",
            )
            .bind(run_id)
            .bind(project_id)
            .bind(reason_code)
            .execute(&mut *tx)
            .await?;
        }
        _ => {}
    }
    audit_run(
        &mut tx,
        project_id,
        "terminate_task_run",
        run_id,
        serde_json::json!({ "termination_reason": reason_code, "result": terminal }),
    )
    .await?;
    // S05 监控事件面（0013）：人为/监管终止终态落账（同事务）。
    crate::run_events::record_run_event(
        &mut tx,
        run_id,
        crate::run_events::RunEvent {
            event_type: crate::run_events::event_type::RUN_FINISHED,
            generation: None,
            dedupe_tag: "",
            detail: Some(serde_json::json!({
                "outcome": terminal,
                "reason": reason_code,
            })),
        },
    )
    .await?;
    emit_run_completed(&mut tx, project_id, run_id, task_id, terminal, None).await?;
    tx.commit().await?;
    Ok(true)
}

/// 进程自然退出的收尾（T-032 §6.4：结构化 `ExitOutcome` 决定终态）；
/// 任务状态不动（D5 裁决）。
async fn finish_run(
    pool: &PgPool,
    run_id: Uuid,
    project_id: Uuid,
    outcome: ExitOutcome,
) -> Result<(), RunDispatchError> {
    let succeeded = matches!(outcome, ExitOutcome::Success);
    let mut tx = pool.begin().await?;
    let target_status = if succeeded {
        crate::run_transition_guard::RunStatus::Succeeded
    } else {
        crate::run_transition_guard::RunStatus::Failed
    };
    let from_status = match crate::run_transition_guard::lock_and_validate_transition(
        &mut tx,
        project_id,
        run_id,
        crate::run_transition_guard::RunKind::Delivery,
        target_status,
    )
    .await
    {
        Ok(status) => status,
        Err(crate::run_transition_guard::RunTransitionGuardError::NotFound(_))
        | Err(crate::run_transition_guard::RunTransitionGuardError::Invalid(
            crate::run_transition_guard::RunTransitionError::TerminalImmutable(_),
        ))
        | Err(crate::run_transition_guard::RunTransitionGuardError::Invalid(
            crate::run_transition_guard::RunTransitionError::IllegalEdge {
                kind: crate::run_transition_guard::RunKind::Delivery,
                from: crate::run_transition_guard::RunStatus::Queued,
                ..
            },
        )) => {
            tx.commit().await?;
            return Ok(());
        }
        Err(error) => return Err(RunDispatchError::Transition(error.to_string())),
    };
    let updated = sqlx::query(
        "UPDATE delivery.task_run \
            SET status = $3, outcome = $3, finished_at = now(), \
                lease_owner = NULL, lease_expires_at = NULL, lease_token_hash = NULL \
          WHERE id = $1 AND project_id = $2 AND status = $4 \
        RETURNING task_id",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(if succeeded { "succeeded" } else { "failed" })
    .bind(from_status.as_str())
    .fetch_optional(&mut *tx)
    .await?;
    let Some(task_id): Option<Uuid> = updated.map(|row| row.get("task_id")) else {
        tx.commit().await?;
        return Ok(());
    };
    if !succeeded {
        let (reason_code, exit_code): (&str, Option<i32>) = match outcome {
            ExitOutcome::NonZeroExit { code } => ("exit_code", Some(code)),
            ExitOutcome::Signaled { .. } => ("signaled", None),
            ExitOutcome::Lost => ("exit_code_lost", None),
            ExitOutcome::Killed { .. } => ("killed", None),
            ExitOutcome::Success => unreachable!("succeeded handled above"),
        };
        sqlx::query(
            "INSERT INTO delivery.task_run_failure \
             (run_id, project_id, category, retryable, reason_code, exit_code, tenant_id) \
             SELECT $1, $2, 'execution', false, $3, $4, tenant_id \
               FROM delivery.task_run WHERE id = $1",
        )
        .bind(run_id)
        .bind(project_id)
        .bind(reason_code)
        .bind(exit_code)
        .execute(&mut *tx)
        .await?;
    }
    audit_run(
        &mut tx,
        project_id,
        "run_finished",
        run_id,
        serde_json::json!({ "outcome": format!("{outcome:?}"), "result": if succeeded { "succeeded" } else { "failed" } }),
    )
    .await?;
    // ACP-05 终态钩子（0051）：Run 终态 → 非终态 continuation 全部 closed
    //（closed 为终态不可迁移；Task 取消路径经 cancel_open_interaction_tx 同理）。
    crate::continuation::close_open_by_run_tx(
        &mut tx,
        project_id,
        run_id,
        "run_finished",
        &crate::actor::AuditActor::dispatch(),
    )
    .await
    .map_err(|e| RunDispatchError::InvalidOption(e.to_string()))?;
    emit_run_completed(
        &mut tx,
        project_id,
        run_id,
        task_id,
        if succeeded { "succeeded" } else { "failed" },
        None,
    )
    .await?;
    // S05 监控事件面（0013）：进程自然退出终态落账（同事务；先到先得的条件
    // UPDATE 保证同 generation 至多一次）。
    crate::run_events::record_run_event(
        &mut tx,
        run_id,
        crate::run_events::RunEvent {
            event_type: crate::run_events::event_type::RUN_FINISHED,
            generation: None,
            dedupe_tag: "",
            detail: Some(serde_json::json!({
                "outcome": if succeeded { "succeeded" } else { "failed" },
                "exit": format!("{outcome:?}"),
            })),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 在 Run 终态事务里产生 `RunCompleted` 事实通知事件（T-061 §5）。
///
/// 语义：**有消费者的事实通知事件**——消费者是 `RunCompletedHandler`，消费动作
/// 是记审计 + 生成执行完成通知；**不改 Run 状态、不触发 D5 失败判定、不重新
/// 调度 Task**（状态推进权归 D5 失败判定作业）。
///
/// 幂等键 `run_completed:<run_id>`：一个 Run 只可能有一次终态（条件 UPDATE
/// `WHERE status IN ('claimed','running')` 先到先得，第二个到达者直接返回，
/// **不写事件**），run_id 同时是生产端与消费端的幂等锚点。
///
/// 与终态、审计**同事务**追加——「状态已变、事件没写」不可表示。
async fn emit_run_completed(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    run_id: Uuid,
    task_id: Uuid,
    status: &str,
    attempt_no: Option<i32>,
) -> Result<(), RunDispatchError> {
    if !crate::outbox::event_is_enabled("RunCompleted") {
        return Ok(());
    }
    let aggregate_id = run_id.to_string();
    let idempotency_key = format!("run_completed:{run_id}");
    let mut payload = serde_json::json!({
        "run_id": run_id,
        "task_id": task_id,
        "status": status,
        "outcome": status,
    });
    if let Some(attempt_no) = attempt_no {
        payload["attempt_no"] = serde_json::json!(attempt_no);
    }
    crate::outbox::PgOutboxAppend
        .append(
            tx,
            crate::outbox::OutboxEvent {
                project_id,
                event_type: "RunCompleted",
                aggregate_type: "TaskRun",
                aggregate_id: &aggregate_id,
                payload,
                idempotency_key: &idempotency_key,
            },
        )
        .await?;
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
