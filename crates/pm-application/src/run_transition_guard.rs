//! PA08-03: shared Run status-transition matrix.
//!
//! This validator is intentionally separate from Task transitions. Callers must
//! still provide transaction/row-lock or compare-and-set protection around writes.
//! Delivery and Reviewer Runs share a table but retain separate lifecycle edges.

use sqlx::{Postgres, Row, Transaction};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunKind {
    Delivery,
    Review,
}

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
    Queued,
    Claimed,
    Running,
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
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
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::TimedOut | Self::Cancelled
        )
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
    IllegalEdge {
        kind: RunKind,
        from: RunStatus,
        to: RunStatus,
    },
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
                | (RunStatus::Running, RunStatus::Cancelled)
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
    Err(RunTransitionError::IllegalEdge {
        kind,
        from: current,
        to: current,
    })
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
            assert!(
                validate_transition(RunKind::Delivery, from, to).is_ok(),
                "expected Delivery edge {from:?} -> {to:?}"
            );
        }
    }

    #[test]
    fn reviewer_run_keeps_its_separate_lifecycle() {
        for to in [RunStatus::Succeeded, RunStatus::Failed, RunStatus::TimedOut] {
            assert!(validate_transition(RunKind::Review, RunStatus::Running, to).is_ok());
        }
        assert!(
            validate_transition(RunKind::Review, RunStatus::Queued, RunStatus::Running).is_ok()
        );
        assert!(
            validate_transition(RunKind::Review, RunStatus::Queued, RunStatus::Claimed).is_err()
        );
        assert!(
            validate_transition(RunKind::Review, RunStatus::Claimed, RunStatus::Running).is_err()
        );
        // Active Reviewer Runs may be cancelled by Project archival; the
        // application caller restricts this edge to reason=project_closed.
    }

    #[test]
    fn terminal_runs_are_immutable_for_both_kinds() {
        for kind in [RunKind::Delivery, RunKind::Review] {
            for terminal in [
                RunStatus::Succeeded,
                RunStatus::Failed,
                RunStatus::TimedOut,
                RunStatus::Cancelled,
            ] {
                assert!(matches!(
                    validate_transition(kind, terminal, RunStatus::Running),
                    Err(RunTransitionError::TerminalImmutable(_))
                ));
            }
        }
    }

    #[test]
    fn same_state_reclaim_is_not_a_status_transition() {
        assert!(
            validate_transition(RunKind::Delivery, RunStatus::Claimed, RunStatus::Claimed).is_err()
        );
        assert!(validate_reacquire(RunKind::Delivery, RunStatus::Claimed).is_ok());
        assert!(validate_reacquire(RunKind::Review, RunStatus::Running).is_err());
    }

    #[test]
    fn unknown_database_values_fail_closed() {
        assert!(RunKind::parse("other").is_err());
        assert!(RunStatus::parse("waiting").is_err());
    }
}

#[derive(Debug, Error)]
pub enum RunTransitionGuardError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Run {0} was not found in the expected Project")]
    NotFound(Uuid),
    #[error(transparent)]
    Invalid(#[from] RunTransitionError),
}

/// Lock the Run row, validate its kind and source-to-target edge, and return
/// the source status. The caller must perform its UPDATE in this same transaction
/// with a source-status predicate (compare-and-set) and keep audit/effects atomic.
pub async fn lock_and_validate_transition(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    run_id: Uuid,
    expected_kind: RunKind,
    target: RunStatus,
) -> Result<RunStatus, RunTransitionGuardError> {
    let row = sqlx::query(
        "SELECT status, run_kind FROM delivery.task_run \
          WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(RunTransitionGuardError::NotFound(run_id))?;
    let status_text: String = row.get("status");
    let kind_text: String = row.get("run_kind");
    let status = RunStatus::parse(&status_text)?;
    let kind = RunKind::parse(&kind_text)?;
    if kind != expected_kind {
        return Err(RunTransitionError::IllegalEdge {
            kind,
            from: status,
            to: target,
        }
        .into());
    }
    validate_transition(kind, status, target)?;
    Ok(status)
}

/// Lock and validate a Delivery Run whose status remains claimed while its
/// lease token/generation are replaced. Callers must still enforce expiry and
/// generation increment semantics in the claim/reacquire SQL.
pub async fn lock_and_validate_reacquire(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    run_id: Uuid,
) -> Result<(), RunTransitionGuardError> {
    let row = sqlx::query(
        "SELECT status, run_kind FROM delivery.task_run \
          WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(RunTransitionGuardError::NotFound(run_id))?;
    let status = RunStatus::parse(row.get::<String, _>("status").as_str())?;
    let kind = RunKind::parse(row.get::<String, _>("run_kind").as_str())?;
    validate_reacquire(kind, status)?;
    Ok(())
}
