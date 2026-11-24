//! Task 前置依赖与解锁（T-024 / `docs/tasks/ai-pm-task-dependency-v1.md`）。
//!
//! 载体是独立边表 `delivery.task_dependency`：`task_id` 是下游（依赖方），
//! `depends_on_task_id` 是上游（被依赖方）。四项已裁定事项（D1–D4）与三项本稿
//! 决定（D5–D8）见设计稿 §2；本模块只实现，不重新裁量。
//!
//! 三个硬不变量：
//! 1. **依赖满足只有一套谓词**（[`dependencies_satisfied`]）：调度领取、建边/删边后
//!    的解锁、终态触发的级联解锁共用它，禁止各写一套。
//! 2. **状态变更仍走唯一受守卫入口**（`apply_task_transition`）：解锁把
//!    `blocked → ready` 也交给它校验 + 审计，不旁路更新状态。
//! 3. **事件与状态同事务**：解锁产生的 `TaskReady` 与迁移在同一个事务里写入，
//!    幂等键沿用命令通道约定 `task:<id>:TaskReady:rev<revision>`。

use serde::Serialize;
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use pm_domain::TaskStatus;

use crate::actor::AuditActor;
use crate::command_executor::{apply_task_transition, system_context, TransitionFacts, Tx};
use crate::outbox::{event_is_enabled, OutboxAppend, OutboxEvent, PgOutboxAppend};

/// PA07-IMP-003（审查75）：依赖边满足条件的封闭三值。
///
/// 语义变更说明：原 D5「终态集合 = completed/cancelled/superseded 即满足」已被
/// PA-07 §9.5 取代——cancelled/superseded 前置**不**满足任何条件，后继保持阻塞
/// 等待 AI-PM 恢复或调整依赖；满足判定改为按边条件逐条求值
/// （[`dependencies_satisfied`]）。
pub const DEPENDENCY_CONDITIONS: [&str; 3] = ["completed", "accepted", "deliverable_approved"];

/// 一次解锁迁移允许的下游状态：只有 `blocked` 需要被解锁；`ready` 及之后的状态
/// 已有自己的事件与 Run，重复投递只会制造第二个语义键（设计稿 §5.3）。
///
/// cancelled/superseded 仍在触发集合：终态变化后重新求值是「重评估」不是「放行」
/// ——条件谓词不会因前置取消而满足，重跑只会得到更阻塞的正确结论。
pub fn unlocks_downstream(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Completed | TaskStatus::Cancelled | TaskStatus::Superseded
    )
}

/// 依赖图里的一个端点（GET 响应元素）。
#[derive(Debug, Clone, Serialize)]
pub struct DependencyEdge {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    /// PA07-IMP-003：该边的满足条件（读面透出，审查75）。
    pub required_condition: String,
    /// deliverable_approved 边的交付物引用（其余条件为 null）。
    pub required_deliverable_ref: Option<serde_json::Value>,
    /// 该边条件当前是否满足（PA-07 UI「✓ Satisfied / ○ Not satisfied · x/y met」
    /// 数据源）：行级求值，谓词与 dependencies_satisfied 同源。
    pub satisfied: bool,
}

/// `GET /v1/pm/tasks/{task_id}/dependencies` 的响应体。
#[derive(Debug, Clone, Serialize)]
pub struct DependencyGraph {
    pub task_id: Uuid,
    /// 本任务依赖的前置（`depends_on_task_id`）。
    pub upstream: Vec<DependencyEdge>,
    /// 依赖本任务的下游（`task_id`）。
    pub downstream: Vec<DependencyEdge>,
}

#[derive(Debug)]
pub enum DependencyError {
    /// 任务不存在（或不属于调用方可见范围）。
    TaskNotFound,
    /// `task_id == depends_on_task_id`。
    SelfDependency,
    /// 下游状态不在 `draft / blocked`（`Ready → Blocked` 不是合法边，D6）。
    TaskNotPending(String),
    /// 同一对端点已有边。
    Duplicate,
    /// 两端不同项目（D3）；库层两条复合外键是第二道保险。
    CrossProject,
    /// 新上游沿现有边能回到下游。
    Cycle,
    /// PA07-IMP-003：条件非法（不在三值枚举、deliverable_approved 缺引用、
    /// 引用缺 id/path 键）。
    InvalidCondition(String),
    Db(sqlx::Error),
}

impl From<sqlx::Error> for DependencyError {
    fn from(e: sqlx::Error) -> Self {
        DependencyError::Db(e)
    }
}

/// 项目级依赖图写锁（T-025 P0，评审 §2.1/§2.2）：同一项目的建边、删边与终态
/// 解锁传播共用这把事务级 advisory lock，使「检环 → 插边」与「终态 → 扫下游」
/// 形成可证明的串行关系——后到的事务必然看到先到者已提交的图/状态，漏检环
/// 与漏解锁都不可能。
///
/// 硬规则：取锁必须发生在事务内**任何行锁之前**（否则出现「持行锁等图锁」与
/// 「持图锁等行锁」互相等待）。`pg_advisory_xact_lock` 同事务可重入，嵌套调用
/// （解锁路径再走受守卫入口）安全；事务结束自动释放。跨项目互不阻塞。
pub(crate) async fn acquire_project_graph_lock(
    tx: &mut Tx,
    project_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('task-graph:' || $1::text, 0))")
        // DEF-P0-05：键必须以 **text** 绑定，且 SQL 文本带独立前缀。
        // ① `bind(Uuid)` + `$1::text`：服务端把参数解析为 text，而 sqlx 按 uuid
        //    编成 16 字节 → 服务端当 text 解析 → 随机 `invalid byte sequence for
        //    encoding "UTF8"`（任务完成/审核裁决 500，flaky）。
        // ② SQL 文本与知识索引器的锁语句**完全相同**时，sqlx 的语句缓存按 SQL
        //    文本复用：同一连接先按 text 绑定准备过、后续按 uuid 绑定复用缓存
        //    语句 → 触发 ①。前缀让两条语句的缓存键不再相撞。
        .bind(project_id.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 依赖满足的唯一判定（PA07-IMP-003，审查75/81）：按边条件逐条求值，全部
/// 满足才放行。cancelled/superseded 前置不满足任何条件（PA-07 §9.5）。
///
/// - `completed`：前置 status = completed；
/// - `accepted`：前置 completed 且存在获批审批行（governance.review_approval
///   status='approved'，人工批准是验收事实源）；
/// - `deliverable_approved`：accepted 且引用的交付物（id/path）包含于前置
///   **当前 Revision 冻结**的 execution_contract.deliverables——不隐式跟随
///   最新版本（审查81）。
pub(crate) async fn dependencies_satisfied(
    tx: &mut Tx,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT NOT EXISTS ( \
             SELECT 1 FROM delivery.task_dependency d \
               JOIN delivery.task u ON u.project_id = d.project_id AND u.id = d.depends_on_task_id \
              WHERE d.project_id = $1 AND d.task_id = $2 \
                AND NOT ( \
                     (d.required_condition = 'completed' AND u.status = 'completed') \
                  OR (d.required_condition = 'accepted' \
                      AND u.status = 'completed' \
                      AND EXISTS (SELECT 1 FROM governance.review_approval ra \
                                   WHERE ra.task_id = u.id AND ra.status = 'approved')) \
                  OR (d.required_condition = 'deliverable_approved' \
                      AND u.status = 'completed' \
                      AND EXISTS (SELECT 1 FROM governance.review_approval ra \
                                   WHERE ra.task_id = u.id AND ra.status = 'approved') \
                      AND EXISTS (SELECT 1 FROM delivery.task_revision tr \
                                   WHERE tr.id = u.current_task_revision_id \
                                     AND (tr.execution_contract->'deliverables') \
                                         @> jsonb_build_array(d.required_deliverable_ref)))))",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await
}

/// T-025 §三（评审 §3.1 / C61）：任务是否仍有**非依赖类**阻塞未解除。
///
/// `delivery.task_waiting.reason` 的取值（`0010_task_state_machine.sql`）只有
/// `needs_info / needs_decision / needs_approval / external_dependency`——**没有依赖类
/// 原因**（T-023 F2：任务间关系只存在于边表）。因此「存在一条活跃 waiting 行」恰好
/// 等价于「依赖之外的阻塞源仍在生效」，依赖满足不得据此把任务迁到 `ready`。
///
/// 选型：v1 取最小谓词，不引入 Blocker 聚合视图（评审 §3.1 方案 B 登记为 P2，见
/// 任务书 §10.6）。**对称义务**：任何解除等待的写入路径都必须重新触发一次解锁
/// 判定，否则任务会停在 `blocked`（当前仓库尚无 waiting 写入口）。
pub(crate) async fn has_active_waiting(
    tx: &mut Tx,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM delivery.task_waiting \
                          WHERE project_id = $1 AND task_id = $2 AND ended_at IS NULL)",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await
}

/// 任务的前置/后置关系（GET）。`tenant_id` 供应用层纵深防御（T-025 §10.9 遗留二）：
/// HTTP 层已做 T-022 可见性，这里再把查询锚定在租户内——即使上层校验被绕过，
/// 跨租户 task_id 也读不出任何边。
pub async fn list_dependencies(
    pool: &PgPool,
    tenant_id: Uuid,
    task_id: Uuid,
) -> Result<DependencyGraph, DependencyError> {
    let _ = sqlx::query("SELECT 1 FROM delivery.task WHERE id = $1 AND tenant_id = $2")
        .bind(task_id)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await?
        .ok_or(DependencyError::TaskNotFound)?;

    // 边表行与任务同租户（tenant_id NOT NULL + 组合外键，I1/I5）；边查询同样
    // 按 tenant 锚定，保证返回的相邻任务都属于本租户。
    // PA07-IMP-003：读面透出边条件与交付物引用（审查75/81）；satisfied 行级求值
    // 与 dependencies_satisfied 三谓词同源（PA-07 UI 依赖页「x/y met」数据源）。
    // satisfied 是**边的属性**，恒对前置任务（depends_on）求值——Upstream 行前置
    // 即对端 t；Blocks 行前置是本任务，需另 join 前置别名 p 求值（对端 t 只做展示）。
    let satisfied_pred = |pre: &str| {
        format!(
            "((d.required_condition = 'completed' AND {pre}.status = 'completed') \
           OR (d.required_condition = 'accepted' AND {pre}.status = 'completed' \
               AND EXISTS (SELECT 1 FROM governance.review_approval ra \
                            WHERE ra.task_id = {pre}.id AND ra.status = 'approved')) \
           OR (d.required_condition = 'deliverable_approved' AND {pre}.status = 'completed' \
               AND EXISTS (SELECT 1 FROM governance.review_approval ra \
                            WHERE ra.task_id = {pre}.id AND ra.status = 'approved') \
               AND EXISTS (SELECT 1 FROM delivery.task_revision tr \
                            WHERE tr.id = {pre}.current_task_revision_id \
                              AND (tr.execution_contract->'deliverables') \
                                  @> jsonb_build_array(d.required_deliverable_ref)))) AS satisfied"
        )
    };
    let upstream = sqlx::query(&format!(
        "SELECT t.id, t.title, t.status, d.required_condition, d.required_deliverable_ref, {pred} \
           FROM delivery.task_dependency d \
           JOIN delivery.task t ON t.project_id = d.project_id AND t.id = d.depends_on_task_id \
          WHERE d.task_id = $1 AND d.tenant_id = $2 \
          ORDER BY t.created_at, t.id",
        pred = satisfied_pred("t"),
    ))
    .bind(task_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let downstream = sqlx::query(&format!(
        "SELECT t.id, t.title, t.status, d.required_condition, d.required_deliverable_ref, {pred} \
           FROM delivery.task_dependency d \
           JOIN delivery.task t ON t.project_id = d.project_id AND t.id = d.task_id \
           JOIN delivery.task p ON p.project_id = d.project_id AND p.id = d.depends_on_task_id \
          WHERE d.depends_on_task_id = $1 AND d.tenant_id = $2 \
          ORDER BY t.created_at, t.id",
        pred = satisfied_pred("p"),
    ))
    .bind(task_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;

    Ok(DependencyGraph {
        task_id,
        upstream: upstream.iter().map(edge_of).collect(),
        downstream: downstream.iter().map(edge_of).collect(),
    })
}

fn edge_of(row: &sqlx::postgres::PgRow) -> DependencyEdge {
    DependencyEdge {
        id: row.get("id"),
        title: row.get("title"),
        status: row.get("status"),
        required_condition: row.get("required_condition"),
        required_deliverable_ref: row.get("required_deliverable_ref"),
        satisfied: row.get("satisfied"),
    }
}

/// 建边选项（PA07-IMP-003）：`None` 条件 = 默认 `completed`（与既有行为一致）。
#[derive(Debug, Default)]
pub struct AddDependencyOptions {
    pub required_condition: Option<String>,
    pub required_deliverable_ref: Option<serde_json::Value>,
}

fn validate_condition(
    options: &AddDependencyOptions,
) -> Result<(String, Option<serde_json::Value>), DependencyError> {
    let condition = options
        .required_condition
        .clone()
        .unwrap_or_else(|| "completed".to_string());
    if !DEPENDENCY_CONDITIONS.contains(&condition.as_str()) {
        return Err(DependencyError::InvalidCondition(format!(
            "required_condition must be one of {DEPENDENCY_CONDITIONS:?}"
        )));
    }
    let deliverable_ref = options.required_deliverable_ref.clone();
    if deliverable_ref.is_some() && condition == "completed" {
        return Err(DependencyError::InvalidCondition(
            "required_deliverable_ref only applies to deliverable_approved edges".to_string(),
        ));
    }
    if condition == "deliverable_approved" {
        let Some(reference) = &deliverable_ref else {
            return Err(DependencyError::InvalidCondition(
                "deliverable_approved requires required_deliverable_ref".to_string(),
            ));
        };
        let has_key = reference.get("id").is_some() || reference.get("path").is_some();
        if !has_key {
            return Err(DependencyError::InvalidCondition(
                "required_deliverable_ref must contain id or path".to_string(),
            ));
        }
    }
    Ok((condition, deliverable_ref))
}

/// 建边（POST）：同一事务内完成校验 → 插边 → 审计 → 解锁判定（D7）。
pub async fn add_dependency(
    pool: &PgPool,
    task_id: Uuid,
    depends_on_task_id: Uuid,
    actor: &AuditActor,
) -> Result<DependencyGraph, DependencyError> {
    add_dependency_with(
        pool,
        task_id,
        depends_on_task_id,
        AddDependencyOptions::default(),
        actor,
    )
    .await
}

/// 建边（POST，带条件选项）：同一事务内完成校验 → 插边 → 审计 → 解锁判定（D7；
/// PA07-IMP-003 增加条件/交付物引用校验与落库）。
pub async fn add_dependency_with(
    pool: &PgPool,
    task_id: Uuid,
    depends_on_task_id: Uuid,
    options: AddDependencyOptions,
    actor: &AuditActor,
) -> Result<DependencyGraph, DependencyError> {
    // 条件与交付物引用的封闭校验先于任何锁（纯输入校验）。
    let (required_condition, required_deliverable_ref) = validate_condition(&options)?;
    let mut tx = pool.begin().await?;

    // ⓪ 无锁预读项目（图锁键）：取锁必须先于任何行锁（T-025 P0）。预读后行
    //    若消失，① 的 FOR UPDATE 返回 None → TaskNotFound。本项目内的并发
    //    建边/删边/终态迁移由此全序化（设计稿评审 §2.1：并发反向边恰一成功）。
    let graph_key: Uuid = sqlx::query_scalar("SELECT project_id FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DependencyError::TaskNotFound)?;
    acquire_project_graph_lock(&mut tx, graph_key).await?;

    // ① 锁下游任务行（项目图锁已持有：检环/插边/解锁判定在本项目内串行）。
    //    T-025 §六（C66）：tenant_id 一并取回，供 ② 做跨租户判定（I3）。
    let downstream = sqlx::query(
        "SELECT project_id, tenant_id, status FROM delivery.task WHERE id = $1 FOR UPDATE",
    )
    .bind(task_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(DependencyError::TaskNotFound)?;
    let project_id: Uuid = downstream.get("project_id");
    let tenant_id: Uuid = downstream.get("tenant_id");
    let status: String = downstream.get("status");

    // ② 上游必须存在且同项目（D3）。
    let upstream = sqlx::query("SELECT project_id, tenant_id FROM delivery.task WHERE id = $1")
        .bind(depends_on_task_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DependencyError::TaskNotFound)?;
    let upstream_project: Uuid = upstream.get("project_id");
    let upstream_tenant: Uuid = upstream.get("tenant_id");
    // T-025 §六（C66）：跨租户一律当「不存在」——模块契约写的是「不存在 / 跨租户 /
    // 非成员一律 404，不泄露存在性」。原实现把别的租户的任务报成 `CrossProject`
    // （409 CROSS_PROJECT_DEPENDENCY），与 404 形成存在性 oracle。同租户跨项目仍是
    // 409（D3，C46 契约不变）。
    if upstream_tenant != tenant_id {
        return Err(DependencyError::TaskNotFound);
    }
    if upstream_project != project_id {
        return Err(DependencyError::CrossProject);
    }

    if depends_on_task_id == task_id {
        return Err(DependencyError::SelfDependency);
    }

    // ③ D6：只允许给尚未进入就绪队列的任务加边。
    if !matches!(status.as_str(), "draft" | "blocked") {
        return Err(DependencyError::TaskNotPending(status));
    }

    // ④ 重复边（唯一键预检；并发插入由 ON CONFLICT 兜底）。
    let duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM delivery.task_dependency \
                          WHERE task_id = $1 AND depends_on_task_id = $2)",
    )
    .bind(task_id)
    .bind(depends_on_task_id)
    .fetch_one(&mut *tx)
    .await?;
    if duplicate {
        return Err(DependencyError::Duplicate);
    }

    // ⑤ 环检测（D2）：从新上游沿 `depends_on_task_id` 上溯，回到下游即有环。
    let cycle: bool = sqlx::query_scalar(
        "WITH RECURSIVE ancestors AS ( \
             SELECT $1::uuid AS id \
             UNION \
             SELECT d.depends_on_task_id \
               FROM delivery.task_dependency d \
               JOIN ancestors a ON d.task_id = a.id \
              WHERE d.project_id = $2) \
         SELECT EXISTS (SELECT 1 FROM ancestors WHERE id = $3)",
    )
    .bind(depends_on_task_id)
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&mut *tx)
    .await?;
    if cycle {
        return Err(DependencyError::Cycle);
    }

    let inserted = sqlx::query(
        "INSERT INTO delivery.task_dependency \
         (tenant_id, project_id, task_id, depends_on_task_id, created_by, \
          required_condition, required_deliverable_ref, updated_by) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4, $5, $6, $4) \
         ON CONFLICT (task_id, depends_on_task_id) DO NOTHING",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(depends_on_task_id)
    .bind(actor.principal_id)
    .bind(&required_condition)
    .bind(&required_deliverable_ref)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(DependencyError::Duplicate);
    }

    crate::pm_write::append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "task.dependency.add",
        "task",
        Some(task_id),
        json!({
            "depends_on_task_id": depends_on_task_id,
            "required_condition": required_condition,
            "required_deliverable_ref": required_deliverable_ref,
        }),
    )
    .await?;

    // ⑥ D7：已满足的前置立即解锁 `blocked` 下游（`draft` 保持原状）。
    unlock_task_if_ready(&mut tx, project_id, task_id).await?;

    tx.commit().await?;
    // 收尾回读：边事务内已校验租户归属（I3），按该租户锚定。
    list_dependencies(pool, tenant_id, task_id).await
}

/// 删边（DELETE）：幂等；删除成功后做一次解锁判定（D8）。
/// 删边（DELETE）：幂等；删除成功后做一次解锁判定（D8）。
///
/// PA07-IMP-003（审查82）：解除依赖必须记录原因——API 层强制必填；本入口为
/// 内部/测试便捷通道，`reason` 缺省记 `"unspecified"`。
pub async fn remove_dependency(
    pool: &PgPool,
    task_id: Uuid,
    depends_on_task_id: Uuid,
    actor: &AuditActor,
) -> Result<bool, DependencyError> {
    remove_dependency_with_reason(pool, task_id, depends_on_task_id, "unspecified", actor).await
}

/// 删边（DELETE，带解除原因）：审计载荷含原因与边条件（审查82：解除不是
/// 临时忽略门禁）。
pub async fn remove_dependency_with_reason(
    pool: &PgPool,
    task_id: Uuid,
    depends_on_task_id: Uuid,
    reason: &str,
    actor: &AuditActor,
) -> Result<bool, DependencyError> {
    let mut tx = pool.begin().await?;

    // ⓪ 与 add_dependency 同款：先无锁预读项目取图锁，再锁行（T-025 P0）。
    //    删边同样修改依赖图，必须与建边/终态迁移共用同一把锁。
    let graph_key: Uuid = sqlx::query_scalar("SELECT project_id FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DependencyError::TaskNotFound)?;
    acquire_project_graph_lock(&mut tx, graph_key).await?;

    let downstream = sqlx::query("SELECT project_id FROM delivery.task WHERE id = $1 FOR UPDATE")
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DependencyError::TaskNotFound)?;
    let project_id: Uuid = downstream.get("project_id");

    let deleted = sqlx::query(
        "DELETE FROM delivery.task_dependency \
          WHERE task_id = $1 AND depends_on_task_id = $2",
    )
    .bind(task_id)
    .bind(depends_on_task_id)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        > 0;

    if deleted {
        crate::pm_write::append_audit_with_after(
            &mut tx,
            project_id,
            actor,
            "task.dependency.remove",
            "task",
            Some(task_id),
            json!({
                "depends_on_task_id": depends_on_task_id,
                "reason": reason,
            }),
        )
        .await?;
        unlock_task_if_ready(&mut tx, project_id, task_id).await?;
    }

    tx.commit().await?;
    Ok(deleted)
}

/// 上游进入终态后的级联解锁（D4）：逐个把满足依赖的 `blocked` 下游迁到 `ready`。
///
/// 下游按 `id` 升序逐个加锁（固定顺序防死锁）；每个下游都重新读取状态再判定，
/// 因此并发解锁同一批下游是幂等的。
pub(crate) async fn unlock_downstream(
    tx: &mut Tx,
    project_id: Uuid,
    upstream_task_id: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT t.id \
           FROM delivery.task_dependency d \
           JOIN delivery.task t ON t.project_id = d.project_id AND t.id = d.task_id \
          WHERE d.project_id = $1 AND d.depends_on_task_id = $2 \
          ORDER BY t.id",
    )
    .bind(project_id)
    .bind(upstream_task_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut unlocked = Vec::new();
    for row in rows {
        let downstream_id: Uuid = row.get("id");
        if unlock_task_if_ready(tx, project_id, downstream_id).await? {
            unlocked.push(downstream_id);
        }
    }
    Ok(unlocked)
}

/// 单个任务的解锁判定：`blocked` 且全部前置满足 → `ready` + `TaskReady`（同事务）。
///
/// 返回是否真的发生迁移。`draft` / `ready` / 执行中 / 终态一律不动——`draft`
/// 是否进 `ready` 由用户或命令通道决定（D7）。
async fn unlock_task_if_ready(
    tx: &mut Tx,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        "SELECT status FROM delivery.task WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let status: String = row.get("status");
    if status != "blocked" {
        return Ok(false);
    }
    if !dependencies_satisfied(tx, project_id, task_id).await? {
        return Ok(false);
    }
    // T-025 §三（C61）：依赖满足 ≠ 所有阻塞解除。仍有活跃 waiting（信息/决策/
    // 审批/外部依赖）时保持 `blocked`；否则会把状态谎报为 `ready`，并产生一条
    // 领取侧必然拒绝（`deny = task_waiting`）的 Run。
    if has_active_waiting(tx, project_id, task_id).await? {
        return Ok(false);
    }

    // R1：状态推进仍经唯一受守卫入口（含 revision guard 与 update_task_status 审计）。
    let ctx = system_context(tx, project_id)
        .await
        .map_err(transition_err)?;
    let (_, revision) = apply_task_transition(
        tx,
        task_id,
        None,
        TaskStatus::Ready,
        &TransitionFacts::for_dispatch(),
        &ctx,
    )
    .await
    .map_err(transition_err)?;

    // 事件与迁移同事务；幂等键与命令通道一致（设计稿 §5.3）。
    if event_is_enabled("TaskReady") {
        let aggregate_id = task_id.to_string();
        let idempotency_key = format!("task:{task_id}:TaskReady:rev{revision}");
        PgOutboxAppend
            .append(
                tx,
                OutboxEvent {
                    project_id,
                    event_type: "TaskReady",
                    aggregate_type: "Task",
                    aggregate_id: &aggregate_id,
                    payload: json!({
                        "task_id": task_id,
                        "to": "ready",
                        "revision": revision,
                        "unlocked_by": "task_dependency",
                    }),
                    idempotency_key: &idempotency_key,
                },
            )
            .await?;
    }
    Ok(true)
}

/// 解锁路径只可能遇到数据库错误或状态机拒绝；后者在数据一致时不会出现，
/// 归并成 `Protocol` 保留上下文，不吞掉。
fn transition_err(e: pm_domain::CommandError) -> sqlx::Error {
    match e {
        pm_domain::CommandError::Db(inner) => inner,
        other => sqlx::Error::Protocol(other.to_string()),
    }
}
