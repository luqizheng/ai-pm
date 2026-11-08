//! 失败判定服务（D5 §6，B4-A~D）。
//!
//! **结构性隔离**（§6.2 P0-4）：候选扫描 SQL 强制谓词 `run_kind = 'delivery'`。
//! review run 的全部失败处置（重试 / 超时回收 / 耗尽降级）由 review 领取拍自理，
//! 本作业对 review run **无任何写入路径**——这样「审查引擎故障」不可能触发交付
//! 被重做（§5.4 不变量 1），也不会污染 `consecutive_failure_count`（规则 R6/R7）。
//!
//! 判定表（§6.1）：
//!
//! | 输入 | 归类 | 输出 |
//! |---|---|---|
//! | delivery run `failed` / `timed_out`，未耗尽 | B4-A/B | 新建 delivery run（`attempt_no+1`），任务保持 `running` |
//! | 尝试次数 / 连续失败达阈值 | B4-C | `running→failed` + `FailureRecord` + 审计 |
//! | 不可恢复类（permission / missing_dependency / policy_violation） | B4-D | 立即 `running→failed` |
//! | `needs_recovery` 任务 | 排除 | 不触碰（恢复控制状态等人工） |

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use pm_domain::TaskStatus;

use crate::review::decide::transition_task;
use crate::review::queue::{audit_review, ReviewError};
use crate::run_dispatch::{insert_task_run, run_kind, run_status, RunDispatchConfig};

/// 一次失败判定步进的结果。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FailureJudgeReport {
    /// 判定的 delivery run 数（`failure_judged_count`）。
    pub judged: usize,
    /// 新建的重试 run 数（`retry_created_count`）。
    pub retries: usize,
    /// 落到任务 `failed` 的条数。
    pub task_failures: usize,
}

/// 不可恢复的失败类别（B4-D：立即失败，不为计数反复尝试）。
fn is_terminating(category: &str) -> bool {
    matches!(
        category,
        "permission" | "missing_dependency" | "policy_violation"
    )
}

/// 扫描一轮需要判定的 delivery run，逐条判定。
///
/// 幂等：`task_run_failure` 行存在即「已判定过」的证据（D3 先例），条件 UPDATE
/// 保证双 worker 并发扫描天然安全。
pub async fn judge_delivery_failures(
    pool: &PgPool,
    cfg: &RunDispatchConfig,
) -> Result<FailureJudgeReport, ReviewError> {
    // ── 候选扫描（结构上排除 review run / needs_recovery 任务）────────────
    let rows = sqlx::query(
        "SELECT r.id, r.project_id, r.task_id, r.attempt_no, r.status, \
                f.category AS failure_category, f.retryable AS failure_retryable, \
                f.reason_code AS failure_reason \
           FROM delivery.task_run r \
           LEFT JOIN delivery.task_run_failure f ON f.run_id = r.id \
           JOIN delivery.task t ON t.id = r.task_id \
          WHERE r.run_kind = 'delivery' \
            AND r.status IN ('failed', 'timed_out') \
            AND t.status = 'running' \
            AND NOT (t.id IN ( \
                SELECT task_id FROM delivery.task_waiting WHERE ended_at IS NULL \
                  AND reason <> 'needs_approval')) \
            AND r.finished_at < now() - make_interval(secs => 5) \
          ORDER BY r.finished_at LIMIT 16",
    )
    .fetch_all(pool)
    .await?;

    let mut report = FailureJudgeReport::default();
    for row in rows {
        let run_id: Uuid = row.get("id");
        let project_id: Uuid = row.get("project_id");
        let task_id: Uuid = row.get("task_id");
        let attempt_no: i32 = row.get("attempt_no");
        let status: String = row.get("status");
        let category: Option<String> = row.get("failure_category");
        let reason: Option<String> = row.get("failure_reason");

        // 条件 UPDATE 标记「本轮已判定」（先到先得）：把 finished_at 往后推，
        // 避免同一 run 被反复扫描；真正的幂等证据是 task_run_failure 行。
        let claimed = sqlx::query(
            "UPDATE delivery.task_run SET finished_at = now() \
              WHERE id = $1 AND status IN ('failed', 'timed_out') \
                AND finished_at < now() - make_interval(secs => 5)",
        )
        .bind(run_id)
        .execute(pool)
        .await?
        .rows_affected();
        if claimed != 1 {
            continue;
        }

        report.judged += 1;
        let mut tx = pool.begin().await?;

        // 已有失败证据行 → 幂等跳过（该 run 已被判定过）。
        let already: i64 =
            sqlx::query_scalar("SELECT count(*) FROM delivery.task_run_failure WHERE run_id = $1")
                .bind(run_id)
                .fetch_one(&mut *tx)
                .await?;
        if already > 0 {
            tx.commit().await?;
            continue;
        }

        // 记录失败证据（不可变表，append-only）。
        let failure_category = category.unwrap_or_else(|| {
            if status == run_status::TIMED_OUT {
                "infrastructure".to_string()
            } else {
                "execution".to_string()
            }
        });
        let retryable = row
            .get::<Option<bool>, _>("failure_retryable")
            .unwrap_or(true)
            && !is_terminating(&failure_category);
        sqlx::query(
            "INSERT INTO delivery.task_run_failure \
             (run_id, project_id, category, retryable, reason_code, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, \
                     (SELECT tenant_id FROM core.project WHERE id = $2))",
        )
        .bind(run_id)
        .bind(project_id)
        .bind(&failure_category)
        .bind(retryable)
        .bind(reason.unwrap_or_else(|| status.clone()))
        .execute(&mut *tx)
        .await?;

        // B4-D：不可恢复类立即失败，不走重试路径。
        if !retryable {
            sqlx::query(
                "UPDATE delivery.task \
                    SET consecutive_failure_count = consecutive_failure_count + 1, \
                        last_failure_category = $2, retry_exhausted = true \
                  WHERE id = $1",
            )
            .bind(task_id)
            .bind(&failure_category)
            .execute(&mut *tx)
            .await?;
            transition_task_failed(&mut tx, project_id, task_id, &failure_category).await?;
            audit_review(
                &mut tx,
                project_id,
                "task.failed",
                run_id,
                serde_json::json!({
                    "task_id": task_id,
                    "category": failure_category,
                    "reason": "unrecoverable_failure",
                }),
            )
            .await?;
            tx.commit().await?;
            report.task_failures += 1;
            continue;
        }

        // B4-C：尝试次数 / 连续失败达阈值 → 任务 failed。
        sqlx::query(
            "UPDATE delivery.task \
                SET consecutive_failure_count = consecutive_failure_count + 1, \
                    last_failure_category = $2 \
              WHERE id = $1",
        )
        .bind(task_id)
        .bind(&failure_category)
        .execute(&mut *tx)
        .await?;
        let counters: (i32, i32) = sqlx::query_as(
            "SELECT attempt_count, consecutive_failure_count FROM delivery.task WHERE id = $1",
        )
        .bind(task_id)
        .fetch_one(&mut *tx)
        .await?;
        let exhausted = counters.0 >= cfg.max_run_attempts || counters.1 >= cfg.max_run_attempts;
        if exhausted {
            sqlx::query("UPDATE delivery.task SET retry_exhausted = true WHERE id = $1")
                .bind(task_id)
                .execute(&mut *tx)
                .await?;
            transition_task_failed(&mut tx, project_id, task_id, &failure_category).await?;
            audit_review(
                &mut tx,
                project_id,
                "task.failed",
                run_id,
                serde_json::json!({
                    "task_id": task_id,
                    "category": failure_category,
                    "attempt_count": counters.0,
                    "consecutive_failure_count": counters.1,
                }),
            )
            .await?;
            tx.commit().await?;
            report.task_failures += 1;
            continue;
        }

        // B4-A/B：可重试 → 新建 delivery run（`attempt_no+1`）。任务保持 running。
        // 调用者须持有 task 行锁（B2），本事务已在上面 UPDATE 过该行。
        let created = insert_task_run(&mut tx, project_id, task_id, None).await?;
        sqlx::query("UPDATE delivery.task_run SET supersedes_run_id = $2 WHERE id = $1")
            .bind(created.run_id)
            .bind(run_id)
            .execute(&mut *tx)
            .await?;
        audit_review(
            &mut tx,
            project_id,
            "task.retry_created",
            run_id,
            serde_json::json!({
                "task_id": task_id,
                "new_run_id": created.run_id,
                "attempt_no": created.attempt_no,
                "attempt_no_of_failed_run": attempt_no,
            }),
        )
        .await?;
        tx.commit().await?;
        report.retries += 1;
    }
    Ok(report)
}

/// `running→failed`（B4-C/D），经 D1 受守卫通道。
async fn transition_task_failed(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    category: &str,
) -> Result<(), ReviewError> {
    // B4-C（重试耗尽）与 B4-D（不可恢复类）都已在库里置好事实：`retry_exhausted`
    // 与 `last_failure_category`。域层守卫据此放行 Running→Failed。
    let _ = category;
    transition_task(tx, project_id, task_id, TaskStatus::Failed).await
}

/// 失败判定作业的循环体（`ai-pm-worker` 用）。
pub async fn judge_failures_once(pool: &PgPool) -> FailureJudgeReport {
    let cfg = RunDispatchConfig::from_env();
    match judge_delivery_failures(pool, &cfg).await {
        Ok(report) => report,
        Err(error) => {
            tracing::warn!(error = %error, "failure judge failed");
            FailureJudgeReport::default()
        }
    }
}

/// 扫描谓词的单一真源导出（测试断言用：必须含 delivery 谓词）。
pub const FAILURE_SCAN_KIND_PREDICATE: &str = run_kind::DELIVERY;
