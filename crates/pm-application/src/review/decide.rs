//! 完成判定器与审批裁决事务（D5 §5）。
//!
//! 核心不变量：**Review 结论只是「能不能完成」的输入，不是完成本身**。
//! `reviewing→completed` 的出口由本模块的唯一判定器给出，且必须过 E2 前置全集
//! （§5.3）；`integration_status='failed'` 是人机均不可越的硬禁令。
//!
//! 矩阵（§5.1）的出口映射全部落在 [`advance_after_review`]：
//!
//! | verdict | 策略 | 出口 |
//! |---|---|---|
//! | `approved` | `auto_accept` 放行 | `reviewing→completed`（原子集：提案 + 两事件） |
//! | `approved` | 需人工 | `review_approval(kind='auto_accept_gate')` + `task_waiting` |
//! | `changes_requested` | 任意 | `reviewing→changes_requested` |
//! | `inconclusive` | 任意 | `review_approval(kind='manual_review')` + `task_waiting` |
//! | 无 verdict（run failed/timed_out） | 任意 | 重试 / 耗尽 → `review_approval(kind='review_degraded')` |

use serde::{Deserialize, Serialize};
use sqlx::{Row, Transaction};
use uuid::Uuid;

use pm_domain::TaskStatus;

use crate::command_executor::{apply_task_transition, system_context, TransitionFacts};
use crate::outbox::{OutboxAppend, OutboxEvent};
use crate::policy_composition::EffectivePolicy;
use crate::review::pipeline::ReviewOutcome;
use crate::review::queue::{audit_review, ReviewError};
use crate::run_dispatch::run_status;

/// 审批类别（`governance.review_approval.kind` 的 CHECK 取值）。
pub mod approval_kind {
    pub const AUTO_ACCEPT_GATE: &str = "auto_accept_gate";
    pub const MANUAL_REVIEW: &str = "manual_review";
    pub const REVIEW_DEGRADED: &str = "review_degraded";
}

/// E2 前置全集的机械读数（§5.3）。
#[derive(Debug, Clone)]
pub struct CompletionGate {
    pub review_verdict: String,
    pub integration_status: String,
    pub pending_manual_criteria: usize,
    pub failed_criteria: usize,
    pub policy: EffectivePolicy,
}

impl CompletionGate {
    /// 硬禁令：`integration_status='failed'` 人机均不可越。
    pub fn hard_blocked(&self) -> bool {
        self.integration_status == "failed"
    }

    /// 是否存在未决的人工裁决条目（`manual` / `review`）。
    pub fn has_pending_manual(&self) -> bool {
        self.pending_manual_criteria > 0
    }

    /// 逐条裁决是否有失败项。
    pub fn has_failed_criteria(&self) -> bool {
        self.failed_criteria > 0
    }

    /// 是否可自动完成（E2 前置 3/4 + 5）。
    pub fn auto_completable(&self) -> bool {
        self.review_verdict == "approved"
            && !self.hard_blocked()
            && !self.has_pending_manual()
            && !self.has_failed_criteria()
            && self.policy.auto_accept
            && self.policy.additional_checks.is_empty()
            && !self.policy.require_human_approval
    }

    /// 拒绝自动放行的原因（诊断用，机器可读）。
    pub fn manual_reason(&self) -> &'static str {
        if self.hard_blocked() {
            "integration_failed"
        } else if self.has_failed_criteria() {
            "criteria_failed"
        } else if self.has_pending_manual() {
            "criteria_awaiting_manual"
        } else if !self.policy.additional_checks.is_empty() {
            "additional_checks"
        } else if self.policy.require_human_approval {
            "require_human_approval"
        } else if !self.policy.auto_accept {
            "auto_accept_disabled"
        } else {
            "unknown"
        }
    }
}

/// 读取任务的四层策略并合成（`task_type` 缺失时只合成三层）。
pub(crate) async fn load_effective_policy(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<EffectivePolicy, ReviewError> {
    let task_type: Option<String> =
        sqlx::query_scalar("SELECT task_type FROM delivery.task WHERE id = $1")
            .bind(task_id)
            .fetch_one(&mut **tx)
            .await?;
    let layers =
        crate::run_dispatch::load_policy_layers(tx, project_id, task_id, task_type.as_deref())
            .await?;
    Ok(EffectivePolicy::compose(&layers))
}

/// 组装 E2 判定读数（读库，不做裁决）。
pub async fn load_completion_gate(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    review_run_id: Uuid,
) -> Result<CompletionGate, ReviewError> {
    let verdict: Option<String> =
        sqlx::query_scalar("SELECT review_verdict FROM delivery.task_run WHERE id = $1")
            .bind(review_run_id)
            .fetch_one(&mut **tx)
            .await?;
    let reviewed_run: Uuid = sqlx::query_scalar(
        "SELECT id FROM delivery.task_run \
          WHERE task_id = $1 AND project_id = $2 AND run_kind = 'delivery' \
          ORDER BY attempt_no DESC, created_at DESC LIMIT 1",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ReviewError::Transition("no delivery run to judge".to_string()))?;
    let integration_status: String =
        sqlx::query_scalar("SELECT integration_status FROM delivery.task_run WHERE id = $1")
            .bind(reviewed_run)
            .fetch_one(&mut **tx)
            .await?;

    let failed_criteria: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.acceptance_criterion_verdict \
          WHERE review_run_id = $1 AND verdict = 'failed'",
    )
    .bind(review_run_id)
    .fetch_one(&mut **tx)
    .await?;
    let pending_manual: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_acceptance_criterion c \
          WHERE c.task_id = $1 \
            AND c.verification_kind IN ('manual', 'review') \
            AND NOT EXISTS (SELECT 1 FROM delivery.acceptance_criterion_verdict v \
                             WHERE v.criterion_id = c.id AND v.review_run_id = $2)",
    )
    .bind(task_id)
    .bind(review_run_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(CompletionGate {
        review_verdict: verdict.unwrap_or_else(|| "none".to_string()),
        integration_status,
        pending_manual_criteria: pending_manual as usize,
        failed_criteria: failed_criteria as usize,
        policy: load_effective_policy(tx, project_id, task_id).await?,
    })
}

/// 审批裁决的结果类别。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ApprovalOutcome {
    Completed {
        task_id: Uuid,
        proposal_id: Option<Uuid>,
    },
    ChangesRequested {
        task_id: Uuid,
    },
    AlreadyDecided {
        status: String,
    },
}

/// 审批裁决的错误（HTTP 映射见 `pm-api`）。
#[derive(Debug, thiserror::Error)]
pub enum ApprovalError {
    #[error("approval not found")]
    NotFound,
    #[error("approval already decided: {0}")]
    AlreadyDecided(String),
    /// 审批锚定的交付快照与被审 run 当前锚定不一致（§5.2 步骤 2）。
    #[error("anchor drift: {0}")]
    AnchorDrift(String),
    /// E2 前置复核拒绝（§5.2 步骤 3）。
    #[error("completion gate refused: {0}")]
    GateRefused(String),
    #[error("review run is not succeeded")]
    ReviewNotSucceeded,
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("state transition refused: {0}")]
    Transition(String),
}

impl ApprovalError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "approval_not_found",
            Self::AlreadyDecided(_) => "already_decided",
            Self::AnchorDrift(_) => "anchor_drift",
            Self::GateRefused(_) => "completion_gate_refused",
            Self::ReviewNotSucceeded => "review_not_succeeded",
            Self::Db(_) => "db_error",
            Self::Transition(_) => "transition_refused",
        }
    }
}

/// 审批裁决请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDecisionRequest {
    pub project_id: Uuid,
    pub approval_id: Uuid,
    /// `approved` / `rejected`。
    pub decision: String,
    pub decided_by: Uuid,
    pub decision_reason: Option<String>,
}

/// **批次推进结果**（拍②收尾的落点）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum AdvanceOutcome {
    /// `reviewing→completed`（含同事务的 draft 提案与两事件）。
    Completed { proposal_id: Option<Uuid> },
    /// `reviewing→changes_requested`。
    ChangesRequested,
    /// 降级人工：`review_approval` + `task_waiting`，任务停 `reviewing`。
    AwaitingApproval { approval_id: Uuid, kind: String },
}

/// 拍②收尾：按 §5.1 矩阵推进任务（**调用方必须已在本事务内**落好 findings +
/// 逐条裁决 + `review_verdict` + review run 终态）。
pub async fn advance_after_review(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    outbox: &dyn OutboxAppend,
    project_id: Uuid,
    review_run_id: Uuid,
    delivery_run_id: Uuid,
    outcome: &ReviewOutcome,
    change_summary: Option<String>,
) -> Result<AdvanceOutcome, ReviewError> {
    let task_id: Uuid = sqlx::query_scalar("SELECT task_id FROM delivery.task_run WHERE id = $1")
        .bind(review_run_id)
        .fetch_one(&mut **tx)
        .await?;

    let verdict = outcome.verdict.as_str().to_string();
    let findings_summary: Vec<String> = outcome
        .findings
        .iter()
        .map(|finding| finding.message.clone())
        .collect();

    audit_review(
        tx,
        project_id,
        if verdict == "changes_requested" {
            "review.changes_requested"
        } else {
            "review.completed"
        },
        review_run_id,
        serde_json::json!({
            "verdict": verdict,
            "delivery_run_id": delivery_run_id,
            "finding_count": outcome.findings.len(),
            "blocker_count": outcome.blocker_count(),
        }),
    )
    .await?;

    let action = match verdict.as_str() {
        "changes_requested" => AdvanceOutcome::ChangesRequested,
        "approved" => {
            let gate = load_completion_gate(tx, project_id, task_id, review_run_id).await?;
            if gate.hard_blocked() || gate.has_failed_criteria() {
                // 防御性：硬禁令命中时 approved 本不应出现（L2 已挡）。
                AdvanceOutcome::ChangesRequested
            } else if gate.auto_completable() {
                let proposal_id = complete_task(
                    tx,
                    outbox,
                    project_id,
                    task_id,
                    review_run_id,
                    delivery_run_id,
                    &verdict,
                    findings_summary,
                    change_summary,
                )
                .await?;
                AdvanceOutcome::Completed { proposal_id }
            } else {
                let approval_id = open_approval(
                    tx,
                    project_id,
                    task_id,
                    review_run_id,
                    delivery_run_id,
                    approval_kind::AUTO_ACCEPT_GATE,
                    Some(gate.manual_reason()),
                )
                .await?;
                AdvanceOutcome::AwaitingApproval {
                    approval_id,
                    kind: approval_kind::AUTO_ACCEPT_GATE.to_string(),
                }
            }
        }
        _ => {
            // inconclusive：降级人工，**不重试、不得当 approved**（Q8.5）。
            let approval_id = open_approval(
                tx,
                project_id,
                task_id,
                review_run_id,
                delivery_run_id,
                approval_kind::MANUAL_REVIEW,
                Some("review_inconclusive"),
            )
            .await?;
            AdvanceOutcome::AwaitingApproval {
                approval_id,
                kind: approval_kind::MANUAL_REVIEW.to_string(),
            }
        }
    };

    // changes_requested 出口：状态推进 + 事件（不改被审对象）。
    if matches!(action, AdvanceOutcome::ChangesRequested) {
        transition_task(tx, project_id, task_id, TaskStatus::ChangesRequested).await?;
        emit_event(
            tx,
            outbox,
            project_id,
            "ReviewCompleted",
            review_run_id,
            format!("review_completed:{review_run_id}"),
            serde_json::json!({
                "review_run_id": review_run_id,
                "task_id": task_id,
                "verdict": verdict,
                "task_status": "changes_requested",
            }),
        )
        .await?;
    }
    Ok(action)
}

/// `reviewing→completed` 的**原子集**（§7.1）：状态迁移 + `task_waiting` 结行 +
/// 审计（调用方已写）+ `draft` 提案 INSERT + 两事件。
#[allow(clippy::too_many_arguments)]
async fn complete_task(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    outbox: &dyn OutboxAppend,
    project_id: Uuid,
    task_id: Uuid,
    review_run_id: Uuid,
    delivery_run_id: Uuid,
    verdict: &str,
    findings_summary: Vec<String>,
    change_summary: Option<String>,
) -> Result<Option<Uuid>, ReviewError> {
    let ctx = system_context(tx, project_id)
        .await
        .map_err(|e| ReviewError::Transition(e.to_string()))?;
    // E2 守卫：完成判定器全过 → `acceptance_evidence_complete = true`。
    // 这是 D5 唯一可以置该事实的位置（D2 命令通道的硬置 true 只用于人工通道）。
    let facts = TransitionFacts {
        delivery_submitted: true,
        acceptance_evidence_complete: true,
        ..TransitionFacts::default()
    };
    apply_task_transition(tx, task_id, None, TaskStatus::Completed, &facts, &ctx)
        .await
        .map_err(|e| ReviewError::Transition(e.to_string()))?;
    end_waiting(tx, task_id, "review completed").await?;

    let writeback = crate::review::knowledge::assemble_writeback(
        tx,
        task_id,
        delivery_run_id,
        verdict,
        findings_summary,
        change_summary.clone(),
    )
    .await?;
    let proposal_id = crate::review::knowledge::write_proposal_in_tx(
        tx,
        project_id,
        task_id,
        review_run_id,
        &writeback,
    )
    .await?;

    emit_event(
        tx,
        outbox,
        project_id,
        "ReviewCompleted",
        review_run_id,
        format!("review_completed:{review_run_id}"),
        serde_json::json!({
            "review_run_id": review_run_id,
            "task_id": task_id,
            "verdict": verdict,
            "task_status": "completed",
        }),
    )
    .await?;
    emit_event(
        tx,
        outbox,
        project_id,
        "KnowledgeBaseWriteRequested",
        review_run_id,
        format!("knowledge_writeback:{review_run_id}"),
        serde_json::json!({
            "review_run_id": review_run_id,
            "task_id": task_id,
            "proposal_id": proposal_id,
            "doc_key": format!("task-{task_id}"),
            "change_summary": change_summary,
        }),
    )
    .await?;

    Ok(proposal_id)
}

/// 降级人工（重试耗尽）：`review_approval(kind='review_degraded')` + `task_waiting`。
pub(crate) async fn open_review_degraded(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    review_run_id: Uuid,
    delivery_run_id: Uuid,
) -> Result<Uuid, ReviewError> {
    open_approval(
        tx,
        project_id,
        task_id,
        review_run_id,
        delivery_run_id,
        approval_kind::REVIEW_DEGRADED,
        Some("review_retry_exhausted"),
    )
    .await
}

/// 开一条 pending 审批行 + `task_waiting(needs_approval)`（同事务，§5.4 不变量 3）。
async fn open_approval(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    review_run_id: Uuid,
    delivery_run_id: Uuid,
    kind: &str,
    reason: Option<&str>,
) -> Result<Uuid, ReviewError> {
    let manifest_hash: String =
        sqlx::query_scalar("SELECT reviewed_manifest_hash FROM delivery.task_run WHERE id = $1")
            .bind(review_run_id)
            .fetch_one(&mut **tx)
            .await?;
    let approval_id: Uuid = sqlx::query_scalar(
        "INSERT INTO governance.review_approval \
         (project_id, task_id, review_run_id, delivery_run_id, delivery_manifest_hash, kind, \
          status, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, 'pending', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(review_run_id)
    .bind(delivery_run_id)
    .bind(&manifest_hash)
    .bind(kind)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        "INSERT INTO delivery.task_waiting \
         (project_id, task_id, reason, questions, tenant_id) \
         VALUES ($1, $2, 'needs_approval', $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(serde_json::json!([{ "approval_id": approval_id, "kind": kind, "reason": reason }]))
    .execute(&mut **tx)
    .await?;
    Ok(approval_id)
}

/// 审批裁决的**单事务**（§5.2 七步）。
pub async fn decide_review_approval(
    pool: &sqlx::PgPool,
    outbox: &dyn OutboxAppend,
    request: &ReviewDecisionRequest,
) -> Result<ApprovalOutcome, ApprovalError> {
    if request.decision != "approved" && request.decision != "rejected" {
        return Err(ApprovalError::GateRefused(format!(
            "invalid decision `{}`",
            request.decision
        )));
    }
    let mut tx = pool.begin().await?;

    // 1) 行锁 pending 审批行。
    let row = sqlx::query(
        "SELECT task_id, review_run_id, delivery_run_id, delivery_manifest_hash, status \
           FROM governance.review_approval \
          WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(request.approval_id)
    .bind(request.project_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApprovalError::NotFound)?;
    let status: String = row.get("status");
    if status != "pending" {
        tx.rollback().await.ok();
        return Ok(ApprovalOutcome::AlreadyDecided { status });
    }
    let task_id: Uuid = row.get("task_id");
    let review_run_id: Uuid = row.get("review_run_id");
    let delivery_run_id: Uuid = row.get("delivery_run_id");
    let anchored_hash: String = row.get("delivery_manifest_hash");

    // 2) 锚定回读校验（I2）：审批锚定的交付快照必须仍是被审 run 的锚定值。
    let run = sqlx::query(
        "SELECT status, reviewed_manifest_hash, review_verdict \
           FROM delivery.task_run WHERE id = $1",
    )
    .bind(review_run_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApprovalError::GateRefused("review run missing".to_string()))?;
    let run_status: String = run.get("status");
    if run_status != run_status::SUCCEEDED {
        return Err(ApprovalError::ReviewNotSucceeded);
    }
    let current_hash: Option<String> = run.get("reviewed_manifest_hash");
    if current_hash.as_deref() != Some(anchored_hash.as_str()) {
        return Err(ApprovalError::AnchorDrift(format!(
            "approval anchors {anchored_hash} but the review run anchors {current_hash:?}"
        )));
    }

    // 3) E2 前置全集复核（approve 路径必过，含硬禁令）。
    if request.decision == "approved" {
        let gate = load_completion_gate(&mut tx, request.project_id, task_id, review_run_id)
            .await
            .map_err(|e| ApprovalError::GateRefused(e.to_string()))?;
        if gate.hard_blocked() {
            return Err(ApprovalError::GateRefused(
                "integration_status=failed".to_string(),
            ));
        }
        if gate.has_failed_criteria() {
            return Err(ApprovalError::GateRefused(
                "a criterion is already judged failed".to_string(),
            ));
        }
        if gate.integration_status != "passed" {
            // `not_run` 是「仅人工可裁」的层：人工批准可放行，但必须有证据面说明。
            tracing::info!(
                task = %task_id,
                "approving a task with integration_status=not_run by human decision"
            );
        }
    }

    // 4) UPDATE 审批行（锚定列 once-written，触发器只允许 pending → 终态）。
    sqlx::query(
        "UPDATE governance.review_approval \
            SET status = $2, decided_by = $3, decision_reason = $4, decided_at = now() \
          WHERE id = $1",
    )
    .bind(request.approval_id)
    .bind(&request.decision)
    .bind(request.decided_by)
    .bind(request.decision_reason.as_deref())
    .execute(&mut *tx)
    .await?;

    // 5) 状态迁移（approve → completed；reject → changes_requested + 理由转 finding）。
    let outcome = if request.decision == "approved" {
        // 人工裁决的终值行：`manual` / `review` 条目在管线阶段**不预插裁决行**
        // （§2.5 修正），终值行在此裁决事务内 INSERT（结论 = 人工批准）。
        insert_manual_verdicts(&mut tx, request, task_id, review_run_id).await?;
        let proposal_id = complete_task(
            &mut tx,
            outbox,
            request.project_id,
            task_id,
            review_run_id,
            delivery_run_id,
            "approved",
            Vec::new(),
            request.decision_reason.clone(),
        )
        .await
        .map_err(|e| ApprovalError::Transition(e.to_string()))?;
        ApprovalOutcome::Completed {
            task_id,
            proposal_id,
        }
    } else {
        insert_decision_finding(&mut tx, request, task_id, review_run_id, delivery_run_id).await?;
        transition_task(
            &mut tx,
            request.project_id,
            task_id,
            TaskStatus::ChangesRequested,
        )
        .await
        .map_err(|e| ApprovalError::Transition(e.to_string()))?;
        end_waiting(&mut tx, task_id, "approval rejected").await?;
        ApprovalOutcome::ChangesRequested { task_id }
    };

    audit_review(
        &mut tx,
        request.project_id,
        "review.approval_decided",
        review_run_id,
        serde_json::json!({
            "approval_id": request.approval_id,
            "decision": request.decision,
            "decided_by": request.decided_by,
            "reason": request.decision_reason,
        }),
    )
    .await
    .map_err(|e| ApprovalError::GateRefused(e.to_string()))?;

    tx.commit().await?;
    Ok(outcome)
}

/// 人工裁决的终值行：对尚无裁决行的 `manual` / `review` 条目 INSERT `passed`。
async fn insert_manual_verdicts(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    request: &ReviewDecisionRequest,
    task_id: Uuid,
    review_run_id: Uuid,
) -> Result<(), sqlx::Error> {
    let criteria: Vec<Uuid> = sqlx::query_scalar(
        "SELECT c.id FROM delivery.task_acceptance_criterion c \
          WHERE c.task_id = $1 AND c.verification_kind IN ('manual', 'review') \
            AND NOT EXISTS (SELECT 1 FROM delivery.acceptance_criterion_verdict v \
                             WHERE v.criterion_id = c.id AND v.review_run_id = $2)",
    )
    .bind(task_id)
    .bind(review_run_id)
    .fetch_all(&mut **tx)
    .await?;
    for criterion_id in criteria {
        sqlx::query(
            "INSERT INTO delivery.acceptance_criterion_verdict \
             (project_id, criterion_id, review_run_id, verdict, evidence, decided_by, decided_at, tenant_id) \
             VALUES ($1, $2, $3, 'passed', $4, $5, now(), \
                     (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(request.project_id)
        .bind(criterion_id)
        .bind(review_run_id)
        .bind(serde_json::json!({
            "decided_by_human": request.decided_by,
            "reason": request.decision_reason,
        }))
        .bind(request.decided_by)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// rejected 的 `decision_reason` 转 finding（结构化失败项）。
async fn insert_decision_finding(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    request: &ReviewDecisionRequest,
    task_id: Uuid,
    review_run_id: Uuid,
    delivery_run_id: Uuid,
) -> Result<(), sqlx::Error> {
    let message = request
        .decision_reason
        .clone()
        .unwrap_or_else(|| "approval rejected without a reason".to_string());
    sqlx::query(
        "INSERT INTO delivery.review_finding \
         (project_id, review_run_id, finding_kind, severity, message, tenant_id) \
         VALUES ($1, $2, 'approval_rejected', 'blocker', $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(request.project_id)
    .bind(review_run_id)
    .bind(&message)
    .execute(&mut **tx)
    .await?;
    let _ = (task_id, delivery_run_id);
    Ok(())
}

/// 受守卫的状态迁移（统一经 D1 通道）。
pub(crate) async fn transition_task(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    to: TaskStatus,
) -> Result<(), ReviewError> {
    let ctx = system_context(tx, project_id)
        .await
        .map_err(|e| ReviewError::Transition(e.to_string()))?;
    apply_task_transition(
        tx,
        task_id,
        None,
        to,
        &TransitionFacts::for_dispatch(),
        &ctx,
    )
    .await
    .map_err(|e| ReviewError::Transition(e.to_string()))?;
    Ok(())
}

/// 结行 `task_waiting` 活跃行（`ended_at` / `ended_by` / `resolution_note`）。
pub(crate) async fn end_waiting(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    task_id: Uuid,
    note: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE delivery.task_waiting \
            SET ended_at = now(), ended_by = $2, resolution_note = $3 \
          WHERE task_id = $1 AND ended_at IS NULL",
    )
    .bind(task_id)
    .bind(crate::run_dispatch::DISPATCH_SERVICE_PRINCIPAL)
    .bind(note)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 同事务写 outbox 事件（未启用的事件类型不产生——沿用 §20.6 开关惯例）。
async fn emit_event(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    outbox: &dyn OutboxAppend,
    project_id: Uuid,
    event_type: &str,
    review_run_id: Uuid,
    idempotency_key: String,
    payload: serde_json::Value,
) -> Result<(), ReviewError> {
    if !crate::outbox::event_is_enabled(event_type) {
        return Ok(());
    }
    let aggregate_id = review_run_id.to_string();
    outbox
        .append(
            tx,
            OutboxEvent {
                project_id,
                event_type,
                aggregate_type: "TaskRun",
                aggregate_id: &aggregate_id,
                payload,
                idempotency_key: &idempotency_key,
            },
        )
        .await?;
    Ok(())
}
