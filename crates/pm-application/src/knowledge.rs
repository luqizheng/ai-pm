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
    #[error("knowledge root misconfigured: {0}")]
    Config(String),
    #[error("knowledge root io error: {0}")]
    Io(String),
    #[error("git error: {0}")]
    Git(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
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
                &["commit", "-m", "chore: bootstrap knowledge templates", "--", "templates"],
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
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CreatedDocument {
    pub document_id: Uuid,
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

    let exists: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM ai.project_document WHERE project_id = $1 AND doc_key = $2")
            .bind(project_id)
            .bind(&req.doc_key)
            .fetch_optional(pool)
            .await?;
    if exists.is_some() {
        return Err(KnowledgeError::Exists(req.doc_key));
    }

    root.ensure_ready()?;
    let (commit_sha, hash) = write_and_commit(
        root,
        &path,
        &req.markdown,
        &format!(
            "docs({}): add {}",
            req.doc_type,
            req.doc_key
        ),
    )?;

    let mut tx = pool.begin().await?;
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.project_document (project_id, doc_key, doc_type, canonical_path) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(project_id)
    .bind(&req.doc_key)
    .bind(&req.doc_type)
    .bind(&path)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO ai.document_revision \
         (project_id, document_id, revision_no, git_commit_sha, content_hash, status, change_summary, created_by) \
         VALUES ($1, $2, 1, $3, $4, 'published', $5, $6)",
    )
    .bind(project_id)
    .bind(document_id)
    .bind(&commit_sha)
    .bind(&hash)
    .bind(&req.change_summary)
    .bind(created_by)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(CreatedDocument {
        document_id,
        revision_no: 1,
        git_commit_sha: commit_sha,
        content_hash: hash,
        canonical_path: path,
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
    let markdown = std::fs::read_to_string(&abs).map_err(|err| {
        KnowledgeError::Io(format!("{}: {err}", abs.display()))
    })?;
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

fn safe_join(root: &Path, relative: &str) -> Result<PathBuf, KnowledgeError> {
    let rel = Path::new(relative);
    if rel.is_absolute() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(KnowledgeError::Config(format!(
            "unsafe canonical path `{relative}`"
        )));
    }
    Ok(root.join(rel))
}

fn git(root: &Path, args: &[&str]) -> Result<String, KnowledgeError> {
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

fn head_sha(root: &Path) -> Result<String, KnowledgeError> {
    git(root, &["rev-parse", "HEAD"])
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
        assert!(root.path().join("templates").join("requirement.md").exists());

        let first = head_sha(root.path()).unwrap();
        root.ensure_ready().unwrap();
        assert_eq!(head_sha(root.path()).unwrap(), first, "second call must not commit");

        let _ = std::fs::remove_dir_all(root.path());
    }

    #[test]
    fn writing_identical_content_reuses_the_commit() {
        let root = temp_root();
        root.ensure_ready().unwrap();
        let body = "# 需求\n\n正文\n";
        let (sha1, hash1) = write_and_commit(&root, "projects/P1/requirement/R1.md", body, "docs: R1").unwrap();
        let (sha2, hash2) = write_and_commit(&root, "projects/P1/requirement/R1.md", body, "docs: R1").unwrap();
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