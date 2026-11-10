//! pm-ai: AI-PM instance model, skills, policy engine, executor.

pub mod analysis_model;
pub mod call_record;
pub mod capability_projection;
pub mod chat_context;
pub mod context;
pub mod conversation_model;
pub mod conversation_reply;
pub mod conversation_router_model;
pub mod discovery_context;
pub mod discovery_orchestrator;
pub mod discovery_writeback;
pub mod fake_model;
pub mod model;
pub mod openai_adapter;
pub mod orchestrator;
pub mod plan_decomposition;
pub mod repo_model;
pub mod retrieval_planner;
pub mod self_review;
pub mod skill_router;
pub mod snapshot_model;
pub mod writeback_model;

pub use analysis_model::{select_analysis_model_config, ModelSelection, RoutedAnalysisModel};
pub use call_record::PURPOSE_CONVERSATION_REPLY;
pub use capability_projection::PgCapabilityProjection;
pub use chat_context::{
    anchor_repository, build_chat_context, ChatContext, CHAT_CAP_PROJECT_PROFILE,
    CHAT_CAP_REPOSITORY_FACTS, CHAT_KEY_PROJECT_PROFILE, CHAT_KEY_REPOSITORY_FACTS,
    CHAT_TOTAL_BUDGET_CHARS,
};
pub use context::{build, fail_run, finish_run, save_snapshot, start_run, ContextError};
pub use conversation_model::{
    complete as complete_conversation, ConversationTurn, CONVERSATION_SYSTEM_PROMPT,
};
pub use conversation_reply::{ConversationReplyMode, ConversationReplyWorker, ReplyReport};
pub use conversation_router_model::LlmIntentClassifier;
pub use discovery_context::{
    assemble as assemble_interview_context, filter_evidence_refs, filter_evidence_refs_and_warn,
    ContextSection, FactIndex, FilteredRefs, InterviewContext, InterviewContextBuilder,
    InterviewContextError, InterviewKind, PgInterviewContextBuilder, SectionDraft, DROPPED_MARKER,
    MAX_EVIDENCE_REFS, SECTION_ORDER, TOTAL_BUDGET_CHARS,
};
pub use discovery_writeback::{
    CapabilityFacts, CapabilityProjection, DecomposeOutcome, DiscoveryWriteBack, DraftOutcome,
    DraftOutcomeKind, EmptyCapabilityProjection, NoWriteBack, RequirementDraft, TaskBatchOutcome,
    WriteBackHook, WriteBackMode, WriteBackModel, AI_FORBIDDEN_COMMANDS, AI_WRITABLE_COMMANDS,
};
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
pub use writeback_model::{
    LlmWriteBackModel, DECOMPOSE_TASKS_SYSTEM_PROMPT, DRAFT_REQUIREMENT_SYSTEM_PROMPT,
};
