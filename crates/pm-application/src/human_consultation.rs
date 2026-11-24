//! Human Consultation（PA07-IMP-006；PA-07 V1.0 §10.2 / 审查85）。
//!
//! 需要人类作出正式决定的工作项——不是通用审批系统或讨论聊天室。四态最小
//! 状态机（pending/in_progress/decided/cancelled）；decided 的四元组（内容/
//! 决定人/时间 + product_scope 适用范围）由 DB CHECK 强制；既有决定被改变时
//! 追加新行（[`supersede_consultation`]），不覆盖历史；**未回应不得视为同意**
//! 由结构保证——不存在任何自动转 decided 的路径。
//!
//! 调度联动：挂 task_id 的咨询同事务写 `delivery.task_waiting('needs_decision')`
//! （claim 七门与解锁判定的现成执法点）；decide/cancel 结束等待。

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::{append_audit_with_after, PmWriteError};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct HumanConsultationRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Option<Uuid>,
    pub title: String,
    pub question: String,
    pub status: String,
    pub decision_content: Option<String>,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<chrono::DateTime<chrono::Utc>>,
    pub product_scope: serde_json::Value,
    pub scope_note: Option<String>,
    pub cancelled_reason: Option<String>,
    pub supersedes_consultation_id: Option<Uuid>,
    pub created_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug)]
pub enum ConsultationError {
    NotFound,
    /// 状态机拒绝（非法迁移或目标态相同）。
    InvalidTransition(String),
    /// decided 缺四元组（DB CHECK 兜底前的应用层校验）。
    IncompleteDecision,
    /// cancel 缺原因。
    MissingReason,
    /// 既有 decided 记录被再次 decide——必须走 supersede。
    AlreadyDecided,
    Db(sqlx::Error),
}

impl From<sqlx::Error> for ConsultationError {
    fn from(e: sqlx::Error) -> Self {
        ConsultationError::Db(e)
    }
}

impl From<PmWriteError> for ConsultationError {
    fn from(e: PmWriteError) -> Self {
        match e {
            PmWriteError::Db(err) => ConsultationError::Db(err),
            PmWriteError::NotFound(msg) => ConsultationError::InvalidTransition(msg.into()),
            other => ConsultationError::InvalidTransition(format!("{other:?}")),
        }
    }
}

/// 建咨询项（pending）。挂 task_id 时同事务写 needs_decision 等待行——
/// 决策未完成期间该任务的领取被现有 claim 门禁阻塞（§10.2）。
pub async fn create_consultation(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    task_id: Option<Uuid>,
    title: &str,
    question: &str,
    product_scope: serde_json::Value,
    scope_note: Option<&str>,
    created_by: Uuid,
    actor: &AuditActor,
) -> Result<Uuid, ConsultationError> {
    let title = title.trim();
    let question = question.trim();
    if title.is_empty() || question.is_empty() {
        return Err(ConsultationError::InvalidTransition(
            "title and question are required".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    let consultation_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO governance.human_consultation \
         (id, tenant_id, project_id, task_id, title, question, product_scope, \
          scope_note, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(consultation_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(task_id)
    .bind(title)
    .bind(question)
    .bind(&product_scope)
    .bind(scope_note)
    .bind(created_by)
    .execute(&mut *tx)
    .await?;
    if let Some(task) = task_id {
        sqlx::query(
            "INSERT INTO delivery.task_waiting \
             (project_id, task_id, reason, questions, requested_from, tenant_id) \
             VALUES ($1, $2, 'needs_decision', jsonb_build_array($3), $4, $5)",
        )
        .bind(project_id)
        .bind(task)
        .bind(title)
        .bind(created_by)
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;
    }
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "consultation.created",
        "human_consultation",
        Some(consultation_id),
        serde_json::json!({ "task_id": task_id, "title": title }),
    )
    .await?;
    tx.commit().await?;
    Ok(consultation_id)
}

/// pending → in_progress。
pub async fn start_consultation(
    pool: &PgPool,
    tenant_id: Uuid,
    consultation_id: Uuid,
    actor: &AuditActor,
) -> Result<(), ConsultationError> {
    transition_status(
        pool,
        tenant_id,
        consultation_id,
        "pending",
        "in_progress",
        None,
        None,
        None,
        actor,
    )
    .await
    .map(|_| ())
}

/// → decided：四元组必填（决定内容/决定人/时间；适用范围已在行上）。
pub async fn decide_consultation(
    pool: &PgPool,
    tenant_id: Uuid,
    consultation_id: Uuid,
    decision_content: &str,
    decided_by: Uuid,
    actor: &AuditActor,
) -> Result<(), ConsultationError> {
    if decision_content.trim().is_empty() {
        return Err(ConsultationError::IncompleteDecision);
    }
    // pending 直决优先；已 start（in_progress）后再决是合法第二路径。
    // （审查85：decide 前不强制 in_progress——直接决定同样是正式决定。）
    match transition_status(
        pool,
        tenant_id,
        consultation_id,
        "pending",
        "decided",
        Some(decision_content),
        Some(decided_by),
        None,
        actor,
    )
    .await
    {
        Ok(_) => Ok(()),
        Err(ConsultationError::InvalidTransition(_)) => transition_status(
            pool,
            tenant_id,
            consultation_id,
            "in_progress",
            "decided",
            Some(decision_content),
            Some(decided_by),
            None,
            actor,
        )
        .await
        .map(|_| ()),
        Err(other) => Err(other),
    }
}

/// → cancelled：原因必填；仅表示不再需要处理，≠默认同意原方案。
pub async fn cancel_consultation(
    pool: &PgPool,
    tenant_id: Uuid,
    consultation_id: Uuid,
    reason: &str,
    actor: &AuditActor,
) -> Result<(), ConsultationError> {
    if reason.trim().is_empty() {
        return Err(ConsultationError::MissingReason);
    }
    let mut tx = pool.begin().await?;
    let (project_id, task_id, status): (Uuid, Option<Uuid>, String) = sqlx::query_as(
        "SELECT project_id, task_id, status \
           FROM governance.human_consultation WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(consultation_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ConsultationError::NotFound)?;
    if status == "decided" || status == "cancelled" {
        return Err(ConsultationError::InvalidTransition(format!(
            "cannot cancel from {status}"
        )));
    }
    sqlx::query(
        "UPDATE governance.human_consultation \
            SET status = 'cancelled', cancelled_reason = $3, updated_at = now() \
          WHERE id = $1",
    )
    .bind(consultation_id)
    .bind(&reason)
    .bind(project_id)
    .execute(&mut *tx)
    .await?;
    end_waiting_if_task(&mut tx, task_id, "consultation cancelled").await?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "consultation.cancelled",
        "human_consultation",
        Some(consultation_id),
        serde_json::json!({ "reason": reason }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 既有决定被改变：新建一行 pending 咨询并关联被替代行（追加式，不覆盖历史）。
/// 新行同样可挂 task 并重写等待。
pub async fn supersede_consultation(
    pool: &PgPool,
    tenant_id: Uuid,
    consultation_id: Uuid,
    title: &str,
    question: &str,
    created_by: Uuid,
    actor: &AuditActor,
) -> Result<Uuid, ConsultationError> {
    let mut tx = pool.begin().await?;
    let old: (
        Uuid,
        Option<Uuid>,
        serde_json::Value,
        Option<String>,
        String,
    ) = sqlx::query_as(
        "SELECT project_id, task_id, product_scope, scope_note, status \
           FROM governance.human_consultation WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(consultation_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ConsultationError::NotFound)?;
    if old.4 != "decided" {
        return Err(ConsultationError::AlreadyDecided);
    }
    let title = title.trim();
    let question = question.trim();
    if title.is_empty() || question.is_empty() {
        return Err(ConsultationError::InvalidTransition(
            "title and question are required".into(),
        ));
    }
    let new_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO governance.human_consultation \
         (id, tenant_id, project_id, task_id, title, question, product_scope, \
          scope_note, supersedes_consultation_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(new_id)
    .bind(tenant_id)
    .bind(old.0)
    .bind(old.1)
    .bind(title)
    .bind(question)
    .bind(&old.2)
    .bind(&old.3)
    .bind(consultation_id)
    .bind(created_by)
    .execute(&mut *tx)
    .await?;
    if let Some(task) = old.1 {
        sqlx::query(
            "INSERT INTO delivery.task_waiting \
             (project_id, task_id, reason, questions, requested_from, tenant_id) \
             VALUES ($1, $2, 'needs_decision', jsonb_build_array($3), $4, $5)",
        )
        .bind(old.0)
        .bind(task)
        .bind(title)
        .bind(created_by)
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;
    }
    append_audit_with_after(
        &mut tx,
        old.0,
        actor,
        "consultation.superseded",
        "human_consultation",
        Some(consultation_id),
        serde_json::json!({ "new_consultation_id": new_id, "title": title }),
    )
    .await?;
    tx.commit().await?;
    Ok(new_id)
}

pub async fn list_consultations(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    status: Option<&str>,
) -> Result<Vec<HumanConsultationRow>, ConsultationError> {
    let sql = match status {
        Some(_) => {
            "SELECT id, project_id, task_id, title, question, status, decision_content, \
                    decided_by, decided_at, product_scope, scope_note, cancelled_reason, \
                    supersedes_consultation_id, created_by, created_at \
               FROM governance.human_consultation \
              WHERE tenant_id = $1 AND project_id = $2 AND status = $3 \
              ORDER BY created_at DESC, id DESC"
        }
        None => {
            "SELECT id, project_id, task_id, title, question, status, decision_content, \
                    decided_by, decided_at, product_scope, scope_note, cancelled_reason, \
                    supersedes_consultation_id, created_by, created_at \
               FROM governance.human_consultation \
              WHERE tenant_id = $1 AND project_id = $2 \
              ORDER BY created_at DESC, id DESC"
        }
    };
    let mut q = sqlx::query_as::<_, HumanConsultationRow>(sql)
        .bind(tenant_id)
        .bind(project_id);
    if let Some(s) = status {
        q = q.bind(s);
    }
    Ok(q.fetch_all(pool).await?)
}

/// 状态推进共用内核（pending→in_progress / pending|in_progress→decided）。
#[allow(clippy::too_many_arguments)]
async fn transition_status(
    pool: &PgPool,
    tenant_id: Uuid,
    consultation_id: Uuid,
    from: &str,
    to: &str,
    decision_content: Option<&str>,
    decided_by: Option<Uuid>,
    _cancel_reason: Option<&str>,
    actor: &AuditActor,
) -> Result<bool, ConsultationError> {
    let mut tx = pool.begin().await?;
    let (project_id, task_id, status): (Uuid, Option<Uuid>, String) = sqlx::query_as(
        "SELECT project_id, task_id, status \
           FROM governance.human_consultation WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(consultation_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ConsultationError::NotFound)?;
    if status != from {
        return Err(ConsultationError::InvalidTransition(format!(
            "cannot {from} -> {to} from {status}"
        )));
    }
    let updated = sqlx::query(
        "UPDATE governance.human_consultation \
            SET status = $3, decision_content = $4, decided_by = $5, \
                decided_at = CASE WHEN $3 = 'decided' THEN now() ELSE decided_at END, \
                updated_at = now() \
          WHERE id = $1",
    )
    .bind(consultation_id)
    .bind(tenant_id)
    .bind(to)
    .bind(decision_content)
    .bind(decided_by)
    .execute(&mut *tx)
    .await?;
    let _ = updated;
    if to == "decided" {
        end_waiting_if_task(&mut tx, task_id, "consultation decided").await?;
    }
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        &format!("consultation.{to}"),
        "human_consultation",
        Some(consultation_id),
        serde_json::json!({
            "from": from,
            "decided_by": decided_by,
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// 结束该任务上由本模块写入的活跃等待（只动 needs_decision，不碰其他原因）。
async fn end_waiting_if_task(
    tx: &mut sqlx::PgConnection,
    task_id: Option<Uuid>,
    note: &str,
) -> Result<(), sqlx::Error> {
    let Some(task_id) = task_id else {
        return Ok(());
    };
    sqlx::query(
        "UPDATE delivery.task_waiting \
            SET ended_at = now(), resolution_note = $2 \
          WHERE task_id = $1 AND reason = 'needs_decision' AND ended_at IS NULL",
    )
    .bind(task_id)
    .bind(note)
    .execute(&mut *tx)
    .await?;
    Ok(())
}
