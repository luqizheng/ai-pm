//! Intake 统一子任务载体（V1.1 §9）+ 流程路由策略 Routing Table（§10.2）。
//!
//! 职责边界（ADR-02 三分法）：Platform 拥有 `integration.intake_record` /
//! `intake_task` 的**唯一写入权**；KB/PC 的分析本体经事件回写结果，由
//! Platform 消费端推进任务状态（§9.4.2 状态迁移责任表）。
//!
//! 状态机（L2）：`PENDING → DISPATCHED → PROCESSING → COMPLETED | FAILED |
//! NEEDS_HUMAN_REVIEW | SKIPPED | CANCELLED`；FAILED 人工重试 = 复用原行
//! attempt+1 重置 PENDING（ADR-12，禁止新建行重试）。父 Intake 整体状态
//! （L3）= **查询时派生**（§9.4.2 聚合规则表），不落列——双写漂移不可表示。

use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use sqlx::Row;
use uuid::Uuid;

use kb_pc_contracts::events;

use crate::outbox::{OutboxAppend, OutboxEvent, PgOutboxAppend};

// ── L2 任务状态机（§9.4.2） ─────────────────────────────────────────────────

pub const TASK_ACTIVE_STATUSES: &[&str] = &["PENDING", "DISPATCHED", "PROCESSING"];
pub const TASK_TERMINAL_OK: &str = "COMPLETED";

/// 八类输入（V1.0 §10.1 路由矩阵；ADR-04 附属待确认 Q4 的增补走目录评审）。
pub const INTAKE_TYPES: &[&str] = &[
    "document_upload",
    "meeting_notes",
    "product_fact",
    "product_decision",
    "engineering_artifact",
    "engineering_result",
    "external_change",
    "analysis_request",
];

/// 接收通道（V1.0 §9.2）。
pub const INTAKE_CHANNELS: &[&str] = &["web", "mcp", "agent_api", "system"];

/// L1 接收壳状态（V1.0 §9.4）。
pub const INTAKE_STATUSES: &[&str] = &[
    "RECEIVED",
    "PERSISTED",
    "ROUTED",
    "CLOSED",
    "FAILED",
    "CANCELLED",
];

/// 任务类型（V1.1 §9.2 CHECK；intake_task.task_type 枚举）。
pub const TASK_TYPES: &[&str] = &[
    "kb_analyze",
    "pc_analyze",
    "pc_decide",
    "kb_archive",
    "kb_supply",
];

// ── P2-3 Routing Table（§10.2 配置化，非硬编码） ────────────────────────────

/// 信号置信度（§10.2 信号契约枚举）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Confidence {
    Low,
    Medium,
    High,
}

impl Confidence {
    pub fn parse(raw: &str) -> Option<Confidence> {
        match raw {
            "high" => Some(Confidence::High),
            "medium" => Some(Confidence::Medium),
            "low" => Some(Confidence::Low),
            _ => None,
        }
    }
}

/// 低于阈值信号的处理动作（§10.2-2「可选任务或忽略，策略配置」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LowConfidenceAction {
    /// 降级为可选任务（phase='optional'，人工可提升）。
    CreateOptionalTask,
    /// 忽略（留痕不建任务）。
    Ignore,
}

/// 信号到达时任务已 COMPLETED 的补充动作（§9.4.3-3〔待决〕策略配置）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AfterTerminalAction {
    /// 创建补充任务（task_type 同、新幂等键 pc_analyze:{intake_id}:{signal_hash}）。
    CreateFollowUpTask,
    /// 登记为人工待办（审计留痕，不建任务）。
    RecordTodo,
}

/// 流程路由策略（Routing Table；V1.1 §10.2，配置化承载——阈值与策略变更走
/// 配置评审，事件目录同机制）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingPolicy {
    /// relevance 过滤开关（§10.2-1）。
    pub relevance_filter: bool,
    /// confidence 阈值（默认 medium，§10.2-2）。
    pub confidence_threshold: Confidence,
    /// 低于阈值动作（默认创建可选任务）。
    pub low_confidence_action: LowConfidenceAction,
    /// 多信号合并开关（§10.2-3 频控/去重）。
    pub signal_merge: bool,
    /// 任务终态后信号到达的补充动作（§9.4.3-3，默认保守=人工待办）。
    pub after_terminal_action: AfterTerminalAction,
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        Self {
            relevance_filter: true,
            confidence_threshold: Confidence::Medium,
            low_confidence_action: LowConfidenceAction::CreateOptionalTask,
            signal_merge: true,
            after_terminal_action: AfterTerminalAction::RecordTodo,
        }
    }
}

impl RoutingPolicy {
    /// 环境变量覆写（配置化：`AI_PM_RT_CONFIDENCE_THRESHOLD=high|medium|low`、
    /// `AI_PM_RT_LOW_CONFIDENCE=create_optional|ignore`、
    /// `AI_PM_RT_RELEVANCE_FILTER=0|1`、`AI_PM_RT_SIGNAL_MERGE=0|1`、
    /// `AI_PM_RT_AFTER_TERMINAL=record_todo|create_follow_up`）。
    pub fn from_env() -> Self {
        let mut policy = Self::default();
        let read = |key: &str| std::env::var(key).ok().map(|v| v.trim().to_lowercase());
        if let Some(v) = read("AI_PM_RT_CONFIDENCE_THRESHOLD") {
            if let Some(c) = Confidence::parse(&v) {
                policy.confidence_threshold = c;
            }
        }
        if let Some(v) = read("AI_PM_RT_LOW_CONFIDENCE") {
            policy.low_confidence_action = if v == "ignore" {
                LowConfidenceAction::Ignore
            } else {
                LowConfidenceAction::CreateOptionalTask
            };
        }
        if let Some(v) = read("AI_PM_RT_RELEVANCE_FILTER") {
            policy.relevance_filter = v != "0" && v != "false";
        }
        if let Some(v) = read("AI_PM_RT_SIGNAL_MERGE") {
            policy.signal_merge = v != "0" && v != "false";
        }
        if let Some(v) = read("AI_PM_RT_AFTER_TERMINAL") {
            policy.after_terminal_action = if v == "create_follow_up" {
                AfterTerminalAction::CreateFollowUpTask
            } else {
                AfterTerminalAction::RecordTodo
            };
        }
        policy
    }

    /// 纯策略裁决（§10.2 规则 1~2；活跃合并/终态补充需查库，见
    /// [`route_cognition_signal`]）。返回任务 phase 或 None（不建任务）。
    pub fn decide_phase(
        &self,
        relevance: &str,
        confidence: Option<Confidence>,
    ) -> Option<&'static str> {
        if self.relevance_filter && relevance == "NO_PRODUCT_RELEVANCE" {
            return None;
        }
        match confidence {
            Some(c) if c >= self.confidence_threshold => Some("required"),
            Some(_) => match self.low_confidence_action {
                LowConfidenceAction::CreateOptionalTask => Some("optional"),
                LowConfidenceAction::Ignore => None,
            },
            // 信号缺 confidence：按不达标处理（fail-closed，不臆测）。
            None => match self.low_confidence_action {
                LowConfidenceAction::CreateOptionalTask => Some("optional"),
                LowConfidenceAction::Ignore => None,
            },
        }
    }
}

// ── 错误类型 ────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum IntakeError {
    #[error("intake not found (or not visible in this tenant/project)")]
    NotFound,
    #[error("task not found under intake {0}")]
    TaskNotFound(Uuid),
    #[error("task {0} is not retryable (status={1})")]
    NotRetryable(Uuid, String),
    #[error("invalid intake request: {0}")]
    Validation(String),
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
}

// ── P3-2 受理（同事务三写） ────────────────────────────────────────────────

/// 单个派生任务规格（接收方按 intake_type 路由矩阵派生，调用方可显式覆盖）。
#[derive(Debug, Clone)]
pub struct TaskSpec {
    pub target_service: String,
    pub task_type: String,
    pub phase: String,
}

/// 提交命令（POST /v1/pm/intakes 的应用层入参）。
pub struct SubmitIntake {
    pub project_id: Uuid,
    pub tenant_id: Uuid,
    pub submitted_by: Uuid,
    pub intake_type: String,
    pub channel: String,
    /// §7.4 Source Reference（{kind, uri|inline_ref, content_hash, size, mime}）。
    pub source_ref: serde_json::Value,
    pub content_hash: Option<String>,
    pub title: Option<String>,
    /// 受理幂等键（D-12：调用方凭 Idempotency-Key 安全重试；缺省按内容派生）。
    pub idempotency_key: Option<String>,
    /// 显式任务覆盖；缺省按 intake_type 路由矩阵派生。
    pub tasks: Option<Vec<TaskSpec>>,
}

/// 受理结果（202 纯受理语义：仅代表已登记+任务已创建，D-3）。
#[derive(Debug, Serialize)]
pub struct IntakeReceipt {
    pub intake_id: Uuid,
    pub status: &'static str,
    pub replayed: bool,
    pub tasks: Vec<TaskReceipt>,
}

#[derive(Debug, Serialize)]
pub struct TaskReceipt {
    pub task_id: Uuid,
    pub task_type: String,
    pub target_service: String,
    pub phase: String,
    pub task_status: String,
}

/// intake_type → 默认任务集（V1.0 §10.1 路由矩阵的主路由列；条件触发的
/// PC 分析不在此列——由 KB 信号经 RT 路由创建，ADR-02 三分法）。
fn default_tasks(intake_type: &str) -> Result<Vec<TaskSpec>, IntakeError> {
    let kb = TaskSpec {
        target_service: "kb".to_string(),
        task_type: "kb_analyze".to_string(),
        phase: "required".to_string(),
    };
    match intake_type {
        "document_upload"
        | "meeting_notes"
        | "product_fact"
        | "engineering_artifact"
        | "engineering_result"
        | "external_change" => Ok(vec![kb]),
        "product_decision" => Ok(vec![TaskSpec {
            target_service: "pc".to_string(),
            task_type: "pc_decide".to_string(),
            phase: "required".to_string(),
        }]),
        // 分析请求默认 KB（知识问答目标域可显式覆盖 tasks）。
        "analysis_request" => Ok(vec![kb]),
        other => Err(IntakeError::Validation(format!(
            "unknown intake_type `{other}` (allowed: {})",
            INTAKE_TYPES.join(", ")
        ))),
    }
}

/// 任务派发事件类型（按 task_type 选择；目录内自洽——kb_supply 归档类复用
/// kb.analysis.requested 的「提交资料分析」语义，见实施偏差登记）。
fn dispatch_event_type(task_type: &str) -> &'static str {
    match task_type {
        "pc_decide" => events::PC_DECISION_SUBMITTED,
        "pc_analyze" => events::PC_ANALYZE_REQUESTED,
        // kb_analyze / kb_archive / kb_supply 同走 kb.analysis.requested。
        _ => events::KB_ANALYSIS_REQUESTED,
    }
}

/// 受理 Intake：**同一 PG 事务三写**（intake_record + intake_task(s) +
/// outbox_event(s)，§9.3-7）；幂等键冲突 = 重复提交，返回既有 Intake
/// （重放标记，零副作用，D-12）。返回 202 纯受理语义。
pub async fn submit_intake(pool: &PgPool, cmd: SubmitIntake) -> Result<IntakeReceipt, IntakeError> {
    if !INTAKE_TYPES.contains(&cmd.intake_type.as_str()) {
        return Err(IntakeError::Validation(format!(
            "unknown intake_type `{}`",
            cmd.intake_type
        )));
    }
    if !INTAKE_CHANNELS.contains(&cmd.channel.as_str()) {
        return Err(IntakeError::Validation(format!(
            "unknown channel `{}`",
            cmd.channel
        )));
    }
    if cmd.source_ref.as_object().is_none() {
        return Err(IntakeError::Validation(
            "source_ref must be a JSON object".into(),
        ));
    }
    let tasks = match &cmd.tasks {
        Some(list) if !list.is_empty() => list.clone(),
        Some(_) => {
            return Err(IntakeError::Validation(
                "explicit tasks must not be empty".into(),
            ))
        }
        None => default_tasks(&cmd.intake_type)?,
    };
    // 任务幂等键 = {task_type}:{intake_id}（§9.6-5）；intake 幂等键 =
    // 显式提供或 {intake_type}:{content_hash||uuid}（D-12 重放锚）。
    let intake_id = Uuid::new_v4();
    let intake_key = cmd.idempotency_key.unwrap_or_else(|| {
        format!(
            "intake:{}:{}",
            cmd.intake_type,
            cmd.content_hash
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string())
        )
    });

    let mut tx = pool.begin().await?;
    // ① intake_record（UNIQUE(tenant_id, project_id, idempotency_key) 幂等锚；
    //    V1.1 §9.2 + D-12）。冲突 → 读既有行按重放返回。
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO integration.intake_record \
         (id, tenant_id, project_id, intake_type, channel, submitted_by, source_ref, \
          content_hash, title, intake_status, idempotency_key) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'ROUTED', $10) \
         ON CONFLICT (tenant_id, project_id, idempotency_key) DO NOTHING RETURNING id",
    )
    .bind(intake_id)
    .bind(cmd.tenant_id)
    .bind(cmd.project_id)
    .bind(&cmd.intake_type)
    .bind(&cmd.channel)
    .bind(cmd.submitted_by)
    .bind(&cmd.source_ref)
    .bind(&cmd.content_hash)
    .bind(&cmd.title)
    .bind(&intake_key)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(receipt_intake_id) = inserted else {
        // 幂等重放：读既有 intake 及其任务（零副作用，D-3/D-12）。
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM integration.intake_record \
              WHERE tenant_id = $1 AND project_id = $2 AND idempotency_key = $3",
        )
        .bind(cmd.tenant_id)
        .bind(cmd.project_id)
        .bind(&intake_key)
        .fetch_optional(&mut *tx)
        .await?;
        let existing = existing.ok_or(IntakeError::Db(sqlx::Error::RowNotFound))?;
        let tasks = sqlx::query(
            "SELECT id, task_type, target_service, phase, task_status \
               FROM integration.intake_task WHERE intake_id = $1 ORDER BY created_at",
        )
        .bind(existing)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(IntakeReceipt {
            intake_id: existing,
            status: "ROUTED",
            replayed: true,
            tasks: tasks
                .into_iter()
                .map(|row| TaskReceipt {
                    task_id: row.get("id"),
                    task_type: row.get("task_type"),
                    target_service: row.get("target_service"),
                    phase: row.get("phase"),
                    task_status: row.get("task_status"),
                })
                .collect(),
        });
    };

    // ② intake_task(s)（活跃部分唯一 uq_intake_task_active 防重）+ ③ outbox
    //    派发事件（任务已创建 ⇒ 事件必已入账，§10.2-6 同事务原子性）。
    let outbox = PgOutboxAppend;
    let mut receipts = Vec::with_capacity(tasks.len());
    for spec in &tasks {
        let task_id = Uuid::new_v4();
        let task_key = format!("{}:{}", spec.task_type, receipt_intake_id);
        let row = sqlx::query(
            "INSERT INTO integration.intake_task \
             (id, tenant_id, project_id, intake_id, target_service, task_type, phase, \
              task_status, idempotency_key) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'PENDING', $8) \
             RETURNING id",
        )
        .bind(task_id)
        .bind(cmd.tenant_id)
        .bind(cmd.project_id)
        .bind(receipt_intake_id)
        .bind(&spec.target_service)
        .bind(&spec.task_type)
        .bind(&spec.phase)
        .bind(&task_key)
        .fetch_one(&mut *tx)
        .await?;
        let created_task_id: Uuid = row.get(0);

        let payload = json!({
            "intake_ref": {
                "target_domain": "requirement",
                "target_type": "intake_record",
                "target_id": receipt_intake_id.to_string(),
                "target_revision_id": null,
            },
            "task_ref": {
                "target_domain": "requirement",
                "target_type": "intake_task",
                "target_id": created_task_id.to_string(),
                "target_revision_id": null,
            },
            "source_ref": cmd.source_ref,
        });
        outbox
            .append(
                &mut tx,
                OutboxEvent {
                    project_id: cmd.project_id,
                    event_type: dispatch_event_type(&spec.task_type),
                    aggregate_type: "intake_task",
                    aggregate_id: &created_task_id.to_string(),
                    payload,
                    idempotency_key: &task_key,
                },
            )
            .await?;
        receipts.push(TaskReceipt {
            task_id: created_task_id,
            task_type: spec.task_type.clone(),
            target_service: spec.target_service.clone(),
            phase: spec.phase.clone(),
            task_status: "PENDING".to_string(),
        });
    }
    // intake.received 登记事件（§13.3：intake 事务内；consumer=登记/通知）。
    outbox
        .append(
            &mut tx,
            OutboxEvent {
                project_id: cmd.project_id,
                event_type: events::INTAKE_RECEIVED,
                aggregate_type: "intake_record",
                aggregate_id: &receipt_intake_id.to_string(),
                payload: json!({
                    "intake_ref": {
                        "target_domain": "requirement",
                        "target_type": "intake_record",
                        "target_id": receipt_intake_id.to_string(),
                        "target_revision_id": null,
                    },
                    "intake_type": cmd.intake_type,
                    "content_hash": cmd.content_hash,
                }),
                idempotency_key: &format!("intake.received:{receipt_intake_id}"),
            },
        )
        .await?;

    tx.commit().await?;
    Ok(IntakeReceipt {
        intake_id: receipt_intake_id,
        status: "ROUTED",
        replayed: false,
        tasks: receipts,
    })
}

// ── P3-3 状态推进（Platform 唯一写入面；全部 CAS） ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_ordering_matches_contract() {
        assert!(Confidence::High > Confidence::Medium);
        assert!(Confidence::Medium > Confidence::Low);
    }

    #[test]
    fn routing_policy_relevance_filter() {
        let policy = RoutingPolicy::default();
        assert_eq!(
            policy.decide_phase("NO_PRODUCT_RELEVANCE", Some(Confidence::High)),
            None
        );
        assert_eq!(
            policy.decide_phase("PRODUCT_CANDIDATE", Some(Confidence::Medium)),
            Some("required")
        );
    }

    #[test]
    fn routing_policy_threshold_medium_default() {
        let policy = RoutingPolicy::default();
        assert_eq!(
            policy.decide_phase("PRODUCT_CANDIDATE", Some(Confidence::Low)),
            Some("optional")
        );
        let strict = RoutingPolicy {
            low_confidence_action: LowConfidenceAction::Ignore,
            ..Default::default()
        };
        assert_eq!(
            strict.decide_phase("PRODUCT_CANDIDATE", Some(Confidence::Low)),
            None
        );
        // 缺 confidence → fail-closed 按低置信处理。
        assert_eq!(
            policy.decide_phase("PRODUCT_CANDIDATE", None),
            Some("optional")
        );
    }

    #[test]
    fn aggregation_rules_follow_9_4_2() {
        use DerivedIntakeStatus::*;
        // NEEDS_HUMAN_REVIEW 优先。
        assert_eq!(
            aggregate_task_status(&[
                ("required".into(), "COMPLETED".into()),
                ("required".into(), "NEEDS_HUMAN_REVIEW".into()),
            ]),
            NeedsHumanReview
        );
        // 必需 FAILED 优先于活跃。
        assert_eq!(
            aggregate_task_status(&[
                ("required".into(), "FAILED".into()),
                ("required".into(), "PENDING".into()),
            ]),
            Failed
        );
        // 必需非终态 → PROCESSING；可选 FAILED 不阻塞。
        assert_eq!(
            aggregate_task_status(&[
                ("required".into(), "COMPLETED".into()),
                ("optional".into(), "FAILED".into()),
                ("required".into(), "PROCESSING".into()),
            ]),
            Processing
        );
        // 全部必需 COMPLETED（可选任意）→ COMPLETED（流程 F/B）。
        assert_eq!(
            aggregate_task_status(&[
                ("required".into(), "COMPLETED".into()),
                ("optional".into(), "SKIPPED".into()),
            ]),
            Completed
        );
        assert_eq!(aggregate_task_status(&[]), Empty);
    }

    #[test]
    fn signal_hash_is_stable_and_content_bound() {
        let s1 = SignalEnvelope {
            relevance: "PRODUCT_CANDIDATE".into(),
            confidence: Some("medium".into()),
            rationale: Some("r".into()),
            subject_refs: vec![],
            evidence_refs: vec![],
        };
        let s2 = s1.clone();
        assert_eq!(s1.signal_hash(), s2.signal_hash());
        let s3 = SignalEnvelope {
            rationale: Some("different".into()),
            ..s2.clone()
        };
        assert_ne!(s2.signal_hash(), s3.signal_hash());
    }
}

mod read;
mod signal;
mod task;

pub use read::*;
pub use signal::*;
pub use task::*;
