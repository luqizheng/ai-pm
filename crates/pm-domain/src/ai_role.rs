//! AI Role 分类与 Catalog 常量（V2 D-1=A，`docs/project-ai-team-v2/
//! ai-pm-ai-team-kickoff-adjudication-pack-v1.md` §二）。
//!
//! Role 实体与 `core.project_member.role` 语义分离：后者六值里四个 Agent
//! 职能值是「任务承接定位」（见 [`crate::project_member`]），本模块的
//! `AiRoleCategory` 是 Role 模板的管理分类，两者不得混用。
//!
//! 与迁移 `0058` 的 `ai.ai_role_template.category` CHECK 保持同一份口径。

use serde::{Deserialize, Serialize};

/// Role 模板的管理分类（稿 §3.2 Catalog 八类；V1 首批只播其中四类）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiRoleCategory {
    Management,
    Product,
    Knowledge,
    Architecture,
    Design,
    Engineering,
    Quality,
    Governance,
}

impl AiRoleCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Management => "management",
            Self::Product => "product",
            Self::Knowledge => "knowledge",
            Self::Architecture => "architecture",
            Self::Design => "design",
            Self::Engineering => "engineering",
            Self::Quality => "quality",
            Self::Governance => "governance",
        }
    }

    /// 数据库取值 → 枚举；认不出即非法（与 0058 CHECK 同口径）。
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "management" => Some(Self::Management),
            "product" => Some(Self::Product),
            "knowledge" => Some(Self::Knowledge),
            "architecture" => Some(Self::Architecture),
            "design" => Some(Self::Design),
            "engineering" => Some(Self::Engineering),
            "quality" => Some(Self::Quality),
            "governance" => Some(Self::Governance),
            _ => None,
        }
    }
}

/// Catalog 首批 5 个预置 role_key（与 0058 迁移种子同一份清单；13 角色目录
/// 按稿 §3.3 分批启用，V1 只播与现有域对齐的这批）。
pub const AI_ROLE_TEMPLATE_SEED_KEYS: [&str; 5] = [
    "project_manager",
    "requirement_analyst",
    "architect",
    "developer",
    "tester",
];

/// 不允许项目禁用的 role_key（稿 §3.3 / AT-D12：AI Project Manager 是管理
/// 协调角色，项目无权关闭）。
pub const AI_ROLE_UNDISABLEABLE_KEYS: [&str; 1] = ["project_manager"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_roundtrips() {
        for value in [
            "management",
            "product",
            "knowledge",
            "architecture",
            "design",
            "engineering",
            "quality",
            "governance",
        ] {
            let parsed = AiRoleCategory::from_str(value).expect("known category must parse");
            assert_eq!(parsed.as_str(), value);
            let json = serde_json::to_string(&parsed).unwrap();
            assert_eq!(json, format!("\"{value}\""));
            let back: AiRoleCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(back, parsed);
        }
        assert!(AiRoleCategory::from_str("unknown").is_none());
    }

    #[test]
    fn seed_keys_cover_undisableable() {
        // 不可禁用清单必须是种子目录的子集。
        for key in AI_ROLE_UNDISABLEABLE_KEYS {
            assert!(
                AI_ROLE_TEMPLATE_SEED_KEYS.contains(&key),
                "undisableable key `{key}` must exist in the seed catalog"
            );
        }
    }
}
