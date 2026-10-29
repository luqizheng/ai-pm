//! Errors surfaced by command execution.

use crate::ids::AuditId;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("revision conflict: expected revision {expected}, actual {actual}")]
    RevisionConflict { expected: i64, actual: i64 },
    #[error("red line blocks command: {command}")]
    RedLineBlocked { command: &'static str },
    #[error("command not handled: {command}")]
    NotHandled { command: String },
    #[error("command {audit_id} already compensated")]
    AlreadyCompensated { audit_id: AuditId },
    #[error("approval request {approval_id} already decided")]
    AlreadyDecided { approval_id: uuid::Uuid },
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("database error: {0}")]
    Db(sqlx::Error),
}

#[cfg(test)]
mod tests {
    // Per brief Step 1: no tests mandated for CommandError in this task.
}
