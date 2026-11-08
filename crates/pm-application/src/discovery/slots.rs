//! 访谈槽位表（DK-B §4.2）：**系统常量，不落库**（零迁移纪律）。
//!
//! 「问完毕」的第二个条件是「槽位全绿」，它必须能由**已持久化的 finding**
//! 纯函数复算，不能采信模型自述。`discovery_finding` 没有槽位列、本批零迁移，
//! 因此编排器把槽位标签写进既有预留列 `actor_role`，形如 `slot:<id>`：
//!
//! - 只有本表里的 id 会被接受（[`is_valid_slot`]），其余一律视为越权槽位；
//! - 解析函数 [`parse_slot_tag`] 只剥前缀，不回检成员关系（调用方负责）；
//! - 人类/采集端写入的 `actor_role`（真正的角色名）不会被误认成槽位标签。
//!
//! 槽位表可调（改本文件即可），不构成数据契约：改表只影响后续轮次的判定。

/// 一个槽位：稳定 id（机器用）+ 中文标签（提示词渲染用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub id: &'static str,
    pub label: &'static str,
}

/// 槽位标签在 `actor_role` 上的前缀。
pub const SLOT_TAG_PREFIX: &str = "slot:";

pub const TASK_DISCOVERY_SLOTS: [Slot; 6] = [
    Slot {
        id: "trigger_scenario",
        label: "触发场景",
    },
    Slot {
        id: "users_and_roles",
        label: "使用角色",
    },
    Slot {
        id: "core_behavior",
        label: "核心行为",
    },
    Slot {
        id: "data_and_state",
        label: "数据与状态",
    },
    Slot {
        id: "boundaries_and_exceptions",
        label: "边界与例外",
    },
    Slot {
        id: "acceptance_criteria",
        label: "验收口径",
    },
];

pub const INITIAL_ONBOARDING_SLOTS: [Slot; 5] = [
    Slot {
        id: "product_positioning",
        label: "产品定位",
    },
    Slot {
        id: "main_roles",
        label: "主要角色",
    },
    Slot {
        id: "core_flows",
        label: "核心流程",
    },
    Slot {
        id: "tech_stack_and_constraints",
        label: "技术栈与约束",
    },
    Slot {
        id: "current_stage_and_pain",
        label: "当前阶段与痛点",
    },
];

pub const UPDATE_SLOTS: [Slot; 3] = [
    Slot {
        id: "change_point",
        label: "变更点",
    },
    Slot {
        id: "impact_scope",
        label: "影响面",
    },
    Slot {
        id: "compatibility_constraints",
        label: "兼容约束",
    },
];

/// 会话 kind → 槽位表（未知 kind 返回空表，调用方据此跳过）。
pub fn slots_for_kind(kind: &str) -> &'static [Slot] {
    match kind {
        "TASK_DISCOVERY" => &TASK_DISCOVERY_SLOTS,
        "INITIAL_ONBOARDING" => &INITIAL_ONBOARDING_SLOTS,
        "UPDATE" => &UPDATE_SLOTS,
        _ => &[],
    }
}

/// 槽位 id 是否属于该 kind 的槽位表。
pub fn is_valid_slot(kind: &str, slot: &str) -> bool {
    slots_for_kind(kind).iter().any(|s| s.id == slot)
}

/// `actor_role` 上的槽位标签。
pub fn slot_tag(slot: &str) -> String {
    format!("{SLOT_TAG_PREFIX}{slot}")
}

/// 从 `actor_role` 剥出槽位 id（无标签返回 `None`，不回检成员关系）。
pub fn parse_slot_tag(actor_role: Option<&str>) -> Option<&str> {
    actor_role
        .and_then(|value| value.strip_prefix(SLOT_TAG_PREFIX))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_tables_match_the_design_cardinality() {
        assert_eq!(slots_for_kind("TASK_DISCOVERY").len(), 6);
        assert_eq!(slots_for_kind("INITIAL_ONBOARDING").len(), 5);
        assert_eq!(slots_for_kind("UPDATE").len(), 3);
        assert!(slots_for_kind("CONVERSATION").is_empty());
    }

    #[test]
    fn slot_membership_is_per_kind() {
        assert!(is_valid_slot("TASK_DISCOVERY", "core_behavior"));
        assert!(!is_valid_slot("UPDATE", "core_behavior"));
        assert!(!is_valid_slot("TASK_DISCOVERY", "made_up_slot"));
    }

    #[test]
    fn tag_roundtrip_and_foreign_actor_roles_are_not_tags() {
        assert_eq!(
            parse_slot_tag(Some("slot:core_behavior")),
            Some("core_behavior")
        );
        assert_eq!(parse_slot_tag(Some("researcher")), None);
        assert_eq!(parse_slot_tag(Some("slot:")), None);
        assert_eq!(parse_slot_tag(None), None);
        assert_eq!(slot_tag("core_behavior"), "slot:core_behavior");
    }
}
