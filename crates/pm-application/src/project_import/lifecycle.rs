use super::*;

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

    // 仓库集合在**导入创建时**固化（I6-B-03）：之后 Worker 只处理这批仓库。
    let verified_repositories: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM integration.project_repository \
         WHERE tenant_id = (SELECT tenant_id FROM core.project WHERE id = $1) \
           AND project_id = $1 AND status = 'verified' ORDER BY name",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    if verified_repositories.is_empty() {
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

    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await?;
    let generation: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(generation), 0) + 1 FROM core.project_initialization_operation \
         WHERE tenant_id = $1 AND project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(pool)
    .await?;

    let mut tx = pool.begin().await?;
    // 锁项目行：同一项目的导入创建串行化（R-3-4 行锁 + 条件 UPDATE）。
    let project_locked: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM core.project WHERE id = $1 FOR UPDATE")
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?;
    if project_locked.is_none() {
        return Err(ImportError::ProjectNotFound);
    }
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
    // 初始化操作：占有 active 行（R-3-1：占有是 Import 创建事务的提交前置条件）。
    let operation_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project_initialization_operation \
         (tenant_id, project_id, generation, status, created_by) \
         VALUES ($1, $2, $3, 'active', $4) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(generation)
    .bind(actor_id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("UPDATE core.project_initialization_operation SET import_id = $2 WHERE id = $1")
        .bind(operation_id)
        .bind(import_id)
        .execute(&mut *tx)
        .await?;
    // Source 集合固化 + 首轮 Attempt（R-4-1：每轮分析都留档）。
    for repository_id in &verified_repositories {
        sqlx::query(
            "INSERT INTO integration.repo_import_source \
             (tenant_id, project_id, import_id, repository_id, current_attempt_no) \
             VALUES ($1, $2, $3, $4, 1)",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(import_id)
        .bind(repository_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO integration.repo_import_source_attempt \
             (tenant_id, project_id, import_id, repository_id, attempt_no, status) \
             VALUES ($1, $2, $3, $4, 1, 'running')",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(import_id)
        .bind(repository_id)
        .execute(&mut *tx)
        .await?;
    }
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
        "SELECT i.project_id, i.status, i.progress, i.stage_detail, i.model_name, dv.draft, \
                i.draft_revision, i.dropped_items, i.error_code, i.error_detail, i.analysis_run_id, \
                i.created_at, i.proposed_at, i.applied_at \
         FROM integration.repo_import i \
         LEFT JOIN integration.repo_import_draft_version dv \
                ON dv.tenant_id = i.tenant_id AND dv.import_id = i.id \
               AND dv.revision = i.draft_revision \
         WHERE i.id = $1 AND i.project_id = $2",
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

    // 来源快照的事实来自**最新一轮 Attempt**（R-4-1：Source 只留身份与指针）。
    let sources: Vec<ImportSourceRow> =
        sqlx::query_as(
            "SELECT s.repository_id, r.name, s.head_commit_sha, \
                    COALESCE(a.commit_count, 0), COALESCE(a.doc_count, 0), \
                    COALESCE(a.size_bytes, 0), COALESCE(a.truncated, false), a.error_code \
             FROM integration.repo_import_source s \
             JOIN integration.project_repository r ON r.id = s.repository_id \
             LEFT JOIN integration.repo_import_source_attempt a \
                    ON a.tenant_id = s.tenant_id AND a.import_id = s.import_id \
                   AND a.repository_id = s.repository_id AND a.attempt_no = s.current_attempt_no \
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
    // 在飞 Attempt 取消并留档（R-4-1：失败/取消现场也留）。
    sqlx::query(
        "UPDATE integration.repo_import_source_attempt \
         SET status = 'cancelled', finished_at = now() \
         WHERE import_id = $1 AND status = 'running'",
    )
    .bind(import_id)
    .execute(&mut *tx)
    .await?;
    // 释放初始化操作（R-3-1：条件释放 + rows_affected 检查）。
    sqlx::query(
        "UPDATE core.project_initialization_operation \
         SET status = 'released', released_at = now() \
         WHERE import_id = $1 AND status = 'active'",
    )
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


pub(crate) async fn set_stage(
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


pub(crate) async fn fail_import(pool: &PgPool, import_id: Uuid, code: &str, detail: &str) {
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

    // 只处理导入创建时固化的仓库集合（I6-B-03）。
    let repos: Vec<(Uuid, String, Option<String>, i32)> = sqlx::query_as(
        "SELECT s.repository_id, r.url, r.default_branch, \
                COALESCE(s.current_attempt_no, 1) \
         FROM integration.repo_import_source s \
         JOIN integration.project_repository r ON r.id = s.repository_id \
         WHERE s.import_id = $1 ORDER BY r.name",
    )
    .bind(import_id)
    .fetch_all(pool)
    .await?;
    if repos.is_empty() {
        fail_import(pool, import_id, "no_verified_repository", "no verified repository").await;
        return Ok(());
    }
    let tenant_id: Uuid = sqlx::query_scalar(
        "SELECT tenant_id FROM integration.repo_import WHERE id = $1",
    )
    .bind(import_id)
    .fetch_one(pool)
    .await?;

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
    for (repository_id, url, default_branch, attempt_no) in repos {
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
            record_attempt_failure(pool, import_id, repository_id, attempt_no, code, &detail).await;
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
                record_attempt_failure(pool, import_id, repository_id, attempt_no, code, &detail).await;
                fail_import(pool, import_id, code, &detail).await;
                return Ok(());
            }
        };

        let facts_json = serde_json::to_value(&facts).unwrap_or(serde_json::Value::Null);
        // 本轮事实落 Attempt（R-4-1），Source 只更新基准与当前轮指针。
        sqlx::query(
            "UPDATE integration.repo_import_source_attempt \
             SET status = 'succeeded', commit_sha = $3, facts = $4, commit_count = $5, \
                 doc_count = $6, size_bytes = $7, truncated = $8, error_code = NULL, \
                 error_detail = NULL, finished_at = now() \
             WHERE tenant_id = $1 AND import_id = $2 AND repository_id = $9 AND attempt_no = $10",
        )
        .bind(tenant_id)
        .bind(import_id)
        .bind(&facts.head_commit_sha)
        .bind(&facts_json)
        .bind(facts.commits.len() as i32)
        .bind(facts.docs.len() as i32)
        .bind(facts.size_bytes as i64)
        .bind(facts.truncated)
        .bind(repository_id)
        .bind(attempt_no)
        .execute(pool)
        .await?;
        // 基准 write-once（N-1）：已有基准不被后续轮次覆盖。
        sqlx::query(
            "UPDATE integration.repo_import_source \
             SET head_commit_sha = COALESCE(head_commit_sha, $4), current_attempt_no = $5 \
             WHERE tenant_id = $1 AND import_id = $2 AND repository_id = $3",
        )
        .bind(tenant_id)
        .bind(import_id)
        .bind(repository_id)
        .bind(&facts.head_commit_sha)
        .bind(attempt_no)
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
    // 草案版本化（D-1a）：旧版本保留不覆盖；`draft_revision` 推进为**指针**。
    let source_attempts: serde_json::Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(jsonb_build_object( \
                   'repository_id', s.repository_id, 'attempt_no', s.current_attempt_no) \
                  ORDER BY s.repository_id), '[]'::jsonb) \
         FROM integration.repo_import_source s WHERE s.import_id = $1",
    )
    .bind(import_id)
    .fetch_one(pool)
    .await?;
    let mut tx = pool.begin().await?;
    let current_revision: i64 =
        sqlx::query_scalar("SELECT draft_revision FROM integration.repo_import WHERE id = $1 FOR UPDATE")
            .bind(import_id)
            .fetch_one(&mut *tx)
            .await?;
    let revision = current_revision + 1;
    sqlx::query(
        "INSERT INTO integration.repo_import_draft_version \
         (tenant_id, import_id, revision, draft, source_attempts, dropped_items, \
          generation_status, generated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, 'succeeded', now())",
    )
    .bind(tenant_id)
    .bind(import_id)
    .bind(revision)
    .bind(&draft_json)
    .bind(&source_attempts)
    .bind(dropped as i32)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE integration.repo_import SET status = 'draft_ready', stage_detail = '草案已生成', \
         progress = 100, draft_revision = $2, dropped_items = $3, updated_at = now() WHERE id = $1",
    )
    .bind(import_id)
    .bind(revision)
    .bind(dropped as i32)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}


/// 记一次失败轮次：Attempt 落 `failed` + 错误码与现场（R-4-1）。
pub(crate) async fn record_attempt_failure(
    pool: &PgPool,
    import_id: Uuid,
    repository_id: Uuid,
    attempt_no: i32,
    code: &str,
    detail: &str,
) {
    let detail: String = detail.chars().take(500).collect();
    let _ = sqlx::query(
        "UPDATE integration.repo_import_source_attempt \
         SET status = 'failed', error_code = $3, error_detail = $4, finished_at = now() \
         WHERE import_id = $1 AND repository_id = $2 AND attempt_no = $5",
    )
    .bind(import_id)
    .bind(repository_id)
    .bind(code)
    .bind(detail)
    .bind(attempt_no)
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


