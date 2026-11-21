//! Issue / Risk / Blocker 领域服务（PA-05-01 收尾；2026-11-21 V1 最小集）。
//!
//! PA-05-01 列明的三个独立领域对象落到 delivery schema（0052）：
//!   - Issue {severity: low|medium|high; status: open → resolved|cancelled}；
//!   - Risk {impact/probability: low|medium|high; status: identified →
//!     mitigating → closed}；
//!   - Blocker {blocked_task_id?: 阻塞对象引用; status: open → cleared}。
//!
//! 统一基础能力（PA-05-01/02/05）：创建即注册 `core.work_item`（统一事项
//! ID）并记 Timeline `created` 活动；状态迁移经显式语义映射同步统一状态，
//! 语义变化记 `status_changed`（§8.3 合并规则）。业务规则从简：终态完整
//! 性由 DB CHECK 强制；解除阻塞不联动 task 状态（blocked_task_id 仅引用，
//! task 侧 blocked 状态仍走 task_dependency 既有链路，V1 不做双向联动）。

use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::work_item::{
    blocker_status_semantic, ensure_work_item, issue_status_semantic, record_activity,
    risk_status_semantic, WorkItemError,
};

#[derive(Debug, thiserror::Error)]
pub enum WorkDomainError {
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Validation(&'static str),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

impl From<WorkItemError> for WorkDomainError {
    fn from(err: WorkItemError) -> Self {
        match err {
            WorkItemError::Db(e) => WorkDomainError::Db(e),
            WorkItemError::NotFound => WorkDomainError::NotFound,
            WorkItemError::Validation(_) => {
                WorkDomainError::Validation("work item target rejected")
            }
        }
    }
}


const SEVERITY: [&str; 3] = ["low", "medium", "high"];

fn severity_ok(v: &str) -> bool {
    SEVERITY.contains(&v)
}

// ─────────────────────────────────────────────────────────── Issue

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct IssueRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub severity: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create_issue(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    title: &str,
    description: Option<&str>,
    severity: Option<&str>,
    actor: &AuditActor,
) -> Result<Uuid, WorkDomainError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(WorkDomainError::Validation("title is required"));
    }
    let severity = severity.unwrap_or("medium");
    if !severity_ok(severity) {
        return Err(WorkDomainError::Validation("invalid severity"));
    }
    let mut tx = pool.begin().await?;
    let issue_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.issue \
         (tenant_id, project_id, title, description, severity, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(title)
    .bind(description.map(str::trim).filter(|s| !s.is_empty()))
    .bind(severity)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;
    domain_wiring(
        &mut tx,
        tenant_id,
        project_id,
        "issue",
        issue_id,
        title,
        issue_status_semantic("open"),
        "Issue created",
        actor.principal_id,
    )
    .await?;
    tx.commit().await?;
    Ok(issue_id)
}

/// Issue 状态迁移（resolved 需配 reason 语义=处置说明入 description 追加？
/// V1 简化：resolve 记活动即可，cancel 必填理由）。
pub async fn transition_issue(
    pool: &PgPool,
    tenant_id: Uuid,
    issue_id: Uuid,
    to: &str,
    reason: Option<&str>,
    actor: &AuditActor,
) -> Result<(), WorkDomainError> {
    if !matches!(to, "resolved" | "cancelled") {
        return Err(WorkDomainError::Validation("target must be resolved|cancelled"));
    }
    if to == "cancelled" && reason.map(str::trim).unwrap_or_default().is_empty() {
        return Err(WorkDomainError::Validation("reason is required"));
    }
    let mut tx = pool.begin().await?;
    let row: Option<(Uuid, String, String)> = sqlx::query_as(
        "SELECT project_id, title, status FROM delivery.issue \
          WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(issue_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((project_id, title, status_before)) = row else {
        return Err(WorkDomainError::NotFound);
    };
    if status_before != "open" {
        return Err(WorkDomainError::Validation("only open issues can transition"));
    }
    let sql = if to == "resolved" {
        "UPDATE delivery.issue SET status = 'resolved', resolved_at = now(), \
         updated_at = now() WHERE id = $1"
    } else {
        "UPDATE delivery.issue SET status = 'cancelled', cancelled_reason = $2, \
         updated_at = now() WHERE id = $1"
    };
    let mut query = sqlx::query(sql).bind(issue_id);
    if to == "cancelled" {
        query = query.bind(reason);
    }
    query.execute(&mut *tx).await?;
    semantic_transition(
        &mut tx,
        tenant_id,
        project_id,
        "issue",
        issue_id,
        &title,
        issue_status_semantic(&status_before),
        issue_status_semantic(to),
        actor.principal_id,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn list_issues(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    status: Option<&str>,
) -> Result<Vec<IssueRow>, WorkDomainError> {
    let rows = sqlx::query_as::<_, IssueRow>(
        "SELECT id, project_id, title, description, severity, status, created_at \
           FROM delivery.issue \
          WHERE tenant_id = $1 AND project_id = $2 \
            AND ($3::text IS NULL OR status = $3) \
          ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(status)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ─────────────────────────────────────────────────────────── Risk

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct RiskRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub impact: String,
    pub probability: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create_risk(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    title: &str,
    description: Option<&str>,
    impact: Option<&str>,
    probability: Option<&str>,
    actor: &AuditActor,
) -> Result<Uuid, WorkDomainError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(WorkDomainError::Validation("title is required"));
    }
    let impact = impact.unwrap_or("medium");
    let probability = probability.unwrap_or("medium");
    if !severity_ok(impact) || !severity_ok(probability) {
        return Err(WorkDomainError::Validation("invalid impact|probability"));
    }
    let mut tx = pool.begin().await?;
    let risk_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.risk \
         (tenant_id, project_id, title, description, impact, probability, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(title)
    .bind(description.map(str::trim).filter(|s| !s.is_empty()))
    .bind(impact)
    .bind(probability)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;
    domain_wiring(
        &mut tx,
        tenant_id,
        project_id,
        "risk",
        risk_id,
        title,
        risk_status_semantic("identified"),
        "Risk identified",
        actor.principal_id,
    )
    .await?;
    tx.commit().await?;
    Ok(risk_id)
}

/// Risk 状态推进（identified → mitigating → closed；不可跳跃/回退——
/// V1 严格线性，closed 终态）。
pub async fn transition_risk(
    pool: &PgPool,
    tenant_id: Uuid,
    risk_id: Uuid,
    to: &str,
    actor: &AuditActor,
) -> Result<(), WorkDomainError> {
    if !matches!(to, "mitigating" | "closed") {
        return Err(WorkDomainError::Validation(
            "target must be mitigating|closed",
        ));
    }
    let mut tx = pool.begin().await?;
    let row: Option<(Uuid, String, String)> = sqlx::query_as(
        "SELECT project_id, title, status FROM delivery.risk \
          WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(risk_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((project_id, title, status_before)) = row else {
        return Err(WorkDomainError::NotFound);
    };
    let allowed = matches!(
        (status_before.as_str(), to),
        ("identified", "mitigating") | ("identified", "closed") | ("mitigating", "closed")
    );
    if !allowed {
        return Err(WorkDomainError::Validation("illegal risk transition"));
    }
    let sql = if to == "closed" {
        "UPDATE delivery.risk SET status = 'closed', closed_at = now(), updated_at = now() \
         WHERE id = $1"
    } else {
        "UPDATE delivery.risk SET status = 'mitigating', updated_at = now() WHERE id = $1"
    };
    sqlx::query(sql)
        .bind(risk_id)
        .execute(&mut *tx)
        .await?;
    semantic_transition(
        &mut tx,
        tenant_id,
        project_id,
        "risk",
        risk_id,
        &title,
        risk_status_semantic(&status_before),
        risk_status_semantic(to),
        actor.principal_id,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn list_risks(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    status: Option<&str>,
) -> Result<Vec<RiskRow>, WorkDomainError> {
    let rows = sqlx::query_as::<_, RiskRow>(
        "SELECT id, project_id, title, description, impact, probability, status, created_at \
           FROM delivery.risk \
          WHERE tenant_id = $1 AND project_id = $2 \
            AND ($3::text IS NULL OR status = $3) \
          ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(status)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ─────────────────────────────────────────────────────────── Blocker

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct BlockerRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub blocked_task_id: Option<Uuid>,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create_blocker(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    title: &str,
    description: Option<&str>,
    blocked_task_id: Option<Uuid>,
    actor: &AuditActor,
) -> Result<Uuid, WorkDomainError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(WorkDomainError::Validation("title is required"));
    }
    let mut tx = pool.begin().await?;
    if let Some(task_id) = blocked_task_id {
        let ok: Option<bool> = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM delivery.task \
              WHERE id = $1 AND project_id = $2 AND tenant_id = $3)",
        )
        .bind(task_id)
        .bind(project_id)
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await?;
        if ok != Some(true) {
            return Err(WorkDomainError::Validation(
                "blocked_task_id must reference a task in this project",
            ));
        }
    }
    let blocker_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.blocker \
         (tenant_id, project_id, title, description, blocked_task_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(title)
    .bind(description.map(str::trim).filter(|s| !s.is_empty()))
    .bind(blocked_task_id)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;
    domain_wiring(
        &mut tx,
        tenant_id,
        project_id,
        "blocker",
        blocker_id,
        title,
        blocker_status_semantic("open"),
        "Blocker raised",
        actor.principal_id,
    )
    .await?;
    tx.commit().await?;
    Ok(blocker_id)
}

/// 清除阻塞（open → cleared；终态）。
pub async fn clear_blocker(
    pool: &PgPool,
    tenant_id: Uuid,
    blocker_id: Uuid,
    actor: &AuditActor,
) -> Result<(), WorkDomainError> {
    let mut tx = pool.begin().await?;
    let row: Option<(Uuid, String, String)> = sqlx::query_as(
        "SELECT project_id, title, status FROM delivery.blocker \
          WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(blocker_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((project_id, title, status_before)) = row else {
        return Err(WorkDomainError::NotFound);
    };
    if status_before != "open" {
        return Err(WorkDomainError::Validation("blocker already cleared"));
    }
    sqlx::query(
        "UPDATE delivery.blocker SET status = 'cleared', cleared_at = now(), updated_at = now() \
         WHERE id = $1",
    )
    .bind(blocker_id)
    .execute(&mut *tx)
    .await?;
    semantic_transition(
        &mut tx,
        tenant_id,
        project_id,
        "blocker",
        blocker_id,
        &title,
        blocker_status_semantic(&status_before),
        blocker_status_semantic("cleared"),
        actor.principal_id,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn list_blockers(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    status: Option<&str>,
) -> Result<Vec<BlockerRow>, WorkDomainError> {
    let rows = sqlx::query_as::<_, BlockerRow>(
        "SELECT id, project_id, title, description, blocked_task_id, status, created_at \
           FROM delivery.blocker \
          WHERE tenant_id = $1 AND project_id = $2 \
            AND ($3::text IS NULL OR status = $3) \
          ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(status)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ─────────────────────────────────────────────────────────── 接线辅助

/// 创建即注册（PA-05-01）+ Timeline `created` 活动（PA-05-05）。
#[allow(clippy::too_many_arguments)]
async fn domain_wiring(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    target_type: &str,
    target_id: Uuid,
    title: &str,
    semantic_status: &str,
    summary: &str,
    actor_id: Uuid,
) -> Result<(), WorkDomainError> {
    let item_id = ensure_work_item(
        tx,
        tenant_id,
        target_type,
        target_id,
        Some(project_id),
        title,
        semantic_status,
        Some(actor_id),
    )
    .await?;
    record_activity(
        tx,
        tenant_id,
        item_id,
        Some(project_id),
        None,
        "created",
        "human",
        Some(actor_id),
        &format!("{summary}: {title}"),
        None,
        None,
    )
    .await?;
    Ok(())
}

/// 状态迁移：统一状态同步 + 语义变化记 `status_changed`（§8.3 合并规则）。
#[allow(clippy::too_many_arguments)]
async fn semantic_transition(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    target_type: &str,
    target_id: Uuid,
    title: &str,
    semantic_before: &str,
    semantic_after: &str,
    actor_id: Uuid,
) -> Result<(), WorkDomainError> {
    let item_id = ensure_work_item(
        tx,
        tenant_id,
        target_type,
        target_id,
        Some(project_id),
        title,
        semantic_after,
        Some(actor_id),
    )
    .await?;
    sqlx::query("UPDATE core.work_item SET status = $1, updated_at = now() WHERE id = $2")
        .bind(semantic_after)
        .bind(item_id)
        .execute(&mut **tx)
        .await?;
    if semantic_before == semantic_after {
        return Ok(());
    }
    record_activity(
        tx,
        tenant_id,
        item_id,
        Some(project_id),
        None,
        "status_changed",
        "human",
        Some(actor_id),
        &format!("{title}: {semantic_before} → {semantic_after}"),
        Some(&json!({
            "from": semantic_before,
            "to": semantic_after,
        })),
        None,
    )
    .await?;
    Ok(())
}

// project_tenant 供 API 层守卫复用（tenant 反查）。
pub use crate::work_item::project_tenant as lookup_project_tenant;
