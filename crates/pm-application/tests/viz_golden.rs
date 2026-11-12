//! Golden Tests：固定 spec 样例 → 校验器输出快照比对。
//!
//! 依据：12 号 V0-1 设计 §8.1。仓内**无** insta / expect-test 等快照 crate，
//! 因此采用**手写文件比对**：
//! - 语料：`crates/pm-application/tests/viz_golden/cases/*.json`（输入 spec）
//! - 期望：`crates/pm-application/tests/viz_golden/expected/*.json`（规范化问题列表）
//!
//! 比对粒度：只比 `{code, severity, at}` 三元组序列，**不比 message 文案**——
//! 文案调整不应导致快照失效（文案由人工评审保障）。
//!
//! 这组测试**与任何渲染器无关**（不依赖 SeeCode / G6）——它是防供应商绑定的真正关键。

use std::path::{Path, PathBuf};

use pm_domain::visualization::{
    codes, validate, Budget, NodeKindDefinition, ObjectResolver, RelationDefinition,
    RelationRegistry, RelationStatus, Severity, SourceRef, ValidationContext, VizSpec,
    SPEC_VERSION,
};

/// 全部对象可见、关系皆存在的解析器（Golden 语料聚焦校验规则本身）。
struct EverythingVisible;

impl ObjectResolver for EverythingVisible {
    fn exists_and_visible(&self, _d: &str, _t: &str, _i: &str) -> bool {
        true
    }
    fn relation_exists(&self, _f: &str, _t: &str, _r: &str) -> bool {
        true
    }
}

/// 除指定 id 外全部可见的解析器（用于「来源对象不可见」用例）。
struct HiddenOne<'a> {
    hidden: &'a str,
}

impl ObjectResolver for HiddenOne<'_> {
    fn exists_and_visible(&self, _d: &str, _t: &str, id: &str) -> bool {
        id != self.hidden
    }
    fn relation_exists(&self, _f: &str, _t: &str, _r: &str) -> bool {
        true
    }
}

/// 词表：与真源同构的最小集合（active 状态，便于语料覆盖"关系未登记/类型不符"）。
fn registry() -> RelationRegistry {
    let relation = |key: &str, status: RelationStatus, source_type: &str, target_type: &str| {
        RelationDefinition {
            key: key.to_string(),
            status,
            domain: "design".to_string(),
            owner_role: "design-owner".to_string(),
            source: "design.design_requirement".to_string(),
            source_type: source_type.to_string(),
            target_type: target_type.to_string(),
            direction: "directed".to_string(),
            semantics: "Golden 语料用".to_string(),
            governance_level: Some("L2".to_string()),
            introduced_in: Some("1.0".to_string()),
            signoff_date: Some("2026-11-12".to_string()),
            signoff_record: Some("golden".to_string()),
            view_alias: false,
            reverse_view_of: None,
            synthetic: false,
            business_fact: true,
            type_key_source: None,
            ref_role_filter: None,
            cardinality: None,
        }
    };
    RelationRegistry {
        version: "1".to_string(),
        relations: vec![
            relation(
                "design.covers_requirement",
                RelationStatus::Active,
                "design_revision",
                "requirement_revision",
            ),
            relation(
                "task.implements_requirement",
                RelationStatus::Active,
                "task",
                "requirement_revision",
            ),
            relation(
                "task.depends_on_task",
                RelationStatus::Draft,
                "task",
                "task",
            ),
            // Golden 夹具专用：同类型节点之间的测试边，避免语义噪音。
            relation(
                "test.peer",
                RelationStatus::Active,
                "requirement_revision",
                "requirement_revision",
            ),
        ],
        node_kinds: vec![
            NodeKindDefinition {
                key: "requirement_revision".to_string(),
                semantics: "需求修订".to_string(),
                domain: "delivery".to_string(),
                owner_role: "delivery-owner".to_string(),
                status: RelationStatus::Active,
            },
            NodeKindDefinition {
                key: "design_revision".to_string(),
                semantics: "设计修订".to_string(),
                domain: "design".to_string(),
                owner_role: "design-owner".to_string(),
                status: RelationStatus::Active,
            },
            NodeKindDefinition {
                key: "task".to_string(),
                semantics: "任务".to_string(),
                domain: "delivery".to_string(),
                owner_role: "delivery-owner".to_string(),
                status: RelationStatus::Active,
            },
        ],
    }
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("viz_golden")
}

fn cases_dir() -> PathBuf {
    golden_dir().join("cases")
}

fn expected_dir() -> PathBuf {
    golden_dir().join("expected")
}

/// 尺寸上限：默认按 `SpecLimits::default()`；payload 用例改用更小的上限，
/// 以便用小语料触发同一问题码（`limits` 本就是注入参数，见 `ValidationContext`）。
/// 按用例名装配上下文（语料只描述 Spec，外部依赖在此注入）。
fn context_for<'a>(
    case_name: &str,
    registry: &'a RelationRegistry,
    everything: &'a EverythingVisible,
    hidden: &'a HiddenOne<'a>,
) -> ValidationContext<'a> {
    let (resolver, current_version): (&dyn ObjectResolver, Option<&str>) =
        if case_name.starts_with("20_unknown_object_ref") {
            (hidden, None)
        } else if case_name.starts_with("21_registry_version_stale") {
            (everything, Some("1"))
        } else {
            (everything, None)
        };
    ValidationContext {
        registry,
        resolver,
        limits: limits_for(case_name),
        allowed_slots: &[],
        current_registry_version: current_version,
    }
}

fn limits_for(case_name: &str) -> pm_domain::visualization::SpecLimits {
    if case_name.starts_with("16_payload_too_large") {
        pm_domain::visualization::SpecLimits {
            max_payload_bytes: 2048,
            ..Default::default()
        }
    } else {
        Default::default()
    }
}

/// 规范化后的单条问题（只保留稳定字段）。
#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct GoldenProblem {
    code: String,
    severity: String,
    at: String,
}

fn severity_name(severity: Severity) -> String {
    match severity {
        Severity::Info => "info".to_string(),
        Severity::Warning => "warning".to_string(),
        Severity::Blocker => "blocker".to_string(),
    }
}

fn normalize(report: &pm_domain::visualization::ValidationReport) -> Vec<GoldenProblem> {
    report
        .problems
        .iter()
        .map(|p| GoldenProblem {
            code: p.code.clone(),
            severity: severity_name(p.severity()),
            at: p.at.clone(),
        })
        .collect()
}

/// 收集语料文件（按文件名排序，保证比对顺序确定）。
fn collect_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .expect("语料目录存在")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    files
}

#[test]
fn golden_cases_match_expected_snapshots() {
    let registry = registry();
    let resolver = EverythingVisible;
    let cases = collect_files(&cases_dir());
    assert!(!cases.is_empty(), "Golden 语料不得为空");

    for case_path in cases {
        let case_name = case_path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("语料文件名");
        let raw = std::fs::read_to_string(&case_path).expect("读取语料");
        let spec: VizSpec = serde_json::from_str(&raw).expect("语料必须是合法 VizSpec");

        let hidden = HiddenOne { hidden: "n1" };
        let ctx = context_for(case_name, &registry, &resolver, &hidden);
        let actual = normalize(&validate(&spec, &ctx));

        let expected_path = expected_dir().join(format!("{case_name}.json"));
        let expected_raw = std::fs::read_to_string(&expected_path).unwrap_or_else(|e| {
            panic!("缺少期望快照 {}: {e}", expected_path.display())
        });
        let expected: Vec<GoldenProblem> =
            serde_json::from_str(&expected_raw).expect("期望快照必须是合法 JSON 数组");

        assert_eq!(
            actual,
            expected,
            "Golden 语料 {case_name} 的校验结果与快照不一致"
        );
    }
}

#[test]
fn every_case_exercises_at_least_one_problem() {
    let registry = registry();
    let resolver = EverythingVisible;
    for case_path in collect_files(&cases_dir()) {
        let case_name = case_path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("语料文件名");
        let raw = std::fs::read_to_string(&case_path).expect("读取语料");
        let spec: VizSpec = serde_json::from_str(&raw).expect("语料必须是合法 VizSpec");
        let hidden = HiddenOne { hidden: "n1" };
        let ctx = context_for(case_name, &registry, &resolver, &hidden);
        let report = validate(&spec, &ctx);
        assert!(
            !report.problems.is_empty(),
            "语料 {case_name} 应至少触发一个问题（否则它不构成回归保护）"
        );
    }
}

#[test]
fn blocker_codes_are_all_covered_by_golden_cases() {
    // 每个阻断级问题码至少被一条语料覆盖，防止规则被删掉而无人察觉。
    let registry = registry();
    let resolver = EverythingVisible;
    let mut covered: std::collections::HashSet<String> = std::collections::HashSet::new();

    for case_path in collect_files(&cases_dir()) {
        let case_name = case_path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("语料文件名");
        let raw = std::fs::read_to_string(&case_path).expect("读取语料");
        let spec: VizSpec = serde_json::from_str(&raw).expect("语料必须是合法 VizSpec");
        let hidden = HiddenOne { hidden: "n1" };
        let ctx = context_for(case_name, &registry, &resolver, &hidden);
        for problem in validate(&spec, &ctx).problems {
            covered.insert(problem.code.clone());
        }
    }

    let all = codes::ALL_CODES;
    let uncovered: Vec<&str> = all
        .iter()
        .filter(|code| !covered.contains(**code))
        .copied()
        .collect();
    assert!(
        uncovered.is_empty(),
        "以下问题码未被任何 Golden 语料覆盖：{uncovered:?}"
    );
}

/// 供语料构造使用的最小合法 spec（测试辅助）。
#[allow(dead_code)]
fn minimal_spec() -> VizSpec {
    VizSpec {
        spec_version: SPEC_VERSION.to_string(),
        slot_key: "golden".to_string(),
        view: pm_domain::visualization::VizView {
            kind: "trace-graph".to_string(),
            focus: vec![],
            lenses: vec![],
            layout_hint: None,
        },
        nodes: vec![pm_domain::visualization::VizNode {
            id: "n1".to_string(),
            label: "节点".to_string(),
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
            budget: Budget::Balanced,
            nature: None,
            registry_version: Some("1".to_string()),
            provenance: None,
        },
    }
}
