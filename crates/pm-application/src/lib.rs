//! pm-application: use cases, ports and application services.

pub mod acceptance_overview;
pub mod acp06_07;
pub mod actor;
pub mod ai_conversations;
pub mod ai_interaction_envelope;
pub mod ai_interactions;
pub mod ai_roles;
pub mod ai_team_policies;
pub mod approver;
pub mod archive;
pub mod authz;
pub mod bootstrap;
pub mod caller;
pub mod comm_overview;
pub mod command_center;
pub mod command_executor;
pub mod continuation;
pub mod conversation_router;
pub mod delivery;
pub mod devenv;
pub mod device_credential;
pub mod discovery;
pub mod discussion_memory;
pub mod git_access;
pub mod ingestion;
pub mod intake;
pub mod interaction;
pub mod kb_service;
pub mod knowledge;
pub mod knowledge_index;
pub mod knowledge_indexer;
pub mod knowledge_migrate;
pub mod knowledge_proposal;
pub mod knowledge_quality;
pub mod knowledge_reference;
pub mod knowledge_search;
pub mod knowledge_write_coordinator;
pub mod onboarding;
pub mod outbox;
pub mod outbox_ops;
pub mod pc;
pub mod pm_write;
pub mod policy_composition;
pub mod policy_engine;
pub mod project_gate;
pub mod project_goal;
pub mod project_import;
pub mod project_member;
pub mod read;
pub mod recovery;
pub mod repo_access;
pub mod repo_facts;
pub mod repository;
pub mod requirement;
pub mod requirement_acceptance;
pub mod requirement_projection;
pub mod requirement_traceability;
pub mod response_delivery;
pub mod review;
pub mod rows;
pub mod run_cleanup;
pub mod run_context;
pub mod run_dispatch;
pub mod run_executor;
pub mod secret;
pub mod snapshot_analysis;
pub mod supervision;
pub mod task_dependency;
pub mod task_repository_binding;
pub mod team_llm;
pub mod team_onboarding;
pub mod worker_command;

pub use acceptance_overview::{
    load_overview, AcceptanceBadge, AcceptanceCounts, AcceptanceOverview, AcceptanceTask,
    ReviewStatus,
};
pub use actor::AuditActor;
// 注意：append_message / list_messages 已被 discovery 占用（lib 级非限定名），
// 对话域的同名函数走 `ai_conversations::` 限定路径（REST 侧即如此引用）。
pub use ai_conversations::{
    create_conversation, list_conversations, ConversationError, ConversationView, MessageView,
    CONVERSATION_LIST_LIMIT, DEFAULT_MESSAGE_LIMIT, MAX_CONTENT_CHARS, MAX_MESSAGE_LIMIT,
    MAX_TITLE_CHARS, MESSAGE_ROLES,
};
pub use ai_roles::{
    list_project_roles, list_role_versions, set_project_role_enabled, AiRoleError,
    AiRoleVersionSummary, AiRoleVersionView, ProjectAiRoleView,
};
pub use archive::{archive_project, ArchiveError, ArchiveOutcome};
pub use authz::{require_project_member, resolve_project_executor, AuthzError, ExecutorKind};
pub use bootstrap::{
    bootstrap_project, bootstrap_project_for_customer, bootstrap_project_for_customer_owned,
    bootstrap_project_owned, bootstrap_project_with_status, resolve_assignee, BootstrapError,
    BootstrapOutcome, BootstrapRequest,
};
pub use caller::{CallerContext, DeviceClaim, DeviceClaimIgnoredReason, HumanActor};
pub use command_executor::{
    ActorKind, CommandSource, CompensableTable, CompensationSpec, ExecutionContext,
    ExecutionReceipt, PgCommandExecutor, SnapshotEntry,
};
pub use discovery::{
    add_finding, add_proposal, add_question, answer_question, append_message, create_session,
    defer_question, get_session, get_session_detail, list_findings, list_messages, list_questions,
    list_sessions, set_proposal_status, submit_evidence, transition_session, DiscoveryError,
    DiscoveryFindingView, DiscoveryMessageView, DiscoveryProposalView, DiscoveryQuestionView,
    DiscoverySessionDetail, DiscoverySessionView, EvidenceInput, NewFinding, NewProposal,
};
pub use ingestion::{submit_event, IngestionError, SubmitEventRequest, SubmitOutcome};
pub use intake::{IntakeError, IntakeReceipt, IntakeState, RoutingPolicy, TaskSpec};
pub use knowledge::{
    content_hash, create_document, delete_document, list_documents, read_document,
    CreateDocumentRequest, CreatedDocument, DocumentContent, DocumentSummary, KnowledgeError,
    KnowledgeRoot,
};
pub use knowledge_index::{
    doc_type_weight, parse_markdown_tree, raw_score, scan_documents_direct, score_section,
    MarkdownSection, SearchHit,
};
pub use knowledge_indexer::{
    apply_transition, index_states, mark_stale, rebuild_quietly, DbSectionIndexer, IndexOutcome,
    IndexStateRow, KnowledgeIndexer, SearchPage, SectionRow, Transition, DB_SECTION_INDEX_VERSION,
    DB_SECTION_PROVIDER,
};
pub use knowledge_migrate::{migrate_requirement_revision, rollback_requirement_migration};
pub use knowledge_proposal::{
    confirm_redline_proposal, list_proposals, submit_proposal, veto_revision, ProposalOutcome,
    ProposalRequest, ProposalRow, RevisionVetoOutcome,
};
pub use knowledge_quality::{GateFailure, GateInput, QualityGateResult};
pub use knowledge_reference::{add_reference, list_references, AddReferenceRequest, ReferenceRow};
pub use knowledge_search::{FallbackReason, FallbackSearchResult, KnowledgeSearchService};
pub use knowledge_write_coordinator::{
    check_base_revision, claim_operation, complete_operation, current_head, find_operation,
    OperationClaim, ProjectWriteGuard, WriteCommand,
};
pub use onboarding::{
    install_guide, tool_catalog, Activation, BaseUrlSource, Catalog, CatalogItem, Guide, LeaseHint,
    NextSteps, Step, ToolInfo, VerifyStep, API_KEY_PLACEHOLDER,
};
pub use outbox::{OutboxAppend, OutboxEvent, PgOutboxAppend};
pub use outbox_ops::{
    dead_letters, replay, stats, DeadLetterListing, DeadLetterRow, EventTypeCounts, OutboxStats,
    ReplayError, ReplayOutcome, StatusCounts,
};
pub use pm_domain::CommandError;
pub use policy_engine::{PolicyContext, PolicyEngine, PolicyLoadError};
pub use project_gate::{project_activity, project_activity_tx, GateError, ProjectActivity};
pub use project_member::{
    apply_member_change, caller_can_see_members, is_project_manager, list_members,
    owned_project_ids, revoke_principal_memberships, MemberChangeOutcome, MemberOp, MemberRow,
    ProjectMemberError,
};
pub use read::{task_context, ReadError, TaskContextRow};
pub use repo_access::{
    access_snapshot_hash, access_snapshot_json, build_task_repo_access_tx, execution_allows,
    snapshot_allows, RepoAccessError, RepoAccessSnapshot,
};
pub use repository::{
    archive_repository, create_repository, delete_repository, delete_ssh_key, ensure_ssh_key,
    generate_ssh_key, get_ssh_key, list_repositories, list_ssh_keys, open_ssh_private_key,
    revoke_ssh_key, update_repository, validate_git_url, verify_repository,
    CreateRepositoryRequest, GitUrl, RepositoryError, RepositoryRow, SshKeyView,
    UpdateRepositoryRequest, VerifyOutcome, REPOSITORY_ROLES,
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
pub use run_context::{
    get_run_context, get_run_context_with, hash_lease_token, renew_lease_by_token, report_progress,
    LeaseError, LeaseState, ProgressReport, RenewedLease, RunContract, CONTRACT_VERSION,
};
pub use run_executor::{
    cancel_run, claim_and_start, poll_active_runs, reconcile_orphans, StartOutcome,
};
pub use supervision::{
    decide_approval, freeze, notifications_feed, unfreeze, veto, DecideRequest, Decision,
    DecisionOutcome, NotificationRow, SupervisionError, VetoOutcome, VetoRequest,
};
pub use task_dependency::{
    add_dependency, list_dependencies, remove_dependency, DependencyEdge, DependencyError,
    DependencyGraph,
};
pub use team_llm::{
    create_team_provider, delete_team_provider, get_project_ai_policy, list_team_providers,
    replace_team_credential, resolve_repo_call_config, set_project_ai_policy,
    set_team_provider_status, test_team_provider, update_team_provider, CreateTeamProviderRequest,
    ProjectAiPolicyRow, ReplaceCredentialRequest, ResolvedProvider, SetStatusRequest, TeamLlmError,
    TeamLlmProviderRow, TestResult, UpdateTeamProviderRequest, PROVIDER_TYPES,
};
