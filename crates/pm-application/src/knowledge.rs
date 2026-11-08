//! Knowledge space: Markdown + Git is the authoritative body; PostgreSQL only
//! carries document identity, canonical path, revision number, commit sha and
//! content hash. Design of record: `docs/ai-pm-markdown-knowledge-space.md`
//! §四 (one authority per fact) and §八 (Git/DB sync).
//!
//! V1 scope: create a document (one file + one commit + two rows), list
//! documents, read a document. No proposals, quality gates, red lines or index
//! state yet — those are follow-up milestones.

use std::path::{Component, Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

/// Valid `doc_type` values (mirrors the CHECK constraint in migration 0006).
pub const DOC_TYPES: [&str; 4] = ["requirement", "decision", "architecture", "knowledge"];

const REQUIREMENT_TEMPLATE: &str = "# 需求标题

## 1. 业务目标

## 2. 功能要求

## 3. 非功能要求

## 4. 验收条件

## 5. 待确认问题

## 6. 关联对象

## 7. 变更说明
";

const CHANGE_REQUEST_TEMPLATE: &str = "# 变更请求

## 变更内容

## 变更原因

## 影响分析

## 兼容性与回滚

## 关联需求 / 任务
";

const ADR_TEMPLATE: &str = "# ADR-NNN：决策标题

## 背景

## 需要解决的问题

## 候选方案

## 决策

## 决策依据

## 后果与风险

## 历史变更
";

const TEMPLATES: [(&str, &str); 3] = [
    ("requirement.md", REQUIREMENT_TEMPLATE),
    ("change-request.md", CHANGE_REQUEST_TEMPLATE),
    ("adr.md", ADR_TEMPLATE),
];

#[derive(Debug, thiserror::Error)]
pub enum KnowledgeError {
    #[error("invalid document key: {0}")]
    InvalidKey(String),
    #[error("invalid document type: {0}")]
    InvalidType(String),
    #[error("document already exists: {0}")]
    Exists(String),
    #[error("document not found")]
    NotFound,
    #[error("document content drifted from its recorded hash")]
    Stale,
    #[error("knowledge conflict: {0}")]
    Conflict(String),
    #[error("knowledge root misconfigured: {0}")]
    Config(String),
    #[error("knowledge root io error: {0}")]
    Io(String),
    #[error("git error: {0}")]
    Git(String),
    /// T-060：索引层与权威元数据均不可用——检索失败（**不是**空结果）。
    #[error("knowledge index unavailable: {0}")]
    IndexUnavailable(String),
    /// T-060：project 写锁排队超时（可重试，不是数据冲突）。
    #[error("knowledge project write is busy: {0}")]
    ProjectWriteBusy(String),
    /// T-060：`base_revision` 与当前 head 不一致（不产生 Revision）。
    #[error("knowledge revision conflict: {0}")]
    RevisionConflict(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// I4-F 归档守卫：**正式知识提交**（发布修订）前，在 **project 行锁**下确认
/// Project 仍然 active。
///
/// 归档事务（`archive.rs`）同样先锁 `core.project` 行 → 二者串行，结论确定：
/// - 归档先落定 → 本次发布被拒（归档后不得出现新的正式版本，I4 §9.1/§9.3）；
/// - 发布先落定 → 该修订成为归档时的最终版本（I4 §11.3）。
///
/// 锁序：`core.project` → 文档/提案行（与 I1 的 `project → run → task` 同向）。
pub(crate) async fn ensure_project_active_for_write(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
) -> Result<(), KnowledgeError> {
    match crate::project_gate::lock_project_activity_tx(tx, project_id).await {
        Ok(crate::project_gate::ProjectActivity::Active) => Ok(()),
        Ok(crate::project_gate::ProjectActivity::Archived) => Err(KnowledgeError::Conflict(
            "project is archived: knowledge writes are closed".to_string(),
        )),
        Err(crate::project_gate::GateError::NotFound) => Err(KnowledgeError::NotFound),
        Err(err) => Err(KnowledgeError::Db(sqlx::Error::Protocol(err.to_string()))),
    }
}

/// The Git repository that owns Markdown bodies. `AI_PM_KNOWLEDGE_ROOT`
/// overrides the default (a sibling of the ai-pm checkout).
#[derive(Debug, Clone)]
pub struct KnowledgeRoot {
    root: PathBuf,
}

impl KnowledgeRoot {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Reads `AI_PM_KNOWLEDGE_ROOT`; otherwise `<cwd>/../ai-pm-knowledge`.
    pub fn from_env() -> Self {
        if let Ok(value) = std::env::var("AI_PM_KNOWLEDGE_ROOT") {
            if !value.trim().is_empty() {
                return Self::new(value);
            }
        }
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new(cwd.join("..").join("ai-pm-knowledge"))
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Creates the repo on first use, seeds the templates, and refuses to run
    /// inside another Git checkout (knowledge bodies must not mix with code).
    pub fn ensure_ready(&self) -> Result<(), KnowledgeError> {
        std::fs::create_dir_all(&self.root).map_err(io_error)?;

        if self.root.join(".git").exists() {
            self.configure_repo()?;
            return self.ensure_templates();
        }
        if let Ok(toplevel) = git(
            self.root.parent().unwrap_or(&self.root),
            &["rev-parse", "--show-toplevel"],
        ) {
            let toplevel = PathBuf::from(toplevel.trim());
            if toplevel != self.root {
                return Err(KnowledgeError::Config(format!(
                    "knowledge root {} is inside git repository {}; point AI_PM_KNOWLEDGE_ROOT at a standalone repository",
                    self.root.display(),
                    toplevel.display()
                )));
            }
        }
        git(&self.root, &["init"])?;
        self.configure_repo()?;
        self.ensure_templates()
    }

    /// Windows checkouts must not rewrite line endings: the recorded
    /// `content_hash` is computed over the file as written (LF).
    fn configure_repo(&self) -> Result<(), KnowledgeError> {
        git(&self.root, &["config", "core.autocrlf", "false"])?;
        git(&self.root, &["config", "core.longpaths", "true"])?;
        Ok(())
    }

    fn ensure_templates(&self) -> Result<(), KnowledgeError> {
        let dir = self.root.join("templates");
        std::fs::create_dir_all(&dir).map_err(io_error)?;
        let mut written = false;
        for (name, content) in TEMPLATES {
            let path = dir.join(name);
            if !path.exists() {
                std::fs::write(&path, content.as_bytes()).map_err(io_error)?;
                written = true;
            }
        }
        if written {
            git(&self.root, &["add", "templates"])?;
            git(
                &self.root,
                &[
                    "commit",
                    "-m",
                    "chore: bootstrap knowledge templates",
                    "--",
                    "templates",
                ],
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CreateDocumentRequest {
    pub doc_key: String,
    pub doc_type: String,
    pub markdown: String,
    pub change_summary: Option<String>,
    /// T-060：写入幂等键。同一 `(project_id, operation_id)` 重放只产生一条修订。
    pub operation_id: String,
    /// T-060：乐观并发基线。新建文档必须为 `None`。
    pub base_revision_id: Option<Uuid>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CreatedDocument {
    pub document_id: Uuid,
    pub revision_id: Uuid,
    pub revision_no: i32,
    pub git_commit_sha: String,
    pub content_hash: String,
    pub canonical_path: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DocumentSummary {
    pub id: Uuid,
    pub doc_key: String,
    pub doc_type: String,
    pub canonical_path: String,
    pub revision_no: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DocumentContent {
    pub document_id: Uuid,
    pub doc_key: String,
    pub doc_type: String,
    pub canonical_path: String,
    pub revision_no: i32,
    pub git_commit_sha: String,
    pub content_hash: String,
    pub markdown: String,
}

/// `projects/<project-code>/<doc_type>/<doc_key>.md`. The key is restricted to
/// `[A-Za-z0-9._-]` (excluding `.` / `..`), so no value escapes the root.
pub fn canonical_path(
    project_code: &str,
    doc_type: &str,
    doc_key: &str,
) -> Result<String, KnowledgeError> {
    validate_key(doc_key)?;
    validate_type(doc_type)?;
    if project_code.is_empty() || project_code.contains(['/', '\\', '.']) {
        return Err(KnowledgeError::Config(format!(
            "project code `{project_code}` is not path-safe"
        )));
    }
    Ok(format!("projects/{project_code}/{doc_type}/{doc_key}.md"))
}

fn validate_key(doc_key: &str) -> Result<(), KnowledgeError> {
    let ok = !doc_key.is_empty()
        && doc_key.len() <= 120
        && doc_key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && doc_key != "."
        && doc_key != "..";
    if ok {
        Ok(())
    } else {
        Err(KnowledgeError::InvalidKey(doc_key.to_string()))
    }
}

fn validate_type(doc_type: &str) -> Result<(), KnowledgeError> {
    if DOC_TYPES.contains(&doc_type) {
        Ok(())
    } else {
        Err(KnowledgeError::InvalidType(doc_type.to_string()))
    }
}

/// Creates one document: writes the file, commits it, then records identity and
/// revision. Re-submitting identical content after a DB failure reuses the
/// existing commit instead of creating a duplicate one.
///
/// T-060：整条写入路径经 `KnowledgeWriteCoordinator`（project 级会话锁 +
/// `operation_id` 幂等 + `base_revision` 校验），提交后同步重建索引。
pub async fn create_document(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    created_by: Uuid,
    req: CreateDocumentRequest,
) -> Result<CreatedDocument, KnowledgeError> {
    let project_code: String = sqlx::query_scalar("SELECT code FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await?
        .ok_or(KnowledgeError::NotFound)?;

    let path = canonical_path(&project_code, &req.doc_type, &req.doc_key)?;

    // 写锁（跨实例有效）；同 Project 的知识写入串行。
    let guard = crate::knowledge_write_coordinator::acquire(
        pool,
        project_id,
        std::time::Duration::from_millis(
            crate::knowledge_write_coordinator::DEFAULT_WRITE_LOCK_TIMEOUT_MS,
        ),
    )
    .await?;

    // 幂等重放：命中既有账本行 → 返回既有结果，不写 Git、不产生新修订。
    if let Some((_, Some(revision_id))) =
        crate::knowledge_write_coordinator::find_operation(pool, project_id, &req.operation_id)
            .await?
    {
        let existing = load_created_document(pool, project_id, revision_id).await?;
        guard.release().await;
        return Ok(existing);
    }

    // 新建文档没有既有修订可作基线：携带 `base_revision_id` 即是契约错误。
    if req.base_revision_id.is_some() {
        guard.release().await;
        return Err(KnowledgeError::RevisionConflict(
            "a new document must not carry base_revision_id".to_string(),
        ));
    }

    let exists: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM ai.project_document WHERE project_id = $1 AND doc_key = $2",
    )
    .bind(project_id)
    .bind(&req.doc_key)
    .fetch_optional(pool)
    .await?;
    if exists.is_some() {
        guard.release().await;
        return Err(KnowledgeError::Exists(req.doc_key));
    }

    // I4-F 归档守卫（前置检查）：Git 写入**之前**确认 Project 仍 active——
    // 避免归档项目留下孤儿 Git 提交（权威判定在下方事务内的行锁检查）。
    match crate::project_gate::project_activity(pool, project_id).await {
        Ok(crate::project_gate::ProjectActivity::Active) => {}
        Ok(crate::project_gate::ProjectActivity::Archived) => {
            guard.release().await;
            return Err(KnowledgeError::Conflict(
                "project is archived: knowledge writes are closed".to_string(),
            ));
        }
        Err(crate::project_gate::GateError::NotFound) => {
            guard.release().await;
            return Err(KnowledgeError::NotFound);
        }
        Err(err) => {
            guard.release().await;
            return Err(KnowledgeError::Db(sqlx::Error::Protocol(err.to_string())));
        }
    }

    root.ensure_ready()?;
    let (commit_sha, hash) = write_and_commit(
        root,
        &path,
        &req.markdown,
        &format!("docs({}): add {}", req.doc_type, req.doc_key),
    )?;

    let mut tx = pool.begin().await?;
    // I4-F 归档守卫：新建文档同样是一次「正式提交」——归档项目不得再建文档。
    crate::knowledge::ensure_project_active_for_write(&mut tx, project_id).await?;
    // 幂等预占在业务事务内：唯一键冲突即重放，事务回滚后不产生第二条修订。
    let claim = crate::knowledge_write_coordinator::claim_operation(
        &mut tx,
        project_id,
        &req.operation_id,
        crate::knowledge_write_coordinator::WriteCommand::CreateDocument,
    )
    .await?;
    if let crate::knowledge_write_coordinator::OperationClaim::Replayed { revision_id, .. } = claim
    {
        tx.rollback().await?;
        if let Some(revision_id) = revision_id {
            let existing = load_created_document(pool, project_id, revision_id).await?;
            guard.release().await;
            return Ok(existing);
        }
        guard.release().await;
        return Err(KnowledgeError::Conflict(format!(
            "write operation {} is already recorded",
            req.operation_id
        )));
    }

    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.project_document (project_id, doc_key, doc_type, canonical_path, tenant_id) \
         VALUES ($1, $2, $3, $4, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(&req.doc_key)
    .bind(&req.doc_type)
    .bind(&path)
    .fetch_one(&mut *tx)
    .await?;

    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.document_revision \
         (project_id, document_id, revision_no, git_commit_sha, content_hash, status, change_summary, created_by, tenant_id) \
         VALUES ($1, $2, 1, $3, $4, 'published', $5, $6, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(document_id)
    .bind(&commit_sha)
    .bind(&hash)
    .bind(&req.change_summary)
    .bind(created_by)
    .fetch_one(&mut *tx)
    .await?;

    crate::knowledge_write_coordinator::complete_operation(
        &mut tx,
        project_id,
        &req.operation_id,
        None,
        Some(revision_id),
    )
    .await?;
    tx.commit().await?;

    // 提交后同步重建索引（失败只记录，不阻断已提交的发布）。
    crate::knowledge_indexer::rebuild_quietly(pool, root, project_id, revision_id).await;

    guard.release().await;

    Ok(CreatedDocument {
        document_id,
        revision_id,
        revision_no: 1,
        git_commit_sha: commit_sha,
        content_hash: hash,
        canonical_path: path,
    })
}

/// 读回一次已提交的文档创建结果（幂等重放用）。
async fn load_created_document(
    pool: &PgPool,
    project_id: Uuid,
    revision_id: Uuid,
) -> Result<CreatedDocument, KnowledgeError> {
    let row = sqlx::query(
        "SELECT r.document_id, r.revision_no, r.git_commit_sha, r.content_hash, d.canonical_path \
           FROM ai.document_revision r \
           JOIN ai.project_document d ON d.id = r.document_id \
          WHERE r.id = $1 AND r.project_id = $2",
    )
    .bind(revision_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or(KnowledgeError::NotFound)?;
    use sqlx::Row;
    Ok(CreatedDocument {
        document_id: row.get("document_id"),
        revision_id,
        revision_no: row.get("revision_no"),
        git_commit_sha: row.get("git_commit_sha"),
        content_hash: row.get("content_hash"),
        canonical_path: row.get("canonical_path"),
    })
}

/// Lists a project's documents with their latest published revision.
pub async fn list_documents(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<DocumentSummary>, KnowledgeError> {
    let rows: Vec<(Uuid, String, String, String, i32, DateTime<Utc>)> = sqlx::query_as(
        "SELECT d.id, d.doc_key, d.doc_type, d.canonical_path, r.revision_no, r.created_at \
         FROM ai.project_document d \
         JOIN LATERAL ( \
             SELECT revision_no, created_at FROM ai.document_revision \
             WHERE document_id = d.id AND status = 'published' \
             ORDER BY revision_no DESC LIMIT 1 \
         ) r ON true \
         WHERE d.project_id = $1 \
         ORDER BY d.doc_type, d.doc_key",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, doc_key, doc_type, canonical_path, revision_no, updated_at)| DocumentSummary {
                id,
                doc_key,
                doc_type,
                canonical_path,
                revision_no,
                updated_at,
            },
        )
        .collect())
}

/// Reads the body from the file and verifies it still matches the recorded
/// hash. A drifted file is reported as `Stale` instead of silently serving it.
pub async fn read_document(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    document_id: Uuid,
) -> Result<DocumentContent, KnowledgeError> {
    let row: Option<(String, String, String, i32, String, String)> = sqlx::query_as(
        "SELECT d.doc_key, d.doc_type, d.canonical_path, r.revision_no, r.git_commit_sha, r.content_hash \
         FROM ai.project_document d \
         JOIN LATERAL ( \
             SELECT revision_no, git_commit_sha, content_hash FROM ai.document_revision \
             WHERE document_id = d.id AND status = 'published' \
             ORDER BY revision_no DESC LIMIT 1 \
         ) r ON true \
         WHERE d.id = $1 AND d.project_id = $2",
    )
    .bind(document_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    let (doc_key, doc_type, canonical_path, revision_no, git_commit_sha, recorded_hash) =
        row.ok_or(KnowledgeError::NotFound)?;

    let abs = safe_join(root.path(), &canonical_path)?;
    let markdown = std::fs::read_to_string(&abs)
        .map_err(|err| KnowledgeError::Io(format!("{}: {err}", abs.display())))?;
    if content_hash(&markdown) != recorded_hash {
        return Err(KnowledgeError::Stale);
    }

    Ok(DocumentContent {
        document_id,
        doc_key,
        doc_type,
        canonical_path,
        revision_no,
        git_commit_sha,
        content_hash: recorded_hash,
        markdown,
    })
}

fn write_and_commit(
    root: &KnowledgeRoot,
    relative: &str,
    markdown: &str,
    message: &str,
) -> Result<(String, String), KnowledgeError> {
    let abs = safe_join(root.path(), relative)?;
    let hash = content_hash(markdown);

    if abs.exists() {
        let existing = std::fs::read_to_string(&abs).map_err(io_error)?;
        if existing == markdown {
            return Ok((head_sha(root.path())?, hash));
        }
        return Err(KnowledgeError::Exists(relative.to_string()));
    }

    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent).map_err(io_error)?;
    }
    std::fs::write(&abs, markdown.as_bytes()).map_err(io_error)?;
    git(root.path(), &["add", "--", relative])?;
    git(root.path(), &["commit", "-m", message, "--", relative])?;
    Ok((head_sha(root.path())?, hash))
}

/// V2：写入一份**新修订**（允许覆盖已有文件）并提交；内容与现有文件相同时复用
/// 当前 HEAD，不产生重复提交。返回 `(commit_sha, content_hash)`。
pub(crate) fn write_revision(
    root: &KnowledgeRoot,
    relative: &str,
    markdown: &str,
    message: &str,
) -> Result<(String, String), KnowledgeError> {
    let abs = safe_join(root.path(), relative)?;
    let hash = content_hash(markdown);
    if abs.exists() {
        let existing = std::fs::read_to_string(&abs).map_err(io_error)?;
        if existing == markdown {
            return Ok((head_sha(root.path())?, hash));
        }
    }
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent).map_err(io_error)?;
    }
    std::fs::write(&abs, markdown.as_bytes()).map_err(io_error)?;
    git(root.path(), &["add", "--", relative])?;
    git(root.path(), &["commit", "-m", message, "--", relative])?;
    Ok((head_sha(root.path())?, hash))
}

/// 否决窗口：对指定修订的 Git 提交做一次反向提交（revert），返回新的 HEAD。
/// 若该提交已被撤销（无可暂存变更）则返回当前 HEAD，保证重复否决幂等。
pub(crate) fn revert_commit(
    root: &KnowledgeRoot,
    sha: &str,
    message: &str,
) -> Result<String, KnowledgeError> {
    git(root.path(), &["revert", "--no-commit", sha])?;
    let staged = git(root.path(), &["diff", "--cached", "--name-only"])?;
    if staged.trim().is_empty() {
        return head_sha(root.path());
    }
    git(root.path(), &["commit", "-m", message])?;
    head_sha(root.path())
}

/// Deletes one document (file + git commit + both rows). Used as the
/// compensating action when a batch document write fails halfway.
pub async fn delete_document(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    document_id: Uuid,
) -> Result<(), KnowledgeError> {
    let canonical: Option<String> = sqlx::query_scalar(
        "SELECT canonical_path FROM ai.project_document WHERE id = $1 AND project_id = $2",
    )
    .bind(document_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let path = canonical.ok_or(KnowledgeError::NotFound)?;

    let abs = safe_join(root.path(), &path)?;
    if abs.exists() {
        std::fs::remove_file(&abs).map_err(io_error)?;
        git(root.path(), &["add", "-A", "--", &path])?;
        git(
            root.path(),
            &["commit", "-m", "docs: rollback batch import", "--", &path],
        )?;
    }

    let mut tx = pool.begin().await?;
    // V2：提案行通过 FKs 引用文档/修订；删除文档前先解绑，避免外键阻止级联。
    sqlx::query(
        "UPDATE ai.document_proposal SET resulting_revision_id = NULL, document_id = NULL \
          WHERE project_id = $1 AND (document_id = $2 OR resulting_revision_id IN \
              (SELECT id FROM ai.document_revision WHERE document_id = $2))",
    )
    .bind(project_id)
    .bind(document_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM ai.document_revision WHERE document_id = $1")
        .bind(document_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM ai.project_document WHERE id = $1")
        .bind(document_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Lowercase hex sha256 used for `content_hash`.
pub fn content_hash(markdown: &str) -> String {
    let digest = Sha256::digest(markdown.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

pub(crate) fn safe_join(root: &Path, relative: &str) -> Result<PathBuf, KnowledgeError> {
    let rel = Path::new(relative);
    if rel.is_absolute() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(KnowledgeError::Config(format!(
            "unsafe canonical path `{relative}`"
        )));
    }
    Ok(root.join(rel))
}

pub(crate) fn git(root: &Path, args: &[&str]) -> Result<String, KnowledgeError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("-c")
        .arg("user.name=AI-PM")
        .arg("-c")
        .arg("user.email=ai-pm@local")
        .args(args)
        .output()
        .map_err(|err| KnowledgeError::Git(format!("failed to run git: {err}")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(KnowledgeError::Git(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ))
    }
}

pub(crate) fn head_sha(root: &Path) -> Result<String, KnowledgeError> {
    git(root, &["rev-parse", "HEAD"])
}

/// S02-I2：按绑定 ref（git_commit_sha）读历史版本正文——§6.1「按 ref 读 Git」
/// 读形态，rebuild_projection / 审计回放共用。`relative` 必须是仓库内相对路径
/// （canonical_path 产物），path traversal 由 `git show` 的 `<sha>:<path>` 语法
/// 天然约束（不存在逃逸路径）。
pub(crate) fn git_read_at(
    root: &Path,
    relative: &str,
    sha: &str,
) -> Result<String, KnowledgeError> {
    git(root, &["show", &format!("{sha}:{relative}")])
}

fn io_error(err: std::io::Error) -> KnowledgeError {
    KnowledgeError::Io(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static SEQ: AtomicU32 = AtomicU32::new(0);

    fn temp_root() -> KnowledgeRoot {
        let path = std::env::temp_dir().join(format!(
            "ai-pm-knowledge-test-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        KnowledgeRoot::new(path)
    }

    #[test]
    fn canonical_path_is_project_scoped_and_typed() {
        let path = canonical_path("MAP-001", "requirement", "REQ-032").unwrap();
        assert_eq!(path, "projects/MAP-001/requirement/REQ-032.md");
    }

    #[test]
    fn traversal_and_unknown_types_are_rejected() {
        assert!(matches!(
            canonical_path("MAP-001", "requirement", "../escape"),
            Err(KnowledgeError::InvalidKey(_))
        ));
        assert!(matches!(
            canonical_path("MAP-001", "requirement", "a/b"),
            Err(KnowledgeError::InvalidKey(_))
        ));
        assert!(matches!(
            canonical_path("MAP-001", "notes", "REQ-1"),
            Err(KnowledgeError::InvalidType(_))
        ));
        assert!(matches!(
            canonical_path("..", "requirement", "REQ-1"),
            Err(KnowledgeError::Config(_))
        ));
    }

    #[test]
    fn ensure_ready_bootstraps_repo_and_templates_once() {
        let root = temp_root();
        root.ensure_ready().unwrap();
        assert!(root.path().join(".git").exists());
        assert!(root
            .path()
            .join("templates")
            .join("requirement.md")
            .exists());

        let first = head_sha(root.path()).unwrap();
        root.ensure_ready().unwrap();
        assert_eq!(
            head_sha(root.path()).unwrap(),
            first,
            "second call must not commit"
        );

        let _ = std::fs::remove_dir_all(root.path());
    }

    #[test]
    fn writing_identical_content_reuses_the_commit() {
        let root = temp_root();
        root.ensure_ready().unwrap();
        let body = "# 需求\n\n正文\n";
        let (sha1, hash1) =
            write_and_commit(&root, "projects/P1/requirement/R1.md", body, "docs: R1").unwrap();
        let (sha2, hash2) =
            write_and_commit(&root, "projects/P1/requirement/R1.md", body, "docs: R1").unwrap();
        assert_eq!(sha1, sha2);
        assert_eq!(hash1, hash2);
        assert_eq!(content_hash(body), hash1);
        let _ = std::fs::remove_dir_all(root.path());
    }

    #[test]
    fn conflicting_content_is_rejected() {
        let root = temp_root();
        root.ensure_ready().unwrap();
        write_and_commit(&root, "projects/P1/knowledge/K1.md", "a", "docs: K1").unwrap();
        assert!(matches!(
            write_and_commit(&root, "projects/P1/knowledge/K1.md", "b", "docs: K1"),
            Err(KnowledgeError::Exists(_))
        ));
        let _ = std::fs::remove_dir_all(root.path());
    }
}
