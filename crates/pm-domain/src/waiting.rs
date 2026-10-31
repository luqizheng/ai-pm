//! Waiting context for `AwaitingDecision` (`ai-pm-task-state-machine-v1.md` §十二).

use serde::{Deserialize, Serialize};

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
