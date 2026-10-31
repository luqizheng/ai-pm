//! 项目 ↔ 代码仓库绑定（P0：纯关系 CRUD）+ 项目级 SSH 凭据。
//!
//! 本模块只负责「建立与管理关系」：仓库地址、默认分支、角色、备注，以及一把
//! 项目级 ed25519 密钥的生成 / 读取 / 删除。不做 clone、不做可达性校验、不做
//! AI 分析——那些属于后续切片。
//!
//! 密钥策略与 `llm_config.rs` 一致：`AI_PM_SECRET_KEY` → `sha256` → AES-256-GCM，
//! 库里只存 `nonce || ciphertext || tag`，AAD 绑定 `project_id`；私钥明文永不回显。
//! 操作走 `pm_write::owner_actor` 的身份约定并写 `governance.audit_log`。

use std::path::{Path, PathBuf};
use std::process::Command;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::pm_write::{PmWriteError, owner_actor};

pub const REPOSITORY_ROLES: [&str; 4] = ["main", "docs", "service", "infra"];
const NONCE_LEN: usize = 12;
const MAX_URL_LEN: usize = 512;
const MAX_NAME_LEN: usize = 120;
const MAX_BRANCH_LEN: usize = 200;
const MAX_DESCRIPTION_LEN: usize = 1000;

#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("project not found")]
    ProjectNotFound,
    #[error("repository not found")]
    NotFound,
    #[error("repository ssh key not found")]
    SshKeyNotFound,
    #[error("invalid repository url")]
    InvalidUrl,
    #[error("repository url already bound to this project")]
    Duplicate,
    #[error("revision conflict")]
    RevisionConflict,
    #[error("invalid repository role")]
    InvalidRole,
    #[error("{0}")]
    Validation(&'static str),
    #[error("AI_PM_SECRET_KEY is not set")]
    SecretKeyMissing,
    #[error("secret could not be encrypted or decrypted")]
    Crypto,
    #[error("ssh-keygen is not available: {0}")]
    SshKeygen(String),
    #[error("database error: {0}")]
    Db(sqlx::Error),
}

impl From<sqlx::Error> for RepositoryError {
    fn from(e: sqlx::Error) -> Self {
        RepositoryError::Db(e)
    }
}

impl From<PmWriteError> for RepositoryError {
    fn from(e: PmWriteError) -> Self {
        match e {
            PmWriteError::NotFound(m) => RepositoryError::Validation(m),
            PmWriteError::Conflict(m) => RepositoryError::Validation(m),
            PmWriteError::Validation(m) => RepositoryError::Validation(m),
            PmWriteError::Db(err) => RepositoryError::Db(err),
        }
    }
}

// ------------------------------------------------------------------ rows / dto

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct RepositoryRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub url: String,
    pub role: String,
    pub default_branch: Option<String>,
    pub description: Option<String>,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 对外只暴露公钥与指纹；私钥密文不出库。
#[derive(Debug, Clone, Serialize)]
pub struct SshKeyView {
    pub project_id: Uuid,
    pub label: String,
    pub public_key: String,
    pub fingerprint: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRepositoryRequest {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateRepositoryRequest {
    pub revision: i64,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

// -------------------------------------------------------------------- ssh key

/// 读取项目密钥（无密钥返回 None）。响应只含公钥与指纹。
pub async fn get_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Option<SshKeyView>, RepositoryError> {
    let row: Option<(String, String, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT label, public_key, fingerprint, created_at \
         FROM integration.project_ssh_key WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(label, public_key, fingerprint, created_at)| SshKeyView {
        project_id,
        label,
        public_key,
        fingerprint,
        created_at,
    }))
}

/// 生成（或重新生成）项目密钥：一个项目一把，重复调用即替换。
pub async fn generate_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor_id: Option<Uuid>,
) -> Result<SshKeyView, RepositoryError> {
    let code = project_code(pool, project_id).await?;
    let generated = generate_ed25519(&format!("ai-pm:{code}"))?;
    let ciphertext = seal_bytes(project_id, generated.private_key.as_bytes())?;
    let actor = resolve_actor(pool, project_id, actor_id).await?;

    let mut tx = pool.begin().await?;
    let (created_at,): (DateTime<Utc>,) = sqlx::query_as(
        "INSERT INTO integration.project_ssh_key \
         (project_id, public_key, private_key_ciphertext, fingerprint) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (project_id) DO UPDATE SET \
             public_key = EXCLUDED.public_key, \
             private_key_ciphertext = EXCLUDED.private_key_ciphertext, \
             fingerprint = EXCLUDED.fingerprint, \
             updated_at = now() \
         RETURNING created_at",
    )
    .bind(project_id)
    .bind(&generated.public_key)
    .bind(&ciphertext)
    .bind(&generated.fingerprint)
    .fetch_one(&mut *tx)
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "ssh_key.generated",
        "project_ssh_key",
        Some(project_id),
    )
    .await?;
    tx.commit().await?;

    Ok(SshKeyView {
        project_id,
        label: "default".to_string(),
        public_key: generated.public_key,
        fingerprint: generated.fingerprint,
        created_at,
    })
}

pub async fn delete_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor_id: Option<Uuid>,
) -> Result<(), RepositoryError> {
    let actor = resolve_actor(pool, project_id, actor_id).await?;
    let mut tx = pool.begin().await?;
    let affected = sqlx::query("DELETE FROM integration.project_ssh_key WHERE project_id = $1")
        .bind(project_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(RepositoryError::SshKeyNotFound);
    }
    append_audit(
        &mut tx,
        project_id,
        actor,
        "ssh_key.deleted",
        "project_ssh_key",
        Some(project_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 解密私钥（仅供后续 clone 切片使用；P0 不暴露 REST）。
pub fn open_ssh_private_key(
    project_id: Uuid,
    ciphertext: &[u8],
) -> Result<String, RepositoryError> {
    let plain = open_bytes(project_id, ciphertext)?;
    String::from_utf8(plain).map_err(|_| RepositoryError::Crypto)
}

// ----------------------------------------------------------------- repositories

pub async fn list_repositories(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<RepositoryRow>, RepositoryError> {
    project_code(pool, project_id).await?;
    let rows = sqlx::query_as::<_, RepositoryRow>(
        "SELECT id, project_id, name, url, role, default_branch, description, revision, \
                created_at, updated_at \
         FROM integration.project_repository \
         WHERE project_id = $1 \
         ORDER BY created_at ASC, name ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn create_repository(
    pool: &PgPool,
    project_id: Uuid,
    actor_id: Option<Uuid>,
    req: CreateRepositoryRequest,
) -> Result<RepositoryRow, RepositoryError> {
    project_code(pool, project_id).await?;
    let name = require_text(Some(req.name), "repository name is required", MAX_NAME_LEN)?;
    let url = normalize_url(&req.url)?;
    let role = normalize_role(req.role.as_deref().unwrap_or("main"))?;
    let default_branch = optional_text(req.default_branch, MAX_BRANCH_LEN)?;
    let description = optional_text(req.description, MAX_DESCRIPTION_LEN)?;
    let actor = resolve_actor(pool, project_id, actor_id).await?;

    let mut tx = pool.begin().await?;
    let row = sqlx::query_as::<_, RepositoryRow>(
        "INSERT INTO integration.project_repository \
         (project_id, name, url, role, default_branch, description) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         RETURNING id, project_id, name, url, role, default_branch, description, \
                   revision, created_at, updated_at",
    )
    .bind(project_id)
    .bind(&name)
    .bind(&url)
    .bind(&role)
    .bind(&default_branch)
    .bind(&description)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => RepositoryError::Duplicate,
        _ => RepositoryError::Db(e),
    })?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "repository.created",
        "project_repository",
        Some(row.id),
    )
    .await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn update_repository(
    pool: &PgPool,
    repository_id: Uuid,
    actor_id: Option<Uuid>,
    req: UpdateRepositoryRequest,
) -> Result<RepositoryRow, RepositoryError> {
    let name = require_text(Some(req.name), "repository name is required", MAX_NAME_LEN)?;
    let url = normalize_url(&req.url)?;
    let role = normalize_role(req.role.as_deref().unwrap_or("main"))?;
    let default_branch = optional_text(req.default_branch, MAX_BRANCH_LEN)?;
    let description = optional_text(req.description, MAX_DESCRIPTION_LEN)?;

    let mut tx = pool.begin().await?;
    let existing: Option<(Uuid, i64)> = sqlx::query_as(
        "SELECT project_id, revision FROM integration.project_repository WHERE id = $1",
    )
    .bind(repository_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (project_id, revision) = existing.ok_or(RepositoryError::NotFound)?;
    if revision != req.revision {
        return Err(RepositoryError::RevisionConflict);
    }
    let actor = resolve_actor(pool, project_id, actor_id).await?;

    let row = sqlx::query_as::<_, RepositoryRow>(
        "UPDATE integration.project_repository SET \
             name = $2, url = $3, role = $4, default_branch = $5, description = $6, \
             revision = revision + 1, updated_at = now() \
         WHERE id = $1 \
         RETURNING id, project_id, name, url, role, default_branch, description, \
                   revision, created_at, updated_at",
    )
    .bind(repository_id)
    .bind(&name)
    .bind(&url)
    .bind(&role)
    .bind(&default_branch)
    .bind(&description)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => RepositoryError::Duplicate,
        _ => RepositoryError::Db(e),
    })?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "repository.updated",
        "project_repository",
        Some(row.id),
    )
    .await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn delete_repository(
    pool: &PgPool,
    repository_id: Uuid,
    actor_id: Option<Uuid>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM integration.project_repository WHERE id = $1")
            .bind(repository_id)
            .fetch_optional(&mut *tx)
            .await?;
    let project_id = project_id.ok_or(RepositoryError::NotFound)?;
    let actor = resolve_actor(pool, project_id, actor_id).await?;

    sqlx::query("DELETE FROM integration.project_repository WHERE id = $1")
        .bind(repository_id)
        .execute(&mut *tx)
        .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "repository.deleted",
        "project_repository",
        Some(repository_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

// ------------------------------------------------------------------- internals

async fn project_code(pool: &PgPool, project_id: Uuid) -> Result<String, RepositoryError> {
    sqlx::query_scalar("SELECT code FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await?
        .ok_or(RepositoryError::ProjectNotFound)
}

async fn resolve_actor(
    pool: &PgPool,
    project_id: Uuid,
    actor_id: Option<Uuid>,
) -> Result<Uuid, RepositoryError> {
    match actor_id {
        Some(id) => Ok(id),
        None => Ok(owner_actor(pool, project_id).await?),
    }
}

async fn append_audit(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    actor_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (id, project_id, actor_id, action, target_type, target_id) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(actor_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn require_text(
    value: Option<String>,
    message: &'static str,
    max: usize,
) -> Result<String, RepositoryError> {
    let text = value.unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Err(RepositoryError::Validation(message));
    }
    if text.chars().count() > max {
        return Err(RepositoryError::Validation("value is too long"));
    }
    Ok(text)
}

fn optional_text(
    value: Option<String>,
    max: usize,
) -> Result<Option<String>, RepositoryError> {
    let text = value.unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Ok(None);
    }
    if text.chars().count() > max {
        return Err(RepositoryError::Validation("value is too long"));
    }
    Ok(Some(text))
}

fn normalize_role(raw: &str) -> Result<String, RepositoryError> {
    let role = raw.trim();
    if REPOSITORY_ROLES.contains(&role) {
        Ok(role.to_string())
    } else {
        Err(RepositoryError::InvalidRole)
    }
}

/// 只接受 `ssh://[user@]host[:port]/path` 与 `git@host:path` 两种 git over SSH
/// 形式；拒绝 `ext::`、`file://`、`http(s)://`、`-` 开头、空白、`..`、`%`、反斜杠。
fn normalize_url(raw: &str) -> Result<String, RepositoryError> {
    let url = raw.trim();
    if url.is_empty() || url.len() > MAX_URL_LEN {
        return Err(RepositoryError::InvalidUrl);
    }
    if url.starts_with('-')
        || url.chars().any(char::is_whitespace)
        || url.contains("..")
        || url.contains('%')
        || url.contains('\\')
    {
        return Err(RepositoryError::InvalidUrl);
    }

    let (user, host, path) = if let Some(rest) = url.strip_prefix("ssh://") {
        let (authority, path) = rest.split_once('/').ok_or(RepositoryError::InvalidUrl)?;
        let (user, host_port) = authority
            .rsplit_once('@')
            .map_or(("git", authority), |(u, h)| (u, h));
        let host = match host_port.rsplit_once(':') {
            Some((h, port)) => {
                if h.is_empty() || port.is_empty() || port.len() > 5 || !port.chars().all(|c| c.is_ascii_digit()) {
                    return Err(RepositoryError::InvalidUrl);
                }
                h
            }
            None => host_port,
        };
        (user, host, path)
    } else if let Some(rest) = url.strip_prefix("git@") {
        let (host, path) = rest.split_once(':').ok_or(RepositoryError::InvalidUrl)?;
        ("git", host, path)
    } else {
        return Err(RepositoryError::InvalidUrl);
    };

    if user.is_empty() || !user.chars().all(host_char) {
        return Err(RepositoryError::InvalidUrl);
    }
    if host.is_empty() || !host.chars().all(host_char) {
        return Err(RepositoryError::InvalidUrl);
    }
    if path.is_empty() || path.starts_with('/') || !path.chars().all(path_char) {
        return Err(RepositoryError::InvalidUrl);
    }
    Ok(url.to_string())
}

fn host_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}

fn path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '~' | '/')
}

// ---------------------------------------------------------------- key + crypto

struct GeneratedKey {
    public_key: String,
    private_key: String,
    fingerprint: String,
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn generate_ed25519(comment: &str) -> Result<GeneratedKey, RepositoryError> {
    let dir = TempDir(std::env::temp_dir().join(format!("ai-pm-ssh-{}", Uuid::new_v4())));
    std::fs::create_dir_all(&dir.0).map_err(|e| RepositoryError::SshKeygen(e.to_string()))?;
    let key_path = dir.0.join("id_ed25519");

    let output = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", comment, "-f"])
        .arg(&key_path)
        .output()
        .map_err(|e| RepositoryError::SshKeygen(e.to_string()))?;
    if !output.status.success() {
        return Err(RepositoryError::SshKeygen(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    let pub_path = key_path.with_extension("pub");
    let public_key = std::fs::read_to_string(&pub_path)
        .map_err(|e| RepositoryError::SshKeygen(e.to_string()))?
        .trim()
        .to_string();
    let private_key = std::fs::read_to_string(&key_path)
        .map_err(|e| RepositoryError::SshKeygen(e.to_string()))?;
    let fingerprint = fingerprint_of(&pub_path)?;

    Ok(GeneratedKey {
        public_key,
        private_key,
        fingerprint,
    })
}

fn fingerprint_of(pub_path: &Path) -> Result<String, RepositoryError> {
    let output = Command::new("ssh-keygen")
        .arg("-lf")
        .arg(pub_path)
        .output()
        .map_err(|e| RepositoryError::SshKeygen(e.to_string()))?;
    if !output.status.success() {
        return Err(RepositoryError::SshKeygen(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .nth(1)
        .map(str::to_string)
        .ok_or_else(|| RepositoryError::SshKeygen("unexpected ssh-keygen -lf output".to_string()))
}

fn cipher() -> Result<Aes256Gcm, RepositoryError> {
    let raw = std::env::var("AI_PM_SECRET_KEY").map_err(|_| RepositoryError::SecretKeyMissing)?;
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(RepositoryError::SecretKeyMissing);
    }
    let key: [u8; 32] = Sha256::digest(raw.as_bytes()).into();
    Aes256Gcm::new_from_slice(&key).map_err(|_| RepositoryError::Crypto)
}

fn seal_bytes(project_id: Uuid, plain: &[u8]) -> Result<Vec<u8>, RepositoryError> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|_| RepositoryError::Crypto)?;
    let encrypted = cipher()?
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plain,
                aad: project_id.as_bytes(),
            },
        )
        .map_err(|_| RepositoryError::Crypto)?;
    let mut out = Vec::with_capacity(NONCE_LEN + encrypted.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&encrypted);
    Ok(out)
}

fn open_bytes(project_id: Uuid, payload: &[u8]) -> Result<Vec<u8>, RepositoryError> {
    if payload.len() <= NONCE_LEN {
        return Err(RepositoryError::Crypto);
    }
    let (nonce, ciphertext) = payload.split_at(NONCE_LEN);
    cipher()?
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: project_id.as_bytes(),
            },
        )
        .map_err(|_| RepositoryError::Crypto)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_accepts_scp_and_ssh_forms() {
        assert!(normalize_url("git@gitlab.local:team/world-map.git").is_ok());
        assert!(normalize_url("ssh://git@gitlab.local:2222/team/repo.git").is_ok());
        assert!(normalize_url("ssh://gitlab.local/team/repo.git").is_ok());
    }

    #[test]
    fn url_rejects_executables_and_traversal() {
        for bad in [
            "ext::sh -c rm",
            "file:///etc/passwd",
            "https://github.com/team/repo.git",
            "-oProxyCommand=touch /tmp/pwned",
            "git@host:../escape",
            "git@host:team/repo with space.git",
            "git@host:team/repo%.git",
            "git@host:team\\repo.git",
        ] {
            assert!(normalize_url(bad).is_err(), "expected reject: {bad}");
        }
    }

    #[test]
    fn role_whitelist_is_enforced() {
        assert!(normalize_role("main").is_ok());
        assert!(normalize_role("docs").is_ok());
        assert!(normalize_role("unknown").is_err());
    }
}