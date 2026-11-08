//! Project 归档（S01 · I1-D，B-1 默认并入 P1~P4，2026-11-04）。
//!
//! 归档事务（单事务）：
//!
//! 1. 锁 `core.project` 行 → 已归档则 409（幂等语义归 REST 层裁决为冲突）；
//! 2. `status → archived`；
//! 3. **P1**：queued Run **条件取消**（`WHERE status='queued'`——只动还没跑的，
//!    claimed/running 不在事务内动）；
//! 4. **N-2**：终态化在飞 Import（active 四态 → cancelled）+ 释放初始化名额 +
//!    取消在飞 Attempt；
//! 5. 审计 `project.archived`（after_data 带各计数）。
//!
//! 事务外（P2）：
//!
//! 6. claimed/running 的活跃 Run 逐条走 `cancel_run(reason="project_archived")`
//!    ——落终态 + 撤凭据 + 终止进程（复用既有取消协议，历史不覆盖）。
//!
//! **P4** 事实：`initializing` 卡死已由 `fail_import` 释放名额修复（差异 G-12）；
//! 归档路径同样释放名额，双保险。

use uuid::Uuid;

use crate::actor::AuditActor;
use crate::run_executor;

/// 归档失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArchiveError {
    #[error("project is already archived")]
    AlreadyArchived,
    #[error("project not found")]
    NotFound,
    #[error("database error: {0}")]
    Db(String),
}

/// 归档结果（供审计与 UI 汇报）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ArchiveOutcome {
    pub project_id: Uuid,
    /// 条件取消的 queued Run 数（P1）。
    pub queued_runs_cancelled: i64,
    /// 终态化的在飞 Import 数（N-2）。
    pub imports_finalized: i64,
    /// 归档时处于 claimed/running、由取消协议处理的活跃 Run 数（P2）。
    pub active_runs_cancelled: usize,
    /// 调用前是否已归档（幂等：REST 层将 409）。
    pub already_archived: bool,
}

/// 归档项目（I1-D 主事务 + P2 进程取消）。
pub async fn archive_project(
    pool: &sqlx::PgPool,
    executor: &dyn crate::run_dispatch::RunExecutor,
    actor: &AuditActor,
    tenant_id: Uuid,
    project_id: Uuid,
    reason: &str,
) -> Result<ArchiveOutcome, ArchiveError> {
    let mut tx = pool.begin().await.map_err(db_err)?;

    // 1) 锁项目行。
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM core.project WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_err)?;
    let Some(status) = status else {
        return Err(ArchiveError::NotFound);
    };
    if status == "archived" {
        return Err(ArchiveError::AlreadyArchived);
    }

    // 2) 状态 → archived。
    sqlx::query("UPDATE core.project SET status = 'archived', updated_at = now() WHERE id = $1")
        .bind(project_id)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;

    // 3) P1：queued Run 条件取消（只动 queued——claimed/running 交 P2 协议）。
    let queued_runs_cancelled = sqlx::query(
        "UPDATE delivery.task_run SET status = 'cancelled', termination_reason = $2, \
         terminated_at = now() \
         WHERE project_id = $1 AND status = 'queued'",
    )
    .bind(project_id)
    .bind(reason)
    .execute(&mut *tx)
    .await
    .map_err(db_err)?
    .rows_affected() as i64;

    // 4) N-2：终态化在飞 Import + 释放初始化名额 + 取消在飞 Attempt。
    let imports_finalized = sqlx::query(
        "UPDATE integration.repo_import SET status = 'cancelled', updated_at = now() \
         WHERE project_id = $1 AND status IN ('queued','cloning','reading','analyzing','draft_ready')",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await
    .map_err(db_err)?
    .rows_affected() as i64;
    sqlx::query(
        "UPDATE core.project_initialization_operation \
         SET status = 'released', released_at = now() \
         WHERE project_id = $1 AND status = 'active'",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await
    .map_err(db_err)?;
    sqlx::query(
        "UPDATE integration.repo_import_source_attempt a \
         SET status = 'cancelled', finished_at = now() \
         FROM integration.repo_import i \
         WHERE i.id = a.import_id AND i.project_id = $1 AND a.status = 'running'",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await
    .map_err(db_err)?;

    // 5) 审计（同事务）。
    crate::pm_write::append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "project.archived",
        "project",
        Some(project_id),
        serde_json::json!({
            "reason": reason,
            "queued_runs_cancelled": queued_runs_cancelled,
            "imports_finalized": imports_finalized,
        }),
    )
    .await
    .map_err(db_err)?;

    tx.commit().await.map_err(db_err)?;

    // 6) P2：活跃 Run 走取消协议（落终态 + 撤凭据 + 终止进程；历史不覆盖）。
    let active: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM delivery.task_run \
         WHERE project_id = $1 AND status IN ('claimed','running')",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(db_err)?;
    let mut active_runs_cancelled = 0usize;
    for run_id in active {
        // P2 协议 reason 固定为 project_archived（record_termination 映射
        // cancelled）；人工 reason 已在 project.archived 审计 after_data。
        if run_executor::cancel_run(pool, executor, run_id, "project_archived")
            .await
            .unwrap_or(false)
        {
            active_runs_cancelled += 1;
        }
    }

    Ok(ArchiveOutcome {
        project_id,
        queued_runs_cancelled,
        imports_finalized,
        active_runs_cancelled,
        already_archived: false,
    })
}

fn db_err(err: sqlx::Error) -> ArchiveError {
    ArchiveError::Db(err.to_string())
}
