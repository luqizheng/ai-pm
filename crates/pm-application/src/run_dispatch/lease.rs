use super::*;

/// 写入本 Run 的多仓基线快照，返回给审计的摘要。
pub(crate) async fn snapshot_repositories(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT tr.repository_id, tr.write_access, \
                tr.base_ref, coalesce(tr.base_commit, pr.head_commit_sha) AS base_commit, \
                pr.name AS repo_name, tr.path_scope \
           FROM delivery.task_repository tr \
           JOIN integration.project_repository pr \
             ON pr.id = tr.repository_id AND pr.project_id = tr.project_id \
          WHERE tr.task_id = $1 \
          ORDER BY tr.repository_id",
    )
    .bind(task_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut summary = Vec::with_capacity(rows.len());
    for row in rows {
        let repository_id: Uuid = row.get("repository_id");
        let write_access: bool = row.get("write_access");
        let base_ref: String = row.get("base_ref");
        let base_commit: String = row.get("base_commit");
        let repo_name: String = row.get("repo_name");
        let path_scope: Option<Vec<String>> = row.get("path_scope");
        let workspace_path = workspace_path_for(repository_id, &repo_name);
        sqlx::query(
            "INSERT INTO delivery.task_run_repository \
             (run_id, project_id, repository_id, base_ref, base_commit, write_access, workspace_path, path_scope, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, \
                     (SELECT tenant_id FROM core.project WHERE id = $2)) \
             ON CONFLICT (run_id, repository_id) DO NOTHING",
        )
        .bind(run_id)
        .bind(project_id)
        .bind(repository_id)
        .bind(&base_ref)
        .bind(&base_commit)
        .bind(write_access)
        .bind(&workspace_path)
        .bind(&path_scope)
        .execute(&mut **tx)
        .await?;
        summary.push(serde_json::json!({
            "repository_id": repository_id,
            "base_commit": base_commit,
            "workspace_path": workspace_path,
        }));
    }
    Ok(summary)
}

/// 隔离已启动的执行器实例并记录终止结果（B3-2 的前置）。
pub(crate) async fn isolate_started_run(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    cfg: &RunDispatchConfig,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'timed_out', outcome = 'timed_out', finished_at = now(), \
                terminated_at = now(), termination_reason = 'lease_expired', \
                infrastructure_retry_count = infrastructure_retry_count + 1, \
                lease_owner = NULL, lease_expires_at = NULL, lease_token_hash = NULL \
          WHERE id = $1",
    )
    .bind(run_id)
    .execute(&mut **tx)
    .await?;
    // 失败记录不可变（append-only）；基础设施类失败不计入任务执行连续失败（B3-4）。
    sqlx::query(
        "INSERT INTO delivery.task_run_failure \
         (run_id, project_id, category, retryable, reason_code, timeout_seconds, tenant_id) \
         SELECT $1, project_id, 'infrastructure', true, 'lease_lost', $2, tenant_id \
           FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .bind(cfg.lease_secs)
    .execute(&mut **tx)
    .await
    .ok();
    // S05 监控事件面（0013）：失联隔离终态落账（同事务；B3-2 新建重试 Run 的
    // 领取经 claim_core 落自己的 run_claimed）。
    crate::run_events::record_run_event(
        tx,
        run_id,
        crate::run_events::RunEvent {
            event_type: crate::run_events::event_type::RUN_FINISHED,
            generation: None,
            dedupe_tag: "",
            detail: Some(serde_json::json!({
                "outcome": "timed_out",
                "reason": "lease_expired",
            })),
        },
    )
    .await?;
    Ok(())
}

/// 退避回队（资格类不满足）：同一行回 `queued`，`dispatch_attempts+1`，指数退避。
pub(crate) async fn requeue_with_backoff(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    _dispatch_attempts: i32,
    cfg: &RunDispatchConfig,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'queued', lease_owner = NULL, lease_expires_at = NULL, lease_token_hash = NULL, \
                dispatch_attempts = dispatch_attempts + 1, \
                next_dispatch_at = now() + make_interval(secs => \
                    LEAST($3::float8, power(2, dispatch_attempts))::float8), \
                release_reason = $2 \
          WHERE id = $1",
    )
    .bind(run_id)
    .bind(reason)
    .bind(cfg.backoff_cap_secs)
    .execute(&mut **tx)
    .await
    .map(|_| ())
}

/// 同事务内把 Run 重置回可领取的 `queued`（不递增 dispatch_attempts）。
pub(crate) async fn requeue_for_redispatch(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'queued', lease_owner = NULL, lease_expires_at = NULL, lease_token_hash = NULL, \
                next_dispatch_at = now() \
          WHERE id = $1",
    )
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .map(|_| ())
}

/// B3-2 共享收尾：隔离已启动的失联 Run 并新建重试 Run（`supersedes_run_id` 指向
/// 旧 Run）+ 审计。`claim_core` 的回收分支与 `run_executor::reconcile_orphans`
/// 必须复用本函数，不得各写一套。前置（B2）：调用者已持有 task 行锁。
pub(crate) async fn recover_lost_run(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    task_id: Uuid,
    project_id: Uuid,
    cfg: &RunDispatchConfig,
) -> Result<CreatedRun, sqlx::Error> {
    crate::run_transition_guard::lock_and_validate_transition(
        tx, project_id, run_id,
        crate::run_transition_guard::RunKind::Delivery,
        crate::run_transition_guard::RunStatus::TimedOut,
    ).await.map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    isolate_started_run(tx, run_id, cfg).await?;
    let created = insert_task_run(tx, project_id, task_id, None).await?;
    sqlx::query("UPDATE delivery.task_run SET supersedes_run_id = $2 WHERE id = $1")
        .bind(created.run_id)
        .bind(run_id)
        .execute(&mut **tx)
        .await?;
    audit_run(
        tx,
        project_id,
        "reclaim_task_run",
        run_id,
        serde_json::json!({
            "reason": "lease_expired_started",
            "new_run_id": created.run_id,
            "attempt_no": created.attempt_no,
        }),
    )
    .await?;
    Ok(created)
}

/// Run 落 `cancelled`（任务终态/不存在），并留审计。
pub(crate) async fn cancel_run(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'cancelled', outcome = 'cancelled', finished_at = now(), \
                release_reason = $2, lease_owner = NULL, lease_expires_at = NULL, lease_token_hash = NULL \
          WHERE id = $1",
    )
    .bind(run_id)
    .bind(reason)
    .execute(&mut **tx)
    .await
    .map(|_| ())?;
    // S05 监控事件面（0013）：取消终态落账（同事务）。
    crate::run_events::record_run_event(
        tx,
        run_id,
        crate::run_events::RunEvent {
            event_type: crate::run_events::event_type::RUN_FINISHED,
            generation: None,
            dedupe_tag: "",
            detail: Some(serde_json::json!({ "outcome": "cancelled", "reason": reason })),
        },
    )
    .await
    .map(|_| ())
}

/// 平台专用审计兜底 principal（`0015` 幂等种子）。
pub const DISPATCH_SERVICE_PRINCIPAL: Uuid = Uuid::from_u128(1);

/// 续租（§5.9，D3 执行器实例内部通道）：仅租约持有者可续。
///
/// D2 的**对外**续租通道是 [`crate::run_context::renew_lease_by_token`]：本函数
/// 以 `lease_owner` 为凭证且不写审计，不能作为 MCP 入口（见 D2 设计 §4.3）。
/// D2 起补上租约过期与 `running` 态检查——过期租约不得被续活。
///
/// T-032 T3：**绝对执行截止优先于租约续期**——`deadline_at` 已过的续租一律拒绝
/// （平台兜底续租也不例外）；续租不重置/延长 deadline（固化语义）。
/// 平台 poll 兜底续租是**进程存活**信号，不写 `last_agent_heartbeat_at`
/// （心跳 ≠ 进展 ≠ 进程存活，四信号分立）。
pub async fn renew_lease(
    pool: &PgPool,
    run_id: Uuid,
    lease_owner: &str,
    lease_secs: i64,
) -> Result<bool, RunDispatchError> {
    let updated = sqlx::query(
        "UPDATE delivery.task_run \
            SET lease_expires_at = now() + make_interval(secs => $3::float8), heartbeat_at = now() \
          WHERE id = $1 AND lease_owner = $2 AND status IN ('claimed', 'running') \
            AND lease_expires_at > now() \
            AND (deadline_at IS NULL OR deadline_at > now())",
    )
    .bind(run_id)
    .bind(lease_owner)
    .bind(lease_secs)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() == 1)
}

/// fencing（T25 + T-032 T2 终裁）：仅**当前 Run 的当前执行会话、当前执行权世代**
/// 可回传结果。被回收 / 被替代的旧执行实例（过期 session、旧 generation、非活跃
/// Run）一律拒绝——**旧 generation 永远不能重新获得授权**——防止迟到结果污染新 Run。
pub async fn accept_executor_result(
    pool: &PgPool,
    run_id: Uuid,
    executor_session: &str,
    lease_generation: i32,
) -> Result<bool, RunDispatchError> {
    let current: Option<bool> = sqlx::query_scalar(
        "SELECT (status IN ('claimed', 'running') AND executor_session = $2 \
                 AND lease_generation = $3) \
           FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .bind(executor_session)
    .bind(lease_generation)
    .fetch_optional(pool)
    .await?;
    Ok(current == Some(true))
}

/// 写一条 `governance.audit_log`（与业务变更同事务）。
///
/// actor 回退链（G6）：项目 owner human → 任意 human 成员 → 平台 service
/// principal（`ai-pm-dispatch`）。第三级保证「项目无 human 成员」时领取不会
/// 因 `RowNotFound` 失败，同时 `after_data` 强制携带 `client_name` / `lease_owner`
/// 保留真实触发者归因（评审 R-F7）。
pub(crate) async fn audit_run(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    action: &str,
    run_id: Uuid,
    after: serde_json::Value,
) -> Result<(), sqlx::Error> {
    let actor: Option<Uuid> = sqlx::query_scalar(
        "SELECT p.id FROM core.project_member m \
           JOIN core.principal p ON p.id = m.principal_id \
          WHERE m.project_id = $1 AND m.role = 'owner' AND p.kind = 'human' AND p.is_active \
          LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?;
    let actor = match actor {
        Some(id) => id,
        None => sqlx::query_scalar(
            "SELECT p.id FROM core.project_member m \
               JOIN core.principal p ON p.id = m.principal_id \
              WHERE m.project_id = $1 AND p.kind = 'human' AND p.is_active LIMIT 1",
        )
        .bind(project_id)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or(DISPATCH_SERVICE_PRINCIPAL),
    };
    audit_run_as(
        tx,
        project_id,
        &crate::actor::AuditActor::principal(actor),
        action,
        run_id,
        after,
    )
    .await
}

/// 分派器：有显式调用人 → [`audit_run_as`]；没有 → [`audit_run`]（猜人回退）。
/// 供 claim / report_progress 等「MCP 与 worker 共用同一事务模板」的路径使用。
pub(crate) async fn audit_run_opt(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    actor: Option<&crate::actor::AuditActor>,
    action: &str,
    run_id: Uuid,
    after: serde_json::Value,
) -> Result<(), sqlx::Error> {
    match actor {
        Some(a) => audit_run_as(tx, project_id, a, action, run_id, after).await,
        None => audit_run(tx, project_id, action, run_id, after).await,
    }
}

/// 同 [`audit_run`]，但由调用方**显式**给出完整审计身份（T-088）：MCP 路径传入
/// 认证调用人（执行主体 = 项目 `dev_agent`，真人三列 + 设备归因来自 `CallerContext`）。
/// 这是 T-021a「actor 必须由调用方显式传入」在 Run 路径上的落点；[`audit_run`]
/// 的猜人回退只服务于没有调用人的 worker / 后台路径。
pub(crate) async fn audit_run_as(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    actor: &crate::actor::AuditActor,
    action: &str,
    run_id: Uuid,
    after: serde_json::Value,
) -> Result<(), sqlx::Error> {
    let mut after = after;
    let run_meta: Option<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT lease_owner, client_name FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_optional(&mut **tx)
            .await?;
    if let (Some((lease_owner, client_name)), Some(map)) = (run_meta, after.as_object_mut()) {
        map.entry("lease_owner")
            .or_insert(serde_json::json!(lease_owner));
        map.entry("client_name")
            .or_insert(serde_json::json!(client_name));
    }

    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, after_data, source_kind, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'task_run', $8, $9, 'run_dispatch', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(action)
    .bind(run_id)
    .bind(after)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
