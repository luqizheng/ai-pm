//! 项目 ↔ 代码仓库绑定（P0：纯关系 CRUD）+ 项目级 SSH 凭据。
//!
//! 本模块只负责「建立与管理关系」：仓库地址、默认分支、角色、备注，以及一把
//! 项目级 ed25519 密钥的生成 / 读取 / 删除。不做 clone、不做可达性校验、不做
//! AI 分析——那些属于后续切片。
//!
//! 密钥策略与 `llm_config.rs` 一致：`AI_PM_SECRET_KEY` → `sha256` → AES-256-GCM，
//! 库里只存 `nonce || ciphertext || tag`，AAD 绑定 `project_id`；私钥明文永不回显。
//! 操作者由调用方显式传入（T-021a：HTTP handler 取 `AuthedPrincipal`），
//! 并写 `governance.audit_log`。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::git_access::{self, GitError};
use crate::actor::AuditActor;
use crate::pm_write::PmWriteError;
use crate::secret::{self, SecretError};

pub const REPOSITORY_ROLES: [&str; 4] = ["main", "docs", "service", "infra"];
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
    #[error("ssh key is still referenced by a verified repository")]
    KeyInUse,
    #[error("git access failed: {0}")]
    Git(String),
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

impl From<SecretError> for RepositoryError {
    fn from(e: SecretError) -> Self {
        match e {
            SecretError::KeyMissing => RepositoryError::SecretKeyMissing,
            SecretError::Crypto => RepositoryError::Crypto,
            SecretError::SshKeygen(detail) => RepositoryError::SshKeygen(detail),
            SecretError::Fingerprint => {
                RepositoryError::SshKeygen("ssh-keygen produced no fingerprint".to_string())
            }
        }
    }
}

impl From<GitError> for RepositoryError {
    fn from(e: GitError) -> Self {
        RepositoryError::Git(e.to_string())
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
    pub ssh_key_id: Option<Uuid>,
    pub status: String,
    pub head_commit_sha: Option<String>,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub last_error_code: Option<String>,
    pub last_error_detail: Option<String>,
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
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// 校验后的 git 地址。`normalized` 是写库的规范形式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitUrl {
    pub user: String,
    pub host: String,
    pub path: String,
    pub normalized: String,
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

/// 读取项目的 active 密钥（无密钥返回 None）。响应只含公钥与指纹。
pub async fn get_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Option<SshKeyView>, RepositoryError> {
    let row: Option<(String, String, String, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT label, public_key, fingerprint, status, created_at \
         FROM integration.project_ssh_key WHERE project_id = $1 AND status = 'active'",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(
        |(label, public_key, fingerprint, status, created_at)| SshKeyView {
            project_id,
            label,
            public_key,
            fingerprint,
            status,
            created_at,
        },
    ))
}

/// 列出项目所有密钥（含已吊销），供管理页展示。
pub async fn list_ssh_keys(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<SshKeyView>, RepositoryError> {
    let rows: Vec<(String, String, String, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT label, public_key, fingerprint, status, created_at \
         FROM integration.project_ssh_key WHERE project_id = $1 ORDER BY created_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(label, public_key, fingerprint, status, created_at)| SshKeyView {
                project_id,
                label,
                public_key,
                fingerprint,
                status,
                created_at,
            },
        )
        .collect())
}

/// 生成或复用项目密钥。`rotate=false` 时已有 active 直接返回（向导幂等）；
/// `rotate=true` 时覆盖为新的密钥对（管理页「重新生成」）。
pub async fn ensure_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    rotate: bool,
) -> Result<SshKeyView, RepositoryError> {
    if !rotate {
        if let Some(existing) = get_ssh_key(pool, project_id).await? {
            return Ok(existing);
        }
    }

    let code = project_code(pool, project_id).await?;
    let generated = secret::generate_ed25519(&format!("ai-pm:{code}"))?;
    let ciphertext = secret::seal_text(project_id, generated.private_key.as_bytes())?;

    let mut tx = pool.begin().await?;
    let (created_at,): (DateTime<Utc>,) = sqlx::query_as(
        "INSERT INTO integration.project_ssh_key \
         (project_id, public_key, private_key_ciphertext, fingerprint, status, created_by, tenant_id) \
         VALUES ($1, $2, $3, $4, 'active', $5, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) \
         ON CONFLICT (project_id) DO UPDATE SET \
             public_key = EXCLUDED.public_key, \
             private_key_ciphertext = EXCLUDED.private_key_ciphertext, \
             fingerprint = EXCLUDED.fingerprint, \
             status = 'active', \
             revoked_at = NULL, \
             created_by = EXCLUDED.created_by, \
             updated_at = now() \
         RETURNING created_at",
    )
    .bind(project_id)
    .bind(&generated.public_key)
    .bind(&ciphertext)
    .bind(&generated.fingerprint)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        if rotate {
            "ssh_key.rotated"
        } else {
            "ssh_key.generated"
        },
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
        status: "active".to_string(),
        created_at,
    })
}

/// 兼容既有调用方：等同于 rotate=true。
pub async fn generate_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
) -> Result<SshKeyView, RepositoryError> {
    ensure_ssh_key(pool, project_id, actor, true).await
}

/// 吊销密钥；仍被 verified 仓库引用时拒绝。
pub async fn revoke_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
) -> Result<(), RepositoryError> {
    let in_use: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.project_repository \
         WHERE project_id = $1 AND status = 'verified'",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    if in_use > 0 {
        return Err(RepositoryError::KeyInUse);
    }
    let mut tx = pool.begin().await?;
    let affected = sqlx::query(
        "UPDATE integration.project_ssh_key \
         SET status = 'revoked', revoked_at = now(), updated_at = now() \
         WHERE project_id = $1 AND status = 'active'",
    )
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
        "ssh_key.revoked",
        "project_ssh_key",
        Some(project_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
) -> Result<(), RepositoryError> {
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
    let plain = secret::open_text(project_id, ciphertext)?;
    String::from_utf8(plain).map_err(|_| RepositoryError::Crypto)
}

// ----------------------------------------------------------------- repositories

pub async fn list_repositories(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<RepositoryRow>, RepositoryError> {
    project_code(pool, project_id).await?;
    let rows = sqlx::query_as::<_, RepositoryRow>(
        "SELECT id, project_id, name, url, role, default_branch, description, \
                ssh_key_id, status, head_commit_sha, last_verified_at, \
                last_error_code, last_error_detail, revision, created_at, updated_at \
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
    actor: &AuditActor,
    req: CreateRepositoryRequest,
) -> Result<RepositoryRow, RepositoryError> {
    project_code(pool, project_id).await?;
    let name = require_text(Some(req.name), "repository name is required", MAX_NAME_LEN)?;
    let url = normalize_url(&req.url)?;
    let role = normalize_role(req.role.as_deref().unwrap_or("main"))?;
    let default_branch = optional_text(req.default_branch, MAX_BRANCH_LEN)?;
    let description = optional_text(req.description, MAX_DESCRIPTION_LEN)?;

    let mut tx = pool.begin().await?;
    let row = sqlx::query_as::<_, RepositoryRow>(
        "INSERT INTO integration.project_repository \
         (project_id, name, url, role, default_branch, description, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id, project_id, name, url, role, default_branch, description, \
                   ssh_key_id, status, head_commit_sha, last_verified_at, \
                   last_error_code, last_error_detail, revision, created_at, updated_at",
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
    actor: &AuditActor,
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

    let row = sqlx::query_as::<_, RepositoryRow>(
        "UPDATE integration.project_repository SET \
             name = $2, url = $3, role = $4, default_branch = $5, description = $6, \
             revision = revision + 1, updated_at = now() \
         WHERE id = $1 \
         RETURNING id, project_id, name, url, role, default_branch, description, \
                   ssh_key_id, status, head_commit_sha, last_verified_at, \
                   last_error_code, last_error_detail, revision, created_at, updated_at",
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
    actor: &AuditActor,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM integration.project_repository WHERE id = $1")
            .bind(repository_id)
            .fetch_optional(&mut *tx)
            .await?;
    let project_id = project_id.ok_or(RepositoryError::NotFound)?;

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

/// 一次可达性校验的结果（同时已回写到仓库行）。
#[derive(Debug, Clone, Serialize)]
pub struct VerifyOutcome {
    pub repository_id: Uuid,
    pub status: String,
    pub default_branch: Option<String>,
    pub head_commit_sha: Option<String>,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
}

/// 归档：保留行与历史，只把状态置 `archived`。
pub async fn archive_repository(
    pool: &PgPool,
    repository_id: Uuid,
    actor: &AuditActor,
) -> Result<RepositoryRow, RepositoryError> {
    let mut tx = pool.begin().await?;
    let project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM integration.project_repository WHERE id = $1")
            .bind(repository_id)
            .fetch_optional(&mut *tx)
            .await?;
    let project_id = project_id.ok_or(RepositoryError::NotFound)?;

    let row = sqlx::query_as::<_, RepositoryRow>(
        "UPDATE integration.project_repository \
         SET status = 'archived', revision = revision + 1, updated_at = now() \
         WHERE id = $1 \
         RETURNING id, project_id, name, url, role, default_branch, description, \
                   ssh_key_id, status, head_commit_sha, last_verified_at, \
                   last_error_code, last_error_detail, revision, created_at, updated_at",
    )
    .bind(repository_id)
    .fetch_one(&mut *tx)
    .await?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "repository.archived",
        "project_repository",
        Some(repository_id),
    )
    .await?;
    tx.commit().await?;
    Ok(row)
}

/// `git ls-remote --heads` 校验可达性并回写状态。
///
/// SSH 地址必须有 active 项目密钥；本地 `file://`（测试夹具）走匿名。
/// 失败不返回 Err（除数据库错误），而是把仓库标记为 `unreachable` 并给出
/// 稳定错误码，供前端展示原因。
pub async fn verify_repository(
    pool: &PgPool,
    repository_id: Uuid,
    actor: &AuditActor,
) -> Result<VerifyOutcome, RepositoryError> {
    let row: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT project_id, url, default_branch \
         FROM integration.project_repository WHERE id = $1",
    )
    .bind(repository_id)
    .fetch_optional(pool)
    .await?;
    let (project_id, url, current_branch) = row.ok_or(RepositoryError::NotFound)?;

    let key_ciphertext: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT private_key_ciphertext FROM integration.project_ssh_key \
         WHERE project_id = $1 AND status = 'active'",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    let is_ssh = url.starts_with("ssh://") || url.starts_with("git@");
    let outcome = if is_ssh && key_ciphertext.is_none() {
        VerifyOutcome {
            repository_id,
            status: "unreachable".to_string(),
            default_branch: current_branch,
            head_commit_sha: None,
            error_code: Some("ssh_key_missing".to_string()),
            error_detail: Some("project has no active ssh key".to_string()),
        }
    } else {
        let material = match key_ciphertext {
            Some(bytes) => {
                let private = open_ssh_private_key(project_id, &bytes)?;
                Some(git_access::write_temp_key(&private)?)
            }
            None => None,
        };
        let key_path = material.as_ref().map(|m| m.key_path());
        let result = git_access::ls_remote(
            &url,
            key_path.as_deref(),
            std::time::Duration::from_secs(30),
        )
        .await;
        match result {
            Ok(heads) => {
                let branch = current_branch
                    .clone()
                    .filter(|wanted| heads.iter().any(|h| &h.branch == wanted))
                    .or_else(|| {
                        heads
                            .iter()
                            .find(|h| h.branch == "main")
                            .map(|h| h.branch.clone())
                    })
                    .or_else(|| {
                        heads
                            .iter()
                            .find(|h| h.branch == "master")
                            .map(|h| h.branch.clone())
                    })
                    .or_else(|| heads.first().map(|h| h.branch.clone()));
                let sha = branch.as_ref().and_then(|branch| {
                    heads.iter().find(|h| &h.branch == branch).map(|h| h.sha.clone())
                });
                VerifyOutcome {
                    repository_id,
                    status: "verified".to_string(),
                    default_branch: branch,
                    head_commit_sha: sha,
                    error_code: None,
                    error_detail: None,
                }
            }
            Err(err) => {
                let (code, detail) = match &err {
                    GitError::Timeout => ("timeout", "git ls-remote timed out".to_string()),
                    GitError::AuthFailed => ("auth_failed", err.to_string()),
                    GitError::NotFound => ("not_found", err.to_string()),
                    GitError::TooLarge => ("too_large", err.to_string()),
                    GitError::Other(msg) => ("git_failed", msg.clone()),
                    GitError::Io(msg) => ("git_failed", msg.clone()),
                };
                let detail = detail.chars().take(500).collect::<String>();
                VerifyOutcome {
                    repository_id,
                    status: "unreachable".to_string(),
                    default_branch: current_branch,
                    head_commit_sha: None,
                    error_code: Some(code.to_string()),
                    error_detail: Some(detail),
                }
            }
        }
    };

    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE integration.project_repository SET \
             status = $2, default_branch = $3, head_commit_sha = $4, \
             last_verified_at = now(), last_error_code = $5, last_error_detail = $6, \
             revision = revision + 1, updated_at = now() \
         WHERE id = $1",
    )
    .bind(repository_id)
    .bind(&outcome.status)
    .bind(&outcome.default_branch)
    .bind(&outcome.head_commit_sha)
    .bind(&outcome.error_code)
    .bind(&outcome.error_detail)
    .execute(&mut *tx)
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "repository.verified",
        "project_repository",
        Some(repository_id),
    )
    .await?;
    tx.commit().await?;

    Ok(outcome)
}

// ------------------------------------------------------------------- internals

async fn project_code(pool: &PgPool, project_id: Uuid) -> Result<String, RepositoryError> {
    sqlx::query_scalar("SELECT code FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await?
        .ok_or(RepositoryError::ProjectNotFound)
}

async fn append_audit(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (id, project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, \
          action, target_type, target_id, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
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
pub fn validate_git_url(raw: &str) -> Result<GitUrl, RepositoryError> {
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
    if let Some(allowed) = allowed_hosts() {
        if !allowed.iter().any(|item| item == host) {
            return Err(RepositoryError::InvalidUrl);
        }
    }
    Ok(GitUrl {
        user: user.to_string(),
        host: host.to_string(),
        path: path.to_string(),
        normalized: url.to_string(),
    })
}

/// `AI_PM_REPO_ALLOWED_HOSTS`（逗号分隔）存在时只允许这些 host。
fn allowed_hosts() -> Option<Vec<String>> {
    let raw = std::env::var("AI_PM_REPO_ALLOWED_HOSTS").ok()?;
    let hosts: Vec<String> = raw
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect();
    if hosts.is_empty() {
        None
    } else {
        Some(hosts)
    }
}

fn normalize_url(raw: &str) -> Result<String, RepositoryError> {
    validate_git_url(raw).map(|url| url.normalized)
}

fn host_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}

fn path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '~' | '/')
}

// ---------------------------------------------------------------- key + crypto


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