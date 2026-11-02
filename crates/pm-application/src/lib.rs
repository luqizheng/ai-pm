//! pm-application: use cases, ports and application services.

pub mod actor;
pub mod bootstrap;
pub mod command_executor;
pub mod delivery;
pub mod git_access;
pub mod ingestion;
pub mod knowledge;
pub mod knowledge_index;
pub mod knowledge_indexer;
pub mod knowledge_migrate;
pub mod knowledge_proposal;
pub mod knowledge_quality;
pub mod knowledge_reference;
pub mod llm_config;
pub mod outbox;
pub mod pm_write;
pub mod policy_composition;
pub mod policy_engine;
pub mod project_import;
pub mod read;
pub mod repo_facts;
pub mod repository;
pub mod review;
pub mod rows;
pub mod run_context;
pub mod run_dispatch;
pub mod run_executor;
pub mod secret;
pub mod supervision;
pub mod task_dependency;

pub use actor::AuditActor;
pub use bootstrap::{
    BootstrapError, BootstrapOutcome, BootstrapRequest, bootstrap_project,
    bootstrap_project_for_customer, bootstrap_project_for_customer_owned,
    bootstrap_project_owned, bootstrap_project_with_status, resolve_assignee,
};
pub use command_executor::{
    ActorKind, CommandSource, CompensationSpec, CompensableTable, ExecutionContext, ExecutionReceipt,
    PgCommandExecutor, SnapshotEntry,
};
pub use pm_domain::CommandError;
pub use outbox::{OutboxAppend, OutboxEvent, PgOutboxAppend};
pub use ingestion::{IngestionError, SubmitEventRequest, SubmitOutcome, submit_event};
pub use knowledge::{
    CreateDocumentRequest, CreatedDocument, DocumentContent, DocumentSummary, KnowledgeError,
    KnowledgeRoot, content_hash, create_document, delete_document, list_documents, read_document,
};
pub use knowledge_index::{MarkdownSection, SearchHit, parse_markdown_tree, search_documents};
pub use knowledge_indexer::{
    FallbackSearchResult, IndexReference, IndexStateRow, KnowledgeIndexer, MarkdownTreeIndexer,
    RetrievedSection, index_revision, index_states, search_with_fallback,
};
pub use knowledge_migrate::{migrate_requirement_revision, rollback_requirement_migration};
pub use knowledge_proposal::{
    ProposalOutcome, ProposalRequest, ProposalRow, RevisionVetoOutcome, confirm_redline_proposal,
    list_proposals, submit_proposal, veto_revision,
};
pub use knowledge_quality::{GateFailure, GateInput, QualityGateResult};
pub use knowledge_reference::{
    AddReferenceRequest, ReferenceRow, add_reference, list_references,
};
pub use llm_config::{
    LlmConfigError, LlmProviderRow, PROVIDERS, UpdateLlmProviderRequest, list_providers,
    load_provider_credentials, update_provider,
};
pub use policy_engine::{PolicyContext, PolicyEngine, PolicyLoadError};
pub use read::{ReadError, TaskContextRow, task_context};
pub use run_context::{
    CONTRACT_VERSION, LeaseError, LeaseState, ProgressReport, RenewedLease, RunContract,
    get_run_context, hash_lease_token, renew_lease_by_token, report_progress,
};
pub use run_executor::{
    StartOutcome, cancel_run, claim_and_start, poll_active_runs, reconcile_orphans,
};
pub use repository::{
    CreateRepositoryRequest, GitUrl, REPOSITORY_ROLES, RepositoryError, RepositoryRow, SshKeyView,
    UpdateRepositoryRequest, VerifyOutcome, archive_repository, create_repository,
    delete_repository, delete_ssh_key, ensure_ssh_key, generate_ssh_key, get_ssh_key,
    list_repositories, list_ssh_keys, open_ssh_private_key, revoke_ssh_key, update_repository,
    validate_git_url, verify_repository,
};
pub use review::{
    advance_after_review, collect_review_input, decide_review_approval, enqueue_review, evaluate,
    input_manifest_hash, judge_delivery_failures, review_policy_version, review_step,
    reviewed_manifest_hash, ApprovalError, ApprovalOutcome, ArtifactProbe, CollectedReview,
    CompletionGate, CriterionAnchor, CriterionOutcome, CriterionSnapshot, CriterionVerdict,
    EnqueueOutcome, FailureJudgeReport, KnowledgeWriteback, RepositoryAnchor, ReviewConfig,
    ReviewDecisionRequest, ReviewError, ReviewFinding, ReviewInput, ReviewOutcome, ReviewStep,
    ReviewVerdict, ReviewedDelivery, Severity, TestEvidence,
};
pub use supervision::{
    DecideRequest, Decision, DecisionOutcome, NotificationRow, SupervisionError, VetoOutcome,
    VetoRequest, decide_approval, freeze, notifications_feed, unfreeze, veto,
};
pub use task_dependency::{
    DependencyEdge, DependencyError, DependencyGraph, add_dependency, list_dependencies,
    remove_dependency,
};
