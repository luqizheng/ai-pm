//! P3-3 RT 路由：kb.cognition_signal.emitted → pc_analyze 任务（§10.2）。

use super::*;
use kb_pc_contracts::events::PC_ANALYZE_REQUESTED;

// ── P3-3 RT 路由：kb.cognition_signal.emitted → pc_analyze 任务（§10.2） ────

/// 信号路由结果（审计/留痕用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteOutcome {
    /// 创建了 pc_analyze 任务（required 或 optional）。
    TaskCreated {
        phase: &'static str,
    },
    /// 合并进既有活跃任务（signal_ref 数组追加）。
    MergedIntoActive,
    /// relevance 过滤忽略（留痕不建任务）。
    FilteredNoRelevance,
    /// 任务已终态：按策略登记待办或创建补充任务。
    AfterTerminalTodo,
    AfterTerminalFollowUpCreated,
}

/// 信号面（RT 决策入参；来自 kb.cognition_signal.emitted payload）。
#[derive(Debug, Clone, Deserialize)]
pub struct SignalEnvelope {
    pub relevance: String,
    pub confidence: Option<String>,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub subject_refs: Vec<serde_json::Value>,
    #[serde(default)]
    pub evidence_refs: Vec<serde_json::Value>,
}

impl SignalEnvelope {
    /// 信号摘要 JSON（signal_ref 载荷；全四元组引用 + 摘要，禁正文）。
    pub fn to_signal_ref(&self) -> serde_json::Value {
        json!({
            "event": events::KB_COGNITION_SIGNAL_EMITTED,
            "relevance": self.relevance,
            "confidence": self.confidence,
            "rationale": self.rationale,
            "subject_refs": self.subject_refs,
            "evidence_refs": self.evidence_refs,
        })
    }

    /// 信号内容 hash（补充任务幂等键：intake_id + 信号内容，§9.4.3-3）。
    pub fn signal_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let canonical = serde_json::to_string(&json!({
            "relevance": self.relevance,
            "rationale": self.rationale,
            "subject_refs": self.subject_refs,
            "evidence_refs": self.evidence_refs,
        }))
        .unwrap_or_default();
        let digest = Sha256::digest(canonical.as_bytes());
        hex_prefix(digest.as_slice(), 16)
    }
}

fn hex_prefix(bytes: &[u8], n: usize) -> String {
    bytes.iter().take(n).map(|b| format!("{b:02x}")).collect()
}

/// RT 路由决策（kb.cognition_signal.emitted 消费入口；Platform 写入面）。
///
/// 事务性：任务行创建与 pc.analyze.requested 派发事件同 PG 事务（§10.2-6）。
/// 多信号合并：活跃任务存在 → signal_ref 合并（§9.4.3-1），不建新行。
pub async fn route_cognition_signal(
    pool: &PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
    intake_id: Uuid,
    signal: SignalEnvelope,
    policy: &RoutingPolicy,
) -> Result<RouteOutcome, IntakeError> {
    let phase = policy.decide_phase(
        &signal.relevance,
        signal.confidence.as_deref().and_then(Confidence::parse),
    );
    let Some(phase) = phase else {
        // 规则 1：NO_PRODUCT_RELEVANCE / 低置信 Ignore → 不创建任务，登记留痕。
        audit_route_decision(pool, project_id, intake_id, "filtered", &signal).await?;
        return Ok(RouteOutcome::FilteredNoRelevance);
    };

    // 频控/去重：活跃任务存在 → 合并（§10.2-3）。
    if policy.signal_merge {
        let merged = merge_signal_into_active(pool, intake_id, &signal).await?;
        if merged {
            return Ok(RouteOutcome::MergedIntoActive);
        }
    }

    // 终态后信号：按策略（§9.4.3-3）。
    let completed: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM integration.intake_task \
          WHERE intake_id = $1 AND task_type = 'pc_analyze' AND task_status = 'COMPLETED'",
    )
    .bind(intake_id)
    .fetch_optional(pool)
    .await?;
    if completed.is_some() {
        return match policy.after_terminal_action {
            AfterTerminalAction::RecordTodo => {
                audit_route_decision(pool, project_id, intake_id, "after_terminal_todo", &signal)
                    .await?;
                Ok(RouteOutcome::AfterTerminalTodo)
            }
            AfterTerminalAction::CreateFollowUpTask => {
                create_pc_analyze_task(
                    pool,
                    project_id,
                    tenant_id,
                    intake_id,
                    &format!("pc_analyze:{intake_id}:{}", signal.signal_hash()),
                    &signal,
                    phase,
                )
                .await?;
                Ok(RouteOutcome::AfterTerminalFollowUpCreated)
            }
        };
    }

    // 首信号：幂等键 pc_analyze:{intake_id}（§9.6-5）。
    create_pc_analyze_task(
        pool,
        project_id,
        tenant_id,
        intake_id,
        &format!("pc_analyze:{intake_id}"),
        &signal,
        phase,
    )
    .await?;
    Ok(RouteOutcome::TaskCreated { phase })
}

/// 活跃任务信号合并；返回是否发生合并。
async fn merge_signal_into_active(
    pool: &PgPool,
    intake_id: Uuid,
    signal: &SignalEnvelope,
) -> Result<bool, IntakeError> {
    let row: Option<(Uuid, serde_json::Value)> = sqlx::query_as(
        "SELECT id, signal_ref FROM integration.intake_task \
          WHERE intake_id = $1 AND task_type = 'pc_analyze' \
            AND task_status IN ('PENDING','DISPATCHED','PROCESSING') \
          ORDER BY created_at LIMIT 1",
    )
    .bind(intake_id)
    .fetch_optional(pool)
    .await?;
    let Some((task_id, existing)) = row else {
        return Ok(false);
    };
    // signal_ref 形态：{"signals": [...]}（首信号包装单元素）；合并=数组追加。
    let mut list: Vec<serde_json::Value> = match existing.get("signals") {
        Some(serde_json::Value::Array(items)) => items.clone(),
        _ => vec![],
    };
    if list.is_empty() {
        if let Some(first) = existing.get("first") {
            list.push(first.clone());
        }
    }
    list.push(signal.to_signal_ref());
    let updated = json!({ "signals": list });
    sqlx::query(
        "UPDATE integration.intake_task SET signal_ref = $2, updated_at = now() WHERE id = $1",
    )
    .bind(task_id)
    .bind(&updated)
    .execute(pool)
    .await?;
    Ok(true)
}

/// 创建 pc_analyze 任务 + 同事务派发事件（§10.2-6）。
#[allow(clippy::too_many_arguments)]
async fn create_pc_analyze_task(
    pool: &PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
    intake_id: Uuid,
    idempotency_key: &str,
    signal: &SignalEnvelope,
    phase: &str,
) -> Result<Uuid, IntakeError> {
    let mut tx = pool.begin().await?;
    let task_id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO integration.intake_task \
         (id, tenant_id, project_id, intake_id, target_service, task_type, phase, \
          task_status, idempotency_key, signal_ref) \
         VALUES ($1, $2, $3, $4, 'pc', 'pc_analyze', $5, 'PENDING', $6, $7) \
         ON CONFLICT (tenant_id, project_id, idempotency_key) DO NOTHING RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(tenant_id)
    .bind(project_id)
    .bind(intake_id)
    .bind(phase)
    .bind(idempotency_key)
    .bind(json!({ "signals": [signal.to_signal_ref()] }))
    .fetch_optional(&mut *tx)
    .await?;
    // 幂等键冲突 = 同键任务已存在（并发重复信号）：复用既有行，不再发事件
    // （outbox 幂等键 dispatch:{key} 也会坍缩，直接跳过语义更干净）。
    let Some(task_id) = task_id else {
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM integration.intake_task \
              WHERE tenant_id = $1 AND project_id = $2 AND idempotency_key = $3",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(idempotency_key)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        return existing.ok_or_else(|| {
            IntakeError::Validation("pc_analyze task conflict but no row found".into())
        });
    };
    let outbox = PgOutboxAppend;
    let payload = json!({
        "intake_ref": {
            "target_domain": "requirement",
            "target_type": "intake_record",
            "target_id": intake_id.to_string(),
            "target_revision_id": null,
        },
        "task_ref": {
            "target_domain": "requirement",
            "target_type": "intake_task",
            "target_id": task_id.to_string(),
            "target_revision_id": null,
        },
        "signal": signal.to_signal_ref(),
        "content_refs": signal.subject_refs,
    });
    outbox
        .append(
            &mut tx,
            OutboxEvent {
                project_id,
                event_type: PC_ANALYZE_REQUESTED,
                aggregate_type: "intake_task",
                aggregate_id: &task_id.to_string(),
                payload,
                idempotency_key: &format!("dispatch:{idempotency_key}"),
            },
        )
        .await?;
    tx.commit().await?;
    Ok(task_id)
}

/// 路由留痕（审计；governance.audit_log，source_kind='kb_pc_routing'）。
/// 系统面 dispatch principal：四身份列显式 NULL（C174 守卫；A5 同款）。
async fn audit_route_decision(
    pool: &PgPool,
    project_id: Uuid,
    intake_id: Uuid,
    decision: &str,
    signal: &SignalEnvelope,
) -> Result<(), IntakeError> {
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, after_data, source_kind, tenant_id) \
         VALUES ($1, $2, NULL, NULL, NULL, NULL, \
                 'kb_pc_signal_routed', 'intake_record', $3, $4, 'kb_pc_routing', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(crate::run_dispatch::DISPATCH_SERVICE_PRINCIPAL)
    .bind(intake_id)
    .bind(json!({
        "decision": decision,
        "relevance": signal.relevance,
        "confidence": signal.confidence,
        "signal_hash": signal.signal_hash(),
    }))
    .execute(pool)
    .await?;
    Ok(())
}
