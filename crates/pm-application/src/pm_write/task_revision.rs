use super::*;

/// PA07-IMP-002（PA-07 V1.0 §3.1 / 实施规范 §4.2）：Task 执行契约的不可变 Revision。
///
/// `delivery.task.revision` 是 CAS 纪元号，不是 Revision 历史；本模块把「实质变更」
/// （`goal` / `constraints` / 验收口径 / `deliverables` 等执行契约）冻结为
/// `delivery.task_revision` 快照行：
/// - 快照行创建后不得原位更新——本模块（及全库）没有针对该表的 UPDATE 路径，
///   不变性由「无写点 + 测试断言」保证；
/// - `revision_no` = 快照创建时的 `task.revision` 纪元：状态迁移也会推进 CAS
///   （`apply_task_transition` 的 `revision + 1`），但状态不是执行契约，不产生
///   快照行——因此同任务的快照号允许有间隙；
/// - `task.current_task_revision_id` 指针与快照同事务推进；
/// - `delivery.task_run.task_revision_id` 在 Run 创建时固定引用当时指针
///   （`run_dispatch::port::insert_task_run` 等），解释历史执行依据（审查81）。
///
/// 快照列映射：`scope` = `task.constraints`；`acceptance_criteria` =
/// `task_acceptance_criterion` 行集冻结；`execution_contract` = deliverables /
/// task_type / git_branch / milestone_id / ai_role_version_id。title、
/// description、knowledge_refs 不属于执行契约，变更不触发新快照。
///
/// 读侧消费（run_context 优先读 Run 固定的快照而非活任务行）归 PA07-IMP-004。
pub(crate) async fn snapshot_and_insert(
    tx: &mut sqlx::PgConnection,
    task_id: Uuid,
    change_reason: Option<&str>,
    created_by: Option<Uuid>,
) -> Result<Uuid, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct TaskContractRow {
        project_id: Uuid,
        tenant_id: Uuid,
        revision: i64,
        goal: Option<String>,
        scope: serde_json::Value,
        deliverables: serde_json::Value,
        task_type: Option<String>,
        git_branch: Option<String>,
        milestone_id: Option<Uuid>,
        ai_role_version_id: Option<Uuid>,
    }
    let task: TaskContractRow = sqlx::query_as(
        "SELECT project_id, tenant_id, revision, goal, constraints AS scope, \
                deliverables, task_type, git_branch, milestone_id, ai_role_version_id \
           FROM delivery.task WHERE id = $1",
    )
    .bind(task_id)
    .fetch_one(&mut *tx)
    .await?;

    let criteria: Vec<(i32, String, String)> = sqlx::query_as(
        "SELECT ordinal, text, verification_kind \
           FROM delivery.task_acceptance_criterion WHERE task_id = $1 ORDER BY ordinal",
    )
    .bind(task_id)
    .fetch_all(&mut *tx)
    .await?;
    // 单个 jsonb 值（数组整体）而非 jsonb[]：Vec<Value> 直接绑定会被编码为
    // PostgreSQL 数组类型，与 jsonb 列不匹配（42804）。
    let acceptance_criteria = json!(criteria
        .into_iter()
        .map(|(ordinal, text, verification_kind)| {
            json!({ "ordinal": ordinal, "text": text, "verification_kind": verification_kind })
        })
        .collect::<Vec<_>>());
    let execution_contract = json!({
        "deliverables": task.deliverables,
        "task_type": task.task_type,
        "git_branch": task.git_branch,
        "milestone_id": task.milestone_id,
        "ai_role_version_id": task.ai_role_version_id,
    });

    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.task_revision \
         (id, tenant_id, project_id, task_id, revision_no, goal, scope, \
          acceptance_criteria, execution_contract, change_reason, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(revision_id)
    .bind(task.tenant_id)
    .bind(task.project_id)
    .bind(task_id)
    .bind(task.revision)
    .bind(&task.goal)
    .bind(&task.scope)
    .bind(&acceptance_criteria)
    .bind(&execution_contract)
    .bind(change_reason)
    .bind(created_by)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE delivery.task SET current_task_revision_id = $2 WHERE id = $1")
        .bind(task_id)
        .bind(revision_id)
        .execute(&mut *tx)
        .await?;
    Ok(revision_id)
}
