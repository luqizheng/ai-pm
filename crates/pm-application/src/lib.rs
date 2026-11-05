//! pm-application: use cases, ports and application services.

pub mod acceptance_overview;
pub mod archive;
pub mod project_gate;
pub mod actor;
pub mod authz;
pub mod bootstrap;
pub mod caller;
pub mod command_center;
pub mod command_executor;
pub mod delivery;
pub mod devenv;
pub mod discovery;
pub mod git_access;
pub mod ingestion;
pub mod knowledge;
pub mod knowledge_index;
pub mod knowledge_indexer;
pub mod knowledge_migrate;
pub mod knowledge_proposal;
pub mod knowledge_quality;
pub mod knowledge_reference;
pub mod knowledge_search;
pub mod knowledge_write_coordinator;
pub mod llm_config;
pub mod onboarding;
pub mod outbox;
pub mod outbox_ops;
pub mod pm_write;
pub mod policy_composition;
pub mod policy_engine;
pub mod project_goal;
pub mod project_import;
pub mod project_member;
pub mod read;
pub mod requirement;
pub mod repo_access;
pub mod repo_facts;
pub mod repository;
pub mod review;
pub mod rows;
pub mod run_cleanup;
pub mod run_context;
pub mod run_dispatch;
pub mod run_executor;
pub mod secret;
pub mod supervision;
pub mod task_dependency;

pub use acceptance_overview::{
    AcceptanceBadge, AcceptanceCounts, AcceptanceOverview, AcceptanceTask, ReviewStatus,
    load_overview,
};
pub use actor::AuditActor;
pub use archive::{ArchiveError, ArchiveOutcome, archive_project};
pub use project_gate::{GateError, ProjectActivity, project_activity, project_activity_tx};
pub use authz::{AuthzError, ExecutorKind, require_project_member, resolve_project_executor};
pub use caller::{CallerContext, DeviceClaim, DeviceClaimIgnoredReason, HumanActor};
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
pub use outbox_ops::{
    DeadLetterListing, DeadLetterRow, EventTypeCounts, OutboxStats, ReplayError, ReplayOutcome,
    StatusCounts, dead_letters, replay, stats,
};
pub use ingestion::{IngestionError, SubmitEventRequest, SubmitOutcome, submit_event};
pub use knowledge::{
    CreateDocumentRequest, CreatedDocument, DocumentContent, DocumentSummary, KnowledgeError,
    KnowledgeRoot, content_hash, create_document, delete_document, list_documents, read_document,
};
pub use knowledge_index::{
    MarkdownSection, SearchHit, doc_type_weight, parse_markdown_tree, raw_score, scan_documents_direct,
    score_section,
};
pub use knowledge_indexer::{
    DB_SECTION_INDEX_VERSION, DB_SECTION_PROVIDER, DbSectionIndexer, IndexOutcome, IndexStateRow,
    KnowledgeIndexer, SectionRow, SearchPage, Transition, apply_transition, index_states,
    mark_stale, rebuild_quietly,
};
pub use knowledge_search::{FallbackReason, FallbackSearchResult, KnowledgeSearchService};
pub use knowledge_write_coordinator::{
    OperationClaim, ProjectWriteGuard, WriteCommand, check_base_revision, claim_operation,
    complete_operation, current_head, find_operation,
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
pub use onboarding::{
    API_KEY_PLACEHOLDER, Activation, BaseUrlSource, Catalog, CatalogItem, Guide, LeaseHint,
    NextSteps, Step, ToolInfo, VerifyStep, install_guide, tool_catalog,
};
pub use policy_engine::{PolicyContext, PolicyEngine, PolicyLoadError};
pub use project_member::{
    MemberChangeOutcome, MemberOp, MemberRow, ProjectMemberError, apply_member_change,
    caller_can_see_members, is_project_manager, list_members, owned_project_ids,
    revoke_principal_memberships,
};
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
pub use discovery::{
    DiscoveryError, DiscoveryFindingView, DiscoveryMessageView, DiscoveryProposalView,
    DiscoveryQuestionView, DiscoverySessionDetail, DiscoverySessionView, EvidenceInput, NewFinding,
    NewProposal, add_finding, add_proposal, add_question, answer_question, append_message,
    create_session, defer_question, get_session, get_session_detail, list_findings, list_messages,
    list_questions, list_sessions, set_proposal_status, submit_evidence,
    transition_session,
};pub use repo_access::{
    RepoAccessError, RepoAccessSnapshot, access_snapshot_hash, access_snapshot_json,
    build_task_repo_access_tx, execution_allows, snapshot_allows,
};

