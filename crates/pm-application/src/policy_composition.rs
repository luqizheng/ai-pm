//! D2.1 完整策略合成（`docs/ai-pm-task-state-machine-v1.md` §15.4–§15.6）。
//!
//! 两分运算，**不能用同一个 AND 处理所有字段**：
//! - 能力类（`auto_accept` / `auto_merge` / `auto_publish_knowledge`）：
//!   所有适用层都必须允许（`None` = 该层不表态，不额外收紧）。
//! - 约束类（`require_human_approval` / `additional_checks`）：只能增加，
//!   任一层要求审批即要求审批；额外校验取并集去重。
//!
//! `policy_version` / `policy_hash` 采用**内容寻址**：对规范化后的有效策略取
//! sha256，`policy_hash` 为全量小写 hex，`policy_version` 为前 12 位。时间戳不在
//! 哈希载荷内，因此同一策略内容重复计算得到同一版本，无需新增版本表。

use std::collections::BTreeSet;

use serde::Serialize;
use sha2::{Digest, Sha256};

/// 快照模式标识：既写入 `policy_snapshot.snapshot_scope`，也参与哈希。
/// 语义变更（例如将来引入分动作审批字段）必须升版本，避免同 hash 不同含义。
pub const POLICY_SCHEMA: &str = "composed-v1";

/// 单层策略声明。`None` 一律表示该层不表态。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyLayer {
    pub auto_accept: Option<bool>,
    pub auto_merge: Option<bool>,
    pub auto_publish_knowledge: Option<bool>,
    pub require_human_approval: Option<bool>,
    pub additional_checks: Vec<serde_json::Value>,
}

/// 四层策略输入（organization → project → task_type → task）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyLayers {
    pub organization: Option<PolicyLayer>,
    pub project: Option<PolicyLayer>,
    pub task_type: Option<PolicyLayer>,
    pub task: Option<PolicyLayer>,
}

/// 合成后的有效策略（Run 级不可变快照的内容）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectivePolicy {
    pub auto_accept: bool,
    pub auto_merge: bool,
    pub auto_publish_knowledge: bool,
    pub require_human_approval: bool,
    pub additional_checks: Vec<serde_json::Value>,
}

/// 哈希载荷：字段顺序固定，不含 `captured_at` 等时间字段。
#[derive(Serialize)]
struct HashPayload<'a> {
    schema: &'static str,
    auto_accept: bool,
    auto_merge: bool,
    auto_publish_knowledge: bool,
    require_human_approval: bool,
    additional_checks: &'a [serde_json::Value],
}

impl EffectivePolicy {
    /// 四层合成。缺层 / `None` 一律视为「该层不表态」。
    pub fn compose(layers: &PolicyLayers) -> Self {
        let applicable: Vec<&PolicyLayer> = [
            layers.organization.as_ref(),
            layers.project.as_ref(),
            layers.task_type.as_ref(),
            layers.task.as_ref(),
        ]
        .into_iter()
        .flatten()
        .collect();

        let require_human_approval = applicable
            .iter()
            .any(|layer| layer.require_human_approval == Some(true));

        Self {
            // §15.6：人工审批否决自动验收；合并与知识生效各自独立判定，
            // 不共用 `requires_human_approval`（原文的显式建议）。
            auto_accept: resolve_capability(&applicable, |layer| layer.auto_accept)
                && !require_human_approval,
            auto_merge: resolve_capability(&applicable, |layer| layer.auto_merge),
            auto_publish_knowledge: resolve_capability(&applicable, |layer| {
                layer.auto_publish_knowledge
            }),
            require_human_approval,
            additional_checks: union_checks(&applicable),
        }
    }

    /// 规范化有效策略的 sha256（小写 hex）。
    pub fn content_hash(&self) -> String {
        let payload = HashPayload {
            schema: POLICY_SCHEMA,
            auto_accept: self.auto_accept,
            auto_merge: self.auto_merge,
            auto_publish_knowledge: self.auto_publish_knowledge,
            require_human_approval: self.require_human_approval,
            additional_checks: &self.additional_checks,
        };
        let bytes = serde_json::to_vec(&payload)
            .expect("policy hash payload only contains serializable primitives");
        hex(&Sha256::digest(&bytes))
    }

    /// 内容寻址版本号：`policy_hash` 的前 12 位。
    pub fn policy_version(&self) -> String {
        self.content_hash()[..12].to_string()
    }

    /// 写入 `policy_snapshot.effective_policy` 的投影。
    pub fn to_snapshot_json(&self) -> serde_json::Value {
        serde_json::json!({
            "auto_accept": self.auto_accept,
            "auto_merge": self.auto_merge,
            "auto_publish_knowledge": self.auto_publish_knowledge,
            "require_human_approval": self.require_human_approval,
            "additional_checks": self.additional_checks,
        })
    }
}

/// 能力类：所有表态层都必须允许；无表态层时视为允许（不额外收紧）。
fn resolve_capability(
    layers: &[&PolicyLayer],
    field: impl Fn(&PolicyLayer) -> Option<bool>,
) -> bool {
    layers.iter().all(|layer| field(layer).unwrap_or(true))
}

/// 约束类：并集去重，按规范化 JSON 排序保证顺序稳定。
fn union_checks(layers: &[&PolicyLayer]) -> Vec<serde_json::Value> {
    let mut seen = BTreeSet::new();
    let mut merged = Vec::new();
    for layer in layers {
        for check in &layer.additional_checks {
            let key = canonical_check(check);
            if seen.insert(key) {
                merged.push(check.clone());
            }
        }
    }
    merged.sort_by_key(canonical_check);
    merged
}

fn canonical_check(value: &serde_json::Value) -> String {
    serde_json::to_string(value).expect("serde_json::Value is always serializable")
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}


#[cfg(test)]
mod tests {
    use super::*;

    fn layer(
        auto_accept: Option<bool>,
        auto_merge: Option<bool>,
        auto_publish_knowledge: Option<bool>,
        require_human_approval: Option<bool>,
        checks: &[&str],
    ) -> PolicyLayer {
        PolicyLayer {
            auto_accept,
            auto_merge,
            auto_publish_knowledge,
            require_human_approval,
            additional_checks: checks
                .iter()
                .map(|value| serde_json::json!(value))
                .collect(),
        }
    }

    #[test]
    fn all_layers_allow_capabilities() {
        let layers = PolicyLayers {
            organization: Some(layer(Some(true), Some(true), Some(true), Some(false), &[])),
            project: Some(layer(Some(true), None, Some(true), None, &[])),
            task_type: Some(layer(None, Some(true), None, None, &[])),
            task: Some(layer(None, None, None, None, &[])),
        };
        let effective = EffectivePolicy::compose(&layers);
        assert!(effective.auto_accept);
        assert!(effective.auto_merge);
        assert!(effective.auto_publish_knowledge);
        assert!(!effective.require_human_approval);
    }

    #[test]
    fn any_layer_forbids_a_capability() {
        let layers = PolicyLayers {
            organization: Some(layer(Some(true), Some(true), Some(true), None, &[])),
            project: Some(layer(Some(false), Some(true), Some(true), None, &[])),
            task_type: None,
            task: Some(layer(Some(true), Some(false), Some(true), None, &[])),
        };
        let effective = EffectivePolicy::compose(&layers);
        assert!(!effective.auto_accept, "one layer forbidding wins");
        assert!(!effective.auto_merge, "one layer forbidding wins");
        assert!(effective.auto_publish_knowledge);
    }

    #[test]
    fn absent_layers_do_not_narrow_capabilities() {
        let layers = PolicyLayers {
            organization: None,
            project: None,
            task_type: None,
            task: None,
        };
        let effective = EffectivePolicy::compose(&layers);
        assert!(effective.auto_accept);
        assert!(effective.auto_merge);
        assert!(effective.auto_publish_knowledge);
        assert!(!effective.require_human_approval);
        assert!(effective.additional_checks.is_empty());
    }

    #[test]
    fn human_approval_vetoes_auto_accept_only() {
        let layers = PolicyLayers {
            organization: Some(layer(None, None, None, None, &[])),
            project: Some(layer(None, None, None, None, &[])),
            task_type: Some(layer(None, None, None, Some(true), &[])),
            task: Some(layer(Some(false), Some(true), Some(true), None, &[])),
        };
        let effective = EffectivePolicy::compose(&layers);
        assert!(effective.require_human_approval);
        assert!(!effective.auto_accept, "approval requirement cancels auto accept");
        assert!(effective.auto_merge, "merge keeps its own capability decision");
        assert!(effective.auto_publish_knowledge);
    }

    #[test]
    fn additional_checks_are_unioned_deduplicated_and_sorted() {
        let layers = PolicyLayers {
            organization: Some(layer(None, None, None, None, &["security_review"])),
            project: Some(layer(None, None, None, None, &["architecture_review", "security_review"])),
            task_type: None,
            task: Some(layer(None, None, None, None, &["release_review"])),
        };
        let effective = EffectivePolicy::compose(&layers);
        let checks: Vec<&str> = effective
            .additional_checks
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(
            checks,
            vec!["architecture_review", "release_review", "security_review"]
        );
    }

    #[test]
    fn content_hash_is_stable_and_moves_with_any_field() {
        let first = EffectivePolicy::compose(&PolicyLayers {
            organization: Some(layer(Some(true), Some(true), Some(true), None, &["a"])),
            ..Default::default()
        });
        let second = EffectivePolicy::compose(&PolicyLayers {
            organization: Some(layer(Some(true), Some(true), Some(true), None, &["a"])),
            task: Some(layer(None, None, None, None, &["a"])),
            ..Default::default()
        });
        assert_eq!(
            first.content_hash(),
            second.content_hash(),
            "same effective policy must hash identically regardless of layer split"
        );
        assert_eq!(first.policy_version(), second.policy_version());

        let mut changed = first.clone();
        changed.additional_checks.push(serde_json::json!("b"));
        assert_ne!(first.content_hash(), changed.content_hash());
        assert_ne!(first.policy_version(), changed.policy_version());
        assert_eq!(first.policy_version(), first.content_hash()[..12]);
    }
}