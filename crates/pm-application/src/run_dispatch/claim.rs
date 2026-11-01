use super::*;

/// 一次 `claim_once` 的结果类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimKind {
    /// 正常领取（`queued → claimed`）或恢复重领同一 Run（B3-1）。
    Claimed,
    /// 已启动执行器的旧 Run 失联，已隔离并新建 Run（B3-2）。
    Reclaimed,
    /// 条件不满足：Run 退避回 `queued`。
    Skipped,
    /// 任务已终态/被取代/不存在：Run 落 `cancelled`。
    Cancelled,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimOutcome {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub kind: ClaimKind,
    /// Model A 签发的一次性租约明文；Model B 与未真正领取的分支都为 `None`。
    pub lease_token: Option<String>,
}


/// 租约持有者（A 裁决）：Model B 是 worker 标识，Model A 是 MCP 客户端。
///
/// **`lease_owner` 不是凭证**：它只用于展示与 fencing 的辅助归因，真正的凭证
/// 是 `lease_token`（见 `docs/ai-pm-slice-d2-mcp-context-v1.md` §三 A 的关键推论）。
#[derive(Debug, Clone, Copy)]
pub(crate) enum LeaseOwner<'a> {
    Worker(&'a str),
    Mcp { client_name: &'a str },
}


/// 领取路径的全部分叉点（A-2：复用同一事务模板，只以参数表达差异）。
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClaimOptions<'a> {
    lease_owner: LeaseOwner<'a>,
    /// Model A 必填：把候选限定在调用方项目内。
    project_filter: Option<Uuid>,
    /// Model A 必填：只领指定任务。
    task_filter: Option<Uuid>,
    /// Model A 恒 `false`（A-1：执行端显式请求不受 `AI_PM_RUN_DISPATCH` 约束）。
    gate_by_dispatch_mode: bool,
    mode: DispatchMode,
    /// 只有 Model A 签发 `lease_token`。
    issue_token: bool,
    client_name: Option<&'a str>,
    client_version: Option<&'a str>,
}


/// Model A（执行端显式领取）成功后的返回：一次领取即可拿到的全部凭据。
#[derive(Debug, Clone)]
pub struct ClaimedTaskRun {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub attempt_no: i32,
    /// 32 字节随机数的小写 hex（64 字符），**只返回这一次**。
    pub lease_token: String,
    pub lease_expires_at: chrono::DateTime<chrono::Utc>,
    pub git_branch: String,
    pub stage: String,
}

/// 执行策略读取端口（G5）：策略读取失败必须**拒绝领取**，不得默认放行。
/// 默认实现等价于此前的字面量 `true`，测试可注入失败实现（验收 C12）。
pub trait ExecutionPolicy: Send + Sync {
    fn execution_enabled(&self) -> Result<bool, String>;
}


/// 生产默认策略：可执行（与 D1 快照中的字面量语义一致）。
pub struct DefaultExecutionPolicy;


impl ExecutionPolicy for DefaultExecutionPolicy {
    fn execution_enabled(&self) -> Result<bool, String> {
        Ok(true)
    }
}


/// 领取（或回收）**恰好一条**可执行的 Run；无候选返回 `Ok(None)`。
///
/// 语义要点：
/// - 候选 = `queued` 且到期，或 `claimed` 且租约过期。**领取与回收是同一条路径**
///   （复用 outbox 的 reclaim 范式），因此不需要独立的 reaper（需求 10）。
/// - B3-1：`executor_session IS NULL` 的租约过期 → 重领**同一** Run，不新建、不记失败。
/// - B3-2：`executor_session IS NOT NULL` 的失联 → 旧 Run 落 `timed_out` 并记录终止
///   结果，再新建 Run（`attempt_no+1`、`supersedes_run_id` 指向旧 Run）。
pub async fn claim_once(
    pool: &PgPool,
    worker_id: &str,
    cfg: &RunDispatchConfig,
    mode: DispatchMode,
) -> Result<Option<ClaimOutcome>, RunDispatchError> {
    claim_once_with_policy(pool, worker_id, cfg, mode, &DefaultExecutionPolicy).await
}


/// 同 [`claim_once`]，但可注入执行策略读取实现（G5 / 验收 C12）。
pub async fn claim_once_with_policy(
    pool: &PgPool,
    worker_id: &str,
    cfg: &RunDispatchConfig,
    mode: DispatchMode,
    policy: &dyn ExecutionPolicy,
) -> Result<Option<ClaimOutcome>, RunDispatchError> {
    claim_core(
        pool,
        cfg,
        &ClaimOptions {
            lease_owner: LeaseOwner::Worker(worker_id),
            project_filter: None,
            task_filter: None,
            gate_by_dispatch_mode: true,
            mode,
            issue_token: false,
            client_name: None,
            client_version: None,
        },
        policy,
    )
    .await
}


/// **Model A（A 裁决）：执行端显式领取指定任务**。
///
/// 不受 `AI_PM_RUN_DISPATCH` 开关约束（A-1）——该开关只门控 worker 的无人值守领取。
/// 成功时一次性签发 `lease_token`（明文只在这里返回，库内只存 sha256 小写 hex）。
///
/// 前置条件（§6.1）：任务不存在/跨项目 → [`RunDispatchError::TaskNotFound`]；
/// 没有可领取的 Run（TaskReadyHandler 尚未创建）→ [`RunDispatchError::NoRunnableRun`]；
/// 已有其他活跃 Run → [`RunDispatchError::RunAlreadyClaimed`]；
/// 任务非 `ready` → [`RunDispatchError::TaskNotReady`]。
pub async fn claim_task_run(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    client_name: &str,
    client_version: Option<&str>,
    cfg: &RunDispatchConfig,
) -> Result<ClaimedTaskRun, RunDispatchError> {
    claim_task_run_with_policy(
        pool,
        project_id,
        task_id,
        client_name,
        client_version,
        cfg,
        &DefaultExecutionPolicy,
    )
    .await
}


/// 同 [`claim_task_run`]，但可注入执行策略读取实现（G5 / 验收 C12）。
#[allow(clippy::too_many_arguments)]
pub async fn claim_task_run_with_policy(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    client_name: &str,
    client_version: Option<&str>,
    cfg: &RunDispatchConfig,
    policy: &dyn ExecutionPolicy,
) -> Result<ClaimedTaskRun, RunDispatchError> {
    let outcome = claim_core(
        pool,
        cfg,
        &ClaimOptions {
            lease_owner: LeaseOwner::Mcp { client_name },
            project_filter: Some(project_id),
            task_filter: Some(task_id),
            gate_by_dispatch_mode: false,
            mode: DispatchMode::On,
            issue_token: true,
            client_name: Some(client_name),
            client_version,
        },
        policy,
    )
    .await?;
    let outcome = outcome.ok_or(RunDispatchError::NoRunnableRun)?;
    let lease_token = outcome.lease_token.ok_or(RunDispatchError::NoRunnableRun)?;
    let (attempt_no, lease_expires_at, stage, git_branch): (
        i32,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT r.attempt_no, r.lease_expires_at, r.stage, t.git_branch \
           FROM delivery.task_run r JOIN delivery.task t ON t.id = r.task_id \
          WHERE r.id = $1 AND r.project_id = $2",
    )
    .bind(outcome.run_id)
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    Ok(ClaimedTaskRun {
        run_id: outcome.run_id,
        task_id: outcome.task_id,
        attempt_no,
        lease_token,
        lease_expires_at: lease_expires_at
            .ok_or_else(|| RunDispatchError::InvalidOption("lease_expires_at missing".into()))?,
        git_branch: git_branch
            .ok_or_else(|| RunDispatchError::InvalidOption("git_branch missing".into()))?,
        stage: stage.unwrap_or_else(|| run_stage::DISPATCH.to_string()),
    })
}


/// A/B 两入口的**唯一事务模板**（A-2）：候选筛选、租约签发、开关门控与审计
/// payload 都由 [`ClaimOptions`] 表达，禁止复制事务体。
pub(crate) async fn claim_core(
    pool: &PgPool,
    cfg: &RunDispatchConfig,
    opts: &ClaimOptions<'_>,
    policy: &dyn ExecutionPolicy,
) -> Result<Option<ClaimOutcome>, RunDispatchError> {
    let mut tx = pool.begin().await?;

    // ① 选候选 Run（不阻塞其他 Worker）。顺序：先锁 run。
    let candidate = if let Some(task_id) = opts.task_filter {
        let project_id = opts
            .project_filter
            .ok_or_else(|| RunDispatchError::InvalidOption("Model A requires a project scope".into()))?;
        sqlx::query(
            "SELECT r.id, r.task_id, r.project_id, r.status, r.dispatch_attempts, \
                    r.executor_session, r.attempt_no \
               FROM delivery.task_run r \
              WHERE r.task_id = $1 AND r.project_id = $2 \
                AND ((r.status = 'queued'  AND r.next_dispatch_at <= now()) \
                  OR (r.status = 'claimed' AND r.lease_expires_at < now())) \
              ORDER BY r.attempt_no DESC, r.created_at \
              FOR UPDATE OF r SKIP LOCKED \
              LIMIT 1",
        )
        .bind(task_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?
    } else {
        sqlx::query(
            "SELECT r.id, r.task_id, r.project_id, r.status, r.dispatch_attempts, \
                    r.executor_session, r.attempt_no \
               FROM delivery.task_run r \
              WHERE (r.status = 'queued'  AND r.next_dispatch_at <= now()) \
                 OR (r.status = 'claimed' AND r.lease_expires_at < now()) \
              ORDER BY r.next_dispatch_at, r.created_at \
              FOR UPDATE OF r SKIP LOCKED \
              LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?
    };
    let Some(candidate) = candidate else {
        if let Some(task_id) = opts.task_filter {
            // Model A：区分「任务不存在」「已有活跃 Run」「等 TaskReady」三种前置失败。
            let project_id = opts
                .project_filter
                .ok_or_else(|| RunDispatchError::InvalidOption("Model A requires a project scope".into()))?;
            let task_exists: Option<String> = sqlx::query_scalar(
                "SELECT status FROM delivery.task WHERE id = $1 AND project_id = $2",
            )
            .bind(task_id)
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?;
            if task_exists.is_none() {
                tx.rollback().await?;
                return Err(RunDispatchError::TaskNotFound);
            }
            let active: i64 = sqlx::query_scalar(concat!(
                "SELECT count(*) FROM delivery.task_run \
                  WHERE task_id = $1 AND status IN (",
                "'queued', 'claimed', 'running'",
                ")"
            ))
            .bind(task_id)
            .fetch_one(&mut *tx)
            .await?;
            tx.rollback().await?;
            return Err(if active > 0 {
                RunDispatchError::RunAlreadyClaimed
            } else {
                RunDispatchError::NoRunnableRun
            });
        }
        tx.commit().await?;
        return Ok(None);
    };
    let mut run_id: Uuid = candidate.get("id");
    let task_id: Uuid = candidate.get("task_id");
    let project_id: Uuid = candidate.get("project_id");
    let run_state: String = candidate.get("status");
    let dispatch_attempts: i32 = candidate.get("dispatch_attempts");
    let executor_session: Option<String> = candidate.get("executor_session");

    // ② 锁任务行（顺序固定：run → task）。B2：新 Run 的 INSERT 必须在此之后。
    let task = sqlx::query(
        "SELECT status, revision, retry_exhausted, task_type FROM delivery.task \
          WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(task) = task else {
        cancel_run(&mut tx, run_id, "task_missing").await?;
        tx.commit().await?;
        if opts.task_filter.is_some() {
            return Err(RunDispatchError::TaskNotFound);
        }
        return Ok(Some(ClaimOutcome {
            run_id,
            task_id,
            kind: ClaimKind::Cancelled,
            lease_token: None,
        }));
    };
    let mut task_state: String = task.get("status");
    let retry_exhausted: bool = task.get("retry_exhausted");
    let task_type: Option<String> = task.get("task_type");

    // ③ 任务已终态 / 被取代 → Run 是死记录，落 cancelled，不动任务。
    let terminal_or_gone = TaskStatus::from_str(&task_state)
        .map(|s| s.is_terminal())
        .unwrap_or(true);
    if terminal_or_gone {
        cancel_run(&mut tx, run_id, "task_terminal").await?;
        tx.commit().await?;
        if opts.task_filter.is_some() {
            return Err(RunDispatchError::TaskNotReady(task_state));
        }
        return Ok(Some(ClaimOutcome {
            run_id,
            task_id,
            kind: ClaimKind::Cancelled,
            lease_token: None,
        }));
    }

    // R1：构造系统执行身份（审计 actor 必须存在）；所有 Task 状态变更经统一
    // 受守卫服务 apply_task_transition。
    let ctx = system_context(&mut tx, project_id)
        .await
        .map_err(|e| RunDispatchError::Transition(e.to_string()))?;

    // ④ 回收分支：候选原本是 `claimed` 且租约已过期。
    if run_state == run_status::CLAIMED {
        if executor_session.is_none() {
            // B3-1：执行器尚未启动 → 恢复**同一** Run（重置租约回队列），不新建、不记失败。
            // 任务仍在 claimed：把它退回 ready 以便重新领取（同一 Run）。
            if task_state == "claimed" {
                apply_transition(&mut tx, task_id, TaskStatus::Claimed, TaskStatus::Ready, &ctx).await?;
                task_state = "ready".to_string();
            }
            requeue_for_redispatch(&mut tx, run_id).await?;
            audit_run(
                &mut tx,
                project_id,
                "reclaim_task_run",
                run_id,
                serde_json::json!({ "reason": "lease_expired_unstarted", "run_id": run_id }),
            )
            .await?;
            // Model B 到此为止（下一轮再领取）；Model A 在同一事务内继续完成领取。
            if opts.task_filter.is_none() {
                tx.commit().await?;
                return Ok(Some(ClaimOutcome {
                    run_id,
                    task_id,
                    kind: ClaimKind::Claimed,
                    lease_token: None,
                }));
            }
        } else {
            // B3-2：执行器已启动且失联 → 隔离旧实例、记录终止结果，再新建 Run。
            isolate_started_run(&mut tx, run_id, cfg).await?;
            let superseded_run_id = run_id;
            // 任务先退回 ready，统一走「正常领取」路径重建。
            if task_state == "claimed" || task_state == "running" {
                let from = TaskStatus::from_str(&task_state).unwrap_or(TaskStatus::Claimed);
                apply_transition(&mut tx, task_id, from, TaskStatus::Ready, &ctx).await?;
                task_state = "ready".to_string();
            }
            let created = insert_task_run(&mut tx, project_id, task_id, None).await?;
            sqlx::query("UPDATE delivery.task_run SET supersedes_run_id = $2 WHERE id = $1")
                .bind(created.run_id)
                .bind(superseded_run_id)
                .execute(&mut *tx)
                .await?;
            audit_run(
                &mut tx,
                project_id,
                "reclaim_task_run",
                superseded_run_id,
                serde_json::json!({
                    "reason": "lease_expired_started",
                    "new_run_id": created.run_id,
                    "attempt_no": created.attempt_no,
                }),
            )
            .await?;
            if opts.task_filter.is_none() {
                tx.commit().await?;
                return Ok(Some(ClaimOutcome {
                    run_id: created.run_id,
                    task_id,
                    kind: ClaimKind::Reclaimed,
                    lease_token: None,
                }));
            }
            run_id = created.run_id;
        }
    }

    // ⑤ 正常领取：任务必须严格为 ready。R4：调度关闭时不领取新 Run
    // （回收分支已在上方执行，不受开关影响）。Model A 恒不受开关门控（A-1）。
    if opts.gate_by_dispatch_mode && !opts.mode.claims() {
        tx.commit().await?;
        return Ok(None);
    }
    if task_state != "ready" {
        // Model A：显式请求遇到非 ready 任务是前置条件失败，**不得**把 Run
        // 推入退避（那会改变无人值守队列的节奏）。
        if opts.task_filter.is_some() {
            tx.rollback().await?;
            return Err(RunDispatchError::TaskNotReady(task_state));
        }
        requeue_with_backoff(&mut tx, run_id, dispatch_attempts, cfg, "task_not_ready").await?;
        audit_run(
            &mut tx,
            project_id,
            "skip_task_run",
            run_id,
            serde_json::json!({ "release_reason": "task_not_ready", "dispatch_attempts": dispatch_attempts + 1 }),
        )
        .await?;
        tx.commit().await?;
        return Ok(Some(ClaimOutcome {
            run_id,
            task_id,
            kind: ClaimKind::Skipped,
            lease_token: None,
        }));
    }

    // ⑤½ 依赖判定（T-024 / 设计稿 §5.2）：前置未满足时 Run 退避回队列——
    // **不**判失败、不改任务状态、不写 `task_run_failure`，它只是「此刻还不能
    // 执行」。位置在候选锁定之后、租约签发与 fencing 之前；Model A/B 共用同一
    // 套判定，不复制逻辑。
    if !crate::task_dependency::dependencies_satisfied(&mut tx, project_id, task_id).await? {
        if opts.task_filter.is_some() {
            tx.rollback().await?;
            return Err(RunDispatchError::TaskNotReady("dependency_unmet".into()));
        }
        requeue_with_backoff(&mut tx, run_id, dispatch_attempts, cfg, "dependency_unmet").await?;
        audit_run(
            &mut tx,
            project_id,
            "skip_task_run",
            run_id,
            serde_json::json!({
                "release_reason": "dependency_unmet",
                "dispatch_attempts": dispatch_attempts + 1,
            }),
        )
        .await?;
        tx.commit().await?;
        return Ok(Some(ClaimOutcome {
            run_id,
            task_id,
            kind: ClaimKind::Skipped,
            lease_token: None,
        }));
    }

    // ⑥ 执行策略门（G5）：读取失败不得默认放行，且拒绝路径审计要留下失败快照。
    match policy.execution_enabled() {
        Ok(true) => {}
        Ok(false) => {
            requeue_with_backoff(&mut tx, run_id, dispatch_attempts, cfg, "policy_disabled").await?;
            audit_run(
                &mut tx,
                project_id,
                "skip_task_run",
                run_id,
                serde_json::json!({
                    "release_reason": "policy_disabled",
                    "policy_snapshot": { "execution_enabled": false },
                    "dispatch_attempts": dispatch_attempts + 1,
                }),
            )
            .await?;
            tx.commit().await?;
            return Ok(Some(ClaimOutcome {
                run_id,
                task_id,
                kind: ClaimKind::Skipped,
                lease_token: None,
            }));
        }
        Err(error) => {
            requeue_with_backoff(&mut tx, run_id, dispatch_attempts, cfg, "policy_unavailable")
                .await?;
            audit_run(
                &mut tx,
                project_id,
                "skip_task_run",
                run_id,
                serde_json::json!({
                    "release_reason": "policy_unavailable",
                    "policy_snapshot": { "execution_enabled": false, "error": error },
                    "dispatch_attempts": dispatch_attempts + 1,
                }),
            )
            .await?;
            tx.commit().await?;
            return Ok(Some(ClaimOutcome {
                run_id,
                task_id,
                kind: ClaimKind::Skipped,
                lease_token: None,
            }));
        }
    }

    // ⑦ 可领取性判定（全部通过才领取）。
    let assessment =
        claimability_assessment(&mut tx, project_id, task_id, retry_exhausted, task_type.as_deref())
            .await?;
    if let Some(reason) = assessment.deny {
        if opts.task_filter.is_some() {
            tx.rollback().await?;
            return Err(RunDispatchError::TaskNotReady(format!("denied: {reason}")));
        }
        requeue_with_backoff(&mut tx, run_id, dispatch_attempts, cfg, reason).await?;
        audit_run(
            &mut tx,
            project_id,
            "skip_task_run",
            run_id,
            serde_json::json!({ "release_reason": reason, "dispatch_attempts": dispatch_attempts + 1 }),
        )
        .await?;
        tx.commit().await?;
        return Ok(Some(ClaimOutcome {
            run_id,
            task_id,
            kind: ClaimKind::Skipped,
            lease_token: None,
        }));
    }

    // ⑧ 状态推进（R1：经统一守卫 validate_transition，与 Run 更新同事务）。
    apply_transition(&mut tx, task_id, TaskStatus::Ready, TaskStatus::Claimed, &ctx).await?;

    // ⑨ 租约签发（D 裁决）：明文只返回给 Model A；库内只存 sha256 小写 hex。
    //    Model B 不签发 → 写入 NULL（覆盖过期旧 Run 可能残留的 hash，验收 C17）。
    let (lease_token, lease_owner) = match opts.lease_owner {
        LeaseOwner::Worker(worker) => (None, worker.to_string()),
        LeaseOwner::Mcp { client_name } => {
            let token = crate::run_context::generate_lease_token()?;
            let prefix = token[..8].to_string();
            (Some(token), format!("mcp:{client_name}:{prefix}"))
        }
    };
    let lease_token_hash = lease_token
        .as_deref()
        .map(crate::run_context::hash_lease_token);

    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'claimed', stage = 'dispatch', claimed_at = now(), \
                lease_owner = $2, heartbeat_at = now(), \
                lease_expires_at = now() + make_interval(secs => $3::float8), \
                lease_token_hash = $4, client_name = $5, client_version = $6, \
                timeout_seconds = $7 \
          WHERE id = $1",
    )
    .bind(run_id)
    .bind(&lease_owner)
    .bind(cfg.lease_secs)
    .bind(&lease_token_hash)
    .bind(opts.client_name)
    .bind(opts.client_version)
    .bind(cfg.timeout_secs)
    .execute(&mut *tx)
    .await?;

    // R-F12：`git_branch` 是平台生成的普通字段（不属状态机），幂等回填即可。
    let git_branch = backfill_git_branch(&mut tx, task_id).await?;

    let baselines = snapshot_repositories(&mut tx, run_id, project_id, task_id).await?;

    // D2.1：完整策略快照（§15.5）。`policy_hash`/`policy_version` 为内容寻址，
    // `execution_enabled` 由策略门与审批证据推导，不再是字面量。
    let policy_snapshot = serde_json::json!({
        "policy_schema": POLICY_SCHEMA,
        "policy_version": assessment.policy_version,
        "policy_hash": assessment.policy_hash,
        "captured_at": chrono::Utc::now().to_rfc3339(),
        "approval_required": assessment.approval_required,
        "approval_status": assessment.approval_status,
        "execution_enabled": assessment.execution_enabled,
        "repository_baselines": baselines.clone(),
        "snapshot_scope": POLICY_SCHEMA,
        "effective_policy": assessment.effective_policy.to_snapshot_json(),
    });
    sqlx::query("UPDATE delivery.task_run SET policy_snapshot = $2 WHERE id = $1")
        .bind(run_id)
        .bind(&policy_snapshot)
        .execute(&mut *tx)
        .await?;

    // Probe 模式：立即释放回 queued（不启动执行器）。
    if opts.mode == DispatchMode::Probe {
        requeue_for_redispatch(&mut tx, run_id).await?;
        sqlx::query("UPDATE delivery.task_run SET release_reason = 'probe_release' WHERE id = $1")
            .bind(run_id)
            .execute(&mut *tx)
            .await?;
    }

    audit_run(
        &mut tx,
        project_id,
        "claim_task_run",
        run_id,
        serde_json::json!({
            "task_id": task_id,
            "lease_owner": lease_owner,
            "lease_secs": cfg.lease_secs,
            "repositories": baselines,
            "git_branch": git_branch,
            "mode": format!("{:?}", opts.mode),
            "model": if opts.issue_token { "mcp" } else { "worker" },
            "client_name": opts.client_name,
            "client_version": opts.client_version,
        }),
    )
    .await?;

    tx.commit().await?;
    Ok(Some(ClaimOutcome {
        run_id,
        task_id,
        kind: ClaimKind::Claimed,
        lease_token,
    }))
}


/// 平台生成的任务分支名（C1）：格式 `task/<task_id 前 8 位>`，幂等回填。
/// 已存在（含人工指定）时保持原值，执行端不得自定分支名。
pub(crate) async fn backfill_git_branch(
    tx: &mut Transaction<'_, Postgres>,
    task_id: Uuid,
) -> Result<String, sqlx::Error> {
    let short = &task_id.to_string()[..8];
    let branch = format!("task/{short}");
    sqlx::query("UPDATE delivery.task SET git_branch = $2 WHERE id = $1 AND git_branch IS NULL")
        .bind(task_id)
        .bind(&branch)
        .execute(&mut **tx)
        .await?;
    sqlx::query_scalar("SELECT git_branch FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(&mut **tx)
        .await
}


