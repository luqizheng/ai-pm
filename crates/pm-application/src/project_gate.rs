//! Project 活动门禁（S01 · I1-A，`docs/ui/smark-pm-s01-i1-…v1.md`）。
//!
//! I1 核验结论：`core.project.status` 的消费者**为零**——Run 创建、claim、
//! Outbox 消费三类入口都不看项目状态，归档因此挡不住新工作。本模块提供
//! **统一门禁**：所有会为新工作打开入口的路径先过这里。
//!
//! 锁序约束：本模块的读**不加锁**（防御纵深读）——归档事务（`archive.rs`）
//! 持有 project 行锁并以条件 UPDATE 收口 queued Run，claim 侧的
//! `FOR UPDATE` 会与该 UPDATE 串行化，读到的必然是归档后的已提交状态；
//! 这与 I1 的锁序 `project → run → task` 同向，不成环。

use sqlx::PgPool;
use uuid::Uuid;

/// 门禁判定结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectActivity {
    /// 项目活跃（planning / initializing / 任意非 archived 状态）——放行。
    Active,
    /// 项目已归档——入口关闭。
    Archived,
}

/// 门禁失败（归档项目不允许新工作）。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GateError {
    #[error("project is archived")]
    Archived,
    #[error("project not found")]
    NotFound,
    #[error("database error: {0}")]
    Db(String),
}

impl ProjectActivity {
    /// 门禁出口：Archived → Err。
    pub fn ensure(self) -> Result<(), GateError> {
        match self {
            Self::Active => Ok(()),
            Self::Archived => Err(GateError::Archived),
        }
    }
}

/// 读项目活动状态（连接池版；判定层防御纵深读）。
pub async fn project_activity(pool: &PgPool, project_id: Uuid) -> Result<ProjectActivity, GateError> {
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| GateError::Db(e.to_string()))?;
    Ok(match status.as_deref() {
        None => return Err(GateError::NotFound),
        Some("archived") => ProjectActivity::Archived,
        Some(_) => ProjectActivity::Active,
    })
}

/// 事务版：在同一事务内读项目状态（供 Outbox handler / enqueue_review 复用）。
pub async fn project_activity_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
) -> Result<ProjectActivity, GateError> {
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| GateError::Db(e.to_string()))?;
    Ok(match status.as_deref() {
        None => return Err(GateError::NotFound),
        Some("archived") => ProjectActivity::Archived,
        Some(_) => ProjectActivity::Active,
    })
}
