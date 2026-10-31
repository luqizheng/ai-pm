//! Commands a principal (usually the AI lead) proposes against a project.

use serde::{Deserialize, Serialize};

use crate::ids::{AuditId, MilestoneId, PrincipalId, ProjectId, RequirementId, RevisionId, TaskId};
use crate::import::{DocumentDraft, MilestoneDraft, RequirementDraft, TaskDraft};
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
    /// 冷启动批量建里程碑；项目取自执行上下文。
    CreateMilestones {
        milestones: Vec<MilestoneDraft>,
    },
    /// 冷启动批量建需求（落 `draft`，不自动定稿）。
    CreateRequirements {
        requirements: Vec<RequirementDraft>,
    },
    /// 冷启动批量建任务；`assignee_role` 解析成对应 Agent principal。
    CreateTasks {
        tasks: Vec<TaskDraft>,
    },
    /// 冷启动批量写知识文档（文件 + git commit + 两表）。
    WriteDocuments {
        documents: Vec<DocumentDraft>,
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
            ProjectCommand::CreateMilestones { .. } => "create_milestones",
            ProjectCommand::CreateRequirements { .. } => "create_requirements",
            ProjectCommand::CreateTasks { .. } => "create_tasks",
            ProjectCommand::WriteDocuments { .. } => "write_documents",
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
            ProjectCommand::CreateMilestones {
                milestones: vec![MilestoneDraft {
                    key: "M1".to_string(),
                    name: "核心地形系统".to_string(),
                    planned_at: None,
                }],
            },
            ProjectCommand::CreateRequirements {
                requirements: vec![RequirementDraft {
                    key: "R1".to_string(),
                    code: "REQ-001".to_string(),
                    title: "地形分块加载".to_string(),
                    markdown: "支持分块与 LOD。".to_string(),
                    priority: "high".to_string(),
                    confidence: crate::proposal::Confidence::High,
                    evidence: Vec::new(),
                }],
            },
            ProjectCommand::CreateTasks {
                tasks: vec![TaskDraft {
                    key: "T1".to_string(),
                    title: "实现分块加载器".to_string(),
                    description: String::new(),
                    priority: "high".to_string(),
                    milestone_key: Some("M1".to_string()),
                    requirement_key: Some("R1".to_string()),
                    assignee_role: Some(crate::import::AssigneeRole::Developer),
                    confidence: crate::proposal::Confidence::High,
                    evidence: Vec::new(),
                }],
            },
            ProjectCommand::WriteDocuments {
                documents: vec![DocumentDraft {
                    doc_key: "architecture".to_string(),
                    doc_type: "architecture".to_string(),
                    markdown: "# 架构".to_string(),
                    change_summary: None,
                }],
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
                "create_milestones",
                "create_requirements",
                "create_tasks",
                "write_documents",
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
