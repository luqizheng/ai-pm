//! Relation Registry 加载器：读 `policies/viz-relation-registry.yaml` 并校验结构。
//!
//! 依据：17 号词表会签表 V1.2 §6；12 号 V0-1 设计 §5.5。
//!
//! 加载期 **fail-fast**：字段缺失、键重复、别名缺 `reverse_view_of` 等一律拒绝加载——
//! 词表错了要在启动时炸，而不是在出图时静默放过。
//!
//! 治理要点（ADR-VIZ-004）：
//! - 状态 `draft` / `active` / `deprecated` / `rejected`：**未会签 ≠ 默认允许**；
//! - 关系键为命名空间受控字符串；
//! - 反向视图别名（`view_alias`）不新增业务事实；
//! - 合成关系（`synthetic`）不得声明为业务事实。

use std::path::Path;

use pm_domain::visualization::{validate_registry, RegistryError, RelationRegistry};

/// 词表文件默认路径（仓库内真源）。
pub const REGISTRY_FILE: &str = "policies/viz-relation-registry.yaml";

/// 从 YAML 文本解析词表并校验结构。
pub fn parse_registry(yaml: &str) -> Result<RelationRegistry, RegistryError> {
    let registry: RelationRegistry =
        serde_yaml::from_str(yaml).map_err(|e| RegistryError::Parse(e.to_string()))?;
    validate_registry(&registry)?;
    Ok(registry)
}

/// 从文件路径加载词表。
pub fn load_registry_from_path(path: impl AsRef<Path>) -> Result<RelationRegistry, RegistryError> {
    let path = path.as_ref();
    let yaml = std::fs::read_to_string(path)
        .map_err(|_| RegistryError::NotFound(path.display().to_string()))?;
    parse_registry(&yaml)
}

/// 从仓库默认路径加载词表。
pub fn load_registry() -> Result<RelationRegistry, RegistryError> {
    load_registry_from_path(REGISTRY_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
version: "1"
relations:
  - key: design.covers_requirement
    status: active
    domain: design
    owner_role: design-owner
    source: design.design_requirement
    source_type: design_revision
    target_type: requirement_revision
    direction: directed
    semantics: "设计覆盖需求"
    governance_level: L2
    introduced_in: "1.0"
node_kinds:
  - key: requirement_revision
    semantics: "需求修订"
    domain: delivery
    owner_role: delivery-owner
    status: active
"#;

    #[test]
    fn parses_minimal_registry() {
        let registry = parse_registry(MINIMAL).expect("parse");
        assert_eq!(registry.version, "1");
        assert_eq!(registry.relations.len(), 1);
        assert!(registry.is_relation_projectable("design.covers_requirement"));
        assert!(registry.node_kind("requirement_revision").is_some());
    }

    #[test]
    fn rejects_duplicate_relation_keys() {
        let yaml = r#"
version: "1"
relations:
  - key: design.covers_requirement
    status: active
    domain: design
    owner_role: design-owner
    source: design.design_requirement
    source_type: design_revision
    target_type: requirement_revision
    direction: directed
    semantics: "重复条目"
  - key: design.covers_requirement
    status: draft
    domain: design
    owner_role: design-owner
    source: design.design_requirement
    source_type: design_revision
    target_type: requirement_revision
    direction: directed
    semantics: "重复条目"
node_kinds: []
"#;
        let err = parse_registry(yaml).expect_err("应拒绝重复键");
        assert!(matches!(err, RegistryError::Invalid(_)));
    }

    #[test]
    fn rejects_view_alias_without_reverse_view_of() {
        let yaml = r#"
version: "1"
relations:
  - key: requirement.refines_design
    status: active
    domain: design
    owner_role: design-owner
    source: design.design_requirement
    source_type: requirement_revision
    target_type: design_revision
    direction: directed
    semantics: "反向视图"
    view_alias: true
node_kinds: []
"#;
        let err = parse_registry(yaml).expect_err("应拒绝缺 reverse_view_of 的别名");
        assert!(matches!(err, RegistryError::Invalid(_)));
    }

    #[test]
    fn rejects_synthetic_declared_as_business_fact() {
        let yaml = r#"
version: "1"
relations:
  - key: derived_trace
    status: active
    domain: visualization
    owner_role: architecture-owner
    source: vizspec
    source_type: "*"
    target_type: "*"
    direction: directed
    semantics: "合成关系"
    synthetic: true
    business_fact: true
node_kinds: []
"#;
        let err = parse_registry(yaml).expect_err("应拒绝合成关系声明为业务事实");
        assert!(matches!(err, RegistryError::Invalid(_)));
    }

    #[test]
    fn rejects_relation_without_owner() {
        let yaml = r#"
version: "1"
relations:
  - key: a.b
    status: active
    domain: d
    owner_role: ""
    source: s
    source_type: t
    target_type: u
    direction: directed
    semantics: m
node_kinds: []
"#;
        let err = parse_registry(yaml).expect_err("应拒绝缺 owner 的关系");
        assert!(matches!(err, RegistryError::Invalid(_)));
    }

    #[test]
    fn rejects_malformed_yaml() {
        let err = parse_registry("version: [unclosed").expect_err("应拒绝非法 YAML");
        assert!(matches!(err, RegistryError::Parse(_)));
    }

    #[test]
    fn draft_relation_is_loaded_but_not_projectable() {
        let yaml = MINIMAL.replace("status: active", "status: draft");
        let registry = parse_registry(&yaml).expect("parse");
        assert!(registry.has_relation("design.covers_requirement"));
        assert!(!registry.is_relation_projectable("design.covers_requirement"));
    }
}
