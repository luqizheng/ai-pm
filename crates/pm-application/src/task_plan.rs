//! S04 · 任务计划统一服务（S4 提案 V1 §1.3/§2/§3 落地）。
//!
//! 「计划」聚合层的**单点实现**：
//! - 计划 CRUD + Revision（改稿/提交/裁决，库级内容冻结同 design_revision）；
//! - 审批复用 S03 专表 `design.design_approval`（kind='task_plan'，R6）；
//! - **正式化**：批准修订的草案批量创建正式任务（`delivery.task`，复用
//!   13 态/仓库绑定/验收标准/依赖边全部现有能力），幂等（UNIQUE
//!   (plan_revision_id, plan_task_key)），并把验收标准/仓库/依赖一并落库；
//! - **重拆（Q2）**：AI 永远产出新 Revision 草案 diff（新增/修改/删除建议），
//!   不原地覆盖；保留人工修改由「拒绝采纳」表达；
//! - **交接（§3.3）**：执行资格检查（计划 approved + 任务 ready + 依赖
//!   completed + 仓库绑定有效 + 验收非空）+ 上下文载荷组装；
//! - **进度（§3.4）**：计划整体进度与阻塞清单读模型。
//!
//! 治理红线（与 S02/S03 一致）：AI 无批准权（decide/正式化 ready 一律真人）；
//! 正式化本身允许 AI 主体在计划批准后触发（机械转写已批准内容），但
//! draft→ready 仍必须真人（复用 `mark_task_batch_ready` 的 require_human）。

use chrono::{DateTime, Utc};
use pm_domain::plan::{TaskPlanRevisionState, TaskPlanStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::design::{ApprovalView, DECIDE_APPROVAL_ACTIONS};
use crate::pm_write::{append_audit, append_audit_with_after, hex_sha256, PmWriteError};
use crate::requirement::require_human_conn;

// ───────────────────────────────────────────────────────────────────── 视图

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct TaskPlanEntry {
    pub id: Uuid,
    pub project_id: Uuid,
    pub code: String,
    pub title: String,
    pub status: String,
    pub current_revision_id: Option<Uuid>,
    pub source_baseline_id: Option<Uuid>,
    pub row_revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct TaskPlanRevisionView {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub version: i32,
    pub content: Value,
    pub state: String,
    pub content_hash: String,
    pub source: String,
    pub instructions: Option<String>,
    pub created_by: Uuid,
    pub submitted_at: Option<DateTime<Utc>>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// 草案里的单条任务建议（content.tasks[] 元素）。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanTaskDraft {
    /// 批内引用键（依赖建议用键；正式化后写入 task.plan_task_key）。
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    /// 可验证验收标准（每条非空；正式化同事务写入 task_acceptance_criterion）。
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    /// 依赖建议（同批更早出现的任务键；正式化落 task_dependency 边）。
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// 仓库建议（repository_id；正式化校验属本项目 + verified 后绑定）。
    #[serde(default)]
    pub repository_ids: Vec<Uuid>,
    /// 可选优先级建议（缺省 normal）。
    #[serde(default)]
    pub priority: Option<String>,
    /// 变更标记（重拆 diff 用：add/update/remove）。
    #[serde(default)]
    pub change: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PlanDetail {
    pub plan: TaskPlanEntry,
    pub revisions: Vec<TaskPlanRevisionView>,
    pub approvals: Vec<ApprovalView>,
    /// 已正式化任务（plan_revision_id 锚定）。
    pub materialized_tasks: Vec<MaterializedTask>,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct MaterializedTask {
    pub task_id: Uuid,
    pub plan_task_key: Option<String>,
    pub title: String,
    pub status: String,
}

// ───────────────────────────────────────────────────────────────────── 请求

#[derive(Debug, Deserialize)]
pub struct CreatePlanRequest {
    #[serde(default)]
    pub code: Option<String>,
    pub title: String,
    /// 来源 S03 冻结基线（Q4：无基线也允许建计划，但拆解准入校验时要求）。
    #[serde(default)]
    pub source_baseline_id: Option<Uuid>,
    #[serde(default)]
    pub tasks: Vec<PlanTaskDraft>,
    #[serde(default)]
    pub instructions: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DraftPlanRevisionRequest {
    #[serde(default)]
    pub tasks: Vec<PlanTaskDraft>,
    #[serde(default)]
    pub instructions: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MaterializeRequest {
    /// 缺省 = 计划当前批准修订。
    #[serde(default)]
    pub plan_revision_id: Option<Uuid>,
    /// 缺省 = 修订内全部任务；显式给键则只正式化子集（部分重试）。
    #[serde(default)]
    pub task_keys: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct SupersedePlanRequest {
    pub successor_plan_id: Uuid,
    #[serde(default)]
    pub reason: Option<String>,
}

// ─────────────────────────────────────────────────────────────── 内部助手

const PLAN_VIEW_COLS: &str = "SELECT p.id, p.project_id, p.code, p.title, p.status, \
     p.current_revision_id, p.source_baseline_id, p.row_revision, p.created_at, p.updated_at";

const PLAN_FROM: &str = "FROM plan.task_plan p";

fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

/// 草案规范化：键非空唯一、标题非空、验收每条非空、依赖只指向更早出现的键
/// （前向图，与 discovery_writeback::validate_task_batch 同款系统侧纪律）。
pub fn validate_plan_tasks(tasks: &[PlanTaskDraft]) -> Result<(), String> {
    if tasks.is_empty() {
        return Err("plan must contain at least one task".to_string());
    }
    let mut seen = std::collections::HashSet::new();
    for (i, t) in tasks.iter().enumerate() {
        let key = t.key.trim();
        if key.is_empty() {
            return Err(format!("tasks[{i}].key is empty"));
        }
        if !seen.insert(key.to_string()) {
            return Err(format!("duplicate task key `{key}`"));
        }
        if t.title.trim().is_empty() {
            return Err(format!("tasks[{i}] (`{key}`) title is empty"));
        }
        if t.acceptance_criteria.iter().any(|c| c.trim().is_empty()) {
            return Err(format!(
                "tasks[{i}] (`{key}`) has an empty acceptance criterion"
            ));
        }
        if t.acceptance_criteria.is_empty() {
            return Err(format!("tasks[{i}] (`{key}`) has no acceptance criteria"));
        }
        for dep in &t.depends_on {
            let dep = dep.trim();
            if dep.is_empty() {
                return Err(format!("tasks[{i}] (`{key}`) has an empty dependency key"));
            }
            if !tasks[..i].iter().any(|x| x.key.trim() == dep) {
                return Err(format!(
                    "tasks[{i}] (`{key}`) depends on `{dep}` which is not an earlier task"
                ));
            }
        }
        if let Some(p) = t.priority.as_deref() {
            if !["low", "normal", "high", "urgent"].contains(&p) {
                return Err(format!("tasks[{i}] (`{key}`) has invalid priority `{p}`"));
            }
        }
    }
    Ok(())
}

fn build_revision_content(tasks: &[PlanTaskDraft]) -> Value {
    json!({ "tasks": tasks })
}

fn content_hash_of(content: &Value) -> String {
    let bytes = serde_json::to_vec(content).expect("serialize content");
    hex_sha256(&bytes)
}

pub(crate) async fn next_plan_code(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<String, sqlx::Error> {
    let max: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(NULLIF(regexp_replace(code, '\\D', '', 'g'), '')::bigint), 0) \
         FROM plan.task_plan WHERE tenant_id = $1 AND project_id = $2 AND code LIKE 'TP-%'",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(&mut *conn)
    .await?;
    Ok(format!("TP-{:03}", max + 1))
}

async fn lock_plan(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    plan_id: Uuid,
) -> Result<(Uuid, String, Option<Uuid>, i64), PmWriteError> {
    sqlx::query_as(
        "SELECT project_id, status, current_revision_id, row_revision \
         FROM plan.task_plan WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(plan_id)
    .bind(tenant_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("task plan not found"))
}

async fn lock_plan_revision(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    plan_id: Uuid,
    revision_id: Uuid,
) -> Result<(i32, String, String, Value), PmWriteError> {
    sqlx::query_as(
        "SELECT version, state, content_hash, content \
         FROM plan.task_plan_revision \
         WHERE id = $1 AND plan_id = $2 AND tenant_id = $3 FOR UPDATE",
    )
    .bind(revision_id)
    .bind(plan_id)
    .bind(tenant_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("task plan revision not found"))
}

async fn fetch_revision_view(
    db: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
    plan_id: Uuid,
    revision_id: Uuid,
) -> Result<TaskPlanRevisionView, PmWriteError> {
    sqlx::query_as(
        "SELECT id, plan_id, version, content, state, content_hash, source, instructions, \
                created_by, submitted_at, approved_by, approved_at, created_at \
         FROM plan.task_plan_revision \
         WHERE tenant_id = $1 AND plan_id = $2 AND id = $3",
    )
    .bind(tenant_id)
    .bind(plan_id)
    .bind(revision_id)
    .fetch_optional(db)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("task plan revision not found"))
}

fn parse_status(status: &str) -> Result<TaskPlanStatus, PmWriteError> {
    TaskPlanStatus::parse(status).ok_or_else(|| PmWriteError::Validation("unknown plan status"))
}

fn parse_revision_state(state: &str) -> Result<TaskPlanRevisionState, PmWriteError> {
    TaskPlanRevisionState::parse(state)
        .ok_or_else(|| PmWriteError::Validation("unknown plan revision state"))
}

fn transition_or_conflict<S: Copy>(
    legal: impl Fn(S, S) -> bool,
    from: S,
    to: S,
    what: &'static str,
) -> Result<(), PmWriteError> {
    if legal(from, to) {
        Ok(())
    } else {
        Err(PmWriteError::Conflict(what))
    }
}

async fn materialized_tasks_of(
    db: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
    revision_id: Uuid,
) -> Result<Vec<MaterializedTask>, PmWriteError> {
    sqlx::query_as(
        "SELECT t.id AS task_id, t.plan_task_key, t.title, t.status \
         FROM delivery.task t \
         WHERE t.tenant_id = $1 AND t.plan_revision_id = $2 \
         ORDER BY t.created_at",
    )
    .bind(tenant_id)
    .bind(revision_id)
    .fetch_all(db)
    .await
    .map_err(PmWriteError::Db)
}

// ─────────────────────────────────────────────────────────────── 计划 CRUD

pub async fn list_plans(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<TaskPlanEntry>, PmWriteError> {
    sqlx::query_as(&format!(
        "{PLAN_VIEW_COLS} {PLAN_FROM} \
         WHERE p.tenant_id = $1 AND p.project_id = $2 ORDER BY p.created_at DESC"
    ))
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

pub async fn plan_detail(
    pool: &PgPool,
    tenant_id: Uuid,
    plan_id: Uuid,
) -> Result<PlanDetail, PmWriteError> {
    let plan: TaskPlanEntry = sqlx::query_as(&format!(
        "{PLAN_VIEW_COLS} {PLAN_FROM} WHERE p.tenant_id = $1 AND p.id = $2"
    ))
    .bind(tenant_id)
    .bind(plan_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("task plan not found"))?;
    let revisions: Vec<TaskPlanRevisionView> = sqlx::query_as(
        "SELECT id, plan_id, version, content, state, content_hash, source, instructions, \
                created_by, submitted_at, approved_by, approved_at, created_at \
         FROM plan.task_plan_revision \
         WHERE tenant_id = $1 AND plan_id = $2 ORDER BY version DESC",
    )
    .bind(tenant_id)
    .bind(plan_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let approvals: Vec<ApprovalView> = sqlx::query_as(
        "SELECT id, kind, design_id, adr_id, baseline_id, plan_id, target_id, target_hash, \
                status, requested_by, decided_by, decision_reason, decided_at, created_at \
         FROM design.design_approval \
         WHERE tenant_id = $1 AND plan_id = $2 ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .bind(plan_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let mut materialized_tasks = Vec::new();
    if let Some(rev) = revisions.first() {
        materialized_tasks = materialized_tasks_of(pool, tenant_id, rev.id).await?;
    }
    Ok(PlanDetail {
        plan,
        revisions,
        approvals,
        materialized_tasks,
    })
}

pub async fn create_plan(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    req: CreatePlanRequest,
    actor: &AuditActor,
) -> Result<PlanDetail, PmWriteError> {
    let title = req.title.trim().to_string();
    if title.is_empty() {
        return Err(PmWriteError::Validation("title is required"));
    }
    validate_plan_tasks(&req.tasks).map_err(PmWriteError::ValidationMsg)?;
    // 基线必须存在且属本项目（Q4；允许缺省=先建计划后冻结基线再拆解）。
    if let Some(baseline_id) = req.source_baseline_id {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "SELECT project_id FROM design.implementation_baseline \
             WHERE id = $1 AND tenant_id = $2",
        )
        .bind(baseline_id)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .map_err(PmWriteError::Db)?;
        if owner != Some(project_id) {
            return Err(PmWriteError::Validation(
                "source_baseline_id does not belong to this project",
            ));
        }
    }
    let content = build_revision_content(&req.tasks);
    let content_hash = content_hash_of(&content);

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let source = crate::design::principal_is_human(&mut tx, actor.principal_id).await?;
    let code = match req.code.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(c) => c.to_string(),
        None => next_plan_code(&mut tx, tenant_id, project_id).await?,
    };
    let plan_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO plan.task_plan \
         (id, tenant_id, project_id, code, title, source_baseline_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(plan_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(&code)
    .bind(&title)
    .bind(req.source_baseline_id)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            PmWriteError::Conflict("task plan code already exists in this project")
        } else {
            PmWriteError::Db(e)
        }
    })?;
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO plan.task_plan_revision \
         (id, tenant_id, project_id, plan_id, version, content, state, content_hash, \
          source, instructions, created_by) \
         VALUES ($1, $2, $3, $4, 1, $5, 'draft', $6, $7, $8, $9)",
    )
    .bind(revision_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(plan_id)
    .bind(&content)
    .bind(&content_hash)
    .bind(if source { "human" } else { "ai" })
    .bind(req.instructions.as_deref().filter(|s| !s.trim().is_empty()))
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    sqlx::query(
        "UPDATE plan.task_plan SET current_revision_id = $1 WHERE id = $2 AND tenant_id = $3",
    )
    .bind(revision_id)
    .bind(plan_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "task_plan.created",
        "task_plan",
        Some(plan_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    plan_detail(pool, tenant_id, plan_id).await
}

/// 新计划修订草案（版本 = max+1；重拆与人工修订共用入口）。
pub async fn create_plan_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    plan_id: Uuid,
    req: DraftPlanRevisionRequest,
    actor: &AuditActor,
) -> Result<TaskPlanRevisionView, PmWriteError> {
    validate_plan_tasks(&req.tasks).map_err(PmWriteError::ValidationMsg)?;
    let content = build_revision_content(&req.tasks);
    let content_hash = content_hash_of(&content);

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, status, _, _) = lock_plan(&mut tx, tenant_id, plan_id).await?;
    if !parse_status(&status)?.content_editable() {
        return Err(PmWriteError::Conflict(
            "plan revisions can only be drafted while the plan is in draft (return it first)",
        ));
    }
    let source = crate::design::principal_is_human(&mut tx, actor.principal_id).await?;
    let version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM plan.task_plan_revision \
         WHERE plan_id = $1 AND tenant_id = $2",
    )
    .bind(plan_id)
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO plan.task_plan_revision \
         (id, tenant_id, project_id, plan_id, version, content, state, content_hash, \
          source, instructions, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, 'draft', $7, $8, $9, $10)",
    )
    .bind(revision_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(plan_id)
    .bind(version)
    .bind(&content)
    .bind(&content_hash)
    .bind(if source { "human" } else { "ai" })
    .bind(req.instructions.as_deref().filter(|s| !s.trim().is_empty()))
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    sqlx::query(
        "UPDATE plan.task_plan SET current_revision_id = $1, updated_at = now() \
         WHERE id = $2 AND tenant_id = $3",
    )
    .bind(revision_id)
    .bind(plan_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "task_plan_revision.created",
        "task_plan_revision",
        Some(revision_id),
        json!({ "version": version, "content_hash": content_hash }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_revision_view(pool, tenant_id, plan_id, revision_id).await
}

/// 改稿（仅 draft 修订；库级触发器双闸）。
pub async fn update_plan_revision_draft(
    pool: &PgPool,
    tenant_id: Uuid,
    plan_id: Uuid,
    revision_id: Uuid,
    req: DraftPlanRevisionRequest,
    actor: &AuditActor,
) -> Result<TaskPlanRevisionView, PmWriteError> {
    validate_plan_tasks(&req.tasks).map_err(PmWriteError::ValidationMsg)?;
    let content = build_revision_content(&req.tasks);
    let content_hash = content_hash_of(&content);

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_plan(&mut tx, tenant_id, plan_id).await?;
    let (version, state, _, _) =
        lock_plan_revision(&mut tx, tenant_id, plan_id, revision_id).await?;
    if !parse_revision_state(&state)?.content_editable() {
        return Err(PmWriteError::Conflict(
            "only draft plan revisions can be edited",
        ));
    }
    sqlx::query(
        "UPDATE plan.task_plan_revision SET content = $1, content_hash = $2 \
         WHERE id = $3 AND plan_id = $4 AND tenant_id = $5 AND state = 'draft'",
    )
    .bind(&content)
    .bind(&content_hash)
    .bind(revision_id)
    .bind(plan_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "task_plan_revision.updated",
        "task_plan_revision",
        Some(revision_id),
        json!({ "version": version, "content_hash": content_hash }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_revision_view(pool, tenant_id, plan_id, revision_id).await
}

/// 提交审核：修订 draft→submitted + 计划 draft→in_review（AI/人类皆可提交）。
pub async fn submit_plan_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    plan_id: Uuid,
    revision_id: Uuid,
    actor: &AuditActor,
) -> Result<TaskPlanRevisionView, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, status, _, _) = lock_plan(&mut tx, tenant_id, plan_id).await?;
    let from_plan = parse_status(&status)?;
    transition_or_conflict(
        TaskPlanStatus::can_transition_to,
        from_plan,
        TaskPlanStatus::InReview,
        "illegal plan status transition to in_review",
    )?;
    let (version, state, content_hash, _) =
        lock_plan_revision(&mut tx, tenant_id, plan_id, revision_id).await?;
    let from_rev = parse_revision_state(&state)?;
    transition_or_conflict(
        TaskPlanRevisionState::can_transition_to,
        from_rev,
        TaskPlanRevisionState::Submitted,
        "illegal plan revision transition to submitted",
    )?;
    sqlx::query(
        "UPDATE plan.task_plan_revision \
         SET state = 'submitted', submitted_at = now() \
         WHERE id = $1 AND plan_id = $2 AND tenant_id = $3 AND state = 'draft'",
    )
    .bind(revision_id)
    .bind(plan_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    sqlx::query(
        "UPDATE plan.task_plan SET status = 'in_review', updated_at = now() \
         WHERE id = $1 AND tenant_id = $2",
    )
    .bind(plan_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "task_plan.submitted",
        "task_plan_revision",
        Some(revision_id),
        json!({ "version": version, "content_hash": content_hash }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_revision_view(pool, tenant_id, plan_id, revision_id).await
}

// ─────────────────────────────────────────────────────── 审批（真人门）

/// 请求批准当前修订（AI/人类皆可发起；每计划至多一条 pending）。
pub async fn request_plan_approval(
    pool: &PgPool,
    tenant_id: Uuid,
    plan_id: Uuid,
    actor: &AuditActor,
) -> Result<ApprovalView, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, status, current_revision_id, _) =
        lock_plan(&mut tx, tenant_id, plan_id).await?;
    if status != TaskPlanStatus::InReview.as_str() {
        return Err(PmWriteError::Conflict(
            "only in_review plans can be requested for approval",
        ));
    }
    let content_hash: String = sqlx::query_scalar(
        "SELECT content_hash FROM plan.task_plan_revision \
         WHERE id = $1 AND tenant_id = $2 AND plan_id = $3",
    )
    .bind(current_revision_id)
    .bind(tenant_id)
    .bind(plan_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::Conflict("plan has no submitted revision"))?;
    let approval_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.design_approval \
         (id, tenant_id, project_id, kind, plan_id, target_id, target_hash, requested_by) \
         VALUES ($1, $2, $3, 'task_plan', $4, $5, $6, $7)",
    )
    .bind(approval_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(plan_id)
    .bind(current_revision_id)
    .bind(&content_hash)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            PmWriteError::Conflict("an approval is already pending for this plan")
        } else {
            PmWriteError::Db(e)
        }
    })?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "task_plan.approval.requested",
        "design_approval",
        Some(approval_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    crate::design::fetch_approval_view(pool, tenant_id, approval_id).await
}

/// 计划审批裁决（唯一批准入口；真人主体；由 design::decide_approval 分派）。
/// approve：修订 approved + 计划 approved；reject：修订 rejected + 计划退回 draft。
pub(crate) async fn apply_plan_decision(
    pool: &PgPool,
    tenant_id: Uuid,
    plan_id: Uuid,
    approve: bool,
    actor: &AuditActor,
) -> Result<(), PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    let (project_id, status, current_revision_id, _) =
        lock_plan(&mut tx, tenant_id, plan_id).await?;
    let from_plan = parse_status(&status)?;
    let to_plan = if approve {
        TaskPlanStatus::Approved
    } else {
        TaskPlanStatus::Draft
    };
    transition_or_conflict(
        TaskPlanStatus::can_transition_to,
        from_plan,
        to_plan,
        "illegal plan status transition on decision",
    )?;
    let (version, state, _, _) = lock_plan_revision(
        &mut tx,
        tenant_id,
        plan_id,
        current_revision_id.ok_or(PmWriteError::Conflict("plan has no current revision"))?,
    )
    .await?;
    let from_rev = parse_revision_state(&state)?;
    let to_rev = if approve {
        TaskPlanRevisionState::Approved
    } else {
        TaskPlanRevisionState::Rejected
    };
    transition_or_conflict(
        TaskPlanRevisionState::can_transition_to,
        from_rev,
        to_rev,
        "illegal plan revision transition on decision",
    )?;
    if approve {
        // 新修订批准：旧批准修订退位（历史保留）。
        sqlx::query(
            "UPDATE plan.task_plan_revision SET state = 'superseded' \
             WHERE plan_id = $1 AND tenant_id = $2 AND state = 'approved' AND id <> $3",
        )
        .bind(plan_id)
        .bind(tenant_id)
        .bind(current_revision_id)
        .execute(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
        sqlx::query(
            "UPDATE plan.task_plan_revision \
             SET state = 'approved', approved_by = $1, approved_at = now() \
             WHERE id = $2 AND plan_id = $3 AND tenant_id = $4 AND state = 'submitted'",
        )
        .bind(actor.principal_id)
        .bind(current_revision_id)
        .bind(plan_id)
        .bind(tenant_id)
        .execute(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
    } else {
        sqlx::query(
            "UPDATE plan.task_plan_revision SET state = 'rejected' \
             WHERE id = $1 AND plan_id = $2 AND tenant_id = $3 AND state = 'submitted'",
        )
        .bind(current_revision_id)
        .bind(plan_id)
        .bind(tenant_id)
        .execute(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
    }
    sqlx::query(
        "UPDATE plan.task_plan SET status = $1, updated_at = now() \
         WHERE id = $2 AND tenant_id = $3",
    )
    .bind(to_plan.as_str())
    .bind(plan_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "task_plan.decided",
        "task_plan",
        Some(plan_id),
        json!({
            "decision": if approve { "approved" } else { "rejected" },
            "revision_id": current_revision_id,
            "version": version,
        }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    Ok(())
}

/// 计划替代（approved → superseded；后继必须已批准；真人门）。
pub async fn supersede_plan(
    pool: &PgPool,
    tenant_id: Uuid,
    plan_id: Uuid,
    req: SupersedePlanRequest,
    actor: &AuditActor,
) -> Result<TaskPlanEntry, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    if req.successor_plan_id == plan_id {
        return Err(PmWriteError::Validation("a plan cannot supersede itself"));
    }
    let (project_id, status, _, _) = lock_plan(&mut tx, tenant_id, plan_id).await?;
    transition_or_conflict(
        TaskPlanStatus::can_transition_to,
        parse_status(&status)?,
        TaskPlanStatus::Superseded,
        "illegal plan status transition to superseded",
    )?;
    let (s_project, s_status): (Uuid, String) = sqlx::query_as(
        "SELECT project_id, status FROM plan.task_plan WHERE id = $1 AND tenant_id = $2",
    )
    .bind(req.successor_plan_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::Validation("successor plan not found"))?;
    if s_project != project_id {
        return Err(PmWriteError::Validation(
            "successor plan is in another project",
        ));
    }
    if s_status != TaskPlanStatus::Approved.as_str() {
        return Err(PmWriteError::Validation("successor plan must be approved"));
    }
    sqlx::query(
        "UPDATE plan.task_plan SET status = 'superseded', updated_at = now() \
         WHERE id = $1 AND tenant_id = $2",
    )
    .bind(plan_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "task_plan.superseded",
        "task_plan",
        Some(plan_id),
        json!({ "successor_plan_id": req.successor_plan_id, "reason": req.reason }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    sqlx::query_as(&format!(
        "{PLAN_VIEW_COLS} {PLAN_FROM} WHERE p.id = $1 AND p.tenant_id = $2"
    ))
    .bind(plan_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

// ─────────────────────────────────────────────── 正式化（批准 → 任务）

/// 正式化：按批准修订的草案批量创建正式任务（幂等）。
///
/// 允许 AI 主体在计划批准后触发（机械转写已批准内容，无裁量）；任务一律
/// `draft`，`plan_revision_id`/`plan_task_key` 落库，验收标准同事务写入；
/// 仓库绑定校验属本项目 + verified；依赖边指向批内更早键对应任务。
pub async fn materialize_plan(
    pool: &PgPool,
    tenant_id: Uuid,
    plan_id: Uuid,
    req: MaterializeRequest,
    actor: &AuditActor,
) -> Result<Value, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, status, current, _) = lock_plan(&mut tx, tenant_id, plan_id).await?;
    if status != TaskPlanStatus::Approved.as_str() {
        // 未批准计划不得产生正式任务（红线）。
        return Err(PmWriteError::Conflict(
            "only approved plans can be materialized into formal tasks",
        ));
    }
    let revision_id = req
        .plan_revision_id
        .or(current)
        .ok_or(PmWriteError::Conflict("plan has no current revision"))?;
    let (_, state, _, content) =
        lock_plan_revision(&mut tx, tenant_id, plan_id, revision_id).await?;
    if state != TaskPlanRevisionState::Approved.as_str() {
        return Err(PmWriteError::Conflict(
            "only approved plan revisions can be materialized",
        ));
    }
    let tasks: Vec<PlanTaskDraft> = serde_json::from_value(
        content
            .get("tasks")
            .cloned()
            .ok_or_else(|| PmWriteError::Validation("revision content.tasks missing"))?,
    )
    .map_err(|_e| PmWriteError::Validation("revision content.tasks malformed"))?;
    let selected: Vec<&PlanTaskDraft> = match &req.task_keys {
        None => tasks.iter().collect(),
        Some(keys) => {
            let wanted: std::collections::HashSet<&str> = keys.iter().map(|k| k.trim()).collect();
            let picked: Vec<&PlanTaskDraft> = tasks
                .iter()
                .filter(|t| wanted.contains(t.key.trim()))
                .collect();
            if picked.len() != wanted.len() {
                return Err(PmWriteError::Validation(
                    "task_keys contains keys not present in the revision",
                ));
            }
            picked
        }
    };
    let mut created = Vec::new();
    let mut existing = Vec::new();
    let mut key_to_task: std::collections::HashMap<String, Uuid> = std::collections::HashMap::new();
    // 先收集批内已正式化任务（依赖边可能指向它们）。
    for t in &tasks {
        let already: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM delivery.task \
             WHERE tenant_id = $1 AND plan_revision_id = $2 AND plan_task_key = $3",
        )
        .bind(tenant_id)
        .bind(revision_id)
        .bind(t.key.trim())
        .fetch_optional(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
        if let Some(id) = already {
            key_to_task.insert(t.key.trim().to_string(), id);
        }
    }
    for t in &selected {
        let key = t.key.trim().to_string();
        if key_to_task.contains_key(&key) {
            existing.push(key.clone());
            continue;
        }
        let task_id = Uuid::new_v4();
        let priority = t.priority.as_deref().unwrap_or("normal");
        sqlx::query(
            "INSERT INTO delivery.task \
             (id, project_id, title, description, status, priority, revision, tenant_id, \
              implementation_baseline_id, plan_revision_id, plan_task_key) \
             VALUES ($1, $2, $3, $4, 'draft', $5, 1, \
                     (SELECT tenant_id FROM core.project WHERE id = $2), \
                     (SELECT source_baseline_id FROM plan.task_plan WHERE id = $6), $7, $8)",
        )
        .bind(task_id)
        .bind(project_id)
        .bind(t.title.trim())
        .bind(t.description.as_deref().filter(|s| !s.trim().is_empty()))
        .bind(priority)
        .bind(plan_id)
        .bind(revision_id)
        .bind(&key)
        .execute(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
        // 验收标准同事务（ordinal 按批内序 ×10，与 pm_write 纪律一致）。
        for (i, criterion) in t.acceptance_criteria.iter().enumerate() {
            sqlx::query(
                "INSERT INTO delivery.task_acceptance_criterion \
                 (id, project_id, task_id, ordinal, text, verification_kind, tenant_id) \
                 VALUES ($1, $2, $3, $4, $5, 'manual', \
                         (SELECT tenant_id FROM core.project WHERE id = $2))",
            )
            .bind(Uuid::new_v4())
            .bind(project_id)
            .bind(task_id)
            .bind(((i + 1) * 10) as i32)
            .bind(criterion.trim())
            .execute(&mut *tx)
            .await
            .map_err(PmWriteError::Db)?;
        }
        // 仓库建议绑定（校验属本项目 + verified；失败不阻断——建议绑定可后补）。
        for repo_id in &t.repository_ids {
            let ok: Option<i32> = sqlx::query_scalar(
                "SELECT 1 FROM integration.project_repository \
                 WHERE id = $1 AND tenant_id = $2 AND project_id = $3 AND status = 'verified'",
            )
            .bind(repo_id)
            .bind(tenant_id)
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(PmWriteError::Db)?;
            if ok.is_some() {
                sqlx::query(
                    "INSERT INTO delivery.task_repository \
                     (task_id, project_id, repository_id, write_access, base_ref, role, tenant_id) \
                     VALUES ($1, $2, $3, TRUE, 'main', 'primary', $4) \
                     ON CONFLICT DO NOTHING",
                )
                .bind(task_id)
                .bind(project_id)
                .bind(repo_id)
                .bind(tenant_id)
                .execute(&mut *tx)
                .await
                .map_err(PmWriteError::Db)?;
            }
        }
        // 依赖边（仅批内更早键；add_dependency 校验在此图锁语境不可复用——
        // 直接插边 + 环由前向图结构保证不存在）。
        for dep in &t.depends_on {
            if let Some(dep_task) = key_to_task.get(dep.trim()) {
                sqlx::query(
                    "INSERT INTO delivery.task_dependency \
                     (id, tenant_id, project_id, task_id, depends_on_task_id, created_by) \
                     VALUES ($1, $2, $3, $4, $5, $6) \
                     ON CONFLICT DO NOTHING",
                )
                .bind(Uuid::new_v4())
                .bind(tenant_id)
                .bind(project_id)
                .bind(task_id)
                .bind(dep_task)
                .bind(actor.principal_id)
                .execute(&mut *tx)
                .await
                .map_err(PmWriteError::Db)?;
            }
        }
        key_to_task.insert(key.clone(), task_id);
        created.push(json!({ "key": key, "task_id": task_id }));
    }
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "task_plan.materialized",
        "task_plan",
        Some(plan_id),
        json!({
            "revision_id": revision_id,
            "created": created.len(),
            "existing": existing.len(),
        }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    Ok(json!({ "created": created, "already_materialized": existing }))
}

// ─────────────────────────────────────────── 交接（S05）与进度读模型

/// 执行资格检查 + 执行上下文载荷（§3.3）。
///
/// 红线：未批准计划/非 ready 任务/依赖未完成/仓库绑定无效 → 不产生交接载荷。
#[derive(Debug, Deserialize)]
pub struct HandoffRequest {
    pub task_id: Uuid,
}

pub async fn prepare_handoff(
    pool: &PgPool,
    tenant_id: Uuid,
    task_id: Uuid,
) -> Result<Value, PmWriteError> {
    // 任务归属 + 计划锚定（跨租户按不存在处理）。
    type HandoffTaskRow = (Uuid, String, String, Option<Uuid>, Option<String>);
    let row: Option<HandoffTaskRow> = sqlx::query_as(
        "SELECT t.project_id, t.status, t.title, t.plan_revision_id, t.plan_task_key \
         FROM delivery.task t WHERE t.id = $1 AND t.tenant_id = $2",
    )
    .bind(task_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let Some((project_id, status, title, plan_revision_id, plan_task_key)) = row else {
        return Err(PmWriteError::NotFound("task not found"));
    };
    if status != "ready" {
        return Err(PmWriteError::Conflict(
            "task must be ready before execution handoff",
        ));
    }
    // 来源计划必须已批准（追溯链完整）。
    if let Some(rev_id) = plan_revision_id {
        let (plan_status, plan_id): (Option<String>, Option<Uuid>) = sqlx::query_as(
            "SELECT p.status, p.id FROM plan.task_plan_revision rv \
             JOIN plan.task_plan p ON p.id = rv.plan_id \
             WHERE rv.id = $1 AND rv.tenant_id = $2",
        )
        .bind(rev_id)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .map_err(PmWriteError::Db)?
        .map(|(s, id)| (Some(s), Some(id)))
        .unwrap_or((None, None));
        match plan_status.as_deref() {
            Some(s) if s == TaskPlanStatus::Approved.as_str() => {}
            Some(_) => {
                return Err(PmWriteError::Conflict(
                    "the source task plan is not approved",
                ))
            }
            None => return Err(PmWriteError::NotFound("source plan revision not found")),
        }
        let _ = plan_id;
    }
    // 前置依赖全部 completed。
    let incomplete: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM delivery.task_dependency d \
         JOIN delivery.task up ON up.id = d.depends_on_task_id \
         WHERE d.task_id = $1 AND up.status <> 'completed'",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;
    if incomplete > 0 {
        return Err(PmWriteError::Conflict(
            "upstream dependencies are not completed",
        ));
    }
    // 验收标准非空（可判定完成条件）。
    let criteria: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM delivery.task_acceptance_criterion WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;
    if criteria == 0 {
        return Err(PmWriteError::Conflict(
            "task has no acceptance criteria; handoff requires verifiable completion conditions",
        ));
    }
    // 仓库绑定有效性（绑定的仓必须仍属本项目且 verified）。
    let invalid_repos: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM delivery.task_repository tr \
         LEFT JOIN integration.project_repository pr \
           ON pr.id = tr.repository_id AND pr.project_id = tr.project_id \
         WHERE tr.task_id = $1 AND (pr.id IS NULL OR pr.status <> 'verified')",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;
    if invalid_repos > 0 {
        return Err(PmWriteError::Conflict(
            "task has invalid (cross-project or unverified) repository bindings",
        ));
    }
    // 上下文载荷（来源追溯 + 基线 + 验收 + 仓库 + 依赖）。
    let baseline_id: Option<Uuid> =
        sqlx::query_scalar("SELECT implementation_baseline_id FROM delivery.task WHERE id = $1")
            .bind(task_id)
            .fetch_one(pool)
            .await
            .map_err(PmWriteError::Db)?;
    let criteria_list: Vec<Value> = sqlx::query(
        "SELECT id, ordinal, text, verification_kind \
         FROM delivery.task_acceptance_criterion WHERE task_id = $1 ORDER BY ordinal",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?
    .into_iter()
    .map(|row| {
        use sqlx::Row;
        json!({
            "id": row.get::<Uuid, _>("id"),
            "ordinal": row.get::<i32, _>("ordinal"),
            "statement": row.get::<String, _>("text"),
            "verification_kind": row.get::<String, _>("verification_kind"),
        })
    })
    .collect();
    let repositories: Vec<Value> = sqlx::query(
        "SELECT tr.repository_id, tr.role, tr.write_access, tr.base_ref, tr.base_commit, pr.name \
         FROM delivery.task_repository tr \
         JOIN integration.project_repository pr ON pr.id = tr.repository_id \
         WHERE tr.task_id = $1",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?
    .into_iter()
    .map(|row| {
        use sqlx::Row;
        json!({
            "repository_id": row.get::<Uuid, _>("repository_id"),
            "name": row.get::<String, _>("name"),
            "role": row.get::<String, _>("role"),
            "write_access": row.get::<bool, _>("write_access"),
            "base_ref": row.get::<String, _>("base_ref"),
            "base_commit": row.get::<Option<String>, _>("base_commit"),
        })
    })
    .collect();
    let dependencies: Vec<Value> = sqlx::query(
        "SELECT d.depends_on_task_id, up.title, up.status \
         FROM delivery.task_dependency d \
         JOIN delivery.task up ON up.id = d.depends_on_task_id \
         WHERE d.task_id = $1",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?
    .into_iter()
    .map(|row| {
        use sqlx::Row;
        json!({
            "task_id": row.get::<Uuid, _>("depends_on_task_id"),
            "title": row.get::<String, _>("title"),
            "status": row.get::<String, _>("status"),
        })
    })
    .collect();
    Ok(json!({
        "task_id": task_id,
        "project_id": project_id,
        "title": title,
        "status": status,
        "plan_revision_id": plan_revision_id,
        "plan_task_key": plan_task_key,
        "implementation_baseline_id": baseline_id,
        "acceptance_criteria": criteria_list,
        "repositories": repositories,
        "dependencies": dependencies,
        "expected_deliverables": [],
    }))
}

#[derive(Debug, Serialize)]
pub struct PlanProgress {
    pub plan_id: Uuid,
    pub status: String,
    pub revision_id: Option<Uuid>,
    pub total_tasks: i64,
    pub by_status: Value,
    pub blocked: Vec<Value>,
    /// 草案任务数（批准修订内）。
    pub planned_tasks: i64,
}

/// 计划整体进度 + 阻塞清单（§3.4）。
pub async fn plan_progress(
    pool: &PgPool,
    tenant_id: Uuid,
    plan_id: Uuid,
) -> Result<PlanProgress, PmWriteError> {
    let plan: TaskPlanEntry = sqlx::query_as(&format!(
        "{PLAN_VIEW_COLS} {PLAN_FROM} WHERE p.tenant_id = $1 AND p.id = $2"
    ))
    .bind(tenant_id)
    .bind(plan_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("task plan not found"))?;
    let mut planned_tasks = 0i64;
    if let Some(rev_id) = plan.current_revision_id {
        let content: Option<Value> = sqlx::query_scalar(
            "SELECT content FROM plan.task_plan_revision WHERE id = $1 AND tenant_id = $2",
        )
        .bind(rev_id)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .map_err(PmWriteError::Db)?;
        if let Some(content) = content {
            if let Some(tasks) = content.get("tasks").and_then(Value::as_array) {
                planned_tasks = tasks.len() as i64;
            }
        }
    }
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT t.status, COUNT(*) FROM delivery.task t \
         WHERE t.tenant_id = $1 AND t.plan_revision_id IN ( \
             SELECT id FROM plan.task_plan_revision WHERE plan_id = $2 AND tenant_id = $1) \
         GROUP BY t.status",
    )
    .bind(tenant_id)
    .bind(plan_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let total_tasks: i64 = rows.iter().map(|(_, n)| n).sum();
    let mut by_status = serde_json::Map::new();
    for (status, n) in &rows {
        by_status.insert(status.clone(), json!(n));
    }
    let blocked: Vec<Value> = sqlx::query(
        "SELECT t.id, t.title, t.status, \
                (SELECT COUNT(*) FROM delivery.task_dependency d \
                 JOIN delivery.task up ON up.id = d.depends_on_task_id \
                 WHERE d.task_id = t.id AND up.status <> 'completed') AS open_upstreams \
         FROM delivery.task t \
         WHERE t.tenant_id = $1 AND t.status = 'blocked' \
           AND t.plan_revision_id IN ( \
               SELECT id FROM plan.task_plan_revision WHERE plan_id = $2 AND tenant_id = $1)",
    )
    .bind(tenant_id)
    .bind(plan_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?
    .into_iter()
    .map(|row| {
        use sqlx::Row;
        json!({
            "task_id": row.get::<Uuid, _>("id"),
            "title": row.get::<String, _>("title"),
            "status": row.get::<String, _>("status"),
            "open_upstreams": row.get::<i64, _>("open_upstreams"),
        })
    })
    .collect();
    Ok(PlanProgress {
        plan_id,
        status: plan.status,
        revision_id: plan.current_revision_id,
        total_tasks,
        by_status: Value::Object(by_status),
        blocked,
        planned_tasks,
    })
}

// 常量占位引用（DECIDE_APPROVAL_ACTIONS 由 design 模块导出的动作词表）。
const _: [&str; 2] = DECIDE_APPROVAL_ACTIONS;
