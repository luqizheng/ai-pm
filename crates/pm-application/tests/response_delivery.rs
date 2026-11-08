//! ACP-04 Delivery 持久化（0050）集成测试：
//! ACP-470 回复三写一账 · ACP-471 失败重投复用原行（attempt+1）·
//! ACP-472 消费事实持久化（幂等、迟到不改变恢复）· ACP-473 generation 恒 1
//! 与工具契约零变化 · unconsumed 指标切表。

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::interaction::{
    mark_consumed, record_response_persisted, report_interaction, submit_interaction_response,
    ReportInteractionInput, SubmitResponseInput,
};
use pm_application::run_dispatch::RunDispatchConfig;
use pm_application::worker_command as cmd;
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

async fn seed_claimed(pool: &PgPool, code: &str) -> (Uuid, Uuid, Uuid, Uuid) {
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
         VALUES ($1, 't', 'claimed', 1, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(outcome.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
           (project_id, task_id, attempt_no, status, execution_mode, stage, lease_owner, \
            lease_token_hash, lease_generation, lease_expires_at, deadline_at, tenant_id) \
         VALUES ($1, $2, 1, 'claimed', 'spawn', 'started', 'client:dbg', 'hash', 1, \
                 now() + interval '300 seconds', now() + interval '3600 seconds', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (outcome.project_id, task_id, run_id, outcome.human_id)
}

fn input(fx: &(Uuid, Uuid, Uuid, Uuid), event_id: &str) -> ReportInteractionInput {
    ReportInteractionInput {
        project_id: fx.0,
        task_id: fx.1,
        run_id: fx.2,
        event_id: event_id.to_string(),
        reason: "needs_decision".into(),
        title: "t".into(),
        question: "q".into(),
        options: None,
        required_response: false,
        checkpoint_path: None,
    }
}

async fn delivery_of(pool: &PgPool, response_id: Uuid) -> (Uuid, String, i32) {
    let row: (Uuid, String, i32) = sqlx::query_as(
        "SELECT id, status, attempt_count FROM delivery.interaction_response_delivery \
          WHERE response_id = $1",
    )
    .bind(response_id)
    .fetch_one(pool)
    .await
    .unwrap();
    row
}

/// ACP-470：回复持久化 = 同事务三写（response + command + delivery 一账）。
#[sqlx::test(migrations = "../../migrations")]
async fn acp470_submit_creates_single_delivery_row(pool: PgPool) {
    let fx = seed_claimed(&pool, "RD-470").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();

    // 关闭情形 6 窗口（terminate acked）。
    let polled = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    assert_eq!(polled.len(), 1);
    cmd::ack_command(
        &pool,
        fx.0,
        polled[0].command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(json!({ "exit_outcome": "Killed", "pid": 1 })),
        None,
    )
    .await
    .unwrap();

    let sub = SubmitResponseInput {
        project_id: fx.0,
        interaction_id: reported.interaction_id,
        responder_principal_id: fx.3,
        idempotency_key: "idem-1".into(),
        response_payload: json!({ "decision": "approve" }),
    };
    let submitted = submit_interaction_response(&pool, &sub, &AuditActor::principal(fx.3))
        .await
        .unwrap();

    // 三写：delivery 一账 + response.continue_command_id 镜像一致。
    let (delivery_id, status, attempt) = delivery_of(&pool, submitted.response_id).await;
    assert_eq!(status, "pending");
    assert_eq!(attempt, 0);
    let (gen, cont_cmd): (i32, Uuid) = sqlx::query_as(
        "SELECT continuation_generation, continue_command_id \
          FROM delivery.interaction_response_delivery WHERE id = $1",
    )
    .bind(delivery_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(gen, 1, "ACP-473: generation stays 1 until ACP-05");
    assert_eq!(cont_cmd, submitted.continue_command_id.unwrap());

    // 一回复一账：UNIQUE(response_id)。
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.interaction_response_delivery WHERE response_id = $1",
    )
    .bind(submitted.response_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

/// ACP-471：重投复用原行——claim 复领（同 worker 幂等重领）→ delivering
/// attempt 递增 → delivered 同 delivery_id；ack(failed) → failed + shield
/// 不释放（failed 为终态：重投经 claim 复领或回复重提交机制，不新建行）。
#[sqlx::test(migrations = "../../migrations")]
async fn acp471_failed_redelivery_reuses_row(pool: PgPool) {
    let fx = seed_claimed(&pool, "RD-471").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    let polled = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        polled[0].command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(json!({ "exit_outcome": "Killed", "pid": 1 })),
        None,
    )
    .await
    .unwrap();

    let sub = SubmitResponseInput {
        project_id: fx.0,
        interaction_id: reported.interaction_id,
        responder_principal_id: fx.3,
        idempotency_key: "idem-1".into(),
        response_payload: json!({ "decision": "approve" }),
    };
    let submitted = submit_interaction_response(&pool, &sub, &AuditActor::principal(fx.3))
        .await
        .unwrap();
    let (delivery_id, _) = {
        let (id, status, _) = delivery_of(&pool, submitted.response_id).await;
        assert_eq!(status, "pending");
        (id, status)
    };

    // 领取（claim）→ delivering + attempt 1。
    let polled2 = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    assert_eq!(polled2.len(), 1);
    let continue_id = polled2[0].command_id;
    let (_, status, attempt) = delivery_of(&pool, submitted.response_id).await;
    assert_eq!(status, "delivering");
    assert_eq!(attempt, 1);

    // 同 worker 幂等重领（模拟执行端重启后的重投路径）→ attempt 2、同 command_id。
    let polled3 = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    assert_eq!(polled3.len(), 1);
    assert_eq!(polled3[0].command_id, continue_id, "same command_id reused");
    let (_, status, attempt) = delivery_of(&pool, submitted.response_id).await;
    assert_eq!(status, "delivering");
    assert_eq!(attempt, 2, "attempt_count increments per delivery attempt");

    // 本次成功 → delivered（同 delivery_id，任务恢复 R-4 同事务）。
    cmd::ack_command(
        &pool,
        fx.0,
        continue_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(json!({ "session_created": true, "input_injected": true })),
        None,
    )
    .await
    .unwrap();
    let (id2, status2, _) = delivery_of(&pool, submitted.response_id).await;
    assert_eq!(id2, delivery_id, "row reused, never recreated");
    assert_eq!(status2, "delivered");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM delivery.task WHERE id = $1")
            .bind(fx.1)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "running"
    );

    // ── failed 记账面（独立链：ACP-451 链关闭后允许新介入）──────────────
    let reported2 = report_interaction(&pool, &input(&fx, "evt-2"), &AuditActor::dispatch())
        .await
        .unwrap();
    let polled4 = cmd::poll_commands(&pool, fx.0, fx.2, &"client:dbg")
        .await
        .unwrap();
    let t2 = polled4
        .iter()
        .find(|c| c.command_type == "agent.process.terminate")
        .expect("new terminate for the second chain");
    cmd::ack_command(
        &pool,
        fx.0,
        t2.command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(json!({ "exit_outcome": "Killed", "pid": 2 })),
        None,
    )
    .await
    .unwrap();
    let sub2 = SubmitResponseInput {
        project_id: fx.0,
        interaction_id: reported2.interaction_id,
        responder_principal_id: fx.3,
        idempotency_key: "idem-2".into(),
        response_payload: json!({ "decision": "approve-2" }),
    };
    let submitted2 = submit_interaction_response(&pool, &sub2, &AuditActor::principal(fx.3))
        .await
        .unwrap();
    let polled5 = cmd::poll_commands(&pool, fx.0, fx.2, &"client:dbg")
        .await
        .unwrap();
    let c2 = polled5
        .iter()
        .find(|c| c.command_type == "agent.task.continue")
        .expect("continue for the second chain");
    cmd::ack_command(
        &pool,
        fx.0,
        c2.command_id,
        "client:dbg",
        cmd::AckOutcome::Failed,
        Some(json!({ "session_created": true })),
        Some("injection_failed".into()),
    )
    .await
    .unwrap();
    let (_, status_failed, _) = delivery_of(&pool, submitted2.response_id).await;
    assert_eq!(status_failed, "failed");
    let shield: String =
        sqlx::query_scalar("SELECT scheduling_shield FROM delivery.task_run WHERE id = $1")
            .bind(fx.2)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        shield, "start_wait",
        "failed injection must not release the shield"
    );
}

/// ACP-472：消费事实持久化——mark_consumed 落 delivery.consumed；迟到/重复
/// 幂等；Task 已在 acked 恢复（R-4）不受消费回执影响。
#[sqlx::test(migrations = "../../migrations")]
async fn acp472_consumption_persists_delivery_fact(pool: PgPool) {
    let fx = seed_claimed(&pool, "RD-472").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    let polled = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        polled[0].command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(json!({ "exit_outcome": "Killed", "pid": 1 })),
        None,
    )
    .await
    .unwrap();
    let sub = SubmitResponseInput {
        project_id: fx.0,
        interaction_id: reported.interaction_id,
        responder_principal_id: fx.3,
        idempotency_key: "idem-1".into(),
        response_payload: json!({ "decision": "approve" }),
    };
    let submitted = submit_interaction_response(&pool, &sub, &AuditActor::principal(fx.3))
        .await
        .unwrap();
    let polled2 = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        polled2[0].command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(json!({ "session_created": true, "input_injected": true })),
        None,
    )
    .await
    .unwrap();
    let (delivery_id, status, _) = delivery_of(&pool, submitted.response_id).await;
    assert_eq!(status, "delivered");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM delivery.task WHERE id = $1")
            .bind(fx.1)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "running",
        "R-4: execution resumed at acked"
    );

    // 消费回执 → delivery consumed（持久化事实）+ Interaction consumed。
    mark_consumed(
        &pool,
        fx.0,
        reported.interaction_id,
        Some(delivery_id),
        &json!({ "kinds": ["initial_input_injected", "process_alive"] }),
        &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    let (d_status, i_status): (String, String) = sqlx::query_as(
        "SELECT d.status, i.status FROM delivery.interaction_response_delivery d \
           JOIN delivery.task_interaction i ON i.id = d.interaction_id \
          WHERE d.id = $1",
    )
    .bind(delivery_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(d_status, "consumed");
    assert_eq!(i_status, "consumed");

    // 迟到/重复回执：幂等（同 delivery_id 重复 → false，状态不变）。
    let again = mark_consumed(
        &pool,
        fx.0,
        reported.interaction_id,
        Some(delivery_id),
        &json!({ "kinds": ["initial_input_injected", "process_alive"] }),
        &AuditActor::dispatch(),
    )
    .await
    .unwrap_err();
    let _ = again; // Interaction 已 consumed → 服务层 Conflict（幂等面在上游）。
    let (_, status_now, _) = delivery_of(&pool, submitted.response_id).await;
    assert_eq!(
        status_now, "consumed",
        "repeated receipt must not change the fact"
    );
}

/// ACP-473 + unconsumed 指标切表：pending 不计数 / delivering·delivered 计数 /
/// consumed 归零。
#[sqlx::test(migrations = "../../migrations")]
async fn acp473_unconsumed_metric_reads_delivery_table(pool: PgPool) {
    use pm_application::comm_overview::load_comm_overview;
    let fx = seed_claimed(&pool, "RD-473").await;
    let reported = report_interaction(&pool, &input(&fx, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    let polled = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        polled[0].command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(json!({ "exit_outcome": "Killed", "pid": 1 })),
        None,
    )
    .await
    .unwrap();

    // 提交后 delivery=pending → 未消费计数 0（pending 不是"已投未消费"）。
    let sub = SubmitResponseInput {
        project_id: fx.0,
        interaction_id: reported.interaction_id,
        responder_principal_id: fx.3,
        idempotency_key: "idem-1".into(),
        response_payload: json!({ "decision": "approve" }),
    };
    let submitted = submit_interaction_response(&pool, &sub, &AuditActor::principal(fx.3))
        .await
        .unwrap();
    let o1 = load_comm_overview(&pool).await.unwrap();
    assert_eq!(o1.interaction_unconsumed_total, 0);

    // 领取 → delivering → 计数 1。
    let polled2 = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    assert_eq!(polled2.len(), 1);
    let o2 = load_comm_overview(&pool).await.unwrap();
    assert_eq!(o2.interaction_unconsumed_total, 1);

    // acked → delivered → 计数仍 1（N-6 稳态告警面）。
    cmd::ack_command(
        &pool,
        fx.0,
        polled2[0].command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(json!({ "session_created": true, "input_injected": true })),
        None,
    )
    .await
    .unwrap();
    let o3 = load_comm_overview(&pool).await.unwrap();
    assert_eq!(o3.interaction_unconsumed_total, 1);

    // 消费 → 归零。
    let (delivery_id, _, _) = delivery_of(&pool, submitted.response_id).await;
    mark_consumed(
        &pool,
        fx.0,
        reported.interaction_id,
        Some(delivery_id),
        &json!({ "kinds": ["initial_input_injected", "process_alive"] }),
        &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    let o4 = load_comm_overview(&pool).await.unwrap();
    assert_eq!(o4.interaction_unconsumed_total, 0);
}
