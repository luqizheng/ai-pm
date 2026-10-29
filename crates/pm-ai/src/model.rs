//! The analysis model contract consumed by the orchestrator (Task 12).

use async_trait::async_trait;
use pm_domain::{AnalysisResult, EventId, ProjectId};

use crate::skill_router::SkillId;

/// Everything a model needs to analyze one inbound event.
///
/// Filled by the orchestrator's context builder (Task 12); `payload` carries
/// the raw inbound event payload.
#[derive(Debug, Clone)]
pub struct AnalysisContext {
    pub project_id: ProjectId,
    pub event_id: EventId,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub task: Option<serde_json::Value>,
    pub requirement_revision: Option<serde_json::Value>,
    pub milestone: Option<serde_json::Value>,
    pub config: serde_json::Value,
    pub policy_version: i32,
}

/// Errors an analysis model can surface to the orchestrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    Timeout,
    Unavailable(String),
    BadOutput(String),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::Timeout => write!(f, "analysis model timed out"),
            ModelError::Unavailable(reason) => write!(f, "analysis model unavailable: {reason}"),
            ModelError::BadOutput(reason) => write!(f, "analysis model returned bad output: {reason}"),
        }
    }
}

impl std::error::Error for ModelError {}

/// A (possibly remote) model that runs the given skills over one context and
/// produces findings plus change proposals.
#[async_trait]
pub trait AnalysisModel: Send + Sync {
    async fn analyze(
        &self,
        context: &AnalysisContext,
        skills: &[SkillId],
    ) -> Result<AnalysisResult, ModelError>;
}

#[cfg(test)]
mod tests {
    // Per brief: model.rs is the Task 12 contract; behavior tests live in
    // fake_model.rs against the trait's only M1 implementation.
}
