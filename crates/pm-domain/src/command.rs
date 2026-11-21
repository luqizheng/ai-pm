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
    CreateMilestones { milestones: Vec<MilestoneDraft> },
    /// 冷启动批量建需求（落 `draft`，不自动定稿）。
    CreateRequirements { requirements: Vec<RequirementDraft> },
    /// 冷启动批量建任务；`assignee_role` 解析成对应 Agent principal。
    CreateTasks { tasks: Vec<TaskDraft> },
    /// 冷启动批量写知识文档（文件 + git commit + 两表）。
    WriteDocuments { documents: Vec<DocumentDraft> },
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

    /// 命令实际影响的权威对象条数（PA07-IMP-002 §4 P2）：批量命令取载荷
    /// 长度（`CreateTasks`=任务数、`CreateMilestones`=里程碑数……），
    /// 单对象命令恒为 1。批量保护闸（self_review / policy 的
    /// `blast_radius`）按本值累计整轮提案——单条 `CreateTasks` 含 11 个
    /// 任务时 radius=11 而非 1；把同一批工作拆成多条提案，整轮累计不变，
    /// 无法绕过阈值。
    pub fn item_count(&self) -> usize {
        match self {
            ProjectCommand::CreateMilestones { milestones } => milestones.len(),
            ProjectCommand::CreateRequirements { requirements } => requirements.len(),
            ProjectCommand::CreateTasks { tasks } => tasks.len(),
            ProjectCommand::WriteDocuments { documents } => documents.len(),
            _ => 1,
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
    use crate::ids::{
        AuditId, MilestoneId, PrincipalId, ProjectId, RequirementId, RevisionId, TaskId,
    };

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

    fn task_draft(key: &str) -> TaskDraft {
        TaskDraft {
            key: key.to_string(),
            title: format!("任务 {key}"),
            description: String::new(),
            priority: "high".to_string(),
            milestone_key: None,
            requirement_key: None,
            assignee_role: Some(crate::import::AssigneeRole::Developer),
            confidence: crate::proposal::Confidence::High,
            evidence: Vec::new(),
        }
    }

    #[test]
    fn item_count_counts_payload_items_for_batch_commands() {
        // 一条 CreateTasks 含 11 个任务：影响范围是 11，不是 1（P2 修复核心）。
        let eleven = ProjectCommand::CreateTasks {
            tasks: (0..11).map(|i| task_draft(&format!("T{i}"))).collect(),
        };
        assert_eq!(eleven.item_count(), 11);

        let ten = ProjectCommand::CreateTasks {
            tasks: (0..10).map(|i| task_draft(&format!("T{i}"))).collect(),
        };
        assert_eq!(ten.item_count(), 10);

        // 其余三类批量命令同理按载荷长度计。
        let milestones = ProjectCommand::CreateMilestones {
            milestones: vec![
                MilestoneDraft {
                    key: "M1".into(),
                    name: "一".into(),
                    planned_at: None,
                },
                MilestoneDraft {
                    key: "M2".into(),
                    name: "二".into(),
                    planned_at: None,
                },
                MilestoneDraft {
                    key: "M3".into(),
                    name: "三".into(),
                    planned_at: None,
                },
            ],
        };
        assert_eq!(milestones.item_count(), 3);

        let requirements = ProjectCommand::CreateRequirements {
            requirements: vec![
                RequirementDraft {
                    key: "R1".into(),
                    code: "REQ-001".into(),
                    title: "甲".into(),
                    markdown: String::new(),
                    priority: "high".into(),
                    confidence: crate::proposal::Confidence::High,
                    evidence: Vec::new(),
                },
                RequirementDraft {
                    key: "R2".into(),
                    code: "REQ-002".into(),
                    title: "乙".into(),
                    markdown: String::new(),
                    priority: "high".into(),
                    confidence: crate::proposal::Confidence::High,
                    evidence: Vec::new(),
                },
            ],
        };
        assert_eq!(requirements.item_count(), 2);

        let documents = ProjectCommand::WriteDocuments {
            documents: vec![DocumentDraft {
                doc_key: "architecture".into(),
                doc_type: "architecture".into(),
                markdown: "# 架构".into(),
                change_summary: None,
            }],
        };
        assert_eq!(documents.item_count(), 1);
    }

    #[test]
    fn item_count_is_one_for_every_single_object_command() {
        for command in sample_commands() {
            if !matches!(
                command,
                ProjectCommand::CreateMilestones { .. }
                    | ProjectCommand::CreateRequirements { .. }
                    | ProjectCommand::CreateTasks { .. }
                    | ProjectCommand::WriteDocuments { .. }
            ) {
                assert_eq!(
                    command.item_count(),
                    1,
                    "{} affects exactly one authoritative object",
                    command.kind()
                );
            }
        }
    }
}
