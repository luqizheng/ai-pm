//! 图形性质：正式业务事实 vs AI 生成的解释性内容。
//!
//! 依据（docs/visualization/ai-pm-viz-03-visualization-spec-v1.md §3、14 号 §6）：
//! - `business`：**真实业务事实**，节点与边引用库内可校验的真实对象与关系。
//! - `explanatory`：AI 对话中生成的**假设 / 示意**内容。恒显「AI 生成 · 非正式认知」，
//!   不得发布到业务槽位（由 `viz.artifact` 的 `nature_publish_check` 在库级强制）。
//!
//! 注意：本枚举表达的是**事实性质**维度，与 `DerivedTrace`（推导方式维度）正交——
//! 一条边可以既是 business（事实性）、又是 `direct=false`（间接推导）。

/// 图形性质（事实性质维度）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Nature {
    /// 真实业务事实：引用库内可校验的真实对象与关系。
    Business,
    /// AI 生成的解释性内容：假设、示意或待确认的分析结论。
    Explanatory,
}

impl Nature {
    /// 全部取值（与词表一一对应）。
    pub const ALL: [Self; 2] = [Self::Business, Self::Explanatory];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Business => "business",
            Self::Explanatory => "explanatory",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|n| n.as_str() == value)
    }

    /// 解释性内容是否可发布为业务事实：**否**。
    ///
    /// 供应用层在发布路径前置校验；库级另有 `viz_artifact_nature_publish_check` 兜底。
    pub fn is_publishable_as_business(self) -> bool {
        matches!(self, Self::Business)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_and_parse_round_trip() {
        for nature in Nature::ALL {
            assert_eq!(Nature::parse(nature.as_str()), Some(nature));
        }
    }

    #[test]
    fn serde_uses_snake_case() {
        let json = serde_json::to_string(&Nature::Explanatory).expect("serialize");
        assert_eq!(json, "\"explanatory\"");
        let back: Nature = serde_json::from_str("\"business\"").expect("deserialize");
        assert_eq!(back, Nature::Business);
    }

    #[test]
    fn explanatory_is_not_publishable_as_business() {
        assert!(Nature::Business.is_publishable_as_business());
        assert!(!Nature::Explanatory.is_publishable_as_business());
    }

    #[test]
    fn unknown_value_parses_to_none() {
        assert_eq!(Nature::parse("hypothetical"), None);
    }
}
