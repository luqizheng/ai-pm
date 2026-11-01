use super::*;

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


