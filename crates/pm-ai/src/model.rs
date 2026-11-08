//! The analysis model contract consumed by the orchestrator (Task 12).

use async_trait::async_trait;
use pm_domain::{AnalysisResult, EventId, ProjectId};
use serde::{Deserialize, Serialize};

use crate::skill_router::SkillId;

/// Everything a model needs to analyze one inbound event.
///
/// Filled by the orchestrator's context builder (Task 12); `payload` carries
/// the raw inbound event payload. Serialized whole into
/// `ai.analysis_run.context_snapshot` JSONB.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
            ModelError::BadOutput(reason) => {
                write!(f, "analysis model returned bad output: {reason}")
            }
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
    use super::*;

    #[test]
    fn analysis_context_json_roundtrip_preserves_all_fields() {
        let context = AnalysisContext {
            project_id: uuid::Uuid::nil(),
            event_id: uuid::Uuid::nil(),
            event_type: "technical_design_submitted".to_string(),
            payload: serde_json::json!({ "fixture": "cache_overlap" }),
            task: Some(serde_json::json!({ "id": "TASK-102", "revision": 7 })),
            requirement_revision: None,
            milestone: Some(serde_json::json!({ "id": "MILESTONE-1" })),
            config: serde_json::json!({ "automation_mode": "ai_lead" }),
            policy_version: 1,
        };
        let json = serde_json::to_string(&context).unwrap();
        let back: AnalysisContext = serde_json::from_str(&json).unwrap();
        // PartialEq is intentionally not derived (review fix kept to the
        // prescribed derive set), so compare via the serialized form.
        assert_eq!(
            serde_json::to_value(&back).unwrap(),
            serde_json::to_value(&context).unwrap()
        );
    }
}
