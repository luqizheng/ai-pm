//! 冷启动导入的草案契约：AI 读仓库后产出的结构化内容，不是变更本身。
//!
//! 草案只存 `integration.repo_import.draft`；`apply` 时才按类型批量转成
//! `ai.change_proposal`，走既有红线审批与执行链。

use serde::{Deserialize, Serialize};

use crate::proposal::Confidence;

/// 证据来源：仓库里的文档路径，或一次提交。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Doc,
    Commit,
}

/// 承接任务的 Agent 角色（对应 `core.project_member.role`）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssigneeRole {
    AiPm,
    Architect,
    Developer,
    Tester,
}

impl AssigneeRole {
    pub fn as_str(self) -> &'static str {
        match self {
            AssigneeRole::AiPm => "ai_pm",
            AssigneeRole::Architect => "architect",
            AssigneeRole::Developer => "developer",
            AssigneeRole::Tester => "tester",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "ai_pm" => Some(AssigneeRole::AiPm),
            "architect" => Some(AssigneeRole::Architect),
            "developer" => Some(AssigneeRole::Developer),
            "tester" => Some(AssigneeRole::Tester),
            _ => None,
        }
    }
}

/// 一条可核验的证据。`ref` 对 doc 是路径，对 commit 是 7–40 位 sha。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Evidence {
    pub kind: EvidenceKind,
    pub repo_id: String,
    pub r#ref: String,
    #[serde(default)]
    pub detail: String,
}

/// 项目画像建议。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DraftProfile {
    #[serde(default)]
    pub suggested_code: String,
    #[serde(default)]
    pub suggested_name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub tech_stack: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MilestoneDraft {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub planned_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequirementDraft {
    pub key: String,
    pub code: String,
    pub title: String,
    #[serde(default)]
    pub markdown: String,
    #[serde(default = "default_priority")]
    pub priority: String,
    #[serde(default = "default_confidence")]
    pub confidence: Confidence,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskDraft {
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_priority")]
    pub priority: String,
    #[serde(default)]
    pub milestone_key: Option<String>,
    #[serde(default)]
    pub requirement_key: Option<String>,
    #[serde(default)]
    pub assignee_role: Option<AssigneeRole>,
    #[serde(default = "default_confidence")]
    pub confidence: Confidence,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// `WriteDocuments` 命令的载荷元素。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentDraft {
    pub doc_key: String,
    pub doc_type: String,
    pub markdown: String,
    #[serde(default)]
    pub change_summary: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RiskDraft {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnknownDraft {
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub why_blocking: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoCoverage {
    pub repository_id: String,
    #[serde(default)]
    pub head_commit_sha: String,
    #[serde(default)]
    pub commits_analyzed: usize,
    #[serde(default)]
    pub docs_analyzed: usize,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DraftCoverage {
    #[serde(default)]
    pub repos: Vec<RepoCoverage>,
}

pub const DRAFT_SCHEMA_VERSION: i32 = 1;

/// AI 对一次冷启动的完整草案。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectDraft {
    #[serde(default = "default_schema_version")]
    pub schema_version: i32,
    #[serde(default)]
    pub profile: DraftProfile,
    #[serde(default)]
    pub milestones: Vec<MilestoneDraft>,
    #[serde(default)]
    pub requirements: Vec<RequirementDraft>,
    #[serde(default)]
    pub tasks: Vec<TaskDraft>,
    #[serde(default)]
    pub risks: Vec<RiskDraft>,
    #[serde(default)]
    pub unknowns: Vec<UnknownDraft>,
    #[serde(default)]
    pub coverage: DraftCoverage,
}

impl Default for ProjectDraft {
    fn default() -> Self {
        Self {
            schema_version: DRAFT_SCHEMA_VERSION,
            profile: DraftProfile::default(),
            milestones: Vec::new(),
            requirements: Vec::new(),
            tasks: Vec::new(),
            risks: Vec::new(),
            unknowns: Vec::new(),
            coverage: DraftCoverage::default(),
        }
    }
}

fn default_priority() -> String {
    "normal".to_string()
}

fn default_confidence() -> Confidence {
    Confidence::Medium
}

fn default_schema_version() -> i32 {
    DRAFT_SCHEMA_VERSION
}

/// 组织编制：草案允许指派到的角色集合。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrgChart {
    pub roles: Vec<AssigneeRole>,
}

impl OrgChart {
    /// P0 固定编制：AI 项目经理 + 三名员工。
    pub fn p0() -> Self {
        Self {
            roles: vec![
                AssigneeRole::AiPm,
                AssigneeRole::Architect,
                AssigneeRole::Developer,
                AssigneeRole::Tester,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_json_roundtrip_with_empty_evidence_and_tasks() {
        let draft = ProjectDraft {
            profile: DraftProfile {
                suggested_code: "MAP-001".to_string(),
                suggested_name: "世界地图编辑器".to_string(),
                summary: "从空仓库冷启动".to_string(),
                domain: "gis".to_string(),
                tech_stack: vec!["Rust".to_string(), "Vue 3".to_string()],
            },
            milestones: vec![MilestoneDraft {
                key: "M1".to_string(),
                name: "核心地形系统".to_string(),
                planned_at: None,
            }],
            requirements: vec![RequirementDraft {
                key: "R1".to_string(),
                code: "REQ-001".to_string(),
                title: "地形分块加载".to_string(),
                markdown: "支持分块与 LOD。".to_string(),
                priority: "high".to_string(),
                confidence: Confidence::High,
                evidence: vec![Evidence {
                    kind: EvidenceKind::Doc,
                    repo_id: "repo-1".to_string(),
                    r#ref: "docs/terrain.md".to_string(),
                    detail: "分块与 LOD".to_string(),
                }],
            }],
            ..ProjectDraft::default()
        };
        let json = serde_json::to_string(&draft).unwrap();
        let back: ProjectDraft = serde_json::from_str(&json).unwrap();
        assert_eq!(back, draft);
        assert!(back.tasks.is_empty());
    }

    #[test]
    fn draft_tolerates_missing_optional_fields() {
        let draft: ProjectDraft = serde_json::from_str(
            r#"{
                "profile": {"suggested_code": "X", "suggested_name": "X"},
                "requirements": [{"key": "R1", "code": "REQ-1", "title": "t"}],
                "tasks": [{"key": "T1", "title": "t"}]
            }"#,
        )
        .unwrap();
        assert_eq!(draft.schema_version, DRAFT_SCHEMA_VERSION);
        assert!(draft.requirements[0].evidence.is_empty());
        assert_eq!(draft.requirements[0].confidence, Confidence::Medium);
        assert!(draft.tasks[0].assignee_role.is_none());
    }

    #[test]
    fn assignee_role_roundtrips_through_str() {
        for role in [
            AssigneeRole::AiPm,
            AssigneeRole::Architect,
            AssigneeRole::Developer,
            AssigneeRole::Tester,
        ] {
            assert_eq!(AssigneeRole::parse(role.as_str()), Some(role));
        }
        assert_eq!(AssigneeRole::parse("owner"), None);
    }
}
