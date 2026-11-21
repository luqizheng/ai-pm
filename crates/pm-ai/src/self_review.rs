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
//! `blast_radius` is a batch-level parameter: M1 passes the saturating sum of
//! `ProjectCommand::impact_count()` across all proposals. Batch commands contribute
//! their collection cardinality; single-object commands contribute one. The gate
//! receives only this aggregate meta-fact and never reads command payload content.
//!
//! Everything here is a pure function: no IO, no reads of command payloads —
//! the gate routes on proposal meta-facts only, so model-controlled bytes
//! have zero influence on routing (the injection surface is empty by
//! construction, mirroring `PolicyContext`).

use pm_application::PolicyEngine;
use pm_domain::{Confidence, Proposal};

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
    use pm_domain::import::TaskDraft;
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

    fn create_tasks(count: usize) -> ProjectCommand {
        let task = TaskDraft {
            key: "T".to_string(),
            title: "Task".to_string(),
            description: String::new(),
            priority: "normal".to_string(),
            milestone_key: None,
            requirement_key: None,
            assignee_role: None,
            confidence: Confidence::High,
            evidence: Vec::new(),
        };
        ProjectCommand::CreateTasks {
            tasks: vec![task; count],
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
    fn create_tasks_batch_cardinality_drives_blast_radius_threshold() {
        let policy = PolicyEngine::for_tests();

        let ten = create_tasks(10);
        let at_threshold =
            review_proposals(&policy, &[proposal(ten.clone(), Confidence::High)], ten.impact_count());
        assert_eq!(at_threshold, vec![ProposalOutcome::Auto]);

        let eleven = create_tasks(11);
        let above_threshold = review_proposals(
            &policy,
            &[proposal(eleven.clone(), Confidence::High)],
            eleven.impact_count(),
        );
        assert_eq!(
            above_threshold,
            vec![ProposalOutcome::Escalate {
                reason: "blast_radius_above_10".to_string(),
            }]
        );
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
