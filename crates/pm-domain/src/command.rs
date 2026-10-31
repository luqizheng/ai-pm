//! Commands a principal (usually the AI lead) proposes against a project.

use serde::{Deserialize, Serialize};

use crate::ids::{AuditId, MilestoneId, PrincipalId, ProjectId, RequirementId, RevisionId, TaskId};
use crate::task::TaskStatus;

/// A single state-changing operation against a project.
///
/// Serialized adjacently tagged: `{"kind": "<snake_name>", "data": {...}}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum ProjectCommand {
    CreateTask {
        project_id: ProjectId,
        title: String,
        description: String,
        assignee_id: Option<PrincipalId>,
    },
    UpdateTaskStatus {
        task_id: TaskId,
        expected_revision: i64,
        new_status: TaskStatus,
    },
    ApproveRequirementRevision {
        project_id: ProjectId,
        requirement_id: RequirementId,
        revision_id: RevisionId,
    },
    AdjustMilestonePlan {
        project_id: ProjectId,
        milestone_id: MilestoneId,
        new_planned_at: Option<chrono::DateTime<chrono::Utc>>,
        expected_revision: i64,
    },
    /// Red line: never executed without pre-confirmation.
    SignAcceptance {
        project_id: ProjectId,
        milestone_id: MilestoneId,
        expected_revision: i64,
    },
    /// Red line; M1 has no handler.
    ChangePolicy {
        project_id: ProjectId,
        new_policy: serde_json::Value,
    },
    RollbackCommand {
        project_id: ProjectId,
        target_audit_id: AuditId,
        reason: String,
    },
}

impl ProjectCommand {
    pub fn kind(&self) -> &'static str {
        match self {
            ProjectCommand::CreateTask { .. } => "create_task",
            ProjectCommand::UpdateTaskStatus { .. } => "update_task_status",
            ProjectCommand::ApproveRequirementRevision { .. } => "approve_requirement_revision",
            ProjectCommand::AdjustMilestonePlan { .. } => "adjust_milestone_plan",
            ProjectCommand::SignAcceptance { .. } => "sign_acceptance",
            ProjectCommand::ChangePolicy { .. } => "change_policy",
            ProjectCommand::RollbackCommand { .. } => "rollback_command",
        }
    }

    pub fn reversibility(&self) -> Reversibility {
        match self {
            ProjectCommand::SignAcceptance { .. } | ProjectCommand::ChangePolicy { .. } => {
                Reversibility::Irreversible
            }
            ProjectCommand::RollbackCommand { .. } => Reversibility::Reversible,
            _ => Reversibility::Compensable,
        }
    }
}

/// How a command may be undone after execution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Reversibility {
    Reversible,
    Compensable,
    Irreversible,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{AuditId, MilestoneId, PrincipalId, ProjectId, RequirementId, RevisionId, TaskId};

    fn sample_commands() -> Vec<ProjectCommand> {
        let planned_at: chrono::DateTime<chrono::Utc> =
            "2026-06-01T08:30:00Z".parse().expect("valid rfc3339");
        vec![
            ProjectCommand::CreateTask {
                project_id: ProjectId::nil(),
                title: "Implement login".to_string(),
                description: "Add login page with session cookie".to_string(),
                assignee_id: Some(PrincipalId::nil()),
            },
            ProjectCommand::UpdateTaskStatus {
                task_id: TaskId::nil(),
                expected_revision: 3,
                new_status: TaskStatus::Running,
            },
            ProjectCommand::ApproveRequirementRevision {
                project_id: ProjectId::nil(),
                requirement_id: RequirementId::nil(),
                revision_id: RevisionId::nil(),
            },
            ProjectCommand::AdjustMilestonePlan {
                project_id: ProjectId::nil(),
                milestone_id: MilestoneId::nil(),
                new_planned_at: Some(planned_at),
                expected_revision: 7,
            },
            ProjectCommand::SignAcceptance {
                project_id: ProjectId::nil(),
                milestone_id: MilestoneId::nil(),
                expected_revision: 1,
            },
            ProjectCommand::ChangePolicy {
                project_id: ProjectId::nil(),
                new_policy: serde_json::json!({ "automation_mode": "controlled" }),
            },
            ProjectCommand::RollbackCommand {
                project_id: ProjectId::nil(),
                target_audit_id: AuditId::nil(),
                reason: "signed in error".to_string(),
            },
        ]
    }

    #[test]
    fn project_command_json_roundtrip_for_every_variant() {
        for command in sample_commands() {
            let json = serde_json::to_string(&command).unwrap();
            let back: ProjectCommand = serde_json::from_str(&json).unwrap();
            assert_eq!(back, command);
        }
    }

    #[test]
    fn serialized_command_is_adjacently_tagged_with_kind_and_data() {
        let json = serde_json::to_value(&ProjectCommand::SignAcceptance {
            project_id: ProjectId::nil(),
            milestone_id: MilestoneId::nil(),
            expected_revision: 1,
        })
        .unwrap();
        assert_eq!(json["kind"], "sign_acceptance");
        assert_eq!(json["data"]["expected_revision"], 1);
    }

    #[test]
    fn kind_returns_snake_case_name_for_every_command() {
        let kinds: Vec<&'static str> = sample_commands().iter().map(|c| c.kind()).collect();
        assert_eq!(
            kinds,
            vec![
                "create_task",
                "update_task_status",
                "approve_requirement_revision",
                "adjust_milestone_plan",
                "sign_acceptance",
                "change_policy",
                "rollback_command",
            ]
        );
    }

    #[test]
    fn reversibility_maps_red_lines_to_irreversible_and_rollback_to_reversible() {
        let by_kind: std::collections::HashMap<&'static str, Reversibility> = sample_commands()
            .into_iter()
            .map(|c| (c.kind(), c.reversibility()))
            .collect();
        assert_eq!(by_kind["sign_acceptance"], Reversibility::Irreversible);
        assert_eq!(by_kind["change_policy"], Reversibility::Irreversible);
        assert_eq!(by_kind["rollback_command"], Reversibility::Reversible);
        assert_eq!(by_kind["create_task"], Reversibility::Compensable);
        assert_eq!(by_kind["update_task_status"], Reversibility::Compensable);
        assert_eq!(
            by_kind["approve_requirement_revision"],
            Reversibility::Compensable
        );
        assert_eq!(by_kind["adjust_milestone_plan"], Reversibility::Compensable);
    }
}
