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
    let (project_id,): (Uuid,) = sqlx::query_as("SELECT id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await?
        .ok_or(PmWriteError::NotFound("unknown project"))?;
    // PA-06-02/04（2026-11-21 用户裁决，PA06-IMP-001 §6-1）：Agent 在已确认
    // 目标/约束内**授权自主创建 Task**（执行性事项，落 draft 态），故本函数
    // 不再设 require_human——原 E2/R69 人类闸（P1-07-TC-04）只保留在
    // `draft→ready`（E2：AI 永不置 ready）与 REST 人类面（pm_writes
    // create_task_handler）。AI 分解链（discovery_writeback）与人类 REST
    // 通道共用本函数；红线类批量导入（create_tasks 命令）仍走命令通道闸。
    // 历史背景：该闸曾按 PA-07 §11 标 BLOCKED_HUMAN_DECISION 保持
    // fail-closed，裁决后按上位基线 PA-06 放开。
    let title = req.title.as_deref().map(str::trim).unwrap_or_default();
    if title.is_empty() {
        return Err(PmWriteError::Validation("title is required"));
    }
    let priority = req.priority.as_deref().unwrap_or("normal");
    if !TASK_PRIORITIES.contains(&priority) {
        return Err(PmWriteError::Validation("invalid priority"));
    }
    // D-1 遗留①（V2 D-2=A）：Role 版本锚必须指向本任务的目标 project
    // （版本不存在或挂在别的项目 → Validation，不泄露跨项目存在性）。
    if let Some(version_id) = req.ai_role_version_id {
        let owner_project: Option<Uuid> = sqlx::query_scalar(
            "SELECT pr.project_id \
               FROM ai.ai_role_version v \
               JOIN ai.project_ai_role pr ON pr.id = v.project_role_id \
              WHERE v.id = $1",
        )
        .bind(version_id)
        .fetch_optional(pool)
        .await?;
        if owner_project != Some(project_id) {
            return Err(PmWriteError::Validation(
                "ai_role_version_id does not belong to this project",
            ));
        }
    }

    // S03→S04 交接（总体设计 §6.3）：Task 引用基线 id 而非内容——基线冻结后
    // 不受设计/需求后续变化影响（SC-08）。跨项目或已 superseded 的基线拒绝。
    if let Some(baseline_id) = req.implementation_baseline_id {
        let (b_project, b_status): (Uuid, String) = sqlx::query_as(
            "SELECT project_id, status FROM design.implementation_baseline WHERE id = $1",
        )
        .bind(baseline_id)
        .fetch_optional(pool)
        .await?
        .ok_or(PmWriteError::Validation(
            "implementation_baseline_id does not exist",
        ))?;
        if b_project != project_id {
            return Err(PmWriteError::Validation(
                "implementation_baseline_id does not belong to this project",
            ));
        }
        if b_status != "frozen" {
            return Err(PmWriteError::Validation(
                "implementation_baseline_id is not frozen",
            ));
        }
    }

    let mut tx = pool.begin().await?;
    let task_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.task \
         (id, project_id, milestone_id, title, description, status, priority, assignee_id, revision, tenant_id, ai_role_version_id, implementation_baseline_id) \
         VALUES ($1, $2, $3, $4, $5, 'draft', $6, $7, 1, \
                 (SELECT tenant_id FROM core.project WHERE id = $2), $8, $9)",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(req.milestone_id)
    .bind(title)
    .bind(req.description)
    .bind(priority)
    .bind(req.assignee_id)
    .bind(req.ai_role_version_id)
    .bind(req.implementation_baseline_id)
    .execute(&mut *tx)
    .await?;
    // DK-D（§7.2/E4）：验收口径与任务行同事务落库——创建即"可验收"，
    // 不留下"任务已存在但无验收口径"的中间态。
    if let Some(criteria) = req.acceptance_criteria {
        replace_acceptance_criteria(&mut tx, project_id, task_id, criteria).await?;
    }
    // PA07-IMP-002：创建即冻结初始 Revision（revision_no=1），指针同事务落位。
    super::task_revision::snapshot_and_insert(
        &mut tx,
        task_id,
        Some("initial"),
        Some(actor.principal_id),
    )
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "task.create",
        "task",
        Some(task_id),
    )
    .await?;
    // PA-05-01/05（注册表）：创建即注册统一事项 + Timeline `created` 活动
    // （同事务，与审计管线同一致性级别）。
    let work_tenant: Uuid = sqlx::query_scalar(
        "SELECT tenant_id FROM core.project WHERE id = $1",
    )
    .bind(project_id)
    .fetch_one(&mut *tx)
    .await?;
    let work_item_id = crate::work_item::ensure_work_item(
        &mut tx,
        work_tenant,
        "task",
        task_id,
        Some(project_id),
        &title,
        crate::work_item::task_status_semantic("draft"),
        Some(actor.principal_id),
    )
    .await?;
    crate::work_item::record_activity(
        &mut tx,
        work_tenant,
        work_item_id,
        Some(project_id),
        None,
        "created",
        "human",
        Some(actor.principal_id),
        &format!("Task created: {title}"),
        None,
        None,
    )
    .await?;
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
            return Err(PmWriteError::Validation(
                "constraints must be a JSON object",
            ));
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
    // DK-0 §7.4 / E2（2026-11-08）：**AI 永不置 ready**。「任务进入就绪队列」是
    // 人类动作（整批预览一次确认），所以 `draft → ready` 这条边在服务层就要真人
    // ——闸门落在唯一的状态直写入口上（§6.2 四道闸的服务层这道），而不是只写在
    // 编排器白名单里。同状态 no-op（ready → ready）不算迁移，不取闸。
    if new_status == TaskStatus::Ready && new_status != from_status {
        crate::requirement::require_human_conn(&mut tx, actor).await?;
    }
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
    // PA07-IMP-002：实质变更字段（goal / constraints / deliverables / 验收口径）
    // 才触发新 Revision；title/description/knowledge_refs 不属于执行契约。
    let mut substantive: Vec<&str> = Vec::new();
    if let Some(goal) = req.goal {
        sqlx::query("UPDATE delivery.task SET goal = $2, updated_at = now() WHERE id = $1")
            .bind(task_id)
            .bind(&goal)
            .execute(&mut *tx)
            .await?;
        contract_audit.insert("goal".into(), json!(goal));
        substantive.push("goal");
    }
    if let Some(constraints) = req.constraints {
        sqlx::query("UPDATE delivery.task SET constraints = $2, updated_at = now() WHERE id = $1")
            .bind(task_id)
            .bind(&constraints)
            .execute(&mut *tx)
            .await?;
        contract_audit.insert("constraints".into(), constraints);
        substantive.push("constraints");
    }
    if let Some(deliverables) = req.deliverables {
        let normalized = normalize_deliverables(deliverables)?;
        sqlx::query("UPDATE delivery.task SET deliverables = $2, updated_at = now() WHERE id = $1")
            .bind(task_id)
            .bind(json!(normalized))
            .execute(&mut *tx)
            .await?;
        contract_audit.insert("deliverables".into(), json!(normalized));
        substantive.push("deliverables");
    }
    if let Some(criteria) = req.acceptance_criteria {
        let count = replace_acceptance_criteria(&mut tx, project_id, task_id, criteria).await?;
        contract_audit.insert("acceptance_criteria".into(), json!(count));
        substantive.push("acceptance_criteria");
    }
    // PA07-IMP-002：实质变更在主 UPDATE（revision+1）之后冻结新快照，
    // revision_no = 推进后的 CAS 纪元；指针与快照同事务。
    if !substantive.is_empty() {
        let reason = substantive.join(",");
        super::task_revision::snapshot_and_insert(
            &mut tx,
            task_id,
            Some(&reason),
            Some(actor.principal_id),
        )
        .await?;
    }
    if let Some(refs) = req.knowledge_refs {
        let count = replace_knowledge_refs(&mut tx, project_id, task_id, refs).await?;
        contract_audit.insert("knowledge_refs".into(), json!(count));
    }

    append_audit(
        &mut tx,
        project_id,
        actor,
        "task.update",
        "task",
        Some(task_id),
    )
    .await?;
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
    // F-8 / ACP-477（DEF-E2E-1 修复，V-6 架构约束「入口仅适配」）：REST 直写
    // Cancelled 与命令通道 update_task_status 共用同一终态联动内核
    // （task_cancelled_linkage_tx：铩未决介入链 + 任务级 continuation 终态
    // 关闭，V1.1 §5.1 CANCELLED 触发行 / §14）。钩子读到的 Task 已是本事务
    // 写入的终态 → 自动跳过 awaiting_decision 回转（§5.3「或随 Task 终态」）。
    if new_status == TaskStatus::Cancelled && new_status != from_status {
        crate::interaction::task_cancelled_linkage_tx(
            &mut tx,
            project_id,
            task_id,
            "task cancelled",
            actor,
        )
        .await
        .map_err(|e| match e {
            crate::interaction::InteractionError::Db(e) => PmWriteError::Db(e),
            // NotFound（Task 行消失）与 Conflict（continuation 关闭异常，现实
            // 仅包 Db 失败）静态降级——与命令通道降级同形（executor.rs）。
            crate::interaction::InteractionError::NotFound => PmWriteError::NotFound("not found"),
            _other => PmWriteError::Conflict("task cancellation linkage failed"),
        })?;
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
            return Err(PmWriteError::Validation(
                "acceptance criterion text is required",
            ));
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

    sqlx::query(
        "DELETE FROM delivery.task_acceptance_criterion WHERE task_id = $1 AND project_id = $2",
    )
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

/// 需求文档身份解析（ADR-06a：本文件在既有使用方白名单内，文档身份表只此处解析）。
///
/// 文档身份规则 `doc_key = "req-{requirement_id}"`（投影唯一绑定点规则，见
/// `requirement_projection::doc_key`）。文档尚不存在（需求未提交/未投影）
/// → `None`（调用方跳过绑定，投影后回填）。
async fn requirement_document_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    requirement_id: Uuid,
) -> Result<Option<Uuid>, PmWriteError> {
    let document_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM ai.project_document WHERE project_id = $1 AND doc_key = $2",
    )
    .bind(project_id)
    .bind(crate::task_knowledge::requirement_doc_key(requirement_id))
    .fetch_optional(&mut **tx)
    .await
    .map_err(PmWriteError::Db)?;
    Ok(document_id)
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
             (project_id, task_id, document_id, is_required, pinned_revision_no, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, \
                     (SELECT tenant_id FROM core.project WHERE id = $1)) \
             ON CONFLICT (task_id, document_id) DO NOTHING",
        )
        .bind(project_id)
        .bind(task_id)
        .bind(item.document_id)
        .bind(item.is_required)
        .bind(item.pinned_revision_no)
        .execute(&mut **tx)
        .await?;
    }
    Ok(input.len())
}

/// 整批 `draft → ready`（DK-0 §7.4 / D-DK-8 / E5）：**人类动作**。
///
/// 为什么必须存在：`update_task` 的直写**不生产 outbox 事件**，而 `TaskReady`
/// 是入调度的唯一触发（handler 建 Run）。AI 生成的任务全是 draft，人类整批确认
/// 后若走直写，任务永远不进调度——本函数把整批确认接上既有命令通道：
/// 每条任务经 [`crate::command_executor::PgCommandExecutor::execute`]
/// （`UpdateTaskStatus` → `apply_task_transition` + 同事务 `TaskReady`），
/// 因此 E5 的「TaskReady 事件数 = 确认任务数」由结构保证，而不是靠调用方纪律。
///
/// 闸门：`require_human`（E2：AI 调本函数 → `Forbidden`，且调用方无旁路——
/// 直写路径同样有 `draft → ready` 真人闸）。
///
/// 幂等：已是 `ready` 的任务进 `already_ready`（不重复发事件）；非 draft/ready
/// 状态进 `skipped`（执行中的任务不能被"批准确认"回退）。
pub async fn mark_task_batch_ready(
    pool: &PgPool,
    executor: &crate::command_executor::PgCommandExecutor,
    tenant_id: Uuid,
    project_id: Uuid,
    task_ids: &[Uuid],
    actor: &AuditActor,
) -> Result<MarkTasksReadyOutcome, PmWriteError> {
    if task_ids.is_empty() {
        return Err(PmWriteError::Validation("task_ids is required"));
    }
    crate::requirement::require_human(pool, actor).await?;

    // 全部 id 必须属于本项目本租户（越权/未知一律 NotFound，不泄露存在性）。
    let rows: Vec<(Uuid, String, i64)> = sqlx::query_as(
        "SELECT id, status, revision FROM delivery.task \
          WHERE tenant_id = $1 AND project_id = $2 AND id = ANY($3) \
          ORDER BY created_at, id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(task_ids)
    .fetch_all(pool)
    .await?;
    if rows.len() != task_ids.len() {
        return Err(PmWriteError::NotFound("task not found in this project"));
    }

    let mut outcome = MarkTasksReadyOutcome::default();
    for (task_id, status, revision) in rows {
        match status.as_str() {
            "ready" => outcome.already_ready.push(task_id),
            "draft" => {
                executor
                    .execute(
                        pm_domain::ProjectCommand::UpdateTaskStatus {
                            task_id,
                            expected_revision: revision,
                            new_status: pm_domain::TaskStatus::Ready,
                        },
                        crate::command_executor::ExecutionContext {
                            project_id,
                            actor: actor.clone(),
                            actor_kind: crate::command_executor::ActorKind::Human,
                            source: crate::command_executor::CommandSource::HumanAction,
                        },
                    )
                    .await
                    .map_err(|error| match error {
                        pm_domain::CommandError::RevisionConflict { .. } => {
                            PmWriteError::Conflict("revision conflict")
                        }
                        pm_domain::CommandError::Validation(message) => {
                            PmWriteError::Validation(Box::leak(message.into_boxed_str()))
                        }
                        pm_domain::CommandError::Db(error) => PmWriteError::Db(error),
                        other => {
                            PmWriteError::Validation(Box::leak(other.to_string().into_boxed_str()))
                        }
                    })?;
                outcome.ready.push(task_id);
            }
            // 其余 11 态（claimed/running/终态…）不接受整批确认。
            _ => outcome.skipped.push(task_id),
        }
    }
    Ok(outcome)
}

pub(crate) async fn task_row(pool: &PgPool, task_id: Uuid) -> Result<TaskRow, PmWriteError> {
    // 聚合列与列表读面共享（TASK_AGGREGATE_COLUMNS），create/update 返回体与
    // 列表行同形，前端无需二次拉取。
    sqlx::query_as::<_, TaskRow>(&format!(
        "SELECT id, code, project_id, milestone_id, title, description, status, priority, \
                assignee_id, revision, ai_role_version_id, planned_end_at, {}, \
                created_at, updated_at \
         FROM delivery.task t WHERE id = $1",
        crate::rows::TASK_AGGREGATE_COLUMNS
    ))
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::from)
}

/// 任务 → 需求修订的钉定（`delivery.task_requirement`，F15）。
///
/// **E1 的唯一执法点**：`revision_id` 必须指向 `state='approved'` 的修订，且该
/// 修订属于 `requirement_id`、需求与任务同项目。编排器（DK-D 任务分解）与将来
/// 任何"任务挂需求执行依据"的路径都只经这里，不各自直写表（V-6）。
///
/// 幂等：同一 `(task_id, revision_id)` 已存在 → `Ok(false)`（重放 no-op）；
/// 新建 → `Ok(true)`。返回 `Ok` 而非 409 是刻意的——分解批的重放不应中断。
///
/// 注意既有偏差：`task_requirement` 当前 PK 是 `(task_id, revision_id)`，
/// S02 裁决目标是 `(task_id, requirement_id)`；PK 改造属 S02-I4（0039 明文
/// 「本迁移不动」），本批**不实施**，只按目标语义写（同一 Task 对同一
/// Requirement 默认一个执行依据）。
pub async fn link_task_requirement(
    pool: &PgPool,
    task_id: Uuid,
    requirement_id: Uuid,
    revision_id: Uuid,
    actor: &AuditActor,
) -> Result<bool, PmWriteError> {
    let mut tx = pool.begin().await?;
    let task: Option<(Uuid, Uuid)> =
        sqlx::query_as("SELECT project_id, tenant_id FROM delivery.task WHERE id = $1")
            .bind(task_id)
            .fetch_optional(&mut *tx)
            .await?;
    let (project_id, tenant_id) = task.ok_or(PmWriteError::NotFound("task not found"))?;

    // 修订必须存在、属于该需求、属于同一项目，且已定稿（E1 硬约束）。
    let revision: Option<(String, Uuid)> = sqlx::query_as(
        "SELECT rr.state, r.project_id \
           FROM delivery.requirement_revision rr \
           JOIN delivery.requirement r ON r.id = rr.requirement_id \
          WHERE rr.id = $1 AND rr.requirement_id = $2 AND rr.tenant_id = $3",
    )
    .bind(revision_id)
    .bind(requirement_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (state, requirement_project) =
        revision.ok_or(PmWriteError::NotFound("requirement revision not found"))?;
    if requirement_project != project_id {
        return Err(PmWriteError::Validation(
            "requirement belongs to another project",
        ));
    }
    if state != "approved" {
        return Err(PmWriteError::Validation(
            "only an approved requirement revision can be pinned to a task",
        ));
    }

    let inserted = sqlx::query(
        "INSERT INTO delivery.task_requirement \
         (project_id, task_id, requirement_id, revision_id, tenant_id) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (task_id, revision_id) DO NOTHING",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(requirement_id)
    .bind(revision_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Ok(false);
    }

    // S05-02 知识注入（批 3）：任务绑定需求 → 同事务生成该需求的**知识锚点**
    // （候选，is_required=true、未钉版）。人工可用 `update_task` 的
    // `knowledge_refs` 全量替换增删/改 required/钉版。
    // 需求文档尚未投影时此处不绑（投影后由 `requirement_projection` 回填）。
    //
    // ADR-06a：文档身份表是冻结的共享容器——本文件已在既有使用方白名单内，
    // 文档身份的解析只能落在这里（新增文件直接访问会被守卫拒绝）。
    let anchored = match requirement_document_id(&mut tx, project_id, requirement_id).await? {
        Some(document_id) => {
            crate::task_knowledge::bind_document_anchor(&mut tx, project_id, task_id, document_id)
                .await
                .map_err(PmWriteError::Db)?;
            Some(document_id)
        }
        None => None,
    };

    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "task.requirement_linked",
        "task",
        Some(task_id),
        json!({
            "requirement_id": requirement_id,
            "revision_id": revision_id,
            "knowledge_anchor_document_id": anchored,
        }),
    )
    .await?;
    // PA-05-03/04（注册表）：钉版定稿需求 → 统一关联 `implements`
    // （source=task，target=requirement；不改变对象归属）；来源 human +
    // 理由记录（PA-05-04）。双方事项自愈注册（存量兼容）。
    let task_item = crate::work_item::ensure_work_item_for_target(
        &mut tx,
        tenant_id,
        "task",
        task_id,
        Some(project_id),
        crate::work_item::task_status_semantic("draft"),
        Some(actor.principal_id),
    )
    .await?;
    let requirement_item = crate::work_item::ensure_work_item_for_target(
        &mut tx,
        tenant_id,
        "requirement",
        requirement_id,
        Some(project_id),
        crate::work_item::requirement_status_semantic("active"),
        Some(actor.principal_id),
    )
    .await?;
    crate::work_item::add_relation(
        &mut tx,
        tenant_id,
        task_item,
        requirement_item,
        "implements",
        "human",
        Some("pinned approved requirement revision"),
        Some(actor.principal_id),
    )
    .await?;
    tx.commit().await?;
    Ok(true)
}
