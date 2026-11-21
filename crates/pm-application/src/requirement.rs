//! S02-I1 · Requirement 需求核心统一服务（S02-04 V1.2 §4/§5/§7/§13/§15.5 落地）。
//!
//! 九命令的**单点实现**（S02D-01/DEF-04）：生命周期规则、并发（CAS 双令牌）、
//! 确认协议（六要素 + 真人主体）、审计词表（S02D-23 点号式）只在本模块实现一次；
//! REST 适配层（pm-api `pm_requirements`）不做业务裁决。MCP 不提供确认类工具
//! （§7.2 A*）；执行器 `ApproveRequirementRevision` 的退役归 S02-I6-B（DEF-02
//! 执行器面），本批只收编 REST 面。
//!
//! 授权（§12.1 DEF-01 六点的服务面）：
//! - 租户边界：所有入口按 `tenant_id` 过滤目标行，越权一律 404（不泄露存在性）；
//! - 项目成员：由 pm-api 适配层 `require_project_visible` 把门（读侧同口径）；
//! - 确认/拒绝/撤回/恢复/归档（I6-A 终态，O-6 裁决）：真人主体
//!   （`core.principal.kind='human'`，S02D-02 #2）+ 可信交互上下文（REST
//!   适配层拒 `api_key` 鉴权，§7.2 A*）+ 项目成员 +
//!   （Lead OR `core.approver_grant` 生效行——`approver::ensure_governance_authorized`；
//!   Q-A1「Lead+显式 Approver」宽读，§7.4 严格读法偏差已在迁移 0040 注记）。
//!
//! 并发（DEF-03 修复方向，I1 §7）：CAS 双令牌 `expected_row_revision` +
//! `expected_current_revision_id`，后至者显式 409；确认请求必须携带
//! `expected_content_hash`（审批人见到并核准的正文哈希，跨版本重放防线，
//! S02D-16/S02D-21）。幂等键机制（I1-D12 重放幂等）归 I6-B/O-5，V1 重放一律 409。
//!
//! 审计（S02D-23）：点号式词表；`governance.audit_log` 无 payload 列，before/after
//! 由 Confirmation 行 + 修订链可推导，V1 不加列。outbox 事件族已在
//! [`crate::outbox::CATALOG`] 注册（S02D-22，`has_handler=false` 不生产——
//! 「不假装消费」，消费方落地后再启用）。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::{append_audit, hex_sha256, lock_head_row, lock_revision_row, PmWriteError};

pub const REQUIREMENT_TYPES: [&str; 2] = ["functional", "non_functional"];
pub const REQUIREMENT_PRIORITIES: [&str; 3] = ["low", "normal", "high"];
pub const REQUIREMENT_LIFECYCLE: [&str; 3] = ["active", "withdrawn", "archived"];

// ─────────────────────────────────────────────────────────────── 视图

/// 需求条目（列表/详情共用）：需求本体 + 当前正式版本 + 最新修订（状态 chip 数据源）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct RequirementEntry {
    pub id: Uuid,
    pub project_id: Uuid,
    pub code: String,
    pub title: String,
    pub requirement_type: String,
    pub priority: String,
    pub lifecycle_status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// CAS 双令牌之一（I1 §7）：并发写命令的客户端可见乐观锁令牌。
    pub row_revision: i64,
    /// 当前正式版本（`NULL` = 尚无正式版本，S02D-04：是否有正式版本由此表达）。
    pub current_revision_id: Option<Uuid>,
    pub current_version: Option<i32>,
    pub current_content_hash: Option<String>,
    /// 最新修订（任意状态）。
    pub latest_revision_id: Option<Uuid>,
    pub latest_version: Option<i32>,
    pub latest_state: Option<String>,
    pub latest_submitted_at: Option<DateTime<Utc>>,
}

/// 修订视图：`markdown`/`summary` 从 content 投影列提取（content 为可重建投影，
/// S02D-03；权威正文协议的 Git 绑定归 S02-I2）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct RevisionView {
    pub id: Uuid,
    pub requirement_id: Uuid,
    pub version: i32,
    pub state: String,
    pub content: serde_json::Value,
    pub markdown: Option<String>,
    pub summary: Option<String>,
    pub content_hash: String,
    /// Markdown 版本引用（I2 定结构；V1 恒 `NULL`）。
    pub markdown_version_ref: Option<String>,
    /// 起草时基于的当前版本（变更识别的输入，§8.3 三步第一步）。
    pub base_current_revision_id: Option<Uuid>,
    pub base_row_revision: Option<i64>,
    pub submitted_at: Option<DateTime<Utc>>,
    /// 提案级来源标记（DK-0 §3.3 / D-DK-5）：编排器随 draft 写入的
    /// "我依据了这些 finding"，**非权威**——不进 `requirement_finding`，
    /// 不参与任何校验；正式溯源关联仍由真人 `link_finding` 物化（S02D-20）。
    /// 缺省 `NULL`（历史修订与非 AI 起草修订没有该子键）。
    pub basis_finding_ids: Option<serde_json::Value>,
    pub created_by: Uuid,
    pub created_by_name: Option<String>,
    /// 过渡列（I1-D09：读取消费者迁 Confirmation 后删列）。
    pub approved_by: Option<Uuid>,
    pub approved_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// 确认记录视图（S02D-21：一次确认/拒绝行为的不可变事实）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct ConfirmationView {
    pub id: Uuid,
    pub revision_id: Uuid,
    pub action: String,
    pub reason: Option<String>,
    pub human_principal_id: Uuid,
    pub human_display_name: Option<String>,
    pub identity_id: Option<Uuid>,
    pub source: String,
    pub content_hash: Option<String>,
    pub markdown_version_ref: Option<String>,
    pub confirmed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RequirementDetail {
    pub requirement: RequirementEntry,
    pub revisions: Vec<RevisionView>,
    pub confirmations: Vec<ConfirmationView>,
}

// ─────────────────────────────────────────────────────────────── 请求

#[derive(Debug, Deserialize)]
pub struct CreateRequirementRequest {
    pub code: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub requirement_type: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    /// 首版草稿正文（缺省 = `# {title}`）。
    #[serde(default)]
    pub markdown: Option<String>,
    /// 提案级来源标记（DK-0 §3.3）：本 session 内**已 CONFIRMED** 的 finding id。
    /// 缺省 = 不带标记（人类直接创建需求的常态）；带标记时整条草案要么全合法、
    /// 要么整条拒绝（不写带病提案）。
    #[serde(default)]
    pub basis_finding_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct DraftRevisionRequest {
    pub markdown: String,
    #[serde(default)]
    pub summary: Option<String>,
    /// 提案级来源标记（见 [`CreateRequirementRequest::basis_finding_ids`]）。
    /// `None` = 不改（`update_draft` 保留原清单）；`Some([])` = 显式清空。
    #[serde(default)]
    pub basis_finding_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmRevisionRequest {
    /// 审批人见到并核准的正文哈希；与库存 `content_hash` 不符即 409（§7.1）。
    pub expected_content_hash: String,
    #[serde(default)]
    pub expected_current_revision_id: Option<Uuid>,
    #[serde(default)]
    pub expected_row_revision: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RejectRevisionRequest {
    #[serde(default)]
    pub reason: Option<String>,
}

/// 需求级生命周期治理命令（§5.2；Approver/Lead 职责）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleAction {
    Withdraw,
    Restore,
    Archive,
}

impl LifecycleAction {
    pub fn parse(action: &str) -> Option<Self> {
        match action {
            "withdraw" => Some(Self::Withdraw),
            "restore" => Some(Self::Restore),
            "archive" => Some(Self::Archive),
            _ => None,
        }
    }

    fn audit_action(self) -> &'static str {
        match self {
            Self::Withdraw => "requirement.withdrawn",
            Self::Restore => "requirement.restored",
            Self::Archive => "requirement.archived",
        }
    }
}

// ─────────────────────────────────────────────────────────── 内部助手

/// 真人主体校验（S02D-02 #2：AI 主体提交确认/治理类命令 → 拒绝，403 可观测）。
/// S02-I3 起供 `requirement_traceability` 复用（登记/关联同为真人命令，§7.4）；
/// DK-D 起供任务 `draft → ready` 复用（§7.4「AI 永不置 ready」，E2）。
pub async fn require_human(pool: &PgPool, actor: &AuditActor) -> Result<(), PmWriteError> {
    let mut conn = pool.acquire().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut conn, actor).await
}

/// [`require_human`] 的连接版：调用方已在事务内时用它，避免事务中再取第二条
/// 连接（单连接池下会自锁）。判定逻辑只有这一处。
pub async fn require_human_conn(
    conn: &mut sqlx::PgConnection,
    actor: &AuditActor,
) -> Result<(), PmWriteError> {
    let kind: Option<String> = sqlx::query_scalar("SELECT kind FROM core.principal WHERE id = $1")
        .bind(actor.principal_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(PmWriteError::Db)?;
    match kind.as_deref() {
        Some("human") => Ok(()),
        Some(_) => Err(PmWriteError::Forbidden(
            "confirmation and lifecycle commands require a human principal",
        )),
        None => Err(PmWriteError::NotFound("principal not found")),
    }
}

/// 修订视图的 SELECT 片段（content 投影提取 + principal 显示名 JOIN）。
const REVISION_VIEW_COLS: &str = "rv.id, rv.requirement_id, rv.version, rv.state, rv.content, \
     rv.content->>'markdown' AS markdown, rv.content->>'summary' AS summary, \
     rv.content->'basis_finding_ids' AS basis_finding_ids, rv.content_hash, \
     rv.markdown_version_ref, rv.base_current_revision_id, rv.base_row_revision, rv.submitted_at, \
     rv.created_by, cb.display_name AS created_by_name, rv.approved_by, ab.display_name AS approved_by_name, \
     rv.created_at";
const REVISION_VIEW_FROM: &str = "FROM delivery.requirement_revision rv \
     LEFT JOIN core.principal cb ON cb.id = rv.created_by \
     LEFT JOIN core.principal ab ON ab.id = rv.approved_by";

/// 按 id 取需求条目（列表 SQL 的单行形态；租户过滤内嵌）。
async fn fetch_entry(
    db: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
    requirement_id: Uuid,
) -> Result<RequirementEntry, PmWriteError> {
    sqlx::query_as(
        "SELECT r.id, r.project_id, r.code, r.title, r.requirement_type, r.priority, \
                r.lifecycle_status, r.created_at, r.updated_at, r.row_revision, \
                cur.id   AS current_revision_id, cur.version AS current_version, \
                cur.content_hash AS current_content_hash, \
                lat.id   AS latest_revision_id, lat.version AS latest_version, \
                lat.state AS latest_state, lat.submitted_at AS latest_submitted_at \
         FROM delivery.requirement r \
         LEFT JOIN delivery.requirement_revision cur ON cur.id = r.current_revision_id \
         LEFT JOIN LATERAL ( \
             SELECT v.id, v.version, v.state, v.submitted_at \
             FROM delivery.requirement_revision v \
             WHERE v.requirement_id = r.id ORDER BY v.version DESC LIMIT 1 \
         ) lat ON TRUE \
         WHERE r.tenant_id = $1 AND r.id = $2",
    )
    .bind(tenant_id)
    .bind(requirement_id)
    .fetch_optional(db)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("requirement not found"))
}

/// 需求行加锁读取（写命令公共前置：租户边界 + 行锁）。
/// （通用实现见 `pm_write::lock_head_row`。）
async fn lock_requirement(
    tx: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    requirement_id: Uuid,
) -> Result<(Uuid, String, Option<Uuid>, i64), PmWriteError> {
    lock_head_row(
        tx,
        tenant_id,
        requirement_id,
        "delivery.requirement",
        "lifecycle_status",
        "requirement not found",
    )
    .await
}

/// 修订行加锁读取：(version, state, content_hash, markdown_version_ref)。
/// （通用实现见 `pm_write::lock_revision_row`；本表旧版锁缺租户过滤，此处补上。）
async fn lock_revision(
    tx: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    requirement_id: Uuid,
    revision_id: Uuid,
) -> Result<(i32, String, String, Option<String>), PmWriteError> {
    lock_revision_row(
        tx,
        tenant_id,
        revision_id,
        requirement_id,
        "delivery.requirement_revision",
        "requirement_id",
        "version, state, content_hash, markdown_version_ref",
        "revision not found",
    )
    .await
}

/// 写命令公共前置：需求必须 active（withdrawn/archived 不再接受新的治理写入；
/// 历史 Revision 与既有执行依据不受影响，§5.5）。
fn ensure_active(lifecycle_status: &str) -> Result<(), PmWriteError> {
    if lifecycle_status != "active" {
        return Err(PmWriteError::Conflict("requirement is not active"));
    }
    Ok(())
}

fn valid_or_default<'a>(
    value: &'a Option<String>,
    allowed: &[&str],
    default: &'a str,
    label: &'static str,
) -> Result<&'a str, PmWriteError> {
    match value.as_deref().map(str::trim) {
        None => Ok(default),
        Some("") => Ok(default),
        Some(v) if allowed.contains(&v) => Ok(v),
        Some(_) => Err(PmWriteError::Validation(label)),
    }
}

// ─────────────────────────────────────── 提案级来源标记（DK-0 §3.3 / D-DK-5）

/// `content.basis_finding_ids` 的硬边界（设计稿 §3.3 边界纪律）。
///
/// 三条同时成立才写进草案，任一条失败 → 整条草案拒绝生成（不留带病提案）：
/// 1. **存在且同项目**：id 必须在 `(tenant_id, project_id)` 内能找到
///    （跨项目/跨租户按不存在处理，不泄露存在性）；
/// 2. **已 CONFIRMED**：`verification_status='CONFIRMED'` 才是可信访谈事实
///    （澄清 ≠ 确认，但草案只能立在已澄清事实上）；
/// 3. **本 session 内**：所有 id 必须落在同一个 `discovery_session`——请求体
///    不带 session 参数（不扩大设计面），因此以「清单自身单一 session」表达
///    「本 session」；跨 session 混合即拒绝。
///
/// 只做校验与去重保序，不落任何关联表：正式溯源仍由真人
/// `link_finding` / `register_source_input` 物化（S02D-20 不动）。
async fn resolve_basis_findings(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    project_id: Uuid,
    ids: &[Uuid],
) -> Result<Vec<Uuid>, PmWriteError> {
    let mut deduped: Vec<Uuid> = Vec::with_capacity(ids.len());
    for id in ids {
        if !deduped.contains(id) {
            deduped.push(*id);
        }
    }
    if deduped.is_empty() {
        return Ok(deduped);
    }

    let rows: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT id, session_id, verification_status FROM discovery.discovery_finding \
         WHERE tenant_id = $1 AND project_id = $2 AND id = ANY($3)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&deduped)
    .fetch_all(&mut *conn)
    .await
    .map_err(PmWriteError::Db)?;

    if rows.len() != deduped.len() {
        return Err(PmWriteError::Validation(
            "basis finding not found in this project — draft rejected",
        ));
    }
    if rows.iter().any(|(_, _, status)| status != "CONFIRMED") {
        return Err(PmWriteError::Validation(
            "basis finding is not CONFIRMED — draft rejected",
        ));
    }
    let mut sessions: Vec<Uuid> = rows.iter().map(|(_, session, _)| *session).collect();
    sessions.sort();
    sessions.dedup();
    if sessions.len() > 1 {
        return Err(PmWriteError::Validation(
            "basis findings span multiple discovery sessions — draft rejected",
        ));
    }
    Ok(deduped)
}

/// 把已校验的 basis 清单写进修订 content（与 `markdown`/`summary` 并列）。
/// 空清单不写子键（"没有标记" 与 "标记为空数组" 语义相同，统一取前者）。
fn with_basis(mut content: serde_json::Value, basis: &[Uuid]) -> serde_json::Value {
    if !basis.is_empty() {
        content["basis_finding_ids"] = json!(basis);
    }
    content
}

// ─────────────────────────────────────────────────────────────── 命令

/// 创建需求（CreateRequirement）：lifecycle_status='active' + 首版 v1 草稿
/// （§5.2 创建即 active；§6.6 导入与运行时新建共用语义，不自动定稿）。
pub async fn create_requirement(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    req: CreateRequirementRequest,
    actor: &AuditActor,
) -> Result<RequirementEntry, PmWriteError> {
    let code = req.code.trim().to_string();
    let title = req.title.trim().to_string();
    if code.is_empty() || title.is_empty() {
        return Err(PmWriteError::Validation("code and title are required"));
    }
    let requirement_type = valid_or_default(
        &req.requirement_type,
        &REQUIREMENT_TYPES,
        "functional",
        "invalid requirement_type",
    )?;
    let priority = valid_or_default(
        &req.priority,
        &REQUIREMENT_PRIORITIES,
        "normal",
        "invalid priority",
    )?;
    let markdown = req
        .markdown
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("# {title}"));
    let summary = req
        .description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let mut tx = pool.begin().await?;
    let exists: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM core.project WHERE id = $1 AND tenant_id = $2")
            .bind(project_id)
            .bind(tenant_id)
            .fetch_optional(&mut *tx)
            .await?;
    if exists.is_none() {
        return Err(PmWriteError::NotFound("unknown project"));
    }

    let requirement_id = Uuid::new_v4();
    let inserted = sqlx::query(
        "INSERT INTO delivery.requirement \
         (id, project_id, code, title, lifecycle_status, requirement_type, priority, \
          created_by, tenant_id) \
         VALUES ($1, $2, $3, $4, 'active', $5, $6, $7, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(requirement_id)
    .bind(project_id)
    .bind(&code)
    .bind(&title)
    .bind(requirement_type)
    .bind(priority)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await;
    if let Err(err) = inserted {
        // 项目内唯一 code（0002 UNIQUE(project_id, code)）；其余唯一键不可能撞新 uuid。
        if let sqlx::Error::Database(db) = &err {
            if db.code().as_deref() == Some("23505") {
                return Err(PmWriteError::Conflict(
                    "requirement code already exists in this project",
                ));
            }
        }
        return Err(PmWriteError::Db(err));
    }

    let revision_id = Uuid::new_v4();
    let basis = match &req.basis_finding_ids {
        Some(ids) => resolve_basis_findings(&mut tx, tenant_id, project_id, ids).await?,
        None => Vec::new(),
    };
    let mut content = json!({ "markdown": markdown });
    if let Some(s) = &summary {
        content["summary"] = json!(s);
    }
    let content = with_basis(content, &basis);
    let content_hash = hex_sha256(markdown.as_bytes());
    sqlx::query(
        "INSERT INTO delivery.requirement_revision \
         (id, requirement_id, project_id, version, content, state, content_hash, \
          created_by, tenant_id) \
         VALUES ($1, $2, $3, 1, $4, 'draft', $5, $6, \
                 (SELECT tenant_id FROM core.project WHERE id = $3))",
    )
    .bind(revision_id)
    .bind(requirement_id)
    .bind(project_id)
    .bind(&content)
    .bind(&content_hash)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.created",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    // PA-05-01/05（注册表）：创建即注册统一事项 + Timeline `created` 活动。
    let work_item_id = crate::work_item::ensure_work_item(
        &mut tx,
        tenant_id,
        "requirement",
        requirement_id,
        Some(project_id),
        &title,
        crate::work_item::requirement_status_semantic("active"),
        Some(actor.principal_id),
    )
    .await?;
    crate::work_item::record_activity(
        &mut tx,
        tenant_id,
        work_item_id,
        Some(project_id),
        None,
        "created",
        "human",
        Some(actor.principal_id),
        &format!("Requirement created: {title}"),
        None,
        None,
    )
    .await?;
    tx.commit().await?;

    fetch_entry(pool, tenant_id, requirement_id).await
}

/// 起草新修订（CreateRequirementRevision）：version 单调递增 + 基版本快照
/// （base_current_revision_id/base_row_revision，§8.3 变更识别输入）。
pub async fn create_draft(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    req: DraftRevisionRequest,
    actor: &AuditActor,
) -> Result<RevisionView, PmWriteError> {
    let markdown = req.markdown.trim().to_string();
    if markdown.is_empty() {
        return Err(PmWriteError::Validation("markdown is required"));
    }
    let mut tx = pool.begin().await?;
    let (project_id, lifecycle, current_revision_id, row_revision) =
        lock_requirement(&mut tx, tenant_id, requirement_id).await?;
    ensure_active(&lifecycle)?;

    let version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(max(version), 0) + 1 FROM delivery.requirement_revision \
         WHERE requirement_id = $1",
    )
    .bind(requirement_id)
    .fetch_one(&mut *tx)
    .await?;

    let mut content = json!({ "markdown": markdown });
    if let Some(s) = req
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        content["summary"] = json!(s);
    }
    let basis = match &req.basis_finding_ids {
        Some(ids) => resolve_basis_findings(&mut tx, tenant_id, project_id, ids).await?,
        None => Vec::new(),
    };
    let content = with_basis(content, &basis);
    let content_hash = hex_sha256(markdown.as_bytes());
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.requirement_revision \
         (id, requirement_id, project_id, version, content, state, content_hash, \
          base_current_revision_id, base_row_revision, created_by, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, 'draft', $6, $7, $8, $9, \
                 (SELECT tenant_id FROM core.project WHERE id = $3))",
    )
    .bind(revision_id)
    .bind(requirement_id)
    .bind(project_id)
    .bind(version)
    .bind(&content)
    .bind(&content_hash)
    .bind(current_revision_id)
    .bind(row_revision)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.revision_drafted",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;

    sqlx::query_as::<_, RevisionView>(&format!(
        "SELECT {REVISION_VIEW_COLS} {REVISION_VIEW_FROM} WHERE rv.id = $1"
    ))
    .bind(revision_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 修改草稿（UpdateRequirementDraft）：仅 draft 可改（pending 起冻结，I1 §5.4）。
pub async fn update_draft(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    revision_id: Uuid,
    req: DraftRevisionRequest,
    actor: &AuditActor,
) -> Result<RevisionView, PmWriteError> {
    let markdown = req.markdown.trim().to_string();
    if markdown.is_empty() {
        return Err(PmWriteError::Validation("markdown is required"));
    }
    let mut tx = pool.begin().await?;
    let (project_id, lifecycle, _, _) =
        lock_requirement(&mut tx, tenant_id, requirement_id).await?;
    ensure_active(&lifecycle)?;

    let (_, state, _, _) = lock_revision(&mut tx, tenant_id, requirement_id, revision_id).await?;
    if state != "draft" {
        return Err(PmWriteError::Conflict("only draft revisions can be edited"));
    }

    let mut content = json!({ "markdown": markdown });
    if let Some(s) = req
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        content["summary"] = json!(s);
    }
    // DK-0 §3.3：`None` = 不改（保留原标记——content 是全量重建的，
    // 不显式保留就会静默丢标记）；`Some([])` = 显式清空；`Some(ids)` = 替换。
    let basis = match &req.basis_finding_ids {
        Some(ids) => resolve_basis_findings(&mut tx, tenant_id, project_id, ids).await?,
        None => sqlx::query_scalar::<_, Option<serde_json::Value>>(
            "SELECT content->'basis_finding_ids' FROM delivery.requirement_revision WHERE id = $1",
        )
        .bind(revision_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?
        .and_then(|value| serde_json::from_value::<Vec<Uuid>>(value).ok())
        .unwrap_or_default(),
    };
    let content = with_basis(content, &basis);
    let content_hash = hex_sha256(markdown.as_bytes());
    sqlx::query(
        "UPDATE delivery.requirement_revision SET content = $3, content_hash = $4 \
         WHERE id = $1 AND requirement_id = $2",
    )
    .bind(revision_id)
    .bind(requirement_id)
    .bind(&content)
    .bind(&content_hash)
    .execute(&mut *tx)
    .await?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.revision_drafted",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;

    sqlx::query_as::<_, RevisionView>(&format!(
        "SELECT {REVISION_VIEW_COLS} {REVISION_VIEW_FROM} WHERE rv.id = $1"
    ))
    .bind(revision_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 提交确认（SubmitRequirementRevision，S02-I2 唯一绑定点）：draft →
/// pending_confirmation + 投影绑定（Markdown 写 Git、markdown_version_ref/
/// content_hash 刷新，§5.4 冻结 + §6.4 映射）。
///
/// 重入语义（§6.7 崩溃窗口）：`pending_confirmation` 且**未绑定**的修订可重复
/// submit——只补投影绑定，不重复翻转状态/审计（首次 submit 已落审计）；
/// 已绑定的重入一律 409。
pub async fn submit_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    revision_id: Uuid,
    actor: &AuditActor,
    root: &crate::knowledge::KnowledgeRoot,
) -> Result<RevisionView, PmWriteError> {
    let mut tx = pool.begin().await?;
    let (project_id, lifecycle, _, _) =
        lock_requirement(&mut tx, tenant_id, requirement_id).await?;
    ensure_active(&lifecycle)?;

    let (_, state, _, _) = lock_revision(&mut tx, tenant_id, requirement_id, revision_id).await?;
    let (markdown,): (Option<String>,) = sqlx::query_as(
        "SELECT content->>'markdown' FROM delivery.requirement_revision WHERE id = $1",
    )
    .bind(revision_id)
    .fetch_one(&mut *tx)
    .await?;
    let markdown = markdown.unwrap_or_default();

    match state.as_str() {
        "draft" => {
            sqlx::query(
                "UPDATE delivery.requirement_revision \
                 SET state = 'pending_confirmation', submitted_at = now() \
                 WHERE id = $1",
            )
            .bind(revision_id)
            .execute(&mut *tx)
            .await?;
            append_audit(
                &mut tx,
                project_id,
                actor,
                "requirement.revision_submitted",
                "requirement",
                Some(requirement_id),
            )
            .await?;
        }
        "pending_confirmation" => {
            let bound: Option<String> = sqlx::query_scalar(
                "SELECT markdown_version_ref FROM delivery.requirement_revision WHERE id = $1",
            )
            .bind(revision_id)
            .fetch_optional(&mut *tx)
            .await?
            .flatten();
            if bound.is_some() {
                return Err(PmWriteError::Conflict("revision is already submitted"));
            }
            // 未绑定的 pending = 上次 submit 在状态翻转后、绑定前中断 → 本次只补绑定。
        }
        _ => {
            return Err(PmWriteError::Conflict(
                "only draft revisions can be submitted",
            ));
        }
    }
    tx.commit().await?;

    if markdown.trim().is_empty() {
        return Err(PmWriteError::Validation("revision markdown is empty"));
    }
    crate::requirement_projection::bind_projection(
        pool,
        root,
        project_id,
        requirement_id,
        revision_id,
        &markdown,
        actor.principal_id,
    )
    .await?;

    sqlx::query_as::<_, RevisionView>(&format!(
        "SELECT {REVISION_VIEW_COLS} {REVISION_VIEW_FROM} WHERE rv.id = $1"
    ))
    .bind(revision_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 正式确认（ConfirmRequirementRevision，S02D-21/DEF-02 服务面）：
/// 六要素 + CAS 双令牌 + 真人主体 + Confirmation 落账；旧 approved → superseded；
/// current_revision_id 原子迁移；row_revision+1。
pub async fn confirm_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    revision_id: Uuid,
    req: ConfirmRevisionRequest,
    actor: &AuditActor,
) -> Result<RequirementEntry, PmWriteError> {
    require_human(pool, actor).await?;
    let expected_hash = req.expected_content_hash.trim().to_string();
    if expected_hash.is_empty() {
        return Err(PmWriteError::Validation(
            "expected_content_hash is required",
        ));
    }

    let mut tx = pool.begin().await?;
    let (project_id, lifecycle, current_revision_id, row_revision) =
        lock_requirement(&mut tx, tenant_id, requirement_id).await?;
    ensure_active(&lifecycle)?;
    // I6-A：确认 = Lead OR 显式 Approver（成员闸由 REST 层 require_project_visible 已过）。
    crate::approver::ensure_governance_authorized(pool, project_id, actor.principal_id).await?;

    // CAS 双令牌（I1 §7）：任一令牌漂移 → 显式 409（DEF-03 修复方向）。
    if let Some(expected) = req.expected_row_revision {
        if expected != row_revision {
            return Err(PmWriteError::Conflict(
                "requirement changed concurrently (row_revision mismatch)",
            ));
        }
    }
    if let Some(expected) = req.expected_current_revision_id {
        if Some(expected) != current_revision_id {
            return Err(PmWriteError::Conflict(
                "current revision moved concurrently (current_revision_id mismatch)",
            ));
        }
    }

    let (_, state, content_hash, markdown_version_ref) =
        lock_revision(&mut tx, tenant_id, requirement_id, revision_id).await?;
    if state != "pending_confirmation" {
        return Err(PmWriteError::Conflict(
            "revision is not pending confirmation",
        ));
    }
    // 六要素之一（I2 §6.4-2）：必须已绑定 Markdown 版本——submit 是唯一绑定点。
    if markdown_version_ref.is_none() {
        return Err(PmWriteError::Conflict(
            "revision is not bound to a markdown version — submit again to bind",
        ));
    }
    // 六要素之一：审批人核准的哈希必须与库存一致（跨版本重放防线，§7.1）。
    if content_hash != expected_hash {
        return Err(PmWriteError::Conflict(
            "content hash mismatch — revision drifted since it was reviewed",
        ));
    }

    // 旧正式版本 → superseded（终态保留，永不删除）。
    let superseded: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE delivery.requirement_revision SET state = 'superseded' \
         WHERE requirement_id = $1 AND state = 'approved' AND id <> $2 RETURNING id",
    )
    .bind(requirement_id)
    .bind(revision_id)
    .fetch_all(&mut *tx)
    .await?;
    for _old in &superseded {
        append_audit(
            &mut tx,
            project_id,
            actor,
            "requirement.revision_superseded",
            "requirement",
            Some(requirement_id),
        )
        .await?;
    }

    sqlx::query(
        "UPDATE delivery.requirement_revision SET state = 'approved', approved_by = $2 \
         WHERE id = $1",
    )
    .bind(revision_id)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE delivery.requirement \
         SET current_revision_id = $2, row_revision = row_revision + 1, \
             updated_at = now(), updated_by = $3 \
         WHERE id = $1",
    )
    .bind(requirement_id)
    .bind(revision_id)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO delivery.requirement_confirmation \
         (project_id, requirement_id, revision_id, markdown_version_ref, content_hash, \
          human_principal_id, identity_id, action, source, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'confirm', 'rest', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(requirement_id)
    .bind(revision_id)
    .bind(&markdown_version_ref)
    .bind(&content_hash)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .execute(&mut *tx)
    .await?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.confirmed",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;

    fetch_entry(pool, tenant_id, requirement_id).await
}

/// 拒绝修订（RejectRequirementRevision）：pending → rejected（终态，I1-D03
/// 不复用；拒绝人/时间/理由入 Confirmation，§5.5）。current_revision_id 不动。
pub async fn reject_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    revision_id: Uuid,
    req: RejectRevisionRequest,
    actor: &AuditActor,
) -> Result<RevisionView, PmWriteError> {
    require_human(pool, actor).await?;

    let mut tx = pool.begin().await?;
    let (project_id, lifecycle, _, _) =
        lock_requirement(&mut tx, tenant_id, requirement_id).await?;
    ensure_active(&lifecycle)?;
    // I6-A：拒绝与确认同权（Lead OR 显式 Approver）。
    crate::approver::ensure_governance_authorized(pool, project_id, actor.principal_id).await?;

    let (_, state, content_hash, markdown_version_ref) =
        lock_revision(&mut tx, tenant_id, requirement_id, revision_id).await?;
    if state != "pending_confirmation" {
        return Err(PmWriteError::Conflict(
            "revision is not pending confirmation",
        ));
    }

    sqlx::query("UPDATE delivery.requirement_revision SET state = 'rejected' WHERE id = $1")
        .bind(revision_id)
        .execute(&mut *tx)
        .await?;

    let reason = req
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    sqlx::query(
        "INSERT INTO delivery.requirement_confirmation \
         (project_id, requirement_id, revision_id, markdown_version_ref, content_hash, \
          human_principal_id, identity_id, action, reason, source, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'reject', $8, 'rest', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(requirement_id)
    .bind(revision_id)
    .bind(&markdown_version_ref)
    .bind(&content_hash)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(reason)
    .execute(&mut *tx)
    .await?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.revision_rejected",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;

    sqlx::query_as::<_, RevisionView>(&format!(
        "SELECT {REVISION_VIEW_COLS} {REVISION_VIEW_FROM} WHERE rv.id = $1"
    ))
    .bind(revision_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 需求级生命周期治理（§5.2）：Withdraw / Restore / Archive。V1 = Lead
/// （`project_member.role='owner'`）；显式 Approver 载体归 I6-A（O-6）。
pub async fn transition_lifecycle(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    action: LifecycleAction,
    actor: &AuditActor,
) -> Result<RequirementEntry, PmWriteError> {
    require_human(pool, actor).await?;

    let mut tx = pool.begin().await?;
    let (project_id, lifecycle, _, _) =
        lock_requirement(&mut tx, tenant_id, requirement_id).await?;

    // I6-A：生命周期治理 = Lead OR 显式 Approver。
    crate::approver::ensure_governance_authorized(pool, project_id, actor.principal_id).await?;

    let next = match (action, lifecycle.as_str()) {
        (LifecycleAction::Withdraw, "active") => "withdrawn",
        (LifecycleAction::Restore, "withdrawn") => "active",
        (LifecycleAction::Archive, "active" | "withdrawn") => "archived",
        (LifecycleAction::Archive, "archived") => {
            return Err(PmWriteError::Conflict("requirement is already archived"));
        }
        // 其余组合（含 I1-D05 archived 恢复）一律拒绝。
        _ => {
            return Err(PmWriteError::Conflict(
                "invalid lifecycle transition for this requirement",
            ))
        }
    };

    sqlx::query(
        "UPDATE delivery.requirement SET lifecycle_status = $2, \
                updated_at = now(), updated_by = $3 WHERE id = $1",
    )
    .bind(requirement_id)
    .bind(next)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        action.audit_action(),
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;

    fetch_entry(pool, tenant_id, requirement_id).await
}

// ─────────────────────────────────────────────────────────────── 读侧

/// 项目需求列表（§8.5 集合视图的数据源：lifecycle_status='active' 且
/// current_revision_id 非空的行即正式基线集合元素——读侧不过滤，由消费方判）。
pub async fn list_requirements(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<RequirementEntry>, PmWriteError> {
    sqlx::query_as(
        "SELECT r.id, r.project_id, r.code, r.title, r.requirement_type, r.priority, \
                r.lifecycle_status, r.created_at, r.updated_at, r.row_revision, \
                cur.id   AS current_revision_id, cur.version AS current_version, \
                cur.content_hash AS current_content_hash, \
                lat.id   AS latest_revision_id, lat.version AS latest_version, \
                lat.state AS latest_state, lat.submitted_at AS latest_submitted_at \
         FROM delivery.requirement r \
         LEFT JOIN delivery.requirement_revision cur ON cur.id = r.current_revision_id \
         LEFT JOIN LATERAL ( \
             SELECT v.id, v.version, v.state, v.submitted_at \
             FROM delivery.requirement_revision v \
             WHERE v.requirement_id = r.id ORDER BY v.version DESC LIMIT 1 \
         ) lat ON TRUE \
         WHERE r.tenant_id = $1 AND r.project_id = $2 \
         ORDER BY r.code",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 需求详情：条目 + 修订链（版本倒序）+ 确认记录（时间倒序）。
pub async fn requirement_detail(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
) -> Result<RequirementDetail, PmWriteError> {
    let requirement = fetch_entry(pool, tenant_id, requirement_id).await?;
    let revisions: Vec<RevisionView> = sqlx::query_as(&format!(
        "SELECT {REVISION_VIEW_COLS} {REVISION_VIEW_FROM} \
         WHERE rv.requirement_id = $1 ORDER BY rv.version DESC"
    ))
    .bind(requirement_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let confirmations: Vec<ConfirmationView> = sqlx::query_as(
        "SELECT rc.id, rc.revision_id, rc.action, rc.reason, rc.human_principal_id, \
                hp.display_name AS human_display_name, rc.identity_id, rc.source, \
                rc.content_hash, rc.markdown_version_ref, rc.confirmed_at \
         FROM delivery.requirement_confirmation rc \
         LEFT JOIN core.principal hp ON hp.id = rc.human_principal_id \
         WHERE rc.tenant_id = $1 AND rc.requirement_id = $2 \
         ORDER BY rc.confirmed_at DESC",
    )
    .bind(tenant_id)
    .bind(requirement_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    Ok(RequirementDetail {
        requirement,
        revisions,
        confirmations,
    })
}
