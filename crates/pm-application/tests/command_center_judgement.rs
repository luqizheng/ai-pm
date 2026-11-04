//! S01-01 · G-01 七态展示状态模型的判定单测。
//!
//! 被测对象 = `command_center::init_judgement` 的纯函数（无 DB）。
//! 基线（用户裁决 V1.1）：
//!   - 七态为 `InitItem` 展示层状态，**不属于**任何领域实体的权威状态；
//!   - `in_progress` = 系统在工作；`waiting_for_human` = 系统已交出球；
//!     `blocked` = 外部条件阻塞、当前无可执行的人类确认动作；
//!   - 人工待办不得被普通进行中覆盖；
//!   - 不得用「存在 PENDING 记录」盲目映射等待人类——按事项的实际操作责任判定。

use pm_application::command_center::init_judgement as j;
use pm_application::command_center::init_status as st;

/* ─────────────────────────── project_goal ─────────────────────────── */

#[test]
fn goal_present_beats_everything() {
    // 已有已发布 Goal：无论还有多少活跃提案，事项完成（演进走提案管线）。
    assert_eq!(j::goal_status(true, true, true), st::COMPLETED);
    assert_eq!(j::goal_status(true, false, false), st::COMPLETED);
}

#[test]
fn goal_redline_pending_is_human_waiting_but_draft_is_not() {
    // 待人工红线确认 → 等待你确认。
    assert_eq!(j::goal_status(false, true, true), st::WAITING_FOR_HUMAN);
    // 仅有 draft / commit_pending 等系统侧提案 → 系统进行中，不是等人。
    assert_eq!(j::goal_status(false, false, true), st::IN_PROGRESS);
    // 无提案无 Goal → 未开始。
    assert_eq!(j::goal_status(false, false, false), st::NOT_STARTED);
}

/* ───────────────────────── repository_access ──────────────────────── */

#[test]
fn zero_repositories_is_legal_not_blocked() {
    // 零仓库合法（I5 §4.7）：未开始，不判失败。
    assert_eq!(j::repository_access_status(0, 0, 0), st::NOT_STARTED);
}

#[test]
fn unreachable_blocks_but_pending_is_system_work() {
    assert_eq!(j::repository_access_status(2, 1, 0), st::BLOCKED);
    assert_eq!(j::repository_access_status(2, 0, 1), st::IN_PROGRESS);
    assert_eq!(j::repository_access_status(2, 0, 0), st::COMPLETED);
}

/* ─────────────────────────── code_analysis ────────────────────────── */

#[test]
fn zero_repositories_makes_analysis_not_applicable() {
    assert_eq!(j::code_analysis_status(0, None), st::NOT_APPLICABLE);
    assert_eq!(j::code_analysis_status(0, Some("analyzing")), st::NOT_APPLICABLE);
}

#[test]
fn analysis_in_flight_is_system_progress() {
    for status in ["queued", "cloning", "reading", "analyzing"] {
        assert_eq!(
            j::code_analysis_status(1, Some(status)),
            st::IN_PROGRESS,
            "status={status}"
        );
    }
}

#[test]
fn analysis_draft_awaits_human_not_completed() {
    // G-01 行为变化：draft_ready / proposed 的下一步动作由人承担
    // （草案自述「需人工确认」），不再是 completed。
    assert_eq!(j::code_analysis_status(1, Some("draft_ready")), st::WAITING_FOR_HUMAN);
    assert_eq!(j::code_analysis_status(1, Some("proposed")), st::WAITING_FOR_HUMAN);
    assert_eq!(j::code_analysis_status(1, Some("applied")), st::COMPLETED);
}

#[test]
fn analysis_terminal_failures_block() {
    for status in ["failed", "rejected", "cancelled"] {
        assert_eq!(
            j::code_analysis_status(1, Some(status)),
            st::BLOCKED,
            "status={status}"
        );
    }
    assert_eq!(j::code_analysis_status(1, None), st::NOT_STARTED);
}

/* ────────────────────────── project_cognition ─────────────────────── */

#[test]
fn no_sessions_is_not_started() {
    assert_eq!(j::cognition_status(&[], 0), st::NOT_STARTED);
}

#[test]
fn session_human_states_win_over_everything() {
    // 会话自声明 NEEDS_USER_INPUT / REVIEW_REQUIRED → 等人，最高优先。
    let sessions = vec![("NEEDS_USER_INPUT".into(), 1), ("IN_PROGRESS".into(), 2)];
    assert_eq!(j::cognition_status(&sessions, 0), st::WAITING_FOR_HUMAN);
    let review = vec![("REVIEW_REQUIRED".into(), 1)];
    assert_eq!(j::cognition_status(&review, 5), st::WAITING_FOR_HUMAN);
}

#[test]
fn pending_findings_must_not_be_covered_by_in_progress() {
    // 验收规则：人工待办不能被普通进行中覆盖——
    // 会话仍在分析，但已有待确认发现 → 等待你确认。
    let sessions = vec![("IN_PROGRESS".into(), 1)];
    assert_eq!(j::cognition_status(&sessions, 2), st::WAITING_FOR_HUMAN);
}

#[test]
fn open_session_without_pending_findings_is_progress() {
    let sessions = vec![("IN_PROGRESS".into(), 1), ("PAUSED".into(), 1)];
    assert_eq!(j::cognition_status(&sessions, 0), st::IN_PROGRESS);
}

#[test]
fn completed_sessions_with_pending_findings_still_wait() {
    // G-06：会话完成 ≠ 认知完成——有 PENDING 发现就得等人。
    let sessions = vec![("COMPLETED".into(), 3)];
    assert_eq!(j::cognition_status(&sessions, 1), st::WAITING_FOR_HUMAN);
    assert_eq!(j::cognition_status(&sessions, 0), st::COMPLETED);
}

/* ────────────────────────── dev_environment ───────────────────────── */

#[test]
fn environment_states() {
    assert_eq!(j::environment_status(0, 0, 0, 0), st::NOT_STARTED);
    assert_eq!(j::environment_status(2, 0, 1, 0), st::BLOCKED);
    // 从未自检 → 证据不足（待判定），不臆断为等待人类或完成。
    assert_eq!(j::environment_status(2, 1, 0, 1), st::UNDETERMINED);
    assert_eq!(j::environment_status(2, 1, 0, 0), st::COMPLETED);
    assert_eq!(j::environment_status(1, 0, 0, 0), st::IN_PROGRESS);
}

/* ─────────────────────────── human_actions ────────────────────────── */

#[test]
fn action_backlog_is_human_waiting_not_progress() {
    assert_eq!(j::human_actions_status(0), st::COMPLETED);
    assert_eq!(j::human_actions_status(3), st::WAITING_FOR_HUMAN);
}
