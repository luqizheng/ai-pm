//! 确定性校验管线 L1–L4（`docs/slice-d/ai-pm-slice-d5-review-v1.md` §4）。
//!
//! 管线是**纯函数**：输入 = 已采集的事实（交付对账状态、集成事实、验收条目、
//! 测试证据、产物存在性探测），输出 = findings + 逐条裁决 + 总体 verdict。
//! 纯函数不碰数据库、不碰工作区——「镜像仓只读探测」的结果由调用方采集后作为
//! 输入喂进来，判定口径因此可被机械复现（同一输入必得同一输出）。
//!
//! 判定口径（L1–L4）：
//!
//! | 检查项 | 通过条件 | 失败产出 |
//! |---|---|---|
//! | L1 交付对账复核 | `delivery_state='delivered'` | `incomplete_delivery`（blocker） |
//! | L2 集成事实判定 | `integration_status='passed'` | `integration_failed`（blocker） |
//! | L3 逐条验收判定 | 每条确定性条目的证据齐备且通过 | `test_failed` / `artifact_missing`（blocker） |
//! | L4 证据一致性 | 声明与证据不矛盾 | `evidence_missing` / `evidence_conflict`（blocker） |
//!
//! **人工裁决不由管线做**：`manual` / `review` 条目不落裁决行，只落
//! `awaiting_manual` finding；终值行由审批端点在裁决事务内 INSERT（§2.5 修正）。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 总体结论三值（§2.1 `review_verdict` 的 CHECK 取值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Approved,
    ChangesRequested,
    Inconclusive,
}

impl ReviewVerdict {
    /// 库内取值。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::ChangesRequested => "changes_requested",
            Self::Inconclusive => "inconclusive",
        }
    }
}

/// 发现严重度三值（§2.2 `severity` 的 CHECK 取值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Blocker,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Blocker => "blocker",
        }
    }
}

/// 单条验收项的裁决值（§2.3 `verdict` 的 CHECK 取值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionVerdict {
    Passed,
    Failed,
    NotEvaluable,
    AwaitingManual,
}

impl CriterionVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::NotEvaluable => "not_evaluable",
            Self::AwaitingManual => "awaiting_manual",
        }
    }
}

/// 一条结构化发现（§2.2）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub finding_kind: String,
    pub severity: Severity,
    /// 可空：非条目类 finding（如交付对账失败）。
    pub criterion_id: Option<Uuid>,
    /// 「仓 + 路径 + 行段」自由文本（Q8.2 定位要素）。
    pub code_location: Option<String>,
    pub message: String,
    pub evidence_ref: Option<String>,
}

/// 验收条目快照（管线输入）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriterionSnapshot {
    pub criterion_id: Uuid,
    pub ordinal: i32,
    pub text: String,
    /// `manual` / `automated_test` / `artifact_exists` / `review`。
    pub verification_kind: String,
}

/// 一条测试证据（来自 `task_repository_commit.test_evidence.tests[]`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestEvidence {
    pub repository_id: Uuid,
    pub name: String,
    pub passed: bool,
    pub evidence_ref: Option<String>,
}

/// 产物存在性的一次只读探测结果（镜像仓内路径现查）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactProbe {
    pub repository_id: Uuid,
    pub path: String,
    pub exists: bool,
}

/// 管线输入：全部为**已采集的事实**，不含任何 I/O。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewInput {
    /// L1：交付对账视图的 `delivery_state`。
    pub delivery_state: String,
    /// L2/L4：Run 级集成事实（`passed` / `failed` / `not_run`）。
    pub integration_status: String,
    /// L3：验收条目快照（按 ordinal 升序）。
    pub criteria: Vec<CriterionSnapshot>,
    /// L3/L4：逐仓测试证据。
    pub tests: Vec<TestEvidence>,
    /// L3：产物存在性探测结果（路径 → 是否存在）。
    pub artifacts: Vec<ArtifactProbe>,
    /// L3：任务声明的交付物路径（`task.deliverables[].path`）。
    pub deliverable_paths: Vec<String>,
}

/// 单条验收项的判定结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriterionOutcome {
    pub criterion_id: Uuid,
    pub ordinal: i32,
    pub verification_kind: String,
    /// `None` = 人工裁决条目（管线不落裁决行，见模块注释）。
    pub verdict: Option<CriterionVerdict>,
    /// 证据原文（结论 ≠ 证据：两者分列）。
    pub evidence: serde_json::Value,
}

/// 管线输出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOutcome {
    pub verdict: ReviewVerdict,
    pub findings: Vec<ReviewFinding>,
    pub criteria: Vec<CriterionOutcome>,
}

impl ReviewOutcome {
    /// 是否存在待人工裁决的条目（`manual` / `review`）——完成判定器据此
    /// 走「需人工」分支（§5.1）。
    pub fn has_pending_manual(&self) -> bool {
        self.criteria
            .iter()
            .any(|outcome| outcome.verdict.is_none())
    }

    /// 阻断级发现数（用于事件 payload 与监控计数）。
    pub fn blocker_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == Severity::Blocker)
            .count()
    }
}

/// L3 产物判定：条目与交付物的关联规则。
///
/// 验收条目本身只有自由文本，没有指向交付物 id 的列。V1 采用**文本关联**：
/// 条目文本若提到某个交付物路径，则只校验该路径；否则校验全部交付物路径。
/// 校验口径 = 该路径在**至少一个**可写仓的镜像探测中存在。
fn artifacts_for_criterion<'a>(
    input: &'a ReviewInput,
    criterion: &CriterionSnapshot,
) -> Vec<&'a ArtifactProbe> {
    let mentioned: Vec<&String> = input
        .deliverable_paths
        .iter()
        .filter(|path| !path.is_empty() && criterion.text.contains(path.as_str()))
        .collect();
    let scoped: Vec<&ArtifactProbe> = input
        .artifacts
        .iter()
        .filter(|probe| mentioned.is_empty() || mentioned.contains(&&probe.path))
        .collect();
    scoped
}

/// 跑一次确定性校验管线。
pub fn evaluate(input: &ReviewInput) -> ReviewOutcome {
    let mut findings: Vec<ReviewFinding> = Vec::new();
    let mut outcomes: Vec<CriterionOutcome> = Vec::new();

    // ── L1 交付对账复核 ────────────────────────────────────────────────
    if input.delivery_state != "delivered" {
        findings.push(ReviewFinding {
            finding_kind: "incomplete_delivery".to_string(),
            severity: Severity::Blocker,
            criterion_id: None,
            code_location: None,
            message: format!(
                "delivery_state is `{}` but review requires `delivered`",
                input.delivery_state
            ),
            evidence_ref: None,
        });
    }

    // ── L2 集成事实判定 ────────────────────────────────────────────────
    if input.integration_status.as_str() == "failed" {
        findings.push(ReviewFinding {
            finding_kind: "integration_failed".to_string(),
            severity: Severity::Blocker,
            criterion_id: None,
            code_location: None,
            message: "integration_status is `failed`; no reviewer or human may complete this task"
                .to_string(),
            evidence_ref: None,
        });
    }

    let failed_tests: Vec<&TestEvidence> = input.tests.iter().filter(|test| !test.passed).collect();
    let automated_criteria: Vec<&CriterionSnapshot> = input
        .criteria
        .iter()
        .filter(|criterion| criterion.verification_kind == "automated_test")
        .collect();

    // ── L3 逐条验收判定 ────────────────────────────────────────────────
    for criterion in &input.criteria {
        match criterion.verification_kind.as_str() {
            "automated_test" => {
                let evidence = serde_json::json!({
                    "tests": input
                        .tests
                        .iter()
                        .map(|test| serde_json::json!({
                            "repository_id": test.repository_id.to_string(),
                            "name": test.name,
                            "passed": test.passed,
                            "evidence_ref": test.evidence_ref,
                        }))
                        .collect::<Vec<_>>(),
                });
                if input.tests.is_empty() {
                    // 声明了自动化验收却没有任何测试证据：证据缺失，按不通过处理
                    // （fail-closed）。
                    findings.push(ReviewFinding {
                        finding_kind: "evidence_missing".to_string(),
                        severity: Severity::Blocker,
                        criterion_id: Some(criterion.criterion_id),
                        code_location: None,
                        message: format!(
                            "criterion #{} requires automated tests but no test evidence was submitted",
                            criterion.ordinal
                        ),
                        evidence_ref: None,
                    });
                    outcomes.push(CriterionOutcome {
                        criterion_id: criterion.criterion_id,
                        ordinal: criterion.ordinal,
                        verification_kind: criterion.verification_kind.clone(),
                        verdict: Some(CriterionVerdict::Failed),
                        evidence,
                    });
                } else if failed_tests.is_empty() {
                    outcomes.push(CriterionOutcome {
                        criterion_id: criterion.criterion_id,
                        ordinal: criterion.ordinal,
                        verification_kind: criterion.verification_kind.clone(),
                        verdict: Some(CriterionVerdict::Passed),
                        evidence,
                    });
                } else {
                    for test in &failed_tests {
                        findings.push(ReviewFinding {
                            finding_kind: "test_failed".to_string(),
                            severity: Severity::Blocker,
                            criterion_id: Some(criterion.criterion_id),
                            code_location: Some(format!("{}:{}", test.repository_id, test.name)),
                            message: format!("test `{}` did not pass", test.name),
                            evidence_ref: test.evidence_ref.clone(),
                        });
                    }
                    outcomes.push(CriterionOutcome {
                        criterion_id: criterion.criterion_id,
                        ordinal: criterion.ordinal,
                        verification_kind: criterion.verification_kind.clone(),
                        verdict: Some(CriterionVerdict::Failed),
                        evidence,
                    });
                }
            }
            "artifact_exists" => {
                let probes = artifacts_for_criterion(input, criterion);
                let missing: Vec<&ArtifactProbe> = probes
                    .iter()
                    .copied()
                    .filter(|probe| !probe.exists)
                    .collect();
                let evidence = serde_json::json!({
                    "artifacts": probes
                        .iter()
                        .map(|probe| serde_json::json!({
                            "repository_id": probe.repository_id.to_string(),
                            "path": probe.path,
                            "exists": probe.exists,
                        }))
                        .collect::<Vec<_>>(),
                });
                if missing.is_empty() && !probes.is_empty() {
                    outcomes.push(CriterionOutcome {
                        criterion_id: criterion.criterion_id,
                        ordinal: criterion.ordinal,
                        verification_kind: criterion.verification_kind.clone(),
                        verdict: Some(CriterionVerdict::Passed),
                        evidence,
                    });
                } else {
                    for probe in &missing {
                        findings.push(ReviewFinding {
                            finding_kind: "artifact_missing".to_string(),
                            severity: Severity::Blocker,
                            criterion_id: Some(criterion.criterion_id),
                            code_location: Some(format!("{}:{}", probe.repository_id, probe.path)),
                            message: format!(
                                "artifact `{}` does not exist in the reviewed delivery",
                                probe.path
                            ),
                            evidence_ref: None,
                        });
                    }
                    if probes.is_empty() {
                        findings.push(ReviewFinding {
                            finding_kind: "artifact_missing".to_string(),
                            severity: Severity::Blocker,
                            criterion_id: Some(criterion.criterion_id),
                            code_location: None,
                            message: format!(
                                "criterion #{} requires an artifact but no delivery artifact was found",
                                criterion.ordinal
                            ),
                            evidence_ref: None,
                        });
                    }
                    outcomes.push(CriterionOutcome {
                        criterion_id: criterion.criterion_id,
                        ordinal: criterion.ordinal,
                        verification_kind: criterion.verification_kind.clone(),
                        verdict: Some(CriterionVerdict::Failed),
                        evidence,
                    });
                }
            }
            _ => {
                // manual / review：管线不判定，只落 finding 提示需人工裁决。
                findings.push(ReviewFinding {
                    finding_kind: "awaiting_manual".to_string(),
                    severity: Severity::Warning,
                    criterion_id: Some(criterion.criterion_id),
                    code_location: None,
                    message: format!(
                        "criterion #{} requires manual decision (`{}`)",
                        criterion.ordinal, criterion.verification_kind
                    ),
                    evidence_ref: None,
                });
                outcomes.push(CriterionOutcome {
                    criterion_id: criterion.criterion_id,
                    ordinal: criterion.ordinal,
                    verification_kind: criterion.verification_kind.clone(),
                    verdict: None,
                    evidence: serde_json::json!({}),
                });
            }
        }
    }

    // ── L4 证据一致性 ─────────────────────────────────────────────────
    if input.integration_status == "not_run" && !automated_criteria.is_empty() {
        for criterion in &automated_criteria {
            findings.push(ReviewFinding {
                finding_kind: "evidence_missing".to_string(),
                severity: Severity::Blocker,
                criterion_id: Some(criterion.criterion_id),
                code_location: None,
                message: format!(
                    "criterion #{} requires automated tests but integration_status is `not_run`",
                    criterion.ordinal
                ),
                evidence_ref: None,
            });
        }
    }
    if input.integration_status == "passed" && !failed_tests.is_empty() {
        findings.push(ReviewFinding {
            finding_kind: "evidence_conflict".to_string(),
            severity: Severity::Blocker,
            criterion_id: None,
            code_location: None,
            message: format!(
                "integration_status is `passed` but {} test(s) report failure",
                failed_tests.len()
            ),
            evidence_ref: None,
        });
    }

    // ── 总体结论（§5.1 矩阵的 verdict 来源行）────────────────────────
    let verdict = decide_verdict(input, &findings, &automated_criteria, &failed_tests);

    ReviewOutcome {
        verdict,
        findings,
        criteria: outcomes,
    }
}

/// 总体结论判定（§5.1 的 verdict 来源行 → 结论值）。
///
/// 顺序即优先级：
/// 1. L1/L2 的事实类阻断 → `changes_requested`（硬禁令：人机均不可越）。
/// 2. `not_run` + 有自动化条目 → `changes_requested`（证据缺失层）；
///    `not_run` + 无自动化条目 → `inconclusive`（只缺证据，不判失败）。
/// 3. 声明与证据矛盾 → `inconclusive`（Q8.5：不得当 Approved）。
/// 4. 任一条目判定失败 → `changes_requested`。
/// 5. 其余 → `approved`（待人工条目只影响**是否自动完成**，不否定通过本身）。
fn decide_verdict(
    input: &ReviewInput,
    findings: &[ReviewFinding],
    automated_criteria: &[&CriterionSnapshot],
    failed_tests: &[&TestEvidence],
) -> ReviewVerdict {
    let has_blocker = |kind: &str| {
        findings
            .iter()
            .any(|finding| finding.finding_kind == kind && finding.severity == Severity::Blocker)
    };

    if has_blocker("incomplete_delivery") || has_blocker("integration_failed") {
        return ReviewVerdict::ChangesRequested;
    }
    if input.integration_status == "not_run" {
        return if automated_criteria.is_empty() {
            ReviewVerdict::Inconclusive
        } else {
            ReviewVerdict::ChangesRequested
        };
    }
    if input.integration_status == "passed" && !failed_tests.is_empty() {
        return ReviewVerdict::Inconclusive;
    }
    if has_blocker("evidence_missing")
        || has_blocker("test_failed")
        || has_blocker("artifact_missing")
    {
        return ReviewVerdict::ChangesRequested;
    }
    ReviewVerdict::Approved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn criterion(ordinal: i32, kind: &str) -> CriterionSnapshot {
        CriterionSnapshot {
            criterion_id: Uuid::from_u128(ordinal as u128),
            ordinal,
            text: format!("criterion {ordinal}"),
            verification_kind: kind.to_string(),
        }
    }

    fn base_input() -> ReviewInput {
        ReviewInput {
            delivery_state: "delivered".to_string(),
            integration_status: "passed".to_string(),
            criteria: vec![criterion(10, "automated_test")],
            tests: vec![TestEvidence {
                repository_id: Uuid::from_u128(1),
                name: "cargo test".to_string(),
                passed: true,
                evidence_ref: None,
            }],
            artifacts: vec![],
            deliverable_paths: vec![],
        }
    }

    #[test]
    fn all_passed_gives_approved() {
        let outcome = evaluate(&base_input());
        assert_eq!(outcome.verdict, ReviewVerdict::Approved);
        assert!(outcome.findings.is_empty());
        assert_eq!(outcome.criteria[0].verdict, Some(CriterionVerdict::Passed));
    }

    #[test]
    fn failed_integration_is_changes_requested() {
        let mut input = base_input();
        input.integration_status = "failed".to_string();
        let outcome = evaluate(&input);
        assert_eq!(outcome.verdict, ReviewVerdict::ChangesRequested);
        assert!(outcome
            .findings
            .iter()
            .any(|f| f.finding_kind == "integration_failed"));
    }

    #[test]
    fn incomplete_delivery_is_changes_requested() {
        let mut input = base_input();
        input.delivery_state = "partial".to_string();
        let outcome = evaluate(&input);
        assert_eq!(outcome.verdict, ReviewVerdict::ChangesRequested);
    }

    #[test]
    fn not_run_with_automated_criterion_is_changes_requested() {
        let mut input = base_input();
        input.integration_status = "not_run".to_string();
        input.tests.clear();
        let outcome = evaluate(&input);
        assert_eq!(outcome.verdict, ReviewVerdict::ChangesRequested);
        assert!(outcome
            .findings
            .iter()
            .any(|f| f.finding_kind == "evidence_missing"));
    }

    #[test]
    fn not_run_without_automated_criterion_is_inconclusive() {
        let mut input = base_input();
        input.integration_status = "not_run".to_string();
        input.tests.clear();
        input.criteria = vec![criterion(10, "manual")];
        let outcome = evaluate(&input);
        assert_eq!(outcome.verdict, ReviewVerdict::Inconclusive);
        assert!(outcome.has_pending_manual());
        assert!(outcome.criteria[0].verdict.is_none());
    }

    #[test]
    fn passed_but_failed_test_is_inconclusive() {
        let mut input = base_input();
        input.tests[0].passed = false;
        let outcome = evaluate(&input);
        assert_eq!(outcome.verdict, ReviewVerdict::Inconclusive);
        assert!(outcome
            .findings
            .iter()
            .any(|f| f.finding_kind == "evidence_conflict"));
    }

    #[test]
    fn missing_artifact_is_changes_requested() {
        let mut input = base_input();
        input.criteria = vec![criterion(10, "artifact_exists")];
        input.tests.clear();
        input.deliverable_paths = vec!["docs/spec.md".to_string()];
        input.artifacts = vec![ArtifactProbe {
            repository_id: Uuid::from_u128(1),
            path: "docs/spec.md".to_string(),
            exists: false,
        }];
        let outcome = evaluate(&input);
        assert_eq!(outcome.verdict, ReviewVerdict::ChangesRequested);
        assert!(outcome
            .findings
            .iter()
            .any(|f| f.finding_kind == "artifact_missing"));
    }

    #[test]
    fn present_artifact_is_approved() {
        let mut input = base_input();
        input.criteria = vec![criterion(10, "artifact_exists")];
        input.tests.clear();
        input.deliverable_paths = vec!["docs/spec.md".to_string()];
        input.artifacts = vec![ArtifactProbe {
            repository_id: Uuid::from_u128(1),
            path: "docs/spec.md".to_string(),
            exists: true,
        }];
        let outcome = evaluate(&input);
        assert_eq!(outcome.verdict, ReviewVerdict::Approved);
        assert_eq!(outcome.criteria[0].verdict, Some(CriterionVerdict::Passed));
    }
}
