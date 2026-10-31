//! pm-ai: AI-PM instance model, skills, policy engine, executor.

pub mod context;
pub mod fake_model;
pub mod model;
pub mod openai_adapter;
pub mod orchestrator;
pub mod repo_model;
pub mod self_review;
pub mod skill_router;

pub use context::{build, fail_run, finish_run, save_snapshot, start_run, ContextError};
pub use fake_model::FakeModel;
pub use model::{AnalysisContext, AnalysisModel, ModelError};
pub use openai_adapter::{OpenAiCompatibleModel, SYSTEM_PROMPT};
pub use orchestrator::{HandleOutcome, Orchestrator, OrchestratorError};
pub use repo_model::{
    FakeRepoAnalysisModel, OpenAiRepoAnalysisModel, REPO_SYSTEM_PROMPT, RepoAnalysisModel,
    RepoMaterial,
};
pub use self_review::{self_review_json, review_proposals, ProposalOutcome};
pub use skill_router::{route, SkillId};
