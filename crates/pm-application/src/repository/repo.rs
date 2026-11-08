use super::*;

pub async fn list_repositories(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<RepositoryRow>, RepositoryError> {
    project_code(pool, project_id).await?;
    let rows = sqlx::query_as::<_, RepositoryRow>(
        "SELECT id, project_id, name, url, role, default_branch, description, \
                ssh_key_id, status, head_commit_sha, selected_branch, selected_commit, selected_confirmed_at, selected_confirmed_by, last_verified_at, \
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
    let selected_branch = optional_text(req.selected_branch, MAX_BRANCH_LEN)?;
    let selected_commit = optional_text(req.selected_commit, MAX_COMMIT_SHA_LEN)?;
    if selected_commit.is_some() && selected_branch.is_none() {
        return Err(RepositoryError::Validation(
            "selected_branch is required when selected_commit is provided",
        ));
    }

    let mut tx = pool.begin().await?;
    let row = sqlx::query_as::<_, RepositoryRow>(
        "INSERT INTO integration.project_repository \
         (project_id, name, url, role, default_branch, description, tenant_id, \
          selected_branch, selected_commit, selected_confirmed_at, selected_confirmed_by) \
         VALUES ($1, $2, $3, $4, $5, $6, \
                 (SELECT tenant_id FROM core.project WHERE id = $1), $7, $8, \
                 CASE WHEN $8 IS NOT NULL THEN now() END, \
                 CASE WHEN $8 IS NOT NULL THEN $9 END) \
         RETURNING id, project_id, name, url, role, default_branch, description, \
                   ssh_key_id, status, head_commit_sha, selected_branch, selected_commit, \
                   selected_confirmed_at, selected_confirmed_by, last_verified_at, \
                   last_error_code, last_error_detail, revision, created_at, updated_at",
    )
    .bind(project_id)
    .bind(&name)
    .bind(&url)
    .bind(&role)
    .bind(&default_branch)
    .bind(&description)
    .bind(&selected_branch)
    .bind(&selected_commit)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => RepositoryError::Duplicate,
        _ => RepositoryError::Db(e),
    })?;

    // 接入时用户显式给出初始基准 = 首次人工确认（S01-03 §5.2），落 append-only 确认记录。
    if let Some(commit) = selected_commit.clone() {
        sqlx::query(
            "INSERT INTO integration.repository_version_confirmation \
             (tenant_id, repository_id, previous_branch, previous_commit, \
              selected_branch, selected_commit, confirmed_by) \
             VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $2, \
                     NULL, NULL, $3, $4, $5)",
        )
        .bind(project_id)
        .bind(row.id)
        .bind(&selected_branch)
        .bind(&commit)
        .bind(actor.principal_id)
        .execute(&mut *tx)
        .await?;
    }

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

/// 更新仓库身份/角色/分支/备注。**不触碰** selected_*（Confirmed 基准）——
/// 那只能走 `confirm_repository_version`（S01-03 §5.1：不得绕过人工确认）。
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
                   ssh_key_id, status, head_commit_sha, selected_branch, selected_commit, selected_confirmed_at, selected_confirmed_by, last_verified_at, \
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

    let deleted = sqlx::query("DELETE FROM integration.project_repository WHERE id = $1")
        .bind(repository_id)
        .execute(&mut *tx)
        .await;
    // 历史引用保护（基线 §5.1；迁移 0031 把仓库引用换轨为 ON DELETE RESTRICT）：
    // 被 Import / Finding / Task 引用的仓库**不可物理删除**，映射为领域错误而不是 500。
    if let Err(err) = deleted {
        if err
            .as_database_error()
            .map(|db| db.is_foreign_key_violation())
            .unwrap_or(false)
        {
            return Err(RepositoryError::HasHistory);
        }
        return Err(err.into());
    }
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
                   ssh_key_id, status, head_commit_sha, selected_branch, selected_commit, selected_confirmed_at, selected_confirmed_by, last_verified_at, \
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

/// 人类确认更新项目采用基准（G-05 承载，POST /update-version）。
///
/// S01-03 §5.2：Observed HEAD（`head_commit_sha`）只属 verify 自动回写；
/// 本函数只写 `selected_*`（Confirmed Commit）并追加 append-only 确认记录
/// （谁/何时/从什么/到什么/原因），不触碰任何正式基线（S01-05 边界 §一）。
/// 已归档仓库不得确认（软解绑后基准治理停止）。
pub async fn confirm_repository_version(
    pool: &PgPool,
    repository_id: Uuid,
    actor: &AuditActor,
    req: ConfirmVersionRequest,
) -> Result<RepositoryRow, RepositoryError> {
    let branch = require_text(
        Some(req.selected_branch),
        "selected branch is required",
        MAX_BRANCH_LEN,
    )?;
    let commit = require_text(
        Some(req.selected_commit),
        "selected commit is required",
        MAX_COMMIT_SHA_LEN,
    )?;
    let reason = optional_text(req.reason, MAX_DESCRIPTION_LEN)?;

    let mut tx = pool.begin().await?;
    type Row = Option<(
        Uuid,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<Uuid>,
        String,
    )>;
    let existing: Row = sqlx::query_as(
        "SELECT project_id, selected_branch, selected_commit, head_commit_sha, \
                selected_confirmed_by, status \
         FROM integration.project_repository WHERE id = $1",
    )
    .bind(repository_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (project_id, prev_branch, prev_commit, _observed_head, _prev_by, status) =
        existing.ok_or(RepositoryError::NotFound)?;
    if status == "archived" {
        return Err(RepositoryError::Archived);
    }

    sqlx::query(
        "INSERT INTO integration.repository_version_confirmation \
         (tenant_id, repository_id, previous_branch, previous_commit, \
          selected_branch, selected_commit, reason, confirmed_by) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $2, \
                 $3, $4, $5, $6, $7, $8)",
    )
    .bind(project_id)
    .bind(repository_id)
    .bind(&prev_branch)
    .bind(&prev_commit)
    .bind(&branch)
    .bind(&commit)
    .bind(&reason)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;

    let row = sqlx::query_as::<_, RepositoryRow>(
        "UPDATE integration.project_repository SET \
             selected_branch = $2, selected_commit = $3, \
             selected_confirmed_at = now(), selected_confirmed_by = $4, \
             revision = revision + 1, updated_at = now() \
         WHERE id = $1 \
         RETURNING id, project_id, name, url, role, default_branch, description, \
                   ssh_key_id, status, head_commit_sha, selected_branch, selected_commit, \
                   selected_confirmed_at, selected_confirmed_by, last_verified_at, \
                   last_error_code, last_error_detail, revision, created_at, updated_at",
    )
    .bind(repository_id)
    .bind(&branch)
    .bind(&commit)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "repository.version_confirmed",
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
///
/// Checkpoint-1 触发链：verify 成功且 Observed HEAD 前进（首次或变化）→
/// 自动受理「仓库快照」intake（engineering_artifact/system），后续由
/// KB 分析 → 信号路由 → PC 派生的 outbox 消费链推进。幂等键锚
/// `{repository}:{head}`，重复 verify / 同 HEAD 零副作用；intake 受理
/// 失败只 warn 不阻断 verify 本身（重跑 verify 即可重触发）。
/// verify 初始行投影（触发链需要 tenant/name/旧 HEAD）。
type VerifyRepoRow = (
    Uuid,           // project_id
    Uuid,           // tenant_id
    String,         // name
    String,         // url
    Option<String>, // default_branch
    Option<String>, // head_commit_sha（Observed，verify 前值）
);

pub async fn verify_repository(
    pool: &PgPool,
    repository_id: Uuid,
    actor: &AuditActor,
) -> Result<VerifyOutcome, RepositoryError> {
    let row: Option<VerifyRepoRow> = sqlx::query_as(
        "SELECT project_id, tenant_id, name, url, default_branch, head_commit_sha \
         FROM integration.project_repository WHERE id = $1",
    )
    .bind(repository_id)
    .fetch_optional(pool)
    .await?;
    let (project_id, tenant_id, name, url, current_branch, previous_head) =
        row.ok_or(RepositoryError::NotFound)?;

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
                    heads
                        .iter()
                        .find(|h| &h.branch == branch)
                        .map(|h| h.sha.clone())
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

    // Checkpoint-1 触发链（见函数头注）：HEAD 前进 → 仓库快照 intake。
    if outcome.status == "verified" {
        if let Some(head) = outcome.head_commit_sha.as_deref() {
            if previous_head.as_deref() != Some(head) {
                let cmd = SnapshotIntakeCmd {
                    tenant_id,
                    project_id,
                    repository_id,
                    name: &name,
                    url: &url,
                    head_commit_sha: head,
                };
                match submit_repository_snapshot_intake(pool, cmd, actor).await {
                    Ok(receipt) => tracing::info!(
                        intake_id = %receipt.intake_id,
                        replayed = receipt.replayed,
                        repository_id = %repository_id,
                        head,
                        "verify: repository snapshot intake submitted"
                    ),
                    Err(err) => tracing::warn!(
                        repository_id = %repository_id,
                        head,
                        error = %err,
                        "verify: repository snapshot intake submit failed (verify result kept)"
                    ),
                }
            }
        }
    }

    Ok(outcome)
}

/// 仓库快照 intake 受理入参（打包降参数；字段语义见 [`submit_repository_snapshot_intake`]）。
struct SnapshotIntakeCmd<'a> {
    tenant_id: Uuid,
    project_id: Uuid,
    repository_id: Uuid,
    name: &'a str,
    url: &'a str,
    head_commit_sha: &'a str,
}

/// 仓库快照 intake 受理（checkpoint-1 触发链；intake.rs 管道由此接续：
/// kb_analyze → 认知信号 → RT 路由 pc_analyze，全链经 outbox 消费端推进）。
/// 幂等键 `{repository}:{head}`：同 commit 重复 verify 重放，新 commit 新 intake。
async fn submit_repository_snapshot_intake(
    pool: &PgPool,
    cmd: SnapshotIntakeCmd<'_>,
    actor: &AuditActor,
) -> Result<crate::intake::IntakeReceipt, crate::intake::IntakeError> {
    let short: String = cmd.head_commit_sha.chars().take(8).collect();
    crate::intake::submit_intake(
        pool,
        crate::intake::SubmitIntake {
            project_id: cmd.project_id,
            tenant_id: cmd.tenant_id,
            submitted_by: actor.principal_id,
            intake_type: "engineering_artifact".to_string(),
            channel: "system".to_string(),
            source_ref: serde_json::json!({
                "kind": "repository_snapshot",
                "uri": cmd.url,
                "repository_id": cmd.repository_id.to_string(),
                "head_commit_sha": cmd.head_commit_sha,
                "content_hash": cmd.head_commit_sha,
            }),
            // commit sha 本身即树内容哈希——直接作为 intake 对账锚。
            content_hash: Some(cmd.head_commit_sha.to_string()),
            title: Some(format!("仓库快照 {} @ {short}", cmd.name)),
            idempotency_key: Some(format!(
                "repo_verify:{}:{}",
                cmd.repository_id, cmd.head_commit_sha
            )),
            tasks: None,
        },
    )
    .await
}

// ------------------------------------------------------------------- internals
