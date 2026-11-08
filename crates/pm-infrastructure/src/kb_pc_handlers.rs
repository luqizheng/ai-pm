//! KB-PC 服务化的消费端 handler（V1.1 §13.3 13 事件接线；P3-3 + checkpoint-1）。
//!
//! 职责切分（§9.4.2 状态迁移责任表）：
//! - **KB/PC 分析本体**（checkpoint-1 范围切割，`kb_pc_analysis` 模块头注同源）：
//!   `repository_snapshot` 输入的 kb_analyze 与携带 KB 文档引用的 pc_analyze 走
//!   **真实分析**（仓库快照 digest / 结构化派生骨架；可注入
//!   `SnapshotAnalysisModel` 做 LLM 受约束增强——P2-13 裁决 D-1=A，失败/无
//!   provider 回退派生并标 derived_fallback/derived，迁移 0062 analysis_mode；
//!   产物 draft/unverified 待人工确认）；其余输入保持 Phase 3 **最小桩**语义
//!   （完成回写、占位 ref、不产信号）——真实文档分析本体归 Phase 4。
//!   分析失败 → processing.failed → Platform 置 FAILED + 运维待办（人工重试
//!   入口，ADR-12）。
//! - **Platform 消费端**：消费结果/信号/失败事件，推进 intake_task 终态、
//!   RT 路由决策、通知留痕——intake 表唯一写入方是 Platform。
//!
//! 幂等纪律（§14.1 至少一次投递 ≠ 恰好一次执行）：全部 handler 重入安全——
//! 状态推进 CAS、结果事件 outbox 幂等键坍缩、通知 `WHERE NOT EXISTS` 锚。
//! KP-1 起 pc.cognition.version_published 接真（生产者=pc/publish.rs ⑫b，
//! Platform 侧状态感知留痕）。

use std::sync::Arc;

use sqlx::PgPool;

use pm_application::intake::{
    self, mark_task_dispatched, mark_task_processing, route_cognition_signal, RoutingPolicy,
    SignalEnvelope,
};
use pm_application::outbox::{OutboxAppend, OutboxEvent, PgOutboxAppend};
use pm_application::snapshot_analysis::SnapshotAnalysisModel;

use crate::kb_pc_analysis;
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

// ── KB 分析本体（消费 kb.analysis.requested；checkpoint-1 仓库快照路径真实化）

/// KB 分析 handler：仓库快照输入走真实分析（`kb_pc_analysis`：kb.document/
/// revision 落库 + 结果事件 + 认知信号，同一 PG 事务；可注入
/// `SnapshotAnalysisModel` 做 LLM 受约束增强）；其余输入保持 Phase 3
/// 桩语义（完成回写、占位 ref、不产信号——信号是 KB 条件产出，§10.2，禁止
/// 「每份资料必过 PC」）。分析失败 → processing.failed（可人工重试）。
pub struct KbAnalyzeRequestedHandler {
    pool: PgPool,
    snapshot_model: Option<Arc<dyn SnapshotAnalysisModel>>,
}

impl KbAnalyzeRequestedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            snapshot_model: None,
        }
    }

    /// 注入快照分析模型（P2-13/D-1=A）；None = 纯结构化派生（既有行为）。
    pub fn with_snapshot_model(mut self, model: Option<Arc<dyn SnapshotAnalysisModel>>) -> Self {
        self.snapshot_model = model;
        self
    }
}

#[async_trait::async_trait]
impl OutboxHandler for KbAnalyzeRequestedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let task_id = task_id_of(event)?;
        // 状态流转确认（Platform 写入面；重投递幂等）。
        mark_task_dispatched(&self.pool, task_id)
            .await
            .map_err(|e| e.to_string())?;
        mark_task_processing(&self.pool, task_id)
            .await
            .map_err(|e| e.to_string())?;

        let (project_id, source_ref): (uuid::Uuid, serde_json::Value) = sqlx::query_as(
            "SELECT t.project_id, i.source_ref \
               FROM integration.intake_task t \
               JOIN integration.intake_record i ON i.id = t.intake_id \
              WHERE t.id = $1",
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let Some(repository_id) = kb_pc_analysis::source_repository_id(&source_ref) else {
            // 非仓库输入：桩语义回退（Phase 4 前真实文档分析未落）。
            return complete_kb_as_stub(&self.pool, project_id, task_id, &source_ref).await;
        };

        match kb_pc_analysis::analyze_repository_snapshot(
            &self.pool,
            project_id,
            task_id,
            repository_id,
            source_ref.clone(),
            self.snapshot_model.as_deref(),
        )
        .await
        {
            Ok((_, _, _)) => Ok(()),
            Err(err) => {
                report_processing_failed(
                    &self.pool,
                    project_id,
                    task_id,
                    "kb_analysis_failed",
                    &err.to_string(),
                )
                .await
            }
        }
    }
}

// ── PC 分析本体（消费 pc.analyze.requested；checkpoint-1 结构化派生真实化）──

/// PC 分析 handler：信号携带 KB 文档引用时走真实结构化派生
/// （`kb_pc_analysis::derive_pc_cognition`：pc.\* draft + source/evidence，
/// conclusion=PROPOSED，M3 前不产正式认知写入；可注入
/// `SnapshotAnalysisModel` 做受约束增强，失败回退派生）；无文档引用
/// 的信号保持桩语义（NO_CHANGE，占位 ref）。派生失败 → processing.failed。
pub struct PcAnalyzeRequestedHandler {
    pool: PgPool,
    snapshot_model: Option<Arc<dyn SnapshotAnalysisModel>>,
}

impl PcAnalyzeRequestedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            snapshot_model: None,
        }
    }

    /// 注入快照分析模型（P2-13/D-1=A）；None = 纯结构化派生（既有行为）。
    pub fn with_snapshot_model(mut self, model: Option<Arc<dyn SnapshotAnalysisModel>>) -> Self {
        self.snapshot_model = model;
        self
    }
}

#[async_trait::async_trait]
impl OutboxHandler for PcAnalyzeRequestedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let task_id = task_id_of(event)?;
        mark_task_dispatched(&self.pool, task_id)
            .await
            .map_err(|e| e.to_string())?;
        mark_task_processing(&self.pool, task_id)
            .await
            .map_err(|e| e.to_string())?;

        let (project_id, intake_id, signal_ref): (
            uuid::Uuid,
            uuid::Uuid,
            Option<serde_json::Value>,
        ) = sqlx::query_as(
            "SELECT project_id, intake_id, signal_ref \
               FROM integration.intake_task WHERE id = $1",
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        // 信号（多信号合并取末位；RT 保证创建时至少一条）。
        let signal = signal_ref
            .as_ref()
            .and_then(|r| r.get("signals"))
            .and_then(|v| v.as_array())
            .and_then(|a| a.last().cloned());
        let Some(signal) = signal else {
            return complete_pc_as_stub(&self.pool, project_id, task_id).await;
        };
        let Some((_, kb_revision_id)) = kb_pc_analysis::kb_document_ref_of(&signal) else {
            return complete_pc_as_stub(&self.pool, project_id, task_id).await;
        };

        // KB 修订行（document 归属 + 内容指纹以修订行为准）。
        let (kb_revision_id, kb_document_id, kb_content_hash): (uuid::Uuid, uuid::Uuid, String) =
            sqlx::query_as(
                "SELECT id, document_id, content_hash FROM kb.document_revision WHERE id = $1",
            )
            .bind(kb_revision_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        // 仓库事实（缓存复用优先；intake 非仓库输入 → 桩语义）。
        let source_ref: serde_json::Value =
            sqlx::query_scalar("SELECT source_ref FROM integration.intake_record WHERE id = $1")
                .bind(intake_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        let Some(repository_id) = kb_pc_analysis::source_repository_id(&source_ref) else {
            return complete_pc_as_stub(&self.pool, project_id, task_id).await;
        };
        let (facts, _) =
            match kb_pc_analysis::collect_repository_facts(&self.pool, repository_id).await {
                Ok(v) => v,
                Err(err) => {
                    return report_processing_failed(
                        &self.pool,
                        project_id,
                        task_id,
                        "pc_derive_failed",
                        &err.to_string(),
                    )
                    .await
                }
            };

        match kb_pc_analysis::derive_pc_cognition(
            &self.pool,
            project_id,
            intake_id,
            task_id,
            repository_id,
            kb_document_id,
            kb_revision_id,
            &kb_content_hash,
            &facts,
            self.snapshot_model.as_deref(),
        )
        .await
        {
            // KP-1 起 pc_analysis_ref 锚分析运行的 pc.source 行（一次分析一条
            // 来源），P3 经 source → evidence → revision → object 展开；
            // target_revision_id 携带 KB 修订（来源钉扎）。
            Ok((source_id, _primary_revision)) => {
                let result_ref = serde_json::json!({
                    "target_domain": "pc",
                    "target_type": "source",
                    "target_id": source_id.to_string(),
                    "target_revision_id": kb_revision_id.to_string(),
                });
                let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
                PgOutboxAppend
                    .append(
                        &mut tx,
                        OutboxEvent {
                            project_id,
                            event_type: ev::PC_ANALYSIS_COMPLETED,
                            aggregate_type: "intake_task",
                            aggregate_id: &task_id.to_string(),
                            payload: serde_json::json!({
                                "pc_analysis_ref": result_ref,
                                "conclusion": "PROPOSED",
                                "refs": [result_ref],
                                "task_ref": task_ref_json(task_id),
                            }),
                            idempotency_key: &format!("pc_analysis:{task_id}"),
                        },
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                tx.commit().await.map_err(|e| e.to_string())?;
                tracing::info!(%task_id, %intake_id, "pc: analysis completed (PROPOSED)");
                Ok(())
            }
            Err(err) => {
                report_processing_failed(
                    &self.pool,
                    project_id,
                    task_id,
                    "pc_derive_failed",
                    &err.to_string(),
                )
                .await
            }
        }
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
        mark_task_dispatched(&self.pool, task_id)
            .await
            .map_err(|e| e.to_string())?;
        mark_task_processing(&self.pool, task_id)
            .await
            .map_err(|e| e.to_string())?;

        use sha2::{Digest, Sha256};
        let content_hash = {
            let canonical = serde_json::to_string(
                event
                    .payload
                    .get("decision")
                    .unwrap_or(&serde_json::Value::Null),
            )
            .unwrap_or_default();
            let digest = Sha256::digest(canonical.as_bytes());
            digest
                .iter()
                .take(16)
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
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
        let (project_id, tenant_id, real_intake_id): (uuid::Uuid, uuid::Uuid, uuid::Uuid) =
            sqlx::query_as(
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

/// pc.cognition.version_published → 状态感知留痕（Platform 消费面；KP-1 起
/// 发布事务真实生产，目录 has_handler 翻真——「不假装消费」要求必须有 handler）。
pub struct PcVersionPublishedHandler {
    pool: PgPool,
}

impl PcVersionPublishedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for PcVersionPublishedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let version_id = event
            .payload
            .get("version_ref")
            .and_then(|r| r.get("target_id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "version_published missing version_ref.target_id".to_string())?
            .to_string();
        notify_once(
            &self.pool,
            event.project_id,
            "pc_version_published",
            &version_id,
            serde_json::json!({ "dedup_key": version_id, "payload": event.payload }),
        )
        .await
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
            &format!(
                "{failed_id}:{}",
                event
                    .payload
                    .get("retry_count")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
            ),
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

// ── 桩语义回退 + 失败上报（checkpoint-1 范围切割的边界出口）───────────────

/// 非仓库输入的 KB 完成回写（Phase 3 桩语义原样保留：占位 ref、不产信号）。
async fn complete_kb_as_stub(
    pool: &PgPool,
    project_id: uuid::Uuid,
    task_id: uuid::Uuid,
    source_ref: &serde_json::Value,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let analysis_id = format!("kb-analysis-{task_id}");
    PgOutboxAppend
        .append(
            &mut tx,
            OutboxEvent {
                project_id,
                event_type: ev::KB_ANALYSIS_COMPLETED,
                aggregate_type: "intake_task",
                aggregate_id: &task_id.to_string(),
                payload: serde_json::json!({
                    "kb_analysis_ref": domain_ref_json("knowledge_base", "kb_analysis", &analysis_id),
                    "source_ref": source_ref,
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
    tracing::info!(%task_id, "kb stub: analysis completed (non-repository input)");
    Ok(())
}

/// 无 KB 文档引用的 PC 完成回写（Phase 3 桩语义原样保留：NO_CHANGE）。
async fn complete_pc_as_stub(
    pool: &PgPool,
    project_id: uuid::Uuid,
    task_id: uuid::Uuid,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    PgOutboxAppend
        .append(
            &mut tx,
            OutboxEvent {
                project_id,
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

/// 分析失败上报：processing.failed → Platform 置 FAILED + 运维待办
/// （人工重试入口，ADR-12；幂等键锚 task+attempt，重投递坍缩）。
async fn report_processing_failed(
    pool: &PgPool,
    project_id: uuid::Uuid,
    task_id: uuid::Uuid,
    error_code: &str,
    detail: &str,
) -> Result<(), String> {
    let attempt: i32 =
        sqlx::query_scalar("SELECT attempt FROM integration.intake_task WHERE id = $1")
            .bind(task_id)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    PgOutboxAppend
        .append(
            &mut tx,
            OutboxEvent {
                project_id,
                event_type: ev::PROCESSING_FAILED,
                aggregate_type: "intake_task",
                aggregate_id: &task_id.to_string(),
                payload: serde_json::json!({
                    "failed_ref": domain_ref_json("requirement", "intake_task", &task_id.to_string()),
                    "error_code": error_code,
                    "error_detail": detail,
                    "retry_count": attempt,
                    "recoverable": true,
                }),
                idempotency_key: &format!("processing_failed:{task_id}:{attempt}"),
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    tracing::warn!(%task_id, %error_code, "analysis failed → processing.failed reported");
    Ok(())
}

/// **唯一的生产 Handler 装配入口扩展**：把 KB-PC 13 事件的 12 个消费者
/// 追加进既有装配（TaskReady/DeliverySubmitted/RunCompleted 之后）。
/// `snapshot_model` 注入两个分析 handler（P2-13/D-1=A；None = 纯派生）。
/// 装配全集由 `OutboxWorker::assert_catalog_consistent` 启动校验兜底。
pub fn register_kb_pc_handlers(
    worker: OutboxWorker,
    pool: &PgPool,
    snapshot_model: Option<Arc<dyn SnapshotAnalysisModel>>,
) -> OutboxWorker {
    worker
        .with_handler(
            ev::INTAKE_RECEIVED,
            Arc::new(IntakeReceivedHandler::new(pool.clone())),
        )
        .with_handler(
            ev::KB_ANALYSIS_REQUESTED,
            Arc::new(
                KbAnalyzeRequestedHandler::new(pool.clone())
                    .with_snapshot_model(snapshot_model.clone()),
            ),
        )
        .with_handler(
            ev::KB_ANALYSIS_COMPLETED,
            Arc::new(KbAnalysisCompletedHandler::new(pool.clone())),
        )
        .with_handler(
            ev::KB_COGNITION_SIGNAL_EMITTED,
            Arc::new(CognitionSignalRouterHandler::new(pool.clone())),
        )
        .with_handler(
            ev::KB_MEMORY_APPROVED,
            Arc::new(KbMemoryApprovedHandler::new(pool.clone())),
        )
        .with_handler(
            ev::PC_ANALYZE_REQUESTED,
            Arc::new(
                PcAnalyzeRequestedHandler::new(pool.clone()).with_snapshot_model(snapshot_model),
            ),
        )
        .with_handler(
            ev::PC_DECISION_SUBMITTED,
            Arc::new(PcDecisionSubmittedStub::new(pool.clone())),
        )
        .with_handler(
            ev::PC_ANALYSIS_COMPLETED,
            Arc::new(PcAnalysisCompletedHandler::new(pool.clone())),
        )
        .with_handler(
            ev::PC_CHANGE_PROPOSED,
            Arc::new(PcChangeProposedHandler::new(pool.clone())),
        )
        .with_handler(
            ev::PC_CONFLICT_DETECTED,
            Arc::new(PcConflictDetectedHandler::new(pool.clone())),
        )
        .with_handler(
            ev::PC_EVIDENCE_REQUESTED,
            Arc::new(PcEvidenceRequestedHandler::new(pool.clone())),
        )
        .with_handler(
            ev::PC_COGNITION_VERSION_PUBLISHED,
            Arc::new(PcVersionPublishedHandler::new(pool.clone())),
        )
        .with_handler(
            ev::PROCESSING_FAILED,
            Arc::new(ProcessingFailedHandler::new(pool.clone())),
        )
}
