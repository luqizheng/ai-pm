//! Structured Memory / L1 Shallow Memory：提案（待审核区）→ 真人审核（K4）→ 不可变版本。
//!
//! 定义稿 §9.1/§9.2 的操作化（迁移 0065 库层兜底）：
//! - **任何内容变化**（新增 / 修改 / 撤回）都以 `PENDING_REVIEW` 修订进入待审核区；
//!   提案人可以是真人或 AI（AI 提议），**审核人必须是真人**（AI 无确认权，403）；
//! - 审核通过才产生版本：`version_no` 递增，新版本 `CURRENT`（撤回提案 → `WITHDRAWN`），
//!   旧 `CURRENT` 自动 `SUPERSEDED`；驳回 / 退回只关闭该提案，永不成为版本；
//! - 版本内容不可变、不可物理删除；每个记忆至多一条待审提案、至多一个 CURRENT；
//! - K4 CAS：`base_memory_revision_id` 必须是该记忆**当前待审提案**本身（审核人看到的
//!   就是被裁决的内容）；批准时还校验提案基线仍是 CURRENT——任一失配 409；
//! - 批准与 `kb.memory.approved` outbox 事件同事务（D-8：consumer 仅 platform，状态
//!   感知与待办，不作为 PC 触发输入）。

use chrono::{DateTime, Utc};
use kb_pc_contracts::registry::{parse_domain, resolve_reference, TargetDomain};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::{
    audit, check_len, ensure_project_writable, is_unique_violation, KbServiceError, TYPE_MEMORY,
    TYPE_MEMORY_REVISION, TYPE_SOURCE,
};
use crate::actor::AuditActor;
use crate::outbox::{OutboxAppend, OutboxEvent, PgOutboxAppend};

/// L1 记忆六类（定义稿 §5.3；与 0065 CHECK 同集）。
pub const MEMORY_TYPES: &[&str] = &[
    "discussion",
    "decision_background",
    "rejected_option",
    "open_question",
    "business_constraint",
    "controversy",
];

/// 每条修订的引用上限。
pub const MAX_REFS: usize = 50;

/// 引用角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefRole {
    /// 来源依据：指向 L2 原文（会话消息 / 任务交互 / 原始资料）。
    Evidence,
    /// 讨论主题：按 Task 检索的锚（task / requirement）。
    Subject,
}

impl RefRole {
    pub fn as_str(self) -> &'static str {
        match self {
            RefRole::Evidence => "evidence",
            RefRole::Subject => "subject",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "evidence" => Some(RefRole::Evidence),
            "subject" => Some(RefRole::Subject),
            _ => None,
        }
    }
}

/// 记忆内容（类型 / 标题 / 摘要 / 详述）。
#[derive(Debug, Clone, Deserialize)]
pub struct MemoryContent {
    pub memory_type: String,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub body: Option<String>,
}

/// 经 Platform 网关解析后的引用（四元组 + 空缺原因已补齐）。
///
/// Platform 对象（task / requirement / conversation_message / task_interaction）由
/// Platform 侧校验后传入（本模块不读 Platform 表）；`knowledge_base` 对象由本模块
/// 在写事务内自校验。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRef {
    pub role: RefRole,
    pub target_domain: String,
    pub target_type: String,
    pub target_id: String,
    pub target_revision_id: Option<String>,
    pub revision_missing_reason: Option<String>,
    pub locator: Option<serde_json::Value>,
}

/// 提案回执。
#[derive(Debug, Clone, Serialize)]
pub struct ProposalReceipt {
    pub memory_id: Uuid,
    pub revision_id: Uuid,
    pub revision_no: i32,
    pub change_action: String,
    pub review_status: String,
}

/// 变更动作（CREATE 走 [`propose_create`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeAction {
    Update,
    Withdraw,
}

impl ChangeAction {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "update" | "UPDATE" => Some(ChangeAction::Update),
            "withdraw" | "WITHDRAW" => Some(ChangeAction::Withdraw),
            _ => None,
        }
    }

    fn as_db(self) -> &'static str {
        match self {
            ChangeAction::Update => "UPDATE",
            ChangeAction::Withdraw => "WITHDRAW",
        }
    }
}

/// K4 审核动作（V1.0 §12.2 K4 契约 `approve | reject | revise`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewAction {
    Approve,
    Reject,
    Revise,
}

impl ReviewAction {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "approve" => Some(ReviewAction::Approve),
            "reject" => Some(ReviewAction::Reject),
            "revise" => Some(ReviewAction::Revise),
            _ => None,
        }
    }
}

/// K4 审核结论。
#[derive(Debug, Clone, Serialize)]
pub struct ReviewOutcome {
    pub memory_id: Uuid,
    pub revision_id: Uuid,
    /// `APPROVED` / `REJECTED` / `REVISION_REQUESTED`。
    pub review_status: String,
    pub version_no: Option<i32>,
    /// 批准后的版本状态：`CURRENT` / `WITHDRAWN`。
    pub version_status: Option<String>,
    pub superseded_revision_id: Option<Uuid>,
}

fn validate_content(content: &MemoryContent) -> Result<(), KbServiceError> {
    if !MEMORY_TYPES.contains(&content.memory_type.as_str()) {
        return Err(KbServiceError::Invalid(format!(
            "memory_type must be one of {MEMORY_TYPES:?}"
        )));
    }
    check_len("title", &content.title, 1, 200)?;
    check_len("summary", &content.summary, 1, 2000)?;
    Ok(())
}

fn validate_reason(change_reason: &str) -> Result<(), KbServiceError> {
    check_len("change_reason", change_reason, 1, 1000)
}

/// 引用形状校验：角色/类型白名单、domain 已登记且通道开放、上限与去重、≥1 evidence。
fn validate_refs(refs: &[ResolvedRef]) -> Result<(), KbServiceError> {
    if refs.len() > MAX_REFS {
        return Err(KbServiceError::Invalid(format!(
            "at most {MAX_REFS} references per revision"
        )));
    }
    let mut seen = std::collections::HashSet::new();
    let mut evidence = 0usize;
    for r in refs {
        let domain = parse_domain(&r.target_domain)
            .map_err(|e| KbServiceError::Invalid(format!("reference domain: {e}")))?;
        resolve_reference(domain)
            .map_err(|e| KbServiceError::Invalid(format!("reference domain: {e}")))?;
        if r.target_id.trim().is_empty() {
            return Err(KbServiceError::Invalid(
                "reference target_id is empty".into(),
            ));
        }
        if r.target_revision_id.is_none() == r.revision_missing_reason.is_none() {
            return Err(KbServiceError::Invalid(
                "reference must pin target_revision_id or record revision_missing_reason".into(),
            ));
        }
        if !seen.insert((r.role, &r.target_domain, &r.target_type, &r.target_id)) {
            return Err(KbServiceError::Invalid(format!(
                "duplicate reference {}:{}/{}",
                r.role.as_str(),
                r.target_type,
                r.target_id
            )));
        }
        if domain == TargetDomain::KnowledgeBase && r.target_type != TYPE_SOURCE {
            return Err(KbServiceError::Invalid(format!(
                "knowledge_base reference type `{}` is not supported (expected source)",
                r.target_type
            )));
        }
        if r.role == RefRole::Evidence {
            evidence += 1;
        }
    }
    if evidence == 0 {
        // 定义稿 §5.1：结构化记忆必须保留来源依据。
        return Err(KbServiceError::Invalid(
            "at least one evidence reference (source basis) is required".into(),
        ));
    }
    Ok(())
}

/// KB 域引用自校验：`knowledge_base/source` 必须已关联到本项目。
async fn check_kb_refs_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    refs: &[ResolvedRef],
) -> Result<(), KbServiceError> {
    for r in refs
        .iter()
        .filter(|r| r.target_domain == TargetDomain::KnowledgeBase.as_str())
    {
        let source_id = Uuid::parse_str(&r.target_id).map_err(|_| {
            KbServiceError::Invalid("knowledge_base source id must be a UUID".into())
        })?;
        if !super::source::source_linked_tx(tx, project_id, source_id).await? {
            return Err(KbServiceError::Invalid(
                "referenced source is not linked to this project".into(),
            ));
        }
    }
    Ok(())
}

/// 非 KB 域引用校验（SEC-P2-1）：按 registry 域映射到物理表，落库前做
/// 存在性 + 同租户校验。`knowledge_base` 仍由 [`check_kb_refs_tx`] 的既有
/// `source → source_project` 通道语义裁决，不在此重复判定。
///
/// 与 `validate_refs` 的分工：后者只做形状/通道校验（不查库），本函数在写事务
/// 内兜底，覆盖直接调用本服务（不经 Platform `resolve_refs`）的入口。
async fn check_non_kb_refs_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    refs: &[ResolvedRef],
) -> Result<(), KbServiceError> {
    for r in refs
        .iter()
        .filter(|r| r.target_domain != TargetDomain::KnowledgeBase.as_str())
    {
        let domain = parse_domain(&r.target_domain)
            .map_err(|e| KbServiceError::Invalid(format!("reference domain: {e}")))?;
        let Some(target) = crate::reference_targets::domain_target(domain, &r.target_type) else {
            continue;
        };
        let id = Uuid::parse_str(&r.target_id).map_err(|_| {
            KbServiceError::Invalid(format!(
                "reference target_id must be a uuid for `{}` domain",
                r.target_domain
            ))
        })?;
        // SEC-P2-1 口径是「同租户」；逻辑检索锚允许同租户跨项目（V1 §5.1）。
        let probe = crate::reference_targets::probe(&mut **tx, target.table, id, tenant_id, None)
            .await?;
        if !target.allows(probe) {
            return Err(KbServiceError::Invalid(format!(
                "reference target {}:{}/{} is missing or out of tenant scope (SEC-P2-1)",
                r.target_domain, r.target_type, r.target_id
            )));
        }
    }
    Ok(())
}

/// 版本内容锚：规范化（类型/标题/摘要/详述 + 排序后的引用四元组）的 sha256。
fn content_hash(content: &MemoryContent, refs: &[ResolvedRef]) -> String {
    let mut keys: Vec<String> = refs
        .iter()
        .map(|r| {
            format!(
                "{}|{}|{}|{}|{}",
                r.role.as_str(),
                r.target_domain,
                r.target_type,
                r.target_id,
                r.target_revision_id.as_deref().unwrap_or("")
            )
        })
        .collect();
    keys.sort();
    let canonical = serde_json::json!({
        "memory_type": content.memory_type,
        "title": content.title.trim(),
        "summary": content.summary.trim(),
        "body": content.body.as_deref().map(str::trim),
        "refs": keys,
    });
    // 摘要原语单源：`pm_write::hex_sha256`（批 A3）。
    crate::pm_write::hex_sha256(canonical.to_string().as_bytes())
}

#[allow(clippy::too_many_arguments)]
async fn insert_revision(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    memory_id: Uuid,
    revision_no: i32,
    change_action: &str,
    base_revision_id: Option<Uuid>,
    content: &MemoryContent,
    change_reason: &str,
    refs: &[ResolvedRef],
    proposed_by: Uuid,
) -> Result<Uuid, KbServiceError> {
    let hash = content_hash(content, refs);
    let inserted = sqlx::query_scalar(
        "INSERT INTO kb.memory_revision \
         (tenant_id, memory_id, revision_no, change_action, base_revision_id, memory_type, \
          title, summary, body, change_reason, content_hash, proposed_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(memory_id)
    .bind(revision_no)
    .bind(change_action)
    .bind(base_revision_id)
    .bind(&content.memory_type)
    .bind(content.title.trim())
    .bind(content.summary.trim())
    .bind(
        content
            .body
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty()),
    )
    .bind(change_reason.trim())
    .bind(&hash)
    .bind(proposed_by)
    .fetch_one(&mut **tx)
    .await;
    let revision_id: Uuid = match inserted {
        Ok(id) => id,
        Err(err) if is_unique_violation(&err) => {
            return Err(KbServiceError::Conflict(
                "memory already has a pending change awaiting review".into(),
            ))
        }
        Err(err) => return Err(err.into()),
    };
    for r in refs {
        sqlx::query(
            "INSERT INTO kb.memory_ref \
             (tenant_id, project_id, revision_id, ref_role, target_domain, target_type, target_id, \
              target_revision_id, revision_missing_reason, locator) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(revision_id)
        .bind(r.role.as_str())
        .bind(&r.target_domain)
        .bind(&r.target_type)
        .bind(&r.target_id)
        .bind(&r.target_revision_id)
        .bind(&r.revision_missing_reason)
        .bind(&r.locator)
        .execute(&mut **tx)
        .await?;
    }
    Ok(revision_id)
}

/// 新增记忆提案（CREATE）：建记忆身份 + 第 1 号待审修订。
pub async fn propose_create(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    actor: &AuditActor,
    content: MemoryContent,
    change_reason: &str,
    refs: Vec<ResolvedRef>,
) -> Result<ProposalReceipt, KbServiceError> {
    validate_content(&content)?;
    validate_reason(change_reason)?;
    validate_refs(&refs)?;
    let mut tx = pool.begin().await?;
    ensure_project_writable(&mut tx, project_id).await?;
    check_kb_refs_tx(&mut tx, project_id, &refs).await?;
    check_non_kb_refs_tx(&mut tx, tenant_id, &refs).await?;
    let memory_id: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.memory (tenant_id, project_id, created_by) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;
    let revision_id = insert_revision(
        &mut tx,
        tenant_id,
        project_id,
        memory_id,
        1,
        "CREATE",
        None,
        &content,
        change_reason,
        &refs,
        actor.principal_id,
    )
    .await?;
    audit(
        &mut tx,
        project_id,
        actor,
        "kb.memory.change_proposed",
        "kb_memory",
        memory_id,
        serde_json::json!({
            "revision_id": revision_id,
            "revision_no": 1,
            "change_action": "CREATE",
            "memory_type": content.memory_type,
            "ref_count": refs.len(),
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(ProposalReceipt {
        memory_id,
        revision_id,
        revision_no: 1,
        change_action: "CREATE".into(),
        review_status: "PENDING_REVIEW".into(),
    })
}

/// 当前 CURRENT 版本（行级只读快照）。
struct CurrentVersion {
    id: Uuid,
    memory_type: String,
    title: String,
    summary: String,
    body: Option<String>,
}

async fn lock_memory_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    memory_id: Uuid,
) -> Result<(), KbServiceError> {
    let found: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM kb.memory WHERE id = $1 AND tenant_id = $2 AND project_id = $3 FOR UPDATE",
    )
    .bind(memory_id)
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?;
    found
        .map(|_| ())
        .ok_or(KbServiceError::NotFound("memory not found"))
}

async fn current_version_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    memory_id: Uuid,
) -> Result<Option<CurrentVersion>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, memory_type, title, summary, body FROM kb.memory_revision \
          WHERE memory_id = $1 AND version_status = 'CURRENT'",
    )
    .bind(memory_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(|r| CurrentVersion {
        id: r.get("id"),
        memory_type: r.get("memory_type"),
        title: r.get("title"),
        summary: r.get("summary"),
        body: r.get("body"),
    }))
}

async fn current_refs_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    revision_id: Uuid,
) -> Result<Vec<ResolvedRef>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT ref_role, target_domain, target_type, target_id, target_revision_id, \
                revision_missing_reason, locator \
           FROM kb.memory_ref WHERE revision_id = $1 ORDER BY created_at, id",
    )
    .bind(revision_id)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| ResolvedRef {
            role: RefRole::parse(r.get::<String, _>("ref_role").as_str())
                .unwrap_or(RefRole::Evidence),
            target_domain: r.get("target_domain"),
            target_type: r.get("target_type"),
            target_id: r.get("target_id"),
            target_revision_id: r.get("target_revision_id"),
            revision_missing_reason: r.get("revision_missing_reason"),
            locator: r.get("locator"),
        })
        .collect())
}

/// 修改 / 撤回提案（UPDATE / WITHDRAW）：钉住当前 CURRENT 为基线。
///
/// WITHDRAW 不接收新内容：内容与引用原样承接自 CURRENT（被撤回的版本自描述），
/// 只要求撤回理由。UPDATE 若内容与引用均无变化 → 400（不产生空版本）。
#[allow(clippy::too_many_arguments)]
pub async fn propose_change(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    memory_id: Uuid,
    actor: &AuditActor,
    action: ChangeAction,
    content: Option<MemoryContent>,
    change_reason: &str,
    refs: Vec<ResolvedRef>,
) -> Result<ProposalReceipt, KbServiceError> {
    validate_reason(change_reason)?;
    if action == ChangeAction::Update {
        let Some(c) = content.as_ref() else {
            return Err(KbServiceError::Invalid("update requires content".into()));
        };
        validate_content(c)?;
        validate_refs(&refs)?;
    } else if content.is_some() || !refs.is_empty() {
        return Err(KbServiceError::Invalid(
            "withdraw carries no new content or references (only change_reason)".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    ensure_project_writable(&mut tx, project_id).await?;
    lock_memory_tx(&mut tx, tenant_id, project_id, memory_id).await?;
    let pending: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM kb.memory_revision WHERE memory_id = $1 AND review_status = 'PENDING_REVIEW'",
    )
    .bind(memory_id)
    .fetch_optional(&mut *tx)
    .await?;
    if pending.is_some() {
        return Err(KbServiceError::Conflict(
            "memory already has a pending change awaiting review".into(),
        ));
    }
    let Some(current) = current_version_tx(&mut tx, memory_id).await? else {
        return Err(KbServiceError::Conflict(
            "memory has no current version (withdrawn, or first proposal not yet approved)".into(),
        ));
    };

    let (content, refs) = match action {
        ChangeAction::Update => {
            check_kb_refs_tx(&mut tx, project_id, &refs).await?;
            check_non_kb_refs_tx(&mut tx, tenant_id, &refs).await?;
            (content.expect("checked above"), refs)
        }
        ChangeAction::Withdraw => {
            let inherited = current_refs_tx(&mut tx, current.id).await?;
            (
                MemoryContent {
                    memory_type: current.memory_type.clone(),
                    title: current.title.clone(),
                    summary: current.summary.clone(),
                    body: current.body.clone(),
                },
                inherited,
            )
        }
    };
    if action == ChangeAction::Update {
        let current_refs = current_refs_tx(&mut tx, current.id).await?;
        let current_content = MemoryContent {
            memory_type: current.memory_type.clone(),
            title: current.title.clone(),
            summary: current.summary.clone(),
            body: current.body.clone(),
        };
        if content_hash(&current_content, &current_refs) == content_hash(&content, &refs) {
            return Err(KbServiceError::Invalid(
                "update proposes no change to the current version".into(),
            ));
        }
    }

    let revision_no: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(revision_no), 0) + 1 FROM kb.memory_revision WHERE memory_id = $1",
    )
    .bind(memory_id)
    .fetch_one(&mut *tx)
    .await?;
    let revision_id = insert_revision(
        &mut tx,
        tenant_id,
        project_id,
        memory_id,
        revision_no,
        action.as_db(),
        Some(current.id),
        &content,
        change_reason,
        &refs,
        actor.principal_id,
    )
    .await?;
    audit(
        &mut tx,
        project_id,
        actor,
        "kb.memory.change_proposed",
        "kb_memory",
        memory_id,
        serde_json::json!({
            "revision_id": revision_id,
            "revision_no": revision_no,
            "change_action": action.as_db(),
            "base_revision_id": current.id,
            "memory_type": content.memory_type,
            "ref_count": refs.len(),
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(ProposalReceipt {
        memory_id,
        revision_id,
        revision_no,
        change_action: action.as_db().into(),
        review_status: "PENDING_REVIEW".into(),
    })
}

/// K4 审核（CAS）：只有真人；`base_memory_revision_id` 必须是当前待审提案。
#[allow(clippy::too_many_arguments)]
pub async fn review(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    memory_id: Uuid,
    actor: &AuditActor,
    action: ReviewAction,
    note: Option<&str>,
    base_memory_revision_id: Uuid,
) -> Result<ReviewOutcome, KbServiceError> {
    // AI 无确认权（S02D-02 #2 同口径；经 Platform 鉴权函数，检查单 A-1）。
    crate::requirement::require_human(pool, actor)
        .await
        .map_err(|e| match e {
            crate::pm_write::PmWriteError::Forbidden(_)
            | crate::pm_write::PmWriteError::NotFound(_) => {
                KbServiceError::Forbidden("memory review requires a human principal")
            }
            crate::pm_write::PmWriteError::Db(err) => KbServiceError::Db(err),
            crate::pm_write::PmWriteError::ValidationMsg(m) => KbServiceError::Invalid(m),
            crate::pm_write::PmWriteError::Conflict(m)
            | crate::pm_write::PmWriteError::Validation(m) => KbServiceError::Invalid(m.into()),
        })?;
    let note = note.map(str::trim).filter(|n| !n.is_empty());
    if let Some(n) = note {
        check_len("note", n, 1, 1000)?;
    }

    let mut tx = pool.begin().await?;
    ensure_project_writable(&mut tx, project_id).await?;
    lock_memory_tx(&mut tx, tenant_id, project_id, memory_id).await?;
    let pending = sqlx::query(
        "SELECT id, change_action, base_revision_id FROM kb.memory_revision \
          WHERE memory_id = $1 AND review_status = 'PENDING_REVIEW'",
    )
    .bind(memory_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(pending) = pending else {
        return Err(KbServiceError::Conflict(
            "memory has no pending change to review".into(),
        ));
    };
    let pending_id: Uuid = pending.get("id");
    if pending_id != base_memory_revision_id {
        return Err(KbServiceError::Conflict(
            "stale review: base_memory_revision_id is not the pending change".into(),
        ));
    }
    let change_action: String = pending.get("change_action");
    let pending_base: Option<Uuid> = pending.get("base_revision_id");

    let outcome = match action {
        ReviewAction::Approve => {
            let current = current_version_tx(&mut tx, memory_id).await?;
            match (change_action.as_str(), current.as_ref()) {
                ("CREATE", None) => {}
                ("UPDATE" | "WITHDRAW", Some(cur)) if Some(cur.id) == pending_base => {}
                _ => {
                    return Err(KbServiceError::Conflict(
                        "stale review: the proposal's base is no longer the current version".into(),
                    ))
                }
            }
            let superseded = current.as_ref().map(|c| c.id);
            if let Some(cur_id) = superseded {
                sqlx::query(
                    "UPDATE kb.memory_revision SET version_status = 'SUPERSEDED' WHERE id = $1",
                )
                .bind(cur_id)
                .execute(&mut *tx)
                .await?;
            }
            let version_no: i32 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(version_no), 0) + 1 FROM kb.memory_revision WHERE memory_id = $1",
            )
            .bind(memory_id)
            .fetch_one(&mut *tx)
            .await?;
            let version_status = if change_action == "WITHDRAW" {
                "WITHDRAWN"
            } else {
                "CURRENT"
            };
            sqlx::query(
                "UPDATE kb.memory_revision \
                    SET review_status = 'APPROVED', reviewed_by = $2, reviewed_at = now(), \
                        review_note = $3, version_no = $4, version_status = $5 \
                  WHERE id = $1",
            )
            .bind(pending_id)
            .bind(actor.principal_id)
            .bind(note)
            .bind(version_no)
            .bind(version_status)
            .execute(&mut *tx)
            .await?;
            audit(
                &mut tx,
                project_id,
                actor,
                "kb.memory.change_approved",
                "kb_memory",
                memory_id,
                serde_json::json!({
                    "revision_id": pending_id,
                    "change_action": change_action,
                    "version_no": version_no,
                    "version_status": version_status,
                    "superseded_revision_id": superseded,
                }),
            )
            .await?;
            emit_memory_approved(
                &mut tx,
                project_id,
                memory_id,
                pending_id,
                actor,
                version_status,
            )
            .await?;
            ReviewOutcome {
                memory_id,
                revision_id: pending_id,
                review_status: "APPROVED".into(),
                version_no: Some(version_no),
                version_status: Some(version_status.into()),
                superseded_revision_id: superseded,
            }
        }
        ReviewAction::Reject | ReviewAction::Revise => {
            let (status, audit_action) = if action == ReviewAction::Reject {
                ("REJECTED", "kb.memory.change_rejected")
            } else {
                ("REVISION_REQUESTED", "kb.memory.change_revision_requested")
            };
            sqlx::query(
                "UPDATE kb.memory_revision \
                    SET review_status = $2, reviewed_by = $3, reviewed_at = now(), review_note = $4 \
                  WHERE id = $1",
            )
            .bind(pending_id)
            .bind(status)
            .bind(actor.principal_id)
            .bind(note)
            .execute(&mut *tx)
            .await?;
            audit(
                &mut tx,
                project_id,
                actor,
                audit_action,
                "kb_memory",
                memory_id,
                serde_json::json!({
                    "revision_id": pending_id,
                    "change_action": change_action,
                    "review_status": status,
                }),
            )
            .await?;
            ReviewOutcome {
                memory_id,
                revision_id: pending_id,
                review_status: status.into(),
                version_no: None,
                version_status: None,
                superseded_revision_id: None,
            }
        }
    };
    tx.commit().await?;
    Ok(outcome)
}

/// `kb.memory.approved`（§13.3 目录三字段；幂等键业务部分 = memory_revision_id）。
async fn emit_memory_approved(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    memory_id: Uuid,
    revision_id: Uuid,
    actor: &AuditActor,
    version_status: &str,
) -> Result<(), KbServiceError> {
    let payload = serde_json::json!({
        "memory_ref": {
            "target_domain": TargetDomain::KnowledgeBase.as_str(),
            "target_type": TYPE_MEMORY,
            "target_id": memory_id.to_string(),
            "target_revision_id": revision_id.to_string(),
        },
        "reviewed_by": actor.principal_id.to_string(),
        "approval_state": version_status,
    });
    kb_pc_contracts::events::validate_payload_references(&payload)
        .map_err(|v| KbServiceError::Invalid(format!("kb.memory.approved payload: {v}")))?;
    PgOutboxAppend
        .append(
            tx,
            OutboxEvent {
                project_id,
                event_type: kb_pc_contracts::events::KB_MEMORY_APPROVED,
                aggregate_type: "kb_memory",
                aggregate_id: &memory_id.to_string(),
                payload,
                idempotency_key: &format!("kb_memory_approved:{revision_id}"),
            },
        )
        .await?;
    Ok(())
}

// ── 读面 ────────────────────────────────────────────────────────────────────

/// 引用视图。
#[derive(Debug, Clone, Serialize)]
pub struct RefView {
    pub ref_role: String,
    pub target_domain: String,
    pub target_type: String,
    pub target_id: String,
    pub target_revision_id: Option<String>,
    pub revision_missing_reason: Option<String>,
    pub locator: Option<serde_json::Value>,
}

/// 修订视图（含审核面与版本面）。
#[derive(Debug, Clone, Serialize)]
pub struct RevisionView {
    pub revision_id: Uuid,
    pub revision_no: i32,
    pub change_action: String,
    pub base_revision_id: Option<Uuid>,
    pub memory_type: String,
    pub title: String,
    pub summary: String,
    pub body: Option<String>,
    pub change_reason: String,
    pub content_hash: String,
    pub proposed_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub review_status: String,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub review_note: Option<String>,
    pub version_no: Option<i32>,
    pub version_status: Option<String>,
    pub refs: Vec<RefView>,
}

/// 记忆整体状态（派生，不落列）。
fn derive_state(revisions: &[RevisionView]) -> &'static str {
    if revisions
        .iter()
        .any(|r| r.version_status.as_deref() == Some("CURRENT"))
    {
        "active"
    } else if revisions
        .iter()
        .any(|r| r.version_status.as_deref() == Some("WITHDRAWN"))
    {
        "withdrawn"
    } else if revisions
        .iter()
        .any(|r| r.review_status == "PENDING_REVIEW")
    {
        "proposed"
    } else {
        "not_adopted"
    }
}

/// 记忆详情：全部修订（提案序），含每条修订的引用。
#[derive(Debug, Clone, Serialize)]
pub struct MemoryDetail {
    pub memory_id: Uuid,
    pub project_id: Uuid,
    /// `active` / `withdrawn` / `proposed` / `not_adopted`。
    pub state: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub current_revision_id: Option<Uuid>,
    pub pending_revision_id: Option<Uuid>,
    pub revisions: Vec<RevisionView>,
}

/// 记忆列表条目。
#[derive(Debug, Clone, Serialize)]
pub struct MemorySummary {
    pub memory_id: Uuid,
    pub state: String,
    pub created_at: DateTime<Utc>,
    /// 当前版本（active 时）或最后一版（withdrawn 时）或首提案（proposed 时）。
    pub headline: RevisionView,
    pub pending_revision_id: Option<Uuid>,
}

async fn load_revisions(
    pool: &PgPool,
    memory_ids: &[Uuid],
) -> Result<Vec<(Uuid, RevisionView)>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT memory_id, id, revision_no, change_action, base_revision_id, memory_type, title, \
                summary, body, change_reason, content_hash, proposed_by, created_at, review_status, \
                reviewed_by, reviewed_at, review_note, version_no, version_status \
           FROM kb.memory_revision WHERE memory_id = ANY($1) \
          ORDER BY memory_id, revision_no",
    )
    .bind(memory_ids)
    .fetch_all(pool)
    .await?;
    let revision_ids: Vec<Uuid> = rows.iter().map(|r| r.get("id")).collect();
    let ref_rows = sqlx::query(
        "SELECT revision_id, ref_role, target_domain, target_type, target_id, target_revision_id, \
                revision_missing_reason, locator \
           FROM kb.memory_ref WHERE revision_id = ANY($1) ORDER BY created_at, id",
    )
    .bind(&revision_ids)
    .fetch_all(pool)
    .await?;
    let mut refs: std::collections::HashMap<Uuid, Vec<RefView>> = std::collections::HashMap::new();
    for r in ref_rows {
        refs.entry(r.get("revision_id")).or_default().push(RefView {
            ref_role: r.get("ref_role"),
            target_domain: r.get("target_domain"),
            target_type: r.get("target_type"),
            target_id: r.get("target_id"),
            target_revision_id: r.get("target_revision_id"),
            revision_missing_reason: r.get("revision_missing_reason"),
            locator: r.get("locator"),
        });
    }
    Ok(rows
        .into_iter()
        .map(|r| {
            let id: Uuid = r.get("id");
            (
                r.get("memory_id"),
                RevisionView {
                    revision_id: id,
                    revision_no: r.get("revision_no"),
                    change_action: r.get("change_action"),
                    base_revision_id: r.get("base_revision_id"),
                    memory_type: r.get("memory_type"),
                    title: r.get("title"),
                    summary: r.get("summary"),
                    body: r.get("body"),
                    change_reason: r.get("change_reason"),
                    content_hash: r.get("content_hash"),
                    proposed_by: r.get("proposed_by"),
                    created_at: r.get("created_at"),
                    review_status: r.get("review_status"),
                    reviewed_by: r.get("reviewed_by"),
                    reviewed_at: r.get("reviewed_at"),
                    review_note: r.get("review_note"),
                    version_no: r.get("version_no"),
                    version_status: r.get("version_status"),
                    refs: refs.remove(&id).unwrap_or_default(),
                },
            )
        })
        .collect())
}

/// 记忆详情（项目内；跨项目 / 不存在一律 NotFound）。
pub async fn get_memory(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    memory_id: Uuid,
) -> Result<MemoryDetail, KbServiceError> {
    let row = sqlx::query(
        "SELECT id, created_by, created_at FROM kb.memory \
          WHERE id = $1 AND tenant_id = $2 AND project_id = $3",
    )
    .bind(memory_id)
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or(KbServiceError::NotFound("memory not found"))?;
    let revisions: Vec<RevisionView> = load_revisions(pool, &[memory_id])
        .await?
        .into_iter()
        .map(|(_, r)| r)
        .collect();
    Ok(MemoryDetail {
        memory_id,
        project_id,
        state: derive_state(&revisions).into(),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        current_revision_id: revisions
            .iter()
            .find(|r| r.version_status.as_deref() == Some("CURRENT"))
            .map(|r| r.revision_id),
        pending_revision_id: revisions
            .iter()
            .find(|r| r.review_status == "PENDING_REVIEW")
            .map(|r| r.revision_id),
        revisions,
    })
}

/// 记忆列表（项目内，新建在前）；`state` 过滤：`active` / `withdrawn` / `proposed` /
/// `not_adopted` / `pending_review`（有待审提案，含已生效记忆的修改提案）。
pub async fn list_memories(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    state: Option<&str>,
) -> Result<Vec<MemorySummary>, KbServiceError> {
    if let Some(s) = state {
        if ![
            "active",
            "withdrawn",
            "proposed",
            "not_adopted",
            "pending_review",
        ]
        .contains(&s)
        {
            return Err(KbServiceError::Invalid(
                "state must be one of active | withdrawn | proposed | not_adopted | pending_review"
                    .into(),
            ));
        }
    }
    let memories = sqlx::query(
        "SELECT id, created_at FROM kb.memory WHERE tenant_id = $1 AND project_id = $2 \
          ORDER BY created_at DESC, id LIMIT 500",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let ids: Vec<Uuid> = memories.iter().map(|r| r.get("id")).collect();
    let mut grouped: std::collections::HashMap<Uuid, Vec<RevisionView>> =
        std::collections::HashMap::new();
    for (memory_id, rev) in load_revisions(pool, &ids).await? {
        grouped.entry(memory_id).or_default().push(rev);
    }
    let mut out = Vec::with_capacity(memories.len());
    for m in memories {
        let memory_id: Uuid = m.get("id");
        let revisions = grouped.remove(&memory_id).unwrap_or_default();
        let derived = derive_state(&revisions);
        let pending = revisions
            .iter()
            .find(|r| r.review_status == "PENDING_REVIEW")
            .map(|r| r.revision_id);
        let keep = match state {
            None => true,
            Some("pending_review") => pending.is_some(),
            Some(s) => s == derived,
        };
        if !keep {
            continue;
        }
        let headline = revisions
            .iter()
            .find(|r| r.version_status.as_deref() == Some("CURRENT"))
            .or_else(|| revisions.iter().rev().find(|r| r.version_status.is_some()))
            .or_else(|| revisions.last())
            .cloned();
        let Some(headline) = headline else { continue };
        out.push(MemorySummary {
            memory_id,
            state: derived.into(),
            created_at: m.get("created_at"),
            headline,
            pending_revision_id: pending,
        });
    }
    Ok(out)
}

/// 被审修订的四元组（K4 端口返回映射用，差异说明 D-KP3-2）。
pub fn revision_ref(revision_id: Uuid) -> kb_pc_contracts::DomainRef {
    kb_pc_contracts::DomainRef::for_domain(
        TargetDomain::KnowledgeBase,
        TYPE_MEMORY_REVISION,
        revision_id.to_string(),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(id: &str) -> ResolvedRef {
        ResolvedRef {
            role: RefRole::Evidence,
            target_domain: "requirement".into(),
            target_type: "conversation_message".into(),
            target_id: id.into(),
            target_revision_id: None,
            revision_missing_reason: Some("no_revision_mechanism".into()),
            locator: None,
        }
    }

    fn content() -> MemoryContent {
        MemoryContent {
            memory_type: "rejected_option".into(),
            title: "放弃方案 X".into(),
            summary: "因 Z 否决".into(),
            body: None,
        }
    }

    #[test]
    fn refs_require_evidence() {
        let mut subject = evidence("t1");
        subject.role = RefRole::Subject;
        assert!(validate_refs(&[subject]).is_err());
        assert!(validate_refs(&[evidence("m1")]).is_ok());
    }

    #[test]
    fn refs_reject_unknown_domain_and_duplicates() {
        let mut bad = evidence("m1");
        bad.target_domain = "kb".into();
        assert!(validate_refs(&[bad]).is_err());
        assert!(validate_refs(&[evidence("m1"), evidence("m1")]).is_err());
    }

    #[test]
    fn refs_require_pin_or_reason() {
        let mut r = evidence("m1");
        r.revision_missing_reason = None;
        assert!(validate_refs(&[r]).is_err());
    }

    #[test]
    fn closed_channel_domain_is_rejected() {
        // design/adr 通道已随 S03 后端落地开放（2026-11-10，registry.rs），
        // 引用可解析；仍未开放的 product_reality 必须被拒（PC-01B §13.1）。
        let mut design_ref = evidence("d1");
        design_ref.target_domain = "design".into();
        assert!(validate_refs(&[design_ref]).is_ok());
        let mut adr_ref = evidence("d1");
        adr_ref.target_domain = "adr".into();
        assert!(validate_refs(&[adr_ref]).is_ok());
        let mut closed = evidence("d1");
        closed.target_domain = "product_reality".into();
        assert!(validate_refs(&[closed]).is_err());
        let mut unknown = evidence("d1");
        unknown.target_domain = "kb_typo".into();
        assert!(validate_refs(&[unknown]).is_err());
    }

    #[test]
    fn content_hash_ignores_ref_order_and_whitespace() {
        let a = vec![evidence("m1"), evidence("m2")];
        let b = vec![evidence("m2"), evidence("m1")];
        let mut c2 = content();
        c2.title = format!("  {}  ", c2.title);
        assert_eq!(content_hash(&content(), &a), content_hash(&c2, &b));
        let mut c3 = content();
        c3.summary = "不同".into();
        assert_ne!(content_hash(&content(), &a), content_hash(&c3, &a));
    }

    #[test]
    fn action_vocabularies() {
        assert_eq!(ReviewAction::parse("approve"), Some(ReviewAction::Approve));
        assert_eq!(ReviewAction::parse("revise"), Some(ReviewAction::Revise));
        assert_eq!(ReviewAction::parse("APPROVE"), None);
        assert_eq!(
            ChangeAction::parse("withdraw"),
            Some(ChangeAction::Withdraw)
        );
        assert_eq!(ChangeAction::parse("merge"), None);
    }
}
