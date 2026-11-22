//! PA08-03: shared Run status-transition matrix.
//!
//! This validator is intentionally separate from Task transitions. Callers must
//! still provide transaction/row-lock or compare-and-set protection around writes.
//! Delivery and Reviewer Runs share a table but retain separate lifecycle edges.

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunKind { Delivery, Review }

impl RunKind {
    pub fn parse(value: &str) -> Result<Self, RunTransitionError> {
        match value {
            "delivery" => Ok(Self::Delivery),
            "review" => Ok(Self::Review),
            other => Err(RunTransitionError::UnknownKind(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Queued, Claimed, Running, Succeeded, Failed, TimedOut, Cancelled,
}

impl RunStatus {
    pub fn parse(value: &str) -> Result<Self, RunTransitionError> {
        match value {
            "queued" => Ok(Self::Queued),
            "claimed" => Ok(Self::Claimed),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "timed_out" => Ok(Self::TimedOut),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(RunTransitionError::UnknownStatus(other.to_string())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Claimed => "claimed",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::TimedOut | Self::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RunTransitionError {
    #[error("unknown Run kind: {0}")]
    UnknownKind(String),
    #[error("unknown Run status: {0}")]
    UnknownStatus(String),
    #[error("terminal Run status {0} cannot transition")]
    TerminalImmutable(&'static str),
    #[error("illegal {kind:?} Run transition: {from:?} -> {to:?}")]
    IllegalEdge { kind: RunKind, from: RunStatus, to: RunStatus },
}

/// Validate one actual status change. Same-state lease reacquisition is not a
/// status transition and is validated separately by validate_reacquire.
pub fn validate_transition(
    kind: RunKind,
    from: RunStatus,
    to: RunStatus,
) -> Result<(), RunTransitionError> {
    if from.is_terminal() {
        return Err(RunTransitionError::TerminalImmutable(from.as_str()));
    }

    let allowed = match kind {
        RunKind::Delivery => matches!(
            (from, to),
            (RunStatus::Queued, RunStatus::Claimed)
                | (RunStatus::Queued, RunStatus::Failed)
                | (RunStatus::Queued, RunStatus::Cancelled)
                | (RunStatus::Claimed, RunStatus::Running)
                | (RunStatus::Claimed, RunStatus::Queued)
                | (RunStatus::Claimed, RunStatus::Succeeded)
                | (RunStatus::Claimed, RunStatus::Failed)
                | (RunStatus::Claimed, RunStatus::TimedOut)
                | (RunStatus::Claimed, RunStatus::Cancelled)
                | (RunStatus::Running, RunStatus::Queued)
                | (RunStatus::Running, RunStatus::Succeeded)
                | (RunStatus::Running, RunStatus::Failed)
                | (RunStatus::Running, RunStatus::TimedOut)
                | (RunStatus::Running, RunStatus::Cancelled)
        ),
        RunKind::Review => matches!(
            (from, to),
            (RunStatus::Queued, RunStatus::Running)
                | (RunStatus::Queued, RunStatus::Cancelled)
                | (RunStatus::Running, RunStatus::Succeeded)
                | (RunStatus::Running, RunStatus::Failed)
                | (RunStatus::Running, RunStatus::TimedOut)
        ),
    };

    if allowed {
        Ok(())
    } else {
        Err(RunTransitionError::IllegalEdge { kind, from, to })
    }
}

/// Reacquisition keeps the Run in claimed while replacing the lease token and
/// increasing generation. It is not a transition and is Delivery-only.
pub fn validate_reacquire(kind: RunKind, current: RunStatus) -> Result<(), RunTransitionError> {
    if kind == RunKind::Delivery && current == RunStatus::Claimed {
        return Ok(());
    }
    Err(RunTransitionError::IllegalEdge { kind, from: current, to: current })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_transition_matrix_accepts_only_existing_edges() {
        for (from, to) in [
            (RunStatus::Queued, RunStatus::Claimed),
            (RunStatus::Queued, RunStatus::Failed),
            (RunStatus::Queued, RunStatus::Cancelled),
            (RunStatus::Claimed, RunStatus::Running),
            (RunStatus::Claimed, RunStatus::Queued),
            (RunStatus::Claimed, RunStatus::Succeeded),
            (RunStatus::Claimed, RunStatus::Failed),
            (RunStatus::Claimed, RunStatus::TimedOut),
            (RunStatus::Claimed, RunStatus::Cancelled),
            (RunStatus::Running, RunStatus::Queued),
            (RunStatus::Running, RunStatus::Succeeded),
            (RunStatus::Running, RunStatus::Failed),
            (RunStatus::Running, RunStatus::TimedOut),
            (RunStatus::Running, RunStatus::Cancelled),
        ] {
            assert!(validate_transition(RunKind::Delivery, from, to).is_ok(),
                "expected Delivery edge {from:?} -> {to:?}");
        }
    }

    #[test]
    fn reviewer_run_keeps_its_separate_lifecycle() {
        for to in [RunStatus::Succeeded, RunStatus::Failed, RunStatus::TimedOut] {
            assert!(validate_transition(RunKind::Review, RunStatus::Running, to).is_ok());
        }
        assert!(validate_transition(RunKind::Review, RunStatus::Queued, RunStatus::Running).is_ok());
        assert!(validate_transition(RunKind::Review, RunStatus::Queued, RunStatus::Claimed).is_err());
        assert!(validate_transition(RunKind::Review, RunStatus::Claimed, RunStatus::Running).is_err());
        assert!(validate_transition(RunKind::Review, RunStatus::Running, RunStatus::Cancelled).is_err());
    }

    #[test]
    fn terminal_runs_are_immutable_for_both_kinds() {
        for kind in [RunKind::Delivery, RunKind::Review] {
            for terminal in [RunStatus::Succeeded, RunStatus::Failed, RunStatus::TimedOut, RunStatus::Cancelled] {
                assert!(matches!(validate_transition(kind, terminal, RunStatus::Running),
                    Err(RunTransitionError::TerminalImmutable(_))));
            }
        }
    }

    #[test]
    fn same_state_reclaim_is_not_a_status_transition() {
        assert!(validate_transition(RunKind::Delivery, RunStatus::Claimed, RunStatus::Claimed).is_err());
        assert!(validate_reacquire(RunKind::Delivery, RunStatus::Claimed).is_ok());
        assert!(validate_reacquire(RunKind::Review, RunStatus::Running).is_err());
    }

    #[test]
    fn unknown_database_values_fail_closed() {
        assert!(RunKind::parse("other").is_err());
        assert!(RunStatus::parse("waiting").is_err());
    }
}
