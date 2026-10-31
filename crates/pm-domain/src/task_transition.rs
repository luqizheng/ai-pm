//! Task transition rules (`ai-pm-task-state-machine-v1.md` §1.3 / §十一 / §十二 / §十九).
//!
//! All state changes go through this rule set (§7.4 硬约束的域层载体)：
//! the edge matrix is the §1.3 规范版 plus the §11.2 Superseded `allowed_from`
//! set; guards (supersession, waiting, evidence) live with the matrix.

use crate::task::TaskStatus;

/// Why a transition was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransitionError {
    /// §12.3 — entering `AwaitingDecision` requires at least one open question.
    #[error("waiting context requires at least one open question")]
    EmptyQuestions,
    /// Edge not in the ruled matrix (§1.3 + §11.2), including terminal→any.
    #[error("illegal transition: {from:?} -> {to:?}")]
    IllegalTransition { from: TaskStatus, to: TaskStatus },
    /// Edge is legal but its guard evidence is missing or negative.
    #[error("missing evidence: {0}")]
    MissingEvidence(&'static str),
    /// §11.2/§11.5 supersession constraints (self-supersession, missing intent…).
    #[error("supersession violation: {0}")]
    SupersessionViolation(String),
}
