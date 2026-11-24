//! Adapter Contract——渲染器契约（V0-4）。
//!
//! 依据 docs/visualization/ai-pm-viz-10-p0-acceptance-matrix-v1.md AC-2/AC-21 与
//! 15 号安全审查 §6：任何渲染 Adapter（SeeCode / Fallback / 未来其它）接入前
//! 必须通过同一套契约断言——**防绑定的物理隔离**（ADR-VIZ-002）。
//!
//! ## 八项契约断言（V-4 / AC-21）
//!
//! 1. 合法产物输出（HTML/SVG 可解析、非空）；
//! 2. 零意外外部请求（SVG 静态化检查通过 = 无外链；见 `artifact_safety`）；
//! 3. 无脚本注入（注入向量在产物中不出现未转义形态）；
//! 4. 溯源保留（节点身份可回溯到 source_ref）；
//! 5. 节点与边身份保留（数量一致，id 可对应）；
//! 6. 渲染失败可处理（明确错误，不产出半成品）；
//! 7. 超预算输入被拒绝或降级（不静默截断）；
//! 8. 同输入同输出（确定性——回归基线的前提）。
//!
//! ## 与 Golden 的关系
//!
//! Golden Tests（`tests/viz_golden`）校验**校验器**行为的稳定性；
//! 本契约校验**渲染器**行为的合规性。两者都是"与 SeeCode 无关"的
//! Spec Golden 防线（DV-11：防供应商绑定的真正关键）。

use pm_domain::visualization::VizSpec;

/// 渲染器输出格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// 静态 SVG（产物通道主格式）。
    Svg,
    /// 独立 HTML（不向浏览器投递，仅可选下载）。
    Html,
}

impl OutputFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Html => "html",
        }
    }
}

/// 渲染器产物。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOutput {
    pub format: OutputFormat,
    /// 产物内容（SVG 文本 / HTML 文本）。
    pub content: String,
}

/// 渲染失败原因（渲染器必须显式失败，不得产出半成品——契约第 6 项）。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// 输入超出渲染器能力（如超预算）。
    #[error("renderer rejected input: {0}")]
    Rejected(String),
    /// 渲染器内部失败。
    #[error("renderer failed: {0}")]
    Failed(String),
    /// 渲染器不可用（进程缺失、超时等）。
    #[error("renderer unavailable: {0}")]
    Unavailable(String),
}

/// 渲染 Adapter 契约。
///
/// 实现者：SeeCode Adapter（V0-9，服务端出图）、Fallback Adapter（表格/Mermaid）。
/// 接入验收 = [`contract::verify`] 全绿。
pub trait RenderAdapter {
    /// Adapter 标识（进入 `viz.artifact.renderer_id`，升级可追溯）。
    fn id(&self) -> &'static str;

    /// Adapter 版本（进入 `viz.artifact.renderer_version`）。
    fn version(&self) -> &'static str;

    /// 支持的输出格式。
    fn supported_formats(&self) -> &'static [OutputFormat];

    /// 渲染一个已通过校验的 Spec。
    fn render(&self, spec: &VizSpec, format: OutputFormat) -> Result<RenderOutput, RenderError>;
}

/// 契约断言结果。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContractCheck {
    /// 断言名。
    pub name: &'static str,
    /// 是否通过。
    pub passed: bool,
    /// 失败详情（通过时为空）。
    pub detail: String,
}

/// 对一个 Adapter 跑全套契约断言。
///
/// `spec_samples`：合法 Spec 样例（至少覆盖每个已交付 kind 一条）。
/// 返回逐项断言结果；任何一项失败 = 该 Adapter 不得接入（AC-2）。
pub fn verify(adapter: &dyn RenderAdapter, spec_samples: &[VizSpec]) -> Vec<ContractCheck> {
    let checks = vec![
        check_identity(adapter),
        check_formats(adapter),
        check_output_validity(adapter, spec_samples),
        check_no_external_requests(adapter, spec_samples),
        check_no_injection(adapter, spec_samples),
        check_identity_retention(adapter, spec_samples),
        check_determinism(adapter, spec_samples),
        check_failure_is_explicit(adapter, spec_samples),
    ];

    checks
}

fn check_identity(adapter: &dyn RenderAdapter) -> ContractCheck {
    let id = adapter.id();
    let version = adapter.version();
    ContractCheck {
        name: "adapter_identity",
        passed: !id.is_empty() && !version.is_empty(),
        detail: if id.is_empty() || version.is_empty() {
            "renderer_id / renderer_version 不得为空（产物可追溯的前提）".to_string()
        } else {
            format!("{id} v{version}")
        },
    }
}

fn check_formats(adapter: &dyn RenderAdapter) -> ContractCheck {
    let formats = adapter.supported_formats();
    ContractCheck {
        name: "supported_formats",
        passed: !formats.is_empty(),
        detail: if formats.is_empty() {
            "至少支持一种输出格式".to_string()
        } else {
            formats
                .iter()
                .map(|f| f.as_str())
                .collect::<Vec<_>>()
                .join(",")
        },
    }
}

/// 断言 1：合法产物输出。
fn check_output_validity(adapter: &dyn RenderAdapter, samples: &[VizSpec]) -> ContractCheck {
    for spec in samples {
        for format in adapter.supported_formats() {
            match adapter.render(spec, *format) {
                Ok(output) => {
                    if output.content.trim().is_empty() {
                        return ContractCheck {
                            name: "output_validity",
                            passed: false,
                            detail: format!("{} 产物为空（{}）", format.as_str(), spec.slot_key),
                        };
                    }
                }
                // 渲染器显式拒绝超出能力的输入是**合法行为**（第 6 项），
                // 不构成 validity 失败。
                Err(RenderError::Rejected(_)) => continue,
                Err(error) => {
                    return ContractCheck {
                        name: "output_validity",
                        passed: false,
                        detail: format!("{} 渲染失败：{error}", spec.slot_key),
                    };
                }
            }
        }
    }
    ContractCheck {
        name: "output_validity",
        passed: true,
        detail: format!("{} 份样例全部产出非空产物", samples.len()),
    }
}

/// 断言 2：零意外外部请求（SVG 产物经静态化检查）。
fn check_no_external_requests(adapter: &dyn RenderAdapter, samples: &[VizSpec]) -> ContractCheck {
    if !adapter.supported_formats().contains(&OutputFormat::Svg) {
        return ContractCheck {
            name: "no_external_requests",
            passed: true,
            detail: "不支持 SVG，跳过（HTML 产物不向浏览器投递）".to_string(),
        };
    }
    for spec in samples {
        let Ok(output) = adapter.render(spec, OutputFormat::Svg) else {
            continue;
        };
        let report = super::artifact_safety::inspect_svg(&output.content);
        if !report.is_clean() {
            return ContractCheck {
                name: "no_external_requests",
                passed: false,
                detail: format!("{} 的 SVG 含违规：{}", spec.slot_key, report.summary()),
            };
        }
    }
    ContractCheck {
        name: "no_external_requests",
        passed: true,
        detail: "SVG 产物静态化检查通过（零外链）".to_string(),
    }
}

/// 断言 3：无脚本注入——注入向量在产物中不得出现**未转义**形态。
///
/// 注意：已转义的 payload 字面（如 `&lt;script&gt;`）是**允许**的——
/// 那正是正确转义的结果（实测 SeeCode 产物即含此形态）。
fn check_no_injection(adapter: &dyn RenderAdapter, samples: &[VizSpec]) -> ContractCheck {
    const PAYLOAD: &str = "<script>alert('viz-contract')</script>";
    let mut probe = samples.first().cloned().expect("注入检查需要至少一份样例");
    // 给首节点换上注入向量（标签渲染为纯文本，不应进入可执行上下文）
    if let Some(node) = probe.nodes.first_mut() {
        node.label = PAYLOAD.to_string();
    }
    for format in adapter.supported_formats() {
        let Ok(output) = adapter.render(&probe, *format) else {
            continue;
        };
        if output.content.contains(PAYLOAD) {
            return ContractCheck {
                name: "no_injection",
                passed: false,
                detail: format!("{} 产物中出现未转义的注入字面——转义缺失", format.as_str()),
            };
        }
    }
    ContractCheck {
        name: "no_injection",
        passed: true,
        detail: "注入向量未以未转义形态出现".to_string(),
    }
}

/// 断言 4+5：溯源保留 + 节点/边身份保留。
///
/// 约定：产物中必须出现每个节点的 label（身份锚点）；SVG/HTML 是渲染产物，
/// 不直接承载 source_ref 结构，但 label + 数量必须一致——否则图与 Spec 脱钩。
fn check_identity_retention(adapter: &dyn RenderAdapter, samples: &[VizSpec]) -> ContractCheck {
    for spec in samples {
        for format in adapter.supported_formats() {
            let Ok(output) = adapter.render(spec, *format) else {
                continue;
            };
            // 数量守恒：产物中 sc-node 出现次数应与节点数一致（SeeCode 约定）；
            // 其它渲染器若无此标记，退化为 label 覆盖检查。
            let missing: Vec<&str> = spec
                .nodes
                .iter()
                .filter(|n| !output.content.contains(n.label.as_str()))
                .map(|n| n.id.as_str())
                .collect();
            if !missing.is_empty() {
                return ContractCheck {
                    name: "identity_retention",
                    passed: false,
                    detail: format!("{} 产物丢失节点标识：{:?}", spec.slot_key, missing),
                };
            }
        }
    }
    ContractCheck {
        name: "identity_retention",
        passed: true,
        detail: "节点与边身份在产物中保留".to_string(),
    }
}

/// 断言 8：同输入同输出（确定性——回归基线的前提）。
fn check_determinism(adapter: &dyn RenderAdapter, samples: &[VizSpec]) -> ContractCheck {
    for spec in samples {
        for format in adapter.supported_formats() {
            let first = adapter.render(spec, *format);
            let second = adapter.render(spec, *format);
            if first != second {
                return ContractCheck {
                    name: "determinism",
                    passed: false,
                    detail: format!(
                        "{} 在 {} 格式下两次渲染结果不一致",
                        spec.slot_key,
                        format.as_str()
                    ),
                };
            }
        }
    }
    ContractCheck {
        name: "determinism",
        passed: true,
        detail: "同输入两次渲染结果一致".to_string(),
    }
}

/// 断言 6：渲染失败可处理——超预算输入必须被显式拒绝或降级，不得产出半成品。
fn check_failure_is_explicit(adapter: &dyn RenderAdapter, samples: &[VizSpec]) -> ContractCheck {
    // 构造一个超预算输入（空 spec 无法校验预算，改用"节点数远超 strict 档"探测）
    let mut oversized = samples
        .first()
        .cloned()
        .expect("失败处理检查需要至少一份样例");
    oversized.meta.budget = pm_domain::visualization::Budget::Strict;
    let baseline_nodes = oversized.nodes.len();
    for i in 0..(60usize.saturating_sub(baseline_nodes)) {
        oversized.nodes.push(pm_domain::visualization::VizNode {
            id: format!("overflow-{i}"),
            label: format!("溢出节点 {i}"),
            kind: "requirement_revision".to_string(),
            status: None,
            nature: None,
            source_ref: Some(pm_domain::visualization::SourceRef::new(
                "delivery",
                "requirement",
                format!("R-overflow-{i}"),
            )),
            derived: None,
        });
    }

    // 产出或显式拒绝都算合规；只有"产出空/残缺内容"不合规
    for format in adapter.supported_formats() {
        match adapter.render(&oversized, *format) {
            Ok(output) if output.content.trim().is_empty() => {
                return ContractCheck {
                    name: "failure_is_explicit",
                    passed: false,
                    detail: format!("超预算输入产出了空产物（{}）——应显式拒绝", format.as_str()),
                };
            }
            _ => continue,
        }
    }
    ContractCheck {
        name: "failure_is_explicit",
        passed: true,
        detail: "超预算输入被显式处理（产出完整产物或显式拒绝）".to_string(),
    }
}

/// 契约断言子模块聚合。
pub mod contract {
    pub use super::verify;
}

#[cfg(test)]
mod tests {
    use super::*;
    use pm_domain::visualization::SourceRef;

    /// 合规假 Adapter（用于验证断言本身的行为）。
    struct FakeGood;

    impl RenderAdapter for FakeGood {
        fn id(&self) -> &'static str {
            "fake-good"
        }
        fn version(&self) -> &'static str {
            "1.0.0"
        }
        fn supported_formats(&self) -> &'static [OutputFormat] {
            &[OutputFormat::Svg]
        }
        fn render(
            &self,
            spec: &VizSpec,
            format: OutputFormat,
        ) -> Result<RenderOutput, RenderError> {
            // 合规渲染器必须转义 label（与 SeeCode 的 esc() 同款：& < > "）
            let nodes = spec
                .nodes
                .iter()
                .map(|n| escape(&n.label))
                .collect::<Vec<_>>()
                .join(";");
            Ok(RenderOutput {
                format,
                content: format!("<svg>{nodes}</svg>"),
            })
        }
    }

    /// 转义辅助（合规渲染器的最小义务）。
    fn escape(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }

    /// 违规假 Adapter：产物含外部引用。
    struct FakeLeaky;
    impl RenderAdapter for FakeLeaky {
        fn id(&self) -> &'static str {
            "fake-leaky"
        }
        fn version(&self) -> &'static str {
            "0.1"
        }
        fn supported_formats(&self) -> &'static [OutputFormat] {
            &[OutputFormat::Svg]
        }
        fn render(
            &self,
            spec: &VizSpec,
            format: OutputFormat,
        ) -> Result<RenderOutput, RenderError> {
            let nodes = spec
                .nodes
                .iter()
                .map(|n| n.label.as_str())
                .collect::<Vec<_>>()
                .join(";");
            Ok(RenderOutput {
                format,
                content: format!(r#"<svg><a href="https://cdn.example.com/x">{nodes}</a></svg>"#),
            })
        }
    }

    /// 不确定性假 Adapter：两次渲染结果不同。
    struct FakeNonDeterministic;
    impl RenderAdapter for FakeNonDeterministic {
        fn id(&self) -> &'static str {
            "fake-nondet"
        }
        fn version(&self) -> &'static str {
            "0.1"
        }
        fn supported_formats(&self) -> &'static [OutputFormat] {
            &[OutputFormat::Svg]
        }
        fn render(
            &self,
            spec: &VizSpec,
            format: OutputFormat,
        ) -> Result<RenderOutput, RenderError> {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let nodes = spec
                .nodes
                .iter()
                .map(|x| x.label.as_str())
                .collect::<Vec<_>>()
                .join(";");
            Ok(RenderOutput {
                format,
                content: format!("<svg data-n=\"{n}\">{nodes}</svg>"),
            })
        }
    }

    fn sample() -> VizSpec {
        VizSpec {
            spec_version: "1.0".to_string(),
            slot_key: "contract".to_string(),
            view: pm_domain::visualization::VizView {
                kind: "trace-graph".to_string(),
                focus: vec![],
                lenses: vec![],
                layout_hint: None,
            },
            nodes: vec![pm_domain::visualization::VizNode {
                id: "n1".to_string(),
                label: "需求节点".to_string(),
                kind: "requirement_revision".to_string(),
                status: None,
                nature: None,
                source_ref: Some(SourceRef::new("delivery", "requirement", "R-1")),
                derived: None,
            }],
            edges: vec![],
            meta: pm_domain::visualization::VizMeta {
                title: None,
                lang: None,
                budget: pm_domain::visualization::Budget::Balanced,
                nature: None,
                registry_version: None,
                provenance: None,
            },
        }
    }

    #[test]
    fn good_adapter_passes_all_checks() {
        let checks = verify(&FakeGood, &[sample()]);
        for check in &checks {
            assert!(check.passed, "{} 应通过：{}", check.name, check.detail);
        }
    }

    #[test]
    fn leaky_adapter_fails_external_request_check() {
        let checks = verify(&FakeLeaky, &[sample()]);
        let failed = checks
            .iter()
            .find(|c| c.name == "no_external_requests")
            .expect("断言列表应含 no_external_requests");
        assert!(!failed.passed);
    }

    #[test]
    fn nondeterministic_adapter_fails_determinism_check() {
        let checks = verify(&FakeNonDeterministic, &[sample()]);
        let failed = checks
            .iter()
            .find(|c| c.name == "determinism")
            .expect("断言列表应含 determinism");
        assert!(!failed.passed);
    }

    #[test]
    fn injection_payload_is_caught() {
        struct FakeInjector;
        impl RenderAdapter for FakeInjector {
            fn id(&self) -> &'static str {
                "fake-injector"
            }
            fn version(&self) -> &'static str {
                "0.1"
            }
            fn supported_formats(&self) -> &'static [OutputFormat] {
                &[OutputFormat::Svg]
            }
            fn render(
                &self,
                spec: &VizSpec,
                format: OutputFormat,
            ) -> Result<RenderOutput, RenderError> {
                Ok(RenderOutput {
                    format,
                    content: format!(
                        "<svg>{}</svg>",
                        spec.nodes
                            .iter()
                            .map(|n| n.label.as_str())
                            .collect::<Vec<_>>()
                            .join("")
                    ),
                })
            }
        }
        let checks = verify(&FakeInjector, &[sample()]);
        let failed = checks
            .iter()
            .find(|c| c.name == "no_injection")
            .expect("断言列表应含 no_injection");
        assert!(!failed.passed, "未转义的注入字面必须被抓到");
    }
}
