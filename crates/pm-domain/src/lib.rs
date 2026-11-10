//! pm-domain: entities, value objects and domain invariants.

pub mod ai_role;
pub mod command;
pub mod design;
pub mod error;
pub mod ids;
pub mod import;
pub mod plan;
pub mod policy;
pub mod project_member;
pub mod proposal;
pub mod repo_access;
pub mod task;
pub mod task_run;
pub mod task_transition;
pub mod tenant;
pub mod waiting;

pub use ai_role::{AiRoleCategory, AI_ROLE_TEMPLATE_SEED_KEYS, AI_ROLE_UNDISABLEABLE_KEYS};
pub use command::{ProjectCommand, Reversibility};
pub use error::CommandError;
pub use ids::*;
pub use policy::PolicyDecision;
pub use project_member::{
    is_agent_role, ProjectMemberRole, PROJECT_MEMBER_AGENT_ROLES, PROJECT_MEMBER_ROLES,
};
pub use proposal::{AnalysisResult, Confidence, Finding, Proposal, RiskLevel};
pub use repo_access::{parse_access_levels, RepoAccess};
pub use task::{LegacyMapping, TaskStatus};
pub use task_run::{
    FailureCategory, FailureRecord, OutcomeVerdict, RetryCounters, RetryPolicy, RunOutcome,
};
pub use task_transition::{
    allowed_edges, validate_transition, SupersedeIntent, TransitionError, TransitionInput,
};
pub use tenant::{TenantRole, SYSTEM_TENANT};
pub use waiting::{WaitingContext, WaitingReason};
