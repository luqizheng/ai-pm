//! S03 · 设计与技术决策统一服务（总体设计 V1.0 §3/§4/§5/§6 落地）。
//!
//! 九个能力的**单点实现**（S03-00 §16.1「写操作单入口」教训）：Technical
//! Design / Design Revision / Design↔Requirement 关联 / ADR / 评审问题 /
//! 专表审批 / 实施基线 / Readiness Gate 全部在本模块收敛；REST 适配层
//! （pm-api `pm_designs`）零业务裁决，MCP 不提供任何确认类工具（A* ②）。
//!
//! 治理边界（Q4/原则 5）：
//! - AI 主体可以创建设计/修订/ADR 草稿、提交评审、请求批准（`source='ai'`
//!   留痕）；
//! - 批准 / 接受 / 冻结一律 `require_human`（真人主体）+ REST 适配层
//!   `ensure_trusted_context`（api_key 拒绝）双闸，AI 无任何批准路径；
//! - 审批走专表 `design.design_approval`（R6：不复用 1:1 硬绑
//!   change_proposal 的 approval_request），`target_hash` 锚定审批人见到的
//!   内容哈希（跨版本重放防线，S02D-16 同款）。
//!
//! 并发（SC-06）：design/adr 行乐观锁 `row_revision`；批准命令可携带
//! `expected_current_revision_id` / `expected_row_revision`，后至者显式 409。
//!
//! 不可变（SC-02/SC-08）：revision 正文库级触发器冻结（draft 改稿除外）；
//! 基线冻结后仅 status 可显式变更；任务引用基线 id 而非内容。

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use pm_domain::design::{AdrRevisionState, AdrStatus, DesignRevisionState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::{
    append_audit, append_audit_with_after, content_hash_of, is_unique_violation, lock_head_row,
    lock_revision_row, next_code, transition_or_conflict, PmWriteError,
};
use crate::requirement::require_human_conn;

/// 任务级同款验收验证方式（复用 pm_write 词表）。
pub const VERIFICATION_KINDS: [&str; 4] = ["manual", "automated_test", "artifact_exists", "review"];

/// 审批裁决动作词表（decide 端点与 task_plan 复用）。
pub const DECIDE_APPROVAL_ACTIONS: [&str; 2] = ["approve", "reject"];

const REVISION_VIEW_COLS: &str = "rv.id, rv.design_id, rv.version, rv.state, rv.content, \
     rv.content_hash, rv.source, rv.analysis_run_id, rv.base_current_revision_id, \
     rv.created_by, cb.display_name AS created_by_name, rv.submitted_at, \
     rv.approved_by, ab.display_name AS approved_by_name, rv.approved_at, rv.created_at";
const REVISION_VIEW_FROM: &str = "FROM design.design_revision rv \
     LEFT JOIN core.principal cb ON cb.id = rv.created_by \
     LEFT JOIN core.principal ab ON ab.id = rv.approved_by";

const DESIGN_BY_ID: &str = "SELECT d.id, d.project_id, d.code, d.title, d.lifecycle_status, \
     d.row_revision, d.current_revision_id, cur.version AS current_version, \
     cur.state AS current_state, lat.version AS latest_version, lat.state AS latest_state, \
     d.created_at, d.updated_at \
     FROM design.design d \
     LEFT JOIN design.design_revision cur ON cur.id = d.current_revision_id \
     LEFT JOIN LATERAL ( \
         SELECT v.version, v.state FROM design.design_revision v \
         WHERE v.design_id = d.id ORDER BY v.version DESC LIMIT 1 \
     ) lat ON TRUE";

// ───────────────────────────────────────────────────────────────────── 视图

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct DesignEntry {
    pub id: Uuid,
    pub project_id: Uuid,
    pub code: String,
    pub title: String,
    pub lifecycle_status: String,
    pub row_revision: i64,
    pub current_revision_id: Option<Uuid>,
    pub current_version: Option<i32>,
    pub current_state: Option<String>,
    pub latest_version: Option<i32>,
    pub latest_state: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct DesignRevisionView {
    pub id: Uuid,
    pub design_id: Uuid,
    pub version: i32,
    pub state: String,
    pub content: Value,
    pub content_hash: String,
    pub source: String,
    pub analysis_run_id: Option<Uuid>,
    pub base_current_revision_id: Option<Uuid>,
    pub created_by: Uuid,
    pub created_by_name: Option<String>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub approved_by: Option<Uuid>,
    pub approved_by_name: Option<String>,
    pub approved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct DesignRequirementLink {
    pub design_revision_id: Uuid,
    pub requirement_id: Uuid,
    pub requirement_revision_id: Uuid,
    pub requirement_code: Option<String>,
    pub requirement_version: Option<i32>,
    pub requirement_revision_state: Option<String>,
    pub is_current: Option<bool>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct ReviewFindingView {
    pub id: Uuid,
    pub design_revision_id: Uuid,
    pub finding_kind: String,
    pub severity: String,
    pub body: String,
    pub status: String,
    pub resolution_note: Option<String>,
    pub raised_by: Uuid,
    pub resolved_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct ApprovalView {
    pub id: Uuid,
    pub kind: String,
    pub design_id: Option<Uuid>,
    pub adr_id: Option<Uuid>,
    pub baseline_id: Option<Uuid>,
    pub target_id: Uuid,
    pub target_hash: String,
    pub status: String,
    pub requested_by: Uuid,
    pub decided_by: Option<Uuid>,
    pub decision_reason: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct AdrEntry {
    pub id: Uuid,
    pub project_id: Uuid,
    pub code: String,
    pub title: String,
    pub status: String,
    pub blockers_implementation: bool,
    pub current_revision_id: Option<Uuid>,
    pub current_version: Option<i32>,
    pub superseded_by_adr_id: Option<Uuid>,
    pub row_revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct AdrRevisionView {
    pub id: Uuid,
    pub adr_id: Uuid,
    pub version: i32,
    pub content: Value,
    pub state: String,
    pub content_hash: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct BaselineEntry {
    pub id: Uuid,
    pub project_id: Uuid,
    pub code: String,
    pub status: String,
    pub content: Value,
    pub content_hash: String,
    pub readiness: Value,
    pub frozen_by: Uuid,
    pub frozen_at: DateTime<Utc>,
    pub superseded_by_baseline_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// 审批收件箱行（项目级）：审批行 + 目标展示标识 + 发起/裁决人显示名。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct ApprovalInboxRow {
    pub id: Uuid,
    pub kind: String,
    pub design_id: Option<Uuid>,
    pub adr_id: Option<Uuid>,
    pub baseline_id: Option<Uuid>,
    pub target_id: Uuid,
    pub target_hash: String,
    pub status: String,
    pub requested_by: Uuid,
    pub requested_by_name: Option<String>,
    pub decided_by: Option<Uuid>,
    pub decided_by_name: Option<String>,
    pub decision_reason: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    /// 目标展示码（设计 code / ADR code / 基线 code）。
    pub target_code: Option<String>,
    /// 目标展示标题。
    pub target_title: Option<String>,
}

/// S03 工作空间项目级聚合（S03 四页共用上下文）。
#[derive(Debug, Serialize)]
pub struct DesignWorkspace {
    pub design_count: i64,
    pub adr_count: i64,
    pub baseline_count: i64,
    pub pending_approval_count: i64,
    pub open_finding_count: i64,
    pub linked_requirement_count: i64,
    pub stale_requirement_count: i64,
    /// 设计按当前修订状态计数（draft / pending_confirmation / approved …）。
    pub design_states: BTreeMap<String, i64>,
    /// ADR 按生命周期状态计数（proposed / under_review / accepted …）。
    pub adr_states: BTreeMap<String, i64>,
}

/// 基线 → 任务交接行。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct BaselineTaskRow {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub created_at: DateTime<Utc>,
}

/// 分析页：设计的需求覆盖行（绑定到具体需求修订）。
#[derive(Debug, sqlx::FromRow, Clone, Serialize)]
pub struct AnalysisRequirementLink {
    pub design_id: Uuid,
    pub design_revision_id: Uuid,
    pub requirement_id: Uuid,
    pub requirement_revision_id: Uuid,
    pub requirement_code: Option<String>,
    pub requirement_title: Option<String>,
    pub requirement_version: Option<i32>,
    pub requirement_state: Option<String>,
    /// 绑定的是否为该需求当前正式版本；false = 已被新版本取代（需影响分析）。
    pub is_current: Option<bool>,
}

/// 分析页：影响关系行（本设计被哪些 ADR 引用）。
#[derive(Debug, sqlx::FromRow, Clone, Serialize)]
pub struct AnalysisAdrLink {
    pub design_id: Uuid,
    pub adr_revision_id: Uuid,
    pub adr_id: Uuid,
    pub adr_code: Option<String>,
    pub adr_title: Option<String>,
    pub adr_status: Option<String>,
    pub adr_version: Option<i32>,
}

/// 分析页：单个设计的分析行（需求覆盖 + 影响关系 + 未闭问题数）。
#[derive(Debug, Serialize)]
pub struct DesignAnalysisRow {
    pub design_id: Uuid,
    pub code: String,
    pub title: String,
    pub lifecycle_status: String,
    pub current_revision_id: Option<Uuid>,
    pub current_version: Option<i32>,
    pub current_state: Option<String>,
    pub requirement_count: i64,
    /// 引用了已被取代的需求修订的绑定数（影响分析的重点）。
    pub stale_requirement_count: i64,
    pub open_finding_count: i64,
    pub requirements: Vec<AnalysisRequirementLink>,
    pub adrs: Vec<AnalysisAdrLink>,
}

// ───────────────────────────────────────────────────────────────────── 请求

#[derive(Debug, Deserialize)]
pub struct CreateDesignRequest {
    /// 缺省自动生成 `TD-001` 形态编号。
    #[serde(default)]
    pub code: Option<String>,
    pub title: String,
    /// 首版草稿正文（缺省 = `# {title}`）。
    #[serde(default)]
    pub markdown: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    /// 技术分析（§3.1；随修订承载，D-01 不设独立实体）。
    #[serde(default)]
    pub technical_analysis: Option<Value>,
    #[serde(default)]
    pub implementation_scope: Option<Value>,
    #[serde(default)]
    pub acceptance_criteria: Option<Vec<AcceptanceCriterionInput>>,
}

#[derive(Debug, Deserialize)]
pub struct DraftDesignRevisionRequest {
    pub markdown: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub technical_analysis: Option<Value>,
    #[serde(default)]
    pub implementation_scope: Option<Value>,
    #[serde(default)]
    pub acceptance_criteria: Option<Vec<AcceptanceCriterionInput>>,
}

#[derive(Debug, Deserialize)]
pub struct LinkDesignRequirementRequest {
    pub requirement_id: Uuid,
    /// 缺省 = 该需求当前正式版本（必须已批准，§2.2）。
    #[serde(default)]
    pub requirement_revision_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct AddFindingRequest {
    #[serde(default)]
    pub finding_kind: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct ResolveFindingRequest {
    /// `acknowledged`（已知悉）或 `resolved`（已解决）。
    pub status: String,
    #[serde(default)]
    pub resolution_note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DecideApprovalRequest {
    /// `approve` | `reject`。
    pub decision: String,
    #[serde(default)]
    pub reason: Option<String>,
    /// SC-06 并发防护：审批人见到并核准时的设计当前版本/行版本。
    #[serde(default)]
    pub expected_current_revision_id: Option<Uuid>,
    #[serde(default)]
    pub expected_row_revision: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAdrRequest {
    #[serde(default)]
    pub code: Option<String>,
    pub title: String,
    pub decision_question: String,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub options: Vec<Value>,
    #[serde(default)]
    pub chosen_option: Option<String>,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub consequences: Option<String>,
    /// 是否实施阻塞项（§6.2：阻塞项必须 accepted 才能过 Readiness Gate）。
    #[serde(default)]
    pub blockers_implementation: bool,
}

#[derive(Debug, Deserialize)]
pub struct DraftAdrRevisionRequest {
    pub decision_question: String,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub options: Vec<Value>,
    #[serde(default)]
    pub chosen_option: Option<String>,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub consequences: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LinkAdrDesignRequest {
    pub design_id: Uuid,
    pub design_revision_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct LinkAdrRequirementRequest {
    pub requirement_id: Uuid,
    #[serde(default)]
    pub requirement_revision_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct SupersedeAdrRequest {
    pub superseded_by_adr_id: Uuid,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetBlockingRequest {
    pub blockers_implementation: bool,
}

#[derive(Debug, Deserialize)]
pub struct RepositoryRefInput {
    pub repository_id: Uuid,
    #[serde(default)]
    pub base_commit: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FreezeBaselineRequest {
    #[serde(default)]
    pub code: Option<String>,
    /// 本次实施依据的设计版本（§6.3：Requirements/Designs/ADRs 全部按
    /// revision 指针冻结）。
    pub design_revision_ids: Vec<Uuid>,
    #[serde(default)]
    pub repository_refs: Vec<RepositoryRefInput>,
}

#[derive(Debug, Deserialize)]
pub struct SupersedeBaselineRequest {
    /// 后继基线（可选：退位并登记替代链；缺省=仅退位）。
    #[serde(default)]
    pub successor_baseline_id: Option<Uuid>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AcceptanceCriterionInput {
    pub statement: String,
    /// 缺省 `manual`。
    #[serde(default)]
    pub verification_kind: Option<String>,
}

// ─────────────────────────────────────────────────────────────── 内部助手

pub async fn principal_is_human(
    conn: &mut sqlx::PgConnection,
    principal_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let kind: Option<String> = sqlx::query_scalar("SELECT kind FROM core.principal WHERE id = $1")
        .bind(principal_id)
        .fetch_optional(&mut *conn)
        .await?;
    Ok(kind.as_deref() == Some("human"))
}

/// 加锁读设计身份行：(project_id, lifecycle_status, current_revision_id, row_revision)。
/// （通用实现见 `pm_write::lock_head_row`。）
async fn lock_design(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    design_id: Uuid,
) -> Result<(Uuid, String, Option<Uuid>, i64), PmWriteError> {
    lock_head_row(
        conn,
        tenant_id,
        design_id,
        "design.design",
        "lifecycle_status",
        "design not found",
    )
    .await
}

async fn fetch_revision_view(
    db: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
) -> Result<DesignRevisionView, PmWriteError> {
    sqlx::query_as::<_, DesignRevisionView>(&format!(
        "SELECT {REVISION_VIEW_COLS} {REVISION_VIEW_FROM} \
         WHERE rv.tenant_id = $1 AND rv.design_id = $2 AND rv.id = $3"
    ))
    .bind(tenant_id)
    .bind(design_id)
    .bind(revision_id)
    .fetch_optional(db)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("design revision not found"))
}

/// 加锁读修订行：(version, state, content_hash, content)。
/// （通用实现见 `pm_write::lock_revision_row`。）
async fn lock_design_revision(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
) -> Result<(i32, String, String, Value), PmWriteError> {
    lock_revision_row(
        conn,
        tenant_id,
        revision_id,
        design_id,
        "design.design_revision",
        "design_id",
        "version, state, content_hash, content",
        "design revision not found",
    )
    .await
}

/// 草稿内容的 JSON 组装（单一定义点：draft 语义全模块一致）。
fn build_revision_content(req: &DraftDesignRevisionRequest) -> Value {
    let mut content = json!({ "markdown": req.markdown });
    let obj = content.as_object_mut().expect("object");
    if let Some(s) = req.summary.as_deref().filter(|s| !s.trim().is_empty()) {
        obj.insert("summary".into(), json!(s));
    }
    if let Some(a) = req.technical_analysis.clone().filter(|v| !v.is_null()) {
        obj.insert("technical_analysis".into(), a);
    }
    if let Some(s) = req.implementation_scope.clone().filter(|v| !v.is_null()) {
        obj.insert("implementation_scope".into(), s);
    }
    if let Some(c) = req.acceptance_criteria.as_ref().filter(|v| !v.is_empty()) {
        obj.insert(
            "acceptance_criteria".into(),
            serde_json::to_value(c).expect("serialize criteria"),
        );
    }
    content
}

fn parse_state(state: &str) -> Result<DesignRevisionState, PmWriteError> {
    DesignRevisionState::parse(state)
        .ok_or_else(|| PmWriteError::Validation("unknown design revision state"))
}

async fn lock_adr(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    adr_id: Uuid,
) -> Result<(Uuid, String, Option<Uuid>, i64), PmWriteError> {
    sqlx::query_as(
        "SELECT project_id, status, current_revision_id, row_revision \
         FROM design.adr WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("adr not found"))
}

pub mod adr;
pub mod approval;
pub mod baseline;
pub mod finding;
pub mod readiness;
pub mod technical;

pub use adr::*;
pub use approval::*;
pub use baseline::*;
pub use finding::*;
pub use readiness::*;
pub use technical::*;
