//! SeeCode Adapter 端到端测试（**真 Node + 真 SeeCode 出图**）。
//!
//! 不是 mock：真实调用 `tools/seecode/scripts/seecode.mjs` 完成
//! spec → HTML → SVG 两步渲染，再经 `inspect_svg` 静态化检查。
//!
//! 覆盖 M-2 与 AC-21 的关键断言：
//! - 产物是**静态 SVG**（无脚本 / 无外链 / 字体内嵌）→ 静态化检查通过
//! - `renderer_id/version` 可追溯
//! - 八项 Adapter 契约断言（`verify()`）全绿
//! - 空 spec / 超能力输入被**显式拒绝**（不产残缺图）

use std::path::PathBuf;

use pm_application::visualization::adapter_contract::{verify, OutputFormat, RenderAdapter};
use pm_application::visualization::artifact_safety::inspect_svg;
use pm_domain::visualization::{
    Budget, Nature, SourceRef, VizEdge, VizMeta, VizNode, VizSpec, VizView, SPEC_VERSION,
};
use pm_infrastructure::viz_seecode_adapter::SeeCodeAdapter;

/// 仓库根下的 SeeCode CLI（测试的工作目录是 crate 目录，需上溯）。
fn cli_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("seecode")
        .join("scripts")
        .join("seecode.mjs")
}

fn trace_spec() -> VizSpec {
    let node = |id: &str, kind: &str, label: &str, status: Option<&str>| VizNode {
        id: id.to_string(),
        label: label.to_string(),
        kind: kind.to_string(),
        status: status.map(str::to_string),
        nature: Some(Nature::Business),
        source_ref: Some(SourceRef::new("delivery", kind, id)),
        derived: None,
    };
    let edge = |from: &str, to: &str, relation: &str| VizEdge {
        id: format!("{from}->{to}"),
        from: from.to_string(),
        to: to.to_string(),
        relation: relation.to_string(),
        label: None,
        nature: Some(Nature::Business),
        derived: None,
    };
    VizSpec {
        spec_version: SPEC_VERSION.to_string(),
        slot_key: "req-trace:adapter-e2e".to_string(),
        view: VizView {
            kind: "trace-graph".to_string(),
            focus: vec!["req-1".to_string()],
            lenses: vec![],
            layout_hint: Some("layered-lr".to_string()),
        },
        nodes: vec![
            node(
                "req-1",
                "requirement_revision",
                "批量导入执行器",
                Some("approved"),
            ),
            node(
                "design-1",
                "design_revision",
                "导入编排设计",
                Some("approved"),
            ),
            node("task-1", "task", "实现导入批处理", Some("ready")),
        ],
        edges: vec![
            edge("design-1", "req-1", "design.covers_requirement"),
            edge("task-1", "req-1", "task.implements_requirement"),
        ],
        meta: VizMeta {
            title: Some("需求追踪（Adapter 端到端）".to_string()),
            lang: Some("zh-CN".to_string()),
            budget: Budget::Balanced,
            nature: Some(Nature::Business),
            registry_version: Some("1".to_string()),
            provenance: None,
        },
    }
}

#[test]
fn cli_is_vendored() {
    assert!(
        cli_path().exists(),
        "SeeCode CLI 应已 vendor：{}",
        cli_path().display()
    );
}

#[test]
fn real_render_produces_static_svg() {
    let adapter = SeeCodeAdapter::new(cli_path());
    let output = adapter
        .render(&trace_spec(), OutputFormat::Svg)
        .expect("真实渲染应成功");

    assert_eq!(output.format, OutputFormat::Svg);
    assert!(output.content.contains("<svg"), "产物应是 SVG");
    // 中文标签应出现在产物中（zh-CN + CJK 换行支持）
    assert!(
        output.content.contains("批量导入执行器"),
        "中文标签应渲染进产物"
    );

    // **AC-7′：静态化检查必须通过**（无 script / 无外链 / 无事件属性）
    let report = inspect_svg(&output.content);
    assert!(
        report.is_clean(),
        "真实 SeeCode 产物必须通过静态化检查：{}",
        report.summary()
    );
}

#[test]
fn real_render_is_deterministic() {
    let adapter = SeeCodeAdapter::new(cli_path());
    let first = adapter
        .render(&trace_spec(), OutputFormat::Svg)
        .expect("第一次渲染");
    let second = adapter
        .render(&trace_spec(), OutputFormat::Svg)
        .expect("第二次渲染");
    assert_eq!(
        first.content, second.content,
        "同输入必须同输出（契约第 8 项，回归基线的前提）"
    );
}

#[test]
fn adapter_passes_full_contract() {
    let adapter = SeeCodeAdapter::new(cli_path());
    let checks = verify(&adapter, &[trace_spec()]);
    for check in &checks {
        assert!(
            check.passed,
            "契约断言 {} 失败：{}",
            check.name, check.detail
        );
    }
    // 八项断言全在
    assert!(checks.len() >= 8, "契约应含八项断言，实际 {}", checks.len());
}

#[test]
fn empty_spec_is_rejected_not_crashed() {
    let adapter = SeeCodeAdapter::new(cli_path());
    let mut spec = trace_spec();
    spec.nodes.clear();
    spec.edges.clear();
    let result = adapter.render(&spec, OutputFormat::Svg);
    assert!(result.is_err(), "空 spec 应显式拒绝（不产残缺图）");
}

#[test]
fn html_format_is_not_offered() {
    let adapter = SeeCodeAdapter::new(cli_path());
    assert_eq!(adapter.supported_formats(), &[OutputFormat::Svg]);
    assert!(adapter.render(&trace_spec(), OutputFormat::Html).is_err());
}

#[test]
fn missing_cli_is_reported_as_unavailable() {
    let adapter = SeeCodeAdapter::new("/nonexistent/seecode.mjs");
    let result = adapter.render(&trace_spec(), OutputFormat::Svg);
    assert!(
        result.is_err(),
        "CLI 缺失应可失败处理（不 panic、不阻断链路）"
    );
}
