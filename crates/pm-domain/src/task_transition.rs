//! Task transition rules (`docs/tasks/ai-pm-task-state-machine-v1.md` §1.3 / §十一 / §十二 / §十九).
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
    // §12.3 关键约束：未解除的等待不得**恢复执行**（Ready/Running）；取消走取消
    // 流程，不要求先解决等待。内容核对归调用方，域层只认显式标志。
    if input.from == TaskStatus::AwaitingDecision
        && !input.waiting_resolved
        && matches!(input.to, TaskStatus::Ready | TaskStatus::Running)
    {
        return Err(TransitionError::MissingEvidence(
            "waiting context must be resolved before leaving AwaitingDecision",
        ));
    }
    // §三 E1：Running → Submitted 不能只依据 Codex 的文字声明。
    if input.from == TaskStatus::Running
        && input.to == TaskStatus::Submitted
        && !input.delivery_submitted
    {
        return Err(TransitionError::MissingEvidence(
            "delivery manifest must be submitted (E1)",
        ));
    }
    // §三 E2：Reviewing → Completed 不能只依赖测试全部通过。
    if input.from == TaskStatus::Reviewing
        && input.to == TaskStatus::Completed
        && !input.acceptance_evidence_complete
    {
        return Err(TransitionError::MissingEvidence(
            "acceptance evidence must be complete (E2)",
        ));
    }
    // §19.2：Running → Failed 需要终止性失败条件（B4-A..B4-D）。retryable 的
    // Execution/Infrastructure/Validation/Unknown 失败是临时故障证据，单独不构成
    // 任务级失败；B4-C（重试耗尽）与 B4-B（总期限耗尽）独立放行。
    if input.from == TaskStatus::Running && input.to == TaskStatus::Failed {
        let terminating_failure = input.failure.as_ref().is_some_and(|f| {
            matches!(
                f.category,
                crate::task_run::FailureCategory::Permission
                    | crate::task_run::FailureCategory::PolicyViolation
                    | crate::task_run::FailureCategory::MissingDependency
            )
        });
        if !(input.retry_exhausted || input.deadline_exhausted || terminating_failure) {
            return Err(TransitionError::MissingEvidence(
                "no terminating failure condition (B4-A..B4-D)",
            ));
        }
    }
    // §11.2 / §11.5 supersession constraints.
    if input.to == TaskStatus::Superseded {
        let intent = input.supersede.as_ref().ok_or_else(|| {
            TransitionError::SupersessionViolation(
                "superseding_task_id, reason and actor are required".to_string(),
            )
        })?;
        let task_id = input.task_id.ok_or_else(|| {
            TransitionError::SupersessionViolation("task identity is required".to_string())
        })?;
        if intent.superseding_task_id == task_id {
            return Err(TransitionError::SupersessionViolation(
                "a task cannot supersede itself".to_string(),
            ));
        }
        if intent.reason.trim().is_empty() {
            return Err(TransitionError::SupersessionViolation(
                "a non-empty reason is required".to_string(),
            ));
        }
        // §11.5 第 5 条：新任务必须先存在，才能正式完成取代操作。
        if !input.replacement_task_exists {
            return Err(TransitionError::MissingEvidence(
                "replacement task must exist before superseding",
            ));
        }
        // §11.2 stop_active_run_before_finalization：停止 Run 是迁入终态的前置
        // 守卫，不是事后补偿（§11.6：不能因点击「取代」就假定执行已结束）。
        if input.from == TaskStatus::Running && !input.active_run_stopped {
            return Err(TransitionError::MissingEvidence(
                "active run must be stopped before superseding",
            ));
        }
    }
    Ok(())
}

/// Explicit supersession intent (§11.5)：旧任务保存 `superseded_by_task_id`，
/// 关联表记录取代关系；本结构承载迁移校验所需的最小必填信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupersedeIntent {
    /// The replacement task (must already exist; caller persists the relation).
    pub superseding_task_id: crate::ids::TaskId,
    /// 为什么取代（必填，落审计）。
    pub reason: String,
    /// 谁发起取代（必填，落审计）。
    pub actor: crate::ids::PrincipalId,
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
    /// §11.1 必填信息 (→ Superseded 时)。
    pub supersede: Option<SupersedeIntent>,
    /// §三 E1：交付清单已提交且 Commit 可读取（Running → Submitted 时必须为 true）。
    pub delivery_submitted: bool,
    /// §三 E2：验收证据与审批策略均满足（Reviewing → Completed 时必须为 true）。
    pub acceptance_evidence_complete: bool,
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
            supersede: None,
            delivery_submitted: false,
            acceptance_evidence_complete: false,
        }
    }

    /// Carries the task identity (required for supersession checks).
    pub fn for_task(task_id: crate::ids::TaskId, from: TaskStatus, to: TaskStatus) -> Self {
        Self {
            task_id: Some(task_id),
            ..Self::minimal(from, to)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{PrincipalId, TaskId};
    use TaskStatus::*;

    /// The exact set of edges this module must accept, kept in test-local form so
    /// a drift in `allowed_edges()` (adding or dropping an edge) fails loudly.
    fn ruled_edges() -> Vec<(TaskStatus, TaskStatus)> {
        let mut edges = allowed_edges().to_vec();
        edges.sort();
        edges
    }

    /// A supersession input that satisfies every §11.2/§11.5 constraint, so
    /// matrix-level tests exercise the edge, not the guards.
    fn valid_supersede(from: TaskStatus) -> TransitionInput {
        let mut input = TransitionInput::for_task(uuid::Uuid::new_v4(), from, Superseded);
        input.supersede = Some(SupersedeIntent {
            superseding_task_id: uuid::Uuid::new_v4(),
            reason: "superseded by a newer task".to_string(),
            actor: PrincipalId::nil(),
        });
        input.replacement_task_exists = true;
        if from == Running {
            input.active_run_stopped = true;
        }
        input
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
                Draft,
                Ready,
                Claimed,
                Running,
                AwaitingDecision,
                Blocked,
                Submitted,
                Reviewing,
                Completed,
                ChangesRequested,
                Failed,
                Cancelled,
                Superseded,
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
            Draft,
            Ready,
            Claimed,
            Running,
            AwaitingDecision,
            Blocked,
            Submitted,
            Reviewing,
            Completed,
            ChangesRequested,
            Failed,
            Cancelled,
            Superseded,
        ] {
            let input = TransitionInput::minimal(status, status);
            assert!(validate_transition(&input).is_err(), "{status:?} -> itself");
        }
    }

    #[test]
    fn cancel_allowed_from_every_non_terminal() {
        for from in [
            Draft,
            Ready,
            Claimed,
            Running,
            AwaitingDecision,
            Blocked,
            Submitted,
            Reviewing,
            ChangesRequested,
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
            let input = valid_supersede(from);
            assert!(
                validate_transition(&input).is_ok(),
                "{from:?} -> Superseded must be allowed with a valid intent"
            );
        }
    }

    #[test]
    fn running_branches_match_ruling_19_6() {
        // 成功→Submitted（须携带交付证据，E1）；临时故障→新 Run（域内不表达）；
        // 外部处理→Blocked；人工决策→AwaitingDecision；终止条件→Failed（须满足
        // B4-A..B4-D 之一）；取消→Cancelled。
        for to in [Blocked, AwaitingDecision, Cancelled] {
            let input = TransitionInput::minimal(Running, to);
            assert!(
                validate_transition(&input).is_ok(),
                "Running -> {to:?} must be allowed at matrix level"
            );
        }
        let mut submitted = TransitionInput::minimal(Running, Submitted);
        submitted.delivery_submitted = true;
        assert!(validate_transition(&submitted).is_ok());
        let mut failed = TransitionInput::minimal(Running, Failed);
        failed.retry_exhausted = true;
        assert!(validate_transition(&failed).is_ok());
        // Running → Superseded 额外受取代守卫约束（§11.2/§11.6）。
        assert!(validate_transition(&valid_supersede(Running)).is_ok());
        // Running 没有直达 Ready/Completed/ChangesRequested 的边。
        for to in [
            Ready,
            Completed,
            ChangesRequested,
            Reviewing,
            Draft,
            Claimed,
        ] {
            let input = TransitionInput::minimal(Running, to);
            assert!(validate_transition(&input).is_err(), "Running -> {to:?}");
        }
    }

    #[test]
    fn supersede_requires_intent_and_distinct_task() {
        let task = TaskId::nil();
        // 无 intent → 违反（必填：superseding_task_id / 原因 / 操作者，§11.1）。
        let mut input = TransitionInput::for_task(task, Draft, Superseded);
        input.replacement_task_exists = true;
        assert!(matches!(
            validate_transition(&input),
            Err(TransitionError::SupersessionViolation(_))
        ));
        // 自我取代 → 违反（§11.5 禁止任务取代自身）。
        let mut self_input = TransitionInput::for_task(task, Draft, Superseded);
        self_input.replacement_task_exists = true;
        self_input.supersede = Some(SupersedeIntent {
            superseding_task_id: task,
            reason: "replaced by itself".to_string(),
            actor: PrincipalId::nil(),
        });
        assert!(matches!(
            validate_transition(&self_input),
            Err(TransitionError::SupersessionViolation(_))
        ));
        // 空原因 → 违反（原因必填）。
        let mut empty_reason = TransitionInput::for_task(task, Draft, Superseded);
        empty_reason.replacement_task_exists = true;
        empty_reason.supersede = Some(SupersedeIntent {
            superseding_task_id: uuid::Uuid::new_v4(),
            reason: String::new(),
            actor: PrincipalId::nil(),
        });
        assert!(matches!(
            validate_transition(&empty_reason),
            Err(TransitionError::SupersessionViolation(_))
        ));
    }

    #[test]
    fn supersede_requires_existing_replacement_task() {
        // §11.5 第 5 条：新任务必须先存在，才能正式完成取代操作。
        let task = TaskId::nil();
        let mut input = TransitionInput::for_task(task, Ready, Superseded);
        input.supersede = Some(SupersedeIntent {
            superseding_task_id: uuid::Uuid::new_v4(),
            reason: "split into TASK-X".to_string(),
            actor: PrincipalId::nil(),
        });
        assert_eq!(
            validate_transition(&input),
            Err(TransitionError::MissingEvidence(
                "replacement task must exist before superseding",
            ))
        );
        input.replacement_task_exists = true;
        assert!(validate_transition(&input).is_ok());
    }

    #[test]
    fn running_supersede_requires_stopped_active_run() {
        // §11.2 stop_active_run_before_finalization：Running 迁入终态前必须先停 Run；
        // 不能因为用户点击了「取代」就假定执行已经结束（§11.6）。
        let task = TaskId::nil();
        let mut input = TransitionInput::for_task(task, Running, Superseded);
        input.supersede = Some(SupersedeIntent {
            superseding_task_id: uuid::Uuid::new_v4(),
            reason: "superseded by TASK-Y".to_string(),
            actor: PrincipalId::nil(),
        });
        input.replacement_task_exists = true;
        input.active_run_stopped = false;
        assert_eq!(
            validate_transition(&input),
            Err(TransitionError::MissingEvidence(
                "active run must be stopped before superseding",
            ))
        );
        input.active_run_stopped = true;
        assert!(validate_transition(&input).is_ok());
    }

    #[test]
    fn awaiting_decision_requires_resolved_context_to_leave() {
        // §12.3 关键约束：不要在收到任何回复后就自动恢复任务——域层强制显式
        // waiting_resolved 标志，内容核对由调用方负责。
        for to in [Ready, Running] {
            let input = TransitionInput::minimal(AwaitingDecision, to);
            assert_eq!(
                validate_transition(&input),
                Err(TransitionError::MissingEvidence(
                    "waiting context must be resolved before leaving AwaitingDecision",
                )),
                "AwaitingDecision -> {to:?} without resolution must be refused"
            );
            let mut resolved = TransitionInput::minimal(AwaitingDecision, to);
            resolved.waiting_resolved = true;
            assert!(validate_transition(&resolved).is_ok());
        }
    }

    #[test]
    fn running_to_failed_requires_terminating_failure_evidence() {
        use crate::task_run::{FailureCategory, FailureRecord};
        // §19.2 B4-A：退出码非零是证据之一，不单独构成任务失败——retryable 的
        // Execution 失败（临时故障）不得进入 Failed。
        let mut input = TransitionInput::minimal(Running, Failed);
        input.failure = Some(FailureRecord {
            run_id: uuid::Uuid::new_v4(),
            category: FailureCategory::Execution,
            retryable: true,
            reason_code: "compile_error".to_string(),
            exit_code: Some(1),
            timeout_seconds: None,
            evidence_ref: None,
        });
        assert_eq!(
            validate_transition(&input),
            Err(TransitionError::MissingEvidence(
                "no terminating failure condition (B4-A..B4-D)",
            ))
        );
        // B4-D 不可恢复集：权限拒绝 → 允许 Failed。
        let mut permission = TransitionInput::minimal(Running, Failed);
        permission.failure = Some(FailureRecord {
            run_id: uuid::Uuid::new_v4(),
            category: FailureCategory::Permission,
            retryable: false,
            reason_code: "permission_denied".to_string(),
            exit_code: None,
            timeout_seconds: None,
            evidence_ref: None,
        });
        assert!(validate_transition(&permission).is_ok());
        // B4-C：重试耗尽 → 允许 Failed（无需 failure 记录）。
        let mut exhausted = TransitionInput::minimal(Running, Failed);
        exhausted.retry_exhausted = true;
        assert!(validate_transition(&exhausted).is_ok());
        // B4-B：总期限耗尽 → 允许 Failed。
        let mut deadline = TransitionInput::minimal(Running, Failed);
        deadline.deadline_exhausted = true;
        assert!(validate_transition(&deadline).is_ok());
    }

    #[test]
    fn submitted_requires_delivery_evidence() {
        // §三 E1：Running → Submitted 不能只依据 Codex 的文字声明，必须验证
        // Commit、文件变更、测试结果和交付清单。
        let input = TransitionInput::minimal(Running, Submitted);
        assert_eq!(
            validate_transition(&input),
            Err(TransitionError::MissingEvidence(
                "delivery manifest must be submitted (E1)",
            ))
        );
        let mut with_evidence = TransitionInput::minimal(Running, Submitted);
        with_evidence.delivery_submitted = true;
        assert!(validate_transition(&with_evidence).is_ok());
    }

    #[test]
    fn completed_requires_acceptance_evidence() {
        // §三 E2：Reviewing → Completed 不能只依赖测试全部通过，还要确认验收
        // 标准和审批策略均已满足。
        let input = TransitionInput::minimal(Reviewing, Completed);
        assert_eq!(
            validate_transition(&input),
            Err(TransitionError::MissingEvidence(
                "acceptance evidence must be complete (E2)",
            ))
        );
        let mut complete = TransitionInput::minimal(Reviewing, Completed);
        complete.acceptance_evidence_complete = true;
        assert!(validate_transition(&complete).is_ok());
    }
}
