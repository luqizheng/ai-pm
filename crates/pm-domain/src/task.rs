//! Task lifecycle status.

use serde::{Deserialize, Serialize};

use crate::waiting::WaitingReason;

/// Lifecycle status of a `delivery.task` row. Stored as lowercase snake-case TEXT.
///
/// 13-state machine per `ai-pm-task-state-machine-v1.md` (V12, §十二 供稿数据模型).
/// The four [`TaskStatus::TERMINAL`] states have no outgoing edges; `Superseded`
/// accepts a fixed `allowed_from` set (§11.2). Transition legality is enforced by
/// [`crate::task_transition`], not by this enum.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
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
}

impl TaskStatus {
    /// Terminal states — zero outgoing edges (§11.1 终态定义, 图1 底注).
    pub const TERMINAL: [TaskStatus; 4] = [
        TaskStatus::Completed,
        TaskStatus::Cancelled,
        TaskStatus::Failed,
        TaskStatus::Superseded,
    ];

    pub fn is_terminal(self) -> bool {
        TaskStatus::TERMINAL.contains(&self)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Draft => "draft",
            TaskStatus::Ready => "ready",
            TaskStatus::Claimed => "claimed",
            TaskStatus::Running => "running",
            TaskStatus::AwaitingDecision => "awaiting_decision",
            TaskStatus::Blocked => "blocked",
            TaskStatus::Submitted => "submitted",
            TaskStatus::Reviewing => "reviewing",
            TaskStatus::Completed => "completed",
            TaskStatus::ChangesRequested => "changes_requested",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
            TaskStatus::Superseded => "superseded",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(TaskStatus::Draft),
            "ready" => Some(TaskStatus::Ready),
            "claimed" => Some(TaskStatus::Claimed),
            "running" => Some(TaskStatus::Running),
            "awaiting_decision" => Some(TaskStatus::AwaitingDecision),
            "blocked" => Some(TaskStatus::Blocked),
            "submitted" => Some(TaskStatus::Submitted),
            "reviewing" => Some(TaskStatus::Reviewing),
            "completed" => Some(TaskStatus::Completed),
            "changes_requested" => Some(TaskStatus::ChangesRequested),
            "failed" => Some(TaskStatus::Failed),
            "cancelled" => Some(TaskStatus::Cancelled),
            "superseded" => Some(TaskStatus::Superseded),
            _ => None,
        }
    }

    /// Maps the legacy 5-value status (`ai-pm-task-state-machine-v1.md` §九 M1–M4
    /// + V3 L1). Returns `None` for values outside the legacy set — the strict
    /// [`TaskStatus::from_str`] owns the 13-value set.
    ///
    /// `in_progress` is deliberately [`LegacyMapping::Ambiguous`]: it splits into
    /// `Claimed`/`Running` depending on whether an active Run exists, a fact this
    /// function cannot know (M1 禁止默认映射). `done` maps without evidence here —
    /// the acceptance-evidence check is a DB-migration duty (M3).
    pub fn legacy_from_str(s: &str) -> Option<LegacyMapping> {
        match s {
            "backlog" => Some(LegacyMapping::Mapped(TaskStatus::Draft, None)),
            "in_progress" => Some(LegacyMapping::Ambiguous {
                candidates: &[TaskStatus::Claimed, TaskStatus::Running],
            }),
            "blocked" => Some(LegacyMapping::Mapped(TaskStatus::Blocked, None)),
            "needs_info" => Some(LegacyMapping::Mapped(
                TaskStatus::AwaitingDecision,
                Some(WaitingReason::NeedsInfo),
            )),
            "done" => Some(LegacyMapping::Mapped(TaskStatus::Completed, None)),
            _ => None,
        }
    }
}

/// Result of mapping one legacy status string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyMapping {
    /// Deterministic mapping. The optional reason is `Some` only when the target
    /// is `AwaitingDecision` (`needs_info` → `NeedsInfo`, V3 L1).
    Mapped(TaskStatus, Option<WaitingReason>),
    /// Cannot be resolved without external facts; callers must resolve explicitly.
    Ambiguous {
        candidates: &'static [TaskStatus],
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::waiting::WaitingReason;

    #[test]
    fn task_status_string_roundtrip_covers_all_variants() {
        let all = [
            (TaskStatus::Draft, "draft"),
            (TaskStatus::Ready, "ready"),
            (TaskStatus::Claimed, "claimed"),
            (TaskStatus::Running, "running"),
            (TaskStatus::AwaitingDecision, "awaiting_decision"),
            (TaskStatus::Blocked, "blocked"),
            (TaskStatus::Submitted, "submitted"),
            (TaskStatus::Reviewing, "reviewing"),
            (TaskStatus::Completed, "completed"),
            (TaskStatus::ChangesRequested, "changes_requested"),
            (TaskStatus::Failed, "failed"),
            (TaskStatus::Cancelled, "cancelled"),
            (TaskStatus::Superseded, "superseded"),
        ];
        assert_eq!(all.len(), 13);
        for (status, text) in all {
            assert_eq!(status.as_str(), text);
            assert_eq!(TaskStatus::from_str(text), Some(status));
        }
        // Strict parser: legacy and unknown values never parse (§九 M2/M4 —
        // legacy values go through `legacy_from_str` only).
        assert_eq!(TaskStatus::from_str("archived"), None);
        assert_eq!(TaskStatus::from_str("backlog"), None);
        assert_eq!(TaskStatus::from_str("needs_info"), None);
        assert_eq!(TaskStatus::from_str("done"), None);
    }

    #[test]
    fn terminal_states_are_exactly_four() {
        assert_eq!(
            TaskStatus::TERMINAL,
            [
                TaskStatus::Completed,
                TaskStatus::Cancelled,
                TaskStatus::Failed,
                TaskStatus::Superseded,
            ]
        );
        for status in TaskStatus::TERMINAL {
            assert!(status.is_terminal());
        }
        for open in [
            TaskStatus::Draft,
            TaskStatus::Ready,
            TaskStatus::Claimed,
            TaskStatus::Running,
            TaskStatus::AwaitingDecision,
            TaskStatus::Blocked,
            TaskStatus::Submitted,
            TaskStatus::Reviewing,
            TaskStatus::ChangesRequested,
        ] {
            assert!(!open.is_terminal());
        }
    }

    #[test]
    fn legacy_mapping_covers_all_five_legacy_values() {
        assert_eq!(
            TaskStatus::legacy_from_str("backlog"),
            Some(LegacyMapping::Mapped(TaskStatus::Draft, None))
        );
        assert_eq!(
            TaskStatus::legacy_from_str("blocked"),
            Some(LegacyMapping::Mapped(TaskStatus::Blocked, None))
        );
        assert_eq!(
            TaskStatus::legacy_from_str("done"),
            Some(LegacyMapping::Mapped(TaskStatus::Completed, None))
        );
        // §九 M2 + V3 L1: needs_info merges into AwaitingDecision with the
        // NeedsInfo waiting reason — no state-level fork.
        assert_eq!(
            TaskStatus::legacy_from_str("needs_info"),
            Some(LegacyMapping::Mapped(
                TaskStatus::AwaitingDecision,
                Some(WaitingReason::NeedsInfo),
            ))
        );
        assert_eq!(TaskStatus::legacy_from_str("archived"), None);
    }

    #[test]
    fn legacy_in_progress_is_ambiguous() {
        // §九 M1: in_progress splits into Claimed/Running depending on whether an
        // active Run exists. The mapping must NOT pick one by default.
        assert_eq!(
            TaskStatus::legacy_from_str("in_progress"),
            Some(LegacyMapping::Ambiguous {
                candidates: &[TaskStatus::Claimed, TaskStatus::Running],
            })
        );
    }
}
