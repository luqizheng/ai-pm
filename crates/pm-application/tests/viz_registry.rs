//! Relation Registry 真源文件的加载校验（集成测试）。
//!
//! 校验对象是仓库内真源 `policies/viz-relation-registry.yaml`——它必须**始终**能被
//! 加载且结构合法，否则词表真源本身就坏了。

use std::path::PathBuf;

use pm_application::visualization::registry_loader::load_registry_from_path;

/// 词表真源路径（测试的工作目录是 crate 目录，需从 manifest 目录上溯到仓库根）。
fn registry_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("policies")
        .join("viz-relation-registry.yaml")
}

#[test]
fn registry_file_loads_and_is_structurally_valid() {
    let registry = load_registry_from_path(registry_path()).expect("词表真源必须可加载");
    assert!(!registry.relations.is_empty(), "词表不得为空");
    assert!(!registry.node_kinds.is_empty(), "节点类型词表不得为空");
}

#[test]
fn every_relation_declares_owner_and_source() {
    let registry = load_registry_from_path(registry_path()).expect("词表真源必须可加载");
    for relation in &registry.relations {
        assert!(
            !relation.owner_role.trim().is_empty(),
            "关系 {} 缺 owner_role",
            relation.key
        );
        assert!(
            !relation.source.trim().is_empty(),
            "关系 {} 缺 source",
            relation.key
        );
        // 命名空间要求适用于**业务域关系**（domain.relation）；
        // visualization 域的合成关系 `derived_trace` 按 17 号词表定名，不带前缀。
        if relation.domain != "visualization" {
            assert!(
                relation.key.contains('.'),
                "业务域关系键必须是命名空间受控字符串：{}",
                relation.key
            );
        }
    }
}

#[test]
fn alias_declares_its_base_relation() {
    let registry = load_registry_from_path(registry_path()).expect("词表真源必须可加载");
    for relation in &registry.relations {
        if relation.view_alias {
            let base = relation
                .reverse_view_of
                .as_ref()
                .expect("反向视图必须声明 reverse_view_of");
            assert!(
                registry.has_relation(base),
                "反向视图 {} 指向的关系 {} 不存在",
                relation.key,
                base
            );
            assert!(
                !relation.is_projectable(),
                "反向视图别名 {} 不得作为独立关系投影",
                relation.key
            );
        }
    }
}

#[test]
fn synthetic_relations_are_not_business_facts() {
    let registry = load_registry_from_path(registry_path()).expect("词表真源必须可加载");
    for relation in &registry.relations {
        if relation.synthetic {
            assert!(
                !relation.business_fact,
                "合成关系 {} 不得声明为业务事实",
                relation.key
            );
        }
    }
}

#[test]
fn signoff_state_is_explicit() {
    let registry = load_registry_from_path(registry_path()).expect("词表真源必须可加载");
    // 会签完成前，所有关系都应是 draft（不可投影）；本测试锁定该不变量，
    // 会签完成后由 Owner 逐条改为 active 并在此处同步放行。
    for relation in &registry.relations {
        if relation.status.is_projectable() {
            // G-1（专家裁决）：active 必须齐备六项审计字段，缺一即拒绝
            let missing = relation.missing_audit_fields();
            assert!(
                missing.is_empty(),
                "已启用的关系 {} 缺少 G-1 审计字段：{:?}",
                relation.key,
                missing
            );
        }
    }
}
