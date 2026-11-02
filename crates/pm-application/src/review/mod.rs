//! Slice D5：Review 编排、失败判定与知识回写（`docs/ai-pm-slice-d5-review-v1.md`）。
//!
//! D5 是**治理闭环**，不是更复杂的执行引擎：
//!
//! - D1 决定能不能执行；D3 负责实际执行；D4 记录执行产生的事实；
//!   **D5 只判断这些事实是否满足任务完成条件**。
//!
//! 三层边界不可越：
//!
//! - Review Run 的 `executor_kind='platform'` 仅做确定性校验，不碰工作区
//!   （§0.2 不变量 1）。
//! - 失败判定服务只读 `task_run_failure` 不可变表，不回写交付物（§0.2 不变量 2）。
//! - 审批裁决只写 `review_approval` + 状态迁移，不回写交付物（§0.2 不变量 3）。
//! - 知识回写只触发 `draft` 提案，不执行知识域状态机（§0.2 不变量 4）。
//! - **Review 失败 ≠ Run 失败 ≠ Task 失败**（§0.2 不变量 5）。
//!
//! 两拍编排（§3.1）：
//!
//! ```text
//! DeliverySubmitted 事件 → queue::enqueue_review（轻拍：B1 守卫 + 建 review run）
//! worker review 领取循环 → worker::review_step（重拍：管线 + 落库 + 推进）
//! ```
//!
//! 失败判定与 review 编排同进程并列（§6.2），扫描谓词 `run_kind='delivery'`
//! 把两条恢复链结构性隔离开。

pub mod anchor;
pub mod collect;
pub mod decide;
pub mod failure;
pub mod knowledge;
pub mod pipeline;
pub mod queue;
pub mod worker;

pub use anchor::{
    CriterionAnchor, RepositoryAnchor, input_manifest_hash, review_policy_version,
    reviewed_manifest_hash,
};
pub use collect::{collect_review_input, detect_anchor_drift, CollectedReview};
pub use decide::{
    advance_after_review, approval_kind, decide_review_approval, load_completion_gate,
    AdvanceOutcome, ApprovalError, ApprovalOutcome, CompletionGate, ReviewDecisionRequest,
};
pub use failure::{judge_delivery_failures, judge_failures_once, FailureJudgeReport};
pub use knowledge::{render_markdown, KnowledgeWriteback};
pub use pipeline::{
    evaluate, ArtifactProbe, CriterionOutcome, CriterionSnapshot, CriterionVerdict, ReviewFinding,
    ReviewInput, ReviewOutcome, ReviewVerdict, Severity, TestEvidence,
};
pub use queue::{
    enqueue_review, load_reviewed_delivery, EnqueueOutcome, ReviewError, ReviewedDelivery,
    PLATFORM_EXECUTOR, REVIEWER_ROLE,
};
pub use worker::{
    review_step, run_review_loop, ReviewConfig, ReviewStep, REVIEW_IDLE_POLL_MS,
};
