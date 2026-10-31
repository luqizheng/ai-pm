//! pm-application: use cases, ports and application services.

pub mod bootstrap;
pub mod command_executor;
pub mod git_access;
pub mod ingestion;
pub mod knowledge;
pub mod knowledge_index;
pub mod llm_config;
pub mod outbox;
pub mod pm_write;
pub mod policy_engine;
pub mod project_import;
pub mod read;
pub mod repo_facts;
pub mod repository;
pub mod rows;
pub mod run_dispatch;
pub mod secret;
pub mod supervision;

pub use bootstrap::{
    BootstrapError, BootstrapOutcome, BootstrapRequest, bootstrap_project,
    bootstrap_project_with_status, resolve_assignee,
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
pub use llm_config::{
    LlmConfigError, LlmProviderRow, PROVIDERS, UpdateLlmProviderRequest, list_providers,
    load_provider_credentials, update_provider,
};
pub use policy_engine::{PolicyContext, PolicyEngine, PolicyLoadError};
pub use read::{ReadError, TaskContextRow, task_context};
pub use repository::{
    CreateRepositoryRequest, GitUrl, REPOSITORY_ROLES, RepositoryError, RepositoryRow, SshKeyView,
    UpdateRepositoryRequest, VerifyOutcome, archive_repository, create_repository,
    delete_repository, delete_ssh_key, ensure_ssh_key, generate_ssh_key, get_ssh_key,
    list_repositories, list_ssh_keys, open_ssh_private_key, revoke_ssh_key, update_repository,
    validate_git_url, verify_repository,
};
pub use supervision::{
    DecideRequest, Decision, DecisionOutcome, NotificationRow, SupervisionError, VetoOutcome,
    VetoRequest, decide_approval, freeze, notifications_feed, unfreeze, veto,
};
