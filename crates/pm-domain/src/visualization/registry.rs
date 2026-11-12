//! Relation Registry（受控关系词表）—— pm-domain 侧的数据结构与查询。
//!
//! Registry 是**映射与校验机制**，不是新的业务关系权威（ADR-VIZ-004）：
//! 它声明「Visualization 可以识别和投影哪些关系」，并指向来源领域的正式定义
//! （业务表或领域注册表），不创造业务关系。
//!
//! 治理要点（17 号 V1.2）：
//! - 状态：`draft`（未会签，**不可投影**）/ `active` / `deprecated`（曾启用后被替代，
//!   保留历史解释）/ `rejected`（会签否决，从未启用）——**未会签 ≠ 默认允许**。
//! - 命名空间：`domain.relation` 受控字符串；`derived_trace` 为可视化域合成关系。
//! - 反向视图别名（`view_alias` + `reverse_view_of`）：**不新增业务事实、不参与
//!   底层关系计数**，同一底层事实不得同时产出两条边。
//! - 跨 Registry 引用（`type_key_source`）：如 PC 关系类型，词表与方向由来源域唯一维护。

use crate::visualization::nature::Nature;

/// 关系登记状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationStatus {
    /// 已提出，未完成领域 Owner 会签——**不可投影**。
    Draft,
    /// 已完成会签并正式启用。
    Active,
    /// 曾启用，现已被替代或废弃——保留历史解释能力，不再投影。
    Deprecated,
    /// 经会签明确否决，从未启用。
    Rejected,
}

impl RelationStatus {
    pub const ALL: [Self; 4] = [Self::Draft, Self::Active, Self::Deprecated, Self::Rejected];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Deprecated => "deprecated",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.as_str() == value)
    }

    /// 该状态是否允许进入正常图投影。
    pub fn is_projectable(self) -> bool {
        matches!(self, Self::Active)
    }
}

/// 一条关系登记。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationDefinition {
    /// 关系键（`domain.relation`）。
    pub key: String,
    /// 登记状态。
    pub status: RelationStatus,
    /// 归属领域。
    pub domain: String,
    /// 责任 Owner 角色。
    pub owner_role: String,
    /// 来源标识（`schema.table.column` 或领域注册表）。
    pub source: String,
    /// 源端点类型（`*` = 通配）。
    pub source_type: String,
    /// 目标端点类型（`*` = 通配）。
    pub target_type: String,
    /// 语义方向（`directed` / `undirected`）。
    pub direction: String,
    /// 业务语义定义。
    pub semantics: String,
    /// 治理级别（L1 / L2），适用时填写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub governance_level: Option<String>,
    /// 引入版本。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub introduced_in: Option<String>,
    /// 会签日期（正式会签后填写）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signoff_date: Option<String>,
    /// 会签记录引用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signoff_record: Option<String>,
    /// 是否为反向视图别名（不新增业务事实）。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub view_alias: bool,
    /// 反向视图所依附的关系键。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverse_view_of: Option<String>,
    /// 是否为合成关系（不对应业务关系表）。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub synthetic: bool,
    /// 是否为业务事实（合成关系为 false）。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub business_fact: bool,
    /// 跨 Registry 来源标识（如 `pc.relationship_type`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_key_source: Option<String>,
    /// 证据角色过滤（如 `evidence`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_role_filter: Option<String>,
    /// 基数（一对多等）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cardinality: Option<String>,
}

impl RelationDefinition {
    /// 端点类型是否匹配（`*` 为通配）。
    pub fn accepts_endpoint(&self, node_kind: &str, is_source: bool) -> bool {
        let expected = if is_source {
            &self.source_type
        } else {
            &self.target_type
        };
        expected == "*" || expected == node_kind
    }

    /// 是否可用于正常投影（active 且非别名）。
    ///
    /// 别名（`view_alias`）不作为独立业务关系参与投影——它只是查询视角。
    pub fn is_projectable(&self) -> bool {
        self.status.is_projectable() && !self.view_alias
    }
}

/// 节点类型词表条目。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeKindDefinition {
    /// 类型键。
    pub key: String,
    /// 语义定义。
    pub semantics: String,
    /// 归属领域。
    pub domain: String,
    /// 责任 Owner 角色。
    pub owner_role: String,
    /// 登记状态。
    pub status: RelationStatus,
}

/// 词表（关系 + 节点类型 + 版本）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationRegistry {
    /// Registry 版本。
    pub version: String,
    /// 关系登记。
    pub relations: Vec<RelationDefinition>,
    /// 节点类型登记。
    #[serde(default)]
    pub node_kinds: Vec<NodeKindDefinition>,
}

impl RelationRegistry {
    /// 按键查找关系。
    pub fn relation(&self, key: &str) -> Option<&RelationDefinition> {
        self.relations.iter().find(|r| r.key == key)
    }

    /// 按键查找节点类型。
    pub fn node_kind(&self, key: &str) -> Option<&NodeKindDefinition> {
        self.node_kinds.iter().find(|k| k.key == key)
    }

    /// 关系键是否可用于正常投影。
    pub fn is_relation_projectable(&self, key: &str) -> bool {
        self.relation(key).is_some_and(|r| r.is_projectable())
    }

    /// 节点类型是否已登记（draft 亦算已登记——登记与启用是两回事）。
    pub fn has_relation_kind(&self, kind: &str) -> bool {
        self.node_kind(kind).is_some()
    }

    /// 关系是否已登记（含 draft/deprecated/rejected）。
    pub fn has_relation(&self, key: &str) -> bool {
        self.relation(key).is_some()
    }
}

/// 词表加载失败原因。
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// 词表文件解析失败。
    #[error("relation registry parse failed: {0}")]
    Parse(String),
    /// 词表结构不合法（字段缺失、键重复等）。
    #[error("invalid relation registry: {0}")]
    Invalid(String),
    /// 词表文件找不到。
    #[error("relation registry file not found: {0}")]
    NotFound(String),
}

/// 校验词表自身的结构合法性（加载期 fail-fast）。
pub fn validate_registry(registry: &RelationRegistry) -> Result<(), RegistryError> {
    let mut seen: Vec<&str> = Vec::new();
    for relation in &registry.relations {
        if relation.key.is_empty() {
            return Err(RegistryError::Invalid("relation key must not be empty".into()));
        }
        if seen.contains(&relation.key.as_str()) {
            return Err(RegistryError::Invalid(format!(
                "duplicate relation key: {}",
                relation.key
            )));
        }
        seen.push(&relation.key);
        if relation.owner_role.is_empty() {
            return Err(RegistryError::Invalid(format!(
                "relation {} must declare an owner_role",
                relation.key
            )));
        }
        if relation.source.is_empty() {
            return Err(RegistryError::Invalid(format!(
                "relation {} must declare a source",
                relation.key
            )));
        }
        if relation.view_alias && relation.reverse_view_of.is_none() {
            return Err(RegistryError::Invalid(format!(
                "relation {} is a view alias but declares no reverse_view_of",
                relation.key
            )));
        }
        if relation.synthetic && relation.business_fact {
            return Err(RegistryError::Invalid(format!(
                "relation {} is synthetic and must not be a business fact",
                relation.key
            )));
        }
    }

    let mut kinds: Vec<&str> = Vec::new();
    for kind in &registry.node_kinds {
        if kinds.contains(&kind.key.as_str()) {
            return Err(RegistryError::Invalid(format!(
                "duplicate node kind: {}",
                kind.key
            )));
        }
        kinds.push(&kind.key);
    }
    Ok(())
}

/// 关系方向与 `Nature` 的组合辅助：合成关系不得声明为业务事实。
pub fn synthetic_requires_explanatory(relation: &RelationDefinition) -> bool {
    relation.synthetic && relation.nature_hint() == Some(Nature::Business)
}

impl RelationDefinition {
    /// 该关系是否被声明为业务事实性质（供合成关系校验使用）。
    pub fn nature_hint(&self) -> Option<Nature> {
        if self.synthetic {
            Some(Nature::Explanatory)
        } else {
            Some(Nature::Business)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relation(key: &str, status: RelationStatus) -> RelationDefinition {
        RelationDefinition {
            key: key.to_string(),
            status,
            domain: "design".to_string(),
            owner_role: "design-owner".to_string(),
            source: "design.design_requirement".to_string(),
            source_type: "design_revision".to_string(),
            target_type: "requirement_revision".to_string(),
            direction: "directed".to_string(),
            semantics: "测试用".to_string(),
            governance_level: Some("L2".to_string()),
            introduced_in: Some("1.0".to_string()),
            signoff_date: None,
            signoff_record: None,
            view_alias: false,
            reverse_view_of: None,
            synthetic: false,
            business_fact: true,
            type_key_source: None,
            ref_role_filter: None,
            cardinality: None,
        }
    }

    #[test]
    fn only_active_is_projectable() {
        assert!(RelationStatus::Active.is_projectable());
        for status in [
            RelationStatus::Draft,
            RelationStatus::Deprecated,
            RelationStatus::Rejected,
        ] {
            assert!(!status.is_projectable(), "{status:?} 不得投影");
        }
    }

    #[test]
    fn view_alias_is_not_projectable() {
        let mut r = relation("requirement.refines_design", RelationStatus::Active);
        r.view_alias = true;
        r.reverse_view_of = Some("design.covers_requirement".to_string());
        assert!(!r.is_projectable(), "反向视图别名不得作为独立关系投影");
    }

    #[test]
    fn endpoint_matching_respects_wildcard() {
        let r = relation("design.covers_requirement", RelationStatus::Active);
        assert!(r.accepts_endpoint("design_revision", true));
        assert!(!r.accepts_endpoint("task", true));
        assert!(r.accepts_endpoint("requirement_revision", false));

        let mut wildcard = relation("derived_trace", RelationStatus::Active);
        wildcard.source_type = "*".to_string();
        wildcard.target_type = "*".to_string();
        assert!(wildcard.accepts_endpoint("anything", true));
        assert!(wildcard.accepts_endpoint("anything", false));
    }

    #[test]
    fn registry_lookup() {
        let registry = RelationRegistry {
            version: "1".to_string(),
            relations: vec![
                relation("design.covers_requirement", RelationStatus::Active),
                relation("task.depends_on_task", RelationStatus::Draft),
            ],
            node_kinds: vec![NodeKindDefinition {
                key: "requirement_revision".to_string(),
                semantics: "需求修订".to_string(),
                domain: "delivery".to_string(),
                owner_role: "delivery-owner".to_string(),
                status: RelationStatus::Active,
            }],
        };
        assert!(registry.has_relation("task.depends_on_task"));
        assert!(registry.is_relation_projectable("design.covers_requirement"));
        assert!(!registry.is_relation_projectable("task.depends_on_task"));
        assert!(registry.node_kind("requirement_revision").is_some());
    }

    #[test]
    fn validate_rejects_duplicate_keys() {
        let registry = RelationRegistry {
            version: "1".to_string(),
            relations: vec![
                relation("a.b", RelationStatus::Active),
                relation("a.b", RelationStatus::Active),
            ],
            node_kinds: vec![],
        };
        assert!(validate_registry(&registry).is_err());
    }

    #[test]
    fn validate_rejects_alias_without_reverse_view() {
        let mut r = relation("a.b", RelationStatus::Active);
        r.view_alias = true;
        let registry = RelationRegistry {
            version: "1".to_string(),
            relations: vec![r],
            node_kinds: vec![],
        };
        assert!(validate_registry(&registry).is_err());
    }

    #[test]
    fn validate_rejects_synthetic_business_fact() {
        let mut r = relation("derived_trace", RelationStatus::Active);
        r.synthetic = true;
        r.business_fact = true;
        let registry = RelationRegistry {
            version: "1".to_string(),
            relations: vec![r],
            node_kinds: vec![],
        };
        assert!(validate_registry(&registry).is_err());
    }

    #[test]
    fn validate_accepts_wellformed_registry() {
        let registry = RelationRegistry {
            version: "1".to_string(),
            relations: vec![relation("design.covers_requirement", RelationStatus::Active)],
            node_kinds: vec![],
        };
        assert!(validate_registry(&registry).is_ok());
    }
}
