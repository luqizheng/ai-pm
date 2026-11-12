//! 校验问题码表（V1.0）。
//!
//! 命名沿用 `E_` / `W_` / `I_` 三级前缀（与 SeeCode 的 `E_`/`W_`/`I_` 惯例同形，
//! 但取值由 AI-PM 自有）：`E_` = 阻断（不得入库/渲染），`W_` = 警示（可渲染但需标注），
//! `I_` = 提示。
//!
//! 码表是**契约**：码值一旦发布不得改写语义（只能新增），否则既有 Golden 语料与
//! 上游修补逻辑会失效。

/// 阻断级问题码（不得入库 / 不得渲染）。
pub mod blocker {
    /// Spec 版本不兼容（major 不同，或 major 同而 minor 更高）。
    pub const SPEC_VERSION_UNSUPPORTED: &str = "E_SPEC_VERSION_UNSUPPORTED";
    /// 节点 id 重复。
    pub const SPEC_DUPLICATE_NODE_ID: &str = "E_SPEC_DUPLICATE_NODE_ID";
    /// 边 id 重复。
    pub const SPEC_DUPLICATE_EDGE_ID: &str = "E_SPEC_DUPLICATE_EDGE_ID";
    /// 边端点未命中任何节点。
    pub const SPEC_EDGE_ENDPOINT_UNKNOWN: &str = "E_SPEC_EDGE_ENDPOINT_UNKNOWN";
    /// 节点缺少来源引用。
    pub const SPEC_MISSING_SOURCE_REF: &str = "E_SPEC_MISSING_SOURCE_REF";
    /// 来源引用指向不存在或无权访问的对象（**无权一律按不存在处理**，不泄露存在性）。
    pub const SPEC_UNKNOWN_OBJECT_REF: &str = "E_SPEC_UNKNOWN_OBJECT_REF";
    /// 关系未在 Registry 登记。
    pub const SPEC_RELATION_NOT_REGISTERED: &str = "E_SPEC_RELATION_NOT_REGISTERED";
    /// 关系两端类型与声明的 source/target type 不符。
    pub const SPEC_RELATION_KIND_MISMATCH: &str = "E_SPEC_RELATION_KIND_MISMATCH";
    /// 关系已登记但尚未完成领域 Owner 会签（draft）。
    pub const SPEC_RELATION_NOT_APPROVED: &str = "E_SPEC_RELATION_NOT_APPROVED";
    /// 节点类型未在词表登记。
    pub const SPEC_NODE_KIND_NOT_REGISTERED: &str = "E_SPEC_NODE_KIND_NOT_REGISTERED";
    /// 超出复杂度预算档位。
    pub const SPEC_BUDGET_EXCEEDED: &str = "E_SPEC_BUDGET_EXCEEDED";
    /// 序列化后超出 payload 上限。
    pub const SPEC_PAYLOAD_TOO_LARGE: &str = "E_SPEC_PAYLOAD_TOO_LARGE";
    /// 标签超长。
    pub const SPEC_LABEL_TOO_LONG: &str = "E_SPEC_LABEL_TOO_LONG";
    /// 标签含控制字符。
    pub const SPEC_LABEL_CONTROL_CHARS: &str = "E_SPEC_LABEL_CONTROL_CHARS";
    /// 槽位不在调用者可写白名单内。
    pub const SPEC_SLOT_NOT_ALLOWED: &str = "E_SPEC_SLOT_NOT_ALLOWED";
    /// 间接推导标记结构非法（`direct=true` 或 `via` 为空）。
    pub const SPEC_DERIVED_TRACE_INVALID: &str = "E_SPEC_DERIVED_TRACE_INVALID";
    /// 边的性质与端点节点性质冲突（business 边挂在 explanatory 节点上）。
    pub const SPEC_NATURE_MISMATCH: &str = "E_SPEC_NATURE_MISMATCH";
}

/// 警示级问题码（可渲染，但需显式标注）。
pub mod warning {
    /// 孤立节点（无任何入边/出边）。
    pub const SPEC_NODE_WITHOUT_EDGE: &str = "W_SPEC_NODE_WITHOUT_EDGE";
    /// 同 kind 同 label 重复。
    pub const SPEC_DUPLICATE_LABEL: &str = "W_SPEC_DUPLICATE_LABEL";
    /// 边端点因权限裁剪而缺失（悬空边）。
    pub const SPEC_DANGLING_EDGE: &str = "W_SPEC_DANGLING_EDGE";
    /// 布局意图与图形种类不匹配。
    pub const SPEC_LAYOUT_HINT_CONFLICT: &str = "W_SPEC_LAYOUT_HINT_CONFLICT";
    /// 焦点未命中任何节点。
    pub const SPEC_FOCUS_UNMATCHED: &str = "W_SPEC_FOCUS_UNMATCHED";
    /// 透镜无可匹配对象。
    pub const SPEC_LENS_UNMATCHED: &str = "W_SPEC_LENS_UNMATCHED";
}

/// 提示级问题码。
pub mod info {
    /// Registry 版本低于当前发布版本。
    pub const SPEC_REGISTRY_VERSION_STALE: &str = "I_SPEC_REGISTRY_VERSION_STALE";
}

/// 全部问题码（供 lint 与文档核对使用）。
pub const ALL_CODES: &[&str] = &[
    blocker::SPEC_VERSION_UNSUPPORTED,
    blocker::SPEC_DUPLICATE_NODE_ID,
    blocker::SPEC_DUPLICATE_EDGE_ID,
    blocker::SPEC_EDGE_ENDPOINT_UNKNOWN,
    blocker::SPEC_MISSING_SOURCE_REF,
    blocker::SPEC_UNKNOWN_OBJECT_REF,
    blocker::SPEC_RELATION_NOT_REGISTERED,
    blocker::SPEC_RELATION_KIND_MISMATCH,
    blocker::SPEC_RELATION_NOT_APPROVED,
    blocker::SPEC_NODE_KIND_NOT_REGISTERED,
    blocker::SPEC_BUDGET_EXCEEDED,
    blocker::SPEC_PAYLOAD_TOO_LARGE,
    blocker::SPEC_LABEL_TOO_LONG,
    blocker::SPEC_LABEL_CONTROL_CHARS,
    blocker::SPEC_SLOT_NOT_ALLOWED,
    blocker::SPEC_DERIVED_TRACE_INVALID,
    blocker::SPEC_NATURE_MISMATCH,
    warning::SPEC_NODE_WITHOUT_EDGE,
    warning::SPEC_DUPLICATE_LABEL,
    warning::SPEC_DANGLING_EDGE,
    warning::SPEC_LAYOUT_HINT_CONFLICT,
    warning::SPEC_FOCUS_UNMATCHED,
    warning::SPEC_LENS_UNMATCHED,
    info::SPEC_REGISTRY_VERSION_STALE,
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn codes_are_unique() {
        let set: HashSet<&str> = ALL_CODES.iter().copied().collect();
        assert_eq!(set.len(), ALL_CODES.len(), "问题码不得重复");
    }

    #[test]
    fn codes_follow_level_prefix() {
        for code in ALL_CODES {
            assert!(
                code.starts_with("E_") || code.starts_with("W_") || code.starts_with("I_"),
                "问题码必须带级别前缀：{code}"
            );
        }
    }
}
