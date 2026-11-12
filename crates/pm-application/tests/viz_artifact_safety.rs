//! 产物静态化守卫 —— **真实 SeeCode 产物**的验收（V0-6 安全基座）。
//!
//! 依据 docs/visualization/ai-pm-viz-15-v0-6-security-review-v1.md §6：
//! AC-7′ 要求入库产物中 `<script>` / `foreignObject` / 可执行 `on*` 属性 /
//! 外部引用计数均为 0。
//!
//! 夹具 `fixtures/seecode_real_artifact.svg` 是 **SeeCode v0.6.3 的真实产物**
//! （用含注入 payload 的 spec 渲染后导出的 SVG）——不是手写样例：
//! 真实产物里存在**已转义**的注入串（如 `&lt;img src=x onerror=1&gt;`），
//! 它们位于文本内容而非标签属性，**不得**被误判为事件属性违规。
//!
//! 这条夹具同时是「渲染器升级后重跑」的安全回归基线（换 SeeCode 版本时
//! 必须重跑本测试）。

use std::path::PathBuf;

use pm_application::visualization::inspect_svg;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("seecode_real_artifact.svg")
}

#[test]
fn real_seecode_artifact_passes_staticization() {
    let path = fixture();
    assert!(path.exists(), "夹具缺失：{}", path.display());
    let svg = std::fs::read_to_string(&path).expect("读取夹具");

    let report = inspect_svg(&svg);
    assert!(
        report.is_clean(),
        "真实产物未通过静态化检查：{}",
        report.summary()
    );
}

#[test]
fn real_artifact_has_no_external_references() {
    let svg = std::fs::read_to_string(fixture()).expect("读取夹具");
    let report = inspect_svg(&svg);
    assert!(
        !report.codes().contains("S_SVG_EXTERNAL_REFERENCE"),
        "真实产物含外部引用：{:?}",
        report.violations
    );
}

#[test]
fn real_artifact_does_not_trigger_event_attribute_false_positive() {
    // 回归防线：真实产物**确实**在文本内容里含 `onerror=` 字面（已转义）。
    // 若守卫改成全文正则，这里会失败——说明实现退化为误报版本。
    let svg = std::fs::read_to_string(fixture()).expect("读取夹具");
    assert!(
        svg.contains("onerror="),
        "夹具前提失效：真实产物应含已转义的 onerror= 字面"
    );
    let report = inspect_svg(&svg);
    assert!(
        !report.codes().contains("S_SVG_EVENT_ATTRIBUTE"),
        "文本内容中的转义串不得被判为事件属性：{:?}",
        report.violations
    );
}

#[test]
fn tampered_real_artifact_is_rejected() {
    // 在真实产物里注入一个 script 标签 → 必须被拦截（证明守卫对真实产物有效，
    // 而不只是对合成样例有效）。
    let svg = std::fs::read_to_string(fixture()).expect("读取夹具");
    let tampered = svg.replace("</svg>", "<script>alert(1)</script></svg>");
    assert_ne!(svg, tampered, "篡改未生效");
    let report = inspect_svg(&tampered);
    assert!(
        report.codes().contains("S_SVG_SCRIPT"),
        "篡改后的产物必须被拦截：{:?}",
        report.violations
    );
}
