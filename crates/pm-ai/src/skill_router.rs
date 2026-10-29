//! Routes inbound event types to the AI skills that must analyze them.

/// Identifier of one analysis skill; also the element type of the route table.
pub type SkillId = &'static str;

/// Returns the skills to run for an inbound event type, in execution order.
/// Unknown event types route to nothing; the orchestrator then concludes
/// `completed_no_change`.
pub fn route(event_type: &str) -> Vec<SkillId> {
    match event_type {
        "requirement_submitted" => vec!["requirement_analysis", "risk_detection"],
        "technical_design_submitted" => vec![
            "requirement_analysis",
            "task_analysis",
            "architecture_consistency",
        ],
        "work_result_submitted" => vec!["task_analysis", "requirement_analysis"],
        "milestone_check" => vec![
            "scheduling_orchestration",
            "risk_detection",
            "task_analysis",
        ],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_requirement_submitted_to_analysis_and_risk_detection() {
        assert_eq!(
            route("requirement_submitted"),
            vec!["requirement_analysis", "risk_detection"]
        );
    }

    #[test]
    fn routes_technical_design_submitted_to_three_skills() {
        assert_eq!(
            route("technical_design_submitted"),
            vec![
                "requirement_analysis",
                "task_analysis",
                "architecture_consistency"
            ]
        );
    }

    #[test]
    fn routes_work_result_submitted_to_task_then_requirement_analysis() {
        assert_eq!(
            route("work_result_submitted"),
            vec!["task_analysis", "requirement_analysis"]
        );
    }

    #[test]
    fn routes_milestone_check_to_scheduling_risk_and_task_skills() {
        assert_eq!(
            route("milestone_check"),
            vec![
                "scheduling_orchestration",
                "risk_detection",
                "task_analysis"
            ]
        );
    }

    #[test]
    fn routes_unknown_event_types_to_no_skills() {
        assert!(route("something_else").is_empty());
        assert!(route("").is_empty());
    }
}
