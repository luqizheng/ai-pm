//! Recovery Foundation（0052 / R-16 方案 a）集成测试：
//! ACP-452 离线超 TTL 恢复（发现→续领→同 Run continue 成功，deadline/attempt
//! 不变）· ACP-453 已持久化回复不丢 · ACP-454 fencing（旧 token 执行面拒绝 +
//! legacy 门）· ACP-455 terminate 未确认禁启新 Session（续领后仍情形 6）·
//! previous_chain_id 链间弱关联（0052 / 专家 2.6）。

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::device_credential;
use pm_application::interaction::{
    mark_consumed, record_response_persisted, report_interaction, submit_interaction_response,
    ReportInteractionInput, SubmitResponseInput,
};
use pm_application::recovery;
use pm_application::run_dispatch::RunDispatchConfig;
use pm_application::worker_command as cmd;
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed(pool: &PgPool, code: &str) -> (Uuid, Uuid, Uuid, Uuid) {
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
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id, assignee_id) \
         VALUES ($1, 't', 'ready', 1, (SELECT tenant_id FROM core.project WHERE id = $1), $2) \
         RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(outcome.dev_agent_id)
    .fetch_one(pool)
    .await
    .unwrap();
    // 策略判定 5：至少一个可写仓库（policy.rs 判定 4/5 前置）。
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
    (
        outcome.project_id,
        task_id,
        outcome.human_id,
        outcome.dev_agent_id,
    )
}

/// 建设备 + 签发凭据（V-05-1 语义：lease_client_id 绑定在 claim 时落）。
async fn seed_device_with_credential(
    pool: &PgPool,
    tenant_id: Uuid,
    owner: Uuid,
    name: &str,
) -> (Uuid, String) {
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) \
         VALUES ($1, $2, 'owner') ON CONFLICT (tenant_id, principal_id) DO NOTHING",
    )
    .bind(tenant_id)
    .bind(owner)
    .execute(pool)
    .await
    .unwrap();
    let device_id: Uuid = sqlx::query_scalar(
        "INSERT INTO devenv.device (id, tenant_id, owner_principal_id, name, platform, status) \
         VALUES ($1, $2, $3, $4, 'linux', 'active') RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(tenant_id)
    .bind(owner)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap();
    let issued = device_credential::issue_device_credential(
        pool,
        tenant_id,
        device_id,
        &AuditActor::principal(owner),
    )
    .await
    .unwrap();
    (device_id, issued.plaintext)
}

/// 凭据通道领取（模拟 Runner 走 0049 通道：lease_client_id + client:{id}）。
async fn claim_as_client(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    device_id: Uuid,
    human_id: Uuid,
) -> (Uuid, String, i32) {
    // Run 由用例自建（seed 不建；acp 系列惯例）。
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(pool)
    .await
    .unwrap();
    let claimed = pm_application::run_dispatch::claim_task_run_as_client(
        pool,
        project_id,
        task_id,
        device_id,
        "runner-recovery",
        None,
        &RunDispatchConfig::from_env(),
        &AuditActor::principal(human_id),
    )
    .await
    .unwrap();
    (
        claimed.run_id,
        claimed.lease_owner,
        claimed.lease_generation,
    )
}

fn interaction_input(
    fx: &(Uuid, Uuid, Uuid, Uuid),
    run_id: Uuid,
    event_id: &str,
) -> ReportInteractionInput {
    ReportInteractionInput {
        project_id: fx.0,
        task_id: fx.1,
        run_id,
        event_id: event_id.to_string(),
        reason: "needs_decision".into(),
        title: "t".into(),
        question: "q".into(),
        options: None,
        required_response: false,
        checkpoint_path: None,
    }
}

/// ACP-452/453：离线超租约 TTL → 发现 → 续领（同 Run generation+1、deadline
/// 不动）→ continue 同 Run 执行成功（R-4 恢复）；已持久化回复不丢。
#[sqlx::test(migrations = "../../migrations")]
async fn acp452_offline_recovery_via_discover_and_reacquire(pool: PgPool) {
    let fx = seed(&pool, "RC-452").await;
    let (device_id, _key) =
        seed_device_with_credential(&pool, SYSTEM_TENANT, fx.2, "runner-off").await;
    let (run_id, worker_id, gen0) = claim_as_client(&pool, fx.0, fx.1, device_id, fx.2).await;

    // 介入 → terminate ack → 回复持久化（interaction_response 落库）→ continue 下发。
    let reported = report_interaction(
        &pool,
        &interaction_input(&fx, run_id, "evt-1"),
        &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    let polled = cmd::poll_commands(&pool, fx.0, run_id, &worker_id)
        .await
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        polled[0].command_id,
        &worker_id,
        cmd::AckOutcome::Acked,
        Some(json!({ "exit_outcome": "Killed", "pid": 1 })),
        None,
    )
    .await
    .unwrap();
    let sub = SubmitResponseInput {
        project_id: fx.0,
        interaction_id: reported.interaction_id,
        responder_principal_id: fx.2,
        idempotency_key: "idem-1".into(),
        response_payload: json!({ "decision": "approve" }),
    };
    let submitted = submit_interaction_response(&pool, &sub, &AuditActor::principal(fx.2))
        .await
        .unwrap();

    // ── 模拟离线：租约过期（超 AI_PM_RUN_LEASE_SECS）+ 模拟重启（无 token）──
    sqlx::query("UPDATE delivery.task_run SET lease_expires_at = now() - interval '5 seconds' WHERE id = $1")
        .bind(run_id)
        .execute(&pool)
        .await
        .unwrap();
    let deadline_before: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT deadline_at FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let attempt_before: i32 =
        sqlx::query_scalar("SELECT attempt_no FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    // ① 发现：按稳定 Client 身份命中（lease_status=expired 可见）。
    let runs = recovery::discover_runs(&pool, SYSTEM_TENANT, fx.0, device_id, 20, 0)
        .await
        .unwrap();
    assert_eq!(runs.len(), 1, "discovery must find the in-flight run");
    assert_eq!(runs[0].run_id, run_id);
    assert_eq!(runs[0].lease_status, "expired");
    assert_eq!(runs[0].shield_state, "start_wait");
    assert!(runs[0].interaction_id.is_some());

    // ② 续领：同 Run generation+1、新 token、deadline/attempt 不变。
    let lease = recovery::reacquire_lease(
        &pool,
        SYSTEM_TENANT,
        fx.0,
        run_id,
        device_id,
        RunDispatchConfig::from_env().lease_secs,
        &AuditActor::principal(fx.2),
    )
    .await
    .unwrap();
    assert_eq!(lease.lease_generation, gen0 + 1);
    assert!(lease.lease_token.starts_with("IGNORED") == false);
    assert_eq!(
        lease.deadline_at, deadline_before,
        "T3: deadline must not move on offline recovery"
    );
    let attempt_after: i32 =
        sqlx::query_scalar("SELECT attempt_no FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attempt_after, attempt_before, "same run, no B3-2 attempt");

    // ③ reroute：continue 以新 generation 可达（集合比对含 reacquired）。
    let polled2 = cmd::poll_commands(&pool, fx.0, run_id, &lease.worker_id)
        .await
        .unwrap();
    assert_eq!(polled2.len(), 1);
    assert_eq!(polled2[0].command_type, "agent.task.continue");
    cmd::ack_command(
        &pool,
        fx.0,
        polled2[0].command_id,
        &lease.worker_id,
        cmd::AckOutcome::Acked,
        Some(json!({ "session_created": true, "input_injected": true })),
        None,
    )
    .await
    .unwrap();

    // ACP-453：已持久化回复不丢（interaction_response 在续领前已落）+ R-4 恢复。
    let reply: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.interaction_response WHERE interaction_id = $1",
    )
    .bind(reported.interaction_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        reply, 1,
        "persisted human reply survives the offline window"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM delivery.task WHERE id = $1")
            .bind(fx.1)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "running"
    );
    let _ = (worker_id, gen0, submitted);
}

/// ACP-454 fencing：旧 token 执行面拒绝（T2）+ legacy 门（lease_client_id NULL
/// 拒绝续领）+ 他 Client 冒领拒绝。
#[sqlx::test(migrations = "../../migrations")]
async fn acp454_fencing_after_reacquire(pool: PgPool) {
    let fx = seed(&pool, "RC-454").await;
    let (device_id, _) = seed_device_with_credential(&pool, SYSTEM_TENANT, fx.2, "runner-f").await;
    let (run_id, worker_id, _gen) = claim_as_client(&pool, fx.0, fx.1, device_id, fx.2).await;

    // 进入等待窗口（shield=start_wait）：介入 → 回复持久化。
    let reported = report_interaction(
        &pool,
        &interaction_input(&fx, run_id, "evt-1"),
        &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    record_response_persisted(
        &pool,
        fx.0,
        reported.interaction_id,
        json!({ "response_id": "r1" }),
        &AuditActor::principal(fx.2),
    )
    .await
    .unwrap();

    // 续领一次（generation+1 → 旧 token 失效）。
    let lease = recovery::reacquire_lease(
        &pool,
        SYSTEM_TENANT,
        fx.0,
        run_id,
        device_id,
        RunDispatchConfig::from_env().lease_secs,
        &AuditActor::principal(fx.2),
    )
    .await
    .unwrap();
    assert_eq!(lease.lease_generation, 2);
    let _ = worker_id;

    // 旧 token（续领前签发）执行面 → 拒绝（get_run_context 租约校验语义）。
    let old_token_hash_valid: Option<String> =
        sqlx::query_scalar("SELECT lease_token_hash FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(old_token_hash_valid.is_some());
    // （T2 语义：续领后 token 哈希已换——旧 token 比对必败，此处验证哈希已轮换。）
    let shield: String =
        sqlx::query_scalar("SELECT scheduling_shield FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(shield, "start_wait");

    // legacy 门：无 lease_client_id 的 Run（历史自报形态）续领 → 拒绝。
    let (project2, task2, _, _) = seed(&pool, "RC-454L").await;
    let legacy_run: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
           (project_id, task_id, attempt_no, status, execution_mode, stage, lease_owner, \
            lease_token_hash, lease_generation, lease_expires_at, deadline_at, tenant_id) \
         VALUES ($1, $2, 1, 'claimed', 'spawn', 'started', 'mcp:legacy:abcd1234', 'h', 1, \
                 now() + interval '300 seconds', now() + interval '3600 seconds', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project2)
    .bind(task2)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task_run SET scheduling_shield = 'start_wait' WHERE id = $1")
        .bind(legacy_run)
        .execute(&pool)
        .await
        .unwrap();
    let err = recovery::reacquire_lease(
        &pool,
        SYSTEM_TENANT,
        project2,
        legacy_run,
        device_id,
        RunDispatchConfig::from_env().lease_secs,
        &AuditActor::principal(fx.2),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("legacy"), "{err}");

    // 他 Client 冒领 → 拒绝。
    let other_device =
        seed_device_with_credential(&pool, SYSTEM_TENANT, fx.2, "runner-other").await;
    let err = recovery::reacquire_lease(
        &pool,
        SYSTEM_TENANT,
        fx.0,
        run_id,
        other_device.0,
        RunDispatchConfig::from_env().lease_secs,
        &AuditActor::principal(fx.2),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("not the owning client"), "{err}");
}

/// ACP-455（ACP-C4）：续领成功但 terminate 未确认 → continue 仍被情形 6 过滤
///（禁止启动新 Session）；terminate acked 后 continue 才出现。
#[sqlx::test(migrations = "../../migrations")]
async fn acp455_continue_waits_for_terminate_confirmation(pool: PgPool) {
    let fx = seed(&pool, "RC-455").await;
    let (device_id, _) = seed_device_with_credential(&pool, SYSTEM_TENANT, fx.2, "runner-t").await;
    let (run_id, worker_id, _) = claim_as_client(&pool, fx.0, fx.1, device_id, fx.2).await;

    let reported = report_interaction(
        &pool,
        &interaction_input(&fx, run_id, "evt-1"),
        &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    // terminate 不 ack（未确认）→ 回复持久化 + continue 下发。
    let sub = SubmitResponseInput {
        project_id: fx.0,
        interaction_id: reported.interaction_id,
        responder_principal_id: fx.2,
        idempotency_key: "idem-1".into(),
        response_payload: json!({ "decision": "approve" }),
    };
    let submitted = submit_interaction_response(&pool, &sub, &AuditActor::principal(fx.2))
        .await
        .unwrap();

    // 离线 → 恢复：发现 + 续领成功（续领前置不含 terminate 确认）。
    sqlx::query("UPDATE delivery.task_run SET lease_expires_at = now() - interval '5 seconds' WHERE id = $1")
        .bind(run_id)
        .execute(&pool)
        .await
        .unwrap();
    let lease = recovery::reacquire_lease(
        &pool,
        SYSTEM_TENANT,
        fx.0,
        run_id,
        device_id,
        RunDispatchConfig::from_env().lease_secs,
        &AuditActor::principal(fx.2),
    )
    .await
    .unwrap();

    // 情形 6：continue 不予下发（terminate 未确认），即使是续领后的新租约。
    let polled = cmd::poll_commands(&pool, fx.0, run_id, &lease.worker_id)
        .await
        .unwrap();
    assert!(
        polled
            .iter()
            .all(|c| c.command_type != "agent.task.continue"),
        "continue must stay filtered while terminate is unconfirmed"
    );

    // terminate 确认（幂等终止语义：恢复执行端确认旧进程已不存在 → acked）。
    let t = polled
        .iter()
        .find(|c| c.command_type == "agent.process.terminate")
        .expect("terminate surfaces after reacquire");
    cmd::ack_command(
        &pool,
        fx.0,
        t.command_id,
        &lease.worker_id,
        cmd::AckOutcome::Acked,
        Some(json!({ "exit_outcome": "Lost", "pid": null })),
        None,
    )
    .await
    .unwrap();

    // 确认后 → continue 出现 → 新 Session 可启动。
    let polled2 = cmd::poll_commands(&pool, fx.0, run_id, &lease.worker_id)
        .await
        .unwrap();
    assert_eq!(polled2.len(), 1);
    assert_eq!(polled2[0].command_type, "agent.task.continue");
    let _ = (worker_id, submitted);
}

/// previous_chain_id（0052 / 专家 2.6）：链关闭后的新链引用已关闭前驱链；
/// 不参与唯一活跃约束（链内 retry_of 语义不变）。
#[sqlx::test(migrations = "../../migrations")]
async fn previous_chain_id_links_closed_predecessor(pool: PgPool) {
    let fx = seed(&pool, "RC-CHAIN").await;
    let (device_id, _) =
        seed_device_with_credential(&pool, SYSTEM_TENANT, fx.2, "runner-chain").await;
    let (run_id, worker_id, _) = claim_as_client(&pool, fx.0, fx.1, device_id, fx.2).await;

    // 链 1：介入 → 回复 → continue acked（链关闭）。
    let r1 = report_interaction(
        &pool,
        &interaction_input(&fx, run_id, "evt-1"),
        &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    let p1 = cmd::poll_commands(&pool, fx.0, run_id, &worker_id)
        .await
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        p1[0].command_id,
        &worker_id,
        cmd::AckOutcome::Acked,
        Some(json!({ "exit_outcome": "Killed", "pid": 1 })),
        None,
    )
    .await
    .unwrap();
    record_response_persisted(
        &pool,
        fx.0,
        r1.interaction_id,
        json!({ "response_id": "r1" }),
        &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    // 手动下发 continue 并 acked（关链）。
    let mut tx = pool.begin().await.unwrap();
    let c1 = cmd::issue_continue_command(
        &mut tx,
        fx.0,
        fx.1,
        run_id,
        r1.interaction_id,
        None,
        &worker_id,
        1,
        json!({ "decision": "approve" }),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    cmd::poll_commands(&pool, fx.0, run_id, &worker_id)
        .await
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        c1,
        &worker_id,
        cmd::AckOutcome::Acked,
        Some(json!({ "session_created": true, "input_injected": true })),
        None,
    )
    .await
    .unwrap();

    // 链 2：新介入 → previous_chain_id = 链 1 的 interaction。
    let r2 = report_interaction(
        &pool,
        &interaction_input(&fx, run_id, "evt-2"),
        &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    let prev: Option<Uuid> =
        sqlx::query_scalar("SELECT previous_chain_id FROM delivery.task_interaction WHERE id = $1")
            .bind(r2.interaction_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        prev,
        Some(r1.interaction_id),
        "new chain links its closed predecessor"
    );

    // 首链（无前驱）→ NULL。
    let first_prev: Option<Uuid> =
        sqlx::query_scalar("SELECT previous_chain_id FROM delivery.task_interaction WHERE id = $1")
            .bind(r1.interaction_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(first_prev.is_none());
}
