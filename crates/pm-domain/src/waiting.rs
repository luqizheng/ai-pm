//! Waiting context for `AwaitingDecision` (`ai-pm-task-state-machine-v1.md` §十二).

use serde::{Deserialize, Serialize};

use crate::ids::PrincipalId;
use crate::task_transition::TransitionError;

/// Why a task sits in `AwaitingDecision`.
///
/// Four reasons share one task state (§12.2 底注：「同一任务状态下的等待原因示例，
/// 不是四个新的任务状态」). `needs_info` was retired as a task status and lives on
/// only as [`WaitingReason::NeedsInfo`] (§12.1 最终结论).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WaitingReason {
    /// 缺少需求、字段定义、接口契约或业务规则（needs_info）。
    NeedsInfo,
    /// 存在多个可行方案，需要负责人作出选择（needs_decision）。
    NeedsDecision,
    /// 等待具有审批权限的人批准（needs_approval）。
    NeedsApproval,
    /// 等待外部系统、团队或资源（external_dependency）。
    ExternalDependency,
}

impl WaitingReason {
    pub fn as_str(self) -> &'static str {
        match self {
            WaitingReason::NeedsInfo => "needs_info",
            WaitingReason::NeedsDecision => "needs_decision",
            WaitingReason::NeedsApproval => "needs_approval",
            WaitingReason::ExternalDependency => "external_dependency",
        }
    }

    #[allow(clippy::should_implement_trait)] // 仓库惯例：inherent from_str/as_str 配对
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "needs_info" => Some(WaitingReason::NeedsInfo),
            "needs_decision" => Some(WaitingReason::NeedsDecision),
            "needs_approval" => Some(WaitingReason::NeedsApproval),
            "external_dependency" => Some(WaitingReason::ExternalDependency),
            _ => None,
        }
    }
}

/// Structured waiting context attached to a task in `AwaitingDecision` (§12.4).
///
/// 「状态」和「等待上下文」分开存储：the status stays a plain enum value; everything
/// that explains *why* it waits lives here. History is append-only — a task that
/// leaves the waiting state keeps its past contexts (storage responsibility of
/// Slice B; this type only models one context).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaitingContext {
    pub reason: WaitingReason,
    /// Open questions the task waits on (§12.3: 设置 waiting_reason 时列出具体问题).
    pub questions: Vec<String>,
    /// Who was asked (user/actor the request was directed at), if known.
    pub requested_from: Option<PrincipalId>,
    pub since: chrono::DateTime<chrono::Utc>,
    /// When the wait should escalate (提醒/升级, §12.3 等待超时) — Q2.2 pending.
    pub deadline: Option<chrono::DateTime<chrono::Utc>>,
}

impl WaitingContext {
    /// §12.3: 进入等待必须列出具体问题；空问题集的等待不是等待。
    pub fn new(
        reason: WaitingReason,
        questions: Vec<String>,
    ) -> Result<Self, TransitionError> {
        if questions.is_empty() {
            return Err(TransitionError::EmptyQuestions);
        }
        Ok(Self {
            reason,
            questions,
            requested_from: None,
            since: chrono::Utc::now(),
            deadline: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::PrincipalId;
    use crate::task_transition::TransitionError;

    #[test]
    fn waiting_reason_roundtrip_covers_all_variants() {
        let all = [
            (WaitingReason::NeedsInfo, "needs_info"),
            (WaitingReason::NeedsDecision, "needs_decision"),
            (WaitingReason::NeedsApproval, "needs_approval"),
            (WaitingReason::ExternalDependency, "external_dependency"),
        ];
        for (reason, text) in all {
            assert_eq!(reason.as_str(), text);
            assert_eq!(WaitingReason::from_str(text), Some(reason));
        }
        assert_eq!(WaitingReason::from_str("waiting"), None);
    }

    #[test]
    fn context_rejects_empty_questions() {
        // §12.3: 进入 AwaitingDecision 时须“列出具体问题”——没有待解决问题的
        // 等待不是等待。
        let err = WaitingContext::new(WaitingReason::NeedsInfo, Vec::new())
            .expect_err("empty questions must be rejected");
        assert_eq!(err, TransitionError::EmptyQuestions);
    }

    #[test]
    fn context_accepts_needs_info_with_questions() {
        let ctx = WaitingContext::new(
            WaitingReason::NeedsInfo,
            vec!["接口契约字段 A 的类型是什么？".to_string()],
        )
        .expect("questions present");
        assert_eq!(ctx.reason, WaitingReason::NeedsInfo);
        assert_eq!(ctx.questions.len(), 1);
        assert_eq!(ctx.requested_from, None::<PrincipalId>);
        assert_eq!(ctx.deadline, None);
    }
}
