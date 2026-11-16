//! P3-6 · Intake 流程 A~F 集成测试 + 幂等矩阵（V1.1 §11 六时序 / §14.4）。
//!
//! 驱动方式：服务层直调（`pm_application::intake`）+ 既有 OutboxWorker
//! （`register_all_handlers` 装配）逐事件 `dispatch_once` 消费——真实 DB，
//! 不 Mock；HTTP 面另见 `apps/pm-api/tests/intake.rs`。
//!
//! | 流程 | 断言焦点 |
//! |---|---|
//! | A | 文档上传 → kb_analyze → 信号 → RT 创建 pc_analyze → NO_CHANGE → 父 COMPLETED |
//! | B | NO_PRODUCT_RELEVANCE 信号被 RT 过滤（不建任务）→ 父 COMPLETED（合法终态） |
//! | C | 失败上报 → FAILED → 人工重试（复用原行 attempt+1）→ 重投递 → COMPLETED |
//! | D | product_decision → pc_decide → change_proposed → COMPLETED + L4 待办留痕 |
//! | E | pc.evidence_requested → kb_supply(optional) 任务创建 → 父不阻塞 |
//! | F | 聚合优先级：NEEDS_HUMAN_REVIEW > FAILED > PROCESSING > COMPLETED |

use pm_application::intake::{self, RoutingPolicy, SignalEnvelope, SubmitIntake};
use pm_application::outbox::{OutboxAppend, OutboxEvent};
use pm_domain::SYSTEM_TENANT;
use pm_infrastructure::outbox_handlers::register_all_handlers;
use pm_infrastructure::outbox_worker::OutboxWorker;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use kb_pc_contracts::events as ev;

async fn seed_principal(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ('11111111-1111-1111-1111-111111111111', 'human', 'seed', true) \
         ON CONFLICT (id) DO NOTHING",
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("c-{code}"))
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(code)
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 提交 intake 的便捷封装。
async fn submit(pool: &PgPool, project_id: Uuid, intake_type: &str) -> intake::IntakeReceipt {
    intake::submit_intake(
        pool,
        SubmitIntake {
            project_id,
            tenant_id: SYSTEM_TENANT,
            submitted_by: uuid::uuid!("11111111-1111-1111-1111-111111111111"),
            intake_type: intake_type.to_string(),
            channel: "web".to_string(),
            source_ref: serde_json::json!({ "kind": "inline_text", "content_hash": "sha256:test" }),
            content_hash: Some("sha256:test".to_string()),
            title: Some("flow test".to_string()),
            idempotency_key: Some(format!("{intake_type}-{}", Uuid::new_v4())),
            tasks: None,
        },
    )
    .await
    .unwrap()
}

/// 驱动 worker 消费至空闲（上限 60 步防死循环）。
async fn drain(worker: &OutboxWorker) {
    for _ in 0..60 {
        match worker.dispatch_once().await.unwrap() {
            pm_infrastructure::outbox_worker::DispatchOutcome::Idle => return,
            _ => continue,
        }
    }
    panic!("outbox queue did not drain within 60 dispatches (handler loop?)");
}

fn worker(pool: &PgPool) -> OutboxWorker {
    register_all_handlers(
        OutboxWorker::new(pool.clone(), "test-worker".to_string()),
        pool,
        None,
        false,
    )
}

async fn tasks_of(
    pool: &PgPool,
    intake_id: Uuid,
) -> Vec<(String, String, String, i32, Option<serde_json::Value>)> {
    sqlx::query(
        "SELECT task_type, task_status, phase, attempt, result_ref \
           FROM integration.intake_task WHERE intake_id = $1 ORDER BY created_at",
    )
    .bind(intake_id)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| {
        (
            r.get::<String, _>("task_type"),
            r.get::<String, _>("task_status"),
            r.get::<String, _>("phase"),
            r.get::<i32, _>("attempt"),
            r.get::<Option<serde_json::Value>, _>("result_ref"),
        )
    })
    .collect()
}

fn status_of(
    tasks: &[(String, String, String, i32, Option<serde_json::Value>)],
    task_type: &str,
) -> String {
    tasks
        .iter()
        .find(|(ty, _, _, _, _)| ty == task_type)
        .map(|(_, st, _, _, _)| st.clone())
        .unwrap_or_else(|| "ABSENT".to_string())
}

async fn task_id_of_type(pool: &PgPool, intake_id: Uuid, task_type: &str) -> Uuid {
    sqlx::query_scalar(
        "SELECT id FROM integration.intake_task WHERE intake_id = $1 AND task_type = $2 \
          ORDER BY created_at LIMIT 1",
    )
    .bind(intake_id)
    .bind(task_type)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn derived(pool: &PgPool, intake_id: Uuid) -> String {
    let tasks: Vec<(String, String)> =
        sqlx::query("SELECT phase, task_status FROM integration.intake_task WHERE intake_id = $1")
            .bind(intake_id)
            .fetch_all(pool)
            .await
            .unwrap()
            .into_iter()
            .map(|r| {
                (
                    r.get::<String, _>("phase"),
                    r.get::<String, _>("task_status"),
                )
            })
            .collect();
    format!("{:?}", intake::aggregate_task_status(&tasks))
}

// ── 流程 A：文档上传全链路（kb_analyze → 信号 → pc_analyze → NO_CHANGE）────

#[sqlx::test(migrations = "../../migrations")]
async fn flow_a_document_upload_full_chain(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "FLOW-A").await;
    let receipt = submit(&pool, project, "document_upload").await;
    assert_eq!(
        receipt.tasks.len(),
        1,
        "document_upload routes kb_analyze (required)"
    );

    // KB 桩消费请求并回写完成（不发信号——信号是 KB 条件产出，§10.2）。
    let w = worker(&pool);
    drain(&w).await;
    let tasks = tasks_of(&pool, receipt.intake_id).await;
    assert_eq!(
        status_of(&tasks, "kb_analyze"),
        "COMPLETED",
        "KB stub completes analysis"
    );

    // 流程 A 信号段：KB 产出 PRODUCT_CANDIDATE/medium 信号（测试显式构造）。
    let kb_task = task_id_of_type(&pool, receipt.intake_id, "kb_analyze").await;
    let mut tx = pool.begin().await.unwrap();
    OutboxAppend::append(
        &pm_application::outbox::PgOutboxAppend,
        &mut tx,
        OutboxEvent {
            project_id: project,
            event_type: ev::KB_COGNITION_SIGNAL_EMITTED,
            aggregate_type: "intake_task",
            aggregate_id: &kb_task.to_string(),
            payload: serde_json::json!({
                "relevance": "PRODUCT_CANDIDATE",
                "confidence": "medium",
                "rationale": "extracted 2 candidate rules",
                "subject_refs": [],
                "evidence_refs": [],
            }),
            idempotency_key: &format!("signal-a-{kb_task}"),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drain(&w).await;

    let tasks = tasks_of(&pool, receipt.intake_id).await;
    // 信号过阈值 → RT 创建 pc_analyze → PC 桩 NO_CHANGE → COMPLETED。
    assert_eq!(
        status_of(&tasks, "pc_analyze"),
        "COMPLETED",
        "RT routes signal to pc_analyze; PC stub completes (NO_CHANGE)"
    );
    assert_eq!(derived(&pool, receipt.intake_id).await, "Completed");

    // pc_analyze 的 signal_ref 记录了信号摘要（四元组引用，无 service 字段）。
    let signal_ref: serde_json::Value = sqlx::query_scalar(
        "SELECT signal_ref FROM integration.intake_task \
          WHERE intake_id = $1 AND task_type = 'pc_analyze' LIMIT 1",
    )
    .bind(receipt.intake_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let signals = signal_ref
        .get("signals")
        .and_then(|v| v.as_array())
        .unwrap();
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0]["relevance"], "PRODUCT_CANDIDATE");
    assert_eq!(signals[0]["confidence"], "medium");
    assert!(
        signal_ref.to_string().find("service").is_none(),
        "no service field in signal_ref (ADR-07)"
    );

    // result_ref 是四元组（无 service 字段）。
    let (_, _, _, _, result_ref) = tasks
        .iter()
        .find(|(ty, _, _, _, _)| ty == "pc_analyze")
        .unwrap();
    let result_ref = result_ref.clone().unwrap();
    assert_eq!(result_ref["target_domain"], "pc");
    assert!(result_ref.to_string().find("service").is_none());
}

// ── 流程 B：NO_PRODUCT_RELEVANCE → 不建 pc_analyze → 父 COMPLETED ──────────

#[sqlx::test(migrations = "../../migrations")]
async fn flow_b_no_relevance_signal_does_not_create_task(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "FLOW-B").await;
    let receipt = submit(&pool, project, "document_upload").await;
    // 取 kb_analyze 任务 id：信号事件以任务为聚合锚。
    let kb_task = task_id_of_type(&pool, receipt.intake_id, "kb_analyze").await;

    // 构造 NO_PRODUCT_RELEVANCE 信号（KB 侧留痕形态，直接入 outbox）。
    let mut tx = pool.begin().await.unwrap();
    OutboxAppend::append(
        &pm_application::outbox::PgOutboxAppend,
        &mut tx,
        OutboxEvent {
            project_id: project,
            event_type: ev::KB_COGNITION_SIGNAL_EMITTED,
            aggregate_type: "intake_task",
            aggregate_id: &kb_task.to_string(),
            payload: serde_json::json!({
                "relevance": "NO_PRODUCT_RELEVANCE",
                "confidence": "high",
                "rationale": "no product relevance",
                "subject_refs": [],
                "evidence_refs": [],
            }),
            idempotency_key: &format!("signal-nr-{kb_task}"),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let w = worker(&pool);
    drain(&w).await;

    let tasks = tasks_of(&pool, receipt.intake_id).await;
    assert_eq!(status_of(&tasks, "kb_analyze"), "COMPLETED");
    assert_eq!(
        status_of(&tasks, "pc_analyze"),
        "ABSENT",
        "RT must not create pc_analyze for NO_PRODUCT_RELEVANCE (§10.2-1)"
    );
    assert_eq!(
        derived(&pool, receipt.intake_id).await,
        "Completed",
        "flow B legal terminal state"
    );
}

// ── 流程 C：失败上报 → FAILED → 人工重试（复用原行）→ COMPLETED ────────────

#[sqlx::test(migrations = "../../migrations")]
async fn flow_c_failure_then_manual_retry_reuses_row(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "FLOW-C").await;
    let receipt = submit(&pool, project, "product_decision").await;
    let decide_task = task_id_of_type(&pool, receipt.intake_id, "pc_decide").await;

    // 场景抽象：请求事件投递耗尽进 DLQ 后人工移除（ADR-12 死信处理面），
    // 服务实际未处理——失败上报（processing.failed）先到。
    sqlx::query("DELETE FROM integration.outbox_event                  WHERE aggregate_id = $1 AND event_type = $2")
        .bind(decide_task.to_string())
        .bind(ev::PC_DECISION_SUBMITTED)
        .execute(&pool).await.unwrap();

    // 失败上报（服务失败 → processing.failed → Platform 消费端置 FAILED）。
    let mut tx = pool.begin().await.unwrap();
    OutboxAppend::append(
        &pm_application::outbox::PgOutboxAppend,
        &mut tx,
        OutboxEvent {
            project_id: project,
            event_type: ev::PROCESSING_FAILED,
            aggregate_type: "intake_task",
            aggregate_id: &decide_task.to_string(),
            payload: serde_json::json!({
                "failed_ref": { "target_domain": "requirement", "target_type": "intake_task",
                                "target_id": decide_task.to_string(), "target_revision_id": null },
                "error_code": "pc_unavailable",
                "retry_count": 3,
                "recoverable": true,
            }),
            idempotency_key: &format!("failed-{decide_task}-1"),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let w = worker(&pool);
    drain(&w).await;

    let tasks = tasks_of(&pool, receipt.intake_id).await;
    assert_eq!(status_of(&tasks, "pc_decide"), "FAILED");
    assert_eq!(derived(&pool, receipt.intake_id).await, "Failed");

    // 人工重试：复用原行（同 id），attempt+1 → PENDING，事件重投。
    let retried = intake::retry_task(&pool, receipt.intake_id, decide_task)
        .await
        .unwrap();
    assert_eq!(
        retried.task_id, decide_task,
        "retry must reuse the original row (ADR-12)"
    );
    assert_eq!(retried.attempt, 1);

    // 非终态任务重试 → 409 语义（IntakeError::NotRetryable）。
    assert!(matches!(
        intake::retry_task(&pool, receipt.intake_id, decide_task).await,
        Err(intake::IntakeError::NotRetryable(_, _))
    ));

    drain(&w).await; // 重试派发事件 → PC 桩消费 → completed 事件回写
    let tasks = tasks_of(&pool, receipt.intake_id).await;
    assert_eq!(
        status_of(&tasks, "pc_decide"),
        "COMPLETED",
        "retry completes via stub"
    );
    assert_eq!(derived(&pool, receipt.intake_id).await, "Completed");
}

// ── 流程 D：product_decision → change_proposed → COMPLETED（L4 分离）───────

#[sqlx::test(migrations = "../../migrations")]
async fn flow_d_decision_produces_proposal_l4_separated(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "FLOW-D").await;
    let receipt = submit(&pool, project, "product_decision").await;

    let w = worker(&pool);
    drain(&w).await;

    let tasks = tasks_of(&pool, receipt.intake_id).await;
    assert_eq!(status_of(&tasks, "pc_decide"), "COMPLETED");
    let result_ref = tasks
        .iter()
        .find(|(ty, _, _, _, _)| ty == "pc_decide")
        .unwrap()
        .4
        .clone()
        .unwrap();
    assert_eq!(
        result_ref["target_type"], "proposal",
        "result_ref pins proposal DomainRef"
    );

    // L4 待办留痕（提案审批在 PC，Platform 展示）。
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.notification \
          WHERE kind = 'pc_proposal_pending' AND payload->>'dedup_key' IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(n > 0, "proposal pending todo must be recorded for display");

    assert_eq!(
        derived(&pool, receipt.intake_id).await,
        "Completed",
        "L4 does not feed back into L3 (§11.4)"
    );
}

// ── 流程 E：pc.evidence_requested → kb_supply(optional) → 不阻塞父 ─────────

#[sqlx::test(migrations = "../../migrations")]
async fn flow_e_evidence_request_creates_optional_kb_supply(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "FLOW-E").await;
    let receipt = submit(&pool, project, "product_decision").await;
    let decide_task = task_id_of_type(&pool, receipt.intake_id, "pc_decide").await;

    let mut tx = pool.begin().await.unwrap();
    OutboxAppend::append(
        &pm_application::outbox::PgOutboxAppend,
        &mut tx,
        OutboxEvent {
            project_id: project,
            event_type: ev::PC_EVIDENCE_REQUESTED,
            aggregate_type: "intake_task",
            aggregate_id: &decide_task.to_string(),
            payload: serde_json::json!({
                "pc_analysis_ref": { "target_domain": "pc", "target_type": "analysis",
                                     "target_id": decide_task.to_string(), "target_revision_id": null },
                "requested_about": [],
                "reason": "archive evidence",
                "request_id": "ev-req-1",
            }),
            idempotency_key: "ev-req-1",
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let w = worker(&pool);
    drain(&w).await;

    let tasks = tasks_of(&pool, receipt.intake_id).await;
    assert_eq!(
        status_of(&tasks, "kb_supply"),
        "COMPLETED",
        "kb_supply routed to KB stub and completed"
    );
    // kb_supply 为 optional：即使失败也不阻塞父（此处已完成）。
    assert_eq!(derived(&pool, receipt.intake_id).await, "Completed");
    assert_eq!(
        status_of(&tasks, "pc_decide"),
        "COMPLETED",
        "evidence request must not touch the originating task"
    );

    // 幂等：同 request_id 重复投递零副作用（不建第二行）。
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.intake_task \
          WHERE intake_id = $1 AND task_type = 'kb_supply'",
    )
    .bind(receipt.intake_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 1);
}

// ── 流程 F：聚合优先级（NEEDS_HUMAN_REVIEW > FAILED > PROCESSING > COMPLETED）

#[sqlx::test(migrations = "../../migrations")]
async fn flow_f_aggregation_priority_rules(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "FLOW-F").await;
    let receipt = submit(&pool, project, "document_upload").await;
    let kb_task = task_id_of_type(&pool, receipt.intake_id, "kb_analyze").await;

    // 直接造多状态任务组合（绕过 handler，专测 §9.4.2 派生规则）。
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO integration.intake_task \
         (tenant_id, project_id, intake_id, target_service, task_type, phase, task_status, idempotency_key) \
         VALUES ($1, $2, $3, 'pc', 'pc_analyze', 'required', 'NEEDS_HUMAN_REVIEW', 'f-nhr')",
    )
    .bind(SYSTEM_TENANT).bind(project).bind(receipt.intake_id)
    .execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();

    // NEEDS_HUMAN_REVIEW 优先于一切（§9.4.2 规则表第一行，不区分 phase）。
    assert_eq!(derived(&pool, receipt.intake_id).await, "NeedsHumanReview");
    // NHR 清除后：required FAILED 优先于活跃。
    sqlx::query("UPDATE integration.intake_task SET task_status = 'PENDING'                  WHERE intake_id = $1 AND task_status = 'NEEDS_HUMAN_REVIEW'")
        .bind(receipt.intake_id)
        .execute(&pool).await.unwrap();
    sqlx::query("UPDATE integration.intake_task SET task_status = 'FAILED' WHERE id = $1")
        .bind(kb_task)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        derived(&pool, receipt.intake_id).await,
        "Failed",
        "required FAILED outranks"
    );
    sqlx::query("UPDATE integration.intake_task SET task_status = 'PENDING' WHERE id = $1")
        .bind(kb_task)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        derived(&pool, receipt.intake_id).await,
        "Processing",
        "required active → PROCESSING"
    );
    sqlx::query("UPDATE integration.intake_task SET task_status = 'COMPLETED' WHERE id = $1")
        .bind(kb_task)
        .execute(&pool)
        .await
        .unwrap();
    // pc_analyze（required）仍非终态 → 不允许 COMPLETED。
    assert_eq!(derived(&pool, receipt.intake_id).await, "Processing");
    sqlx::query(
        "UPDATE integration.intake_task SET task_status = 'COMPLETED' \
                 WHERE intake_id = $1 AND task_type = 'pc_analyze'",
    )
    .bind(receipt.intake_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(derived(&pool, receipt.intake_id).await, "Completed");
}

// ── 幂等矩阵（§14.4）────────────────────────────────────────────────────────

/// 重复提交（同 Idempotency-Key）：replayed=true、任务/事件零增量。
#[sqlx::test(migrations = "../../migrations")]
async fn idempotent_resubmission_has_zero_side_effects(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "IDEM-1").await;
    let key = format!("intake-idem-{}", Uuid::new_v4());

    let submit_with_key = async |key: &str| {
        intake::submit_intake(
            &pool,
            SubmitIntake {
                project_id: project,
                tenant_id: SYSTEM_TENANT,
                submitted_by: uuid::uuid!("11111111-1111-1111-1111-111111111111"),
                intake_type: "document_upload".to_string(),
                channel: "web".to_string(),
                source_ref: serde_json::json!({ "kind": "inline_text" }),
                content_hash: Some("sha256:idem".to_string()),
                title: None,
                idempotency_key: Some(key.to_string()),
                tasks: None,
            },
        )
        .await
        .unwrap()
    };
    let first = submit_with_key(&key).await;
    assert!(!first.replayed);
    let second = submit_with_key(&key).await;
    assert!(
        second.replayed,
        "duplicate submission replays existing intake"
    );
    assert_eq!(first.intake_id, second.intake_id);
    assert_eq!(first.tasks.len(), second.tasks.len());

    let task_n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM integration.intake_task WHERE intake_id = $1")
            .bind(first.intake_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(task_n, 1, "no extra task rows on replay");
    let outbox_n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.outbox_event \
          WHERE aggregate_id = ANY(\
            SELECT id::text FROM integration.intake_task WHERE intake_id = $1)",
    )
    .bind(first.intake_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        outbox_n <= 2,
        "outbox events collapsed by idempotency key (got {outbox_n})"
    );
}

/// 多信号合并：两个信号同 intake → 单活跃任务 + signal_ref 数组两条（§9.4.3-1）。
#[sqlx::test(migrations = "../../migrations")]
async fn multi_signal_merge_appends_to_active_task(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "IDEM-2").await;
    let receipt = submit(&pool, project, "document_upload").await;
    let _kb_task = task_id_of_type(&pool, receipt.intake_id, "kb_analyze").await;

    let policy = RoutingPolicy::default();
    let signal = |rationale: &str| SignalEnvelope {
        relevance: "PRODUCT_CANDIDATE".to_string(),
        confidence: Some("medium".to_string()),
        rationale: Some(rationale.to_string()),
        subject_refs: vec![],
        evidence_refs: vec![],
    };
    intake::route_cognition_signal(
        &pool,
        project,
        SYSTEM_TENANT,
        receipt.intake_id,
        signal("first"),
        &policy,
    )
    .await
    .unwrap();
    let outcome = intake::route_cognition_signal(
        &pool,
        project,
        SYSTEM_TENANT,
        receipt.intake_id,
        signal("second"),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(outcome, intake::RouteOutcome::MergedIntoActive);

    let rows: Vec<(String, serde_json::Value)> = sqlx::query_as(
        "SELECT task_status, signal_ref FROM integration.intake_task \
          WHERE intake_id = $1 AND task_type = 'pc_analyze'",
    )
    .bind(receipt.intake_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "only one active pc_analyze (uq_intake_task_active)"
    );
    assert_eq!(rows[0].0, "PENDING");
    let signals = rows[0].1.get("signals").and_then(|v| v.as_array()).unwrap();
    assert_eq!(
        signals.len(),
        2,
        "second signal merged into signal_ref (array append)"
    );

    // 消费链继续跑通：合并任务被桩完成 → completed 回写。
    let w = worker(&pool);
    drain(&w).await;
    let tasks = tasks_of(&pool, receipt.intake_id).await;
    assert_eq!(status_of(&tasks, "pc_analyze"), "COMPLETED");
}

/// 后到信号（任务已终态）：默认策略 RecordTodo（审计留痕，不建任务）。
#[sqlx::test(migrations = "../../migrations")]
async fn signal_after_terminal_defaults_to_todo(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "IDEM-3").await;
    let receipt = submit(&pool, project, "document_upload").await;
    let kb_task = task_id_of_type(&pool, receipt.intake_id, "kb_analyze").await;

    // 先路由首信号建 pc_analyze，再把它置终态；随后到信号命中「任务已终态」
    // 分支（§9.4.3-3 查询的是同 (intake_id, task_type) 的 COMPLETED 任务）。
    let policy = RoutingPolicy::default();
    let first = intake::route_cognition_signal(
        &pool,
        project,
        SYSTEM_TENANT,
        receipt.intake_id,
        SignalEnvelope {
            relevance: "PRODUCT_CANDIDATE".to_string(),
            confidence: Some("high".to_string()),
            rationale: Some("first signal".to_string()),
            subject_refs: vec![],
            evidence_refs: vec![],
        },
        &policy,
    )
    .await
    .unwrap();
    assert!(matches!(first, intake::RouteOutcome::TaskCreated { .. }));
    let pc_task = task_id_of_type(&pool, receipt.intake_id, "pc_analyze").await;
    sqlx::query("UPDATE integration.intake_task SET task_status='COMPLETED' WHERE id=$1")
        .bind(pc_task)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE integration.intake_task SET task_status='COMPLETED' WHERE id=$1")
        .bind(kb_task)
        .execute(&pool)
        .await
        .unwrap();
    let outcome = intake::route_cognition_signal(
        &pool,
        project,
        SYSTEM_TENANT,
        receipt.intake_id,
        SignalEnvelope {
            relevance: "PRODUCT_CANDIDATE".to_string(),
            confidence: Some("high".to_string()),
            rationale: Some("late signal".to_string()),
            subject_refs: vec![],
            evidence_refs: vec![],
        },
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(outcome, intake::RouteOutcome::AfterTerminalTodo);
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.intake_task \
          WHERE intake_id = $1 AND task_type = 'pc_analyze'",
    )
    .bind(receipt.intake_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        n, 1,
        "default after-terminal action = record todo: no NEW task row (only the first signal's row, §9.4.3-3)"
    );
}

/// 目录一致性：装配 handler 覆盖 CATALOG has_handler=true 全集（启动校验等价）。
#[test]
fn handler_assembly_covers_catalog() {
    let mut expected: Vec<&str> = pm_application::outbox::CATALOG
        .iter()
        .filter(|s| s.has_handler)
        .map(|s| s.event_type)
        .collect();
    expected.sort_unstable();
    // 装配出的 worker（需要 pool 才能构造；此处直接对齐常量清单）。
    let wired: Vec<&str> = {
        let mut v: Vec<&str> = vec![
            "TaskReady",
            "DeliverySubmitted",
            "RunCompleted",
            // Document Processing（0016/TD-02 §4）：Team 级事件（project 可空），
            // handler 承担送达语义（DocprocSettleHandler）。
            "DocprocJobCompleted",
            "DocprocJobFailed",
            ev::INTAKE_RECEIVED,
            ev::KB_ANALYSIS_REQUESTED,
            ev::KB_ANALYSIS_COMPLETED,
            ev::KB_COGNITION_SIGNAL_EMITTED,
            ev::KB_MEMORY_APPROVED,
            ev::PC_ANALYZE_REQUESTED,
            ev::PC_DECISION_SUBMITTED,
            ev::PC_ANALYSIS_COMPLETED,
            ev::PC_CHANGE_PROPOSED,
            ev::PC_CONFLICT_DETECTED,
            ev::PC_EVIDENCE_REQUESTED,
            ev::PC_COGNITION_VERSION_PUBLISHED,
            // P3-11 ProjC 独立域事件（阶段一加法；Platform 留痕消费者）。
            ev::PROJ_CHANGE_PROPOSED,
            ev::PROJ_COGNITION_VERSION_PUBLISHED,
            // WP-0 内容管线接缝：KB 修订发布（留痕型 handler，WP-3 接真索引）。
            ev::KB_DOCUMENT_REVISION_PUBLISHED,
            ev::PROCESSING_FAILED,
        ];
        v.sort_unstable();
        v
    };
    assert_eq!(
        wired, expected,
        "register_all_handlers must cover catalog has_handler=true"
    );
}
