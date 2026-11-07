//! KB-PC 服务化 Phase 3 的消费端 handler（V1.1 §13.3 13 事件接线；P3-3）。
//!
//! 职责切分（§9.4.2 状态迁移责任表）：
//! - **KB/PC 分析本体 = 最小桩**（任务书四、P3-3）：收到请求事件 → 状态流转
//!   确认（经 `pm_application::intake` 的 Platform 写入面，桩不直写 intake 表）
//!   → 回写结果事件（与业务同事务，§9.3-7 语义）。真实分析本体归 Phase 4/5。
//! - **Platform 消费端**：消费结果/信号/失败事件，推进 intake_task 终态、
//!   RT 路由决策、通知留痕——intake 表唯一写入方是 Platform。
//!
//! 幂等纪律（§14.1 至少一次投递 ≠ 恰好一次执行）：全部 handler 重入安全——
//! 状态推进 CAS、结果事件 outbox 幂等键坍缩、通知 `WHERE NOT EXISTS` 锚。
//! pc.cognition.version_published 不注册（has_handler=false：生产者归 PC-01C，
//! 「不假装消费」§20.6）。

use std::sync::Arc;

use sqlx::PgPool;

use pm_application::intake::{
    self, mark_task_dispatched, mark_task_processing, route_cognition_signal, RoutingPolicy,
    SignalEnvelope,
};
use pm_application::outbox::{OutboxAppend, OutboxEvent, PgOutboxAppend};

use crate::outbox::ClaimedOutboxEvent;
use crate::outbox_worker::{OutboxHandler, OutboxWorker};

use kb_pc_contracts::events as ev;

/// 从 payload.task_ref.target_id 取任务 ID（全部 13 事件的请求类载荷都带
/// task_ref 四元组；缺视为畸形事件，报错走重试/死信）。
fn task_id_of(event: &ClaimedOutboxEvent) -> Result<uuid::Uuid, String> {
    let raw = event
        .payload
        .get("task_ref")
        .and_then(|t| t.get("target_id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("{} payload missing task_ref.target_id", event.event_type))?;
    uuid::Uuid::parse_str(raw).map_err(|e| format!("task_ref.target_id `{raw}`: {e}"))
}

/// 四元组引用构造（payload 内引用对象；禁止 service 字段——类型上不含）。
fn domain_ref_json(domain: &str, ty: &str, id: &str) -> serde_json::Value {
    serde_json::json!({
        "target_domain": domain,
        "target_type": ty,
        "target_id": id,
        "target_revision_id": null,
    })
}

fn intake_ref_json(intake_id: uuid::Uuid) -> serde_json::Value {
    domain_ref_json("requirement", "intake_record", &intake_id.to_string())
}

fn task_ref_json(task_id: uuid::Uuid) -> serde_json::Value {
    domain_ref_json("requirement", "intake_task", &task_id.to_string())
}

// ── KB 桩（消费 kb.analysis.requested）─────────────────────────────────────

/// KB 分析桩：状态流转 → 同事务回写 kb.analysis.completed。
///
/// **不自动产出认知候选信号**（§10.2：信号是 KB 分析的**条件产出**——
/// 「KB 分析完成」≠「必有信号」；`NO_PRODUCT_RELEVANCE`/不产信号都是合法
/// 形态，禁止「每份资料必过 PC」）。流程 A 的信号时序由集成测试显式构造
/// （真实信号产出口径归 Phase 4 KB 分析本体）。
pub struct KbAnalyzeRequestedStub {
    pool: PgPool,
}

impl KbAnalyzeRequestedStub {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for KbAnalyzeRequestedStub {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let task_id = task_id_of(event)?;
        // 状态流转确认（Platform 写入面；重投递幂等）。
        mark_task_dispatched(&self.pool, task_id).await.map_err(|e| e.to_string())?;
        mark_task_processing(&self.pool, task_id).await.map_err(|e| e.to_string())?;

        let intake_id: uuid::Uuid = sqlx::query_scalar(
            "SELECT intake_id FROM integration.intake_task WHERE id = $1",
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        // 结果事件（与「分析落账」同一事务语义；桩无业务表，事务=outbox 单写）。
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        let outbox = PgOutboxAppend;
        let analysis_id = format!("kb-analysis-{task_id}");
        outbox
            .append(
                &mut tx,
                OutboxEvent {
                    project_id: event.project_id,
                    event_type: ev::KB_ANALYSIS_COMPLETED,
                    aggregate_type: "intake_task",
                    aggregate_id: &task_id.to_string(),
                    payload: serde_json::json!({
                        "kb_analysis_ref": domain_ref_json("knowledge_base", "kb_analysis", &analysis_id),
                        "source_ref": event.payload.get("source_ref").cloned().unwrap_or(serde_json::Value::Null),
                        "document_ref": domain_ref_json("knowledge_base", "document_revision", &format!("kb-stub-doc-{task_id}")),
                        "memory_draft_refs": [],
                        "conclusion": "COMPLETED",
                        "task_ref": task_ref_json(task_id),
                    }),
                    idempotency_key: &format!("kb_analysis:{task_id}"),
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())?;
        tracing::info!(%task_id, %intake_id, "kb stub: analysis completed");
        Ok(())
    }
}

// ── PC 桩（消费 pc.analyze.requested / pc.decision.submitted）──────────────

/// PC 分析桩：状态流转 → pc.analysis.completed（NO_CHANGE；Proposal/Conflict
/// 实体与发布事务归 PC-01C，桩仅回写完成事件，M3 前禁开认知写入）。
pub struct PcAnalyzeRequestedStub {
    pool: PgPool,
}

impl PcAnalyzeRequestedStub {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for PcAnalyzeRequestedStub {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let task_id = task_id_of(event)?;
        mark_task_dispatched(&self.pool, task_id).await.map_err(|e| e.to_string())?;
        mark_task_processing(&self.pool, task_id).await.map_err(|e| e.to_string())?;

        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        PgOutboxAppend
            .append(
                &mut tx,
                OutboxEvent {
                    project_id: event.project_id,
                    event_type: ev::PC_ANALYSIS_COMPLETED,
                    aggregate_type: "intake_task",
                    aggregate_id: &task_id.to_string(),
                    payload: serde_json::json!({
                        "pc_analysis_ref": domain_ref_json("pc", "analysis", &format!("pc-analysis-{task_id}")),
                        "conclusion": "NO_CHANGE",
                        "refs": [],
                        "task_ref": task_ref_json(task_id),
                    }),
                    idempotency_key: &format!("pc_analysis:{task_id}"),
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())?;
        tracing::info!(%task_id, "pc stub: analysis completed (NO_CHANGE)");
        Ok(())
    }
}

/// PC 决策桩（流程 E）：pc.decision.submitted → pc.change_proposed（桩替身
/// 事件；proposal 实体审批在 PC，归 PC-01C）。
pub struct PcDecisionSubmittedStub {
    pool: PgPool,
}

impl PcDecisionSubmittedStub {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for PcDecisionSubmittedStub {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let task_id = task_id_of(event)?;
        mark_task_dispatched(&self.pool, task_id).await.map_err(|e| e.to_string())?;
        mark_task_processing(&self.pool, task_id).await.map_err(|e| e.to_string())?;

        use sha2::{Digest, Sha256};
        let content_hash = {
            let canonical = serde_json::to_string(
                event.payload.get("decision").unwrap_or(&serde_json::Value::Null),
            )
            .unwrap_or_default();
            let digest = Sha256::digest(canonical.as_bytes());
            digest.iter().take(16).map(|b| format!("{b:02x}")).collect::<String>()
        };

        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        PgOutboxAppend
            .append(
                &mut tx,
                OutboxEvent {
                    project_id: event.project_id,
                    event_type: ev::PC_CHANGE_PROPOSED,
                    aggregate_type: "intake_task",
                    aggregate_id: &task_id.to_string(),
                    payload: serde_json::json!({
                        "proposal_ref": domain_ref_json("pc", "proposal", &format!("pc-proposal-{task_id}")),
                        "governance_level": "L2",
                        "members": [],
                        "content_hash": content_hash,
                        "task_ref": task_ref_json(task_id),
                    }),
                    idempotency_key: &format!("pc_proposal:{task_id}"),
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())?;
        tracing::info!(%task_id, "pc stub: decision accepted, change proposed");
        Ok(())
    }
}

// ── Platform 消费端（结果回写 / RT 路由 / 留痕）────────────────────────────

fn result_ref_of(event: &ClaimedOutboxEvent, keys: &[&str]) -> Option<serde_json::Value> {
    keys.iter()
        .find_map(|k| event.payload.get(*k))
        .filter(|v| v.is_object())
        .cloned()
}

/// kb.analysis.completed → 任务终态 COMPLETED（result_ref = KB 文档修订四元组）。
pub struct KbAnalysisCompletedHandler {
    pool: PgPool,
}

impl KbAnalysisCompletedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for KbAnalysisCompletedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let task_id = task_id_of(event)?;
        let result_ref = result_ref_of(event, &["document_ref", "kb_analysis_ref"])
            .ok_or_else(|| "kb.analysis.completed missing document_ref".to_string())?;
        intake::complete_task(&self.pool, task_id, result_ref)
            .await
            .map_err(|e| e.to_string())?;
        tracing::info!(%task_id, "platform: kb analysis completed → task COMPLETED");
        Ok(())
    }
}

/// kb.cognition_signal.emitted → RT 路由决策（§10.2）：过滤/阈值/合并 →
/// 必要时创建 pc_analyze 任务 + 同事务派发事件。
pub struct CognitionSignalRouterHandler {
    pool: PgPool,
}

impl CognitionSignalRouterHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for CognitionSignalRouterHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let signal: SignalEnvelope = serde_json::from_value(event.payload.clone())
            .map_err(|e| format!("cognition signal payload: {e}"))?;
        // correlation_id 承载原始 intake_id（事件信封级追踪字段，§13.1）。
        // 解析顺序：aggregate_id → 幂等键 → payload.task_ref.target_id
        // （KB 桩发的信号事件 aggregate_id=任务 ID，可反查 intake）。
        let intake_id = uuid::Uuid::parse_str(&event.aggregate_id)
            .or_else(|_| uuid::Uuid::parse_str(&event.idempotency_key))
            .ok()
            .or_else(|| {
                event
                    .payload
                    .get("task_ref")
                    .and_then(|t| t.get("target_id"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| uuid::Uuid::parse_str(s).ok())
            })
            .ok_or_else(|| "cognition signal cannot resolve intake id".to_string())?;
        // route_cognition_signal 的 intake 归属校验：intake_task 反查 intake
        // （signal 由 KB 分析产出，其任务行必已存在）。
        let (project_id, tenant_id, real_intake_id): (uuid::Uuid, uuid::Uuid, uuid::Uuid) = sqlx::query_as(
            "SELECT t.project_id, t.tenant_id, t.intake_id \
               FROM integration.intake_task t \
              WHERE t.id = $1",
        )
        .bind(intake_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        let outcome = route_cognition_signal(
            &self.pool,
            project_id,
            tenant_id,
            real_intake_id,
            signal,
            &RoutingPolicy::from_env(),
        )
        .await
        .map_err(|e| e.to_string())?;
        tracing::info!(?outcome, %real_intake_id, "platform RT: signal routed");
        Ok(())
    }
}

/// 通知留痕（integration.notification；幂等锚 kind + payload->>'key'）。
async fn notify_once(
    pool: &PgPool,
    project_id: uuid::Uuid,
    kind: &str,
    dedup_key: &str,
    payload: serde_json::Value,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO integration.notification \
         (project_id, source_proposal_id, kind, payload, tenant_id) \
         SELECT $1, NULL, $2, $3, (SELECT tenant_id FROM core.project WHERE id = $1) \
         WHERE NOT EXISTS ( \
             SELECT 1 FROM integration.notification \
              WHERE project_id = $1 AND kind = $2 AND payload->>'dedup_key' = $4)",
    )
    .bind(project_id)
    .bind(kind)
    .bind(payload)
    .bind(dedup_key)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// kb.memory.approved → 状态感知留痕（D-8：仅 platform；不作为 PC 触发输入）。
pub struct KbMemoryApprovedHandler {
    pool: PgPool,
}

impl KbMemoryApprovedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for KbMemoryApprovedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let memory_id = event
            .payload
            .get("memory_ref")
            .and_then(|r| r.get("target_id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "kb.memory.approved missing memory_ref.target_id".to_string())?
            .to_string();
        notify_once(
            &self.pool,
            event.project_id,
            "kb_memory_approved",
            &memory_id,
            serde_json::json!({ "dedup_key": memory_id, "payload": event.payload }),
        )
        .await?;
        Ok(())
    }
}

/// pc.analysis.completed → 任务终态（NO_CHANGE 结论也是合法 COMPLETED，§9.4.2）。
pub struct PcAnalysisCompletedHandler {
    pool: PgPool,
}

impl PcAnalysisCompletedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for PcAnalysisCompletedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let task_id = task_id_of(event)?;
        let result_ref = result_ref_of(event, &["pc_analysis_ref"])
            .ok_or_else(|| "pc.analysis.completed missing pc_analysis_ref".to_string())?;
        intake::complete_task(&self.pool, task_id, result_ref)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// pc.change_proposed → 任务 COMPLETED + result_ref=proposal 四元组
/// （流程 D：L3 处理完成与 L4 提案治理分离——审批待办在 PC，不回灌任务状态）。
pub struct PcChangeProposedHandler {
    pool: PgPool,
}

impl PcChangeProposedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for PcChangeProposedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let task_id = task_id_of(event)?;
        let result_ref = result_ref_of(event, &["proposal_ref"])
            .ok_or_else(|| "pc.change_proposed missing proposal_ref".to_string())?;
        intake::complete_task(&self.pool, task_id, result_ref)
            .await
            .map_err(|e| e.to_string())?;
        notify_once(
            &self.pool,
            event.project_id,
            "pc_proposal_pending",
            &format!("pc-proposal-{task_id}"),
            serde_json::json!({ "dedup_key": format!("pc-proposal-{task_id}"), "payload": event.payload }),
        )
        .await?;
        Ok(())
    }
}

/// pc.conflict.detected → 任务 COMPLETED + result_ref=conflict 四元组
/// （流程 A：冲突登记即 PC 处理结果；冲突处置治理在 PC 侧）。
pub struct PcConflictDetectedHandler {
    pool: PgPool,
}

impl PcConflictDetectedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for PcConflictDetectedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let task_id = task_id_of(event)?;
        let result_ref = result_ref_of(event, &["conflict_ref"])
            .ok_or_else(|| "pc.conflict.detected missing conflict_ref".to_string())?;
        intake::complete_task(&self.pool, task_id, result_ref)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// pc.evidence_requested → RT 路由创建 kb_supply 任务（optional）+ 同事务
/// 派发 kb.analysis.requested（资料补充执行转达；K6 fulfil 与 Memory 体系归
/// Phase 4——实施偏差登记）。
pub struct PcEvidenceRequestedHandler {
    pool: PgPool,
}

impl PcEvidenceRequestedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for PcEvidenceRequestedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        // request_id：目录幂等键业务部分；桩/PC 在 payload 携带，缺失回退事件键。
        let request_id = event
            .payload
            .get("request_id")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("req-{}", event.id));
        // 反查 PC 分析任务 → intake（pc_analysis_ref.target_id = pc_analyze 任务）。
        let analysis_task_id = event
            .payload
            .get("pc_analysis_ref")
            .and_then(|r| r.get("target_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .or_else(|| uuid::Uuid::parse_str(&event.aggregate_id).ok())
            .ok_or_else(|| "pc.evidence_requested cannot resolve analysis task".to_string())?;
        let (tenant_id, project_id, intake_id): (uuid::Uuid, uuid::Uuid, uuid::Uuid) =
            sqlx::query_as(
                "SELECT tenant_id, project_id, intake_id FROM integration.intake_task WHERE id = $1",
            )
            .bind(analysis_task_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        let task_id = uuid::Uuid::new_v4();
        let task_key = format!("kb_supply:{request_id}");
        let inserted: Option<uuid::Uuid> = sqlx::query_scalar(
            "INSERT INTO integration.intake_task \
             (id, tenant_id, project_id, intake_id, target_service, task_type, phase, \
              task_status, idempotency_key, signal_ref) \
             VALUES ($1, $2, $3, $4, 'kb', 'kb_supply', 'optional', 'PENDING', $5, $6) \
             ON CONFLICT (tenant_id, project_id, idempotency_key) DO NOTHING RETURNING id",
        )
        .bind(task_id)
        .bind(tenant_id)
        .bind(project_id)
        .bind(intake_id)
        .bind(&task_key)
        .bind(event.payload.get("reason").cloned().map(|r| {
            serde_json::json!({ "signals": [{ "reason": r, "requested_about": event.payload.get("requested_about") }] })
        }))
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        if let Some(task_id) = inserted {
            PgOutboxAppend
                .append(
                    &mut tx,
                    OutboxEvent {
                        project_id,
                        event_type: ev::KB_ANALYSIS_REQUESTED,
                        aggregate_type: "intake_task",
                        aggregate_id: &task_id.to_string(),
                        payload: serde_json::json!({
                            "intake_ref": intake_ref_json(intake_id),
                            "task_ref": task_ref_json(task_id),
                            "source_ref": { "kind": "evidence_request", "request_id": request_id },
                        }),
                        idempotency_key: &task_key,
                    },
                )
                .await
                .map_err(|e| e.to_string())?;
        }
        tx.commit().await.map_err(|e| e.to_string())?;
        tracing::info!(%request_id, "platform RT: kb_supply task ensured (optional)");
        Ok(())
    }
}

/// processing.failed → 任务 FAILED + 运维待办留痕（§14.6 恢复入口联动）。
pub struct ProcessingFailedHandler {
    pool: PgPool,
}

impl ProcessingFailedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for ProcessingFailedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let failed_id = event
            .payload
            .get("failed_ref")
            .and_then(|r| r.get("target_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .ok_or_else(|| "processing.failed missing failed_ref.target_id".to_string())?;
        let error_code = event
            .payload
            .get("error_code")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        intake::fail_task(&self.pool, failed_id, &error_code)
            .await
            .map_err(|e| e.to_string())?;
        notify_once(
            &self.pool,
            event.project_id,
            "processing_failed",
            &format!("{failed_id}:{}", event.payload.get("retry_count").and_then(|v| v.as_i64()).unwrap_or(0)),
            serde_json::json!({ "dedup_key": format!("{failed_id}"), "payload": event.payload }),
        )
        .await?;
        Ok(())
    }
}

/// intake.received → 接收留痕（登记/通知）。
pub struct IntakeReceivedHandler {
    pool: PgPool,
}

impl IntakeReceivedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for IntakeReceivedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let intake_id = uuid::Uuid::parse_str(&event.aggregate_id)
            .map_err(|e| format!("intake.received aggregate_id: {e}"))?;
        notify_once(
            &self.pool,
            event.project_id,
            "intake_received",
            &intake_id.to_string(),
            serde_json::json!({ "dedup_key": intake_id.to_string(), "payload": event.payload }),
        )
        .await?;
        Ok(())
    }
}

/// **唯一的生产 Handler 装配入口扩展**：把 KB-PC 13 事件的 12 个消费者
/// 追加进既有装配（TaskReady/DeliverySubmitted/RunCompleted 之后）。
/// 装配全集由 `OutboxWorker::assert_catalog_consistent` 启动校验兜底。
pub fn register_kb_pc_handlers(worker: OutboxWorker, pool: &PgPool) -> OutboxWorker {
    worker
        .with_handler(ev::INTAKE_RECEIVED, Arc::new(IntakeReceivedHandler::new(pool.clone())))
        .with_handler(ev::KB_ANALYSIS_REQUESTED, Arc::new(KbAnalyzeRequestedStub::new(pool.clone())))
        .with_handler(ev::KB_ANALYSIS_COMPLETED, Arc::new(KbAnalysisCompletedHandler::new(pool.clone())))
        .with_handler(ev::KB_COGNITION_SIGNAL_EMITTED, Arc::new(CognitionSignalRouterHandler::new(pool.clone())))
        .with_handler(ev::KB_MEMORY_APPROVED, Arc::new(KbMemoryApprovedHandler::new(pool.clone())))
        .with_handler(ev::PC_ANALYZE_REQUESTED, Arc::new(PcAnalyzeRequestedStub::new(pool.clone())))
        .with_handler(ev::PC_DECISION_SUBMITTED, Arc::new(PcDecisionSubmittedStub::new(pool.clone())))
        .with_handler(ev::PC_ANALYSIS_COMPLETED, Arc::new(PcAnalysisCompletedHandler::new(pool.clone())))
        .with_handler(ev::PC_CHANGE_PROPOSED, Arc::new(PcChangeProposedHandler::new(pool.clone())))
        .with_handler(ev::PC_CONFLICT_DETECTED, Arc::new(PcConflictDetectedHandler::new(pool.clone())))
        .with_handler(ev::PC_EVIDENCE_REQUESTED, Arc::new(PcEvidenceRequestedHandler::new(pool.clone())))
        .with_handler(ev::PROCESSING_FAILED, Arc::new(ProcessingFailedHandler::new(pool.clone())))
}
