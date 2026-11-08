//! 拍②：worker review 领取循环（D5 §3.1）。
//!
//! 三步，各自事务：
//!
//! 1. **超时回收前置**：`running` 且超 review 预算的 review run → 条件 UPDATE
//!    置 `timed_out`（先到先得）→ 走重试（§3.4 场景 3）。
//! 2. **领取**：只扫 `status='queued'` 的 review run → `running`（条件 UPDATE
//!    先到先得，多个 worker 不会重复领取，§3.4 场景 4/5）。
//! 3. **执行**：采集只读输入 → 锚定漂移复核 → 确定性管线 → findings + 逐条裁决
//!    + verdict 落库 → run `succeeded` → 按 §5.1 矩阵推进任务（同事务）。
//!
//! 失败语义（§5.4 不变量 1）：review run 的 `failed`/`timed_out` **不触碰**
//! delivery run 的 status/outcome，也不计入 `consecutive_failure_count`；恢复链
//! 新建的 run `run_kind` 恒为 `'review'`，**绝不创建 delivery run**。

use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, Transaction};
use uuid::Uuid;

use crate::outbox::OutboxAppend;
use crate::review::collect::{collect_review_input, detect_anchor_drift};
use crate::review::decide::{advance_after_review, open_review_degraded, AdvanceOutcome};
use crate::review::pipeline::evaluate;
use crate::review::queue::{audit_review, ReviewError};

/// review run 的默认预算（秒）：确定性校验是纯 CPU/IO 轻操作，远超即视为卡死。
pub const DEFAULT_REVIEW_BUDGET_SECS: i64 = 900;

/// review 编排的可调项。
#[derive(Debug, Clone, Copy)]
pub struct ReviewConfig {
    /// 单条 review run 的预算（`running` 超时即回收）。
    pub budget_secs: i64,
    /// 重试上限（含首次），与 D1 `max_run_attempts` 同义。
    pub max_attempts: i32,
}

impl ReviewConfig {
    pub fn from_env() -> Self {
        fn num(key: &str, default: i64) -> i64 {
            std::env::var(key)
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(default)
        }
        let cfg = crate::run_dispatch::RunDispatchConfig::from_env();
        Self {
            budget_secs: num("AI_PM_REVIEW_BUDGET_SECS", DEFAULT_REVIEW_BUDGET_SECS),
            max_attempts: cfg.max_run_attempts,
        }
    }
}

/// 一次 `review_step` 的结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ReviewStep {
    /// 空闲（无 queued / 无超时 run）。
    Idle,
    /// 回收了卡死 run。
    Reclaimed { count: usize },
    /// 完成了一轮审查并推进了任务。
    Reviewed {
        run_id: Uuid,
        verdict: String,
        advance: AdvanceOutcome,
    },
    /// 审查运行失败（run 落 `failed`）并已排下一轮重试。
    Failed {
        run_id: Uuid,
        retry_run_id: Option<Uuid>,
    },
}

/// worker 循环的一次完整步进：先回收，再领取执行，最后重试失败的。
pub async fn review_step(
    pool: &PgPool,
    outbox: &dyn OutboxAppend,
    cfg: &ReviewConfig,
) -> Result<ReviewStep, ReviewError> {
    let reclaimed = recover_stuck_reviews(pool, outbox, cfg).await?;
    if reclaimed > 0 {
        return Ok(ReviewStep::Reclaimed { count: reclaimed });
    }
    let Some(run_id) = claim_review_run(pool).await? else {
        return Ok(ReviewStep::Idle);
    };
    run_review(pool, outbox, run_id, cfg).await
}

/// 回收卡死的 review run：`running` 且 `running_since` 超预算 → `timed_out`。
async fn recover_stuck_reviews(
    pool: &PgPool,
    outbox: &dyn OutboxAppend,
    cfg: &ReviewConfig,
) -> Result<usize, ReviewError> {
    let stuck: Vec<(Uuid, Uuid, Uuid, i32)> = sqlx::query_as(
        "SELECT id, project_id, task_id, attempt_no FROM delivery.task_run \
          WHERE run_kind = 'review' AND status = 'running' \
            AND coalesce(started_at, created_at) < now() - make_interval(secs => $1) \
          ORDER BY created_at LIMIT 8",
    )
    .bind(cfg.budget_secs as f64)
    .fetch_all(pool)
    .await?;
    if stuck.is_empty() {
        return Ok(0);
    }
    let mut reclaimed = 0usize;
    for (run_id, project_id, task_id, attempt_no) in stuck {
        let updated = sqlx::query(
            "UPDATE delivery.task_run \
                SET status = 'timed_out', outcome = 'timed_out', finished_at = now(), \
                    termination_reason = 'review_budget_exceeded' \
              WHERE id = $1 AND status = 'running'",
        )
        .bind(run_id)
        .execute(pool)
        .await?
        .rows_affected();
        if updated != 1 {
            continue;
        }
        reclaimed += 1;
        let retry =
            schedule_retry(pool, outbox, project_id, task_id, run_id, attempt_no, cfg).await?;
        tracing::warn!(run = %run_id, retry = ?retry, "review run exceeded its budget");
    }
    Ok(reclaimed)
}

/// 领取一条 queued review run → `running`（条件 UPDATE 先到先得）。
async fn claim_review_run(pool: &PgPool) -> Result<Option<Uuid>, ReviewError> {
    let rows = sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'running', started_at = now(), stage = NULL \
          WHERE id = (SELECT id FROM delivery.task_run \
                       WHERE run_kind = 'review' AND status = 'queued' \
                       ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED) \
          RETURNING id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.first().map(|row| row.get::<Uuid, _>("id")))
}

/// 执行一条已领取的 review run：采集 → 漂移复核 → 管线 → 落库 → 推进。
async fn run_review(
    pool: &PgPool,
    outbox: &dyn OutboxAppend,
    review_run_id: Uuid,
    cfg: &ReviewConfig,
) -> Result<ReviewStep, ReviewError> {
    let collected = match collect_review_input(pool, review_run_id).await {
        Ok(collected) => collected,
        Err(error) => {
            return fail_review(pool, outbox, review_run_id, cfg, &error.to_string()).await
        }
    };

    // 锚定漂移防御（§3.3）：verdict 落库前回读重算，漂移即本轮不落。
    {
        let mut tx = pool.begin().await?;
        match detect_anchor_drift(&mut tx, &collected, review_run_id).await? {
            Some(reason) => {
                tx.rollback().await.ok();
                return fail_review(
                    pool,
                    outbox,
                    review_run_id,
                    cfg,
                    &format!("anchor_drift: {reason}"),
                )
                .await;
            }
            None => tx.commit().await?,
        }
    }

    let outcome = evaluate(&collected.input);

    let mut tx = pool.begin().await?;
    // 终态条件校验：run 仍是本 worker 领取的 `running`（防重复执行覆盖）。
    let still_running: bool =
        sqlx::query_scalar("SELECT (status = 'running') FROM delivery.task_run WHERE id = $1")
            .bind(review_run_id)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(false);
    if !still_running {
        tx.rollback().await.ok();
        return Ok(ReviewStep::Idle);
    }

    persist_findings(&mut tx, &collected, review_run_id, &outcome).await?;
    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'succeeded', outcome = 'succeeded', finished_at = now(), \
                review_verdict = $2 \
          WHERE id = $1 AND status = 'running'",
    )
    .bind(review_run_id)
    .bind(outcome.verdict.as_str())
    .execute(&mut *tx)
    .await?;

    let advance = advance_after_review(
        &mut tx,
        outbox,
        collected.project_id,
        review_run_id,
        collected.delivery_run_id,
        &outcome,
        None,
    )
    .await?;
    tx.commit().await?;

    Ok(ReviewStep::Reviewed {
        run_id: review_run_id,
        verdict: outcome.verdict.as_str().to_string(),
        advance,
    })
}

/// 落 findings + 逐条确定性裁决（`manual` / `review` 条目**不落裁决行**）。
async fn persist_findings(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    collected: &crate::review::collect::CollectedReview,
    review_run_id: Uuid,
    outcome: &crate::review::pipeline::ReviewOutcome,
) -> Result<(), ReviewError> {
    for finding in &outcome.findings {
        sqlx::query(
            "INSERT INTO delivery.review_finding \
             (project_id, review_run_id, criterion_id, finding_kind, severity, \
              code_location, message, evidence_ref, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, \
                     (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(collected.project_id)
        .bind(review_run_id)
        .bind(finding.criterion_id)
        .bind(&finding.finding_kind)
        .bind(finding.severity.as_str())
        .bind(finding.code_location.as_deref())
        .bind(&finding.message)
        .bind(finding.evidence_ref.as_deref())
        .execute(&mut **tx)
        .await?;
    }
    for criterion in &outcome.criteria {
        let Some(verdict) = criterion.verdict else {
            continue;
        };
        sqlx::query(
            "INSERT INTO delivery.acceptance_criterion_verdict \
             (project_id, criterion_id, review_run_id, verdict, evidence, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, \
                     (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(collected.project_id)
        .bind(criterion.criterion_id)
        .bind(review_run_id)
        .bind(verdict.as_str())
        .bind(&criterion.evidence)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// review run 失败：落 `failed` + `task_run_failure` 证据 → 排重试 / 降级人工。
async fn fail_review(
    pool: &PgPool,
    outbox: &dyn OutboxAppend,
    review_run_id: Uuid,
    cfg: &ReviewConfig,
    reason: &str,
) -> Result<ReviewStep, ReviewError> {
    let row =
        sqlx::query("SELECT project_id, task_id, attempt_no FROM delivery.task_run WHERE id = $1")
            .bind(review_run_id)
            .fetch_one(pool)
            .await?;
    let project_id: Uuid = row.get("project_id");
    let task_id: Uuid = row.get("task_id");
    let attempt_no: i32 = row.get("attempt_no");

    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'failed', outcome = 'failed', finished_at = now(), \
                release_reason = $2 \
          WHERE id = $1 AND status = 'running'",
    )
    .bind(review_run_id)
    .bind(reason.chars().take(200).collect::<String>())
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 1 {
        // 失败证据不可变（append-only）；category='validation'（审查能力失效）。
        sqlx::query(
            "INSERT INTO delivery.task_run_failure \
             (run_id, project_id, category, retryable, reason_code, tenant_id) \
             VALUES ($1, $2, 'validation', true, $3, \
                     (SELECT tenant_id FROM core.project WHERE id = $2)) \
             ON CONFLICT (run_id) DO NOTHING",
        )
        .bind(review_run_id)
        .bind(project_id)
        .bind(reason.chars().take(120).collect::<String>())
        .execute(&mut *tx)
        .await?;
        audit_review(
            &mut tx,
            project_id,
            "review.failed",
            review_run_id,
            serde_json::json!({ "reason": reason.chars().take(500).collect::<String>() }),
        )
        .await?;
    }
    tx.commit().await?;

    let retry = schedule_retry(
        pool,
        outbox,
        project_id,
        task_id,
        review_run_id,
        attempt_no,
        cfg,
    )
    .await?;
    Ok(ReviewStep::Failed {
        run_id: review_run_id,
        retry_run_id: retry,
    })
}

/// 重试判定：未耗尽 → 新建 `run_kind='review'` 的 run（`attempt_no+1`）；
/// 耗尽 → 降级人工（`review_approval(kind='review_degraded')`），任务停 `reviewing`。
///
/// **绝不创建 delivery run**（§5.4 不变量 4）。
async fn schedule_retry(
    pool: &PgPool,
    outbox: &dyn OutboxAppend,
    project_id: Uuid,
    task_id: Uuid,
    failed_run_id: Uuid,
    attempt_no: i32,
    cfg: &ReviewConfig,
) -> Result<Option<Uuid>, ReviewError> {
    if attempt_no < cfg.max_attempts {
        let mut tx = pool.begin().await?;
        let anchors: Option<(Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT input_manifest_hash, reviewed_manifest_hash, review_policy_version \
               FROM delivery.task_run WHERE id = $1",
        )
        .bind(failed_run_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((input, reviewed, policy)) = anchors else {
            tx.commit().await?;
            return Ok(None);
        };
        let next_attempt: i32 = sqlx::query_scalar(
            "SELECT coalesce(max(attempt_no), 0) + 1 FROM delivery.task_run \
                                 WHERE task_id = $1 AND run_kind = 'review'",
        )
        .bind(task_id)
        .fetch_one(&mut *tx)
        .await?;
        let new_run: Uuid = sqlx::query_scalar(
            "INSERT INTO delivery.task_run \
             (project_id, task_id, attempt_no, status, run_kind, role_id, executor_kind, \
              input_manifest_hash, reviewed_manifest_hash, review_policy_version, \
              supersedes_run_id, next_dispatch_at, execution_mode, tenant_id) \
             VALUES ($1, $2, $3, 'queued', 'review', $4, 'platform', $5, $6, $7, $8, now(), \
                     'spawn', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
        )
        .bind(project_id)
        .bind(task_id)
        .bind(next_attempt)
        .bind(crate::review::queue::REVIEWER_ROLE)
        .bind(&input)
        .bind(&reviewed)
        .bind(&policy)
        .bind(failed_run_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(Some(new_run));
    }

    // 耗尽：降级人工。任务停 `reviewing`（不退出），不静默跳过强制审查（Q8.3）。
    let delivery_run_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM delivery.task_run \
          WHERE task_id = $1 AND project_id = $2 AND run_kind = 'delivery' \
          ORDER BY attempt_no DESC, created_at DESC LIMIT 1",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let Some(delivery_run_id) = delivery_run_id else {
        return Ok(None);
    };
    let mut tx = pool.begin().await?;
    open_review_degraded(&mut tx, project_id, task_id, failed_run_id, delivery_run_id).await?;
    audit_review(
        &mut tx,
        project_id,
        "review.degraded",
        failed_run_id,
        serde_json::json!({ "attempt_no": attempt_no, "reason": "review_retry_exhausted" }),
    )
    .await?;
    tx.commit().await?;
    let _ = outbox;
    Ok(None)
}

/// worker 循环体的空闲休眠。
pub const REVIEW_IDLE_POLL_MS: u64 = 100;

/// 无限循环版本（`ai-pm-worker` 直接用）。
pub async fn run_review_loop(pool: PgPool, outbox: std::sync::Arc<dyn OutboxAppend>) -> ! {
    let cfg = ReviewConfig::from_env();
    loop {
        match review_step(&pool, outbox.as_ref(), &cfg).await {
            Ok(ReviewStep::Idle) => {
                tokio::time::sleep(Duration::from_millis(REVIEW_IDLE_POLL_MS)).await
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(error = %error, "review step failed; backing off");
                tokio::time::sleep(Duration::from_millis(REVIEW_IDLE_POLL_MS)).await;
            }
        }
    }
}
