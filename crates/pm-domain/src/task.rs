//! Task lifecycle status.

use serde::{Deserialize, Serialize};

/// Lifecycle status of a `delivery.task` row. Stored as lowercase snake-case TEXT.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Backlog,
    InProgress,
    Blocked,
    NeedsInfo,
    Done,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Backlog => "backlog",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Blocked => "blocked",
            TaskStatus::NeedsInfo => "needs_info",
            TaskStatus::Done => "done",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "backlog" => Some(TaskStatus::Backlog),
            "in_progress" => Some(TaskStatus::InProgress),
            "blocked" => Some(TaskStatus::Blocked),
            "needs_info" => Some(TaskStatus::NeedsInfo),
            "done" => Some(TaskStatus::Done),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_status_string_roundtrip_covers_all_variants() {
        let all = [
            (TaskStatus::Backlog, "backlog"),
            (TaskStatus::InProgress, "in_progress"),
            (TaskStatus::Blocked, "blocked"),
            (TaskStatus::NeedsInfo, "needs_info"),
            (TaskStatus::Done, "done"),
        ];
        for (status, text) in all {
            assert_eq!(status.as_str(), text);
            assert_eq!(TaskStatus::from_str(text), Some(status));
        }
        assert_eq!(TaskStatus::from_str("archived"), None);
    }
}
