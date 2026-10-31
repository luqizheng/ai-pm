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

/// Full ruled edge matrix: §1.3 规范版 + §11.2 Superseded `allowed_from` +
/// §12.3 waiting resume edges + §19.6 Running branches.
///
/// Terminal states have no outgoing edges (§11.1); `Claimed`,
/// `AwaitingDecision` and `ChangesRequested` deliberately do **not** reach
/// `Superseded` (Q1.1 未裁，不得擅自放宽).
pub fn allowed_edges() -> &'static [(TaskStatus, TaskStatus)] {
    use TaskStatus::*;
    &[
        (ChangesRequested, Cancelled),
        (ChangesRequested, Ready),
        (AwaitingDecision, Cancelled),
        (AwaitingDecision, Ready),
        (AwaitingDecision, Running),
        (Blocked, Cancelled),
        (Blocked, Ready),
        (Blocked, Superseded),
        (Claimed, Cancelled),
        (Claimed, Ready),
        (Claimed, Running),
        (Draft, Cancelled),
        (Draft, Ready),
        (Draft, Superseded),
        (Ready, Cancelled),
        (Ready, Claimed),
        (Ready, Superseded),
        (Reviewing, Cancelled),
        (Reviewing, ChangesRequested),
        (Reviewing, Completed),
        (Reviewing, Superseded),
        (Running, AwaitingDecision),
        (Running, Blocked),
        (Running, Cancelled),
        (Running, Failed),
        (Running, Submitted),
        (Running, Superseded),
        (Submitted, Cancelled),
        (Submitted, Reviewing),
        (Submitted, Superseded),
    ]
}

/// Refuses any transition whose edge is not in the ruled matrix.
///
/// Guards (supersession intent, waiting resolution, failure/acceptance evidence)
/// are layered on top by later tasks in this module; this function is the single
/// entry point and must stay the only legal path into a status change (§7.4).
pub fn validate_transition(input: &TransitionInput) -> Result<(), TransitionError> {
    let edge = (input.from, input.to);
    if !allowed_edges().contains(&edge) {
        return Err(TransitionError::IllegalTransition {
            from: input.from,
            to: input.to,
        });
    }
    Ok(())
}

/// Facts a caller must supply for one transition. External realities (an active
/// Run stopped, a replacement task existing, waiting being resolved) are inputs,
/// not lookups: the domain stays I/O-free (§7.3 伪代码的事务编排归应用层).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionInput {
    /// The task undergoing the transition (needed for self-supersession check).
    pub task_id: Option<crate::ids::TaskId>,
    pub from: TaskStatus,
    pub to: TaskStatus,
    /// Present when entering `AwaitingDecision` (§12.3).
    pub waiting: Option<crate::waiting::WaitingContext>,
    /// Set by the caller after verifying every open question is answered (§12.3
    /// 关键约束: 不要在收到任何回复后就自动恢复任务 — the caller checks content;
    /// the domain only demands the explicit flag).
    pub waiting_resolved: bool,
    /// Set once the replacement task exists and the relation is persisted
    /// (§11.5 第 5 条: 新任务必须先存在).
    pub replacement_task_exists: bool,
    /// §11.2 `stop_active_run_before_finalization`.
    pub active_run_stopped: bool,
    /// §19.4 task-level retry counters exhausted (B4-C).
    pub retry_exhausted: bool,
    /// §19.2 B4-B: the task's total deadline is spent.
    pub deadline_exhausted: bool,
    /// §19.5 structured failure driving `Running → Failed` (B4-A/B4-D).
    pub failure: Option<crate::task_run::FailureRecord>,
}

impl TransitionInput {
    /// Bare minimum for guard-free edges (e.g. `Draft → Ready`).
    pub fn minimal(from: TaskStatus, to: TaskStatus) -> Self {
        Self {
            task_id: None,
            from,
            to,
            waiting: None,
            waiting_resolved: false,
            replacement_task_exists: false,
            active_run_stopped: false,
            retry_exhausted: false,
            deadline_exhausted: false,
            failure: None,
        }
    }

    /// Carries the task identity (required for supersession checks).
    pub fn for_task(
        task_id: crate::ids::TaskId,
        from: TaskStatus,
        to: TaskStatus,
    ) -> Self {
        Self {
            task_id: Some(task_id),
            ..Self::minimal(from, to)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TaskStatus::*;

    /// The exact set of edges this module must accept, kept in test-local form so
    /// a drift in `allowed_edges()` (adding or dropping an edge) fails loudly.
    fn ruled_edges() -> Vec<(TaskStatus, TaskStatus)> {
        let mut edges = allowed_edges().to_vec();
        edges.sort();
        edges
    }

    #[test]
    fn allowed_edges_has_no_duplicates() {
        let mut edges = allowed_edges().to_vec();
        edges.sort();
        let mut uniq = edges.clone();
        uniq.dedup();
        assert_eq!(edges.len(), uniq.len(), "duplicate edge in matrix");
    }

    #[test]
    fn edge_count_matches_ruled_matrix() {
        // 21 non-cancel edges + 9 cancel edges (every non-terminal state) = 30.
        assert_eq!(allowed_edges().len(), 30);
        assert_eq!(ruled_edges().len(), 30);
    }

    #[test]
    fn terminal_states_have_no_outgoing_edges() {
        for terminal in TaskStatus::TERMINAL {
            for to in [
                Draft, Ready, Claimed, Running, AwaitingDecision, Blocked, Submitted,
                Reviewing, Completed, ChangesRequested, Failed, Cancelled, Superseded,
            ] {
                let input = TransitionInput::minimal(terminal, to);
                assert!(
                    matches!(
                        validate_transition(&input),
                        Err(TransitionError::IllegalTransition { .. })
                    ),
                    "terminal {terminal:?} must not transition to {to:?}"
                );
            }
        }
    }

    #[test]
    fn identity_transition_is_illegal() {
        for status in [
            Draft, Ready, Claimed, Running, AwaitingDecision, Blocked, Submitted,
            Reviewing, Completed, ChangesRequested, Failed, Cancelled, Superseded,
        ] {
            let input = TransitionInput::minimal(status, status);
            assert!(validate_transition(&input).is_err(), "{status:?} -> itself");
        }
    }

    #[test]
    fn cancel_allowed_from_every_non_terminal() {
        for from in [
            Draft, Ready, Claimed, Running, AwaitingDecision, Blocked, Submitted,
            Reviewing, ChangesRequested,
        ] {
            let input = TransitionInput::minimal(from, Cancelled);
            assert!(
                validate_transition(&input).is_ok(),
                "{from:?} -> Cancelled must be allowed"
            );
        }
    }

    #[test]
    fn cancel_rejected_from_terminal_states() {
        for from in TaskStatus::TERMINAL {
            let input = TransitionInput::minimal(from, Cancelled);
            assert!(validate_transition(&input).is_err());
        }
    }

    #[test]
    fn supersede_rejected_outside_ruled_sources() {
        // Q1.1 未裁：Claimed / AwaitingDecision / ChangesRequested 不得直接取代。
        for from in [Claimed, AwaitingDecision, ChangesRequested] {
            let input = TransitionInput::minimal(from, Superseded);
            assert!(
                matches!(
                    validate_transition(&input),
                    Err(TransitionError::IllegalTransition { .. })
                ),
                "{from:?} -> Superseded must be illegal"
            );
        }
    }

    #[test]
    fn supersede_allowed_from_ruled_sources() {
        // §11.2 allowed_from: Draft, Ready, Blocked, Running, Submitted, Reviewing.
        for from in [Draft, Ready, Blocked, Running, Submitted, Reviewing] {
            let input = TransitionInput::minimal(from, Superseded);
            assert!(
                validate_transition(&input).is_ok(),
                "{from:?} -> Superseded must be allowed at matrix level"
            );
        }
    }

    #[test]
    fn running_branches_match_ruling_19_6() {
        // 成功→Submitted；临时故障→新 Run（域内不表达）；外部处理→Blocked；
        // 人工决策→AwaitingDecision；终止条件→Failed；取消→Cancelled。
        for to in [
            Submitted, Blocked, AwaitingDecision, Failed, Cancelled, Superseded,
        ] {
            let input = TransitionInput::minimal(Running, to);
            assert!(
                validate_transition(&input).is_ok(),
                "Running -> {to:?} must be allowed at matrix level"
            );
        }
        // Running 没有直达 Ready/Completed/ChangesRequested 的边。
        for to in [Ready, Completed, ChangesRequested, Reviewing, Draft, Claimed] {
            let input = TransitionInput::minimal(Running, to);
            assert!(validate_transition(&input).is_err(), "Running -> {to:?}");
        }
    }
}
