//! Deterministic self-review gate (Task 13, V2 M1 vertical slice).
//!
//! Routes each AI proposal to one of `Auto`, `RequirePreConfirmation` or
//! `Escalate` before the executor sees it. The judgment order is FIXED
//! (controller ruling) and short-circuits per proposal:
//!
//! 1. **Red line** — `policy.is_redline(cmd)` →
//!    `RequirePreConfirmation { role: policy.required_role(cmd).unwrap() }`.
//!    A red line always requires pre-confirmation; neither confidence nor
//!    blast radius may weaken it.
//! 2. **Low confidence** — `proposal.confidence == Confidence::Low` →
//!    `Escalate { reason: "confidence_low" }` (same string as
//!    `PolicyEngine`'s `EscalateCondition::reason()`).
//! 3. **Blast radius** — `blast_radius > policy.escalate_threshold()`
//!    (STRICTLY greater; the yaml `blast_radius_above: 10` means 11 triggers
//!    and 10 does not) →
//!    `Escalate { reason: format!("blast_radius_above_{threshold}") }`.
//! 4. Otherwise → `Auto`.
//!
//! `blast_radius` is a batch-level parameter — the orchestrator passes
//! [`round_blast_radius`]: the saturating sum of every proposal's actual
//! affected-item count (`ProjectCommand::item_count`, PA07-IMP-002 §4 P2),
//! NOT the proposal count. Batch commands contribute their collection
//! cardinality; single-object commands contribute one. It is applied to every
//! non-red-line proposal in the round, so splitting one 11-task `CreateTasks`
//! into eleven single-task proposals cannot slip under the threshold: the
//! round still sums to 11.
//!
//! Everything here is a pure function: no IO — the gate routes on proposal
//! meta-facts only (plus the payload-derived item count of the command
//! enum itself, which the model cannot inflate: lengths are structural),
//! so model-controlled bytes have zero influence on routing (the injection
//! surface is empty by construction, mirroring `PolicyContext`).

use pm_application::PolicyEngine;
use pm_domain::{Confidence, Proposal};

/// 整轮提案的批量影响范围（PA07-IMP-002 §4 P2）：按每条命令**实际影响的
/// 权威对象条数**累计（`ProjectCommand::item_count`），而非提案条数。
///
/// - 单条 `CreateTasks` 含 11 个任务 → radius=11（旧算法按提案数计 1，
///   批量闸永不触发）；
/// - 拆成 11 条单任务提案 → 整轮累计仍为 11，阈值无法被绕过；
/// - `CreateMilestones` / `CreateRequirements` / `WriteDocuments` 同理；
/// - 单对象命令（`create_task` / `update_task_status` / …）恒为 1。
pub fn round_blast_radius(proposals: &[Proposal]) -> usize {
    proposals
        .iter()
        .map(|proposal| proposal.command.item_count())
        .fold(0usize, |total, count| total.saturating_add(count))
}

/// Per-proposal routing verdict produced by [`review_proposals`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalOutcome {
    /// No red line, no escalate condition: may be auto-applied.
    Auto,
    /// Red line: a human holding `role` must pre-confirm before execution.
    RequirePreConfirmation { role: String },
    /// Escalate condition hit: human review before execution, with a stable
    /// machine-readable `reason`.
    Escalate { reason: String },
}

/// Reviews every proposal against the policy in the fixed order documented at
/// module level. Output position corresponds one-to-one with input position.
pub fn review_proposals(
    policy: &PolicyEngine,
    proposals: &[Proposal],
    blast_radius: usize,
) -> Vec<ProposalOutcome> {
    let threshold = policy.escalate_threshold();
    proposals
        .iter()
        .map(|proposal| {
            // 1. Red line: always pre-confirmation, never weakened by
            //    confidence or blast radius.
            if policy.is_redline(&proposal.command) {
                ProposalOutcome::RequirePreConfirmation {
                    role: policy
                        .required_role(&proposal.command)
                        .expect("is_redline implies required_role is present"),
                }
            // 2. Low confidence.
            } else if proposal.confidence == Confidence::Low {
                ProposalOutcome::Escalate {
                    reason: "confidence_low".to_string(),
                }
            // 3. Blast radius strictly above the threshold.
            } else if blast_radius > threshold {
                ProposalOutcome::Escalate {
                    reason: format!("blast_radius_above_{threshold}"),
                }
            // 4. Default.
            } else {
                ProposalOutcome::Auto
            }
        })
        .collect()
}

/// Serializes outcomes into the shape stored in `analysis_run.self_review`
/// (Task 14): `{"outcomes": [{"proposal_seq": n, "outcome": "...", ...}]}`.
pub fn self_review_json(outcomes: &[ProposalOutcome]) -> serde_json::Value {
    let entries: Vec<serde_json::Value> = outcomes
        .iter()
        .enumerate()
        .map(|(seq, outcome)| match outcome {
            ProposalOutcome::Auto => serde_json::json!({
                "proposal_seq": seq,
                "outcome": "auto",
            }),
            ProposalOutcome::RequirePreConfirmation { role } => serde_json::json!({
                "proposal_seq": seq,
                "outcome": "require_pre_confirmation",
                "role": role,
            }),
            ProposalOutcome::Escalate { reason } => serde_json::json!({
                "proposal_seq": seq,
                "outcome": "escalate",
                "reason": reason,
            }),
        })
        .collect();
    serde_json::json!({ "outcomes": entries })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pm_domain::{ProjectCommand, RiskLevel, TaskStatus};

    fn proposal(command: ProjectCommand, confidence: Confidence) -> Proposal {
        Proposal {
            command,
            rationale: "测试提案".to_string(),
            risk_level: RiskLevel::Low,
            confidence,
            evidence: Vec::new(),
        }
    }

    fn create_task() -> ProjectCommand {
        ProjectCommand::CreateTask {
            project_id: uuid::Uuid::nil(),
            title: "缓存调研".to_string(),
            description: "对比本地缓存与集中式缓存".to_string(),
            assignee_id: None,
        }
    }

    fn update_task_status() -> ProjectCommand {
        ProjectCommand::UpdateTaskStatus {
            task_id: uuid::Uuid::nil(),
            expected_revision: 7,
            new_status: TaskStatus::Blocked,
        }
    }

    fn sign_acceptance() -> ProjectCommand {
        ProjectCommand::SignAcceptance {
            project_id: uuid::Uuid::nil(),
            milestone_id: uuid::Uuid::nil(),
            expected_revision: 4,
        }
    }

    fn adjust_milestone_plan() -> ProjectCommand {
        ProjectCommand::AdjustMilestonePlan {
            project_id: uuid::Uuid::nil(),
            milestone_id: uuid::Uuid::nil(),
            new_planned_at: None,
            expected_revision: 4,
        }
    }

    fn batch_create_tasks(n: usize) -> ProjectCommand {
        use pm_domain::import::TaskDraft;
        ProjectCommand::CreateTasks {
            tasks: (0..n)
                .map(|i| TaskDraft {
                    key: format!("T{i}"),
                    title: format!("任务 {i}"),
                    description: String::new(),
                    priority: "high".to_string(),
                    milestone_key: None,
                    requirement_key: None,
                    assignee_role: None,
                    confidence: Confidence::High,
                    evidence: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn round_blast_radius_counts_items_not_proposals() {
        // 一条 CreateTasks 含 11 任务：radius=11（旧算法 proposals.len()=1）。
        let eleven = vec![proposal(batch_create_tasks(11), Confidence::High)];
        assert_eq!(round_blast_radius(&eleven), 11);
        // 10 个任务不超过阈值（10 > 10 为假）。
        let ten = vec![proposal(batch_create_tasks(10), Confidence::High)];
        assert_eq!(round_blast_radius(&ten), 10);
    }

    #[test]
    fn round_blast_radius_accumulates_across_proposals_no_split_bypass() {
        // 拆分绕过：11 条单对象提案累计=11，仍超阈值。
        let split: Vec<Proposal> = (0..11)
            .map(|_| proposal(create_task(), Confidence::High))
            .collect();
        assert_eq!(round_blast_radius(&split), 11);
        // 混合累计：CreateTasks(6) + CreateTasks(5) = 11。
        let mixed = vec![
            proposal(batch_create_tasks(6), Confidence::High),
            proposal(batch_create_tasks(5), Confidence::High),
        ];
        assert_eq!(round_blast_radius(&mixed), 11);
        // 10 条单对象提案累计=10，不超阈值。
        let calm: Vec<Proposal> = (0..10)
            .map(|_| proposal(create_task(), Confidence::High))
            .collect();
        assert_eq!(round_blast_radius(&calm), 10);
    }

    #[test]
    fn round_blast_radius_escalates_batch_create_tasks_above_threshold() {
        let policy = PolicyEngine::for_tests();
        // 11 任务的一揽子提案 → Escalate（blast_radius_above_10）。
        let eleven = vec![proposal(batch_create_tasks(11), Confidence::High)];
        let outcomes = review_proposals(&policy, &eleven, round_blast_radius(&eleven));
        assert_eq!(
            outcomes,
            vec![ProposalOutcome::Escalate {
                reason: "blast_radius_above_10".to_string()
            }]
        );
        // 10 任务的一揽子提案 → Auto（批量建任务非红线，10 不严格大于 10）。
        let ten = vec![proposal(batch_create_tasks(10), Confidence::High)];
        let outcomes = review_proposals(&policy, &ten, round_blast_radius(&ten));
        assert_eq!(outcomes, vec![ProposalOutcome::Auto]);
    }

    #[test]
    fn sign_acceptance_red_line_requires_pre_confirmation_with_authorized_acceptor() {
        let policy = PolicyEngine::for_tests();
        let outcomes =
            review_proposals(&policy, &[proposal(sign_acceptance(), Confidence::High)], 1);
        assert_eq!(
            outcomes,
            vec![ProposalOutcome::RequirePreConfirmation {
                role: "authorized_acceptor".to_string()
            }]
        );
    }

    #[test]
    fn low_confidence_non_red_line_escalates_with_confidence_low_reason() {
        let policy = PolicyEngine::for_tests();
        let outcomes = review_proposals(
            &policy,
            &[proposal(update_task_status(), Confidence::Low)],
            1,
        );
        assert_eq!(
            outcomes,
            vec![ProposalOutcome::Escalate {
                reason: "confidence_low".to_string()
            }]
        );
    }

    #[test]
    fn blast_radius_strictly_above_threshold_escalates_at_threshold_does_not() {
        let policy = PolicyEngine::for_tests();
        // 11 > 10: escalates with the dynamic reason string.
        let above = review_proposals(&policy, &[proposal(create_task(), Confidence::High)], 11);
        assert_eq!(
            above,
            vec![ProposalOutcome::Escalate {
                reason: "blast_radius_above_10".to_string()
            }]
        );
        // 10 is NOT strictly greater than 10: no escalation.
        let at = review_proposals(&policy, &[proposal(create_task(), Confidence::High)], 10);
        assert_eq!(at, vec![ProposalOutcome::Auto]);
    }

    #[test]
    fn red_line_wins_over_low_confidence_and_large_blast_radius() {
        let policy = PolicyEngine::for_tests();
        let outcomes =
            review_proposals(&policy, &[proposal(sign_acceptance(), Confidence::Low)], 11);
        assert_eq!(
            outcomes,
            vec![ProposalOutcome::RequirePreConfirmation {
                role: "authorized_acceptor".to_string()
            }]
        );
    }

    #[test]
    fn mixed_list_preserves_input_order_one_to_one() {
        let policy = PolicyEngine::for_tests();
        let proposals = vec![
            proposal(create_task(), Confidence::High),
            proposal(sign_acceptance(), Confidence::High),
            proposal(update_task_status(), Confidence::Low),
        ];
        let outcomes = review_proposals(&policy, &proposals, 3);
        assert_eq!(outcomes.len(), proposals.len());
        assert_eq!(outcomes[0], ProposalOutcome::Auto);
        assert_eq!(
            outcomes[1],
            ProposalOutcome::RequirePreConfirmation {
                role: "authorized_acceptor".to_string()
            }
        );
        assert_eq!(
            outcomes[2],
            ProposalOutcome::Escalate {
                reason: "confidence_low".to_string()
            }
        );
    }

    #[test]
    fn empty_input_yields_empty_vec_and_empty_outcomes_json() {
        let policy = PolicyEngine::for_tests();
        let outcomes = review_proposals(&policy, &[], 0);
        assert!(outcomes.is_empty());
        assert_eq!(
            self_review_json(&outcomes),
            serde_json::json!({ "outcomes": [] })
        );
    }

    #[test]
    fn self_review_json_has_expected_shape_for_every_outcome_kind() {
        let policy = PolicyEngine::for_tests();
        // blast_radius is a batch parameter: one call cannot mix an Auto with
        // a blast-radius Escalate, so cover all four entry shapes via two calls.
        let calm = review_proposals(
            &policy,
            &[
                // seq 0: plain auto
                proposal(create_task(), Confidence::High),
                // seq 1: red line
                proposal(sign_acceptance(), Confidence::High),
                // seq 2: low confidence
                proposal(update_task_status(), Confidence::Low),
            ],
            3,
        );
        // seq 3: high confidence + blast_radius 11 → rule 3
        let hot = review_proposals(
            &policy,
            &[proposal(adjust_milestone_plan(), Confidence::High)],
            11,
        );
        let outcomes = [calm, hot].concat();
        assert_eq!(
            self_review_json(&outcomes),
            serde_json::json!({
                "outcomes": [
                    { "proposal_seq": 0, "outcome": "auto" },
                    {
                        "proposal_seq": 1,
                        "outcome": "require_pre_confirmation",
                        "role": "authorized_acceptor"
                    },
                    {
                        "proposal_seq": 2,
                        "outcome": "escalate",
                        "reason": "confidence_low"
                    },
                    {
                        "proposal_seq": 3,
                        "outcome": "escalate",
                        "reason": "blast_radius_above_10"
                    }
                ]
            })
        );
    }
}
