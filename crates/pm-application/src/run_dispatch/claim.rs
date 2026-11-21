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
    /// T-032（ADR-0002 L1/B1）：Model A / Model B **统一签发**的一次性租约明文；
    /// 仅未真正领取的分支为 `None`。worker 拿到后注入执行端环境（L2）。
    pub lease_token: Option<String>,
    /// T-032 T2：本次执行权世代（首次领取=1，重领递增）；未领取分支为 0。
    pub lease_generation: i32,
    /// V-05-1：lease_owner 快照（client:{device_id} / mcp:{name}:{token8} /
    /// worker id）——ClaimedTaskRun 透传给执行端作命令路由身份。
    pub lease_owner: String,
}

/// 租约持有者（A 裁决）：Model B 是 worker 标识，Model A 是 MCP 客户端。
///
/// **`lease_owner` 不是凭证**：它只用于展示与 fencing 的辅助归因，真正的凭证
/// 是 `lease_token`（见 `docs/slice-d/ai-pm-slice-d2-mcp-context-v1.md` §三 A 的关键推论）。
#[derive(Debug, Clone, Copy)]
pub(crate) enum LeaseOwner<'a> {
    Worker(&'a str),
    Mcp {
        client_name: &'a str,
    },
    /// V-05-1 Client Registry 凭据通道（0049）：稳定身份 = device.id；
    /// lease_owner = `client:{device_id}`（跨重启稳定，R-11 token8 债消除）。
    Client {
        device_id: Uuid,
    },
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

    client_name: Option<&'a str>,
    client_version: Option<&'a str>,

    /// T-088：认证调用人的完整审计身份（MCP 路径）；worker 路径为 None。
    /// `claimed_device_id` 同时落 `task_run.executor_id`（同源同值，仅归因）。
    audit_actor: Option<&'a crate::actor::AuditActor>,
}

/// Model A（执行端显式领取）成功后的返回：一次领取即可拿到的全部凭据。
#[derive(Debug, Clone)]
pub struct ClaimedTaskRun {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub attempt_no: i32,
    /// 32 字节随机数的小写 hex（64 字符），**只返回这一次**。
    pub lease_token: String,
    /// T-032 T2：本次执行权世代（MCP 请求必须携带，服务端双验证）。
    pub lease_generation: i32,
    pub lease_expires_at: chrono::DateTime<chrono::Utc>,
    pub git_branch: String,
    pub stage: String,
    /// V-05-1：lease_owner 快照（Client 通道 = `client:{device_id}`）——
    /// 命令路由身份；执行端须原样用于 poll/ack（不得本地重建）。
    pub lease_owner: String,
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

            client_name: None,
            client_version: None,
            audit_actor: None,
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
        None,
    )
    .await
}

/// 同 [`claim_task_run`]，但由调用方**显式**给出完整审计身份（T-088 MCP 路径）：
/// 审计行写真人三列 + 设备归因，`task_run.executor_id` 落已校验的设备 id。
/// V-05-1 凭据通道领取（0049）：`lease_owner = client:{device_id}` +
/// `lease_client_id = device_id` + execution_mode='spawn'。稳定身份使命令
/// 路由跨执行端重启可用（R-11 token8 债消除；ACP-462）。
#[allow(clippy::too_many_arguments)]
pub async fn claim_task_run_as_client(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    device_id: Uuid,
    client_name: &str,
    client_version: Option<&str>,
    cfg: &RunDispatchConfig,
    actor: &crate::actor::AuditActor,
) -> Result<ClaimedTaskRun, RunDispatchError> {
    let outcome = claim_core(
        pool,
        cfg,
        &ClaimOptions {
            lease_owner: LeaseOwner::Client { device_id },
            project_filter: Some(project_id),
            task_filter: Some(task_id),
            gate_by_dispatch_mode: false,
            mode: crate::run_dispatch::DispatchMode::On,
            client_name: Some(client_name),
            client_version,
            audit_actor: Some(actor),
        },
        &DefaultExecutionPolicy,
    )
    .await?;
    let outcome = outcome.ok_or(RunDispatchError::NoRunnableRun)?;
    finalize_claimed_run(pool, project_id, outcome).await
}

pub async fn claim_task_run_as(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    client_name: &str,
    client_version: Option<&str>,
    cfg: &RunDispatchConfig,
    actor: &crate::actor::AuditActor,
) -> Result<ClaimedTaskRun, RunDispatchError> {
    claim_task_run_with_policy(
        pool,
        project_id,
        task_id,
        client_name,
        client_version,
        cfg,
        &DefaultExecutionPolicy,
        Some(actor),
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
    audit_actor: Option<&crate::actor::AuditActor>,
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
            client_name: Some(client_name),
            client_version,
            audit_actor,
        },
        policy,
    )
    .await?;
    let outcome = outcome.ok_or(RunDispatchError::NoRunnableRun)?;
    finalize_claimed_run(pool, project_id, outcome).await
}

/// Model A 领取收尾（MCP / Client 双通道共用）：回读 Run/Task 事实 + 组装
/// 执行端凭据包（含 V-05-1 lease_owner 快照）。
async fn finalize_claimed_run(
    pool: &PgPool,
    project_id: Uuid,
    outcome: ClaimOutcome,
) -> Result<ClaimedTaskRun, RunDispatchError> {
    let lease_token = outcome.lease_token.ok_or(RunDispatchError::NoRunnableRun)?;
    let (attempt_no, lease_expires_at, stage, git_branch, lease_generation): (
        i32,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<String>,
        Option<String>,
        i32,
    ) = sqlx::query_as(
        "SELECT r.attempt_no, r.lease_expires_at, r.stage, t.git_branch, r.lease_generation \
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
        lease_generation,
        lease_expires_at: lease_expires_at
            .ok_or_else(|| RunDispatchError::InvalidOption("lease_expires_at missing".into()))?,
        git_branch: git_branch
            .ok_or_else(|| RunDispatchError::InvalidOption("git_branch missing".into()))?,
        stage: stage.unwrap_or_else(|| run_stage::DISPATCH.to_string()),
        // V-05-1：lease_owner 快照随响应下发——执行端命令路由身份以此为准。
        lease_owner: outcome.lease_owner,
    })
}

/// A/B 两入口的**唯一事务模板**（A-2）：候选筛选、租约签发、开关门控与审计
/// payload 都由 [`ClaimOptions`] 表达，禁止复制事务体。
///
/// **I1-C 两阶段协议**（I1 §5.2，验收 I1-010~014）：
///
/// - **阶段 A（候选发现）**：无锁读出候选 Run 的 `project_id` 集合——不锁 Run，
///   不代表获得 claim 权限；
/// - **阶段 B（按统一锁序 claim）**：对候选 Project 依次
///   `锁 core.project → 查 archived → 在该 Project 内重新查候选 Run
///   （FOR UPDATE OF r SKIP LOCKED）→ 锁 Task → 重新验证 → claim`。
///
/// 规则（§5.2/§5.4）：
/// 1. 阶段 B 的候选 Run **必须重新查询**，不得直接使用阶段 A 的结果完成 claim；
/// 2. Project 已归档 → 跳过该 Project，继续找下一个候选（不失败、不报错）；
/// 3. 所有 claim 分支**先锁 Project 再锁 Run 再锁 Task**，无「先锁 Run 后锁
///    Project」的分支（锁序 `project → run → task`，无反向环）；
/// 4. 阶段 A 不持有跨事务锁（纯读，连接释放后无残留）；
/// 5. Model A（`task_filter` 指定任务）锁序不变：调用方已给出唯一 Project，
///    阶段 A/B 合并为一次「锁 Project → 锁 Run → 锁 Task」。
pub(crate) async fn claim_core(
    pool: &PgPool,
    cfg: &RunDispatchConfig,
    opts: &ClaimOptions<'_>,
    policy: &dyn ExecutionPolicy,
) -> Result<Option<ClaimOutcome>, RunDispatchError> {
    // Model A：范围已确定，单 Project 内直接走阶段 B。
    if let Some(task_id) = opts.task_filter {
        let project_id = opts.project_filter.ok_or_else(|| {
            RunDispatchError::InvalidOption("Model A requires a project scope".into())
        })?;
        return claim_core_in_project(pool, cfg, opts, policy, project_id, Some(task_id)).await;
    }

    // ── 阶段 A：只读发现候选 Project（不锁 Run，不代表获得 claim 权限）──
    // D-1=b：命中 `AI_PM_RUN_DISPATCH_EXCLUDE_PROJECTS` 的项目在候选发现阶段被
    // 静默排除——不领、不审计、不推退避（Model A 显式领取不经此路径，不受限）。
    // D-2=c：领取型项目（active device_project_binding → device.is_runner）恒被
    // worker 静默排除（静态事实判定，与心跳新鲜度无关——语义定版见实施基线 §三）；
    // 与 env 排除列表并集生效（belt-and-suspenders）。排除列表为空时仍带 runner
    // 排除，非领取型项目零行为漂移。
    let candidates: Vec<Uuid> = if cfg.exclude_projects.is_empty() {
        sqlx::query_scalar(
            "SELECT r.project_id FROM delivery.task_run r \
              WHERE r.run_kind = 'delivery' \
                AND ((r.status = 'queued'  AND r.next_dispatch_at <= now()) \
                  OR (r.status = 'claimed' AND r.lease_expires_at < now())) \
                AND NOT EXISTS ( \
                  SELECT 1 FROM devenv.device_project_binding b \
                   JOIN devenv.device d ON d.id = b.device_id AND d.is_runner \
                  WHERE b.project_id = r.project_id AND b.unbound_at IS NULL) \
              ORDER BY r.next_dispatch_at, r.created_at \
              LIMIT $1",
        )
        .bind(CLAIM_PROJECT_CANDIDATE_LIMIT)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_scalar(
            "SELECT r.project_id FROM delivery.task_run r \
              WHERE r.run_kind = 'delivery' \
                AND ((r.status = 'queued'  AND r.next_dispatch_at <= now()) \
                  OR (r.status = 'claimed' AND r.lease_expires_at < now())) \
                AND NOT EXISTS ( \
                  SELECT 1 FROM core.project p \
                   WHERE p.id = r.project_id AND p.code = ANY($2::text[])) \
                AND NOT EXISTS ( \
                  SELECT 1 FROM devenv.device_project_binding b \
                   JOIN devenv.device d ON d.id = b.device_id AND d.is_runner \
                  WHERE b.project_id = r.project_id AND b.unbound_at IS NULL) \
              ORDER BY r.next_dispatch_at, r.created_at \
              LIMIT $1",
        )
        .bind(CLAIM_PROJECT_CANDIDATE_LIMIT)
        .bind(&cfg.exclude_projects)
        .fetch_all(pool)
        .await?
    };

    // ── 阶段 B：按候选顺序逐 Project 尝试（§5.4：候选被抢占 → 尝试下一个；
    //    归档 → 跳过该 Project；候选遍历 LIMIT 界定单轮工作量，饥饿由
    //    「下一轮 Worker 再扫」兜底）。
    for project_id in candidates {
        if let Some(outcome) =
            claim_core_in_project(pool, cfg, opts, policy, project_id, None).await?
        {
            return Ok(Some(outcome));
        }
    }
    Ok(None)
}

/// 单轮候选发现的 Project 数上限（阶段 A → B 的扫描窗口；
/// 其余候选由 Worker 下一轮 claim 兜底，避免长事务）。
const CLAIM_PROJECT_CANDIDATE_LIMIT: i64 = 32;

/// 阶段 B：在**单个 Project** 内按统一锁序执行 claim。
///
/// 返回 `Ok(None)` 表示「本 Project 本轮无可领取的 Run」——包括：候选被其他
/// Worker 抢占（SKIP LOCKED 跳过）、Project 已归档、Task 终态等合法跳过情形。
async fn claim_core_in_project(
    pool: &PgPool,
    cfg: &RunDispatchConfig,
    opts: &ClaimOptions<'_>,
    policy: &dyn ExecutionPolicy,
    project_id: Uuid,
    task_filter: Option<Uuid>,
) -> Result<Option<ClaimOutcome>, RunDispatchError> {
    let mut tx = pool.begin().await?;

    // ① 锁 Project 行（统一锁序第一把锁：project → run → task）。
    //    归档事务同锁序 → 归档与 claim 线性化：归档先提交 → 这里读到 archived
    //    合法跳过；claim 先提交 → 归档随后把 queued 条件取消（I1-010/012/013）。
    let project_status: Option<String> =
        sqlx::query_scalar("SELECT status FROM core.project WHERE id = $1 FOR UPDATE")
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?;
    // 仅 closed 跳过（planning/initializing 是导入流水线过程态，放行）。
    if project_status.as_deref() == Some("closed") {
        tx.commit().await?;
        return Ok(None);
    }
    let Some(_) = project_status else {
        // Project 不存在（历史脏数据）：候选作废。
        tx.rollback().await?;
        return Ok(None);
    };

    // ② 在该 Project 范围内**重新查询**候选 Run（§5.2 规则 1：不得直接使用
    //    阶段 A 的结果）。FOR UPDATE OF r SKIP LOCKED：候选被其他 Worker 抢占
    //    → 该行被跳过，本 Project 本轮出局（§5.4）。
    let candidate = if let Some(task_id) = task_filter {
        sqlx::query(
            "SELECT r.id, r.task_id, r.project_id, r.status, r.dispatch_attempts, \
                    r.executor_session, r.stage, r.lease_generation, r.attempt_no, \
                    r.scheduling_shield \
               FROM delivery.task_run r \
              WHERE r.task_id = $1 AND r.project_id = $2 AND r.run_kind = 'delivery' \
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
                    r.executor_session, r.stage, r.lease_generation, r.attempt_no, \
                    r.scheduling_shield \
               FROM delivery.task_run r \
              WHERE r.project_id = $1 AND r.run_kind = 'delivery' \
                AND ((r.status = 'queued'  AND r.next_dispatch_at <= now()) \
                  OR (r.status = 'claimed' AND r.lease_expires_at < now())) \
              ORDER BY r.next_dispatch_at, r.created_at \
              FOR UPDATE OF r SKIP LOCKED \
              LIMIT 1",
        )
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?
    };
    let Some(candidate) = candidate else {
        if let Some(task_id) = opts.task_filter {
            // Model A：区分「任务不存在」「已有活跃 Run」「等 TaskReady」三种前置失败。
            let project_id = opts.project_filter.ok_or_else(|| {
                RunDispatchError::InvalidOption("Model A requires a project scope".into())
            })?;
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
                  WHERE task_id = $1 AND ",
                "run_kind = 'delivery' AND status IN (",
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
    let candidate_stage: Option<String> = candidate.get("stage");
    let _lease_generation_prev: i32 = candidate.get("lease_generation");
    let candidate_shield: String = candidate.get("scheduling_shield");

    // I1-C：归档守卫已在阶段 B 的 Project 行锁内完成（见 claim_core_in_project ①）——
    // 不再需要池外防御纵深读；锁内读到的必然是归档后的已提交状态。

    // ③ 锁任务行（顺序固定：project → run → task）。B2：新 Run 的 INSERT 必须在此之后。
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
            lease_generation: 0,
            lease_owner: String::new(),
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
            lease_generation: 0,
            lease_owner: String::new(),
        }));
    }

    // R1：构造系统执行身份（审计 actor 必须存在）；所有 Task 状态变更经统一
    // 受守卫服务 apply_task_transition。
    let ctx = system_context(&mut tx, project_id)
        .await
        .map_err(|e| RunDispatchError::Transition(e.to_string()))?;

    // ④ 回收分支：候选原本是 `claimed` 且租约已过期。
    if run_state == run_status::CLAIMED {
        // N-1 调度屏蔽（S2，0046 scheduling_shield）：人工等待/回复待投递/超时
        // 未处置的 Run **拒绝重领**——不 requeue（下一轮还会看到它）、不改任何
        // 状态，等待结束后自然恢复既有回收语义（ACP-428：不得启动未携带人工
        // 回复的新进程）。设计稿 V1.1 §4.2；CANCELLED 介入的恢复走正常重领。
        if candidate_shield != "none" {
            if opts.task_filter.is_some() {
                tx.rollback().await?;
                return Err(RunDispatchError::TaskNotReady(
                    "awaiting_interaction".into(),
                ));
            }
            audit_run(
                &mut tx,
                project_id,
                "skip_task_run",
                run_id,
                serde_json::json!({
                    "release_reason": "scheduling_shield",
                    "scheduling_shield": candidate_shield,
                }),
            )
            .await?;
            tx.commit().await?;
            return Ok(Some(ClaimOutcome {
                run_id,
                task_id,
                kind: ClaimKind::Skipped,
                lease_token: None,
                lease_generation: 0,
                lease_owner: String::new(),
            }));
        }
        // T-032 T1：starting 判定 = **启动意图未确认**。`executor_session` 是启动
        // 意图（spawn 前写入）；`stage='dispatch'` 表示进程身份尚未确认（spawn
        // 失败残留 / spawn 后未落库的崩溃窗口）。二者任一成立 → B3-1 同 Run 重领
        // （session 由下一轮 claim_and_start 生成新值覆盖，覆盖前留事件）；
        // 仅 stage='started'（进程身份已确认）才走 B3-2 隔离。
        let unstarted =
            executor_session.is_none() || candidate_stage.as_deref() == Some(run_stage::DISPATCH);
        if unstarted {
            // B3-1：执行器尚未启动 → 恢复**同一** Run（重置租约回队列），不新建、不记失败。
            // 任务仍在 claimed：把它退回 ready 以便重新领取（同一 Run）。
            if task_state == "claimed" {
                apply_transition(
                    &mut tx,
                    task_id,
                    TaskStatus::Claimed,
                    TaskStatus::Ready,
                    &ctx,
                )
                .await?;
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
                    lease_generation: 0,
                    lease_owner: String::new(),
                }));
            }
        } else {
            // B3-2：执行器已启动且失联 → 隔离旧实例、记录终止结果，再新建 Run。
            let superseded_run_id = run_id;
            // 任务先退回 ready，统一走「正常领取」路径重建。
            if task_state == "claimed" || task_state == "running" {
                let from = TaskStatus::from_str(&task_state).unwrap_or(TaskStatus::Claimed);
                apply_transition(&mut tx, task_id, from, TaskStatus::Ready, &ctx).await?;
                task_state = "ready".to_string();
            }
            let created =
                recover_lost_run(&mut tx, superseded_run_id, task_id, project_id, cfg).await?;
            if opts.task_filter.is_none() {
                tx.commit().await?;
                return Ok(Some(ClaimOutcome {
                    run_id: created.run_id,
                    task_id,
                    kind: ClaimKind::Reclaimed,
                    lease_token: None,
                    lease_generation: 0,
                    lease_owner: String::new(),
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
            lease_generation: 0,
            lease_owner: String::new(),
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
            lease_generation: 0,
            lease_owner: String::new(),
        }));
    }

    // ⑤¾ Product 执行门禁（PA07-IMP-004，PA-07 §5.1 门禁 2）：任一有效关联
    // Product 处于 suspended/archived 时该工作范围不得执行；FOR UPDATE 与
    // lifecycle 写路径串行化（暂停 vs 领取竞态由行锁解决，§16）。deny 处理
    // 与依赖门禁同款：退避回队、不判失败、不写 failure。
    if let Some((blocked_product, deny_reason)) =
        crate::task_products::product_gate_denied(&mut tx, task_id).await?
    {
        if opts.task_filter.is_some() {
            tx.rollback().await?;
            return Err(RunDispatchError::TaskNotReady(deny_reason.into()));
        }
        requeue_with_backoff(&mut tx, run_id, dispatch_attempts, cfg, deny_reason).await?;
        audit_run(
            &mut tx,
            project_id,
            "skip_task_run",
            run_id,
            serde_json::json!({
                "release_reason": deny_reason,
                "blocked_product_id": blocked_product,
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
            lease_generation: 0,
            lease_owner: String::new(),
        }));
    }

    // ⑥ 执行策略门（G5）：读取失败不得默认放行，且拒绝路径审计要留下失败快照。
    match policy.execution_enabled() {
        Ok(true) => {}
        Ok(false) => {
            requeue_with_backoff(&mut tx, run_id, dispatch_attempts, cfg, "policy_disabled")
                .await?;
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
                lease_generation: 0,
                lease_owner: String::new(),
            }));
        }
        Err(error) => {
            requeue_with_backoff(
                &mut tx,
                run_id,
                dispatch_attempts,
                cfg,
                "policy_unavailable",
            )
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
                lease_generation: 0,
                lease_owner: String::new(),
            }));
        }
    }

    // ⑦ 可领取性判定（全部通过才领取）。
    let assessment = claimability_assessment(
        &mut tx,
        project_id,
        task_id,
        retry_exhausted,
        task_type.as_deref(),
    )
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
            lease_generation: 0,
            lease_owner: String::new(),
        }));
    }

    // ⑧ 状态推进（R1：经统一守卫 validate_transition，与 Run 更新同事务）。
    apply_transition(
        &mut tx,
        task_id,
        TaskStatus::Ready,
        TaskStatus::Claimed,
        &ctx,
    )
    .await?;

    // ⑨ 租约签发（D 裁决 + T-032 终裁）：Model A / Model B **统一签发**明文
    //    token（ADR-0002 L1：执行端 MCP 认证凭据；worker 注入执行端环境 L2），
    //    库内只存 sha256 小写 hex。generation 每次「成功获取执行权」递增
    //    （首次=1，重领+1，心跳不递增——T2）；deadline_at 仅首次固化
    //    （COALESCE），重领不得重置或延长（T3）。
    let (lease_token, lease_owner) = match opts.lease_owner {
        LeaseOwner::Worker(worker) => {
            let token = crate::run_context::generate_lease_token()?;
            (Some(token), worker.to_string())
        }
        LeaseOwner::Mcp { client_name } => {
            let token = crate::run_context::generate_lease_token()?;
            let prefix = token[..8].to_string();
            (Some(token), format!("mcp:{client_name}:{prefix}"))
        }
        LeaseOwner::Client { device_id } => {
            let token = crate::run_context::generate_lease_token()?;
            (Some(token), format!("client:{device_id}"))
        }
    };
    let lease_token_hash = lease_token
        .as_deref()
        .map(crate::run_context::hash_lease_token);

    // T-059 / ADR-0002 L5：执行形态由领取通道显式表达（替代 lease_owner 前缀推导）。
    let execution_mode = match &opts.lease_owner {
        LeaseOwner::Worker(_) => "spawn",
        // ACP Runner（V0.5 WP-3 / V1.1 §0 载体双形态）：client_name 携保留前缀
        // `runner:` 领取 = 客户端**进程载体**（Model B spawn 形态），interaction /
        // 命令查询面 / Continuation 通道对其适用；四工具契约不动，前缀是 V-05-1
        // Client Registry（Client 身份 + 能力三分验证）落地前的过渡形态标记。
        // 其余 MCP 领取 = Agent 自执行（Model A human；ACP-425 退化路径不受影响）。
        LeaseOwner::Mcp { client_name } if client_name.starts_with("runner:") => "spawn",
        LeaseOwner::Mcp { .. } => "human",
        // V-05-1 凭据通道 = Runner 进程载体（spawn 形态；不再依赖 runner: 前缀）。
        LeaseOwner::Client { .. } => "spawn",
    };

    let lease_generation: i32 = sqlx::query_scalar(
        "UPDATE delivery.task_run \
            SET status = 'claimed', stage = 'dispatch', claimed_at = now(), \
                lease_owner = $2, heartbeat_at = now(), \
                lease_expires_at = now() + make_interval(secs => $3::float8), \
                lease_token_hash = $4, client_name = $5, client_version = $6, \
                timeout_seconds = $7, \
                lease_generation = COALESCE(lease_generation, 0) + 1, \
                deadline_at = COALESCE(deadline_at, \
                    now() + make_interval(secs => COALESCE($7::int, $8::int)::float8)), \
                executor_id = COALESCE($9, executor_id), \
                execution_mode = $10,\
                lease_client_id = $11 \
          WHERE id = $1 \
         RETURNING lease_generation",
    )
    .bind(run_id)
    .bind(&lease_owner)
    .bind(cfg.lease_secs)
    .bind(&lease_token_hash)
    .bind(opts.client_name)
    .bind(opts.client_version)
    .bind(cfg.timeout_secs)
    .bind(cfg.deadline_default_secs as i32)
    // T-088：已校验的设备归因（ADR-0001 D6 无 FK 前向列；自报不作凭证）。
    .bind(opts.audit_actor.and_then(|a| a.claimed_device_id))
    .bind(execution_mode)
    // V-05-1：凭据通道写稳定 Client 身份（约束 B：复合外键
    // fk_task_run_client_tenant 在库层兜底租户一致）。
    .bind(match opts.lease_owner {
        LeaseOwner::Client { device_id } => Some(device_id),
        _ => None,
    })
    .fetch_one(&mut *tx)
    .await?;

    // R-F12：`git_branch` 是平台生成的普通字段（不属状态机），幂等回填即可。
    let git_branch = backfill_git_branch(&mut tx, task_id).await?;

    let baselines = snapshot_repositories(&mut tx, run_id, project_id, task_id).await?;

    // D2.1：完整策略快照（§15.5）。`policy_hash`/`policy_version` 为内容寻址，
    // `execution_enabled` 由策略门与审批证据推导，不再是字面量。
    // I3-C/D（V1.1 简化裁决）：每仓**可访问**快照（Task 范围 ∩ 仓库状态 verified，
    // 单一 `read_write` 档位）叠加进快照；其内容寻址哈希落 `policy_snapshot_hash`。
    let run_tenant: Uuid =
        sqlx::query_scalar("SELECT tenant_id FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&mut *tx)
            .await?;
    let repo_access =
        crate::repo_access::build_task_repo_access_tx(&mut tx, run_tenant, project_id, task_id)
            .await
            .map_err(|e| match e {
                crate::repo_access::RepoAccessError::Db(db) => {
                    RunDispatchError::Db(sqlx::Error::Protocol(db))
                }
                other => RunDispatchError::InvalidOption(other.to_string()),
            })?;
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
        "repo_access": crate::repo_access::access_snapshot_json(&repo_access),
    });
    let policy_snapshot_hash = crate::repo_access::access_snapshot_hash(&repo_access);
    sqlx::query(
        "UPDATE delivery.task_run \
         SET policy_snapshot = $2, policy_snapshot_hash = $3, \
             repository_policy_snapshots = $4 \
         WHERE id = $1",
    )
    .bind(run_id)
    .bind(&policy_snapshot)
    .bind(&policy_snapshot_hash)
    .bind(crate::repo_access::access_snapshot_json(&repo_access))
    .execute(&mut *tx)
    .await?;

    // PA07-IMP-004：门禁证据快照（审查81）——依赖逐边快照（前置 Revision/
    // 条件/交付物引用/结论）+ Product 门禁快照（各关联 Product 执行状态）。
    // 与 policy_snapshot 同事务、只在成功领取路径写（deny/skip 分支早已 return）。
    crate::task_products::write_run_gate_snapshots(&mut tx, project_id, task_id, run_id).await?;

    // Probe 模式：立即释放回 queued（不启动执行器）。
    if opts.mode == DispatchMode::Probe {
        requeue_for_redispatch(&mut tx, run_id).await?;
        sqlx::query("UPDATE delivery.task_run SET release_reason = 'probe_release' WHERE id = $1")
            .bind(run_id)
            .execute(&mut *tx)
            .await?;
    }

    audit_run_opt(
        &mut tx,
        project_id,
        opts.audit_actor,
        "claim_task_run",
        run_id,
        serde_json::json!({
            "task_id": task_id,
            "lease_owner": lease_owner,
            "lease_secs": cfg.lease_secs,
            "lease_generation": lease_generation,
            "repositories": baselines,
            "git_branch": git_branch,
            "mode": format!("{:?}", opts.mode),
            "model": if opts.task_filter.is_some() { "mcp" } else { "worker" },
            "client_name": opts.client_name,
            "client_version": opts.client_version,
        }),
    )
    .await?;

    // S05 监控事件面（0013）：领取时刻落账（同事务；重领 generation+1 自然成新行）。
    crate::run_events::record_run_event(
        &mut tx,
        run_id,
        crate::run_events::RunEvent {
            event_type: crate::run_events::event_type::RUN_CLAIMED,
            generation: Some(lease_generation),
            dedupe_tag: "",
            detail: Some(serde_json::json!({
                "lease_owner": lease_owner,
                "execution_mode": execution_mode,
            })),
        },
    )
    .await?;

    tx.commit().await?;
    Ok(Some(ClaimOutcome {
        run_id,
        task_id,
        kind: ClaimKind::Claimed,
        lease_token,
        lease_generation,
        lease_owner,
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
