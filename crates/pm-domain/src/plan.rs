//! S04 任务计划域的纯状态机（S4 提案 V1 §1.3/Q2/Q6 裁决落地）。
//!
//! 两个生命周期：
//! - [`TaskPlanStatus`]：计划聚合根状态。`draft → in_review → approved`；
//!   退回（`changes_requested` 语义）回到 `draft` 修改后重新提交（提案 §1.3
//!   CHANGES_REQUESTED→REVISED→IN_REVIEW 的收敛形态：Plan Revision 承担
//!   REVISED，计划状态本身退回 draft 避免两条进入 approved 的路径）；
//!   已批准计划被新计划替代时 `superseded`。
//! - [`TaskPlanRevisionState`]：计划内容版本状态（draft 可改稿 → submitted
//!   冻结 → approved/rejected 裁决；批准新修订时旧批准修订退位 superseded，
//!   与 design_revision 同款纪律）。
//!
//! Q6 裁决：任务状态沿用 13 态（draft=已规划 PLANNED、ready=可执行 READY、
//! blocked=被阻塞 BLOCKED），不在本模块重复定义。
//!
//! 正式任务批量的 draft→ready 沿用 `pm_write::mark_task_batch_ready`
//! （「AI 永不置 ready」红线在 require_human，E2）。

/// 计划聚合根状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPlanStatus {
    /// 草案/修改中：可自由改稿、发起拆解。
    Draft,
    /// 人工审批中：内容已冻结，等待裁决。
    InReview,
    /// 已批准：可正式化为任务、可交接 S05。
    Approved,
    /// 被替代（新计划批准后旧计划显式退位）。
    Superseded,
}

impl TaskPlanStatus {
    pub const ALL: [Self; 4] = [Self::Draft, Self::InReview, Self::Approved, Self::Superseded];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::InReview => "in_review",
            Self::Approved => "approved",
            Self::Superseded => "superseded",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.as_str() == value)
    }

    /// 提案 §1.3 生命周期矩阵（收敛形态）。
    pub fn can_transition_to(self, to: Self) -> bool {
        matches!(
            (self, to),
            (Self::Draft, Self::InReview)
                | (Self::InReview, Self::Draft) // 退回修改
                | (Self::InReview, Self::Approved) // 人工批准
                | (Self::Approved, Self::Superseded) // 新计划替代
        )
    }

    /// 内容是否仍可编辑（仅 draft）。
    pub fn content_editable(self) -> bool {
        self == Self::Draft
    }
}

/// 计划内容版本状态（与 design_revision 同款纪律）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPlanRevisionState {
    Draft,
    Submitted,
    Approved,
    Rejected,
    Superseded,
}

impl TaskPlanRevisionState {
    pub const ALL: [Self; 5] = [
        Self::Draft,
        Self::Submitted,
        Self::Approved,
        Self::Rejected,
        Self::Superseded,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Submitted => "submitted",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.as_str() == value)
    }

    pub fn can_transition_to(self, to: Self) -> bool {
        matches!(
            (self, to),
            (Self::Draft, Self::Submitted)
                | (Self::Draft, Self::Rejected)
                | (Self::Submitted, Self::Approved)
                | (Self::Submitted, Self::Rejected)
                | (Self::Approved, Self::Superseded)
        )
    }

    pub fn content_editable(self) -> bool {
        self == Self::Draft
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_status_matrix() {
        assert!(TaskPlanStatus::Draft.can_transition_to(TaskPlanStatus::InReview));
        assert!(TaskPlanStatus::InReview.can_transition_to(TaskPlanStatus::Draft));
        assert!(TaskPlanStatus::InReview.can_transition_to(TaskPlanStatus::Approved));
        assert!(TaskPlanStatus::Approved.can_transition_to(TaskPlanStatus::Superseded));
        // draft 不得直批（必须经 in_review，人工控制红线）。
        assert!(!TaskPlanStatus::Draft.can_transition_to(TaskPlanStatus::Approved));
        assert!(!TaskPlanStatus::Superseded.can_transition_to(TaskPlanStatus::Draft));
        assert!(TaskPlanStatus::Draft.content_editable());
        assert!(!TaskPlanStatus::InReview.content_editable());
    }

    #[test]
    fn plan_revision_matrix() {
        assert!(TaskPlanRevisionState::Draft.can_transition_to(TaskPlanRevisionState::Submitted));
        assert!(!TaskPlanRevisionState::Draft.can_transition_to(TaskPlanRevisionState::Approved));
        assert!(TaskPlanRevisionState::Submitted.can_transition_to(TaskPlanRevisionState::Approved));
        assert!(TaskPlanRevisionState::Approved.can_transition_to(TaskPlanRevisionState::Superseded));
        assert!(TaskPlanRevisionState::Draft.content_editable());
        assert!(!TaskPlanRevisionState::Submitted.content_editable());
    }

    #[test]
    fn wire_roundtrip() {
        for s in TaskPlanStatus::ALL {
            assert_eq!(TaskPlanStatus::parse(s.as_str()), Some(s));
        }
        for s in TaskPlanRevisionState::ALL {
            assert_eq!(TaskPlanRevisionState::parse(s.as_str()), Some(s));
        }
    }
}
