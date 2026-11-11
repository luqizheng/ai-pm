use super::*;

/// §20.7 L3：任务状态迁移的目标状态 → 需异步分发的 outbox 事件。只登记语义无
/// 歧义的迁移边：进入 `ready` 即「任务满足执行条件」（TaskReady），进入
/// `submitted` 即「交付已提交」（DeliverySubmitted）。`RunRequested` /
/// `RunCompleted` / `ReviewCompleted` / `KnowledgeBaseWriteRequested` 的触发动作
/// 是创建 Run、评审结论落库与知识回写请求——这些业务动作目前没有落点，故不
/// 由纯状态列变更产生（§20.6「不假装消费」）。
pub(crate) fn outbox_event_for_task_target(to: &str) -> Option<&'static str> {
    match to {
        "ready" => Some("TaskReady"),
        "submitted" => Some("DeliverySubmitted"),
        _ => None,
    }
}

/// Compensation specs are stored as JSONB and replayed with interpolated
/// identifiers; column names must be plain lowercase SQL identifiers.
pub(crate) fn validate_identifier(name: &str) -> Result<(), CommandError> {
    let first_ok = name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_');
    let rest_ok = name
        .chars()
        .skip(1)
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if first_ok && rest_ok {
        Ok(())
    } else {
        Err(CommandError::Validation(format!(
            "invalid column identifier `{name}` in compensation spec"
        )))
    }
}

/// 单条批量命令的条目上限（spec §十六：建议 ≤200，超出拆批）。
///
/// 导入层（`project_import::proposal`）按此上限把大批草案**拆成多条命令**，
/// 故上限必须是跨模块可见的常量——否则调用方只能各写一份魔数。
pub const MAX_BATCH_ITEMS: usize = 200;

pub(crate) fn validate_batch(len: usize, what: &str) -> Result<(), CommandError> {
    if len == 0 {
        return Err(CommandError::Validation(format!("{what} batch is empty")));
    }
    if len > MAX_BATCH_ITEMS {
        return Err(CommandError::Validation(format!(
            "{what} batch exceeds {MAX_BATCH_ITEMS} items"
        )));
    }
    Ok(())
}

pub(crate) fn db_err(e: sqlx::Error) -> CommandError {
    CommandError::Db(e)
}
