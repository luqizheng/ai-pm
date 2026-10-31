//! Run-level outcome and structured failure records (`ai-pm-task-state-machine-v1.md` §十九).
//!
//! Run 是只追加的执行记录：失败信息不可变，重试产生新 Run，不覆盖旧 Run
//! （§19.5、详情页稿原则一）。本模块先落 §19.5 的数据结构；重试计数与
//! 判定规则由后续任务带测试补充。

use serde::{Deserialize, Serialize};

use crate::ids::RunId;

/// Outcome of one execution instance (Run 级，§19.1 三层失败层级).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
}

/// Structured failure category — failures are classified, never grepped from
/// log text (§19.5「基于结构化的 FailureReason，而不是从日志文本中搜索 error」).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    Infrastructure,
    Execution,
    Validation,
    Permission,
    PolicyViolation,
    MissingDependency,
    Unknown,
}

/// One immutable failure record for a Run (§19.5). Non-zero exit codes land
/// here as *evidence* — 退出码非零是证据之一，不单独构成任务失败（§19.2 B4-A）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FailureRecord {
    pub run_id: RunId,
    pub category: FailureCategory,
    /// Whether the failure is retryable at Run level (§19.3 事件判定表).
    pub retryable: bool,
    /// Machine-readable reason (e.g. `compile_error`, `lease_lost`).
    pub reason_code: String,
    pub exit_code: Option<i32>,
    pub timeout_seconds: Option<u64>,
    /// Pointer to persisted evidence (log excerpt id, artifact hash…).
    pub evidence_ref: Option<String>,
}
