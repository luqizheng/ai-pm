//! Project-scoped read services (Task 8): read-only queries that the inbound
//! event pipeline (Task 9) and the MCP tools (Task 16) consume as context.
//!
//! Every query is project-scoped: the target row is only visible when its
//! `project_id` matches the caller's project — anything else, including a task
//! that exists in another project, reads as [`ReadError::NotFound`].

use pm_domain::{ProjectId, TaskId};
use serde_json::Value;
use sqlx::{PgPool, Row};

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("requested row not found (or not visible in this project)")]
    NotFound,
    #[error("database error: {0}")]
    Db(sqlx::Error),
}

/// The context snapshot for one task: its own fields, the content of the latest
/// linked requirement revision (through `delivery.task_requirement`), and the
/// milestone row as JSONB — either may be absent.
#[derive(Debug, Clone)]
pub struct TaskContextRow {
    pub task_id: TaskId,
    pub title: String,
    pub status: String,
    pub revision: i64,
    pub requirement: Option<Value>,
    pub milestone: Option<Value>,
}

/// Reads one task's context within `project_id`. A task of another project is
/// invisible: the `project_id` predicate turns it into [`ReadError::NotFound`],
/// the same error a missing task produces.
pub async fn task_context(
    pool: &PgPool,
    project_id: ProjectId,
    task_id: TaskId,
) -> Result<TaskContextRow, ReadError> {
    let row = sqlx::query(
        "SELECT t.id AS task_id, t.title, t.status, t.revision, \
                req.content AS requirement, \
                CASE WHEN m.id IS NULL THEN NULL::jsonb ELSE to_jsonb(m) END AS milestone \
         FROM delivery.task t \
         LEFT JOIN LATERAL ( \
             SELECT rr.content FROM delivery.task_requirement tr \
             JOIN delivery.requirement_revision rr ON rr.id = tr.revision_id \
             WHERE tr.task_id = t.id \
             ORDER BY rr.created_at DESC, rr.version DESC LIMIT 1 \
         ) req ON TRUE \
         LEFT JOIN delivery.milestone m ON m.id = t.milestone_id \
         WHERE t.id = $1 AND t.project_id = $2",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(ReadError::Db)?;
    match row {
        Some(row) => Ok(TaskContextRow {
            task_id: row.get("task_id"),
            title: row.get("title"),
            status: row.get("status"),
            revision: row.get("revision"),
            requirement: row.get("requirement"),
            milestone: row.get("milestone"),
        }),
        None => Err(ReadError::NotFound),
    }
}
