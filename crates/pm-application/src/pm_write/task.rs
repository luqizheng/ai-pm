use super::*;

use pm_domain::TaskStatus;

/// 创建任务：status 固定 draft、revision=1、priority 缺省 normal。
pub async fn create_task(
    pool: &PgPool,
    req: CreateTaskRequest,
    actor: &AuditActor,
) -> Result<TaskRow, PmWriteError> {
    // mock 契约：project 未知（含缺省）→ 404，先于 title 校验。
    let project_id = req
        .project_id
        .ok_or(PmWriteError::NotFound("unknown project"))?;
    let (project_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(pool)
            .await?
            .ok_or(PmWriteError::NotFound("unknown project"))?;
    let title = req.title.as_deref().map(str::trim).unwrap_or_default();
    if title.is_empty() {
        return Err(PmWriteError::Validation("title is required"));
    }
    let priority = req.priority.as_deref().unwrap_or("normal");
    if !TASK_PRIORITIES.contains(&priority) {
        return Err(PmWriteError::Validation("invalid priority"));
    }

    let mut tx = pool.begin().await?;
    let task_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.task \
         (id, project_id, milestone_id, title, description, status, priority, assignee_id, revision, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, 'draft', $6, $7, 1, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(req.milestone_id)
    .bind(title)
    .bind(req.description)
    .bind(priority)
    .bind(req.assignee_id)
    .execute(&mut *tx)
    .await?;
    append_audit(&mut tx, project_id, actor, "task.create", "task", Some(task_id)).await?;
    tx.commit().await?;

    task_row(pool, task_id).await
}


/// 全字段编辑 + revision 乐观锁（不匹配 409），成功后 revision+1。
///
/// D2 起同时承担契约写路径（待裁 #11 已裁决并入 D2）：`goal` / `constraints` /
/// `deliverables` 与 `task_acceptance_criterion` / `task_knowledge_ref` 两表的
/// 增删都在**同一事务**内完成，保证契约读写闭环、C3 不再依赖 SQL 手动种子。
pub async fn update_task(
    pool: &PgPool,
    task_id: Uuid,
    req: UpdateTaskRequest,
    actor: &AuditActor,
) -> Result<TaskRow, PmWriteError> {
    let status = req.status.as_deref().unwrap_or_default();
    if !TASK_STATUSES.contains(&status) {
        return Err(PmWriteError::Validation("invalid status"));
    }
    let new_status =
        TaskStatus::from_str(status).ok_or(PmWriteError::Validation("invalid status"))?;
    let priority = req.priority.as_deref().unwrap_or_default();
    if !TASK_PRIORITIES.contains(&priority) {
        return Err(PmWriteError::Validation("invalid priority"));
    }
    let title = req.title.as_deref().map(str::trim).unwrap_or_default();
    if title.is_empty() {
        return Err(PmWriteError::Validation("title is required"));
    }
    if let Some(constraints) = &req.constraints {
        if !constraints.is_object() {
            return Err(PmWriteError::Validation("constraints must be a JSON object"));
        }
    }
    let mut tx = pool.begin().await?;
    // T-025 §五（C68）：UI 直写**终态**同样是依赖图的写者——与 `add_dependency` /
    // `remove_dependency` / `apply_task_transition` 共用同一把项目级图锁（键由
    // project_id 派生），且取锁必须先于事务内任何行锁，否则「并发建边 vs 直写完成」
    // 仍会漏解锁（P0-2 在同一条路径复发）。非终态直写不取图锁，保持热路径零开销。
    // 预读 project_id 只为拿到锁键；行消失时下面的 `FOR UPDATE` 返回 NotFound。
    if crate::task_dependency::unlocks_downstream(new_status) {
        let graph_key: Uuid =
            sqlx::query_scalar("SELECT project_id FROM delivery.task WHERE id = $1")
                .bind(task_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(PmWriteError::NotFound("not found"))?;
        crate::task_dependency::acquire_project_graph_lock(&mut tx, graph_key).await?;
    }
    // 乐观锁校验必须与 UPDATE 原子：原实现为无锁读 → 两个 PATCH 可同时通过校验。
    let (project_id, current_revision, current_status): (Uuid, i64, String) = sqlx::query_as(
        "SELECT project_id, revision, status FROM delivery.task WHERE id = $1 FOR UPDATE",
    )
    .bind(task_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(PmWriteError::NotFound("not found"))?;
    // mock 契约：revision 缺省/不匹配都是 409 revision conflict。
    if req.revision.unwrap_or(current_revision + 1) != current_revision {
        return Err(PmWriteError::Conflict("revision conflict"));
    }

    // T-025 §10.9 遗留一（P2）：直写状态走与 `apply_task_transition` 同一张合法边表
    // ——UI 不得绕过 13 态状态机（terminal → any、非法回退等）。其余字段照常直写；
    // 状态未变（same → same）是 no-op，不触发校验。UI 路径不带守卫证据，故任何
    // 「边合法但缺证据」的迁移同样在此被拒（走命令通道带证据执行）。
    let from_status = TaskStatus::from_str(&current_status)
        .ok_or(PmWriteError::Validation("invalid current status"))?;
    if new_status != from_status {
        let input = pm_domain::task_transition::TransitionInput {
            task_id: None,
            from: from_status,
            to: new_status,
            waiting: None,
            waiting_resolved: false,
            replacement_task_exists: false,
            active_run_stopped: false,
            retry_exhausted: false,
            deadline_exhausted: false,
            failure: None,
            supersede: None,
            delivery_submitted: false,
            acceptance_evidence_complete: false,
        };
        pm_domain::task_transition::validate_transition(&input).map_err(|_| {
            PmWriteError::Validation(
                "status transition rejected by the state machine (use the command channel with guard evidence)",
            )
        })?;
    }

    sqlx::query(
        "UPDATE delivery.task \
         SET title = $2, description = $3, status = $4, priority = $5, assignee_id = $6, \
             revision = revision + 1, updated_at = now() \
         WHERE id = $1",
    )
    .bind(task_id)
    .bind(title)
    .bind(req.description)
    .bind(status)
    .bind(priority)
    .bind(req.assignee_id)
    .execute(&mut *tx)
    .await?;

    let mut contract_audit = serde_json::Map::new();
    if let Some(goal) = req.goal {
        sqlx::query("UPDATE delivery.task SET goal = $2, updated_at = now() WHERE id = $1")
            .bind(task_id)
            .bind(&goal)
            .execute(&mut *tx)
            .await?;
        contract_audit.insert("goal".into(), json!(goal));
    }
    if let Some(constraints) = req.constraints {
        sqlx::query("UPDATE delivery.task SET constraints = $2, updated_at = now() WHERE id = $1")
            .bind(task_id)
            .bind(&constraints)
            .execute(&mut *tx)
            .await?;
        contract_audit.insert("constraints".into(), constraints);
    }
    if let Some(deliverables) = req.deliverables {
        let normalized = normalize_deliverables(deliverables)?;
        sqlx::query("UPDATE delivery.task SET deliverables = $2, updated_at = now() WHERE id = $1")
            .bind(task_id)
            .bind(json!(normalized))
            .execute(&mut *tx)
            .await?;
        contract_audit.insert("deliverables".into(), json!(normalized));
    }
    if let Some(criteria) = req.acceptance_criteria {
        let count = replace_acceptance_criteria(&mut tx, project_id, task_id, criteria).await?;
        contract_audit.insert("acceptance_criteria".into(), json!(count));
    }
    if let Some(refs) = req.knowledge_refs {
        let count = replace_knowledge_refs(&mut tx, project_id, task_id, refs).await?;
        contract_audit.insert("knowledge_refs".into(), json!(count));
    }

    append_audit(&mut tx, project_id, actor, "task.update", "task", Some(task_id)).await?;
    if !contract_audit.is_empty() {
        append_audit_with_after(
            &mut tx,
            project_id,
            actor,
            "task.update_contract",
            "task",
            Some(task_id),
            json!({ "contract": contract_audit }),
        )
        .await?;
    }
    // T-024 D4 / T-025 §五：UI 直写终态的路径同样触发下游解锁（与命令通道共用同一
    // 解锁函数与同一终态集合定义）。
    if crate::task_dependency::unlocks_downstream(new_status) {
        crate::task_dependency::unlock_downstream(&mut tx, project_id, task_id).await?;
    }
    tx.commit().await?;

    task_row(pool, task_id).await
}


pub(crate) fn normalize_deliverables(
    input: Vec<DeliverableInput>,
) -> Result<Vec<serde_json::Value>, PmWriteError> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(input.len());
    for item in input {
        let path = item.path.trim().to_string();
        if path.is_empty() {
            return Err(PmWriteError::Validation("deliverable path is required"));
        }
        if !seen.insert(path.clone()) {
            return Err(PmWriteError::Validation("duplicate deliverable path"));
        }
        let id = item
            .id
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        out.push(json!({ "id": id, "path": path }));
    }
    Ok(out)
}


/// 全量替换验收项；`ordinal` 缺省按 `max+10` 追加（允许间隙，不重排既有条目）。
pub(crate) async fn replace_acceptance_criteria(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    input: Vec<AcceptanceCriterionInput>,
) -> Result<usize, PmWriteError> {
    struct Normalized {
        ordinal: i32,
        text: String,
        kind: String,
    }
    let mut normalized: Vec<Normalized> = Vec::with_capacity(input.len());
    let mut next = 10i32;
    for item in input {
        let text = item.text.trim().to_string();
        if text.is_empty() {
            return Err(PmWriteError::Validation("acceptance criterion text is required"));
        }
        let kind = item
            .verification_kind
            .unwrap_or_else(|| "manual".to_string());
        if !VERIFICATION_KINDS.contains(&kind.as_str()) {
            return Err(PmWriteError::Validation("invalid verification_kind"));
        }
        let ordinal = match item.ordinal {
            Some(ordinal) if ordinal >= 1 => ordinal,
            Some(_) => return Err(PmWriteError::Validation("ordinal must be >= 1")),
            None => next,
        };
        if normalized.iter().any(|entry| entry.ordinal == ordinal) {
            return Err(PmWriteError::Validation("duplicate acceptance ordinal"));
        }
        next = ordinal.max(next) + 10;
        normalized.push(Normalized {
            ordinal,
            text,
            kind,
        });
    }

    sqlx::query("DELETE FROM delivery.task_acceptance_criterion WHERE task_id = $1 AND project_id = $2")
        .bind(task_id)
        .bind(project_id)
        .execute(&mut **tx)
        .await?;
    for entry in &normalized {
        sqlx::query(
            "INSERT INTO delivery.task_acceptance_criterion \
             (project_id, task_id, ordinal, text, verification_kind, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, \
                     (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(project_id)
        .bind(task_id)
        .bind(entry.ordinal)
        .bind(&entry.text)
        .bind(&entry.kind)
        .execute(&mut **tx)
        .await?;
    }
    Ok(normalized.len())
}


/// 全量替换知识引用；文档必须属于同一项目（跨项目一律拒绝）。
pub(crate) async fn replace_knowledge_refs(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    input: Vec<KnowledgeRefInput>,
) -> Result<usize, PmWriteError> {
    let mut seen = std::collections::HashSet::new();
    for item in &input {
        if !seen.insert(item.document_id) {
            return Err(PmWriteError::Validation("duplicate knowledge document_id"));
        }
    }
    let ids: Vec<Uuid> = input.iter().map(|item| item.document_id).collect();
    let found: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.project_document WHERE project_id = $1 AND id = ANY($2)",
    )
    .bind(project_id)
    .bind(&ids)
    .fetch_one(&mut **tx)
    .await?;
    if found as usize != ids.len() {
        return Err(PmWriteError::Validation(
            "knowledge_refs contain a document outside this project",
        ));
    }

    sqlx::query("DELETE FROM delivery.task_knowledge_ref WHERE task_id = $1 AND project_id = $2")
        .bind(task_id)
        .bind(project_id)
        .execute(&mut **tx)
        .await?;
    for item in &input {
        sqlx::query(
            "INSERT INTO delivery.task_knowledge_ref \
             (project_id, task_id, document_id, is_required, tenant_id) \
             VALUES ($1, $2, $3, $4, \
                     (SELECT tenant_id FROM core.project WHERE id = $1)) \
             ON CONFLICT (task_id, document_id) DO NOTHING",
        )
        .bind(project_id)
        .bind(task_id)
        .bind(item.document_id)
        .bind(item.is_required)
        .execute(&mut **tx)
        .await?;
    }
    Ok(input.len())
}


pub(crate) async fn task_row(pool: &PgPool, task_id: Uuid) -> Result<TaskRow, PmWriteError> {
    sqlx::query_as::<_, TaskRow>(
        "SELECT id, project_id, milestone_id, title, description, status, priority, \
                assignee_id, revision, created_at, updated_at \
         FROM delivery.task WHERE id = $1",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::from)
}


