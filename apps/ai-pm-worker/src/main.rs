//! `ai-pm-worker`: claims `ai_analysis` jobs and runs them through the
//! end-to-end orchestrator (Task 14). One job at a time, 60ms poll
//! between empty claims.

use std::sync::Arc;

use pm_ai::model::AnalysisModel;
use pm_ai::orchestrator::{HandleOutcome, Orchestrator};
use pm_application::project_import::{import_id_for_event, run_import_job};
use pm_application::{PgCommandExecutor, PolicyEngine};
use pm_infrastructure::job_queue::{self, ClaimedJob};
use pm_infrastructure::outbox_handlers::register_all_handlers;
use pm_infrastructure::outbox_worker::OutboxWorker;
use sqlx::PgPool;
use tracing_subscriber::EnvFilter;

/// Attempts ceiling / backoff base handed to `job_queue::fail`; the same
/// values the orchestrator uses when it fails a job itself.
const MAX_JOB_ATTEMPTS: i32 = 5;
const JOB_BACKOFF_BASE_SECS: i64 = 5;
/// Lease length for a claimed job, in seconds. One hour, matching the
/// Task 10 claim hardening: a real model analysis runs far longer than a
/// minute, and an expired lease would let another worker double-claim a
/// live analysis.
const LEASE_SECS: i64 = 3600;
/// Idle poll interval.
const POLL_INTERVAL_MS: u64 = 60;
/// 第七条循环（Batch G 会话回复）的轮询间隔。LLM 回复以秒计，1s 足够
/// （与主循环 60ms 的「事件驱动」节奏不同：这里慢轮询是对的）。
const CONVERSATION_REPLY_POLL_MS: u64 = 1000;
/// 第八条循环（DK-B 访谈编排）的轮询间隔，同上：LLM 调用以秒计。
const DISCOVERY_ORCHESTRATOR_POLL_MS: u64 = 1000;
/// Default on-disk policy — the red-line single source of truth
/// (global constraint: `policies/ai-pm-ai-lead-v2.yaml`). Relative to the
/// working directory the worker is started from (repo root in dev).
const DEFAULT_POLICY_FILE: &str = "policies/ai-pm-ai-lead-v2.yaml";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("database pool");

    let policy_path =
        std::env::var("AI_PM_POLICY_FILE").unwrap_or_else(|_| DEFAULT_POLICY_FILE.to_string());
    let policy_yaml = std::fs::read_to_string(&policy_path).unwrap_or_else(|e| {
        panic!("policy file `{policy_path}` unreadable: {e}; set AI_PM_POLICY_FILE or start from the repo root")
    });
    let policy = Arc::new(
        PolicyEngine::from_yaml(&policy_yaml)
            .expect("policy yaml must parse (it is the same file the tests pin)"),
    );
    let executor = Arc::new(PgCommandExecutor::new(pool.clone(), policy.clone()));
    // Task 17: the analysis model is chosen per job behind the same
    // `Arc<dyn AnalysisModel>` port. `AI_PM_ANALYSIS_MODEL=auto` (default)
    // resolves the project/tenant LLM provider and — when nothing resolves
    // or resolution fails — falls back to the deterministic fake with a
    // loud structured warning; `fake` pins the fixtures for development.
    let model: Arc<dyn AnalysisModel> = Arc::new(
        pm_ai::analysis_model::RoutedAnalysisModel::from_env(pool.clone()),
    );
    let orchestrator = Orchestrator {
        pool: pool.clone(),
        model,
        policy,
        executor,
    };

    // P2-13/D-1=A（2026-11-08）：outbox 侧仓库快照分析的 LLM 增强模型
    // （KB/PC 两 handler 共享）。逐调用按项目解析 provider；无 provider 或
    // 调用失败回退结构化派生并标 analysis_mode（D-3=A，绝不回退假数据）。
    // `AI_PM_SNAPSHOT_ANALYSIS_MODEL=auto|derived`（默认 auto）。
    let snapshot_model: Arc<dyn pm_application::snapshot_analysis::SnapshotAnalysisModel> =
        Arc::new(pm_ai::snapshot_model::LlmSnapshotAnalysisModel::from_env(
            pool.clone(),
        ));

    let worker_id = format!("ai-pm-worker-{}", uuid::Uuid::new_v4());
    tracing::info!(service = "ai-pm-worker", %worker_id, "started");

    // 第二条循环：冷启动 `repo_import` job，与 `ai_analysis` 互不阻塞。
    let repo_pool = pool.clone();
    let repo_worker_id = worker_id.clone();
    tokio::spawn(async move {
        loop {
            match job_queue::claim(&repo_pool, "repo_import", &repo_worker_id, LEASE_SECS).await {
                Some(job) => handle_repo_job(&repo_pool, &job).await,
                None => {
                    tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await
                }
            }
        }
    });

    // 第三条循环：outbox 事件分发（spec §20）。**统一 Handler 装配**
    // （T-061 §6.2.1）：本 worker 与 `pm-worker` 共享同一张 outbox 表，注册
    // 集合必须一致——否则零 Handler 实例会把可处理事件误判为死信。
    // 装配集合：TaskReady → 创建 queued Run；DeliverySubmitted → D5 拍①；
    // RunCompleted → 记审计 + 生成执行完成通知（记录型）。
    // 其余未注册事件明确死信、可 replay，绝不假装消费。
    let outbox_pool = pool.clone();
    let outbox_worker_id = worker_id.clone();
    tokio::spawn(async move {
        let worker = register_all_handlers(
            OutboxWorker::new(outbox_pool.clone(), outbox_worker_id),
            &outbox_pool,
            Some(snapshot_model),
            // 与第八循环同一个开关：分析完成 → 自动开访谈（默认 off）。
            pm_ai::discovery_orchestrator::DiscoveryOrchestratorMode::from_env().enabled(),
        );
        // 目录一致性校验：不一致即 panic（fail fast）。
        worker.assert_catalog_consistent();
        worker.run().await
    });

    // 第四条循环：Run 调度与执行（Slice D1/D3）。R4：开关只控制**领取**新 Run；
    // 租约过期回收在本循环内持续运行，不因调度关闭而停摆。执行器由
    // `AI_PM_RUN_EXECUTOR` 选择（codex/mock → CodexExecutor 适配器；缺省 noop →
    // 领取即 start 失败退避，不伪造执行成功）。
    let dispatch_pool = pool.clone();
    let dispatch_worker_id = worker_id.clone();
    tokio::spawn(async move {
        let mut stats_ticks = 0u32;
        let cfg = pm_application::run_dispatch::RunDispatchConfig::from_env();
        let mode = pm_application::run_dispatch::DispatchMode::from_env();
        let executor_mode = std::env::var("AI_PM_RUN_EXECUTOR");
        let codex_executor_enabled = matches!(executor_mode.as_deref(), Ok("codex") | Ok("mock"));
        let run_executor: Arc<dyn pm_application::run_dispatch::RunExecutor> =
            if codex_executor_enabled {
                Arc::new(pm_infrastructure::codex_executor::CodexExecutor::from_env(
                    dispatch_pool.clone(),
                ))
            } else {
                Arc::new(pm_application::run_dispatch::NoopExecutor)
            };
        // PA08-04: discovery is metadata-only and always safe to run. Actual
        // filesystem deletion requires the explicit opt-in below and a Codex
        // executor; the default is non-destructive discovery (stage ledger candidates, delete no files).
        let reclamation_requested = std::env::var("AI_PM_WORKSPACE_RECLAMATION_ENABLED")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        // Local worktrees are host-owned. Deletion is enabled only when the
        // operator explicitly confirms a shared workspace root (or a single
        // worker owns the entire workspace volume).
        let reclamation_shared_root = std::env::var("AI_PM_WORKSPACE_RECLAMATION_SHARED_ROOT")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let reclamation_root_is_absolute = std::path::Path::new(
            &pm_application::run_dispatch::workspace_root_from_env(),
        )
        .is_absolute();
        let reclamation_enabled =
            reclamation_requested && reclamation_shared_root && reclamation_root_is_absolute;
        if reclamation_requested && !reclamation_enabled {
            tracing::warn!(
                "workspace reclamation requested without shared-root acknowledgement and absolute workspace root; deletion remains disabled"
            );
        }
        let reclamation_retention_days = std::env::var("AI_PM_WORKSPACE_RECLAMATION_RETENTION_DAYS")
            .ok()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|days| *days >= 0)
            .unwrap_or(pm_application::workspace_reclamation::DEFAULT_RETENTION_DAYS);
        let workspace_reclaimer: Option<
            Arc<dyn pm_application::workspace_reclamation::WorkspaceReclaimer>,
        > = if codex_executor_enabled {
            Some(Arc::new(
                pm_infrastructure::codex_executor::CodexExecutor::from_env(dispatch_pool.clone()),
            ))
        } else {
            None
        };
        tracing::info!(
            enabled = reclamation_enabled,
            retention_days = reclamation_retention_days,
            "workspace reclamation policy loaded (deletion defaults off)"
        );
        // 启动时一次孤儿 reconcile（T-030 §3 异常路径②）；失败只告警。
        match pm_application::run_executor::reconcile_orphans(
            &dispatch_pool,
            run_executor.as_ref(),
            &cfg,
        )
        .await
        {
            Ok(n) if n > 0 => tracing::info!(reconciled = n, "orphan runs reconciled at startup"),
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "orphan reconcile failed; relying on lease recovery")
            }
        }
        tracing::info!(mode = ?mode, lease_secs = cfg.lease_secs, executor = run_executor.kind(), "run dispatch loop started");
        loop {
            match pm_application::run_executor::claim_and_start(
                &dispatch_pool,
                run_executor.as_ref(),
                &dispatch_worker_id,
                &cfg,
                mode,
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "claim_and_start failed; backing off");
                }
            }
            match pm_application::run_executor::poll_active_runs(
                &dispatch_pool,
                run_executor.as_ref(),
                &dispatch_worker_id,
                &cfg,
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "poll_active_runs failed; backing off");
                }
            }
            // ACP 通信面（0046 / 设计稿 V1.1 §5.3）：人工等待超时判定（§8.2，
            // Platform 持久化 deadline 后不重算；超时 ≠ 同意，链保持开放）。
            match pm_application::interaction::interaction_timeout_sweep(&dispatch_pool).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(timed_out = n, "interaction timeout sweep"),
                Err(e) => tracing::warn!(error = %e, "interaction timeout sweep failed"),
            }
            // I1-E 持久化清理账本（迁移 0035）：主路径——无时间窗、可重试、
            // 阈值后转人工（needs_human 持续可查询）。
            match pm_application::run_cleanup::run_cleanup_round(
                &dispatch_pool,
                run_executor.as_ref(),
            )
            .await
            {
                Ok((ok, failed)) => {
                    if ok + failed > 0 {
                        tracing::info!(cleared = ok, failed, "run cleanup round");
                    }
                }
                Err(e) => tracing::warn!(error = %e, "run cleanup round failed"),
            }
            // T-032 D3 旧路径保留为兜底：撤凭据前已遗留的终态行（无账本行的
            // 历史窗口——终止顺序反转引入的窗口，reconcile_orphans 不覆盖终态行）。
            if let Err(e) = pm_application::run_executor::cleanup_finished_processes(
                &dispatch_pool,
                run_executor.as_ref(),
            )
            .await
            {
                tracing::warn!(error = %e, "finished-run cleanup failed");
            }
            // F-5（审计 2026-11-07）：通信面可观测——每 ~30s 一次聚合日志
            //（字段名即未来 Prometheus 指标名；仓内零 metrics 基建，house
            // style = 聚合函数 + 结构化日志）。
            stats_ticks += 1;
            if stats_ticks >= 500 {
                stats_ticks = 0;
                match pm_application::comm_overview::load_comm_overview(&dispatch_pool).await {
                    Ok(o) => {
                        tracing::info!(
                            interaction_shielded_runs = o.interaction_shielded_runs,
                            interaction_unconsumed_total = o.interaction_unconsumed_total,
                            interaction_open_chains = o.interaction_open_chains,
                            worker_command_pending_total = o.worker_command_pending_total,
                            worker_command_pending_age_seconds =
                                o.worker_command_pending_age_seconds,
                            "comm_overview"
                        );
                        if o.worker_command_pending_age_seconds > 600 {
                            tracing::warn!(
                                age = o.worker_command_pending_age_seconds,
                                "worker command pending age exceeded 600s"
                            );
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "comm_overview failed"),
                }

                match pm_application::workspace_reclamation::stage_candidates(
                    &dispatch_pool,
                    reclamation_retention_days,
                )
                .await
                {
                    Ok(staged) if staged > 0 => {
                        tracing::info!(staged, "workspace reclamation candidates staged");
                    }
                    Ok(_) => {}
                    Err(error) => tracing::warn!(
                        error = %error,
                        "workspace reclamation candidate discovery failed"
                    ),
                }

                if reclamation_enabled {
                    if let Some(reclaimer) = workspace_reclaimer.as_deref() {
                        match pm_application::workspace_reclamation::run_worktree_reclamation_round(
                            &dispatch_pool,
                            reclaimer,
                            reclamation_retention_days,
                        )
                        .await
                        {
                            Ok(report) if report.claimed > 0 => tracing::info!(
                                claimed = report.claimed,
                                completed = report.completed,
                                failed = report.failed,
                                skipped = report.skipped,
                                "Run worktree reclamation round"
                            ),
                            Ok(_) => {}
                            Err(error) => tracing::warn!(
                                error = %error,
                                "Run worktree reclamation round failed"
                            ),
                        }

                        match pm_application::workspace_reclamation::run_repository_mirror_reclamation_round(
                            &dispatch_pool,
                            reclaimer,
                            reclamation_retention_days,
                        )
                        .await
                        {
                            Ok(report) if report.claimed > 0 => tracing::info!(
                                claimed = report.claimed,
                                completed = report.completed,
                                failed = report.failed,
                                skipped = report.skipped,
                                "Repository mirror reclamation round"
                            ),
                            Ok(_) => {}
                            Err(error) => tracing::warn!(
                                error = %error,
                                "Repository mirror reclamation round failed"
                            ),
                        }
                    } else {
                        tracing::warn!(
                            "workspace reclamation enabled but Codex executor is not configured"
                        );
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
        }
    });

    // 第五条循环：D5 review 领取拍（确定性校验管线 → findings/verdict → 推进任务）。
    // 与失败判定作业并列，共享事务模板但不共享错误处理（§6.2）。
    let review_pool = pool.clone();
    tokio::spawn(async move {
        let outbox: Arc<dyn pm_application::outbox::OutboxAppend> =
            Arc::new(pm_application::outbox::PgOutboxAppend);
        pm_application::review::run_review_loop(review_pool, outbox).await
    });

    // 第六条循环：D5 失败判定作业（B4-A~D）。扫描谓词强制 run_kind='delivery'，
    // 对 review run 零写入路径（§6.2 结构性隔离）。
    let judge_pool = pool.clone();
    tokio::spawn(async move {
        loop {
            let report = pm_application::review::judge_failures_once(&judge_pool).await;
            if report.judged > 0 {
                tracing::info!(
                    judged = report.judged,
                    retries = report.retries,
                    task_failures = report.task_failures,
                    "failure judge step"
                );
            }
            tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
        }
    });

    // 第七条循环：assistant 会话回复（Batch G，D-4 会话存储的生产者侧）。
    // `AI_PM_CONVERSATION_REPLY=off|on`（默认 off，与 AI_PM_RUN_DISPATCH 的
    // 谨慎传统对齐：新的计费型生产者不得在升级后静默启动）。扫描/防重入/
    // 三级 provider 解析/落账细节都在 `pm_ai::conversation_reply`。
    let conversation_pool = pool.clone();
    tokio::spawn(async move {
        let mode = pm_ai::conversation_reply::ConversationReplyMode::from_env();
        tracing::info!(
            ?mode,
            "conversation reply loop (AI_PM_CONVERSATION_REPLY=off|on, default off)"
        );
        if !mode.enabled() {
            return;
        }
        let mut worker = pm_ai::conversation_reply::ConversationReplyWorker::new(conversation_pool);
        loop {
            match worker.reply_round().await {
                Ok(report) if report.has_activity() => {
                    tracing::info!(
                        replied = report.replied,
                        failed = report.failed,
                        skipped_no_provider = report.skipped_no_provider,
                        "conversation reply round"
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "conversation reply round failed; backing off")
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(CONVERSATION_REPLY_POLL_MS)).await;
        }
    });

    // 第八条循环：访谈编排（DK-B）。开关 off 时 spawn 立即返回。
    spawn_discovery_orchestrator(pool.clone());

    loop {
        match job_queue::claim(&pool, "ai_analysis", &worker_id, LEASE_SECS).await {
            Some(job) => handle_job(&orchestrator, &job).await,
            None => tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await,
        }
    }
}

/// 第八条循环：访谈编排（DK-B / D-DK-9）。
///
/// 与第七循环同款形态：本函数只管 spawn + 节奏，扫描谓词/提示词预算/
/// 输出校验/三条件/降级纪律都在 `pm_ai::discovery_orchestrator`。
/// 开关 `AI_PM_DISCOVERY_ORCHESTRATOR=off|on`，**默认 off**——新的计费型
/// 生产者不得在升级后静默启动。
fn spawn_discovery_orchestrator(pool: PgPool) {
    tokio::spawn(async move {
        let mode = pm_ai::discovery_orchestrator::DiscoveryOrchestratorMode::from_env();
        tracing::info!(
            ?mode,
            "discovery orchestrator loop (AI_PM_DISCOVERY_ORCHESTRATOR=off|on, default off)"
        );
        if !mode.enabled() {
            return;
        }
        // DK-C 注入：S1~S5 逐会话只读投影（读侧 fail-open，投影故障 WARN 后
        // S6-only 继续，不阻塞轮次）。
        let mut worker =
            pm_ai::discovery_orchestrator::DiscoveryOrchestratorWorker::new(pool.clone())
                .with_context_builder(std::sync::Arc::new(pm_ai::PgInterviewContextBuilder::new(
                    pool.clone(),
                )));
        // DK-F（G-1）：生产写回接线。DK-D 只落了端口与骨架（`NoWriteBack`
        // 默认、模型仅测试 Fake），其验收记录 §十一「生产注入」把「真 provider
        // 适配 + KnowledgeRoot 配置 + CapabilityProjection 真投影」派给本批。
        //
        // 边界不变：写回钩子只写「草案」（止步 pending_confirmation）与 draft
        // 任务批；AI 无确认权（R3）。知识 Git 根由 `AI_PM_KNOWLEDGE_ROOT` 决定
        // （默认 `<cwd>/../ai-pm-knowledge`）——需求修订的 Markdown 绑定点就在
        // 那里，容器里必须可写。
        let write_back = pm_ai::discovery_writeback::DiscoveryWriteBack {
            pool: pool.clone(),
            model: std::sync::Arc::new(pm_ai::LlmWriteBackModel::new(pool.clone())),
            capability: std::sync::Arc::new(pm_ai::PgCapabilityProjection::new(pool.clone())),
            knowledge: pm_application::knowledge::KnowledgeRoot::from_env(),
            mode: pm_ai::discovery_writeback::WriteBackMode::from_env(),
        };
        tracing::info!(
            knowledge_root = %pm_application::knowledge::KnowledgeRoot::from_env().path().display(),
            "discovery write-back wired (draft-only: AI stops at pending_confirmation)",
        );
        worker = worker.with_write_back(std::sync::Arc::new(write_back));
        loop {
            match worker.orchestrate_round().await {
                Ok(report) if report.has_activity() => {
                    tracing::info!(
                        processed = report.processed,
                        applied = report.applied,
                        findings_added = report.findings_added,
                        questions_added = report.questions_added,
                        answers_consumed = report.answers_consumed,
                        to_review_required = report.to_review_required,
                        to_needs_user_input = report.to_needs_user_input,
                        skipped_no_provider = report.skipped_no_provider,
                        invalid_rounds = report.invalid_rounds,
                        stale_rounds = report.stale_rounds,
                        failed = report.failed,
                        overreach_total = report.overreach_total,
                        "discovery orchestration round"
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "discovery orchestration round failed; backing off")
                }
            }
            // DK-F：写回扫描面（真栈联调补的触发面）。编排轮次只在
            // `IN_PROGRESS`/`NEEDS_USER_INPUT` 上跑，而起草的输入是人工已
            // CONFIRMED 的 finding——两者之间隔着「人工裁决」这一步，所以
            // 轮次内的写回调用永远看不到可起草的输入。扫描面独立于编排谓词：
            // 扫 `REVIEW_REQUIRED` + 已有 CONFIRMED finding 的会话。
            match worker.sweep_write_back().await {
                Ok(report) if report.drafts_submitted > 0 || report.task_batches_generated > 0 => {
                    tracing::info!(
                        drafts_submitted = report.drafts_submitted,
                        task_batches_generated = report.task_batches_generated,
                        "discovery write-back sweep"
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "discovery write-back sweep failed; backing off")
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(
                DISCOVERY_ORCHESTRATOR_POLL_MS,
            ))
            .await;
        }
    });
}

async fn handle_repo_job(pool: &PgPool, job: &ClaimedJob) {
    let import_id = match import_id_for_event(pool, job.event_id).await {
        Ok(Some(import_id)) => import_id,
        other => {
            tracing::error!(job_id = %job.job_id, ?other, "repo import job has no import id");
            let _ = job_queue::fail(
                pool,
                job.job_id,
                "repo import job has no import id",
                MAX_JOB_ATTEMPTS,
                JOB_BACKOFF_BASE_SECS,
            )
            .await;
            return;
        }
    };
    // Phase 3B：四级 Role 链（role_key='general'）；解析失败 = 显式
    // Configuration Error，job 直接失败（不回退 Fake，DEC-LLM-004 兑现）。
    let (model, model_name) =
        match pm_ai::repo_model::model_from_llm_provider(pool, job.project_id).await {
            Ok(pair) => pair,
            Err(err) => {
                tracing::error!(
                    job_id = %job.job_id,
                    error = %err,
                    "repo import model resolution failed (explicit configuration error)"
                );
                // 导入行必须同步落 failed——否则它永远停在 analyzing，
                // 客户端只能看到「分析中」而无从得知配置坏了（2026-11-14）。
                pm_application::project_import::fail_import(
                    pool,
                    import_id,
                    "model_configuration",
                    &format!("model resolution failed: {err}"),
                )
                .await;
                let _ = job_queue::fail(
                    pool,
                    job.job_id,
                    &format!("model_configuration: {err}"),
                    MAX_JOB_ATTEMPTS,
                    JOB_BACKOFF_BASE_SECS,
                )
                .await;
                return;
            }
        };
    if let Err(e) = sqlx::query("UPDATE integration.repo_import SET model_name = $2 WHERE id = $1")
        .bind(import_id)
        .bind(&model_name)
        .execute(pool)
        .await
    {
        tracing::error!(job_id = %job.job_id, error = %e, "model_name write failed");
    }
    match run_import_job(pool, model.as_ref(), import_id).await {
        Ok(()) => {
            if let Err(e) = job_queue::complete(pool, job.job_id).await {
                tracing::error!(job_id = %job.job_id, error = %e, "job complete failed");
            }
        }
        Err(e) => {
            tracing::warn!(job_id = %job.job_id, error = %e, "repo import job failed, backing off");
            let reason = e.to_string();
            if let Err(e) = job_queue::fail(
                pool,
                job.job_id,
                &reason,
                MAX_JOB_ATTEMPTS,
                JOB_BACKOFF_BASE_SECS,
            )
            .await
            {
                tracing::error!(job_id = %job.job_id, error = %e, "job fail failed");
            }
            // 重试耗尽（job 落 failed）→ 导入行同步落终态；否则它永远停在
            // queued/analyzing，客户端只能看到「分析中」（2026-11-14）。
            let job_status: Option<String> =
                sqlx::query_scalar("SELECT status FROM integration.job WHERE id = $1")
                    .bind(job.job_id)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten();
            if job_status.as_deref() == Some("failed") {
                pm_application::project_import::fail_import(pool, import_id, "job_failed", &reason)
                    .await;
            }
        }
    }
}

/// One claimed job: `Completed` (including the idempotent already-done
/// redelivery path) completes the job; `RetryScheduled` means the
/// orchestrator already failed the job with backoff — touching it here
/// (a `complete`) would erase the scheduled retry; a `Model`/`Db` error
/// fails it with backoff so the queue retries.
async fn handle_job(orchestrator: &Orchestrator, job: &ClaimedJob) {
    match orchestrator.handle_ai_job(job).await {
        Ok(HandleOutcome::Completed) => {
            if let Err(e) = job_queue::complete(&orchestrator.pool, job.job_id).await {
                tracing::error!(job_id = %job.job_id, error = %e, "job complete failed");
            }
        }
        Ok(HandleOutcome::RetryScheduled) => {
            tracing::info!(
                job_id = %job.job_id,
                "retry already scheduled by the orchestrator; leaving the job queued"
            );
        }
        Err(e) => {
            tracing::warn!(job_id = %job.job_id, error = %e, "job failed, backing off");
            if let Err(e) = job_queue::fail(
                &orchestrator.pool,
                job.job_id,
                &e.to_string(),
                MAX_JOB_ATTEMPTS,
                JOB_BACKOFF_BASE_SECS,
            )
            .await
            {
                tracing::error!(job_id = %job.job_id, error = %e, "job fail failed");
            }
        }
    }
}
