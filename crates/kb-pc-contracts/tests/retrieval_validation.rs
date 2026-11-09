//! 检索计划校验（契约层纯函数）——**「Validator 是不可绕过的唯一执行入口」**的
//! 机械证明。
//!
//! 覆盖：
//! - 六项校验逐项 deny（成员/跨域/历史/白名单/敏感/查询安全）；
//! - 预算夹紧（候选/跳数/超时上限）；
//! - 恶意计划输出拒绝（SQL 片段、内部表名）；
//! - 确定性降级计划形态合法（单一执行路径的前提）；
//! - `ValidatedPlan` 只在校验通过时可得（失败只给报告，不给计划）。

use kb_pc_contracts::retrieval::{
    deterministic_fallback_plan, validate_plan, RetrievalBudget, RetrievalCaller, RetrievalChannel,
    RetrievalIntent, RetrievalMode, RetrievalPlan, TraversalSpec, ValidationInputs,
};
use uuid::Uuid;

fn caller() -> RetrievalCaller {
    RetrievalCaller {
        principal_id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        project_id: Uuid::new_v4(),
        auth_mode: "interactive".into(),
    }
}

fn inputs() -> ValidationInputs {
    ValidationInputs {
        project_visible: true,
        cross_domain_allowed: true,
        allow_historical: true,
        allowed_relationship_types: vec!["constrains".into(), "governed_by".into()],
        sensitive_blocked: false,
    }
}

fn plan() -> RetrievalPlan {
    RetrievalPlan {
        intent: RetrievalIntent::FactualLookup,
        mode: RetrievalMode::PcOnly,
        domains: vec!["PC".into()],
        retrieval_modes: vec![RetrievalChannel::PcEntity],
        entities: Vec::new(),
        rewritten_queries: vec!["订单状态".into()],
        traversal: None,
        analysis: None,
        include_history: false,
        budget: Some(RetrievalBudget {
            max_candidates: 60,
            max_hops: 3,
            timeout_ms: 8_000,
        }),
    }
}

fn denied_id(report: &kb_pc_contracts::retrieval::ValidationReport) -> Vec<String> {
    report
        .checks
        .iter()
        .filter(|c| !c.passed)
        .map(|c| c.id.clone())
        .collect()
}

// ── 六项逐项 deny ────────────────────────────────────────────────────────

#[test]
fn non_member_is_denied_by_membership_check() {
    let mut inp = inputs();
    inp.project_visible = false;
    let err = validate_plan(plan(), &caller(), &inp).unwrap_err();
    assert!(denied_id(&err).contains(&"project_membership".to_string()));
}

#[test]
fn cross_domain_is_denied_when_not_allowed() {
    let mut inp = inputs();
    inp.cross_domain_allowed = false;
    let mut p = plan();
    p.mode = RetrievalMode::CrossDomain;
    let err = validate_plan(p, &caller(), &inp).unwrap_err();
    assert!(denied_id(&err).contains(&"cross_domain".to_string()));
}

#[test]
fn history_is_denied_when_not_permitted() {
    let mut inp = inputs();
    inp.allow_historical = false;
    let mut p = plan();
    p.include_history = true;
    let err = validate_plan(p, &caller(), &inp).unwrap_err();
    assert!(denied_id(&err).contains(&"history".to_string()));
}

#[test]
fn relationship_traversal_requires_non_empty_whitelist() {
    let mut inp = inputs();
    inp.allowed_relationship_types = Vec::new();
    let mut p = plan();
    p.retrieval_modes.push(RetrievalChannel::Relationship);
    p.traversal = Some(TraversalSpec {
        enabled: true,
        max_hops: 3,
    });
    let err = validate_plan(p, &caller(), &inp).unwrap_err();
    assert!(denied_id(&err).contains(&"relationship_whitelist".to_string()));
}

#[test]
fn sensitive_knowledge_is_denied() {
    let mut inp = inputs();
    inp.sensitive_blocked = true;
    let err = validate_plan(plan(), &caller(), &inp).unwrap_err();
    assert!(denied_id(&err).contains(&"sensitive".to_string()));
}

#[test]
fn sql_like_query_is_denied() {
    let mut p = plan();
    p.rewritten_queries = vec!["SELECT * FROM governance.audit_log".into()];
    let err = validate_plan(p, &caller(), &inputs()).unwrap_err();
    assert!(denied_id(&err).contains(&"query_safety".to_string()));
}

#[test]
fn internal_table_reference_in_entity_is_denied() {
    let mut p = plan();
    p.entities = vec![kb_pc_contracts::retrieval::EntityAnchor {
        kind: "pc_object".into(),
        type_hint: None,
        key_or_name: "integration.outbox_event".into(),
    }];
    let err = validate_plan(p, &caller(), &inputs()).unwrap_err();
    assert!(denied_id(&err).contains(&"query_safety".to_string()));
}

// ── 夹紧与合法路径 ────────────────────────────────────────────────────────

#[test]
fn over_budget_plan_is_clamped_not_denied() {
    let mut p = plan();
    p.budget = Some(RetrievalBudget {
        max_candidates: 10_000,
        max_hops: 99,
        timeout_ms: 600_000,
    });
    let validated = validate_plan(p, &caller(), &inputs()).expect("clamping must not deny");
    let budget = validated.plan().budget.unwrap();
    assert_eq!(
        budget.max_candidates,
        kb_pc_contracts::retrieval::MAX_CANDIDATES_LIMIT
    );
    assert_eq!(budget.max_hops, kb_pc_contracts::retrieval::MAX_HOPS_LIMIT);
    assert_eq!(
        budget.timeout_ms,
        kb_pc_contracts::retrieval::MAX_TIMEOUT_MS
    );
    // 夹紧事实留痕（depth_budget 项带 reason）。
    let check = validated
        .report()
        .checks
        .iter()
        .find(|c| c.id == "depth_budget")
        .unwrap();
    assert!(
        check.reason.is_some(),
        "clamping must be recorded in the report"
    );
}

#[test]
fn valid_plan_yields_validated_plan_with_scope() {
    let c = caller();
    let validated = validate_plan(plan(), &c, &inputs()).expect("valid plan must pass");
    assert!(validated.report().all_passed());
    assert_eq!(validated.tenant_id(), c.tenant_id);
    assert_eq!(validated.project_id(), c.project_id);
}

#[test]
fn deterministic_fallback_plan_is_legal_and_bounded() {
    let p = deterministic_fallback_plan("订单状态");
    assert_eq!(p.mode, RetrievalMode::PcKb);
    assert!(
        p.traversal.is_none(),
        "fallback plan must not request traversal"
    );
    let budget = p.budget.unwrap();
    assert_eq!(
        budget.max_candidates,
        kb_pc_contracts::retrieval::MAX_CANDIDATES_LIMIT
    );
    assert_eq!(budget.max_hops, 0);
    let validated = validate_plan(p, &caller(), &inputs()).expect("fallback plan must validate");
    assert!(validated.report().all_passed());
}
