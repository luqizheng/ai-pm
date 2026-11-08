//! pm-ai: AI-PM instance model, skills, policy engine, executor.

pub mod analysis_model;
pub mod call_record;
pub mod context;
pub mod conversation_model;
pub mod conversation_reply;
pub mod fake_model;
pub mod model;
pub mod openai_adapter;
pub mod orchestrator;
pub mod repo_model;
pub mod self_review;
pub mod skill_router;
pub mod snapshot_model;

pub use analysis_model::{select_analysis_model_config, ModelSelection, RoutedAnalysisModel};
pub use call_record::PURPOSE_CONVERSATION_REPLY;
pub use context::{build, fail_run, finish_run, save_snapshot, start_run, ContextError};
pub use conversation_model::{
    complete as complete_conversation, ConversationTurn, CONVERSATION_SYSTEM_PROMPT,
};
pub use conversation_reply::{ConversationReplyMode, ConversationReplyWorker, ReplyReport};
pub use fake_model::FakeModel;
pub use model::{AnalysisContext, AnalysisModel, ModelError};
pub use openai_adapter::{OpenAiCompatibleModel, SYSTEM_PROMPT};
pub use orchestrator::{HandleOutcome, Orchestrator, OrchestratorError};
pub use repo_model::{
    FakeRepoAnalysisModel, OpenAiRepoAnalysisModel, RepoAnalysisModel, RepoMaterial,
    REPO_SYSTEM_PROMPT,
};
pub use self_review::{review_proposals, self_review_json, ProposalOutcome};
pub use skill_router::{route, SkillId};
pub use snapshot_model::{
    LlmSnapshotAnalysisModel, SnapshotModelMode, SNAPSHOT_DIGEST_SYSTEM_PROMPT,
    SNAPSHOT_OBJECTS_SYSTEM_PROMPT,
};
