//! `/v1/pm/*` 人工写服务（Slice A）：需求修订的起草/定稿与任务的创建/编辑。
//!
//! 与命令通道（`command_executor`，走提案/红线治理）不同，这些是人在 UI 上的
//! 直接操作：本地事务落地 + `governance.audit_log` 行（动作名对齐前端审计页
//! 词汇）。操作者由调用方显式传入（T-021a：HTTP handler 取 `AuthedPrincipal`），
//! 应用层不再自行猜测 actor。

use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::rows::{RequirementRevisionRow, TaskRow};

pub const TASK_STATUSES: [&str; 13] = [
    "draft",
    "ready",
    "claimed",
    "running",
    "awaiting_decision",
    "blocked",
    "submitted",
    "reviewing",
    "completed",
    "changes_requested",
    "failed",
    "cancelled",
    "superseded",
];
pub const TASK_PRIORITIES: [&str; 4] = ["low", "normal", "high", "urgent"];
/// 验收项的验证方式（D2 `0015` 的 CHECK 取值）。
pub const VERIFICATION_KINDS: [&str; 4] = ["manual", "automated_test", "artifact_exists", "review"];

fn default_true() -> bool {
    true
}

/// 双 Option 反序列化：字段缺失 = `None`（不改），显式 `null` = `Some(None)`（清空）。
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

/// 交付物条目（D2 §5：JSONB 对象数组，`id` 供 D4 对账锚定）。
#[derive(Debug, Deserialize)]
pub struct DeliverableInput {
    #[serde(default)]
    pub id: Option<String>,
    pub path: String,
}

/// 逐条验收项（D2 C 裁决：独立表逐条留痕）。
#[derive(Debug, Deserialize)]
pub struct AcceptanceCriterionInput {
    /// 允许间隙；缺省时按 `max+10` 追加（禁止全量重排）。
    #[serde(default)]
    pub ordinal: Option<i32>,
    pub text: String,
    #[serde(default)]
    pub verification_kind: Option<String>,
}

/// 任务 → 知识文档引用（C2：以 `document_id` 为准）。
#[derive(Debug, Deserialize)]
pub struct KnowledgeRefInput {
    pub document_id: Uuid,
    #[serde(default = "default_true")]
    pub is_required: bool,
}

#[derive(Debug)]
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

/// 测试夹具：解析项目的操作者（三级回退）。生产路径不再调用——actor 必须由
/// 调用方显式传入（T-021a / `docs/ai-pm-principal-identity-v1.md` Q3）。
#[cfg(test)]
pub async fn owner_actor(pool: &PgPool, project_id: Uuid) -> Result<Uuid, PmWriteError> {
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT p.id FROM core.project_member m \
         JOIN core.principal p ON p.id = m.principal_id \
         WHERE m.project_id = $1 AND m.role = 'owner' AND p.kind = 'human' AND p.is_active \
         LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    if let Some(actor) = owner {
        return Ok(actor);
    }
    let member: Option<Uuid> = sqlx::query_scalar(
        "SELECT p.id FROM core.project_member m \
         JOIN core.principal p ON p.id = m.principal_id \
         WHERE m.project_id = $1 AND p.kind = 'human' AND p.is_active \
         LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    if let Some(actor) = member {
        return Ok(actor);
    }
    sqlx::query_scalar(
        "SELECT id FROM core.principal WHERE kind = 'human' AND is_active ORDER BY created_at LIMIT 1",
    )
    .fetch_optional(pool)
    .await?
    .ok_or(PmWriteError::Validation("no human principal available as actor"))
}

async fn append_audit(
    tx: &mut sqlx::PgConnection,
    project_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (id, project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, \
          action, target_type, target_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .execute(tx)
    .await?;
    Ok(())
}

/// 人工写路径的审计行（D2 契约变更需要 `after_data` 快照）。
async fn append_audit_with_after(
    tx: &mut sqlx::PgConnection,
    project_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    after: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (id, project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, \
          action, target_type, target_id, after_data) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(after)
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
    actor: &AuditActor,
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
    .bind(actor.principal_id)
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
    actor: &AuditActor,
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

    sqlx::query(
        "UPDATE delivery.requirement_revision SET state = 'approved', approved_by = $2 WHERE id = $1",
    )
    .bind(revision_id)
    .bind(actor.principal_id)
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

/// 创建任务：status 固定 draft、revision=1、priority 缺省 normal。
pub async fn create_task(
    pool: &PgPool,
    req: CreateTaskRequest,
    actor: &AuditActor,
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

    let mut tx = pool.begin().await?;
    let task_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.task \
         (id, project_id, milestone_id, title, description, status, priority, assignee_id, revision) \
         VALUES ($1, $2, $3, $4, $5, 'draft', $6, $7, 1)",
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
    /// D2 契约：目标。缺失=不改，`null`=清空。
    #[serde(default, deserialize_with = "double_option")]
    pub goal: Option<Option<String>>,
    #[serde(default)]
    pub constraints: Option<serde_json::Value>,
    #[serde(default)]
    pub deliverables: Option<Vec<DeliverableInput>>,
    /// 提供即**全量替换**（空数组=清空）。
    #[serde(default)]
    pub acceptance_criteria: Option<Vec<AcceptanceCriterionInput>>,
    /// 提供即**全量替换**（空数组=清空）。
    #[serde(default)]
    pub knowledge_refs: Option<Vec<KnowledgeRefInput>>,
}

/// 全字段编辑 + revision 乐观锁（不匹配 409），成功后 revision+1。
///
/// D2 起同时承担契约写路径（待裁 #11 已裁决并入 D2）：`goal` / `constraints` /
/// `deliverables` 与 `task_acceptance_criterion` / `task_knowledge_ref` 两表的
/// 增删都在**同一事务**内完成，保证契约读写闭环、C3 不再依赖 SQL 手动种子。
pub async fn update_task(
    pool: &PgPool,
    task_id: Uuid,
    req: UpdateTaskRequest,
    actor: &AuditActor,
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
    if let Some(constraints) = &req.constraints {
        if !constraints.is_object() {
            return Err(PmWriteError::Validation("constraints must be a JSON object"));
        }
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

    let mut contract_audit = serde_json::Map::new();
    if let Some(goal) = req.goal {
        sqlx::query("UPDATE delivery.task SET goal = $2, updated_at = now() WHERE id = $1")
            .bind(task_id)
            .bind(&goal)
            .execute(&mut *tx)
            .await?;
        contract_audit.insert("goal".into(), json!(goal));
    }
    if let Some(constraints) = req.constraints {
        sqlx::query("UPDATE delivery.task SET constraints = $2, updated_at = now() WHERE id = $1")
            .bind(task_id)
            .bind(&constraints)
            .execute(&mut *tx)
            .await?;
        contract_audit.insert("constraints".into(), constraints);
    }
    if let Some(deliverables) = req.deliverables {
        let normalized = normalize_deliverables(deliverables)?;
        sqlx::query("UPDATE delivery.task SET deliverables = $2, updated_at = now() WHERE id = $1")
            .bind(task_id)
            .bind(json!(normalized))
            .execute(&mut *tx)
            .await?;
        contract_audit.insert("deliverables".into(), json!(normalized));
    }
    if let Some(criteria) = req.acceptance_criteria {
        let count = replace_acceptance_criteria(&mut tx, project_id, task_id, criteria).await?;
        contract_audit.insert("acceptance_criteria".into(), json!(count));
    }
    if let Some(refs) = req.knowledge_refs {
        let count = replace_knowledge_refs(&mut tx, project_id, task_id, refs).await?;
        contract_audit.insert("knowledge_refs".into(), json!(count));
    }

    append_audit(&mut tx, project_id, actor, "task.update", "task", Some(task_id)).await?;
    if !contract_audit.is_empty() {
        append_audit_with_after(
            &mut tx,
            project_id,
            actor,
            "task.update_contract",
            "task",
            Some(task_id),
            json!({ "contract": contract_audit }),
        )
        .await?;
    }
    tx.commit().await?;

    task_row(pool, task_id).await
}

fn normalize_deliverables(
    input: Vec<DeliverableInput>,
) -> Result<Vec<serde_json::Value>, PmWriteError> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(input.len());
    for item in input {
        let path = item.path.trim().to_string();
        if path.is_empty() {
            return Err(PmWriteError::Validation("deliverable path is required"));
        }
        if !seen.insert(path.clone()) {
            return Err(PmWriteError::Validation("duplicate deliverable path"));
        }
        let id = item
            .id
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        out.push(json!({ "id": id, "path": path }));
    }
    Ok(out)
}

/// 全量替换验收项；`ordinal` 缺省按 `max+10` 追加（允许间隙，不重排既有条目）。
async fn replace_acceptance_criteria(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    input: Vec<AcceptanceCriterionInput>,
) -> Result<usize, PmWriteError> {
    struct Normalized {
        ordinal: i32,
        text: String,
        kind: String,
    }
    let mut normalized: Vec<Normalized> = Vec::with_capacity(input.len());
    let mut next = 10i32;
    for item in input {
        let text = item.text.trim().to_string();
        if text.is_empty() {
            return Err(PmWriteError::Validation("acceptance criterion text is required"));
        }
        let kind = item
            .verification_kind
            .unwrap_or_else(|| "manual".to_string());
        if !VERIFICATION_KINDS.contains(&kind.as_str()) {
            return Err(PmWriteError::Validation("invalid verification_kind"));
        }
        let ordinal = match item.ordinal {
            Some(ordinal) if ordinal >= 1 => ordinal,
            Some(_) => return Err(PmWriteError::Validation("ordinal must be >= 1")),
            None => next,
        };
        if normalized.iter().any(|entry| entry.ordinal == ordinal) {
            return Err(PmWriteError::Validation("duplicate acceptance ordinal"));
        }
        next = ordinal.max(next) + 10;
        normalized.push(Normalized {
            ordinal,
            text,
            kind,
        });
    }

    sqlx::query("DELETE FROM delivery.task_acceptance_criterion WHERE task_id = $1 AND project_id = $2")
        .bind(task_id)
        .bind(project_id)
        .execute(&mut **tx)
        .await?;
    for entry in &normalized {
        sqlx::query(
            "INSERT INTO delivery.task_acceptance_criterion \
             (project_id, task_id, ordinal, text, verification_kind) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(project_id)
        .bind(task_id)
        .bind(entry.ordinal)
        .bind(&entry.text)
        .bind(&entry.kind)
        .execute(&mut **tx)
        .await?;
    }
    Ok(normalized.len())
}

/// 全量替换知识引用；文档必须属于同一项目（跨项目一律拒绝）。
async fn replace_knowledge_refs(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    input: Vec<KnowledgeRefInput>,
) -> Result<usize, PmWriteError> {
    let mut seen = std::collections::HashSet::new();
    for item in &input {
        if !seen.insert(item.document_id) {
            return Err(PmWriteError::Validation("duplicate knowledge document_id"));
        }
    }
    let ids: Vec<Uuid> = input.iter().map(|item| item.document_id).collect();
    let found: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.project_document WHERE project_id = $1 AND id = ANY($2)",
    )
    .bind(project_id)
    .bind(&ids)
    .fetch_one(&mut **tx)
    .await?;
    if found as usize != ids.len() {
        return Err(PmWriteError::Validation(
            "knowledge_refs contain a document outside this project",
        ));
    }

    sqlx::query("DELETE FROM delivery.task_knowledge_ref WHERE task_id = $1 AND project_id = $2")
        .bind(task_id)
        .bind(project_id)
        .execute(&mut **tx)
        .await?;
    for item in &input {
        sqlx::query(
            "INSERT INTO delivery.task_knowledge_ref \
             (project_id, task_id, document_id, is_required) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (task_id, document_id) DO NOTHING",
        )
        .bind(project_id)
        .bind(task_id)
        .bind(item.document_id)
        .bind(item.is_required)
        .execute(&mut **tx)
        .await?;
    }
    Ok(input.len())
}

/// 审批决策的审计行（action='approval.decide'，approve/reject 都落一条；
/// approve 的红线执行审计由 command_executor 另行落 redline 行）。
pub async fn record_decision_audit(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    approval_id: Uuid,
) -> Result<(), PmWriteError> {
    let mut tx = pool.begin().await?;
    append_audit(&mut tx, project_id, actor, "approval.decide", "approval_request", Some(approval_id)).await?;
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
