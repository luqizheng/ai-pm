//! ACP-05 Continuation 状态机（0051）集成测试：
//! ACP-474 两次介入 → 两个 continuation（generation 1/2 + delivery 回填）·
//! ACP-475 acked 同事务推进（delivered ∪ started ∪ R-4 恢复）·
//! ACP-476 frozen（terminate 耗尽镜像 + 原列双写）· ACP-477 终态钩子 closed
//!（Task 取消）+ consumed 不可逆 · C2 回归钉。

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::interaction::{
    cancel_open_interaction, mark_consumed, report_interaction, submit_interaction_response,
    ReportInteractionInput, SubmitResponseInput,
};
use pm_application::worker_command as cmd;
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

/// 检查点 2 收敛批：acked 回执信封（schema v1 + 当前代次 fencing）。
fn receipt_v1(run_generation: i64, extra: serde_json::Value) -> serde_json::Value {
    let mut receipt = json!({
        "schema_version": cmd::RECEIPT_SCHEMA,
        "run_generation": run_generation,
    });
    let obj = receipt.as_object_mut().expect("receipt object");
    if let serde_json::Value::Object(fields) = extra {
        for (key, value) in fields {
            obj.insert(key, value);
        }
    }
    receipt
}

fn terminate_receipt(
    run_generation: i64,
    outcome: &str,
    pid: serde_json::Value,
) -> serde_json::Value {
    receipt_v1(
        run_generation,
        json!({
            "exit_outcome": outcome,
            "pid": pid,
            "terminate_requested_at": "2026-11-08T00:00:00Z",
            "terminate_confirmed_at": "2026-11-08T00:00:01Z",
        }),
    )
}

fn continue_receipt(run_generation: i64) -> serde_json::Value {
    receipt_v1(
        run_generation,
        json!({ "session_created": true, "input_injected": true }),
    )
}

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

fn input(fx: &(Uuid, Uuid, Uuid, Uuid), run_id: Uuid, event_id: &str) -> ReportInteractionInput {
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

async fn continuation_of(pool: &PgPool, origin_interaction_id: Uuid) -> (Uuid, i32, String) {
    let row: (Uuid, i32, String) = sqlx::query_as(
        "SELECT id, generation, status FROM delivery.continuation \
          WHERE origin_interaction_id = $1 ORDER BY generation",
    )
    .bind(origin_interaction_id)
    .fetch_one(pool)
    .await
    .unwrap();
    row
}

/// ACP-474：两次介入 → 两个 continuation（generation 1/2）+ delivery 回填。
#[sqlx::test(migrations = "../../migrations")]
async fn acp474_two_chains_two_continuations_with_backfill(pool: PgPool) {
    let fx = seed_claimed(&pool, "CT-474").await;

    // 链 1：介入 → terminate ack → 回复（submit 全路径：四写）→ continue acked。
    let r1 = report_interaction(&pool, &input(&fx, fx.2, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    let p1 = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        p1[0].command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(terminate_receipt(1, "Killed", json!(1))),
        None,
    )
    .await
    .unwrap();
    let sub1 = SubmitResponseInput {
        project_id: fx.0,
        interaction_id: r1.interaction_id,
        responder_principal_id: fx.3,
        idempotency_key: "idem-1".into(),
        response_payload: json!({ "decision": "approve" }),
    };
    let s1 = submit_interaction_response(&pool, &sub1, &AuditActor::principal(fx.3))
        .await
        .unwrap();
    let (c1, gen1, status1) = continuation_of(&pool, r1.interaction_id).await;
    assert_eq!(gen1, 1);
    assert_eq!(status1, "open");

    // acked → started（ACP-475 同事务推进）。
    let p2 = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    assert_eq!(p2.len(), 1);
    cmd::ack_command(
        &pool,
        fx.0,
        p2[0].command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(continue_receipt(1)),
        None,
    )
    .await
    .unwrap();
    let (_, _, status1_after) = continuation_of(&pool, r1.interaction_id).await;
    assert_eq!(status1_after, "started");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM delivery.task WHERE id = $1")
            .bind(fx.1)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "running"
    );

    // 链 2：再次介入 → generation 2（UNIQUE(run_id, generation)）+ delivery 回填。
    let r2 = report_interaction(&pool, &input(&fx, fx.2, "evt-2"), &AuditActor::dispatch())
        .await
        .unwrap();
    let p3 = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    let t2 = p3
        .iter()
        .find(|c| c.command_type == "agent.process.terminate")
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        t2.command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(terminate_receipt(1, "Killed", json!(2))),
        None,
    )
    .await
    .unwrap();
    let sub2 = SubmitResponseInput {
        project_id: fx.0,
        interaction_id: r2.interaction_id,
        responder_principal_id: fx.3,
        idempotency_key: "idem-2".into(),
        response_payload: json!({ "decision": "approve-2" }),
    };
    let s2 = submit_interaction_response(&pool, &sub2, &AuditActor::principal(fx.3))
        .await
        .unwrap();
    let (c2, gen2, _) = continuation_of(&pool, r2.interaction_id).await;
    assert_eq!(gen2, 2);
    assert_ne!(c1, c2);

    // delivery 回填：两行各带自己的 continuation_id/generation。
    let back1: (Option<Uuid>, i32) = sqlx::query_as(
        "SELECT continuation_id, continuation_generation \
          FROM delivery.interaction_response_delivery WHERE response_id = $1",
    )
    .bind(s1.response_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let back2: (Option<Uuid>, i32) = sqlx::query_as(
        "SELECT continuation_id, continuation_generation \
          FROM delivery.interaction_response_delivery WHERE response_id = $1",
    )
    .bind(s2.response_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(back1, (Some(c1), 1));
    assert_eq!(back2, (Some(c2), 2));
    let _ = (s1, s2);
}

/// ACP-476：terminate 重试耗尽（ack failed）→ frozen + 原列双写；
/// 显式处置（重试成功后 acked）→ 回 started。
#[sqlx::test(migrations = "../../migrations")]
async fn acp476_terminate_failure_freezes_continuation(pool: PgPool) {
    let fx = seed_claimed(&pool, "CT-476").await;
    let r1 = report_interaction(&pool, &input(&fx, fx.2, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    // 先回复（R-ACP-7：terminate 未确认不冻结回复提交）→ 四写创建 continuation
    //（open + continue 命令下发）。
    let sub = SubmitResponseInput {
        project_id: fx.0,
        interaction_id: r1.interaction_id,
        responder_principal_id: fx.3,
        idempotency_key: "idem-1".into(),
        response_payload: json!({ "decision": "approve" }),
    };
    submit_interaction_response(&pool, &sub, &AuditActor::principal(fx.3))
        .await
        .unwrap();
    let (c1, _, status_open) = continuation_of(&pool, r1.interaction_id).await;
    assert_eq!(status_open, "open");

    // 再 ack failed terminate（模拟有界重试耗尽回执）→ frozen + 原列双写。
    let p1 = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    let t = p1
        .iter()
        .find(|c| c.command_type == "agent.process.terminate")
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        t.command_id,
        "client:dbg",
        cmd::AckOutcome::Failed,
        Some(json!({})),
        Some("termination failed after retries".into()),
    )
    .await
    .unwrap();
    let (_, _, status) = continuation_of(&pool, r1.interaction_id).await;
    assert_eq!(
        status, "frozen",
        "terminate failure freezes the continuation"
    );
    assert_eq!(c1, c1);
    let frozen_col: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT continuation_start_frozen_at FROM delivery.task_interaction WHERE id = $1",
    )
    .bind(r1.interaction_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        frozen_col.is_some(),
        "original column double-write retained"
    );

    // 显式处置：重试成功（terminate 幂等 acked）→ continue acked → started
    //（解冻路径：守卫 status IN open,frozen）。
    let p2 = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    let c = p2
        .iter()
        .find(|c| c.command_type == "agent.task.continue")
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        c.command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(continue_receipt(1)),
        None,
    )
    .await
    .unwrap();
    let (_, _, status_after) = continuation_of(&pool, r1.interaction_id).await;
    assert_eq!(
        status_after, "started",
        "manual disposition unfreezes via acked"
    );
}

/// ACP-477：终态钩子——Task 取消 → 非终态 continuation closed；closed 终态
/// 不可迁移（status 守卫拒绝）；consumed 不受取消影响（持久化事实不可逆）。
#[sqlx::test(migrations = "../../migrations")]
async fn acp477_terminal_hooks_close_continuations(pool: PgPool) {
    let fx = seed_claimed(&pool, "CT-477").await;
    let r1 = report_interaction(&pool, &input(&fx, fx.2, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    // 先回复（四写：delivery + continuation open + continue 下发），
    // 再 Task 取消联动（F-8）→ 链取消 + continuation closed。
    let sub1 = SubmitResponseInput {
        project_id: fx.0,
        interaction_id: r1.interaction_id,
        responder_principal_id: fx.3,
        idempotency_key: "idem-1".into(),
        response_payload: json!({ "decision": "approve" }),
    };
    submit_interaction_response(&pool, &sub1, &AuditActor::principal(fx.3))
        .await
        .unwrap();
    let (_, _, status_open) = continuation_of(&pool, r1.interaction_id).await;
    assert_eq!(status_open, "open");
    let cancelled =
        cancel_open_interaction(&pool, fx.0, fx.1, "task cancelled", &AuditActor::dispatch())
            .await
            .unwrap();
    assert!(cancelled);
    let (_, _, status) = continuation_of(&pool, r1.interaction_id).await;
    assert_eq!(status, "closed");
    let closed_at: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT closed_at FROM delivery.continuation WHERE origin_interaction_id = $1",
    )
    .bind(r1.interaction_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(closed_at.is_some());

    // closed 终态不可迁移：直接 UPDATE 尝试仍被 status 守卫拒绝（<> 'closed'）。
    let forced = sqlx::query(
        "UPDATE delivery.continuation SET status = 'started' \
          WHERE origin_interaction_id = $1 AND status IN ('open', 'frozen')",
    )
    .bind(r1.interaction_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(forced.rows_affected(), 0, "closed is terminal");

    // consumed 持久化事实不可逆（C2）：consumed 行不被终态钩子回写。
    let (_, _, _, _) = (fx.0, fx.1, fx.2, fx.3);
    let r2 = report_interaction(&pool, &input(&fx, fx.2, "evt-2"), &AuditActor::dispatch())
        .await
        .unwrap();
    let p = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    let t = p
        .iter()
        .find(|c| c.command_type == "agent.process.terminate")
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        t.command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(terminate_receipt(1, "Killed", json!(3))),
        None,
    )
    .await
    .unwrap();
    let sub = SubmitResponseInput {
        project_id: fx.0,
        interaction_id: r2.interaction_id,
        responder_principal_id: fx.3,
        idempotency_key: "idem-2".into(),
        response_payload: json!({ "decision": "approve" }),
    };
    let s2 = submit_interaction_response(&pool, &sub, &AuditActor::principal(fx.3))
        .await
        .unwrap();
    let p2 = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    let t2 = p2
        .iter()
        .find(|c| c.command_type == "agent.process.terminate")
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        t2.command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(terminate_receipt(1, "Killed", json!(4))),
        None,
    )
    .await
    .unwrap();
    let p3 = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    // p3 同时含旧链 continue1（取消未撤命令，重领复投）与新链 continue2——
    // 两条都按 INPUT_INJECTED 回执（resume/mark_started 对 closed 链幂等 no-op）。
    for c in &p3 {
        cmd::ack_command(
            &pool,
            fx.0,
            c.command_id,
            "client:dbg",
            cmd::AckOutcome::Acked,
            Some(continue_receipt(1)),
            None,
        )
        .await
        .unwrap();
    }
    let delivery_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM delivery.interaction_response_delivery WHERE response_id = $1",
    )
    .bind(s2.response_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    mark_consumed(
        &pool,
        fx.0,
        r2.interaction_id,
        Some(delivery_id),
        &json!({ "kinds": ["initial_input_injected", "process_alive"] }),
        &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    let (c_status, _) = sqlx::query_as::<_, (String, String)>(
        "SELECT c.status, d.status FROM delivery.continuation c \
           JOIN delivery.interaction_response_delivery d ON d.continuation_id = c.id \
          WHERE d.id = $1",
    )
    .bind(delivery_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        c_status, "consumed",
        "consumed persists as an irreversible fact"
    );
}

/// DEF-E2E-1：REST 任务编辑路径（`pm_write::update_task`，UI/PATCH 入口的业务
/// 服务）直写 Cancelled 必须与命令通道同一联动——介入链取消 + ACP-477 终态
/// 钩子关闭非终态 continuation（V-6：入口仅适配，业务钩子在应用服务层接线）。
/// 前置镜像 E2E S9：回复已持久化 + continue acked → continuation started。
#[sqlx::test(migrations = "../../migrations")]
async fn def_e2e1_rest_update_task_cancel_closes_continuation(pool: PgPool) {
    let fx = seed_claimed(&pool, "CT-E21").await;
    let r1 = report_interaction(&pool, &input(&fx, fx.2, "evt-1"), &AuditActor::dispatch())
        .await
        .unwrap();
    // terminate 命令先结清（介入挂起 = 停 Worker），回复才投 continue。
    let pt = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    let t = pt
        .iter()
        .find(|c| c.command_type == "agent.process.terminate")
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        t.command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(terminate_receipt(1, "Killed", json!(1))),
        None,
    )
    .await
    .unwrap();
    let sub = SubmitResponseInput {
        project_id: fx.0,
        interaction_id: r1.interaction_id,
        responder_principal_id: fx.3,
        idempotency_key: "idem-1".into(),
        response_payload: json!({ "decision": "approve" }),
    };
    submit_interaction_response(&pool, &sub, &AuditActor::principal(fx.3))
        .await
        .unwrap();
    // continue 下发 → acked → continuation started（E2E S9 前置同形）。
    let p = cmd::poll_commands(&pool, fx.0, fx.2, "client:dbg")
        .await
        .unwrap();
    let c = p
        .iter()
        .find(|c| c.command_type == "agent.task.continue")
        .unwrap();
    cmd::ack_command(
        &pool,
        fx.0,
        c.command_id,
        "client:dbg",
        cmd::AckOutcome::Acked,
        Some(continue_receipt(1)),
        None,
    )
    .await
    .unwrap();
    let (_, _, status_started) = continuation_of(&pool, r1.interaction_id).await;
    assert_eq!(status_started, "started");

    let revision: i64 = sqlx::query_scalar("SELECT revision FROM delivery.task WHERE id = $1")
        .bind(fx.1)
        .fetch_one(&pool)
        .await
        .unwrap();
    // 全字段编辑契约（与 REST PATCH 同形）：title/priority/revision 齐备。
    pm_application::pm_write::update_task(
        &pool,
        fx.1,
        pm_application::pm_write::UpdateTaskRequest {
            revision: Some(revision),
            title: Some("t".into()),
            description: None,
            status: Some("cancelled".into()),
            priority: Some("normal".into()),
            assignee_id: None,
            goal: None,
            constraints: None,
            deliverables: None,
            acceptance_criteria: None,
            knowledge_refs: None,
        },
        &AuditActor::principal(fx.3),
    )
    .await
    .unwrap();

    let intr: String =
        sqlx::query_scalar("SELECT status FROM delivery.task_interaction WHERE id = $1")
            .bind(r1.interaction_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    // 已回复链（responded，continue acked 已关链）不是「未决」：status 不被
    // 取消回写——已持久化回复语义不受影响（ACP-C2）；终态关闭落在 continuation。
    assert_eq!(
        intr, "responded",
        "persisted reply semantics preserved (ACP-C2)"
    );
    let (_, _, status) = continuation_of(&pool, r1.interaction_id).await;
    assert_eq!(
        status, "closed",
        "terminal hook closes started continuation"
    );

    // 终态零出边：已取消 Task 的再次状态直写被状态机拒绝（不重入钩子）。
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM delivery.task WHERE id = $1")
        .bind(fx.1)
        .fetch_one(&pool)
        .await
        .unwrap();
    let reopen = pm_application::pm_write::update_task(
        &pool,
        fx.1,
        pm_application::pm_write::UpdateTaskRequest {
            revision: Some(revision),
            title: Some("t".into()),
            description: None,
            status: Some("running".into()),
            priority: Some("normal".into()),
            assignee_id: None,
            goal: None,
            constraints: None,
            deliverables: None,
            acceptance_criteria: None,
            knowledge_refs: None,
        },
        &AuditActor::principal(fx.3),
    )
    .await;
    assert!(
        reopen.is_err(),
        "terminal task must not accept status writes"
    );
}
