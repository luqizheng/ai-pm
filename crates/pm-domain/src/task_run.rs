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

/// Task-level retry budget (§19.4)。默认值是 P2 起始配置，不是所有任务的固定
/// 标准——大型编译/跨仓重构/架构任务应采用不同预算（由调用方按任务类型给定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// 一个执行阶段最多尝试次数，**包含首次执行**。
    pub max_run_attempts: u32,
    /// 连续失败达到该阈值即停止自动重试。
    pub max_consecutive_failures: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_run_attempts: 3,
            max_consecutive_failures: 3,
        }
    }
}

/// Whether automatic retry may continue after one more Run outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeVerdict {
    Continue,
    StopRetry,
}

/// Task-level retry counters (§19.5)。`last_failure_category` 由调用方从
/// [`FailureRecord`] 回填——计数器只管计数，不解释失败。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RetryCounters {
    /// Attempts in the current execution stage (stage switch reopens the window).
    pub attempt_count: u32,
    /// Consecutive failed Runs; reset to zero by one successful Run (§19.4).
    pub consecutive_failure_count: u32,
    pub last_failure_category: Option<FailureCategory>,
    /// Latched once either threshold is hit; drives `Running → Failed` (B4-C).
    pub retry_exhausted: bool,
}

impl RetryCounters {
    /// Registers one Run outcome against the retry budget.
    ///
    /// Rules (§19.4): a successful Run zeroes the consecutive-failure series;
    /// the total-attempt window restarts when the execution stage changes
    /// (`same_stage == false`); either threshold reaching its limit stops
    /// automatic retry — the two limits are independent judges, not one.
    pub fn register_outcome(
        &mut self,
        outcome: RunOutcome,
        same_stage: bool,
        policy: &RetryPolicy,
    ) -> OutcomeVerdict {
        match outcome {
            RunOutcome::Succeeded => {
                self.consecutive_failure_count = 0;
                OutcomeVerdict::Continue
            }
            _ => {
                if same_stage {
                    self.attempt_count += 1;
                } else {
                    self.attempt_count = 1;
                }
                self.consecutive_failure_count += 1;
                if self.attempt_count >= policy.max_run_attempts
                    || self.consecutive_failure_count >= policy.max_consecutive_failures
                {
                    self.retry_exhausted = true;
                    OutcomeVerdict::StopRetry
                } else {
                    OutcomeVerdict::Continue
                }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> RetryPolicy {
        RetryPolicy::default()
    }

    #[test]
    fn defaults_are_three_and_three() {
        // §19.4 P2 起始配置：max_run_attempts = 3（含首次）、max_consecutive_failures = 3。
        let p = policy();
        assert_eq!(p.max_run_attempts, 3);
        assert_eq!(p.max_consecutive_failures, 3);
    }

    #[test]
    fn consecutive_failures_reset_on_success() {
        // §19.4：发生一次成功的 Run 后，连续失败计数归零。
        let mut counters = RetryCounters::default();
        let p = policy();
        counters.register_outcome(RunOutcome::Failed, true, &p);
        counters.register_outcome(RunOutcome::Succeeded, true, &p);
        counters.register_outcome(RunOutcome::Failed, true, &p);
        assert_eq!(counters.consecutive_failure_count, 1);
    }

    #[test]
    fn stage_change_restarts_consecutive_series() {
        // §19.4 例：第 1 次网络失败、第 2 次编译成功、第 3 次测试失败——
        // 不能把这三次机械地认定为连续失败 3 次。
        let mut counters = RetryCounters::default();
        let p = policy();
        assert_eq!(
            counters.register_outcome(RunOutcome::Failed, true, &p),
            OutcomeVerdict::Continue
        );
        assert_eq!(
            counters.register_outcome(RunOutcome::Succeeded, true, &p),
            OutcomeVerdict::Continue
        );
        assert_eq!(
            counters.register_outcome(RunOutcome::Failed, false, &p),
            OutcomeVerdict::Continue
        );
        assert_eq!(counters.consecutive_failure_count, 1);
    }

    #[test]
    fn attempts_reach_max_stops_retry() {
        let mut counters = RetryCounters::default();
        let p = policy();
        assert_eq!(
            counters.register_outcome(RunOutcome::Failed, true, &p),
            OutcomeVerdict::Continue
        );
        assert_eq!(
            counters.register_outcome(RunOutcome::Failed, true, &p),
            OutcomeVerdict::Continue
        );
        assert_eq!(
            counters.register_outcome(RunOutcome::Failed, true, &p),
            OutcomeVerdict::StopRetry
        );
        assert!(counters.retry_exhausted);
        assert_eq!(counters.attempt_count, 3);
    }

    #[test]
    fn consecutive_failures_alone_stop_retry() {
        // 连续失败阈值独立于总尝试窗口：拉高 max_run_attempts 后，连续失败
        // 达到阈值仍须停止（两个判定器互不隶属，§19.4）。
        let mut counters = RetryCounters::default();
        let p = RetryPolicy {
            max_run_attempts: 10,
            max_consecutive_failures: 3,
        };
        counters.register_outcome(RunOutcome::TimedOut, true, &p);
        counters.register_outcome(RunOutcome::TimedOut, true, &p);
        assert_eq!(
            counters.register_outcome(RunOutcome::TimedOut, true, &p),
            OutcomeVerdict::StopRetry
        );
        assert_eq!(counters.attempt_count, 3);
    }

    #[test]
    fn stage_switch_resets_attempt_window() {
        // §19.4：总尝试次数是否重置，由执行阶段是否发生变化决定。
        let mut counters = RetryCounters::default();
        let p = policy();
        counters.register_outcome(RunOutcome::Failed, true, &p);
        counters.register_outcome(RunOutcome::Failed, true, &p);
        let verdict = counters.register_outcome(RunOutcome::Failed, false, &p);
        assert_eq!(verdict, OutcomeVerdict::StopRetry); // 连续失败达阈值兜底
        assert_eq!(counters.attempt_count, 1); // 但总尝试窗口已重开
    }

    #[test]
    fn success_does_not_stop_retry() {
        let mut counters = RetryCounters::default();
        let p = policy();
        let verdict = counters.register_outcome(RunOutcome::Succeeded, true, &p);
        assert_eq!(verdict, OutcomeVerdict::Continue);
        assert!(!counters.retry_exhausted);
    }
}
