use super::*;

/// 一次状态迁移必须供给的外部事实（§7.3：域层无 I/O，事实由调用方提供）。
#[derive(Debug, Clone, Copy, Default)]
pub struct TransitionFacts {
    pub waiting_resolved: bool,
    pub replacement_task_exists: bool,
    pub active_run_stopped: bool,
    pub retry_exhausted: bool,
    pub deadline_exhausted: bool,
    pub delivery_submitted: bool,
    pub acceptance_evidence_complete: bool,
}

impl TransitionFacts {
    /// 既有命令通道的事实：交付证据（Running→Submitted / Reviewing→Completed）
    /// 的完整校验属 D4；此处保持既有行为不变，其余事实保持默认 false。
    pub fn for_command_channel() -> Self {
        Self {
            delivery_submitted: true,
            acceptance_evidence_complete: true,
            ..Self::default()
        }
    }

    /// 调度/回收路径的事实：全部为默认（`ready↔claimed` 等 guard-free 边）。
    pub fn for_dispatch() -> Self {
        Self::default()
    }
}

/// **Task 状态变更的唯一受守卫入口（R1）**：行锁 → revision guard →
/// `pm-domain::validate_transition` → 状态推进 → 审计。`update_task_status`
/// 命令通道与 `run_dispatch` 领取/回收都复用本函数，因此不存在第二条绕过
/// 状态机的写入路径。调用方负责事务边界与（如适用）outbox 事件登记。
pub(crate) async fn apply_task_transition(
    tx: &mut Tx,
    task_id: Uuid,
    expected_revision: Option<i64>,
    to: TaskStatus,
    facts: &TransitionFacts,
    ctx: &ExecutionContext,
) -> Result<(AuditId, i64), CommandError> {
    // T-025 P0（评审 §2.2）：终态迁移与依赖图写操作（建边/删边）共用项目级图锁，
    // 且取锁必须先于下方 FOR UPDATE——「上游完成」与「并发建边」可证明串行：
    // 后到者必然看到先到者已提交的边或状态，漏解锁与漏检环都不可能。
    // 非终态迁移（ready/claimed/running 往返）不取锁，调度热路径零开销；
    // 解锁路径（blocked→ready）运行在建边/终态事务已持有的图锁内（xact lock
    // 可重入，且 Ready 非终态本就不会再取）。取锁点先于任何行锁 → 无死锁。
    if crate::task_dependency::unlocks_downstream(to) {
        crate::task_dependency::acquire_project_graph_lock(tx, ctx.project_id)
            .await
            .map_err(db_err)?;
    }
    let row = sqlx::query(
        "SELECT status, revision FROM delivery.task WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(task_id)
    .bind(ctx.project_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_err)?;
    let row = row.ok_or_else(|| {
        CommandError::Validation(format!(
            "task {task_id} not found in project {}",
            ctx.project_id
        ))
    })?;
    let status_before: String = row.get("status");
    let revision: i64 = row.get("revision");
    if let Some(expected) = expected_revision {
        if revision != expected {
            return Err(CommandError::RevisionConflict {
                expected,
                actual: revision,
            });
        }
    }
    let from = TaskStatus::from_str(&status_before).ok_or_else(|| {
        CommandError::Validation(format!("unknown task status `{status_before}`"))
    })?;
    let input = TransitionInput {
        task_id: Some(task_id),
        from,
        to,
        waiting: None,
        waiting_resolved: facts.waiting_resolved,
        replacement_task_exists: facts.replacement_task_exists,
        active_run_stopped: facts.active_run_stopped,
        retry_exhausted: facts.retry_exhausted,
        deadline_exhausted: facts.deadline_exhausted,
        failure: None,
        supersede: None,
        delivery_submitted: facts.delivery_submitted,
        acceptance_evidence_complete: facts.acceptance_evidence_complete,
    };
    validate_transition(&input).map_err(|e| {
        CommandError::Validation(format!(
            "state machine refused {} -> {}: {e}",
            status_before,
            to.as_str()
        ))
    })?;
    // E2 / E5（PA-06-02，PA07-IMP-002 §4 P1，2026-11-21 审计）：「AI 永不置
    // ready」在本入口单点执法——迁入 Ready 仅允许人类主体（REST 直写另有
    // require_human；E5 整批确认与红线批准以 Human 经本入口执行）与平台
    // 调度解锁路径（ActorKind::Service，上游完成解锁下游 D7/D8）。AI 主体
    // （AiPm/DevAgent）提案 UpdateTaskStatus{to: Ready} 此前可经
    // self_review=Auto 以 AiPm 执行，绕过 E5 整批人类确认——审计实测
    // （TransitionFacts::for_command_channel 无 actor 门 + (Draft, Ready)
    // 是 pm_domain 无守卫合法边）。拦截于状态推进之前，任务保持原状态。
    if to == TaskStatus::Ready
        && ctx.actor_kind != ActorKind::Human
        && ctx.actor_kind != ActorKind::Service
    {
        return Err(CommandError::Validation(format!(
            "E2: moving a task into ready requires a human principal (actor_kind={:?}); \
             AI proposals for ready must go through human confirmation",
            ctx.actor_kind
        )));
    }
    // PA07-IMP-005（PA-07 §13，审查89-90）：迁入 Completed 前检查 Product 级
    // 验收完整性——每个仍有效关联 Product 的最新验收必须是 approved；撤回
    // Product 不阻塞（历史保留）。这是单点执法：review complete_task / D2
    // 命令通道 / MCP 全部 completed 迁移都经过本入口。
    if to == TaskStatus::Completed {
        crate::task_products::acceptance_gate(&mut **tx, task_id)
            .await
            .map_err(db_err)?
            .map_err(|unsatisfied| {
                CommandError::Validation(format!(
                    "product acceptance incomplete: {:?}",
                    unsatisfied
                ))
            })?;
    }
    sqlx::query(
        "UPDATE delivery.task SET status = $1, revision = revision + 1, updated_at = now() WHERE id = $2",
    )
    .bind(to.as_str())
    .bind(task_id)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    let resulting_revision = revision + 1;
    let before = json!({ "status": status_before, "revision": revision });
    let after = json!({ "status": to.as_str(), "revision": resulting_revision });
    let compensation = CompensationSpec::Restore(vec![SnapshotEntry {
        table: CompensableTable::Task,
        target_id: task_id,
        fields: json!({ "status": status_before, "revision": resulting_revision }),
    }]);
    let audit_id = insert_audit(
        tx,
        ctx,
        "update_task_status",
        "task",
        Some(task_id),
        &before,
        &after,
        Some(&compensation),
        audit_source(ctx),
    )
    .await?;
    // PA-05-02/05/07：状态迁移接线（统一事项自愈注册 + 统一状态同步 +
    // 语义变化记 Timeline `status_changed`；语义未变不记 = §8.3 合并规则）。
    // 来源区分（PA-05-05）：Human=human，其余（ai_pm/dev_agent/service）=agent。
    let work_actor_type = match ctx.actor_kind {
        ActorKind::Human => "human",
        ActorKind::AiPm | ActorKind::DevAgent | ActorKind::Service => "agent",
    };
    let work_tenant = crate::work_item::project_tenant(tx, ctx.project_id)
        .await
        .map_err(|err| match err {
            crate::work_item::WorkItemError::Db(e) => CommandError::Db(e),
            crate::work_item::WorkItemError::NotFound => CommandError::Validation(format!(
                "project {} not found for work item wiring",
                ctx.project_id
            )),
            crate::work_item::WorkItemError::Validation(m) => {
                CommandError::Validation(m.to_string())
            }
        })?;
    crate::work_item::on_task_transition(
        tx,
        work_tenant,
        ctx.project_id,
        task_id,
        &status_before,
        to.as_str(),
        work_actor_type,
        Some(ctx.actor.principal_id),
    )
    .await
    .map_err(|err| match err {
        crate::work_item::WorkItemError::Db(e) => CommandError::Db(e),
        crate::work_item::WorkItemError::NotFound => {
            CommandError::Validation(format!("task {task_id} work item target not found"))
        }
        crate::work_item::WorkItemError::Validation(m) => CommandError::Validation(m.to_string()),
    })?;
    // D4（T-024）：上游进入终态时，在**同一事务**里解锁满足依赖的 blocked 下游。
    // `unlock_downstream` 会反过来调用本函数（`blocked → ready`），因此必须用
    // `Box::pin` 打断 async 递归的类型尺寸计算，否则无法求出 future 大小。
    if crate::task_dependency::unlocks_downstream(to) {
        Box::pin(crate::task_dependency::unlock_downstream(
            tx,
            ctx.project_id,
            task_id,
        ))
        .await
        .map_err(db_err)?;
    }
    Ok((audit_id, resulting_revision))
}

/// 解析平台内部动作（调度/回收）的执行身份：固定 `ai-pm-dispatch` service
/// principal（`0015` 幂等种子）。平台动作没有认证身份，审计三列为 NULL。
pub(crate) async fn system_context(
    _tx: &mut Tx,
    project_id: ProjectId,
) -> Result<ExecutionContext, CommandError> {
    Ok(ExecutionContext {
        project_id,
        actor: crate::actor::AuditActor::dispatch(),
        actor_kind: ActorKind::Service,
        source: CommandSource::System,
    })
}
