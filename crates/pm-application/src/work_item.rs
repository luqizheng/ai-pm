//! 统一事项基础模型与 Activity Timeline（PA-05；2026-11-21 注册表裁决）。
//!
//! PA-05-01：统一基础模型 + 独立领域对象——本模块提供注册表
//! （`core.work_item`：统一事项 ID=主键、基础描述、创建信息、关联挂载）、
//! 统一关联（`core.work_item_relation`：多对多+关系类型+来源/理由）、
//! Activity Timeline（`core.work_item_activity`：追加式、human|agent 来源
//! 区分、事项/Product/Project/时间四维）与已读状态（`core.work_item_read_state`）。
//!
//! 边界（PA-05-06/07，不得越过）：
//! - 正式知识正文权威 = Markdown + Git；本表只存版本引用（revision_ref）。
//! - Execution Logs（工具调用/命令执行/自动检查）归 `delivery.task_run_event`，
//!   不写入 Timeline——Timeline 只记有业务意义的活动（8.1 粒度）。
//! - 历史不覆盖：纠错 = 追加 `correction` 活动，禁止 UPDATE 活动行。
//! - Processed/Approved 在领域对象自身状态，不进 read_state（四概念不分混）。
//!
//! 状态映射（PA-05-02）：统一语义仅供跨领域过滤/待办/统计，各领域映射
//! 显式声明；同名状态不默认同义。

use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum WorkItemError {
    #[error("work item not found")]
    NotFound,
    #[error("{0}")]
    Validation(&'static str),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

pub type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

/// V1 首批注册的领域对象（PA-05-01：其他有明确领域语义的对象随后续批次
/// 显式加入，不强制转化普通文档/讨论）。
pub const TARGET_TYPES: [&str; 6] = ["task", "requirement", "adr", "issue", "risk", "blocker"];

/// PA-05-03 V1 首批关系类型。
pub const RELATION_TYPES: [&str; 6] = [
    "implements",
    "supports",
    "affects",
    "blocks",
    "derived_from",
    "references",
];

// ─────────────────────────────────────────── 统一状态语义映射（PA-05-02）

/// delivery.task.status → 统一语义（13 值显式映射）。
pub fn task_status_semantic(status: &str) -> &'static str {
    match status {
        "draft" | "ready" => "open",
        "claimed" | "running" => "in_progress",
        "awaiting_decision" | "submitted" | "reviewing" | "changes_requested" => "in_review",
        "blocked" => "blocked",
        "completed" => "done",
        "failed" => "failed",
        _ => "cancelled", // cancelled / superseded
    }
}

/// delivery.requirement.lifecycle_status → 统一语义。
pub fn requirement_status_semantic(status: &str) -> &'static str {
    match status {
        "active" => "open",
        "withdrawn" => "cancelled",
        "archived" => "archived",
        _ => "open",
    }
}

/// delivery.issue.status → 统一语义。
pub fn issue_status_semantic(status: &str) -> &'static str {
    match status {
        "resolved" => "done",
        "cancelled" => "cancelled",
        _ => "open",
    }
}

/// delivery.risk.status → 统一语义。
pub fn risk_status_semantic(status: &str) -> &'static str {
    match status {
        "mitigating" => "in_progress",
        "closed" => "done",
        _ => "open",
    }
}

/// delivery.blocker.status → 统一语义。
pub fn blocker_status_semantic(status: &str) -> &'static str {
    match status {
        "cleared" => "done",
        _ => "blocked",
    }
}

/// design.adr.status → 统一语义。
pub fn adr_status_semantic(status: &str) -> &'static str {
    match status {
        "proposed" => "open",
        "under_review" => "in_review",
        "accepted" => "done",
        "rejected" => "cancelled",
        "superseded" | "deprecated" => "archived",
        _ => "open",
    }
}

// ─────────────────────────────────────────── 注册表

/// 项目归属租户（接线点上下文补充；项目不存在 → NotFound）。
pub async fn project_tenant(tx: &mut Tx<'_>, project_id: Uuid) -> Result<Uuid, WorkItemError> {
    let tenant_id: Option<Uuid> =
        sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(&mut **tx)
            .await?;
    tenant_id.ok_or(WorkItemError::NotFound)
}

async fn target_title(
    tx: &mut Tx<'_>,
    target_type: &str,
    target_id: Uuid,
) -> Result<String, WorkItemError> {
    let sql = match target_type {
        "task" => "SELECT title FROM delivery.task WHERE id = $1",
        "requirement" => "SELECT title FROM delivery.requirement WHERE id = $1",
        "adr" => "SELECT title FROM design.adr WHERE id = $1",
        "issue" => "SELECT title FROM delivery.issue WHERE id = $1",
        "risk" => "SELECT title FROM delivery.risk WHERE id = $1",
        "blocker" => "SELECT title FROM delivery.blocker WHERE id = $1",
        _ => return Err(WorkItemError::Validation("unknown work item target type")),
    };
    let title: Option<String> = sqlx::query_scalar(sql)
        .bind(target_id)
        .fetch_optional(&mut **tx)
        .await?;
    title.ok_or(WorkItemError::NotFound)
}

/// 注册（幂等）：同一领域对象重复注册返回既有 id（自愈注册支持存量事项
/// 首次接线）；title/status 以最新注册参数为准。
#[allow(clippy::too_many_arguments)]
pub async fn ensure_work_item(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    target_type: &str,
    target_id: Uuid,
    project_id: Option<Uuid>,
    title: &str,
    semantic_status: &str,
    created_by: Option<Uuid>,
) -> Result<Uuid, WorkItemError> {
    if !TARGET_TYPES.contains(&target_type) {
        return Err(WorkItemError::Validation("unknown work item target type"));
    }
    let item_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.work_item \
         (tenant_id, target_type, target_id, project_id, title, status, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (tenant_id, target_type, target_id) \
         DO UPDATE SET title = EXCLUDED.title, updated_at = now() \
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(target_type)
    .bind(target_id)
    .bind(project_id)
    .bind(title)
    .bind(semantic_status)
    .bind(created_by)
    .fetch_one(&mut **tx)
    .await?;
    Ok(item_id)
}

/// 领域对象自愈注册：查领域标题后注册（存量事项首次经过接线点时调用）。
pub async fn ensure_work_item_for_target(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    target_type: &str,
    target_id: Uuid,
    project_id: Option<Uuid>,
    semantic_status: &str,
    created_by: Option<Uuid>,
) -> Result<Uuid, WorkItemError> {
    let title = target_title(tx, target_type, target_id).await?;
    ensure_work_item(
        tx,
        tenant_id,
        target_type,
        target_id,
        project_id,
        &title,
        semantic_status,
        created_by,
    )
    .await
}

// ─────────────────────────────────────────── Activity Timeline（追加式）

/// 记录一条 Timeline 活动（PA-05-05/07；无 UPDATE 路径——纠错由调用方
/// 追加 `correction` 类型）。`actor_type`：human|agent（PA-05-05 来源区分）。
#[allow(clippy::too_many_arguments)]
pub async fn record_activity(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    item_id: Uuid,
    project_id: Option<Uuid>,
    product_id: Option<Uuid>,
    activity_type: &str,
    actor_type: &str,
    actor_id: Option<Uuid>,
    summary: &str,
    detail: Option<&Value>,
    revision_ref: Option<&Value>,
) -> Result<Uuid, WorkItemError> {
    if actor_type != "human" && actor_type != "agent" {
        return Err(WorkItemError::Validation("actor_type must be human|agent"));
    }
    let activity_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.work_item_activity \
         (tenant_id, item_id, project_id, product_id, activity_type, actor_type, actor_id, summary, detail, revision_ref) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
    )
    .bind(tenant_id)
    .bind(item_id)
    .bind(project_id)
    .bind(product_id)
    .bind(activity_type)
    .bind(actor_type)
    .bind(actor_id)
    .bind(summary)
    .bind(detail)
    .bind(revision_ref)
    .fetch_one(&mut **tx)
    .await?;
    Ok(activity_id)
}

/// Timeline 四维查询（PA-05-05：事项/Product/Project/时间；三过滤条件
/// 至少给一个，全空 → Validation）。
#[allow(clippy::too_many_arguments)]
pub async fn list_activity(
    pool: &PgPool,
    tenant_id: Uuid,
    item_id: Option<Uuid>,
    project_id: Option<Uuid>,
    product_id: Option<Uuid>,
    limit: i64,
    offset: i64,
) -> Result<Vec<ActivityRow>, WorkItemError> {
    if item_id.is_none() && project_id.is_none() && product_id.is_none() {
        return Err(WorkItemError::Validation(
            "one of item_id / project_id / product_id is required",
        ));
    }
    let rows = sqlx::query_as::<_, ActivityRow>(
        "SELECT id, item_id, project_id, product_id, activity_type, actor_type, actor_id, \
                summary, detail, revision_ref, created_at \
           FROM core.work_item_activity \
          WHERE tenant_id = $1 \
            AND ($2::uuid IS NULL OR item_id = $2) \
            AND ($3::uuid IS NULL OR project_id = $3) \
            AND ($4::uuid IS NULL OR product_id = $4) \
          ORDER BY created_at DESC, id DESC \
          LIMIT $5 OFFSET $6",
    )
    .bind(tenant_id)
    .bind(item_id)
    .bind(project_id)
    .bind(product_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct ActivityRow {
    pub id: Uuid,
    pub item_id: Uuid,
    pub project_id: Option<Uuid>,
    pub product_id: Option<Uuid>,
    pub activity_type: String,
    pub actor_type: String,
    pub actor_id: Option<Uuid>,
    pub summary: String,
    pub detail: Option<Value>,
    pub revision_ref: Option<Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ─────────────────────────────────────────── 统一关联（PA-05-03/04）

/// 建立关联：来源 human|agent + 理由（PA-05-04 记录要求）；关联不改变
/// 对象归属。重复 (source, target, type) → 幂等返回既有关系 id。
#[allow(clippy::too_many_arguments)]
pub async fn add_relation(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    source_item_id: Uuid,
    target_item_id: Uuid,
    relation_type: &str,
    origin: &str,
    reason: Option<&str>,
    created_by: Option<Uuid>,
) -> Result<Uuid, WorkItemError> {
    if !RELATION_TYPES.contains(&relation_type) {
        return Err(WorkItemError::Validation("unknown relation type"));
    }
    if origin != "human" && origin != "agent" {
        return Err(WorkItemError::Validation("origin must be human|agent"));
    }
    if source_item_id == target_item_id {
        return Err(WorkItemError::Validation("self relation is not allowed"));
    }
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO core.work_item_relation \
         (tenant_id, source_item_id, target_item_id, relation_type, origin, reason, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (source_item_id, target_item_id, relation_type) DO NOTHING \
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(source_item_id)
    .bind(target_item_id)
    .bind(relation_type)
    .bind(origin)
    .bind(reason)
    .bind(created_by)
    .fetch_optional(&mut **tx)
    .await?;
    // 幂等：已存在 → 返回既有关系 id，不重复记 relation_changed 活动。
    let Some(relation_id) = inserted else {
        let existing: Uuid = sqlx::query_scalar(
            "SELECT id FROM core.work_item_relation \
              WHERE source_item_id = $1 AND target_item_id = $2 AND relation_type = $3",
        )
        .bind(source_item_id)
        .bind(target_item_id)
        .bind(relation_type)
        .fetch_one(&mut **tx)
        .await?;
        return Ok(existing);
    };
    record_activity(
        tx,
        tenant_id,
        source_item_id,
        None,
        None,
        "relation_changed",
        origin,
        created_by,
        &format!("Relation {relation_type} established"),
        Some(&serde_json::json!({
            "relation_type": relation_type,
            "source_item_id": source_item_id,
            "target_item_id": target_item_id,
            "origin": origin,
            "reason": reason,
        })),
        None,
    )
    .await?;
    Ok(relation_id)
}

/// 解除关联（PA-05-04：人类可直接调整普通关联；Agent 不得覆盖人类明确
/// 修改的关系——V1 删除通道仅服务人类操作面，agent 删除不在 API 暴露）。
/// 历史不覆盖：解除本身记 `relation_changed` 活动。
pub async fn remove_relation(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    relation_id: Uuid,
    actor_id: Option<Uuid>,
) -> Result<(), WorkItemError> {
    let row: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        "DELETE FROM core.work_item_relation \
          WHERE id = $1 AND tenant_id = $2 \
          RETURNING source_item_id, target_item_id, relation_type",
    )
    .bind(relation_id)
    .bind(tenant_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((source_item_id, target_item_id, relation_type)) = row else {
        return Err(WorkItemError::NotFound);
    };
    record_activity(
        tx,
        tenant_id,
        source_item_id,
        None,
        None,
        "relation_changed",
        "human",
        actor_id,
        &format!("Relation {relation_type} removed"),
        Some(&serde_json::json!({
            "relation_type": relation_type,
            "source_item_id": source_item_id,
            "target_item_id": target_item_id,
            "removed": true,
        })),
        None,
    )
    .await?;
    Ok(())
}

// ─────────────────────────────────────────── 接线复合函数（批次 B）

/// Task 状态迁移接线（`apply_task_transition` 单点调用）：自愈注册 →
/// 统一状态同步 → 语义变化时记 `status_changed` 活动（语义未变不记，
/// 对应 PA-05-07 §8.3 合并规则）。
#[allow(clippy::too_many_arguments)]
pub async fn on_task_transition(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
    status_before: &str,
    status_after: &str,
    actor_type: &str,
    actor_id: Option<Uuid>,
) -> Result<(), WorkItemError> {
    let semantic_before = task_status_semantic(status_before);
    let semantic_after = task_status_semantic(status_after);
    let item_id = ensure_work_item_for_target(
        tx,
        tenant_id,
        "task",
        task_id,
        Some(project_id),
        semantic_after,
        actor_id,
    )
    .await?;
    if semantic_before == semantic_after {
        return Ok(());
    }
    // 注册冲突分支不覆盖状态（避免错误注册清真实状态）——迁移接线显式同步。
    sqlx::query("UPDATE core.work_item SET status = $1, updated_at = now() WHERE id = $2")
        .bind(semantic_after)
        .bind(item_id)
        .execute(&mut **tx)
        .await?;
    let title = target_title(tx, "task", task_id).await?;
    record_activity(
        tx,
        tenant_id,
        item_id,
        Some(project_id),
        None,
        "status_changed",
        actor_type,
        actor_id,
        &format!("Task \"{title}\": {status_before} → {status_after}"),
        Some(&serde_json::json!({
            "from": status_before,
            "to": status_after,
            "semantic_from": semantic_before,
            "semantic_to": semantic_after,
        })),
        None,
    )
    .await?;
    Ok(())
}

// ─────────────────────────────────────────── 三 Feed 视角（PA-05-08 批次 C）

/// Feed 行：活动 + 所属事项定位 + 调用者已读状态。同一活动在 My Feed 中
/// 只出现一次（按活动行去重——活动行的 project/product 本就单一）。
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct FeedRow {
    pub id: Uuid,
    pub item_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub item_title: String,
    pub activity_type: String,
    pub actor_type: String,
    pub actor_id: Option<Uuid>,
    pub summary: String,
    pub detail: Option<Value>,
    pub project_id: Option<Uuid>,
    pub product_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub read_at: Option<chrono::DateTime<chrono::Utc>>,
}

const FEED_SELECT: &str = "\
SELECT a.id, a.item_id, w.target_type, w.target_id, w.title AS item_title, \
       a.activity_type, a.actor_type, a.actor_id, a.summary, a.detail, \
       a.project_id, a.product_id, a.created_at, r.read_at \
  FROM core.work_item_activity a \
  JOIN core.work_item w ON w.id = a.item_id \
  LEFT JOIN core.work_item_read_state r \
         ON r.item_id = a.item_id AND r.principal_id = $2 AND r.tenant_id = a.tenant_id";

/// My Feed 视图/筛选参数（PA-05 UI：All activity | Unread | Needs attention
/// 三视图 + Project/Activity type/Actor 筛选）。
#[derive(Debug, Clone, Default)]
pub struct MyFeedFilter {
    /// 仅未读（调用者 read_at 为空）。
    pub unread_only: bool,
    /// 仅需关注：未读且事项统一状态仍开放（open/blocked/in_review）——
    /// 「读了不等于处理完」（PA-05-09），事项关闭后活动只留历史。
    pub attention_only: bool,
    /// human | agent。
    pub actor_type: Option<String>,
    /// 按活动类型过滤（created/status_changed/relation_changed/…）。
    pub activity_type: Option<String>,
    /// My Feed 内再收敛到单一项目。
    pub project_id: Option<Uuid>,
}

const MY_FEED_SCOPE: &str = "(a.project_id IN (SELECT project_id FROM core.project_member WHERE tenant_id = $1 AND principal_id = $2) OR a.product_id IN (SELECT product_id FROM core.product_member WHERE tenant_id = $1 AND principal_id = $2))";

/// My Feed（PA-05-08）：调用者 Project Membership（项目活动）∪ Product
/// Membership（产品维度活动）的去重集合，时间倒序。已读状态按调用者。
pub async fn my_feed(
    pool: &PgPool,
    tenant_id: Uuid,
    principal_id: Uuid,
    filter: Option<&MyFeedFilter>,
    limit: i64,
    offset: i64,
) -> Result<Vec<FeedRow>, WorkItemError> {
    let mut sql = format!("{FEED_SELECT} WHERE a.tenant_id = $1 AND {MY_FEED_SCOPE}");
    let f = filter.cloned().unwrap_or_default();
    if f.unread_only || f.attention_only {
        sql.push_str(" AND r.read_at IS NULL");
    }
    if let Some(actor) = f.actor_type.as_deref() {
        sql.push_str(&format!(" AND a.actor_type = '{actor}'"));
    }
    if let Some(kind) = f.activity_type.as_deref() {
        sql.push_str(&format!(" AND a.activity_type = '{kind}'"));
    }
    if let Some(project) = f.project_id {
        sql.push_str(&format!(" AND a.project_id = '{project}'"));
    }
    if f.attention_only {
        // 需关注 = 未读且事项仍开放（open/blocked/in_review）。
        sql.push_str(" AND w.status IN ('open','blocked','in_review')");
    }
    sql.push_str(&format!(
        " ORDER BY a.created_at DESC, a.id DESC LIMIT {limit} OFFSET {offset}"
    ));
    let rows = sqlx::query_as::<_, FeedRow>(&sql)
        .bind(tenant_id)
        .bind(principal_id)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// My Feed 计数（页头徽标与视图条：unread / needs attention / 总量）。
pub async fn my_feed_counts(
    pool: &PgPool,
    tenant_id: Uuid,
    principal_id: Uuid,
) -> Result<FeedCounts, WorkItemError> {
    let base = format!(
        "FROM core.work_item_activity a \
         JOIN core.work_item w ON w.id = a.item_id \
         LEFT JOIN core.work_item_read_state r \
                ON r.item_id = a.item_id AND r.principal_id = $2 AND r.tenant_id = $1 \
         WHERE a.tenant_id = $1 AND {MY_FEED_SCOPE}"
    );
    let total: i64 = sqlx::query_scalar(&format!("SELECT count(*) {base}"))
        .bind(tenant_id)
        .bind(principal_id)
        .fetch_one(pool)
        .await?;
    let unread: i64 = sqlx::query_scalar(&format!("SELECT count(*) {base} AND r.read_at IS NULL"))
        .bind(tenant_id)
        .bind(principal_id)
        .fetch_one(pool)
        .await?;
    let attention: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) {base} AND r.read_at IS NULL AND w.status IN ('open','blocked','in_review')"
    ))
    .bind(tenant_id)
    .bind(principal_id)
    .fetch_one(pool)
    .await?;
    Ok(FeedCounts {
        total,
        unread,
        attention,
    })
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct FeedCounts {
    pub total: i64,
    pub unread: i64,
    pub attention: i64,
}

/// 全部标记已读（PA-05 UI「Mark all as read」）：My Feed 范围内所有未读
/// 置读；只动 Read（普通活动支持批量标记，PA-05-09）。
pub async fn mark_all_my_feed_read(
    pool: &PgPool,
    tenant_id: Uuid,
    principal_id: Uuid,
) -> Result<u64, WorkItemError> {
    let result = sqlx::query(
        "INSERT INTO core.work_item_read_state (tenant_id, principal_id, item_id, read_at) \
         SELECT $1, $2, D.item_id, now() \
           FROM ( \
             SELECT DISTINCT a.item_id \
               FROM core.work_item_activity a \
               LEFT JOIN core.work_item_read_state r \
                      ON r.item_id = a.item_id AND r.principal_id = $2 AND r.tenant_id = $1 \
              WHERE a.tenant_id = $1 AND r.read_at IS NULL \
                AND (a.project_id IN (SELECT project_id FROM core.project_member WHERE tenant_id = $1 AND principal_id = $2) \
                  OR a.product_id IN (SELECT product_id FROM core.product_member WHERE tenant_id = $1 AND principal_id = $2)) \
           ) AS D \
         ON CONFLICT (principal_id, item_id) DO UPDATE SET read_at = now()",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Project Feed：项目全部活动（成员资格由 API 层守卫校验）。
pub async fn project_feed(
    pool: &PgPool,
    tenant_id: Uuid,
    principal_id: Uuid,
    project_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<FeedRow>, WorkItemError> {
    let rows = sqlx::query_as::<_, FeedRow>(&format!(
        "{FEED_SELECT} \
          WHERE a.tenant_id = $1 AND a.project_id = $3 \
          ORDER BY a.created_at DESC, a.id DESC LIMIT $4 OFFSET $5"
    ))
    .bind(tenant_id)
    .bind(principal_id)
    .bind(project_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Product Feed：产品维度活动（成员资格由 API 层守卫校验）。
pub async fn product_feed(
    pool: &PgPool,
    tenant_id: Uuid,
    principal_id: Uuid,
    product_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<FeedRow>, WorkItemError> {
    let rows = sqlx::query_as::<_, FeedRow>(&format!(
        "{FEED_SELECT} \
          WHERE a.tenant_id = $1 AND a.product_id = $3 \
          ORDER BY a.created_at DESC, a.id DESC LIMIT $4 OFFSET $5"
    ))
    .bind(tenant_id)
    .bind(principal_id)
    .bind(product_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 已读状态写面（PA-05-09）：批量标记/清除 Read；只动 read_at，不碰
/// Notified/Processed/Approved（四概念互不等同）。
pub async fn mark_read_state(
    pool: &PgPool,
    tenant_id: Uuid,
    principal_id: Uuid,
    item_ids: &[Uuid],
    read: bool,
) -> Result<u64, WorkItemError> {
    if item_ids.is_empty() {
        return Err(WorkItemError::Validation("item_ids must not be empty"));
    }
    let result = if read {
        sqlx::query(
            "INSERT INTO core.work_item_read_state (tenant_id, principal_id, item_id, read_at) \
             SELECT $1, $2, x.item_id, now() \
               FROM unnest($3::uuid[]) AS x(item_id) \
             ON CONFLICT (principal_id, item_id) \
               DO UPDATE SET read_at = now()",
        )
        .bind(tenant_id)
        .bind(principal_id)
        .bind(item_ids)
        .execute(pool)
        .await?
    } else {
        sqlx::query(
            "UPDATE core.work_item_read_state SET read_at = NULL \
              WHERE tenant_id = $1 AND principal_id = $2 AND item_id = ANY($3)",
        )
        .bind(tenant_id)
        .bind(principal_id)
        .bind(item_ids)
        .execute(pool)
        .await?
    };
    Ok(result.rows_affected())
}

// ─────────────────────────────────────────── 详情面（PA-05 UI 屏 04/05/06）

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct WorkItemDetailRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub title: String,
    pub status: String,
    pub project_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct RelationViewRow {
    pub id: Uuid,
    /// outgoing = 本事项为 source；incoming = 本事项为 target。
    pub direction: String,
    pub relation_type: String,
    pub other_item_id: Uuid,
    pub other_type: String,
    pub other_title: String,
    pub other_status: String,
    pub origin: String,
    pub reason: Option<String>,
}

/// 统一工作项详情（PA-05 UI 屏 04）：注册行 + 双向关联（含对端事项标题/
/// 状态）+ 最近 Timeline 活动。守卫（项目可见性）由 API 层执行。
pub async fn work_item_detail(
    pool: &PgPool,
    tenant_id: Uuid,
    item_id: Uuid,
    activity_limit: i64,
) -> Result<(WorkItemDetailRow, Vec<RelationViewRow>, Vec<ActivityRow>), WorkItemError> {
    let item: Option<WorkItemDetailRow> = sqlx::query_as(
        "SELECT id, tenant_id, target_type, target_id, title, status, project_id, \
                created_at, updated_at \
           FROM core.work_item WHERE id = $1 AND tenant_id = $2",
    )
    .bind(item_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?;
    let Some(item) = item else {
        return Err(WorkItemError::NotFound);
    };
    let relations = sqlx::query_as::<_, RelationViewRow>(
        "SELECT r.id, 'outgoing' AS direction, r.relation_type, \
                w.id AS other_item_id, w.target_type AS other_type, \
                w.title AS other_title, w.status AS other_status, r.origin, r.reason \
           FROM core.work_item_relation r \
           JOIN core.work_item w ON w.id = r.target_item_id \
          WHERE r.source_item_id = $1 \
         UNION ALL \
         SELECT r.id, 'incoming' AS direction, r.relation_type, \
                w.id, w.target_type, w.title, w.status, r.origin, r.reason \
           FROM core.work_item_relation r \
           JOIN core.work_item w ON w.id = r.source_item_id \
          WHERE r.target_item_id = $1 \
         ORDER BY relation_type, other_title",
    )
    .bind(item_id)
    .fetch_all(pool)
    .await?;
    let activities = list_activity(
        pool,
        tenant_id,
        Some(item_id),
        None,
        None,
        activity_limit,
        0,
    )
    .await?;
    Ok((item, relations, activities))
}

/// Execution log 行（PA-05-08 §8.2：详细执行过程留在 task_run_event，
/// 屏 06 的 Logs 页签数据源；仅 task 事项有执行面）。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ExecutionLogRow {
    pub id: Uuid,
    pub run_id: Uuid,
    pub event_type: String,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
    pub detail: Option<Value>,
}

pub async fn item_execution_logs(
    pool: &PgPool,
    tenant_id: Uuid,
    task_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<ExecutionLogRow>, WorkItemError> {
    let rows = sqlx::query_as::<_, ExecutionLogRow>(
        "SELECT id, run_id, event_type, occurred_at, detail \
           FROM delivery.task_run_event \
          WHERE tenant_id = $1 AND task_id = $2 \
          ORDER BY occurred_at DESC, id DESC LIMIT $3 OFFSET $4",
    )
    .bind(tenant_id)
    .bind(task_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 单条 Timeline 活动（PA-05 UI 屏 05 Activity detail；含调用者已读状态，
/// 打开即标已读由 API 层调用 mark_read_state 完成）。
pub async fn get_activity(
    pool: &PgPool,
    tenant_id: Uuid,
    principal_id: Uuid,
    activity_id: Uuid,
) -> Result<FeedRow, WorkItemError> {
    let row = sqlx::query_as::<_, FeedRow>(&format!(
        "{FEED_SELECT} WHERE a.tenant_id = $1 AND a.id = $3"
    ))
    .bind(tenant_id)
    .bind(principal_id)
    .bind(activity_id)
    .fetch_optional(pool)
    .await?;
    row.ok_or(WorkItemError::NotFound)
}
