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

/// 仓库绑定/解绑 SSH key（迁移 0022，PUT /repositories/{id}/ssh-key）。
/// `Some(key_id)` = 绑定该项目下的 active key；`None` = 解绑（回退项目默认）。
/// 不走 PATCH revision（同 verify：治理动作不经乐观并发），已归档仓库拒绝。
pub async fn bind_repository_ssh_key(
    pool: &PgPool,
    repository_id: Uuid,
    actor: &AuditActor,
    ssh_key_id: Option<Uuid>,
) -> Result<RepositoryRow, RepositoryError> {
    let mut tx = pool.begin().await?;
    let repo: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT project_id, status FROM integration.project_repository WHERE id = $1",
    )
    .bind(repository_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (project_id, status) = repo.ok_or(RepositoryError::NotFound)?;
    if status == "archived" {
        return Err(RepositoryError::Archived);
    }
    if let Some(key_id) = ssh_key_id {
        let key: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT project_id, status FROM integration.project_ssh_key WHERE id = $1",
        )
        .bind(key_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((key_project, key_status)) = key else {
            return Err(RepositoryError::SshKeyNotFound);
        };
        if key_project != project_id {
            return Err(RepositoryError::Validation(
                "ssh key belongs to another project",
            ));
        }
        if key_status != "active" {
            return Err(RepositoryError::Validation("ssh key is revoked"));
        }
    }

    let row = sqlx::query_as::<_, RepositoryRow>(
        "UPDATE integration.project_repository SET ssh_key_id = $2, updated_at = now() \
         WHERE id = $1 \
         RETURNING id, project_id, name, url, role, default_branch, description, \
                   ssh_key_id, status, head_commit_sha, selected_branch, selected_commit, \
                   selected_confirmed_at, selected_confirmed_by, last_verified_at, \
                   last_error_code, last_error_detail, revision, created_at, updated_at",
    )
    .bind(repository_id)
    .bind(ssh_key_id)
    .fetch_one(&mut *tx)
    .await?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        if ssh_key_id.is_some() {
            "repository.ssh_key_bound"
        } else {
            "repository.ssh_key_unbound"
        },
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
        String,
        String,
        Uuid,
    )>;
    let existing: Row = sqlx::query_as(
        "SELECT r.project_id, r.selected_branch, r.selected_commit, r.head_commit_sha, \
                r.selected_confirmed_by, r.status, r.name, r.url, \
                (SELECT tenant_id FROM core.project WHERE id = r.project_id) \
         FROM integration.project_repository r WHERE r.id = $1",
    )
    .bind(repository_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (
        project_id,
        prev_branch,
        prev_commit,
        _observed_head,
        _prev_by,
        status,
        name,
        url,
        tenant_id,
    ) = existing.ok_or(RepositoryError::NotFound)?;
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

    // A-1（裁决 2026-11-08）：确认新采用基准 → 认知分析联动。分析锚点已改为
    // `selected_commit ?? Observed HEAD ?? 分支 tip`（见 kb_pc_analysis），
    // 确认动作本身意味着「认知应锚到新版本」，故这里直接受理快照 intake。
    // 幂等键与 verify 同族（repo_snapshot:{repo}:{commit}）：一 commit 一次
    // 受理，确认已在 verify 链分析过的版本时自然重放零副作用。
    let cmd = SnapshotIntakeCmd {
        tenant_id,
        project_id,
        repository_id,
        name: &name,
        url: &url,
        head_commit_sha: &commit,
        key_prefix: "repo_confirm",
        // 联动受理保持既有幂等键语义（无触发时刻段）。
        trigger_token: None,
        run_mode: "full",
        since_commit: None,
    };
    match submit_repository_snapshot_intake(pool, cmd, actor).await {
        Ok(receipt) => tracing::info!(
            intake_id = %receipt.intake_id,
            replayed = receipt.replayed,
            repository_id = %repository_id,
            commit,
            "confirm: repository snapshot intake submitted"
        ),
        Err(err) => tracing::warn!(
            repository_id = %repository_id,
            commit,
            error = %err,
            "confirm: repository snapshot intake submit failed (confirm result kept)"
        ),
    }
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
    Option<Uuid>,   // ssh_key_id（仓库级绑定；None = 跟随项目默认）
);

pub async fn verify_repository(
    pool: &PgPool,
    repository_id: Uuid,
    actor: &AuditActor,
) -> Result<VerifyOutcome, RepositoryError> {
    let row: Option<VerifyRepoRow> = sqlx::query_as(
        "SELECT project_id, tenant_id, name, url, default_branch, head_commit_sha, ssh_key_id \
         FROM integration.project_repository WHERE id = $1",
    )
    .bind(repository_id)
    .fetch_optional(pool)
    .await?;
    let (project_id, tenant_id, name, url, current_branch, previous_head, bound_key) =
        row.ok_or(RepositoryError::NotFound)?;

    // 迁移 0022：仓库绑定了 key 用绑定把（失效 → 按 missing 处理，不 500）；
    // 未绑定回退「项目默认 key → 最新 active」。
    let key_ciphertext: Option<Vec<u8>> = match bound_key {
        Some(key_id) => key_ciphertext_by_id(pool, project_id, key_id).await?,
        None => fallback_key_ciphertext(pool, project_id).await?,
    };

    let is_ssh = url.starts_with("ssh://") || url.starts_with("git@");
    let outcome = if is_ssh && key_ciphertext.is_none() {
        VerifyOutcome {
            repository_id,
            status: "unreachable".to_string(),
            default_branch: current_branch,
            head_commit_sha: None,
            error_code: Some("ssh_key_missing".to_string()),
            error_detail: Some("no active ssh key bound to this repository".to_string()),
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
                    GitError::Network(msg) => ("network_error", msg.clone()),
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
                    key_prefix: "repo_verify",
                    // verify 联动保持既有幂等键语义（无触发时刻段）。
                    trigger_token: None,
                    run_mode: "full",
                    since_commit: None,
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
    /// 幂等键族（`{prefix}:{repository}:{commit}`）：verify=repo_verify、
    /// 确认联动=repo_confirm、手动分析=repo_analyze。族内同 commit 跨触发
    /// 仍可能各一条——键锚 commit，KB/PC 内容寻址去重保证同 commit
    /// 二次分析零新增修订。
    key_prefix: &'a str,
    /// 手动分析的「触发时刻」段（S1）：`Some` 时键追加该段，使 HEAD 未前进
    /// 也能重触发；`None` 保持 verify / confirm 既有键格式与语义不变。
    trigger_token: Option<&'a str>,
    /// D-2 运行口径（仅手动分析链携带；verify/confirm 恒 full）：
    /// incremental = 自 since_commit 起区间采集 + 既有认知 merge 重析。
    run_mode: &'a str,
    /// 增量起点（= 上一份关闭报告固化的 last_analyzed_commit_sha）。
    since_commit: Option<&'a str>,
}

/// 仓库快照 intake 受理（checkpoint-1 触发链；intake.rs 管道由此接续：
/// kb_analyze → 认知信号 → RT 路由 pc_analyze，全链经 outbox 消费端推进）。
///
/// 幂等键：`{prefix}:{repository}:{commit}`（联动），或手动分析的
/// `{prefix}:{repository}:{commit}:{trigger_token}`。同键重复受理重放，
/// 新键新 intake；`trigger_token = None` 时既有键语义完全不变。
async fn submit_repository_snapshot_intake(
    pool: &PgPool,
    cmd: SnapshotIntakeCmd<'_>,
    actor: &AuditActor,
) -> Result<crate::intake::IntakeReceipt, crate::intake::IntakeError> {
    let short: String = cmd.head_commit_sha.chars().take(8).collect();
    let idempotency_key = match cmd.trigger_token {
        Some(token) => format!(
            "{}:{}:{}:{}",
            cmd.key_prefix, cmd.repository_id, cmd.head_commit_sha, token
        ),
        None => format!(
            "{}:{}:{}",
            cmd.key_prefix, cmd.repository_id, cmd.head_commit_sha
        ),
    };
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
                "run_mode": cmd.run_mode,
                "since_commit": cmd.since_commit,
            }),
            // commit sha 本身即树内容哈希——直接作为 intake 对账锚。
            content_hash: Some(cmd.head_commit_sha.to_string()),
            title: Some(format!("仓库快照 {} @ {short}", cmd.name)),
            idempotency_key: Some(idempotency_key),
            tasks: None,
        },
    )
    .await
}

/// S1（2026-11-13）：单仓库**手动增量分析** —— 人工触发仓库快照 intake。
///
/// 与 verify / confirm 联动的区别：
/// - 幂等键族 `repo_analyze:{repo}:{commit}:{trigger}`（**不与** `repo_verify:`
///   / `repo_confirm:` 共用），`trigger` 锚触发时刻，故 **HEAD 未前进也允许
///   重触发**；调用方显式传 `idempotency_key` 时该键即 `trigger`，重复提交
///   命中幂等重放（零副作用，防双击双发）。
/// - 进行中/限流对齐 `intake::retry_task`：活跃任务
///   （PENDING / DISPATCHED / PROCESSING）不可并发重触发 → 409；
///   FAILED / NEEDS_HUMAN_REVIEW 等终态**释放名额**，允许重触发。
///   检查时排除本次幂等键自身的 intake，保证重放不被自己的活跃任务挡住。
/// - 锚点 = `selected_commit ?? head_commit_sha`（与 kb_pc_analysis 一致）；
///   未核验或无锚点 → 422；已归档 → 409。
pub async fn analyze_repository(
    pool: &PgPool,
    repository_id: Uuid,
    actor: &AuditActor,
    req: AnalyzeRepositoryRequest,
) -> Result<AnalyzeRepositoryOutcome, RepositoryError> {
    type AnalyzeRow = (
        Uuid,           // project_id
        Uuid,           // tenant_id
        String,         // name
        String,         // url
        String,         // status
        Option<String>, // selected_commit（Confirmed）
        Option<String>, // head_commit_sha（Observed）
        Option<String>, // last_analyzed_commit_sha（D-1 锚点）
    );
    let row: Option<AnalyzeRow> = sqlx::query_as(
        "SELECT r.project_id, r.tenant_id, r.name, r.url, r.status, \
                r.selected_commit, r.head_commit_sha, r.last_analyzed_commit_sha \
         FROM integration.project_repository r WHERE r.id = $1",
    )
    .bind(repository_id)
    .fetch_optional(pool)
    .await?;
    let (
        project_id,
        tenant_id,
        name,
        url,
        status,
        selected_commit,
        head_commit_sha,
        last_analyzed_commit_sha,
    ) = row.ok_or(RepositoryError::NotFound)?;
    if status == "archived" {
        return Err(RepositoryError::Archived);
    }
    // D-2 运行口径：incremental（默认）/ full；请求 mode 非法 → 400。
    let run_mode = match req.mode.as_deref() {
        None | Some("incremental") => "incremental",
        Some("full") => "full",
        Some(_other) => {
            return Err(RepositoryError::Validation(
                "mode must be incremental or full",
            ))
        }
    };
    // 增量起点 = D-1 锚点（上一份关闭报告固化）；缺失（首次分析）自动退全量。
    let since_commit = if run_mode == "incremental" {
        last_analyzed_commit_sha.filter(|value| !value.trim().is_empty())
    } else {
        None
    };
    let run_mode = if since_commit.is_some() {
        run_mode
    } else {
        "full"
    };
    // 未核验或没有可用锚点，一律 422（前端无 requirement count 端点，不臆测）。
    let commit = selected_commit
        .or(head_commit_sha)
        .filter(|value| !value.trim().is_empty())
        .ok_or(RepositoryError::AnalyzeNoAnchor)?;
    if status != "verified" {
        return Err(RepositoryError::AnalyzeNoAnchor);
    }

    // D-5 门：同仓库存在未关闭报告（analyzing/analyzed）→ 409 报告门
    // （报告关闭才放行；failed/cancelled/closed 不占门）。
    let open_report: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM integration.repo_analysis_report \
          WHERE repository_id = $1 AND status IN ('analyzing','analyzed') \
          ORDER BY created_at DESC LIMIT 1",
    )
    .bind(repository_id)
    .fetch_optional(pool)
    .await?;
    if let Some(report_id) = open_report {
        return Err(RepositoryError::ReportOpen { report_id });
    }

    // 「触发时刻」：显式幂等键优先（可重放），否则由服务端生成且每次唯一。
    let trigger_token: String = req
        .idempotency_key
        .filter(|key| !key.trim().is_empty())
        .unwrap_or_else(|| format!("{}-{}", Utc::now().timestamp_millis(), Uuid::new_v4()));
    let analysis_key = format!("repo_analyze:{repository_id}:{commit}:{trigger_token}");

    // 活跃任务检查：排除本次幂等键自身，使重放直达 submit_intake 的 ON CONFLICT。
    // 状态口径与 retry_task 的「非 FAILED / NEEDS_HUMAN_REVIEW 不可重试」同族：
    // 只有活跃态占名额；终态（含失败 / 待人工复核 / 已完成）释放名额。
    let active: Option<String> = sqlx::query_scalar(
        "SELECT t.task_status \
           FROM integration.intake_task t \
           JOIN integration.intake_record r ON r.id = t.intake_id \
          WHERE r.project_id = $1 \
            AND r.idempotency_key LIKE 'repo_analyze:' || $2 || ':%' \
            AND r.idempotency_key <> $3 \
            AND t.task_status IN ('PENDING','DISPATCHED','PROCESSING') \
          LIMIT 1",
    )
    .bind(project_id)
    .bind(repository_id.to_string())
    .bind(&analysis_key)
    .fetch_optional(pool)
    .await?;
    if active.is_some() {
        return Err(RepositoryError::AnalyzeInProgress);
    }

    // 人工动作留审计（与 verify / confirm 同口径；独立于 intake 受理事务）。
    let mut audit_tx = pool.begin().await?;
    append_audit(
        &mut audit_tx,
        project_id,
        actor,
        "repository.analysis_requested",
        "project_repository",
        Some(repository_id),
    )
    .await?;
    audit_tx.commit().await?;

    let cmd = SnapshotIntakeCmd {
        tenant_id,
        project_id,
        repository_id,
        name: &name,
        url: &url,
        head_commit_sha: &commit,
        key_prefix: "repo_analyze",
        trigger_token: Some(&trigger_token),
        run_mode,
        since_commit: since_commit.as_deref(),
    };
    let receipt = match submit_repository_snapshot_intake(pool, cmd, actor).await {
        Ok(receipt) => receipt,
        Err(crate::intake::IntakeError::Db(err)) => return Err(RepositoryError::Db(err)),
        Err(other) => return Err(RepositoryError::AnalyzeIntake(other.to_string())),
    };
    // D-5：受理即建报告（analyzing）；幂等重放落到既有行。报告行缺失不阻塞
    // 受理（存量/异常路径下钩子全为静默跳过）。
    let report_id = match crate::analysis_report::ensure_for_intake(
        pool,
        tenant_id,
        project_id,
        repository_id,
        &commit,
        receipt.intake_id,
        run_mode,
        since_commit.as_deref(),
    )
    .await
    {
        Ok(id) => id,
        Err(err) => return Err(RepositoryError::Db(err)),
    };
    Ok(AnalyzeRepositoryOutcome {
        repository_id,
        project_id,
        commit,
        intake_id: receipt.intake_id,
        report_id,
        status: receipt.status,
        replayed: receipt.replayed,
    })
}

// ------------------------------------------------------------------- internals
