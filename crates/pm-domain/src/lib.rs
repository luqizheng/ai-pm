//! pm-domain: entities, value objects and domain invariants.

pub mod command;
pub mod error;
pub mod ids;
pub mod import;
pub mod policy;
pub mod proposal;
pub mod task;
pub mod task_run;
pub mod task_transition;
pub mod tenant;
pub mod waiting;

pub use command::{ProjectCommand, Reversibility};
pub use error::CommandError;
pub use ids::*;
pub use policy::PolicyDecision;
pub use proposal::{AnalysisResult, Confidence, Finding, Proposal, RiskLevel};
pub use task::{LegacyMapping, TaskStatus};
pub use tenant::{TenantRole, SYSTEM_TENANT};
pub use task_run::{FailureCategory, FailureRecord, OutcomeVerdict, RetryCounters, RetryPolicy, RunOutcome};
pub use task_transition::{
    allowed_edges, validate_transition, SupersedeIntent, TransitionError, TransitionInput,
};
pub use waiting::{WaitingContext, WaitingReason};
