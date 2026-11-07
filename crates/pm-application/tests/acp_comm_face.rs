//! ACP 通信面集成测试（0046 / 设计稿 V1.1）：
//! ACP-429 上报事务全链 · ACP-430 幂等 · ACP-431 开放链冲突 · ACP-437 claim
//! 过期重投 · ACP-439 超时不自动重领 · ACP-427 poll 屏蔽 · ACP-428 重领屏蔽 ·
//! ACP-441 注入失败不判交付 · 情形 6（terminate 未回执时 continue 不下发）·
//! 全生命周期（pending→responded→consumed + guard 转换 + Task/等待映射）。

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::interaction::{
    cancel_open_interaction, interaction_timeout_sweep, mark_consumed, record_response_persisted,
    report_interaction, retry_interaction, ReportInteractionInput,
};
use pm_application::run_dispatch::{
    claim_once, DispatchMode, NoopExecutor, RunDispatchConfig,
};
use pm_application::run_executor::poll_active_runs;
use pm_application::worker_command as cmd;
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    task_id: Uuid,
    run_id: Uuid,
    lease_token: String,
    lease_generation: i32,
    human_id: Uuid,
}

fn cfg() -> RunDispatchConfig {
    RunDispatchConfig {
        lease_secs: 300,
        backoff_cap_secs: 300,
        max_run_attempts: 3,
        max_infrastructure_attempts: 5,
        timeout_secs: 1800,
        deadline_default_secs: 3600,
        agent_heartbeat_stale_secs: 900,
        progress_stale_secs: 3600,
    }
}

async fn seed(pool: &PgPool, code: &str) -> (Uuid, Uuid, Uuid) {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id) \
         VALUES ($1, 't', 'ready', 1, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(outcome.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, status, head_commit_sha, tenant_id) \
         VALUES ($1, 'web', 'git@host:repo', 'verified', 'headsha', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, base_ref, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(outcome.project_id)
    .bind(repo_id)
    .execute(pool)
    .await
    .unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (outcome.project_id, task_id, outcome.human_id)
}

/// 铸造一个 started/running 的 Model B spawn Run（含一次性明文 token；
/// `claim_once` 的 token 只进 ExecutableRun 不回传，故直接落库对齐 post-claim 态）。
async fn claimed_run(pool: &PgPool, code: &str) -> Fx {
    let (project_id, task_id, human_id) = seed(pool, code).await;
    let token = pm_application::run_context::generate_lease_token().unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "UPDATE delivery.task_run \
            SET status = 'running', execution_mode = 'spawn', \
                executor_session = 'codex:test', stage = 'started', \
                lease_owner = 'w1', lease_token_hash = $3, lease_generation = 1, \
                lease_expires_at = now() + interval '300 seconds', \
                deadline_at = now() + interval '3600 seconds' \
          WHERE id = (SELECT id FROM delivery.task_run \
                       WHERE task_id = $1 AND status = 'queued' LIMIT 1) \
         RETURNING id",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(pm_application::run_context::hash_lease_token(&token))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task SET status = 'claimed' WHERE id = $1")
        .bind(task_id)
        .execute(pool)
        .await
        .unwrap();
    Fx {
        project_id,
        task_id,
        run_id,
        lease_token: token,
        lease_generation: 1,
        human_id,
    }
}

fn input(fx: &Fx, event_id: &str) -> ReportInteractionInput {
    ReportInteractionInput {
        project_id: fx.project_id,
        task_id: fx.task_id,
        run_id: fx.run_id,
        event_id: event_id.to_string(),
        reason: "needs_decision".into(),
        title: "需要确认迁移策略".into(),
        question: "是否允许删除旧字段？".into(),
        options: Some(json!([{ "value": "approve", "label": "允许" }])),
        required_response: true,
        checkpoint_path: Some(".aipm/checkpoint.json".into()),
    }
}

async fn run_field(pool: &PgPool, run_id: Uuid, col: &str) -> String {
    sqlx::query_scalar(&format!("SELECT {col} FROM delivery.task_run WHERE id = $1"))
        .bind(run_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn task_status(pool: &PgPool, task_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn open_waiting(pool: &PgPool, task_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM delivery.task_waiting WHERE task_id = $1 AND ended_at IS NULL")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn acp429_430_report_persists_full_transaction_and_is_idempotent(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-429").await;
    let actor = AuditActor::principal(fx.human_id);

    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &actor).await.unwrap();
    assert_eq!(reported.status, "pending");
    assert!(!reported.replayed);
    // 同事务四件套：guard、Task 态、等待行、terminate 命令。
    assert_eq!(run_field(&pool, fx.run_id, "scheduling_shield").await, "human_wait");
    assert_eq!(task_status(&pool, fx.task_id).await, "awaiting_decision");
    assert_eq!(open_waiting(&pool, fx.task_id).await, 1);
    let commands: (String, String, i32) = sqlx::query_as(
        "SELECT command_type, worker_id, lease_generation FROM delivery.worker_command \
          WHERE run_id = $1",
    )
    .bind(fx.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(commands.0, "agent.process.terminate");
    assert_eq!(commands.1, "w1", "worker_id must snapshot run.lease_owner");
    assert_eq!(commands.2, fx.lease_generation);

    // ACP-430：同 event_id 幂等重放。
    let replay = report_interaction(&pool, &input(&fx, "evt-1"), &actor).await.unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.interaction_id, reported.interaction_id);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.task_interaction")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn acp431_open_chain_conflicts(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-431").await;
    let actor = AuditActor::principal(fx.human_id);
    report_interaction(&pool, &input(&fx, "evt-1"), &actor).await.unwrap();
    let err = report_interaction(&pool, &input(&fx, "evt-2"), &actor).await.unwrap_err();
    assert!(matches!(err, pm_application::interaction::InteractionError::Conflict(_)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn acp439_timeout_sweep_shields_but_keeps_chain_open(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-439").await;
    let actor = AuditActor::principal(fx.human_id);
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &actor).await.unwrap();
    sqlx::query("UPDATE delivery.task_interaction SET deadline_at = now() - interval '1 second' WHERE id = $1")
        .bind(reported.interaction_id)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(interaction_timeout_sweep(&pool).await.unwrap(), 1);
    let (status, chain_open): (String, bool) = sqlx::query_as(
        "SELECT status, chain_open FROM delivery.task_interaction WHERE id = $1",
    )
    .bind(reported.interaction_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "timed_out");
    assert!(chain_open, "un-dispositioned timeout keeps the chain open (ACP-439)");
    assert_eq!(run_field(&pool, fx.run_id, "scheduling_shield").await, "timed_out");

    // 超时未处置：新上报仍被开放链拒绝；重试必须走 retry_of 挂链。
    assert!(report_interaction(&pool, &input(&fx, "evt-2"), &actor).await.is_err());
    let retry_id = retry_interaction(&pool, fx.project_id, reported.interaction_id, &actor)
        .await
        .unwrap();
    let (new_status, new_open, retry_of): (String, bool, Uuid) = sqlx::query_as(
        "SELECT status, chain_open, retry_of FROM delivery.task_interaction WHERE id = $1",
    )
    .bind(retry_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(new_status, "pending");
    assert!(new_open);
    assert_eq!(retry_of, reported.interaction_id);
    let old_open: bool = sqlx::query_scalar(
        "SELECT chain_open FROM delivery.task_interaction WHERE id = $1",
    )
    .bind(reported.interaction_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!old_open, "superseded row must yield chain_open");
    assert_eq!(run_field(&pool, fx.run_id, "scheduling_shield").await, "human_wait");
}

#[sqlx::test(migrations = "../../migrations")]
async fn lifecycle_response_delivery_consumption_and_guard_release(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-LIFE").await;
    let actor = AuditActor::dispatch();
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::principal(fx.human_id))
        .await
        .unwrap();

    // 领取 terminate（w1）→ acked（exit_outcome 证据）。
    let polled = cmd::poll_commands(&pool, fx.project_id, fx.run_id, "w1").await.unwrap();
    assert_eq!(polled.len(), 1, "only terminate surfaces; continue does not exist yet");
    let terminate_id = polled[0].command_id;
    cmd::ack_command(
        &pool,
        fx.project_id,
        terminate_id,
        "w1",
        cmd::AckOutcome::Acked,
        Some(json!({ "exit_outcome": "Killed", "pid": 4321 })),
        None,
    )
    .await
    .unwrap();
    // F-10：acked 终止 → 账本 completed。
    let ledger: String = sqlx::query_scalar(
        "SELECT status FROM delivery.task_run_cleanup WHERE run_id = $1",
    )
    .bind(fx.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ledger, "completed");

    // 回复持久化（ACP-03 回执）：pending → responded + guard start_wait。
    record_response_persisted(
        &pool,
        fx.project_id,
        reported.interaction_id,
        json!({ "response_id": "resp-1" }),
        &actor,
    )
    .await
    .unwrap();
    assert_eq!(run_field(&pool, fx.run_id, "scheduling_shield").await, "start_wait");

    // 下发 continue（continue 命令在 terminate 未回执时被 poll 过滤——情形 6）。
    let mut tx = pool.begin().await.unwrap();
    let continue_id = cmd::issue_continue_command(
        &mut tx,
        fx.project_id,
        fx.task_id,
        fx.run_id,
        reported.interaction_id,
        None,
        "w1",
        fx.lease_generation,
        json!({ "decision": "approve", "content": "允许删除" }),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // 先领取（terminate 已 acked → 情形 6 窗口关闭，continue 正常下发）。
    let polled2 = cmd::poll_commands(&pool, fx.project_id, fx.run_id, "w1").await.unwrap();
    assert_eq!(polled2.len(), 1);
    assert_eq!(polled2[0].command_id, continue_id);
    assert_eq!(polled2[0].command_type, "agent.task.continue");

    // ACP-441：注入失败（缺 input_injected）不得 acked。
    let bad = cmd::ack_command(
        &pool,
        fx.project_id,
        continue_id,
        "w1",
        cmd::AckOutcome::Acked,
        Some(json!({ "session_created": true })),
        None,
    )
    .await;
    assert!(bad.is_err(), "session_created alone must not ack a continue");
    assert_eq!(run_field(&pool, fx.run_id, "scheduling_shield").await, "start_wait");

    // 正确回执：session_created + input_injected → acked + 屏蔽释放 + Task 仍在等待。
    cmd::ack_command(
        &pool,
        fx.project_id,
        continue_id,
        "w1",
        cmd::AckOutcome::Acked,
        Some(json!({ "session_created": true, "input_injected": true, "session_id": "sess-2" })),
        None,
    )
    .await
    .unwrap();
    assert_eq!(run_field(&pool, fx.run_id, "scheduling_shield").await, "none");
    // R-4（ACP-450）：INPUT_INJECTED = 执行恢复确认边界——Task 同事务回 Running，
    // 等待 resolve，链关闭；Interaction 保持 responded（consumed 仅为观测事实）。
    assert_eq!(task_status(&pool, fx.task_id).await, "running",
        "continue acked resumes execution in the same transaction (R-4)");
    assert_eq!(open_waiting(&pool, fx.task_id).await, 0);
    let (status, chain_open): (String, bool) = sqlx::query_as(
        "SELECT status, chain_open FROM delivery.task_interaction WHERE id = $1",
    )
    .bind(reported.interaction_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "responded", "interaction stays responded (N-6 legal steady state)");
    assert!(!chain_open, "chain closed at acked (R-4)");
    // ACP-451：链已关闭 → 新 Session 运行中可再次介入（新链）。
    let second = report_interaction(&pool, &input(&fx, "evt-2"), &AuditActor::principal(fx.human_id))
        .await
        .unwrap();
    assert!(!second.replayed, "new chain allowed after acked-closed chain");

    // ACP-434：证据不足禁止伪造 CONSUMED。
    let bad_evidence = mark_consumed(
        &pool,
        fx.project_id,
        reported.interaction_id,
        None,
        &json!({ "kinds": ["initial_input_injected"] }),
        &actor,
    )
    .await;
    assert!(bad_evidence.is_err());

    // 合法消费：consumed（迟到回执不改变已确认的恢复结果——Task 保持 running）。
    mark_consumed(
        &pool,
        fx.project_id,
        reported.interaction_id,
        Some(Uuid::new_v4()),
        &json!({ "kinds": ["initial_input_injected", "process_alive"] }),
        &actor,
    )
    .await
    .unwrap();
    let (status, chain_open): (String, bool) = sqlx::query_as(
        "SELECT status, chain_open FROM delivery.task_interaction WHERE id = $1",
    )
    .bind(reported.interaction_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "consumed");
    assert!(!chain_open);
    assert_eq!(task_status(&pool, fx.task_id).await, "running");
    assert_eq!(open_waiting(&pool, fx.task_id).await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn acp437_claim_lease_expiry_redelivers_same_command(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-437").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();

    let first = cmd::poll_commands(&pool, fx.project_id, fx.run_id, "w1").await.unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].attempt, 1);

    // 模拟 Runner 崩溃：claim 租约过期 + lease 也过期（ACP-438：命令面不依赖租约）。
    sqlx::query(
        "UPDATE delivery.worker_command SET claim_expires_at = now() - interval '1 second' \
          WHERE id = $1",
    )
    .bind(first[0].command_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task_run SET lease_expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(fx.run_id)
        .execute(&pool)
        .await
        .unwrap();

    let second = cmd::poll_commands(&pool, fx.project_id, fx.run_id, "w1").await.unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].command_id, first[0].command_id, "same command_id redelivered");
    assert_eq!(second[0].attempt, 2);

    // 其他 worker 不能领取别人的命令。
    let other = cmd::poll_commands(&pool, fx.project_id, fx.run_id, "w2").await.unwrap();
    assert!(other.is_empty(), "worker binding must hold");

    cmd::ack_command(
        &pool,
        fx.project_id,
        first[0].command_id,
        "w1",
        cmd::AckOutcome::Acked,
        Some(json!({ "exit_outcome": "Killed" })),
        None,
    )
    .await
    .unwrap();
    let after = cmd::poll_commands(&pool, fx.project_id, fx.run_id, "w1").await.unwrap();
    assert!(after.is_empty(), "acked commands never redeliver");
    let _ = reported;
}

#[sqlx::test(migrations = "../../migrations")]
async fn acp427_428_shield_blocks_poll_signals_and_reclaim(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-427").await;
    report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();

    // S1：deadline 已过 + 心跳/进展陈旧 → poll 跳过（Run 不被回收/不 TimedOut）。
    sqlx::query(
        "UPDATE delivery.task_run \
            SET deadline_at = now() - interval '1 second', \
                last_agent_heartbeat_at = now() - interval '3600 seconds', \
                last_progress_at = now() - interval '3600 seconds' \
          WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let handled = poll_active_runs(&pool, &NoopExecutor, "w1", &cfg()).await.unwrap();
    assert_eq!(handled, 0, "shielded runs must be invisible to all four signals");
    assert_eq!(run_field(&pool, fx.run_id, "status").await, "running");
    assert_eq!(run_field(&pool, fx.run_id, "scheduling_shield").await, "human_wait");

    // S2：租约过期 → 回收被拒（Model B skip、Run 原样保留）。
    sqlx::query(
        "UPDATE delivery.task_run SET status = 'claimed', lease_expires_at = now() - interval '1 second' \
          WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let outcome = claim_once(&pool, "w2", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .expect("the only candidate is skipped, not silently consumed");
    assert_eq!(outcome.kind, pm_application::run_dispatch::ClaimKind::Skipped);
    assert_eq!(outcome.run_id, fx.run_id);
    assert_eq!(run_field(&pool, fx.run_id, "status").await, "claimed",
        "shielded run must not be reclaimed or requeued");
    assert_eq!(run_field(&pool, fx.run_id, "scheduling_shield").await, "human_wait");

    // S2（Model A）：显式请求 → 前置失败（不泄露细节）。
    let err = pm_application::run_dispatch::claim_task_run_as(
        &pool,
        fx.project_id,
        fx.task_id,
        "client-a",
        None,
        &cfg(),
        &AuditActor::principal(fx.human_id),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, pm_application::run_dispatch::RunDispatchError::TaskNotReady(_)),
        "Model A claim against a shielded run must be refused"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn situation6_continue_never_surfaces_while_terminate_unconfirmed(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-S6").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    cmd::issue_continue_command(
        &mut tx,
        fx.project_id,
        fx.task_id,
        fx.run_id,
        reported.interaction_id,
        None,
        "w1",
        fx.lease_generation,
        json!({ "decision": "approve" }),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // terminate 仍未回执 → poll 只返回 terminate，continue 被过滤（情形 6）。
    let polled = cmd::poll_commands(&pool, fx.project_id, fx.run_id, "w1").await.unwrap();
    assert_eq!(polled.len(), 1);
    assert_eq!(polled[0].command_type, "agent.process.terminate");
    assert!(cmd::has_unconfirmed_termination(&pool, fx.run_id).await.unwrap());

    // terminate 失败（重试耗尽）→ 启动冻结 + Task 阻塞；continue 仍不下发。
    cmd::ack_command(
        &pool,
        fx.project_id,
        polled[0].command_id,
        "w1",
        cmd::AckOutcome::Failed,
        None,
        Some("termination retries exhausted".into()),
    )
    .await
    .unwrap();
    let frozen: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT continuation_start_frozen_at FROM delivery.task_interaction WHERE id = $1",
    )
    .bind(reported.interaction_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(frozen.is_some(), "R-ACP-7: startup freeze must be recorded");
    // Task 停在 awaiting_decision（人工等待即既有合法阻塞表达；
    // AwaitingDecision→Blocked 非合法边，不得绕开 13 态表达阻塞）。
    assert_eq!(task_status(&pool, fx.task_id).await, "awaiting_decision");
    // F-10：终止失败（旧进程退出未确认）→ 账本 needs_human（转人工清理）。
    let ledger: (String, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT status, completed_at FROM delivery.task_run_cleanup WHERE run_id = $1",
    )
    .bind(fx.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ledger.0, "needs_human");
    assert!(ledger.1.is_none(), "not completed");
    // acked 路径的 completed 账本由 acp446 之外的 terminate ack 流覆盖
    //（lifecycle 测试 ack terminate 后同样可查）。
    // terminate 已终态（failed）→ 情形 6 的「未回执」窗口关闭，continue 恢复
    // 可下发；但执行端启动前检查（frozen + §9.4 检查 8）会拒绝真正启动——
    // 平台侧只保证"拒绝得干净"，启动拒绝由执行端 Runner 落实（WP-3）。
    let after_fail = cmd::poll_commands(&pool, fx.project_id, fx.run_id, "w1").await.unwrap();
    assert_eq!(after_fail.len(), 1);
    assert_eq!(after_fail[0].command_type, "agent.task.continue");
}

#[sqlx::test(migrations = "../../migrations")]
async fn cancel_open_interaction_closes_chain_and_unshields(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-CXL").await;
    report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    let cancelled = cancel_open_interaction(
        &pool,
        fx.project_id,
        fx.task_id,
        "mistaken report",
        &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    assert!(cancelled);
    assert_eq!(run_field(&pool, fx.run_id, "scheduling_shield").await, "none",
        "cancelled intervention (task alive) restores normal scheduling (case 5)");
    assert_eq!(open_waiting(&pool, fx.task_id).await, 0);
    // 链已关闭 → 允许新介入。
    let again = report_interaction(&pool, &input(&fx, "evt-2"), &AuditActor::principal(fx.human_id))
        .await
        .unwrap();
    assert!(!again.replayed);
}

#[sqlx::test(migrations = "../../migrations")]
async fn runner_prefix_claim_stamps_spawn_execution_mode(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-RUN").await;
    // 夹具是手工铸造的；这里走真实 MCP 领取通道验证前缀标记。
    let (project_id, task_id, human_id) = seed(&pool, "ACP-RUN2").await;
    let _ = fx;
    // claimability：执行器指派前置（与 run_dispatch 种子一致）。
    let agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', 'dev') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task SET assignee_id = $1 WHERE id = $2")
        .bind(agent)
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();
    let actor = AuditActor::principal(human_id);
    let claimed = pm_application::run_dispatch::claim_task_run_as(
        &pool,
        project_id,
        task_id,
        "runner:test-host",
        None,
        &cfg(),
        &actor,
    )
    .await
    .unwrap();
    let mode: String = sqlx::query_scalar(
        "SELECT execution_mode FROM delivery.task_run WHERE id = $1",
    )
    .bind(claimed.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mode, "spawn", "runner: prefixed MCP claim = client-side process carrier");

    // 对照：普通 MCP 领取仍是 Model A human（ACP-425 退化路径不变）。
    let (project_id2, task_id2, human_id2) = seed(&pool, "ACP-RUN3").await;
    let agent2: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', 'dev2') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task SET assignee_id = $1 WHERE id = $2")
        .bind(agent2)
        .bind(task_id2)
        .execute(&pool)
        .await
        .unwrap();
    let claimed2 = pm_application::run_dispatch::claim_task_run_as(
        &pool,
        project_id2,
        task_id2,
        "codex-cli",
        None,
        &cfg(),
        &AuditActor::principal(human_id2),
    )
    .await
    .unwrap();
    let mode2: String = sqlx::query_scalar(
        "SELECT execution_mode FROM delivery.task_run WHERE id = $1",
    )
    .bind(claimed2.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mode2, "human");
}

#[sqlx::test(migrations = "../../migrations")]
async fn acp445_task_cancel_cancels_open_chain_in_same_transaction(pool: PgPool) {
    use pm_application::command_executor::{ExecutionContext, PgCommandExecutor};
    use pm_application::PolicyEngine;
    use pm_domain::{ProjectCommand, TaskStatus};
    use std::sync::Arc;

    let fx = claimed_run(&pool, "ACP-445").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    assert_eq!(task_status(&pool, fx.task_id).await, "awaiting_decision");

    // Task -> Cancelled（经唯一命令通道）：同事务联动取消介入链（F-8 钩子）。
    let ex = PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()));
    let ctx = ExecutionContext {
        project_id: fx.project_id,
        actor: AuditActor::principal(fx.human_id),
        actor_kind: pm_application::command_executor::ActorKind::Human,
        source: pm_application::command_executor::CommandSource::HumanIntervention { intervention_id: reported.interaction_id },
    };
    ex.execute(
        ProjectCommand::UpdateTaskStatus {
            task_id: fx.task_id,
            expected_revision: 3, // seed=1, claimed->running=2, ->awaiting_decision=3
            new_status: TaskStatus::Cancelled,
        },
        ctx,
    )
    .await
    .expect("cancel must execute");

    let (status, chain_open): (String, bool) = sqlx::query_as(
        "SELECT status, chain_open FROM delivery.task_interaction WHERE id = $1",
    )
    .bind(reported.interaction_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "cancelled", "open interaction must be cancelled with the task");
    assert!(!chain_open);
    assert_eq!(run_field(&pool, fx.run_id, "scheduling_shield").await, "none",
        "guard must not survive the task (V1.1 §4.1: terminal task -> none)");
    assert_eq!(open_waiting(&pool, fx.task_id).await, 0, "waiting resolved in same tx");

    // ACP-445 正向面：已取消 Task 上不得产生有效新 Interaction。
    // （run 仍处 running：report 会走到 Task 迁移检查处被拒并整体回滚。）
    let err = report_interaction(&pool, &input(&fx, "evt-2"), &AuditActor::dispatch())
        .await
        .unwrap_err();
    assert!(matches!(err, pm_application::interaction::InteractionError::Conflict(_)));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.task_interaction")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "no new interaction row may persist for a cancelled task");
}

// ───────────────────────── ACP-03：Human Response 提交（ACP-446~456） ─────────────────────────

use serde_json::Value as JsonValue;
use pm_application::interaction::{submit_interaction_response, SubmitResponseInput};

fn submit_input(fx: &Fx, interaction_id: Uuid, key: &str, payload: JsonValue) -> SubmitResponseInput {
    SubmitResponseInput {
        project_id: fx.project_id,
        interaction_id,
        responder_principal_id: fx.human_id,
        idempotency_key: key.into(),
        response_payload: payload,
    }
}

async fn f9_style_setup(pool: &PgPool) -> (Fx, Uuid) {
    let fx = claimed_run(pool, "ACP-03").await;
    let reported = report_interaction(pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    (fx, reported.interaction_id)
}

#[sqlx::test(migrations = "../../migrations")]
async fn acp446_reply_full_transaction_issues_continue(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-446").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    let mut inp = submit_input(
        &fx,
        reported.interaction_id,
        "idem-1",
        json!({ "decision": "approve", "content": "允许删除" }),
    );
    inp.interaction_id = reported.interaction_id;
    let out = submit_interaction_response(&pool, &inp, &AuditActor::principal(fx.human_id))
        .await
        .unwrap();
    assert_eq!(out.status, "responded");
    assert!(!out.replayed);

    let row: (String, String) = sqlx::query_as(
        "SELECT ti.status, r.scheduling_shield FROM delivery.task_interaction ti \
         JOIN delivery.task_run r ON r.id = ti.run_id WHERE ti.id = $1",
    )
    .bind(reported.interaction_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "responded");
    assert_eq!(row.1, "start_wait", "guard release point (V1.1 §4.1)");

    // 同事务投递：continue 命令已下发且回写 response 行。
    let cmd: (String, String) = sqlx::query_as(
        "SELECT command_type, worker_id FROM delivery.worker_command \
          WHERE run_id = $1 AND command_type = 'agent.task.continue'",
    )
    .bind(fx.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cmd.1, "w1", "routed to lease_owner snapshot");
    assert_eq!(out.continue_command_id, Some(cmd_id(&pool, fx.run_id).await));

    // 检查快照落库。
    let authz: JsonValue = sqlx::query_scalar(
        "SELECT authorization_context FROM delivery.interaction_response WHERE id = $1",
    )
    .bind(out.response_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(authz["authority"], "project_member");
}

async fn cmd_id(pool: &PgPool, run_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "SELECT id FROM delivery.worker_command \
          WHERE run_id = $1 AND command_type = 'agent.task.continue'",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn acp448_needs_approval_requires_lead_or_approver(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-448").await;
    let mut inp = input(&fx, "evt-1");
    inp.reason = "needs_approval".into();
    let reported = report_interaction(&pool, &inp, &AuditActor::dispatch())
        .await
        .unwrap();
    let mut sub = submit_input(
        &fx,
        reported.interaction_id,
        "idem-1",
        json!({ "decision": "approve" }),
    );
    sub.interaction_id = reported.interaction_id;

    // Lead（owner）放行。
    submit_interaction_response(&pool, &sub, &AuditActor::principal(fx.human_id))
        .await
        .unwrap();

    // 第二条介入链（retry 语义此处直接用新 run 太重——改用另一项目验证非 Lead 拒绝）。
    let fx2 = claimed_run(&pool, "ACP-448b").await;
    let mut inp2 = input(&fx2, "evt-1");
    inp2.reason = "needs_approval".into();
    let reported2 = report_interaction(&pool, &inp2, &AuditActor::dispatch())
        .await
        .unwrap();
    let outsider: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'outsider') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut sub2 = submit_input(
        &fx2,
        reported2.interaction_id,
        "idem-1",
        json!({ "decision": "approve" }),
    );
    sub2.interaction_id = reported2.interaction_id;
    sub2.responder_principal_id = outsider;
    let err = submit_interaction_response(&pool, &sub2, &AuditActor::principal(outsider))
        .await
        .unwrap_err();
    assert!(matches!(err, pm_application::interaction::InteractionError::Forbidden(_)));

    // 显式 Approver 授权（0040 approver_grant）后放行。
    sqlx::query(
        "INSERT INTO core.approver_grant (project_id, principal_id, granted_by, tenant_id) \
         VALUES ($1, $2, $2, (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(fx2.project_id)
    .bind(outsider)
    .execute(&pool)
    .await
    .unwrap();
    submit_interaction_response(&pool, &sub2, &AuditActor::principal(outsider))
        .await
        .unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn acp449_450_status_and_deadline_gates(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-449").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    let mut sub = submit_input(&fx, reported.interaction_id, "idem-1", json!({ "content": "ok" }));
    sub.interaction_id = reported.interaction_id;
    let actor = &AuditActor::principal(fx.human_id);

    // ④ 非 pending：responded / consumed / cancelled 一律 409。
    for forced in ["responded", "consumed", "cancelled"] {
        sqlx::query("UPDATE delivery.task_interaction SET status = $2 WHERE id = $1")
            .bind(reported.interaction_id)
            .bind(forced)
            .execute(&pool)
            .await
            .unwrap();
        let err = submit_interaction_response(&pool, &sub, actor).await.unwrap_err();
        assert!(matches!(err, pm_application::interaction::InteractionError::Conflict(_)));
    }
    // ⑤ deadline（保持 pending，清扫间隙竞态兜底）。
    sqlx::query(
        "UPDATE delivery.task_interaction SET status = 'pending', deadline_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(reported.interaction_id)
    .execute(&pool)
    .await
    .unwrap();
    let err = submit_interaction_response(&pool, &sub, actor).await.unwrap_err();
    assert!(matches!(err, pm_application::interaction::InteractionError::Conflict(_)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn acp451_payload_conformance(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-451").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    let mut sub = submit_input(&fx, reported.interaction_id, "idem-1", json!({ "decision": "hack" }));
    sub.interaction_id = reported.interaction_id;
    let actor = &AuditActor::principal(fx.human_id);
    let err = submit_interaction_response(&pool, &sub, actor).await.unwrap_err();
    assert!(matches!(err, pm_application::interaction::InteractionError::Validation(_)));
    // 合法 decision 放行。
    sub.response_payload = json!({ "decision": "approve" });
    submit_interaction_response(&pool, &sub, actor).await.unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn acp452_453_idempotent_replay_and_single_response(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-452").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    let mut sub = submit_input(&fx, reported.interaction_id, "idem-1", json!({ "decision": "approve" }));
    sub.interaction_id = reported.interaction_id;
    let actor = &AuditActor::principal(fx.human_id);
    let first = submit_interaction_response(&pool, &sub, actor).await.unwrap();
    let replay = submit_interaction_response(&pool, &sub, actor).await.unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.response_id, first.response_id);
    // §9.3：不同 key 的第二次回复 → 409（无静默覆盖）。
    let mut sub2 = submit_input(&fx, reported.interaction_id, "idem-2", json!({ "decision": "approve" }));
    sub2.interaction_id = reported.interaction_id;
    let err = submit_interaction_response(&pool, &sub2, actor).await.unwrap_err();
    assert!(matches!(err, pm_application::interaction::InteractionError::Conflict(_)));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.interaction_response")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn acp455_frozen_does_not_block_submission(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-455").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    sqlx::query(
        "UPDATE delivery.task_interaction SET continuation_start_frozen_at = now() WHERE id = $1",
    )
    .bind(reported.interaction_id)
    .execute(&pool)
    .await
    .unwrap();
    let mut sub = submit_input(&fx, reported.interaction_id, "idem-1", json!({ "decision": "approve" }));
    sub.interaction_id = reported.interaction_id;
    // R-ACP-7：冻结只挡启动，不挡提交。
    submit_interaction_response(&pool, &sub, &AuditActor::principal(fx.human_id))
        .await
        .unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn acp456_cross_project_reads_as_not_found(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-456").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    let other = claimed_run(&pool, "ACP-456b").await;
    let mut sub = submit_input(&other, reported.interaction_id, "idem-1", json!({ "content": "ok" }));
    sub.project_id = other.project_id;
    sub.interaction_id = reported.interaction_id;
    let err = submit_interaction_response(&pool, &sub, &AuditActor::principal(other.human_id))
        .await
        .unwrap_err();
    assert!(matches!(err, pm_application::interaction::InteractionError::NotFound));
}

#[sqlx::test(migrations = "../../migrations")]
async fn f5_comm_overview_counts_shielded_pending_unconsumed(pool: PgPool) {
    use pm_application::comm_overview::load_comm_overview;
    let fx = claimed_run(&pool, "ACP-F5").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    let o = load_comm_overview(&pool).await.unwrap();
    assert_eq!(o.interaction_shielded_runs, 1);
    assert_eq!(o.interaction_open_chains, 1);
    assert_eq!(o.interaction_unconsumed_total, 0, "pending ≠ unconsumed");
    assert!(o.worker_command_pending_total >= 1, "terminate command pending");
}

/// ACP-448（R-3/V1.2 §1.3）：execution_mode='human' 的 Run 调用
/// pm_report_interaction → fail-closed 拒绝，且零副作用
///（不产生 Interaction、不产生 terminate 命令）。
/// Model A 的等待走既有 awaiting_decision/task_waiting 入口。
#[sqlx::test(migrations = "../../migrations")]
async fn acp448_human_mode_run_cannot_report_interaction(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-448").await;
    sqlx::query("UPDATE delivery.task_run SET execution_mode = 'human' WHERE id = $1")
        .bind(fx.run_id)
        .execute(&pool)
        .await
        .unwrap();

    let err = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("spawn-mode"),
        "rejection must name the admission rule (got: {msg})"
    );
    // 零副作用：无 Interaction、无命令。
    let interactions: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.task_interaction")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(interactions, 0, "no Interaction may be created on a human run");
    let commands: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.worker_command")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(commands, 0, "no terminate command may be created on a human run");
    // 调度保护未被触碰。
    assert_eq!(run_field(&pool, fx.run_id, "scheduling_shield").await, "none");
    // Task 仍为 claimed（未转 awaiting_decision）。
    assert_eq!(task_status(&pool, fx.task_id).await, "claimed");
}

/// ACP-450/451 幂等面（R-4）：continue acked 的重复回执 = replayed 且状态稳定
///（Task 保持 running、等待行保持 resolve、Interaction 保持 responded 未 consumed）。
#[sqlx::test(migrations = "../../migrations")]
async fn acp450_repeated_continue_ack_is_idempotent(pool: PgPool) {
    let fx = claimed_run(&pool, "ACP-450").await;
    let actor = AuditActor::dispatch();
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();

    // terminate 先回执（关闭情形 6 窗口）。
    let polled = cmd::poll_commands(&pool, fx.project_id, fx.run_id, "w1").await.unwrap();
    assert_eq!(polled.len(), 1);
    cmd::ack_command(
        &pool,
        fx.project_id,
        polled[0].command_id,
        "w1",
        cmd::AckOutcome::Acked,
        Some(json!({ "exit_outcome": "Killed", "pid": 1 })),
        None,
    )
    .await
    .unwrap();

    // 回复持久化 + 下发 continue。
    record_response_persisted(
        &pool,
        fx.project_id,
        reported.interaction_id,
        json!({ "response_id": "resp-1" }),
        &actor,
    )
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let continue_id = cmd::issue_continue_command(
        &mut tx,
        fx.project_id,
        fx.task_id,
        fx.run_id,
        reported.interaction_id,
        None,
        "w1",
        fx.lease_generation,
        json!({ "decision": "approve" }),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // 领取并回执（INPUT_INJECTED）→ R-4 执行恢复。
    let polled2 = cmd::poll_commands(&pool, fx.project_id, fx.run_id, "w1").await.unwrap();
    assert_eq!(polled2.len(), 1);
    cmd::ack_command(
        &pool,
        fx.project_id,
        continue_id,
        "w1",
        cmd::AckOutcome::Acked,
        Some(json!({ "session_created": true, "input_injected": true })),
        None,
    )
    .await
    .unwrap();
    assert_eq!(task_status(&pool, fx.task_id).await, "running");
    assert_eq!(open_waiting(&pool, fx.task_id).await, 0);

    // 重复回执（网络重试/重投后重复 ack）→ replayed，状态不变。
    let replay = cmd::ack_command(
        &pool,
        fx.project_id,
        continue_id,
        "w1",
        cmd::AckOutcome::Acked,
        Some(json!({ "session_created": true, "input_injected": true })),
        None,
    )
    .await
    .unwrap();
    assert!(replay.replayed);
    assert_eq!(task_status(&pool, fx.task_id).await, "running");
    assert_eq!(open_waiting(&pool, fx.task_id).await, 0);
    let (status, chain_open): (String, bool) = sqlx::query_as(
        "SELECT status, chain_open FROM delivery.task_interaction WHERE id = $1",
    )
    .bind(reported.interaction_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "responded", "interaction must stay responded after ack replay");
    assert!(!chain_open);
}
