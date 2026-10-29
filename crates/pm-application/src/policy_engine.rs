//! Deterministic policy engine — the single source of truth for red lines and
//! escalation rules in the V2 "AI lead" trust model (主稿 §10.4).
//!
//! Design invariants:
//! - **Blacklist model.** `defaults.effect: auto_apply`: any command kind no rule
//!   mentions is auto-applied. V1's `deny` default is deliberately gone.
//! - **Dispatch is by command `kind()` string only.** The YAML names commands the
//!   same way `ProjectCommand::kind()` does (snake_case). Red-line kinds whose
//!   variants do not exist yet in M1 (e.g. `release_version`) stay in the YAML and
//!   start guarding the moment a future task adds the variants — no engine change.
//! - **Zero injection surface.** `PolicyContext` carries no payload: the engine
//!   never sees command content, so no model-controlled bytes can steer a decision.
//! - **Priority.** Red lines win first (`RequirePreConfirmation`), escalate
//!   conditions are evaluated in YAML order (first hit wins), else the default
//!   effect applies.
//! - YAML keys this M1 engine parses but ignores: `policy_id` (one policy version,
//!   no version management — YAGNI), `post_execution.notify`, `post_execution.sampling`.

use std::collections::HashMap;

use pm_domain::{Confidence, PolicyDecision, ProjectCommand};
use serde::Deserialize;

/// Canonical V2 policy, mirrored verbatim in `policies/ai-pm-ai-lead-v2.yaml` and
/// in the tests' embedded `POLICY_YAML` (three copies, one value). Keep in sync.
pub const DEFAULT_POLICY_YAML: &str = r#"# AI-PM V2 "AI lead" trust model — deterministic policy (single source of truth).
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

/// Threshold used when the policy carries no `blast_radius_above` condition.
const DEFAULT_BLAST_RADIUS_THRESHOLD: usize = 10;

/// Why a policy definition could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum PolicyLoadError {
    #[error("policy yaml parse failed: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("invalid veto_window duration `{0}` (expected e.g. 4h, 30m, 90s, 7d)")]
    InvalidVetoWindow(String),
}

/// The inputs a policy decision may rest on. Deliberately payload-free: the
/// engine reasons about *meta* facts of a proposal, never about its content
/// (injection surface is zero by construction).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyContext {
    pub confidence: Confidence,
    pub blast_radius: usize,
    pub unresolved_conflict: bool,
}

/// Config root. Unknown keys are ignored by serde; known-but-unmodeled keys are
/// called out in the module docs.
#[derive(Debug, Deserialize)]
struct PolicyConfig {
    /// Informational only: M1 has exactly one policy version, so the engine does
    /// not store it (YAGNI: no policy_version management).
    #[allow(dead_code)]
    policy_id: String,
    #[serde(default)]
    defaults: DefaultsConfig,
    #[serde(default)]
    escalate: EscalateConfig,
    #[serde(default)]
    red_lines: Vec<RedLineConfig>,
    #[serde(default)]
    post_execution: PostExecutionConfig,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct DefaultsConfig {
    /// Only `auto_apply` is supported: this engine is a blacklist model by
    /// design. Any other value (e.g. V1's `deny`) fails to load.
    effect: DefaultEffect,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum DefaultEffect {
    #[default]
    AutoApply,
}

#[derive(Debug, Deserialize, Default)]
struct EscalateConfig {
    #[serde(default)]
    when: Vec<EscalateCondition>,
}

/// One `escalate.when` entry. The YAML shape is a single-key map per list item:
/// `- confidence: low`, `- blast_radius_above: 10`, `- unresolved_conflict: true`,
/// `- anomaly_baseline: true`. Untagged, because serde_yaml 0.9 does not
/// deserialize map-shaped externally tagged enums; each variant's field name is
/// the key and every key is unique across variants, so exactly one variant can
/// match any given item (unknown keys elsewhere fail with a load error).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
enum EscalateCondition {
    Confidence { confidence: Confidence },
    BlastRadiusAbove { blast_radius_above: usize },
    UnresolvedConflict { unresolved_conflict: bool },
    AnomalyBaseline { anomaly_baseline: bool },
}

impl EscalateCondition {
    fn matches(&self, ctx: &PolicyContext) -> bool {
        match self {
            EscalateCondition::Confidence { confidence } => ctx.confidence == *confidence,
            EscalateCondition::BlastRadiusAbove { blast_radius_above } => {
                ctx.blast_radius > *blast_radius_above
            }
            EscalateCondition::UnresolvedConflict {
                unresolved_conflict,
            } => ctx.unresolved_conflict == *unresolved_conflict,
            // M1 has no behavioral-baseline anomaly detector yet (主稿 §10.2):
            // the condition parses — keeping the config forward-compatible —
            // but never fires. Wire it when anomaly data exists.
            EscalateCondition::AnomalyBaseline { .. } => false,
        }
    }

    /// Stable, machine-readable reason string for the first hit (deterministic).
    fn reason(&self) -> String {
        match self {
            EscalateCondition::Confidence { confidence } => match confidence {
                Confidence::Low => "confidence_low".to_string(),
                Confidence::Medium => "confidence_medium".to_string(),
                Confidence::High => "confidence_high".to_string(),
            },
            EscalateCondition::BlastRadiusAbove { blast_radius_above } => {
                format!("blast_radius_above_{blast_radius_above}")
            }
            EscalateCondition::UnresolvedConflict { .. } => "unresolved_conflict".to_string(),
            EscalateCondition::AnomalyBaseline { .. } => "anomaly_baseline".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RedLineConfig {
    /// Command `kind()` string (snake_case), not a variant name.
    command: String,
    required_role: String,
}

#[derive(Debug, Deserialize, Default)]
struct PostExecutionConfig {
    #[serde(default)]
    veto_window: VetoWindowConfig,
    // `notify` / `sampling` intentionally unmodeled in M1 (ignored by serde).
}

#[derive(Debug, Clone, Deserialize)]
struct VetoWindowConfig {
    #[serde(
        default = "default_veto_window",
        deserialize_with = "duration_from_str"
    )]
    default: chrono::Duration,
    #[serde(default, deserialize_with = "duration_map_from_str")]
    by_command_type: HashMap<String, chrono::Duration>,
}

impl Default for VetoWindowConfig {
    fn default() -> Self {
        Self {
            default: default_veto_window(),
            by_command_type: HashMap::new(),
        }
    }
}

fn default_veto_window() -> chrono::Duration {
    chrono::Duration::hours(4)
}

/// Parses `4h` / `30m` / `90s` / `7d` style durations.
fn parse_veto_window(text: &str) -> Result<chrono::Duration, PolicyLoadError> {
    let text = text.trim();
    let digits_end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (digits, unit) = text.split_at(digits_end);
    let value: u64 = digits
        .parse()
        .map_err(|_| PolicyLoadError::InvalidVetoWindow(text.to_string()))?;
    let seconds = match unit {
        "s" => value,
        "m" => value.saturating_mul(60),
        "h" => value.saturating_mul(3600),
        "d" => value.saturating_mul(86_400),
        _ => return Err(PolicyLoadError::InvalidVetoWindow(text.to_string())),
    };
    chrono::Duration::try_seconds(i64::try_from(seconds).unwrap_or(i64::MAX))
        .ok_or_else(|| PolicyLoadError::InvalidVetoWindow(text.to_string()))
}

fn duration_from_str<'de, D>(deserializer: D) -> Result<chrono::Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let text = String::deserialize(deserializer)?;
    parse_veto_window(&text).map_err(serde::de::Error::custom)
}

fn duration_map_from_str<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, chrono::Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = HashMap::<String, String>::deserialize(deserializer)?;
    raw.into_iter()
        .map(|(kind, text)| parse_veto_window(&text).map(|duration| (kind, duration)))
        .collect::<Result<HashMap<_, _>, PolicyLoadError>>()
        .map_err(serde::de::Error::custom)
}

/// Parsed, ready-to-evaluate policy rules.
#[derive(Debug, Clone)]
pub struct PolicyEngine {
    default_effect: DefaultEffect,
    escalate_when: Vec<EscalateCondition>,
    /// red-line command kind → required confirmation role.
    red_lines: HashMap<String, String>,
    veto_windows: VetoWindowConfig,
}

impl PolicyEngine {
    pub fn from_yaml(text: &str) -> Result<Self, PolicyLoadError> {
        let config: PolicyConfig = serde_yaml::from_str(text)?;
        Ok(Self {
            default_effect: config.defaults.effect,
            escalate_when: config.escalate.when,
            red_lines: config
                .red_lines
                .into_iter()
                .map(|rule| (rule.command, rule.required_role))
                .collect(),
            veto_windows: config.post_execution.veto_window,
        })
    }

    /// Built-in instance equal in value to `policies/ai-pm-ai-lead-v2.yaml`.
    pub fn for_tests() -> Self {
        Self::from_yaml(DEFAULT_POLICY_YAML).expect("built-in policy yaml must parse")
    }

    /// Red line → `RequirePreConfirmation` (escalate is never reached); otherwise
    /// the first matching `escalate.when` condition (YAML order); otherwise the
    /// default effect (`AutoApply`, blacklist model).
    pub fn evaluate(&self, command: &ProjectCommand, ctx: &PolicyContext) -> PolicyDecision {
        if let Some(role) = self.required_role(command) {
            return PolicyDecision::RequirePreConfirmation { role };
        }
        for condition in &self.escalate_when {
            if condition.matches(ctx) {
                return PolicyDecision::Escalate {
                    reason: condition.reason(),
                };
            }
        }
        match self.default_effect {
            DefaultEffect::AutoApply => PolicyDecision::AutoApply,
        }
    }

    pub fn is_redline(&self, command: &ProjectCommand) -> bool {
        self.red_lines.contains_key(command.kind())
    }

    pub fn required_role(&self, command: &ProjectCommand) -> Option<String> {
        self.red_lines.get(command.kind()).cloned()
    }

    /// Post-execution veto window: per-kind override first, else the policy
    /// default (4h when unset).
    pub fn veto_window(&self, command: &ProjectCommand) -> chrono::Duration {
        self.veto_windows
            .by_command_type
            .get(command.kind())
            .copied()
            .unwrap_or(self.veto_windows.default)
    }

    /// The blast-radius threshold the escalate rules enforce (10 when unset).
    pub fn escalate_threshold(&self) -> usize {
        self.escalate_when
            .iter()
            .find_map(|condition| match condition {
                EscalateCondition::BlastRadiusAbove { blast_radius_above } => {
                    Some(*blast_radius_above)
                }
                _ => None,
            })
            .unwrap_or(DEFAULT_BLAST_RADIUS_THRESHOLD)
    }
}
