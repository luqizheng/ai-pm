//! `/v1/pm/*` 人工写服务（Slice A）：需求修订的起草/定稿与任务的创建/编辑。
//!
//! 与命令通道（`command_executor`，走提案/红线治理）不同，这些是人在 UI 上的
//! 直接操作：本地事务落地 + `governance.audit_log` 行（动作名对齐前端审计页
//! 词汇）。操作者解析采用 V1 约定：取该项目的 owner human principal（bootstrap
//! 与演示种子都保证存在）；iam 登录身份到 principal 的映射表留后续里程碑。

use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::rows::{RequirementRevisionRow, TaskRow};

pub const TASK_STATUSES: [&str; 6] =
    ["backlog", "in_progress", "in_review", "done", "blocked", "cancelled"];
pub const TASK_PRIORITIES: [&str; 4] = ["low", "normal", "high", "urgent"];

pub enum PmWriteError {
    NotFound(&'static str),
    Conflict(&'static str),
    Validation(&'static str),
    Db(sqlx::Error),
}

impl From<sqlx::Error> for PmWriteError {
    fn from(e: sqlx::Error) -> Self {
        PmWriteError::Db(e)
    }
}

/// 解析项目的操作者：owner 角色的 human principal（V1 身份约定，见模块注释）。
pub async fn owner_actor(pool: &PgPool, project_id: Uuid) -> Result<Uuid, PmWriteError> {
    sqlx::query_scalar(
        "SELECT p.id FROM core.project_member m \
         JOIN core.principal p ON p.id = m.principal_id \
         WHERE m.project_id = $1 AND m.role = 'owner' AND p.kind = 'human' AND p.is_active \
         LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or(PmWriteError::Validation("project has no owner principal"))
}

async fn append_audit(
    tx: &mut sqlx::PgConnection,
    project_id: Uuid,
    actor_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (id, project_id, actor_id, action, target_type, target_id) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(actor_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .execute(tx)
    .await?;
    Ok(())
}

// ------------------------------------------------------- requirement revisions

#[derive(Debug, Deserialize)]
pub struct CreateRevisionRequest {
    #[serde(default)]
    pub content: Option<RevisionContent>,
}

#[derive(Debug, Deserialize)]
pub struct RevisionContent {
    #[serde(default)]
    pub markdown: Option<String>,
}

/// 新建草稿修订：version 自增、content_hash = sha256("{rid}:{version}:{markdown}")。
pub async fn create_revision(
    pool: &PgPool,
    requirement_id: Uuid,
    req: CreateRevisionRequest,
) -> Result<RequirementRevisionRow, PmWriteError> {
    let markdown = req
        .content
        .and_then(|c| c.markdown)
        .map(|m| m.trim().to_string())
        .unwrap_or_default();
    if markdown.is_empty() {
        return Err(PmWriteError::Validation("content.markdown is required"));
    }
    let (project_id,): (Uuid,) =
        sqlx::query_as("SELECT project_id FROM delivery.requirement WHERE id = $1")
            .bind(requirement_id)
            .fetch_optional(pool)
            .await?
            .ok_or(PmWriteError::NotFound("not found"))?;
    let actor = owner_actor(pool, project_id).await?;

    let mut tx = pool.begin().await?;
    let version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(max(version), 0) + 1 FROM delivery.requirement_revision \
         WHERE requirement_id = $1",
    )
    .bind(requirement_id)
    .fetch_one(&mut *tx)
    .await?;

    let hash_input = format!("{requirement_id}:{version}:{markdown}");
    let content_hash = hex_sha256(hash_input.as_bytes());
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.requirement_revision \
         (id, requirement_id, project_id, version, content, state, content_hash, created_by) \
         VALUES ($1, $2, $3, $4, $5, 'draft', $6, $7)",
    )
    .bind(revision_id)
    .bind(requirement_id)
    .bind(project_id)
    .bind(version)
    .bind(json!({ "markdown": markdown }))
    .bind(&content_hash)
    .bind(actor)
    .execute(&mut *tx)
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.propose_revision",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;

    let row = sqlx::query_as::<_, RequirementRevisionRow>(
        "SELECT id, requirement_id, project_id, version, content, state, content_hash, \
                created_by, approved_by, created_at \
         FROM delivery.requirement_revision WHERE id = $1",
    )
    .bind(revision_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// 定稿：draft → approved；同需求其余 approved → superseded；requirement 指向
/// 新定稿、status='approved'、row_revision+1。非 draft（含不存在）一律 409。
pub async fn approve_revision(
    pool: &PgPool,
    requirement_id: Uuid,
    revision_id: Uuid,
) -> Result<RequirementRevisionRow, PmWriteError> {
    let mut tx = pool.begin().await?;
    let (project_id, state): (Uuid, String) = sqlx::query_as(
        "SELECT project_id, state FROM delivery.requirement_revision \
         WHERE id = $1 AND requirement_id = $2",
    )
    .bind(revision_id)
    .bind(requirement_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(PmWriteError::Conflict("not a draft revision"))?;
    if state != "draft" {
        return Err(PmWriteError::Conflict("not a draft revision"));
    }
    let actor = owner_actor(pool, project_id).await?;

    sqlx::query(
        "UPDATE delivery.requirement_revision SET state = 'approved', approved_by = $2 WHERE id = $1",
    )
    .bind(revision_id)
    .bind(actor)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE delivery.requirement_revision SET state = 'superseded' \
         WHERE requirement_id = $1 AND state = 'approved' AND id <> $2",
    )
    .bind(requirement_id)
    .bind(revision_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE delivery.requirement \
         SET current_revision_id = $2, status = 'approved', row_revision = row_revision + 1 \
         WHERE id = $1",
    )
    .bind(requirement_id)
    .bind(revision_id)
    .execute(&mut *tx)
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.approve_revision",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;

    let row = sqlx::query_as::<_, RequirementRevisionRow>(
        "SELECT id, requirement_id, project_id, version, content, state, content_hash, \
                created_by, approved_by, created_at \
         FROM delivery.requirement_revision WHERE id = $1",
    )
    .bind(revision_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

// --------------------------------------------------------------------- tasks

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub milestone_id: Option<Uuid>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub assignee_id: Option<Uuid>,
}

/// 创建任务：status 固定 backlog、revision=1、priority 缺省 normal。
pub async fn create_task(
    pool: &PgPool,
    req: CreateTaskRequest,
) -> Result<TaskRow, PmWriteError> {
    // mock 契约：project 未知（含缺省）→ 404，先于 title 校验。
    let project_id = req
        .project_id
        .ok_or(PmWriteError::NotFound("unknown project"))?;
    let (project_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(pool)
            .await?
            .ok_or(PmWriteError::NotFound("unknown project"))?;
    let title = req.title.as_deref().map(str::trim).unwrap_or_default();
    if title.is_empty() {
        return Err(PmWriteError::Validation("title is required"));
    }
    let priority = req.priority.as_deref().unwrap_or("normal");
    if !TASK_PRIORITIES.contains(&priority) {
        return Err(PmWriteError::Validation("invalid priority"));
    }
    let actor = owner_actor(pool, project_id).await?;

    let mut tx = pool.begin().await?;
    let task_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.task \
         (id, project_id, milestone_id, title, description, status, priority, assignee_id, revision) \
         VALUES ($1, $2, $3, $4, $5, 'backlog', $6, $7, 1)",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(req.milestone_id)
    .bind(title)
    .bind(req.description)
    .bind(priority)
    .bind(req.assignee_id)
    .execute(&mut *tx)
    .await?;
    append_audit(&mut tx, project_id, actor, "task.create", "task", Some(task_id)).await?;
    tx.commit().await?;

    task_row(pool, task_id).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    #[serde(default)]
    pub revision: Option<i64>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub assignee_id: Option<Uuid>,
}

/// 全字段编辑 + revision 乐观锁（不匹配 409），成功后 revision+1。
pub async fn update_task(
    pool: &PgPool,
    task_id: Uuid,
    req: UpdateTaskRequest,
) -> Result<TaskRow, PmWriteError> {
    let status = req.status.as_deref().unwrap_or_default();
    if !TASK_STATUSES.contains(&status) {
        return Err(PmWriteError::Validation("invalid status"));
    }
    let priority = req.priority.as_deref().unwrap_or_default();
    if !TASK_PRIORITIES.contains(&priority) {
        return Err(PmWriteError::Validation("invalid priority"));
    }
    let title = req.title.as_deref().map(str::trim).unwrap_or_default();
    if title.is_empty() {
        return Err(PmWriteError::Validation("title is required"));
    }
    let mut tx = pool.begin().await?;
    let (project_id, current_revision): (Uuid, i64) =
        sqlx::query_as("SELECT project_id, revision FROM delivery.task WHERE id = $1")
            .bind(task_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(PmWriteError::NotFound("not found"))?;
    // mock 契约：revision 缺省/不匹配都是 409 revision conflict。
    if req.revision.unwrap_or(current_revision + 1) != current_revision {
        return Err(PmWriteError::Conflict("revision conflict"));
    }
    let actor = owner_actor(pool, project_id).await?;

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
    append_audit(&mut tx, project_id, actor, "task.update", "task", Some(task_id)).await?;
    tx.commit().await?;

    task_row(pool, task_id).await
}

/// 审批决策的审计行（action='approval.decide'，approve/reject 都落一条；
/// approve 的红线执行审计由 command_executor 另行落 redline 行）。
pub async fn record_decision_audit(
    pool: &PgPool,
    project_id: Uuid,
    actor_id: Uuid,
    approval_id: Uuid,
) -> Result<(), PmWriteError> {
    let mut tx = pool.begin().await?;
    append_audit(&mut tx, project_id, actor_id, "approval.decide", "approval_request", Some(approval_id)).await?;
    tx.commit().await?;
    Ok(())
}

async fn task_row(pool: &PgPool, task_id: Uuid) -> Result<TaskRow, PmWriteError> {
    sqlx::query_as::<_, TaskRow>(
        "SELECT id, project_id, milestone_id, title, description, status, priority, \
                assignee_id, revision, created_at, updated_at \
         FROM delivery.task WHERE id = $1",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::from)
}

/// 小写 hex 的 sha256（pm-domain 同款摘要约定，无 hex 依赖时手写编码）。
fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::hex_sha256;

    #[test]
    fn hex_sha256_is_lowercase_hex_of_sha256() {
        // echo -n "abc" | sha256sum
        assert_eq!(
            hex_sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
