//! P0 外部 Commit 采纳闭环：发现 → AI 分析 → 澄清 → 人类采纳 → 关闭。
//!
//! 硬约束（迁移 0054 头注同步）：
//! * **发现 ≠ 采纳**：状态机 `discovered → analyzed → awaiting_adoption →
//!   adopted → closed`（分析失败入 `failed` 可重试），CHECK 约束强制
//!   「未分析不得采纳」。
//! * 采纳决定（approved / no_update / rejected）一律人类主体
//!   （[`crate::requirement::require_human`]）；AI 分析产物恒 draft/unverified。
//! * 知识更新不直接写权威正文：approve 把分析期已生成、人已见其正文的
//!   拟稿交 [`crate::knowledge_proposal::submit_proposal`]（质量门 + 红线
//!   人类确认 + Markdown+Git 写入协调），本模块只记录提案引用。
//! * 防重复发现：`UNIQUE(repository_id, commit_sha)` + 扫描锚点
//!   `last_external_scan_commit_sha`，重复扫描 ON CONFLICT DO NOTHING。
//!
//! LLM 分层纪律与 [`crate::snapshot_analysis`] 同款：本层只依赖
//! [`CommitAnalysisModel`] trait；实现放在 `pm-ai`（provider 路由 + 记账）。
//! `Ok(None)` = 无可用 provider → 结构化派生（[`derived_analysis`]，
//! analysis_mode='derived'）；`Err(_)` = 调用/解析失败 → 行入 `failed`
//! 可重试（派生回退只适用于「无 provider」，调用失败必须显式可见）。

use std::collections::HashSet;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::git_access::{self, GitError};
use crate::repo_facts::{self, pm_ai_port::ModelError};
use crate::repository::{fallback_key_ciphertext, key_ciphertext_by_id, open_ssh_private_key};

const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
const CLONE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
const GIT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// 单次扫描最多登记的新提交数（超出部分留在远端，本次只标记 truncated）。
const MAX_SCAN_COMMITS: usize = 50;
/// 单提交登记的变更文件上限（超出截断，防 jsonb 失控）。
const MAX_CHANGED_FILES: usize = 200;
/// 分析用 diff 摘录字符上限。
const MAX_DIFF_EXCERPT_CHARS: usize = 32_000;
/// UI 直接读 diff 的上限。
const MAX_DIFF_VIEW_CHARS: usize = 64_000;
/// 候选关联对象上限（每类）。
const MAX_CANDIDATES_PER_KIND: usize = 5;
/// 分析期知识更新建议上限。
const MAX_KNOWLEDGE_UPDATES: usize = 3;
/// 澄清问题上限。
const MAX_CLARIFICATION_QUESTIONS: usize = 5;
/// 拟稿正文字符上限。
const MAX_PROPOSED_MARKDOWN_CHARS: usize = 24_000;
/// 拟稿上下文（当前 published 正文）字符上限。
const MAX_DOC_CONTEXT_CHARS: usize = 8_000;

// ── 领域类型 ──────────────────────────────────────────────────────────────

/// 变更文件（diff-tree --name-status；change ∈ A/M/D/R…）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub change: String,
}

/// 结构化候选：从项目数据库按词面匹配出的可能关联对象（恒可追溯，不由 LLM 发明）。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RelatedCandidate {
    pub domain: String, // requirement | task | knowledge
    pub id: String,
    pub title: String,
    /// 命中原因（subject_match / path_match）。
    pub reason: String,
}

/// 分析可引用的既有知识文档（doc_key 白名单 + 当前正文摘要）。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KnownDoc {
    pub doc_key: String,
    pub doc_type: String,
    #[serde(skip)]
    pub current_markdown: Option<String>,
    pub current_revision_id: Option<Uuid>,
}

/// LLM 分析端口输入（全部为可追溯事实，LLM 不得发明 id/doc_key）。
pub struct CommitAnalysisInput<'a> {
    pub project_id: Uuid,
    pub repository_name: &'a str,
    pub commit_sha: &'a str,
    pub author: &'a str,
    pub committed_at: &'a str,
    pub subject: &'a str,
    pub branch: &'a str,
    pub changed_files: &'a [ChangedFile],
    pub diff_excerpt: &'a str,
    pub diff_truncated: bool,
    pub candidates: &'a [RelatedCandidate],
    pub known_docs: &'a [KnownDoc],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelatedObject {
    pub domain: String,
    pub id: String,
    pub title: String,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviationJudgment {
    pub detected: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnowledgeUpdateSuggestion {
    pub doc_key: String,
    pub doc_type: String,
    pub title: String,
    pub summary: String,
    /// 拟稿正文（人批准前可见；derived 模式不产出建议，故恒有值）。
    pub proposed_markdown: String,
}

/// 分析产物（落 `analysis` jsonb；采纳决定只认这份内容）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitAnalysis {
    pub summary: String,
    pub analysis_mode: String, // llm | derived
    pub related_objects: Vec<RelatedObject>,
    pub deviation: Option<DeviationJudgment>,
    pub knowledge_updates: Vec<KnowledgeUpdateSuggestion>,
}

/// LLM 分析端口。实现放 `pm-ai`（provider 路由 + `ai.llm_call_record` 记账）。
#[async_trait::async_trait]
pub trait CommitAnalysisModel: Send + Sync {
    /// `Ok(None)` = 无可用 provider（上层结构化派生）；`Err` = 调用/解析失败。
    async fn analyze_commit(
        &self,
        input: &CommitAnalysisInput<'_>,
    ) -> Result<Option<CommitAnalysisOutput>, ModelError>;
}

/// 模型完整输出：落库分析 + 澄清问题（问题落 clarification 行，不入 analysis jsonb）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitAnalysisOutput {
    pub analysis: CommitAnalysis,
    pub clarification_questions: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExternalCommitError {
    #[error("external commit not found")]
    NotFound,
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error("human principal required")]
    Forbidden(String),
    #[error("git operation failed: {0}")]
    Git(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("knowledge pipeline error: {0}")]
    Knowledge(#[from] crate::knowledge::KnowledgeError),
}

impl ExternalCommitError {
    /// 稳定错误码（前端文案映射）。
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "external_commit_not_found",
            Self::Validation(_) => "external_commit_validation",
            Self::Conflict(_) => "external_commit_conflict",
            Self::Forbidden(_) => "human_principal_required",
            Self::Git(_) => "git_failed",
            Self::Db(_) => "internal",
            Self::Knowledge(_) => "knowledge_pipeline_failed",
        }
    }
}

impl From<GitError> for ExternalCommitError {
    fn from(e: GitError) -> Self {
        git_err(e)
    }
}

impl From<crate::repository::RepositoryError> for ExternalCommitError {
    fn from(e: crate::repository::RepositoryError) -> Self {
        use crate::repository::RepositoryError as E;
        match e {
            E::ProjectNotFound | E::NotFound | E::SshKeyNotFound => Self::NotFound,
            E::Archived => Self::Conflict("repository is archived".to_string()),
            other => Self::Validation(format!("repository error: {other}")),
        }
    }
}

impl From<crate::pm_write::PmWriteError> for ExternalCommitError {
    fn from(e: crate::pm_write::PmWriteError) -> Self {
        use crate::pm_write::PmWriteError as E;
        match e {
            E::Forbidden(msg) => Self::Forbidden(msg.to_string()),
            E::NotFound(_) => Self::NotFound,
            E::Db(db) => Self::Db(db),
            E::Conflict(msg) | E::Validation(msg) => Self::Validation(msg.to_string()),
            E::ValidationMsg(msg) => Self::Validation(msg),
        }
    }
}

// ── 行读取 ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ExternalCommitRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub repository_id: Uuid,
    pub commit_sha: String,
    pub branch: String,
    pub author_name: String,
    pub committed_at: DateTime<Utc>,
    pub subject: String,
    pub changed_files: serde_json::Value,
    pub status: String,
    pub analysis: Option<serde_json::Value>,
    pub analysis_mode: Option<String>,
    pub analyzed_at: Option<DateTime<Utc>>,
    pub clarification_state: String,
    pub adoption_decision: Option<String>,
    pub adoption_note: Option<String>,
    pub adopted_by: Option<Uuid>,
    pub adopted_at: Option<DateTime<Utc>>,
    pub knowledge_proposals: serde_json::Value,
    pub adopted_revision_ids: serde_json::Value,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub discovered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ClarificationRow {
    pub id: Uuid,
    pub external_commit_id: Uuid,
    pub question: String,
    pub answer: Option<String>,
    pub answered_by: Option<Uuid>,
    pub answered_at: Option<DateTime<Utc>>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

// ── 缓存与仓库访问 ────────────────────────────────────────────────────────

/// 仓库访问信息（ensure_repo_cache 返回）。
struct RepoAccess {
    name: String,
    branch: String,
    selected_commit: Option<String>,
    last_scan_anchor: Option<String>,
}

fn is_safe_ref(name: &str) -> bool {
    !name.is_empty()
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
}

fn git_err(e: GitError) -> ExternalCommitError {
    let code = e.stable_code();
    ExternalCommitError::Git(format!("{code}: {e}"))
}

/// 加载仓库访问信息 + 确保 bare clone 缓存可用（fetch 刷新或 fresh clone）。
async fn ensure_repo_cache(
    pool: &PgPool,
    repository_id: Uuid,
) -> Result<(RepoAccess, std::path::PathBuf), ExternalCommitError> {
    let row = sqlx::query(
        "SELECT project_id, name, url, default_branch, selected_branch, selected_commit, \
                ssh_key_id, status, last_external_scan_commit_sha \
         FROM integration.project_repository WHERE id = $1",
    )
    .bind(repository_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ExternalCommitError::NotFound)?;
    let project_id: Uuid = row.get("project_id");
    let name: String = row.get("name");
    let url: String = row.get("url");
    let default_branch: Option<String> = row.get("default_branch");
    let selected_branch: Option<String> = row.get("selected_branch");
    let selected_commit: Option<String> = row.get("selected_commit");
    let ssh_key_id: Option<Uuid> = row.get("ssh_key_id");
    let status: String = row.get("status");
    let last_scan_anchor: Option<String> = row.get("last_external_scan_commit_sha");

    if status == "archived" {
        return Err(ExternalCommitError::Conflict(
            "repository is archived".to_string(),
        ));
    }

    // 分支链与 verify 同款：Confirmed 分支 → default → main → master。
    let branch = selected_branch
        .or(default_branch)
        .unwrap_or_else(|| "main".to_string());
    if !is_safe_ref(&branch) {
        return Err(ExternalCommitError::Validation(
            "unsafe branch name on repository".to_string(),
        ));
    }

    let is_ssh = url.starts_with("ssh://") || url.starts_with("git@");
    let key_material = if is_ssh {
        let ciphertext = match ssh_key_id {
            Some(key_id) => key_ciphertext_by_id(pool, project_id, key_id).await?,
            None => fallback_key_ciphertext(pool, project_id).await?,
        };
        let Some(bytes) = ciphertext else {
            return Err(ExternalCommitError::Validation(
                "no active ssh key bound to this repository".to_string(),
            ));
        };
        let private = open_ssh_private_key(project_id, &bytes)?;
        Some(git_access::write_temp_key(&private)?)
    } else {
        None
    };
    let key_path = key_material.as_ref().map(|m| m.key_path());

    let dest = repo_facts::repo_cache_dir(repository_id);
    if dest.is_dir() {
        git_access::fetch_bare(&dest, key_path.as_deref(), FETCH_TIMEOUT)
            .await
            .map_err(git_err)?;
    } else {
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        git_access::clone_bare(&url, key_path.as_deref(), &dest, CLONE_TIMEOUT)
            .await
            .map_err(git_err)?;
    }

    Ok((
        RepoAccess {
            name,
            branch,
            selected_commit,
            last_scan_anchor,
        },
        dest,
    ))
}

// ── 1. 发现（Discovery） ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RepositoryScanOutcome {
    pub repository_id: Uuid,
    pub repository_name: String,
    pub branch: String,
    pub remote_head: Option<String>,
    /// 锚点（本次扫描起点；首次扫描=远端 HEAD，只建锚不发现）。
    pub anchor_used: Option<String>,
    pub discovered: usize,
    pub truncated: bool,
    pub note: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoverOutcome {
    pub scanned_repositories: usize,
    pub discovered_commits: usize,
    pub repositories: Vec<RepositoryScanOutcome>,
}

/// 扫描项目仓库的外部新 Commit。`repository_id=None` 扫全部 active 仓库。
///
/// 锚点链：`last_external_scan_commit_sha` → Confirmed `selected_commit` →
/// 首扫只建锚。防重复：UNIQUE(repository_id, commit_sha) + ON CONFLICT DO NOTHING。
pub async fn discover_external_commits(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    repository_id: Option<Uuid>,
) -> Result<DiscoverOutcome, ExternalCommitError> {
    crate::repository::support::project_code(pool, project_id)
        .await
        .map_err(|_| ExternalCommitError::NotFound)?;

    let repos: Vec<(Uuid, String)> = match repository_id {
        Some(rid) => {
            let exists: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM integration.project_repository \
                 WHERE id = $1 AND project_id = $2 AND status <> 'archived'",
            )
            .bind(rid)
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
            let (id,) = exists.ok_or(ExternalCommitError::NotFound)?;
            vec![(id, String::new())]
        }
        None => {
            sqlx::query_as(
                "SELECT id, name FROM integration.project_repository \
                 WHERE project_id = $1 AND status <> 'archived' \
                 ORDER BY created_at ASC",
            )
            .bind(project_id)
            .fetch_all(pool)
            .await?
        }
    };

    let mut outcomes = Vec::new();
    let mut total_discovered = 0usize;
    for (rid, name) in repos {
        let mut outcome = match scan_repository(pool, project_id, rid, actor).await {
            Ok(outcome) => outcome,
            Err(err) => RepositoryScanOutcome {
                repository_id: rid,
                repository_name: name.clone(),
                branch: String::new(),
                remote_head: None,
                anchor_used: None,
                discovered: 0,
                truncated: false,
                note: None,
                error: Some(err.to_string()),
            },
        };
        if outcome.repository_name.is_empty() {
            outcome.repository_name = name;
        }
        total_discovered += outcome.discovered;
        outcomes.push(outcome);
    }

    Ok(DiscoverOutcome {
        scanned_repositories: outcomes.len(),
        discovered_commits: total_discovered,
        repositories: outcomes,
    })
}

/// 单仓库扫描：ls-remote 取 HEAD → 与锚点比对 → fetch/clone → 区间 commits +
/// 变更文件 → 落行 + 推进锚点（同事务）。
async fn scan_repository(
    pool: &PgPool,
    project_id: Uuid,
    repository_id: Uuid,
    actor: &AuditActor,
) -> Result<RepositoryScanOutcome, ExternalCommitError> {
    let (access, dest) = ensure_repo_cache(pool, repository_id).await?;

    // 远端 HEAD（bare 缓存里 rev-parse 即可——fetch 已刷新）。
    let head = String::from_utf8_lossy(
        &git_access::git_stdout(&dest, &["rev-parse", &access.branch], GIT_READ_TIMEOUT)
            .await
            .map_err(git_err)?,
    )
    .trim()
    .to_string();
    if head.is_empty() {
        return Err(ExternalCommitError::Git(
            "git_failed: empty rev-parse output".to_string(),
        ));
    }

    let mut note: Option<String> = None;
    let anchor = match access
        .last_scan_anchor
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        Some(anchor) => Some(anchor.to_string()),
        None => access
            .selected_commit
            .clone()
            .filter(|s| !s.trim().is_empty()),
    };
    let Some(anchor) = anchor else {
        // 首扫只建锚：避免把既有历史全量灌进采纳队列。
        set_scan_anchor(pool, repository_id, &head).await?;
        return Ok(RepositoryScanOutcome {
            repository_id,
            repository_name: access.name.clone(),
            branch: access.branch.clone(),
            remote_head: Some(head.clone()),
            anchor_used: None,
            discovered: 0,
            truncated: false,
            note: Some("首次扫描建立锚点，历史提交不进入采纳队列".to_string()),
            error: None,
        });
    };

    if anchor == head {
        return Ok(RepositoryScanOutcome {
            repository_id,
            repository_name: access.name.clone(),
            branch: access.branch.clone(),
            remote_head: Some(head.clone()),
            anchor_used: Some(anchor),
            discovered: 0,
            truncated: false,
            note: None,
            error: None,
        });
    }

    // 锚点必须仍是 HEAD 祖先（force push 等历史重写 → 重新建锚，不猜区间）。
    let ancestor = git_access::git_run(
        &dest,
        &["merge-base", "--is-ancestor", &anchor, &head],
        GIT_READ_TIMEOUT,
    )
    .await
    .map_err(git_err)?;
    if !ancestor.ok() {
        set_scan_anchor(pool, repository_id, &head).await?;
        return Ok(RepositoryScanOutcome {
            repository_id,
            repository_name: access.name.clone(),
            branch: access.branch.clone(),
            remote_head: Some(head.clone()),
            anchor_used: Some(anchor),
            discovered: 0,
            truncated: false,
            note: Some("锚点已不在远端历史中（force push 等），已重建锚点".to_string()),
            error: None,
        });
    }

    // 区间 commits（锚..HEAD，新→旧）。
    let cap = (MAX_SCAN_COMMITS + 1).to_string();
    let raw_log = git_access::git_stdout(
        &dest,
        &[
            "log",
            &head,
            &format!("^{anchor}"),
            "--format=%H%x1f%an%x1f%aI%x1f%s",
            "-n",
            cap.as_str(),
        ],
        GIT_READ_TIMEOUT,
    )
    .await
    .map_err(git_err)?;
    let mut commits: Vec<repo_facts::CommitFact> = Vec::new();
    for line in String::from_utf8_lossy(&raw_log).lines() {
        let mut parts = line.split('\x1f');
        let (Some(sha), Some(author), Some(authored_at), Some(subject)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        commits.push(repo_facts::CommitFact {
            sha: sha.to_string(),
            author: author.to_string(),
            authored_at: authored_at.to_string(),
            subject: subject.chars().take(200).collect(),
        });
    }
    let truncated = commits.len() > MAX_SCAN_COMMITS;
    if truncated {
        commits.truncate(MAX_SCAN_COMMITS);
        note = Some(format!("区间提交过多，仅跟踪最近 {MAX_SCAN_COMMITS} 条"));
    }

    let mut discovered = 0usize;
    let mut tx = pool.begin().await?;
    for commit in &commits {
        let changed = changed_files_of(&dest, &commit.sha).await?;
        let committed_at = DateTime::parse_from_rfc3339(&commit.authored_at)
            .map(|t| t.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let inserted = sqlx::query(
            "INSERT INTO integration.external_commit \
             (tenant_id, project_id, repository_id, commit_sha, branch, author_name, \
              committed_at, subject, changed_files, discovered_by) \
             VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, \
                     $3, $4, $5, $6, $7, $8, $9) \
             ON CONFLICT (repository_id, commit_sha) DO NOTHING",
        )
        .bind(project_id)
        .bind(repository_id)
        .bind(&commit.sha)
        .bind(&access.branch)
        .bind(&commit.author)
        .bind(committed_at)
        .bind(&commit.subject)
        .bind(serde_json::to_value(&changed).unwrap_or_else(|_| serde_json::json!([])))
        .bind(actor.principal_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        discovered += inserted as usize;
    }
    sqlx::query(
        "UPDATE integration.project_repository \
         SET last_external_scan_commit_sha = $2, last_external_scan_at = now(), \
             revision = revision + 1, updated_at = now() \
         WHERE id = $1",
    )
    .bind(repository_id)
    .bind(&head)
    .execute(&mut *tx)
    .await?;
    crate::repository::support::append_audit(
        &mut tx,
        project_id,
        actor,
        "external_commit.scan",
        "project_repository",
        Some(repository_id),
    )
    .await?;
    tx.commit().await?;

    Ok(RepositoryScanOutcome {
        repository_id,
        repository_name: access.name.clone(),
        branch: access.branch.clone(),
        remote_head: Some(head),
        anchor_used: Some(anchor),
        discovered,
        truncated,
        note,
        error: None,
    })
}

async fn set_scan_anchor(
    pool: &PgPool,
    repository_id: Uuid,
    head: &str,
) -> Result<(), ExternalCommitError> {
    sqlx::query(
        "UPDATE integration.project_repository \
         SET last_external_scan_commit_sha = $2, last_external_scan_at = now(), \
             revision = revision + 1, updated_at = now() \
         WHERE id = $1",
    )
    .bind(repository_id)
    .bind(head)
    .execute(pool)
    .await?;
    Ok(())
}

/// 单提交变更文件（diff-tree --root -r --name-status；merge/root 均可读）。
async fn changed_files_of(dest: &Path, sha: &str) -> Result<Vec<ChangedFile>, ExternalCommitError> {
    let raw = git_access::git_stdout(
        dest,
        &[
            "diff-tree",
            "--no-commit-id",
            "--name-status",
            "-r",
            "--root",
            sha,
        ],
        GIT_READ_TIMEOUT,
    )
    .await
    .map_err(git_err)?;
    let mut files = Vec::new();
    for line in String::from_utf8_lossy(&raw).lines() {
        let mut parts = line.splitn(2, '\t');
        let (Some(change), Some(path)) = (parts.next(), parts.next()) else {
            continue;
        };
        let change = change.chars().next().map(String::from).unwrap_or_default();
        files.push(ChangedFile {
            path: path.to_string(),
            change,
        });
        if files.len() >= MAX_CHANGED_FILES {
            break;
        }
    }
    Ok(files)
}

// ── 2. AI 分析 ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AnalyzeOutcome {
    pub id: Uuid,
    pub status: String,
    pub analysis_mode: Option<String>,
    pub clarification_state: Option<String>,
    pub error: Option<String>,
}

/// 分析一条外部 Commit。`discovered` / `failed`（重试）态可用；其余 409。
///
/// LLM 不可用（`Ok(None)`）→ 结构化派生（[`derived_analysis`]）；
/// LLM 调用/解析失败 → 行入 `failed`（attempts/last_error 可观测，可重试）。
/// `root` = 项目知识库根：当前文档正文从 Git 工作树读（Markdown+Git 权威，
/// 数据库只存修订账目——数据库不承载 Markdown 正文）。
pub async fn analyze_external_commit(
    pool: &PgPool,
    root: &crate::knowledge::KnowledgeRoot,
    model: &dyn CommitAnalysisModel,
    commit_id: Uuid,
    actor: &AuditActor,
) -> Result<AnalyzeOutcome, ExternalCommitError> {
    let row = sqlx::query(
        "SELECT id, project_id, repository_id, commit_sha, author_name, committed_at::text, \
                subject, changed_files, status \
         FROM integration.external_commit WHERE id = $1",
    )
    .bind(commit_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ExternalCommitError::NotFound)?;
    let project_id: Uuid = row.get("project_id");
    let repository_id: Uuid = row.get("repository_id");
    let sha: String = row.get("commit_sha");
    let author: String = row.get("author_name");
    let committed_at: String = row.get("committed_at");
    let subject: String = row.get("subject");
    let changed_json: serde_json::Value = row.get("changed_files");
    let status: String = row.get("status");
    if status != "discovered" && status != "failed" {
        return Err(ExternalCommitError::Conflict(format!(
            "external commit is {status}, only discovered/failed can be analyzed"
        )));
    }

    let changed_files: Vec<ChangedFile> = serde_json::from_value(changed_json).unwrap_or_default();
    let (access, dest) = ensure_repo_cache(pool, repository_id).await?;

    // diff 摘录（有界）。
    let raw_diff = git_access::git_stdout(
        &dest,
        &["show", "--format=", "--patch", "--unified=2", &sha],
        GIT_READ_TIMEOUT,
    )
    .await;
    let (diff_excerpt, diff_truncated) = match raw_diff {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes).to_string();
            let truncated = text.chars().count() > MAX_DIFF_EXCERPT_CHARS;
            (
                text.chars()
                    .take(MAX_DIFF_EXCERPT_CHARS)
                    .collect::<String>(),
                truncated,
            )
        }
        Err(_) => (String::new(), false),
    };

    let candidates = load_candidates(pool, project_id, &subject, &changed_files).await;
    let known_docs = load_known_docs(pool, root, project_id).await;

    let input = CommitAnalysisInput {
        project_id,
        repository_name: &access.name,
        commit_sha: &sha,
        author: &author,
        committed_at: &committed_at,
        subject: &subject,
        branch: &access.branch,
        changed_files: &changed_files,
        diff_excerpt: &diff_excerpt,
        diff_truncated,
        candidates: &candidates,
        known_docs: &known_docs,
    };

    let attempts: i32 = sqlx::query_scalar(
        "UPDATE integration.external_commit SET attempts = attempts + 1, updated_at = now() \
         WHERE id = $1 RETURNING attempts",
    )
    .bind(commit_id)
    .fetch_one(pool)
    .await?;

    match model.analyze_commit(&input).await {
        Ok(Some(mut output)) => {
            sanitize_analysis(&mut output, &candidates, &known_docs);
            finish_analysis(
                pool,
                commit_id,
                actor,
                output.analysis,
                output.clarification_questions,
                attempts,
            )
            .await
        }
        Ok(None) => {
            let analysis = derived_analysis(&subject, &author, &sha, &changed_files, &candidates);
            finish_analysis(pool, commit_id, actor, analysis, Vec::new(), attempts).await
        }
        Err(err) => {
            let detail = err.to_string().chars().take(500).collect::<String>();
            sqlx::query(
                "UPDATE integration.external_commit \
                 SET status = 'failed', last_error = $2, updated_at = now() WHERE id = $1",
            )
            .bind(commit_id)
            .bind(&detail)
            .execute(pool)
            .await?;
            Ok(AnalyzeOutcome {
                id: commit_id,
                status: "failed".to_string(),
                analysis_mode: None,
                clarification_state: None,
                error: Some(detail),
            })
        }
    }
}

/// 分析成功收尾：analysis 落 jsonb + 澄清问题落行 + 状态推进
/// （有 pending 问题 → analyzed；零问题 → awaiting_adoption + not_needed 显式记录）。
async fn finish_analysis(
    pool: &PgPool,
    commit_id: Uuid,
    actor: &AuditActor,
    analysis: CommitAnalysis,
    questions: Vec<String>,
    attempts: i32,
) -> Result<AnalyzeOutcome, ExternalCommitError> {
    let mut questions: Vec<String> = questions
        .into_iter()
        .map(|q| q.trim().chars().take(500).collect::<String>())
        .filter(|q| !q.is_empty())
        .take(MAX_CLARIFICATION_QUESTIONS)
        .collect();
    questions.sort();
    questions.dedup();
    let (clarification_state, status) = if questions.is_empty() {
        ("not_needed", "awaiting_adoption")
    } else {
        ("pending", "analyzed")
    };
    let project_id: Uuid =
        sqlx::query_scalar("SELECT project_id FROM integration.external_commit WHERE id = $1")
            .bind(commit_id)
            .fetch_one(pool)
            .await?;

    let mut tx = pool.begin().await?;
    let analysis_json = serde_json::to_value(&analysis)
        .map_err(|e| ExternalCommitError::Validation(format!("analysis serialize: {e}")))?;
    sqlx::query(
        "UPDATE integration.external_commit \
         SET status = $2, analysis = $3, analysis_mode = $4, analyzed_at = now(), \
             clarification_state = $5, last_error = NULL, attempts = $6, updated_at = now() \
         WHERE id = $1 AND status IN ('discovered', 'failed')",
    )
    .bind(commit_id)
    .bind(status)
    .bind(&analysis_json)
    .bind(&analysis.analysis_mode)
    .bind(clarification_state)
    .bind(attempts)
    .execute(&mut *tx)
    .await?;

    if !questions.is_empty() {
        for question in &questions {
            sqlx::query(
                "INSERT INTO integration.external_commit_clarification \
                 (tenant_id, external_commit_id, project_id, question) \
                 VALUES ((SELECT tenant_id FROM core.project WHERE id = $2), $1, $2, $3)",
            )
            .bind(commit_id)
            .bind(project_id)
            .bind(question)
            .execute(&mut *tx)
            .await?;
        }
    }
    crate::repository::support::append_audit(
        &mut tx,
        project_id,
        actor,
        "external_commit.analyzed",
        "external_commit",
        Some(commit_id),
    )
    .await?;
    tx.commit().await?;

    Ok(AnalyzeOutcome {
        id: commit_id,
        status: status.to_string(),
        analysis_mode: Some(analysis.analysis_mode),
        clarification_state: Some(clarification_state.to_string()),
        error: None,
    })
}

/// 分析产物中的澄清问题在 finish_analysis 入口截断去重；本函数仅为文档锚点。
const _: () = ();

/// 输出约束：related id ⊆ 结构化候选、doc_key ∈ 已有文档白名单（编造直接丢弃，
/// 与 snapshot 证据白名单同纪律）；长度有界；澄清问题上限截断。
fn sanitize_analysis(
    output: &mut CommitAnalysisOutput,
    candidates: &[RelatedCandidate],
    known_docs: &[KnownDoc],
) {
    let analysis = &mut output.analysis;
    // related：id 必须命中候选；domain/title 一律以候选回填（LLM 只产 id/relation）。
    analysis.related_objects = analysis
        .related_objects
        .iter()
        .filter_map(|r| {
            candidates
                .iter()
                .find(|c| c.id == r.id)
                .map(|c| RelatedObject {
                    domain: c.domain.clone(),
                    id: r.id.clone(),
                    title: c.title.clone(),
                    relation: r.relation.clone(),
                })
        })
        .take(MAX_CANDIDATES_PER_KIND * 3)
        .collect();

    // knowledge_updates：doc_key 必须命中已有文档；doc_type 以白名单回填。
    analysis.knowledge_updates.retain_mut(|update| {
        match known_docs.iter().find(|d| d.doc_key == update.doc_key) {
            Some(doc) => {
                update.doc_type = doc.doc_type.clone();
                true
            }
            None => false,
        }
    });
    if analysis.knowledge_updates.len() > MAX_KNOWLEDGE_UPDATES {
        analysis.knowledge_updates.truncate(MAX_KNOWLEDGE_UPDATES);
    }

    analysis.summary = analysis.summary.chars().take(2000).collect();
    if let Some(deviation) = &mut analysis.deviation {
        deviation.note = deviation.note.chars().take(2000).collect();
    }
    for update in &mut analysis.knowledge_updates {
        update.summary = update.summary.chars().take(1000).collect();
        update.proposed_markdown = update
            .proposed_markdown
            .chars()
            .take(MAX_PROPOSED_MARKDOWN_CHARS)
            .collect();
    }
    analysis.analysis_mode = "llm".to_string();

    output.clarification_questions = output
        .clarification_questions
        .iter()
        .map(|q| q.trim().chars().take(500).collect::<String>())
        .filter(|q| !q.is_empty())
        .take(MAX_CLARIFICATION_QUESTIONS)
        .collect();
}

/// 结构化派生（无 LLM 时的合法降级）：摘要与关联对象来自可追溯事实；
/// 不产出知识更新建议（无法拟稿 → 不建议）与偏离判断（不可判定 → None）。
fn derived_analysis(
    subject: &str,
    author: &str,
    sha: &str,
    changed_files: &[ChangedFile],
    candidates: &[RelatedCandidate],
) -> CommitAnalysis {
    let docs = changed_files
        .iter()
        .filter(|f| f.path.to_ascii_lowercase().ends_with(".md"))
        .count();
    let summary = format!(
        "{}（作者 {}；提交 {}；变更 {} 个文件，其中 Markdown 文档 {} 个）",
        subject,
        author,
        sha.chars().take(8).collect::<String>(),
        changed_files.len(),
        docs
    );
    let related = candidates
        .iter()
        .map(|c| RelatedObject {
            domain: c.domain.clone(),
            id: c.id.clone(),
            title: c.title.clone(),
            relation: format!("{}（词面匹配）", c.reason),
        })
        .collect();
    CommitAnalysis {
        summary,
        analysis_mode: "derived".to_string(),
        related_objects: related,
        deviation: None,
        knowledge_updates: Vec::new(),
    }
}

/// 词面候选：subject 词元 + 变更路径段 对 requirement/task/doc_key 的匹配。
/// 纯结构化（无 LLM），命中原因可追溯。
async fn load_candidates(
    pool: &PgPool,
    project_id: Uuid,
    subject: &str,
    changed_files: &[ChangedFile],
) -> Vec<RelatedCandidate> {
    let tokens = match_tokens(subject, changed_files);
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();

    let requirements: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id::text, code, title FROM delivery.requirement \
         WHERE project_id = $1 AND lifecycle_status = 'active' LIMIT 200",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let mut req_hits = 0;
    for (id, code, title) in requirements {
        let hay = format!("{code} {title}").to_ascii_lowercase();
        if tokens.iter().any(|t| hay.contains(t.as_str())) && req_hits < MAX_CANDIDATES_PER_KIND {
            out.push(RelatedCandidate {
                domain: "requirement".to_string(),
                id,
                title,
                reason: "subject_match".to_string(),
            });
            req_hits += 1;
        }
    }

    let tasks: Vec<(String, String)> = sqlx::query_as(
        "SELECT id::text, title FROM delivery.task \
         WHERE project_id = $1 AND status NOT IN ('cancelled', 'superseded') LIMIT 200",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let mut task_hits = 0;
    for (id, title) in tasks {
        let hay = title.to_ascii_lowercase();
        if tokens.iter().any(|t| hay.contains(t.as_str())) && task_hits < MAX_CANDIDATES_PER_KIND {
            out.push(RelatedCandidate {
                domain: "task".to_string(),
                id,
                title,
                reason: "subject_match".to_string(),
            });
            task_hits += 1;
        }
    }

    // 文档路径段 → doc_key 匹配（path_match）。
    let docs = crate::knowledge::list_document_identities(pool, project_id, 100)
        .await
        .unwrap_or_default();
    let path_stems: HashSet<String> = changed_files
        .iter()
        .flat_map(|f| {
            f.path
                .split('/')
                .filter_map(|seg| seg.rsplit_once('.').map(|(stem, _)| stem.to_string()))
                .collect::<Vec<_>>()
        })
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let mut doc_hits = 0;
    for (doc_key, _doc_type) in docs {
        let hay = doc_key.to_ascii_lowercase();
        let hit = tokens.iter().any(|t| hay.contains(t.as_str()))
            || path_stems.iter().any(|stem| hay.contains(stem.as_str()));
        if hit && doc_hits < MAX_CANDIDATES_PER_KIND {
            out.push(RelatedCandidate {
                domain: "knowledge".to_string(),
                id: doc_key.clone(),
                title: doc_key,
                reason: "path_match".to_string(),
            });
            doc_hits += 1;
        }
    }
    out
}

fn match_tokens(subject: &str, changed_files: &[ChangedFile]) -> Vec<String> {
    let mut tokens: Vec<String> = subject
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.chars().count() >= 2)
        .map(|t| t.to_ascii_lowercase())
        .collect();
    for file in changed_files.iter().take(20) {
        for seg in file.path.split('/') {
            let stem = seg.rsplit_once('.').map(|(s, _)| s).unwrap_or(seg);
            if stem.chars().count() >= 4 {
                tokens.push(stem.to_ascii_lowercase());
            }
        }
    }
    tokens.sort();
    tokens.dedup();
    tokens.truncate(20);
    tokens
}

/// 已有知识文档白名单 + 当前正文（拟稿上下文）。
///
/// 正文从知识库 Git 工作树按 canonical_path 读取（Markdown+Git 权威）；
/// 修订账目（哪条是 published）来自数据库 LATERAL 投影。Git 读不到
/// （未初始化/文件缺失）→ current_markdown=None，白名单身份不受影响。
async fn load_known_docs(
    pool: &PgPool,
    root: &crate::knowledge::KnowledgeRoot,
    project_id: Uuid,
) -> Vec<KnownDoc> {
    let documents = crate::knowledge::list_published_document_context(
        pool,
        root,
        project_id,
        20,
        MAX_DOC_CONTEXT_CHARS,
    )
    .await
    .unwrap_or_else(|err| {
        tracing::warn!(%project_id, error = %err, "load_known_docs query failed");
        Vec::new()
    });

    documents
        .into_iter()
        .map(|document| KnownDoc {
            doc_key: document.doc_key,
            doc_type: document.doc_type,
            current_markdown: document.markdown,
            current_revision_id: document.revision_id,
        })
        .collect()
}

// ── 3. 澄清（Clarification） ──────────────────────────────────────────────

/// 人类答复一条澄清问题；全部答复 → clarification_state=resolved 且状态推进
/// awaiting_adoption。「无需澄清」不在答复路径——分析期零问题即显式落
/// not_needed（见 finish_analysis），不存在跳过记录的路径。
pub async fn answer_clarification(
    pool: &PgPool,
    commit_id: Uuid,
    clarification_id: Uuid,
    actor: &AuditActor,
    answer: &str,
) -> Result<ClarificationRow, ExternalCommitError> {
    let answer = answer.trim();
    if answer.is_empty() {
        return Err(ExternalCommitError::Validation(
            "clarification answer must not be empty".to_string(),
        ));
    }
    crate::requirement::require_human(pool, actor).await?;

    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT c.id, e.status \
         FROM integration.external_commit_clarification c \
         JOIN integration.external_commit e ON e.id = c.external_commit_id \
         WHERE c.id = $1 AND c.external_commit_id = $2",
    )
    .bind(clarification_id)
    .bind(commit_id)
    .fetch_optional(pool)
    .await?;
    let (_, commit_status) = row.ok_or(ExternalCommitError::NotFound)?;
    if commit_status != "analyzed" {
        return Err(ExternalCommitError::Conflict(format!(
            "external commit is {commit_status}; clarifications answerable only while analyzed"
        )));
    }

    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE integration.external_commit_clarification \
         SET answer = $3, answered_by = $4, answered_at = now(), status = 'answered', \
             updated_at = now() \
         WHERE id = $1 AND external_commit_id = $2 AND status = 'open'",
    )
    .bind(clarification_id)
    .bind(commit_id)
    .bind(answer)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        tx.rollback().await?;
        return Err(ExternalCommitError::Conflict(
            "clarification is not open".to_string(),
        ));
    }
    let open_left: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.external_commit_clarification \
         WHERE external_commit_id = $1 AND status = 'open'",
    )
    .bind(commit_id)
    .fetch_one(&mut *tx)
    .await?;
    if open_left == 0 {
        sqlx::query(
            "UPDATE integration.external_commit \
             SET clarification_state = 'resolved', status = 'awaiting_adoption', updated_at = now() \
             WHERE id = $1 AND status = 'analyzed'",
        )
        .bind(commit_id)
        .execute(&mut *tx)
        .await?;
    }
    crate::repository::support::append_audit(
        &mut tx,
        project_id_of(pool, commit_id).await?,
        actor,
        "external_commit.clarification_answered",
        "external_commit",
        Some(commit_id),
    )
    .await?;
    tx.commit().await?;

    let row: ClarificationRow = sqlx::query_as(
        "SELECT id, external_commit_id, question, answer, answered_by, answered_at, status, \
                created_at \
         FROM integration.external_commit_clarification WHERE id = $1",
    )
    .bind(clarification_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

async fn project_id_of(pool: &PgPool, commit_id: Uuid) -> Result<Uuid, ExternalCommitError> {
    sqlx::query_scalar("SELECT project_id FROM integration.external_commit WHERE id = $1")
        .bind(commit_id)
        .fetch_optional(pool)
        .await?
        .ok_or(ExternalCommitError::NotFound)
}

// ── 4. 采纳（Adoption）与 5. 关闭（Closure） ──────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct AdoptionDecisionRequest {
    /// approved | no_update | rejected
    pub decision: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdoptionOutcome {
    pub id: Uuid,
    pub status: String,
    pub adoption_decision: String,
    pub knowledge_proposals: serde_json::Value,
}

/// 人类采纳决定。`approved` 把分析期拟稿（人已可见）交既有提案管线
/// （质量门 + 红线人类确认）；`no_update` / `rejected` 显式落终态。
pub async fn decide_adoption(
    pool: &PgPool,
    root: &crate::knowledge::KnowledgeRoot,
    commit_id: Uuid,
    actor: &AuditActor,
    req: AdoptionDecisionRequest,
) -> Result<AdoptionOutcome, ExternalCommitError> {
    crate::requirement::require_human(pool, actor).await?;

    let row = sqlx::query(
        "SELECT project_id, status, commit_sha, analysis \
         FROM integration.external_commit WHERE id = $1",
    )
    .bind(commit_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ExternalCommitError::NotFound)?;
    let project_id: Uuid = row.get("project_id");
    let status: String = row.get("status");
    let sha: String = row.get("commit_sha");
    let analysis_json: serde_json::Value = row.try_get("analysis").unwrap_or_default();
    if status != "awaiting_adoption" {
        return Err(ExternalCommitError::Conflict(format!(
            "external commit is {status}; adoption requires awaiting_adoption"
        )));
    }
    let analysis: CommitAnalysis = serde_json::from_value(analysis_json)
        .map_err(|e| ExternalCommitError::Validation(format!("stored analysis unreadable: {e}")))?;

    let decision = req.decision.as_str();
    let note = req
        .note
        .as_deref()
        .map(|n| n.chars().take(2000).collect::<String>());

    match decision {
        "approved" => {
            let mut proposals: Vec<serde_json::Value> = Vec::new();
            for (index, update) in analysis.knowledge_updates.iter().enumerate() {
                // 幂等键锚 commit + doc_key：重复决定/重试重放同一提案。
                let operation_id = format!("ec_adopt:{commit_id}:{index}:{}", update.doc_key);
                let current_revision =
                    current_published_revision(pool, project_id, &update.doc_key).await;
                // 质量门 #6（reference）：拟稿必须带来源追溯；缺省时按管线
                // 既有 source_events 尾注惯例补一行（knowledge_migrate 同款）。
                let mut markdown = update.proposed_markdown.clone();
                if !markdown.contains("source_events:") && !markdown.contains("projects/") {
                    markdown.push_str(&format!(
                        "\n\n---\nsource_events:\n  - external_commit:{commit_id}\n"
                    ));
                }
                let request = crate::knowledge_proposal::ProposalRequest {
                    project_id,
                    doc_key: update.doc_key.clone(),
                    doc_type: update.doc_type.clone(),
                    markdown,
                    change_summary: Some(format!(
                        "外部提交 {} 采纳同步：{}",
                        short_sha(&sha),
                        update.summary
                    )),
                    source_event_id: None,
                    analysis_run_id: None,
                    source_ref: Some(format!("external_commit:{commit_id}")),
                    redline_kind: None,
                    base_revision_id: current_revision,
                    operation_id,
                    created_by: actor.principal_id,
                };
                match crate::knowledge_proposal::submit_proposal(pool, root, request).await {
                    Ok(outcome) => proposals.push(proposal_record(&update.doc_key, &outcome)),
                    Err(err) => proposals.push(serde_json::json!({
                        "doc_key": update.doc_key,
                        "outcome": "pipeline_failed",
                        "error": err.to_string(),
                    })),
                }
            }
            let proposals_json = serde_json::Value::Array(proposals);
            sqlx::query(
                "UPDATE integration.external_commit \
                 SET status = 'adopted', adoption_decision = 'approved', adoption_note = $2, \
                     adopted_by = $3, adopted_at = now(), knowledge_proposals = $4, updated_at = now() \
                 WHERE id = $1 AND status = 'awaiting_adoption'",
            )
            .bind(commit_id)
            .bind(&note)
            .bind(actor.principal_id)
            .bind(&proposals_json)
            .execute(pool)
            .await?;
            audit(
                pool,
                project_id,
                actor,
                "external_commit.adoption_approved",
                commit_id,
            )
            .await?;
            Ok(AdoptionOutcome {
                id: commit_id,
                status: "adopted".to_string(),
                adoption_decision: "approved".to_string(),
                knowledge_proposals: proposals_json,
            })
        }
        "no_update" | "rejected" => {
            let decision_value = if decision == "no_update" {
                "no_update"
            } else {
                "rejected"
            };
            sqlx::query(
                "UPDATE integration.external_commit \
                 SET status = 'closed', adoption_decision = $2, adoption_note = $3, \
                     adopted_by = $4, adopted_at = now(), updated_at = now() \
                 WHERE id = $1 AND status = 'awaiting_adoption'",
            )
            .bind(commit_id)
            .bind(decision_value)
            .bind(&note)
            .bind(actor.principal_id)
            .execute(pool)
            .await?;
            audit(
                pool,
                project_id,
                actor,
                if decision_value == "no_update" {
                    "external_commit.adoption_no_update"
                } else {
                    "external_commit.adoption_rejected"
                },
                commit_id,
            )
            .await?;
            Ok(AdoptionOutcome {
                id: commit_id,
                status: "closed".to_string(),
                adoption_decision: decision_value.to_string(),
                knowledge_proposals: serde_json::json!([]),
            })
        }
        other => Err(ExternalCommitError::Validation(format!(
            "unknown adoption decision `{other}` (expected approved|no_update|rejected)"
        ))),
    }
}

/// 关闭已采纳的提交：全部知识提案必须已到终态（committed/rejected/rolled_back
/// /duplicate），pending 中 → 409 列出；终态 → 回填修订引用并置 closed。
pub async fn close_adoption(
    pool: &PgPool,
    commit_id: Uuid,
    actor: &AuditActor,
) -> Result<AdoptionOutcome, ExternalCommitError> {
    let row = sqlx::query(
        "SELECT project_id, status, knowledge_proposals \
         FROM integration.external_commit WHERE id = $1",
    )
    .bind(commit_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ExternalCommitError::NotFound)?;
    let project_id: Uuid = row.get("project_id");
    let status: String = row.get("status");
    let proposals_json: serde_json::Value = row.get("knowledge_proposals");
    if status != "adopted" {
        return Err(ExternalCommitError::Conflict(format!(
            "external commit is {status}; close requires adopted"
        )));
    }

    let mut revision_ids: Vec<Uuid> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    if let Some(items) = proposals_json.as_array() {
        for item in items {
            let proposal_id = item.get("proposal_id").and_then(|v| v.as_str());
            let outcome = item.get("outcome").and_then(|v| v.as_str()).unwrap_or("");
            let Some(proposal_id) = proposal_id else {
                continue; // duplicate / pipeline_failed 无提案行可查
            };
            if outcome == "pipeline_failed" {
                continue;
            }
            let Ok(pid) = Uuid::parse_str(proposal_id) else {
                continue;
            };
            let row: Option<(String, Option<Uuid>)> = sqlx::query_as(
                "SELECT status, resulting_revision_id FROM ai.document_proposal \
                 WHERE id = $1 AND project_id = $2",
            )
            .bind(pid)
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
            let Some((proposal_status, revision_id)) = row else {
                pending.push(format!("{proposal_id}:missing"));
                continue;
            };
            match proposal_status.as_str() {
                "committed" => {
                    if let Some(revision_id) = revision_id {
                        revision_ids.push(revision_id);
                    } else {
                        pending.push(format!("{proposal_id}:committed_without_revision"));
                    }
                }
                "rejected" | "rolled_back" => {}
                other => pending.push(format!("{proposal_id}:{other}")),
            }
        }
    }
    if !pending.is_empty() {
        return Err(ExternalCommitError::Conflict(format!(
            "knowledge proposals still pending: {}",
            pending.join(", ")
        )));
    }

    let revision_json = serde_json::to_value(&revision_ids)
        .map_err(|e| ExternalCommitError::Validation(format!("revisions serialize: {e}")))?;
    sqlx::query(
        "UPDATE integration.external_commit \
         SET status = 'closed', adopted_revision_ids = $2, updated_at = now() \
         WHERE id = $1 AND status = 'adopted'",
    )
    .bind(commit_id)
    .bind(&revision_json)
    .execute(pool)
    .await?;
    audit(pool, project_id, actor, "external_commit.closed", commit_id).await?;
    Ok(AdoptionOutcome {
        id: commit_id,
        status: "closed".to_string(),
        adoption_decision: "approved".to_string(),
        knowledge_proposals: proposals_json,
    })
}

async fn current_published_revision(
    pool: &PgPool,
    project_id: Uuid,
    doc_key: &str,
) -> Option<Uuid> {
    crate::knowledge::current_published_revision_id(pool, project_id, doc_key)
        .await
        .ok()
        .flatten()
}

fn proposal_record(
    doc_key: &str,
    outcome: &crate::knowledge_proposal::ProposalOutcome,
) -> serde_json::Value {
    use crate::knowledge_proposal::ProposalOutcome as O;
    match outcome {
        O::Committed {
            proposal_id,
            revision_id,
            ..
        } => serde_json::json!({
            "doc_key": doc_key,
            "proposal_id": proposal_id,
            "revision_id": revision_id,
            "outcome": "committed",
        }),
        O::RedlinePending { proposal_id } => serde_json::json!({
            "doc_key": doc_key,
            "proposal_id": proposal_id,
            "outcome": "redline_pending",
        }),
        O::Rejected {
            proposal_id,
            failures,
        } => serde_json::json!({
            "doc_key": doc_key,
            "proposal_id": proposal_id,
            "outcome": "rejected",
            "failures": failures,
        }),
        O::Duplicate {
            document_id,
            revision_id,
            ..
        } => serde_json::json!({
            "doc_key": doc_key,
            "document_id": document_id,
            "revision_id": revision_id,
            "outcome": "duplicate",
        }),
    }
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(8).collect()
}

async fn audit(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    action: &str,
    commit_id: Uuid,
) -> Result<(), ExternalCommitError> {
    let mut tx = pool.begin().await?;
    crate::repository::support::append_audit(
        &mut tx,
        project_id,
        actor,
        action,
        "external_commit",
        Some(commit_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

// ── 读面 ──────────────────────────────────────────────────────────────────

pub async fn list_external_commits(
    pool: &PgPool,
    project_id: Uuid,
    status: Option<&str>,
) -> Result<Vec<serde_json::Value>, ExternalCommitError> {
    let rows = sqlx::query(
        "SELECT e.*, \
                (SELECT count(*) FROM integration.external_commit_clarification c \
                  WHERE c.external_commit_id = e.id AND c.status = 'open') AS open_questions, \
                r.name AS repository_name \
         FROM integration.external_commit e \
         LEFT JOIN integration.project_repository r ON r.id = e.repository_id \
         WHERE e.project_id = $1 AND ($2::text IS NULL OR e.status = $2) \
         ORDER BY e.committed_at DESC, e.discovered_at DESC, e.id LIMIT 200",
    )
    .bind(project_id)
    .bind(status)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "id": row.get::<Uuid, _>("id"),
                "repository_id": row.get::<Uuid, _>("repository_id"),
                "repository_name": row.get::<Option<String>, _>("repository_name"),
                "commit_sha": row.get::<String, _>("commit_sha"),
                "branch": row.get::<String, _>("branch"),
                "author_name": row.get::<String, _>("author_name"),
                "committed_at": row.get::<DateTime<Utc>, _>("committed_at"),
                "subject": row.get::<String, _>("subject"),
                "status": row.get::<String, _>("status"),
                "analysis_mode": row.get::<Option<String>, _>("analysis_mode"),
                "clarification_state": row.get::<String, _>("clarification_state"),
                "open_questions": row.get::<i64, _>("open_questions"),
                "adoption_decision": row.get::<Option<String>, _>("adoption_decision"),
                "attempts": row.get::<i32, _>("attempts"),
                "last_error": row.get::<Option<String>, _>("last_error"),
                "discovered_at": row.get::<DateTime<Utc>, _>("discovered_at"),
            })
        })
        .collect())
}

/// 详情：行 + 澄清列表。
pub async fn get_external_commit(
    pool: &PgPool,
    commit_id: Uuid,
) -> Result<(ExternalCommitRow, Vec<ClarificationRow>, String), ExternalCommitError> {
    let row: ExternalCommitRow = sqlx::query_as(
        "SELECT id, project_id, repository_id, commit_sha, branch, author_name, \
                committed_at, subject, changed_files, status, analysis, analysis_mode, \
                analyzed_at, clarification_state, adoption_decision, adoption_note, \
                adopted_by, adopted_at, knowledge_proposals, adopted_revision_ids, \
                attempts, last_error, discovered_at \
         FROM integration.external_commit WHERE id = $1",
    )
    .bind(commit_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ExternalCommitError::NotFound)?;
    let repository_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM integration.project_repository WHERE id = $1")
            .bind(row.repository_id)
            .fetch_optional(pool)
            .await?
            .flatten();
    let clarifications: Vec<ClarificationRow> = sqlx::query_as(
        "SELECT id, external_commit_id, question, answer, answered_by, answered_at, status, \
                created_at \
         FROM integration.external_commit_clarification \
         WHERE external_commit_id = $1 ORDER BY created_at ASC",
    )
    .bind(commit_id)
    .fetch_all(pool)
    .await?;
    Ok((row, clarifications, repository_name.unwrap_or_default()))
}

/// 变更前后差异（git show patch，有界 64K；缓存缺失时 fetch/clone 重建）。
pub async fn read_commit_diff(
    pool: &PgPool,
    commit_id: Uuid,
) -> Result<(String, bool, String), ExternalCommitError> {
    let repository_id: Uuid =
        sqlx::query_scalar("SELECT repository_id FROM integration.external_commit WHERE id = $1")
            .bind(commit_id)
            .fetch_optional(pool)
            .await?
            .ok_or(ExternalCommitError::NotFound)?;
    let sha: String =
        sqlx::query_scalar("SELECT commit_sha FROM integration.external_commit WHERE id = $1")
            .bind(commit_id)
            .fetch_one(pool)
            .await?;
    let (_, dest) = ensure_repo_cache(pool, repository_id).await?;
    let raw = git_access::git_stdout(
        &dest,
        &["show", "--format=", "--patch", "--unified=3", &sha],
        GIT_READ_TIMEOUT,
    )
    .await
    .map_err(git_err)?;
    let text = String::from_utf8_lossy(&raw).to_string();
    let truncated = text.chars().count() > MAX_DIFF_VIEW_CHARS;
    let diff = text.chars().take(MAX_DIFF_VIEW_CHARS).collect();
    Ok((diff, truncated, sha))
}
