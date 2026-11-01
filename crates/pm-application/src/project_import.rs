//! 冷启动导入状态机：`queued → cloning → reading → analyzing → draft_ready`，
//! `apply` 之后转 `proposed`，审批执行完成转 `applied` 或 `rejected`。
//!
//! 导入产物**不直写业务表**：草案只落 `integration.repo_import.draft`，由
//! `propose_import` 转成 `ai.change_proposal`，走既有红线审批与执行链。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::git_access;
use crate::actor::AuditActor;
use crate::pm_write::PmWriteError;
use crate::repo_facts::{self, RepoAnalysisModel, RepoMaterial, pm_ai_port::ModelError};
use pm_domain::import::{Evidence, EvidenceKind, ProjectDraft};

/// `list_imports` 的结果行（14 列）。
type ImportListRow = (
    Uuid,
    String,
    i32,
    String,
    i64,
    Option<Uuid>,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
);
/// `get_import` 的结果行（14 列，含草案与阶段详情）。
type ImportDetailRow = (
    Uuid,
    String,
    i32,
    Option<String>,
    String,
    Option<serde_json::Value>,
    i64,
    i32,
    Option<String>,
    Option<String>,
    Option<Uuid>,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
);
/// `get_import` 的来源快照行（8 列）。
type ImportSourceRow = (Uuid, String, Option<String>, i32, i32, i64, bool, Option<String>);

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("project not found")]
    ProjectNotFound,
    #[error("project already has requirements")]
    ProjectNotEmpty,
    #[error("project has no verified repository")]
    NoVerifiedRepository,
    #[error("an import is already in progress")]
    ImportInProgress,
    #[error("import not found")]
    NotFound,
    #[error("import can no longer be cancelled")]
    NotCancellable,
    #[error("draft revision is stale")]
    StaleDraft,
    #[error("no draft items selected")]
    NoSelection,
    #[error("project code is already taken")]
    ProjectCodeTaken,
    #[error("import already applied")]
    AlreadyApplied,
    #[error("import has no grounded output")]
    NoGroundedOutput,
    #[error("{0}")]
    InvalidState(&'static str),
    #[error("model failed: {0}")]
    Model(String),
    #[error("git failed: {0}")]
    Git(String),
    #[error("database error: {0}")]
    Db(sqlx::Error),
}

impl From<sqlx::Error> for ImportError {
    fn from(e: sqlx::Error) -> Self {
        ImportError::Db(e)
    }
}

impl From<PmWriteError> for ImportError {
    fn from(e: PmWriteError) -> Self {
        match e {
            PmWriteError::NotFound(_) => ImportError::NotFound,
            PmWriteError::Conflict(message) | PmWriteError::Validation(message) => {
                ImportError::Model(message.to_string())
            }
            PmWriteError::Db(err) => ImportError::Db(err),
        }
    }
}

pub const IN_PROGRESS: [&str; 4] = ["queued", "cloning", "reading", "analyzing"];

#[derive(Debug, Clone, Serialize)]
pub struct StartImport {
    pub import_id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportSourceView {
    pub repository_id: Uuid,
    pub label: String,
    pub head_commit_sha: Option<String>,
    pub commit_count: i32,
    pub doc_count: i32,
    pub size_bytes: i64,
    pub truncated: bool,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportDetail {
    pub id: Uuid,
    pub project_id: Uuid,
    pub status: String,
    pub progress: i32,
    pub stage_detail: Option<String>,
    pub model_name: String,
    pub draft: Option<ProjectDraft>,
    pub draft_revision: i64,
    pub dropped_items: i32,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
    pub analysis_run_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub proposed_at: Option<DateTime<Utc>>,
    pub applied_at: Option<DateTime<Utc>>,
    pub sources: Vec<ImportSourceView>,
}

/// 项目下的导入批次（审批页分组用）。
#[derive(Debug, Clone, Serialize)]
pub struct ImportListItem {
    pub id: Uuid,
    pub status: String,
    pub progress: i32,
    pub model_name: String,
    pub draft_revision: i64,
    pub analysis_run_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub proposed_at: Option<DateTime<Utc>>,
    pub applied_at: Option<DateTime<Utc>>,
}

pub async fn list_imports(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<ImportListItem>, ImportError> {
    let rows: Vec<ImportListRow> = sqlx::query_as(
        "SELECT id, status, progress, model_name, draft_revision, analysis_run_id, \
                created_at, proposed_at, applied_at \
         FROM integration.repo_import WHERE project_id = $1 ORDER BY created_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, status, progress, model_name, draft_revision, analysis_run_id, created_at, proposed_at, applied_at)| {
                ImportListItem {
                    id,
                    status,
                    progress,
                    model_name,
                    draft_revision,
                    analysis_run_id,
                    created_at,
                    proposed_at,
                    applied_at,
                }
            },
        )
        .collect())
}

/// 启动一次导入：校验空项目 + 至少一个 verified 仓库，落事件与 job。
pub async fn start_import(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
) -> Result<StartImport, ImportError> {
    let project_exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await?;
    if project_exists.is_none() {
        return Err(ImportError::ProjectNotFound);
    }

    let requirement_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.requirement WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(pool)
            .await?;
    if requirement_count > 0 {
        return Err(ImportError::ProjectNotEmpty);
    }

    let verified_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.project_repository \
         WHERE project_id = $1 AND status = 'verified'",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    if verified_count == 0 {
        return Err(ImportError::NoVerifiedRepository);
    }

    let in_progress: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.repo_import \
         WHERE project_id = $1 AND status = ANY($2)",
    )
    .bind(project_id)
    .bind(
        IN_PROGRESS
            .iter()
            .map(|status| status.to_string())
            .collect::<Vec<_>>(),
    )
    .fetch_one(pool)
    .await?;
    if in_progress > 0 {
        return Err(ImportError::ImportInProgress);
    }

    let actor_id = actor.principal_id;
    let import_id = Uuid::new_v4();
    let payload = serde_json::json!({ "import_id": import_id });
    let payload_text = payload.to_string();
    let payload_hash = crate::knowledge::content_hash(&payload_text);

    let mut tx = pool.begin().await?;
    let event_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.inbound_event \
         (project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash, tenant_id) \
         VALUES ($1, $2, 'repo_import', 'repo_import_requested', $3, $4, $5, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(actor_id)
    .bind(import_id.to_string())
    .bind(&payload)
    .bind(&payload_hash)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO integration.job (project_id, event_id, job_type, tenant_id) \
         VALUES ($1, $2, 'repo_import', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(event_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO integration.repo_import \
         (id, project_id, requested_by, status, progress, model_name, tenant_id) \
         VALUES ($1, $2, $3, 'queued', 0, 'fake', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(import_id)
    .bind(project_id)
    .bind(actor_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE core.project SET status = 'initializing' WHERE id = $1 AND status = 'planning'")
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(StartImport {
        import_id,
        status: "queued".to_string(),
    })
}

/// 读取导入详情；`queued/cloning/reading/analyzing` 阶段不返回草案。
pub async fn get_import(
    pool: &PgPool,
    project_id: Uuid,
    import_id: Uuid,
) -> Result<ImportDetail, ImportError> {
    let row: Option<ImportDetailRow> = sqlx::query_as(
        "SELECT project_id, status, progress, stage_detail, model_name, draft, \
                draft_revision, dropped_items, error_code, error_detail, analysis_run_id, \
                created_at, proposed_at, applied_at \
         FROM integration.repo_import WHERE id = $1 AND project_id = $2",
    )
    .bind(import_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or(ImportError::NotFound)?;
    let (
        project_id,
        status,
        progress,
        stage_detail,
        model_name,
        draft_value,
        draft_revision,
        dropped_items,
        error_code,
        error_detail,
        analysis_run_id,
        created_at,
        proposed_at,
        applied_at,
    ) = row;
    let draft = if IN_PROGRESS.contains(&status.as_str()) {
        None
    } else {
        draft_value.and_then(|value| serde_json::from_value::<ProjectDraft>(value).ok())
    };

    let sources: Vec<ImportSourceRow> =
        sqlx::query_as(
            "SELECT s.repository_id, r.name, s.head_commit_sha, s.commit_count, s.doc_count, \
                    s.size_bytes, s.truncated, s.error_code \
             FROM integration.repo_import_source s \
             JOIN integration.project_repository r ON r.id = s.repository_id \
             WHERE s.import_id = $1 ORDER BY r.name",
        )
        .bind(import_id)
        .fetch_all(pool)
        .await?;

    Ok(ImportDetail {
        id: import_id,
        project_id,
        status,
        progress,
        stage_detail,
        model_name,
        draft,
        draft_revision,
        dropped_items,
        error_code,
        error_detail,
        analysis_run_id,
        created_at,
        proposed_at,
        applied_at,
        sources: sources
            .into_iter()
            .map(
                |(repository_id, label, head_commit_sha, commit_count, doc_count, size_bytes, truncated, error_code)| {
                    ImportSourceView {
                        repository_id,
                        label,
                        head_commit_sha,
                        commit_count,
                        doc_count,
                        size_bytes,
                        truncated,
                        error_code,
                    }
                },
            )
            .collect(),
    })
}

/// 取消导入：只允许在草案生成（`draft_ready`）及之前。
pub async fn cancel_import(
    pool: &PgPool,
    project_id: Uuid,
    import_id: Uuid,
) -> Result<(), ImportError> {
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM integration.repo_import WHERE id = $1 AND project_id = $2")
            .bind(import_id)
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    let status = status.ok_or(ImportError::NotFound)?;
    if !IN_PROGRESS.contains(&status.as_str()) && status != "draft_ready" {
        return Err(ImportError::NotCancellable);
    }
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE integration.repo_import SET status = 'cancelled', updated_at = now() WHERE id = $1")
        .bind(import_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE integration.job SET status = 'cancelled' \
         WHERE event_id = (SELECT id FROM integration.inbound_event WHERE idempotency_key = $1)",
    )
    .bind(import_id.to_string())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn set_stage(
    pool: &PgPool,
    import_id: Uuid,
    status: &str,
    detail: &str,
    progress: i32,
) -> Result<(), ImportError> {
    sqlx::query(
        "UPDATE integration.repo_import SET status = $2, stage_detail = $3, progress = $4, \
         updated_at = now() WHERE id = $1",
    )
    .bind(import_id)
    .bind(status)
    .bind(detail)
    .bind(progress)
    .execute(pool)
    .await?;
    Ok(())
}

async fn fail_import(pool: &PgPool, import_id: Uuid, code: &str, detail: &str) {
    let detail: String = detail.chars().take(500).collect();
    let _ = sqlx::query(
        "UPDATE integration.repo_import SET status = 'failed', error_code = $2, error_detail = $3, \
         updated_at = now() WHERE id = $1",
    )
    .bind(import_id)
    .bind(code)
    .bind(detail)
    .execute(pool)
    .await;
}

fn model_error_code(err: &ModelError) -> (&'static str, String) {
    match err {
        ModelError::Timeout => ("model_timeout", err.to_string()),
        ModelError::Unavailable(reason) => ("model_unavailable", reason.clone()),
        ModelError::BadOutput(reason) => ("bad_output", reason.clone()),
    }
}

fn git_error_code(err: &git_access::GitError) -> (&'static str, String) {
    use git_access::GitError;
    match err {
        GitError::Timeout => ("git_timeout", err.to_string()),
        GitError::AuthFailed => ("git_auth_failed", err.to_string()),
        GitError::NotFound => ("git_not_found", err.to_string()),
        GitError::TooLarge => ("repo_too_large", err.to_string()),
        GitError::Other(message) => ("git_failed", message.clone()),
        GitError::Io(message) => ("git_failed", message.clone()),
    }
}

/// 执行一次导入 job。Fake 模型也能跑通 `queued → draft_ready`。
pub async fn run_import_job(
    pool: &PgPool,
    model: &dyn RepoAnalysisModel,
    import_id: Uuid,
) -> Result<(), ImportError> {
    let import: Option<(Uuid, String)> =
        sqlx::query_as("SELECT project_id, status FROM integration.repo_import WHERE id = $1")
            .bind(import_id)
            .fetch_optional(pool)
            .await?;
    let (project_id, status) = import.ok_or(ImportError::NotFound)?;
    if status == "cancelled" {
        return Ok(());
    }
    if !IN_PROGRESS.contains(&status.as_str()) {
        return Ok(());
    }

    set_stage(pool, import_id, "cloning", "克隆仓库", 10).await?;

    let repos: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT id, url, default_branch FROM integration.project_repository \
         WHERE project_id = $1 AND status = 'verified' ORDER BY name",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    if repos.is_empty() {
        fail_import(pool, import_id, "no_verified_repository", "no verified repository").await;
        return Ok(());
    }

    let key_ciphertext: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT private_key_ciphertext FROM integration.project_ssh_key \
         WHERE project_id = $1 AND status = 'active'",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let key_material = match key_ciphertext {
        Some(bytes) => {
            let private = crate::repository::open_ssh_private_key(project_id, &bytes)
                .map_err(|e| ImportError::Git(e.to_string()))?;
            Some(
                git_access::write_temp_key(&private).map_err(|e| ImportError::Git(e.to_string()))?,
            )
        }
        None => None,
    };
    let key_path = key_material.as_ref().map(|m| m.key_path());

    let limits = repo_facts::Limits::from_env();
    let mut materials: Vec<RepoMaterial> = Vec::new();
    for (repository_id, url, default_branch) in repos {
        let dest = repo_facts::repo_cache_dir(repository_id);
        let _ = std::fs::remove_dir_all(&dest);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(err) = git_access::clone_bare(
            &url,
            key_path.as_deref(),
            &dest,
            std::time::Duration::from_secs(600),
        )
        .await
        {
            let (code, detail) = git_error_code(&err);
            record_source_error(pool, import_id, repository_id, code).await;
            fail_import(pool, import_id, code, &detail).await;
            return Ok(());
        }

        let branch = match default_branch {
            Some(branch) => branch,
            None => match git_access::ls_remote(&url, key_path.as_deref(), std::time::Duration::from_secs(30)).await {
                Ok(heads) => heads
                    .iter()
                    .find(|h| h.branch == "main")
                    .or_else(|| heads.iter().find(|h| h.branch == "master"))
                    .or_else(|| heads.first())
                    .map(|h| h.branch.clone())
                    .unwrap_or_else(|| "main".to_string()),
                Err(_) => "main".to_string(),
            },
        };

        let facts = match repo_facts::collect(&dest, &branch, limits).await {
            Ok(facts) => facts,
            Err(err) => {
                let (code, detail) = git_error_code(&err);
                record_source_error(pool, import_id, repository_id, code).await;
                fail_import(pool, import_id, code, &detail).await;
                return Ok(());
            }
        };

        let facts_json = serde_json::to_value(&facts).unwrap_or(serde_json::Value::Null);
        sqlx::query(
            "INSERT INTO integration.repo_import_source \
             (import_id, repository_id, head_commit_sha, commit_count, doc_count, size_bytes, truncated, facts, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, \
                     (SELECT tenant_id FROM integration.repo_import WHERE id = $1)) \
             ON CONFLICT (import_id, repository_id) DO UPDATE SET \
                 head_commit_sha = EXCLUDED.head_commit_sha, commit_count = EXCLUDED.commit_count, \
                 doc_count = EXCLUDED.doc_count, size_bytes = EXCLUDED.size_bytes, \
                 truncated = EXCLUDED.truncated, facts = EXCLUDED.facts, error_code = NULL",
        )
        .bind(import_id)
        .bind(repository_id)
        .bind(&facts.head_commit_sha)
        .bind(facts.commits.len() as i32)
        .bind(facts.docs.len() as i32)
        .bind(facts.size_bytes as i64)
        .bind(facts.truncated)
        .bind(&facts_json)
        .execute(pool)
        .await?;
        materials.push(RepoMaterial {
            repository_id: repository_id.to_string(),
            facts,
        });
    }

    set_stage(pool, import_id, "reading", "读取 commit 与 docs", 45).await?;
    set_stage(pool, import_id, "analyzing", "AI 分析", 70).await?;

    let draft = match model
        .draft(&materials, &pm_domain::import::OrgChart::p0())
        .await
    {
        Ok(draft) => draft,
        Err(err) => {
            let (code, detail) = model_error_code(&err);
            fail_import(pool, import_id, code, &detail).await;
            return Ok(());
        }
    };

    let (grounded, dropped) = ground_draft(&draft, &materials);
    if grounded.requirements.is_empty() && grounded.tasks.is_empty() {
        fail_import(
            pool,
            import_id,
            "no_grounded_output",
            "all draft items lacked verifiable evidence",
        )
        .await;
        return Ok(());
    }
    let draft_json = serde_json::to_value(&grounded).unwrap_or(serde_json::Value::Null);
    sqlx::query(
        "UPDATE integration.repo_import SET status = 'draft_ready', stage_detail = '草案已生成', \
         progress = 100, draft = $2, draft_revision = draft_revision + 1, \
         dropped_items = $3, updated_at = now() WHERE id = $1",
    )
    .bind(import_id)
    .bind(&draft_json)
    .bind(dropped as i32)
    .execute(pool)
    .await?;
    Ok(())
}

async fn record_source_error(pool: &PgPool, import_id: Uuid, repository_id: Uuid, code: &str) {
    let _ = sqlx::query(
        "INSERT INTO integration.repo_import_source \
         (import_id, repository_id, error_code, tenant_id) \
         VALUES ($1, $2, $3, \
                 (SELECT tenant_id FROM integration.repo_import WHERE id = $1)) \
         ON CONFLICT (import_id, repository_id) DO UPDATE SET error_code = EXCLUDED.error_code",
    )
    .bind(import_id)
    .bind(repository_id)
    .bind(code)
    .execute(pool)
    .await;
}

/// 由 job 的入站事件反查导入 id（worker 用）。
pub async fn import_id_for_event(pool: &PgPool, event_id: Uuid) -> Result<Option<Uuid>, ImportError> {
    let key: Option<String> =
        sqlx::query_scalar("SELECT idempotency_key FROM integration.inbound_event WHERE id = $1")
            .bind(event_id)
            .fetch_optional(pool)
            .await?;
    Ok(key.and_then(|key| key.parse::<Uuid>().ok()))
}

fn commit_matches(materials: &[RepoMaterial], repo_id: &str, reference: &str) -> bool {
    let reference = reference.trim().to_ascii_lowercase();
    if reference.len() < 7
        || reference.len() > 40
        || !reference.chars().all(|c| c.is_ascii_hexdigit())
    {
        return false;
    }
    materials.iter().any(|material| {
        material.repository_id == repo_id
            && material
                .facts
                .commits
                .iter()
                .any(|commit| commit.sha.to_ascii_lowercase().starts_with(&reference))
    })
}

fn doc_matches(materials: &[RepoMaterial], repo_id: &str, reference: &str) -> bool {
    materials.iter().any(|material| {
        material.repository_id == repo_id
            && material.facts.docs.iter().any(|doc| doc.path == reference)
    })
}

fn evidence_grounded(evidence: &Evidence, materials: &[RepoMaterial]) -> bool {
    match evidence.kind {
        EvidenceKind::Doc => doc_matches(materials, &evidence.repo_id, &evidence.r#ref),
        EvidenceKind::Commit => commit_matches(materials, &evidence.repo_id, &evidence.r#ref),
    }
}

/// 证据接地：无真实 doc 路径或 commit sha 支撑的需求/任务被丢弃并计数。
pub fn ground_draft(draft: &ProjectDraft, materials: &[RepoMaterial]) -> (ProjectDraft, usize) {
    let mut grounded = draft.clone();
    let before = grounded.requirements.len() + grounded.tasks.len();

    grounded.requirements.retain(|requirement| {
        requirement
            .evidence
            .iter()
            .any(|evidence| evidence_grounded(evidence, materials))
    });
    grounded.tasks.retain(|task| {
        task.evidence
            .iter()
            .any(|evidence| evidence_grounded(evidence, materials))
    });
    for risk in &mut grounded.risks {
        risk.evidence
            .retain(|evidence| evidence_grounded(evidence, materials));
    }

    let after = grounded.requirements.len() + grounded.tasks.len();
    (grounded, before - after)
}
// ------------------------------------------------------------------ apply

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyProject {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyRequest {
    pub draft_revision: i64,
    pub project: ApplyProject,
    #[serde(default)]
    pub milestone_keys: Vec<String>,
    #[serde(default)]
    pub requirement_keys: Vec<String>,
    #[serde(default)]
    pub task_keys: Vec<String>,
    #[serde(default)]
    pub write_knowledge_documents: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProposalRef {
    pub kind: String,
    pub proposal_id: Uuid,
    pub approval_id: Uuid,
    pub item_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProposedImport {
    pub outcome: String,
    pub proposals: Vec<ProposalRef>,
    pub analysis_run_id: Uuid,
}

fn lowest_confidence(draft: &ProjectDraft) -> &'static str {
    use pm_domain::proposal::Confidence;
    let mut lowest = Confidence::High;
    for confidence in draft
        .requirements
        .iter()
        .map(|item| item.confidence)
        .chain(draft.tasks.iter().map(|item| item.confidence))
    {
        lowest = match (lowest, confidence) {
            (Confidence::Low, _) | (_, Confidence::Low) => Confidence::Low,
            (Confidence::Medium, _) | (_, Confidence::Medium) => Confidence::Medium,
            _ => Confidence::High,
        };
    }
    match lowest {
        Confidence::Low => "low",
        Confidence::Medium => "medium",
        Confidence::High => "high",
    }
}

fn evidence_labels(items: impl Iterator<Item = String>) -> Vec<String> {
    let mut labels: Vec<String> = items.collect();
    labels.sort();
    labels.dedup();
    labels.truncate(50);
    labels
}

fn profile_markdown(draft: &ProjectDraft) -> String {
    let mut out = format!(
        "# {}（{}）\n\n{}\n\n",
        draft.profile.suggested_name, draft.profile.suggested_code, draft.profile.summary
    );
    if !draft.profile.tech_stack.is_empty() {
        out.push_str(&format!(
            "## 技术栈\n\n{}\n\n",
            draft.profile.tech_stack.join(", ")
        ));
    }
    if !draft.risks.is_empty() {
        out.push_str("## 风险\n\n");
        for risk in &draft.risks {
            out.push_str(&format!("- [{}] {}\n", risk.severity, risk.description));
        }
        out.push('\n');
    }
    if !draft.unknowns.is_empty() {
        out.push_str("## 待确认问题\n\n");
        for unknown in &draft.unknowns {
            out.push_str(&format!(
                "- {}（阻断原因：{}）\n",
                unknown.question, unknown.why_blocking
            ));
        }
        out.push('\n');
    }
    out.push_str("> 本文由仓库冷启动导入生成，需人工确认。\n");
    out
}

/// 把草案按类型转成 ≤4 条提案。只写 `ai.analysis_run` / `ai.change_proposal` /
/// `governance.approval_request`，`delivery.*` 零直接写入。
pub async fn propose_import(
    pool: &PgPool,
    project_id: Uuid,
    import_id: Uuid,
    req: ApplyRequest,
) -> Result<ProposedImport, ImportError> {
    let existing: Option<(String, Option<Uuid>, Option<serde_json::Value>, i64)> = sqlx::query_as(
        "SELECT status, analysis_run_id, draft, draft_revision \
         FROM integration.repo_import WHERE id = $1 AND project_id = $2",
    )
    .bind(import_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let (status, analysis_run_id, draft_value, draft_revision) =
        existing.ok_or(ImportError::NotFound)?;

    if status == "applied" {
        return Err(ImportError::AlreadyApplied);
    }
    if status == "proposed" {
        let run_id = analysis_run_id.ok_or(ImportError::InvalidState("proposed without run"))?;
        return existing_proposals(pool, project_id, run_id).await;
    }
    if status != "draft_ready" {
        return Err(ImportError::InvalidState("import is not ready to apply"));
    }
    if draft_revision != req.draft_revision {
        return Err(ImportError::StaleDraft);
    }
    let draft: ProjectDraft = draft_value
        .and_then(|value| serde_json::from_value(value).ok())
        .ok_or(ImportError::InvalidState("draft is missing"))?;

    let selected_milestones: Vec<_> = draft
        .milestones
        .iter()
        .filter(|item| req.milestone_keys.contains(&item.key))
        .cloned()
        .collect();
    let selected_requirements: Vec<_> = draft
        .requirements
        .iter()
        .filter(|item| req.requirement_keys.contains(&item.key))
        .cloned()
        .collect();
    let selected_tasks: Vec<_> = draft
        .tasks
        .iter()
        .filter(|item| req.task_keys.contains(&item.key))
        .cloned()
        .collect();
    if selected_milestones.is_empty()
        && selected_requirements.is_empty()
        && selected_tasks.is_empty()
        && !req.write_knowledge_documents
    {
        return Err(ImportError::NoSelection);
    }

    let code_owner: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM core.project WHERE code = $1 AND id <> $2")
            .bind(&req.project.code)
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    if code_owner.is_some() {
        return Err(ImportError::ProjectCodeTaken);
    }

    let event_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM integration.inbound_event \
         WHERE project_id = $1 AND idempotency_key = $2",
    )
    .bind(project_id)
    .bind(import_id.to_string())
    .fetch_one(pool)
    .await?;

    let milestone_names: std::collections::HashMap<String, String> = draft
        .milestones
        .iter()
        .map(|item| (item.key.clone(), item.name.clone()))
        .collect();
    let requirement_codes: std::collections::HashMap<String, String> = draft
        .requirements
        .iter()
        .map(|item| (item.key.clone(), item.code.clone()))
        .collect();

    let mut commands: Vec<(String, pm_domain::ProjectCommand, Vec<String>)> = Vec::new();
    if !selected_milestones.is_empty() {
        let evidence = evidence_labels(selected_milestones.iter().map(|item| item.key.clone()));
        commands.push((
            "create_milestones".to_string(),
            pm_domain::ProjectCommand::CreateMilestones {
                milestones: selected_milestones.clone(),
            },
            evidence,
        ));
    }
    if !selected_requirements.is_empty() {
        let evidence = evidence_labels(selected_requirements.iter().flat_map(|item| {
            item.evidence
                .iter()
                .map(|ev| format!("{}:{}", ev.repo_id, ev.r#ref))
        }));
        commands.push((
            "create_requirements".to_string(),
            pm_domain::ProjectCommand::CreateRequirements {
                requirements: selected_requirements.clone(),
            },
            evidence,
        ));
    }
    if !selected_tasks.is_empty() {
        let tasks: Vec<pm_domain::import::TaskDraft> = selected_tasks
            .iter()
            .map(|task| {
                let mut copy = task.clone();
                copy.milestone_key = task
                    .milestone_key
                    .as_ref()
                    .and_then(|key| milestone_names.get(key).cloned());
                copy.requirement_key = task
                    .requirement_key
                    .as_ref()
                    .and_then(|key| requirement_codes.get(key).cloned());
                copy
            })
            .collect();
        let evidence = evidence_labels(selected_tasks.iter().flat_map(|item| {
            item.evidence
                .iter()
                .map(|ev| format!("{}:{}", ev.repo_id, ev.r#ref))
        }));
        commands.push((
            "create_tasks".to_string(),
            pm_domain::ProjectCommand::CreateTasks { tasks },
            evidence,
        ));
    }
    if req.write_knowledge_documents {
        let mut documents: Vec<pm_domain::import::DocumentDraft> = selected_requirements
            .iter()
            .map(|item| pm_domain::import::DocumentDraft {
                doc_key: item.code.clone(),
                doc_type: "requirement".to_string(),
                markdown: if item.markdown.is_empty() {
                    format!("# {}\n\n", item.title)
                } else {
                    item.markdown.clone()
                },
                change_summary: Some("冷启动导入：来自仓库事实".to_string()),
            })
            .collect();
        documents.push(pm_domain::import::DocumentDraft {
            doc_key: "project-profile".to_string(),
            doc_type: "knowledge".to_string(),
            markdown: profile_markdown(&draft),
            change_summary: Some("冷启动导入：项目画像".to_string()),
        });
        commands.push((
            "write_documents".to_string(),
            pm_domain::ProjectCommand::WriteDocuments { documents },
            Vec::new(),
        ));
    }

    let confidence = lowest_confidence(&draft);
    let model_name: String =
        sqlx::query_scalar("SELECT model_name FROM integration.repo_import WHERE id = $1")
            .bind(import_id)
            .fetch_one(pool)
            .await?;
    let rationale = "AI 项目经理读取代码仓库 commit 与 docs 后生成的冷启动提案；执行前须人工确认。";

    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE core.project SET code = $2, name = $3, description = $4, updated_at = now() \
         WHERE id = $1",
    )
    .bind(project_id)
    .bind(&req.project.code)
    .bind(&req.project.name)
    .bind(&req.project.description)
    .execute(&mut *tx)
    .await?;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.analysis_run \
         (project_id, event_id, status, model_name, policy_version, config_snapshot, result, finished_at, tenant_id) \
         VALUES ($1, $2, 'succeeded', $3, 1, '{}'::jsonb, $4, now(), \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(event_id)
    .bind(&model_name)
    .bind(serde_json::json!({ "import_id": import_id }))
    .fetch_one(&mut *tx)
    .await?;

    let mut proposals = Vec::new();
    for (seq, (kind, command, evidence)) in commands.into_iter().enumerate() {
        let item_count = match &command {
            pm_domain::ProjectCommand::CreateMilestones { milestones } => milestones.len(),
            pm_domain::ProjectCommand::CreateRequirements { requirements } => requirements.len(),
            pm_domain::ProjectCommand::CreateTasks { tasks } => tasks.len(),
            pm_domain::ProjectCommand::WriteDocuments { documents } => documents.len(),
            _ => 0,
        };
        let proposed_data = serde_json::to_value(&command)
            .map_err(|e| ImportError::Model(format!("command serialization failed: {e}")))?;
        let evidence_value = serde_json::to_value(&evidence).unwrap_or(serde_json::Value::Null);
        let proposal_id: Uuid = sqlx::query_scalar(
            "INSERT INTO ai.change_proposal \
             (project_id, analysis_run_id, proposal_seq, target_type, target_id, command_type, \
              proposed_data, rationale, evidence, risk_level, confidence, status, tenant_id) \
             VALUES ($1, $2, $3, 'project', $4, $5, $6, $7, $8, 'high', $9, 'redline_pending', \
                     (SELECT tenant_id FROM core.project WHERE id = $1)) \
             RETURNING id",
        )
        .bind(project_id)
        .bind(run_id)
        .bind((seq + 1) as i32)
        .bind(project_id)
        .bind(&kind)
        .bind(&proposed_data)
        .bind(rationale)
        .bind(&evidence_value)
        .bind(confidence)
        .fetch_one(&mut *tx)
        .await?;
        let approval_id: Uuid = sqlx::query_scalar(
            "INSERT INTO governance.approval_request \
             (project_id, proposal_id, required_role, tenant_id) \
             VALUES ($1, $2, 'project_owner', \
                     (SELECT tenant_id FROM ai.change_proposal WHERE id = $2)) RETURNING id",
        )
        .bind(project_id)
        .bind(proposal_id)
        .fetch_one(&mut *tx)
        .await?;
        proposals.push(ProposalRef {
            kind,
            proposal_id,
            approval_id,
            item_count,
        });
    }

    sqlx::query(
        "UPDATE integration.repo_import SET status = 'proposed', analysis_run_id = $2, \
         proposed_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(import_id)
    .bind(run_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

Ok(ProposedImport {
        outcome: "proposed".to_string(),
        proposals,
        analysis_run_id: run_id,
    })
}

/// 已生成提案的批次（审批页整批批准用）。
pub async fn import_proposals(
    pool: &PgPool,
    project_id: Uuid,
    import_id: Uuid,
) -> Result<ProposedImport, ImportError> {
    let row: Option<(String, Option<Uuid>)> = sqlx::query_as(
        "SELECT status, analysis_run_id FROM integration.repo_import \
         WHERE id = $1 AND project_id = $2",
    )
    .bind(import_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let (status, analysis_run_id) = row.ok_or(ImportError::NotFound)?;
    if status != "proposed" && status != "applied" {
        return Err(ImportError::InvalidState("import has no proposals yet"));
    }
    let run_id = analysis_run_id.ok_or(ImportError::InvalidState("import has no analysis run"))?;
    existing_proposals(pool, project_id, run_id).await
}

/// 审批决策后推进导入状态：
/// - 任一提案被驳回 → import `rejected`（已批准部分不回滚）；
/// - 全部 executed → import `applied` + 项目 `planning` + 审计；
/// - 存在 failed → import 保持 `proposed` 并标 `partial`。
pub async fn advance_import_after_decision(
    pool: &PgPool,
    analysis_run_id: Uuid,
    actor: &AuditActor,
) -> Result<(), ImportError> {
    let import: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, project_id FROM integration.repo_import WHERE analysis_run_id = $1",
    )
    .bind(analysis_run_id)
    .fetch_optional(pool)
    .await?;
    let Some((import_id, project_id)) = import else {
        return Ok(());
    };
    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM ai.change_proposal WHERE analysis_run_id = $1")
            .bind(analysis_run_id)
            .fetch_all(pool)
            .await?;
    if statuses.is_empty() {
        return Ok(());
    }

    if statuses.iter().any(|status| status == "vetoed") {
        sqlx::query(
            "UPDATE integration.repo_import SET status = 'rejected', updated_at = now() \
             WHERE id = $1",
        )
        .bind(import_id)
        .execute(pool)
        .await?;
        return Ok(());
    }
    if statuses.iter().all(|status| status == "executed") {
        let mut tx = pool.begin().await?;
        sqlx::query(
            "UPDATE integration.repo_import SET status = 'applied', applied_at = now(), \
             error_code = NULL, error_detail = NULL, updated_at = now() WHERE id = $1",
        )
        .bind(import_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE core.project SET status = 'planning', updated_at = now() WHERE id = $1",
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO governance.audit_log \
             (project_id, actor_id, action, target_type, target_id, after_data, analysis_run_id, tenant_id) \
             VALUES ($1, $2, 'project_import_applied', 'project', $1, $3, $4, \
                     (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(project_id)
        .bind(actor.principal_id)
        .bind(serde_json::json!({ "import_id": import_id }))
        .bind(analysis_run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(());
    }
    if statuses.iter().any(|status| status == "failed") {
        sqlx::query(
            "UPDATE integration.repo_import SET status = 'proposed', error_code = 'partial', \
             error_detail = 'some import proposals failed to execute', updated_at = now() \
             WHERE id = $1",
        )
        .bind(import_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn existing_proposals(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
) -> Result<ProposedImport, ImportError> {
    let rows: Vec<(String, Uuid, Uuid)> = sqlx::query_as(
        "SELECT p.command_type, p.id, a.id \
         FROM ai.change_proposal p \
         JOIN governance.approval_request a ON a.proposal_id = p.id \
         WHERE p.project_id = $1 AND p.analysis_run_id = $2 ORDER BY p.proposal_seq",
    )
    .bind(project_id)
    .bind(run_id)
    .fetch_all(pool)
    .await?;
    let proposals = rows
        .into_iter()
        .map(|(kind, proposal_id, approval_id)| ProposalRef {
            kind,
            proposal_id,
            approval_id,
            item_count: 0,
        })
        .collect();
    Ok(ProposedImport {
        outcome: "proposed".to_string(),
        proposals,
        analysis_run_id: run_id,
    })
}