use super::*;

/// R1：Task 状态变更经**统一受守卫服务**（`pm-domain::validate_transition` +
/// 行锁 + 审计）。`run_dispatch` 与既有 `update_task_status` 都经由同一条通道，
/// 因此不存在「第二条绕过状态机的写入路径」。`_from` 仅作调用点自述——实际前态
/// 由服务在行锁内从数据库读取并校验。
pub async fn apply_transition(
    tx: &mut Transaction<'static, Postgres>,
    task_id: Uuid,
    _from: TaskStatus,
    to: TaskStatus,
    ctx: &ExecutionContext,
) -> Result<(), RunDispatchError> {
    apply_task_transition(tx, task_id, None, to, &TransitionFacts::for_dispatch(), ctx)
        .await
        .map_err(|e| RunDispatchError::Transition(e.to_string()))?;
    Ok(())
}
