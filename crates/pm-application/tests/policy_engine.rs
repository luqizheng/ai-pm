//! Deterministic policy engine tests (Task 5, Step 1).
//!
//! Embedded YAML string per brief — tests never read a file path. The constant
//! below is a verbatim copy of `policies/ai-pm-ai-lead-v2.yaml` and of
//! `DEFAULT_POLICY_YAML` in `crates/pm-application/src/policy_engine.rs`
//! (three copies, one value).

use pm_application::{PolicyContext, PolicyEngine};
use pm_domain::{Confidence, PolicyDecision, ProjectCommand};

const POLICY_YAML: &str = r#"# AI-PM V2 "AI lead" trust model — deterministic policy (single source of truth).
# Mirrored verbatim in crates/pm-application/src/policy_engine.rs (DEFAULT_POLICY_YAML)
# and crates/pm-application/tests/policy_engine.rs (POLICY_YAML).
policy_id: ai-pm-ai-lead-v2

defaults:
  effect: auto_apply         # blacklist model: anything not matched below is auto-applied

escalate:                    # pre-execution human review; never evaluated for red lines
  when:
    - confidence: low
    - blast_radius_above: 10
    - unresolved_conflict: true
    - anomaly_baseline: true # M1: parses but never fires (no behavioral-baseline detector yet)

red_lines:                   # pre-confirmation required, per role
  - command: release_version
    required_role: release_manager
  - command: external_commitment
    required_role: account_owner
  - command: sign_acceptance
    required_role: authorized_acceptor
  - command: destructive_write
    required_role: project_owner
  - command: grant_permission
    required_role: project_owner
  - command: change_policy
    required_role: platform_admin
  - command: spend_above_quota
    required_role: quota_approver
  # 冷启动导入：批量命令共用 project_import 红线语义，执行前须 owner 确认。
  # create_tasks 不在此列（PA-06-04）；其批量影响量按实际条目数进入 blast_radius，>10 时升级。
  - command: create_milestones
    required_role: project_owner
  - command: create_requirements
    required_role: project_owner
  - command: write_documents
    required_role: project_owner

post_execution:
  notify: always             # parsed-ignored in M1 (notification stream lands in a later task)
  veto_window:
    default: 4h
    by_command_type:
      adjust_milestone_plan: 24h
  sampling:                  # parsed-ignored in M1
    base_rate: 0.05
    risk_weighted: true
"#;

fn engine() -> PolicyEngine {
    PolicyEngine::from_yaml(POLICY_YAML).expect("embedded policy yaml parses")
}

fn ctx(confidence: Confidence, blast_radius: usize, unresolved_conflict: bool) -> PolicyContext {
    PolicyContext {
        confidence,
        blast_radius,
        unresolved_conflict,
    }
}

fn create_task(title: &str) -> ProjectCommand {
    ProjectCommand::CreateTask {
        project_id: pm_domain::ProjectId::nil(),
        title: title.to_string(),
        description: "payload content the engine must never look at".to_string(),
        assignee_id: None,
    }
}

fn update_task_status() -> ProjectCommand {
    ProjectCommand::UpdateTaskStatus {
        task_id: pm_domain::TaskId::nil(),
        expected_revision: 1,
        new_status: pm_domain::TaskStatus::Running,
    }
}

fn adjust_milestone_plan() -> ProjectCommand {
    ProjectCommand::AdjustMilestonePlan {
        project_id: pm_domain::ProjectId::nil(),
        milestone_id: pm_domain::MilestoneId::nil(),
        new_planned_at: None,
        expected_revision: 1,
    }
}

fn sign_acceptance() -> ProjectCommand {
    ProjectCommand::SignAcceptance {
        project_id: pm_domain::ProjectId::nil(),
        milestone_id: pm_domain::MilestoneId::nil(),
        expected_revision: 1,
    }
}

fn change_policy(new_policy: serde_json::Value) -> ProjectCommand {
    ProjectCommand::ChangePolicy {
        project_id: pm_domain::ProjectId::nil(),
        new_policy,
    }
}

fn rollback_command() -> ProjectCommand {
    ProjectCommand::RollbackCommand {
        project_id: pm_domain::ProjectId::nil(),
        target_audit_id: pm_domain::AuditId::nil(),
        reason: "undo".to_string(),
    }
}

/// Brief 1) sign_acceptance → RequirePreConfirmation{role:"authorized_acceptor"}
#[test]
fn test_1_sign_acceptance_requires_authorized_acceptor() {
    let engine = engine();
    let decision = engine.evaluate(&sign_acceptance(), &ctx(Confidence::High, 0, false));
    assert_eq!(
        decision,
        PolicyDecision::RequirePreConfirmation {
            role: "authorized_acceptor".to_string()
        }
    );
    assert!(engine.is_redline(&sign_acceptance()));
    assert_eq!(
        engine.required_role(&sign_acceptance()),
        Some("authorized_acceptor".to_string())
    );
    // Priority: a red line wins over escalate — even when every escalate
    // condition would fire, the answer stays RequirePreConfirmation.
    assert_eq!(
        engine.evaluate(&sign_acceptance(), &ctx(Confidence::Low, 50, true)),
        PolicyDecision::RequirePreConfirmation {
            role: "authorized_acceptor".to_string()
        }
    );
}

/// Brief 2) change_policy → RequirePreConfirmation{role:"platform_admin"}
#[test]
fn test_2_change_policy_requires_platform_admin() {
    let engine = engine();
    let decision = engine.evaluate(
        &change_policy(serde_json::json!({ "automation_mode": "controlled" })),
        &ctx(Confidence::High, 0, false),
    );
    assert_eq!(
        decision,
        PolicyDecision::RequirePreConfirmation {
            role: "platform_admin".to_string()
        }
    );
    assert!(engine.is_redline(&change_policy(serde_json::json!({}))));
    assert_eq!(
        engine.required_role(&change_policy(serde_json::json!({}))),
        Some("platform_admin".to_string())
    );
}

/// Brief 3) update_task_status + confidence=High → AutoApply
#[test]
fn test_3_update_task_status_high_confidence_auto_applies() {
    let engine = engine();
    let decision = engine.evaluate(&update_task_status(), &ctx(Confidence::High, 3, false));
    assert_eq!(decision, PolicyDecision::AutoApply);
    assert!(!engine.is_redline(&update_task_status()));
    assert_eq!(engine.required_role(&update_task_status()), None);
}

/// Brief 4) update_task_status + confidence=Low → Escalate
#[test]
fn test_4_update_task_status_low_confidence_escalates() {
    let engine = engine();
    let decision = engine.evaluate(&update_task_status(), &ctx(Confidence::Low, 0, false));
    assert_eq!(
        decision,
        PolicyDecision::Escalate {
            reason: "confidence_low".to_string()
        }
    );
    // Medium confidence does not match `confidence: low` and falls through to AutoApply.
    assert_eq!(
        engine.evaluate(&update_task_status(), &ctx(Confidence::Medium, 0, false)),
        PolicyDecision::AutoApply
    );
}

/// Brief 5) create_task + blast_radius=11 → Escalate
#[test]
fn test_5_create_task_above_blast_radius_threshold_escalates() {
    let engine = engine();
    let decision = engine.evaluate(
        &create_task("Sweep reorder"),
        &ctx(Confidence::High, 11, false),
    );
    assert_eq!(
        decision,
        PolicyDecision::Escalate {
            reason: "blast_radius_above_10".to_string()
        }
    );
    // The threshold is strictly "above": exactly 10 does not escalate.
    assert_eq!(
        engine.evaluate(&create_task("One task"), &ctx(Confidence::High, 10, false)),
        PolicyDecision::AutoApply
    );
    // The engine surfaces the same threshold it enforces.
    assert_eq!(engine.escalate_threshold(), 10);
}

/// Brief 6) test_policy_decision_ignores_payload_content:
/// PolicyContext has no payload field — the exhaustive struct pattern below is a
/// compile-time proof of exactly that (adding a payload field breaks this build).
/// Then: same command kind with different payload content, and the same inputs
/// evaluated repeatedly, must yield identical decisions.
#[test]
fn test_policy_decision_ignores_payload_content() {
    // Compile-time proof: PolicyContext exposes exactly these three fields.
    let context = ctx(Confidence::High, 5, false);
    let PolicyContext {
        confidence: _,
        blast_radius: _,
        unresolved_conflict: _,
    } = context;

    let engine = engine();

    // Same kind (create_task), different payload content → identical decision.
    let a = engine.evaluate(&create_task("Title A"), &ctx(Confidence::High, 1, false));
    let b = engine.evaluate(
        &create_task("Totally different title"),
        &ctx(Confidence::High, 1, false),
    );
    assert_eq!(a, b, "payload content must not influence the decision");

    // Even on the red-line path the payload (here: the policy body itself) is ignored.
    let c = engine.evaluate(
        &change_policy(serde_json::json!({})),
        &ctx(Confidence::High, 0, false),
    );
    let d = engine.evaluate(
        &change_policy(serde_json::json!({ "grant": "self_authorize_everything" })),
        &ctx(Confidence::High, 0, false),
    );
    assert_eq!(c, d);

    // Idempotence: identical inputs, evaluated repeatedly, always identical output.
    let command = update_task_status();
    let first = engine.evaluate(&command, &ctx(Confidence::Low, 12, true));
    for _ in 0..3 {
        assert_eq!(
            engine.evaluate(&command, &ctx(Confidence::Low, 12, true)),
            first
        );
    }
    // And an equal-but-distinct command/ctx pair lands on the same decision.
    assert_eq!(
        engine.evaluate(&update_task_status(), &ctx(Confidence::Low, 12, true)),
        first
    );
}

/// Brief 7) veto_window: sign_acceptance→4h, adjust_milestone_plan→24h, 其它→4h
#[test]
fn test_7_veto_window_defaults_and_per_command_override() {
    let engine = engine();
    assert_eq!(
        engine.veto_window(&sign_acceptance()),
        chrono::Duration::hours(4)
    );
    assert_eq!(
        engine.veto_window(&adjust_milestone_plan()),
        chrono::Duration::hours(24)
    );
    // Every other kind falls back to the default window.
    assert_eq!(
        engine.veto_window(&update_task_status()),
        chrono::Duration::hours(4)
    );
    assert_eq!(
        engine.veto_window(&change_policy(serde_json::json!({}))),
        chrono::Duration::hours(4)
    );
}

/// Brief 8) 未知命令 kind → AutoApply（黑名单模型默认放行）
#[test]
fn test_8_command_kind_unknown_to_policy_auto_applies() {
    let engine = engine();
    // rollback_command / approve_requirement_revision appear in no YAML section:
    // the blacklist model lets them through.
    assert_eq!(
        engine.evaluate(&rollback_command(), &ctx(Confidence::High, 0, false)),
        PolicyDecision::AutoApply
    );
    assert_eq!(
        engine.evaluate(
            &ProjectCommand::ApproveRequirementRevision {
                project_id: pm_domain::ProjectId::nil(),
                requirement_id: pm_domain::RequirementId::nil(),
                revision_id: pm_domain::RevisionId::nil(),
            },
            &ctx(Confidence::Medium, 1, false),
        ),
        PolicyDecision::AutoApply
    );
    assert!(!engine.is_redline(&rollback_command()));
    assert_eq!(engine.required_role(&rollback_command()), None);
}

/// Dispatch is by command `kind()` string, not by variant identity: adding a red
/// line for a kind whose ProjectCommand variant does not exist yet (or reusing an
/// existing kind) needs no engine change. Also proves the hard 4h veto default
/// and threshold fallback of 10 when the YAML omits them.
#[test]
fn red_line_lookup_is_by_command_kind_string() {
    let yaml = r#"
policy_id: test-minimal
defaults:
  effect: auto_apply
escalate:
  when: []
red_lines:
  - command: create_task
    required_role: project_owner
"#;
    let engine = PolicyEngine::from_yaml(yaml).expect("minimal policy yaml parses");
    let decision = engine.evaluate(&create_task("anything"), &ctx(Confidence::High, 0, false));
    assert_eq!(
        decision,
        PolicyDecision::RequirePreConfirmation {
            role: "project_owner".to_string()
        }
    );
    assert!(engine.is_redline(&create_task("x")));
    // Omitted post_execution → hard 4h default; omitted blast_radius_above → 10.
    assert_eq!(
        engine.veto_window(&create_task("x")),
        chrono::Duration::hours(4)
    );
    assert_eq!(engine.escalate_threshold(), 10);
}

/// `for_tests()` must behave exactly like an engine built from the embedded
/// canonical YAML: identical decisions across a command × context matrix.
#[test]
fn for_tests_matches_embedded_canonical_yaml() {
    let embedded = engine();
    let builtin = PolicyEngine::for_tests();

    let commands: Vec<ProjectCommand> = vec![
        create_task("matrix"),
        update_task_status(),
        adjust_milestone_plan(),
        sign_acceptance(),
        change_policy(serde_json::json!({})),
        rollback_command(),
    ];
    let contexts = [
        ctx(Confidence::Low, 0, false),
        ctx(Confidence::Medium, 10, false),
        ctx(Confidence::High, 11, true),
    ];

    for command in &commands {
        for context in &contexts {
            assert_eq!(
                embedded.evaluate(command, context),
                builtin.evaluate(command, context)
            );
        }
        assert_eq!(embedded.is_redline(command), builtin.is_redline(command));
        assert_eq!(
            embedded.required_role(command),
            builtin.required_role(command)
        );
        assert_eq!(embedded.veto_window(command), builtin.veto_window(command));
    }
    assert_eq!(embedded.escalate_threshold(), builtin.escalate_threshold());
}
