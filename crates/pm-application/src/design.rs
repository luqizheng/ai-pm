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

// ─────────────────────────────────────────────────────────── Readiness Gate

/// 门禁五条件之一（§6.1）。
#[derive(Debug, Serialize)]
pub struct GateCondition {
    pub condition: &'static str,
    pub passed: bool,
    pub details: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GateReport {
    pub ready: bool,
    pub design_revision_ids: Vec<Uuid>,
    pub conditions: Vec<GateCondition>,
}

/// Readiness Gate 五条件判定（§6.1；freeze 前置 + dry-run 端点共用）。
pub async fn evaluate_readiness(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    design_revision_ids: &[Uuid],
) -> Result<GateReport, PmWriteError> {
    if design_revision_ids.is_empty() {
        return Err(PmWriteError::Validation(
            "design_revision_ids is required for readiness evaluation",
        ));
    }
    let mut conditions = Vec::with_capacity(5);

    // 条件 2：设计已批准（人类正式批准，AI 草稿不合格）。
    let rows: Vec<(Uuid, i32, String, Uuid)> = sqlx::query_as(
        "SELECT rv.id, rv.version, rv.state, rv.design_id \
         FROM design.design_revision rv \
         WHERE rv.tenant_id = $1 AND rv.project_id = $2 AND rv.id = ANY($3) \
         ORDER BY rv.design_id, rv.version",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(design_revision_ids)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let mut c2_details = Vec::new();
    for missing in design_revision_ids
        .iter()
        .filter(|id| !rows.iter().any(|(rid, _, _, _)| rid == *id))
    {
        c2_details.push(format!("revision {missing} not found in this project"));
    }
    for (rid, version, state, _) in &rows {
        if state != DesignRevisionState::Approved.as_str() {
            c2_details.push(format!(
                "revision {rid} (v{version}) is `{state}`, not approved"
            ));
        }
    }
    conditions.push(GateCondition {
        condition: "design_approved",
        passed: c2_details.is_empty(),
        details: c2_details,
    });

    // 条件 1：需求依据已确认（绑定的需求修订必须 approved 且为当前正式版本）。
    let links: Vec<(Uuid, Uuid, Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT dr.design_revision_id, dr.requirement_id, dr.requirement_revision_id, \
                r.current_revision_id \
         FROM design.design_requirement dr \
         LEFT JOIN delivery.requirement r ON r.id = dr.requirement_id \
         WHERE dr.tenant_id = $1 AND dr.project_id = $2 \
           AND dr.design_revision_id = ANY($3)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(design_revision_ids)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let mut c1_details = Vec::new();
    if links.is_empty() {
        c1_details.push("no requirement basis linked to the selected revisions".to_string());
    }
    for (_rid, requirement_id, revision_id, current) in &links {
        let state: Option<String> = sqlx::query_scalar(
            "SELECT state FROM delivery.requirement_revision \
             WHERE tenant_id = $1 AND id = $2 AND requirement_id = $3",
        )
        .bind(tenant_id)
        .bind(revision_id)
        .bind(requirement_id)
        .fetch_optional(pool)
        .await
        .map_err(PmWriteError::Db)?;
        match state.as_deref() {
            None => c1_details.push(format!(
                "requirement revision {revision_id} not found for requirement {requirement_id}"
            )),
            Some(s) if s != "approved" => c1_details.push(format!(
                "linked requirement revision {revision_id} is `{s}`, not approved"
            )),
            _ => {}
        }
        if state.is_some() && current.is_some() && current.as_ref() != Some(revision_id) {
            // §6.2：需求已被新版本取代而设计仍引用旧版本 → 阻止进入 S04。
            c1_details.push(format!(
                "requirement {requirement_id} moved past linked revision {revision_id}; \
                 impact assessment required"
            ));
        }
    }
    conditions.push(GateCondition {
        condition: "requirements_confirmed",
        passed: c1_details.is_empty(),
        details: c1_details,
    });

    // 条件 3：必要 ADR 已解决（阻塞项必须 accepted；经设计与需求双链收集）。
    let blocking: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT DISTINCT a.id, a.code, a.status \
         FROM design.adr a \
         WHERE a.tenant_id = $1 AND a.project_id = $2 AND a.blockers_implementation \
           AND a.id IN ( \
               SELECT adr_id FROM design.adr_design_revision \
                WHERE tenant_id = $1 AND project_id = $2 \
                  AND design_revision_id = ANY($3) \
               UNION \
               SELECT adr_id FROM design.adr_requirement \
                WHERE tenant_id = $1 AND project_id = $2 \
                  AND requirement_revision_id IN ( \
                      SELECT requirement_revision_id FROM design.design_requirement \
                       WHERE tenant_id = $1 AND project_id = $2 \
                         AND design_revision_id = ANY($3)))",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(design_revision_ids)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let c3_details: Vec<String> = blocking
        .iter()
        .filter(|(_, _, status)| {
            AdrStatus::parse(status)
                .map(|s| !s.resolved_for_implementation())
                .unwrap_or(true)
        })
        .map(|(id, code, status)| format!("blocking ADR {code} ({id}) is `{status}`"))
        .collect();
    conditions.push(GateCondition {
        condition: "adrs_resolved",
        passed: c3_details.is_empty(),
        details: c3_details,
    });

    // 条件 4/5：实施范围与验收标准齐备（按 revision content 结构化字段判定）。
    let mut c4_details = Vec::new();
    let mut c5_details = Vec::new();
    for (rid, _, _, _) in &rows {
        let content: Option<Value> = sqlx::query_scalar(
            "SELECT content FROM design.design_revision \
             WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(rid)
        .fetch_optional(pool)
        .await
        .map_err(PmWriteError::Db)?;
        let Some(content) = content else { continue };
        match scope_included_len(&content) {
            Ok(n) if n > 0 => {}
            Ok(_) => c4_details.push(format!("revision {rid} has no implementation scope")),
            Err(e) => c4_details.push(format!("revision {rid}: {e}")),
        }
        match acceptance_ready(&content) {
            Ok(()) => {}
            Err(e) => c5_details.push(format!("revision {rid}: {e}")),
        }
    }
    conditions.push(GateCondition {
        condition: "scope_defined",
        passed: c4_details.is_empty(),
        details: c4_details,
    });
    conditions.push(GateCondition {
        condition: "acceptance_ready",
        passed: c5_details.is_empty(),
        details: c5_details,
    });

    let ready = conditions.iter().all(|c| c.passed);
    Ok(GateReport {
        ready,
        design_revision_ids: design_revision_ids.to_vec(),
        conditions,
    })
}

fn scope_included_len(content: &Value) -> Result<usize, String> {
    let scope = content
        .get("implementation_scope")
        .ok_or_else(|| "content.implementation_scope missing".to_string())?;
    let included = scope
        .get("included")
        .ok_or_else(|| "implementation_scope.included missing".to_string())?;
    included
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|v| v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false))
                .count()
        })
        .ok_or_else(|| "implementation_scope.included must be an array".to_string())
}

fn acceptance_ready(content: &Value) -> Result<(), String> {
    let criteria = content
        .get("acceptance_criteria")
        .ok_or_else(|| "content.acceptance_criteria missing".to_string())?;
    let items = criteria
        .as_array()
        .ok_or_else(|| "acceptance_criteria must be an array".to_string())?;
    if items.is_empty() {
        return Err("acceptance_criteria is empty".to_string());
    }
    for (i, item) in items.iter().enumerate() {
        let statement = item
            .get("statement")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if statement.is_empty() {
            return Err(format!("acceptance_criteria[{i}] statement is empty"));
        }
        let kind = item
            .get("verification_kind")
            .and_then(Value::as_str)
            .unwrap_or("manual");
        if !VERIFICATION_KINDS.contains(&kind) {
            return Err(format!(
                "acceptance_criteria[{i}] verification_kind `{kind}` is invalid"
            ));
        }
    }
    Ok(())
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

// ────────────────────────────────────────────────────── Technical Design

pub async fn list_designs(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<DesignEntry>, PmWriteError> {
    sqlx::query_as(&format!(
        "{DESIGN_BY_ID} WHERE d.tenant_id = $1 AND d.project_id = $2 ORDER BY d.created_at DESC"
    ))
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

#[derive(Debug, Serialize)]
pub struct DesignDetail {
    pub design: DesignEntry,
    pub revisions: Vec<DesignRevisionView>,
    pub approvals: Vec<ApprovalView>,
}

pub async fn design_detail(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
) -> Result<DesignDetail, PmWriteError> {
    let design: DesignEntry = sqlx::query_as(&format!(
        "{DESIGN_BY_ID} WHERE d.tenant_id = $1 AND d.id = $2"
    ))
    .bind(tenant_id)
    .bind(design_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("design not found"))?;
    let revisions: Vec<DesignRevisionView> = sqlx::query_as(&format!(
        "SELECT {REVISION_VIEW_COLS} {REVISION_VIEW_FROM} \
         WHERE rv.tenant_id = $1 AND rv.design_id = $2 ORDER BY rv.version DESC"
    ))
    .bind(tenant_id)
    .bind(design_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let approvals: Vec<ApprovalView> = sqlx::query_as(
        "SELECT id, kind, design_id, adr_id, baseline_id, target_id, target_hash, status, \
                requested_by, decided_by, decision_reason, decided_at, created_at \
         FROM design.design_approval \
         WHERE tenant_id = $1 AND design_id = $2 ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .bind(design_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    Ok(DesignDetail {
        design,
        revisions,
        approvals,
    })
}

pub async fn create_design(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    req: CreateDesignRequest,
    actor: &AuditActor,
) -> Result<DesignDetail, PmWriteError> {
    let title = req.title.trim().to_string();
    if title.is_empty() {
        return Err(PmWriteError::Validation("title is required"));
    }
    let markdown = req
        .markdown
        .clone()
        .unwrap_or_else(|| format!("# {title}\n"));
    let draft = DraftDesignRevisionRequest {
        markdown,
        summary: req.summary.clone(),
        technical_analysis: req.technical_analysis.clone(),
        implementation_scope: req.implementation_scope.clone(),
        acceptance_criteria: req.acceptance_criteria.clone(),
    };
    let content = build_revision_content(&draft);
    let content_hash = content_hash_of(&content);

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let source = if principal_is_human(&mut tx, actor.principal_id).await? {
        "human"
    } else {
        "ai"
    };
    let code = match req.code.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(c) => c.to_string(),
        None => next_code(&mut tx, tenant_id, project_id, "design.design", "TD").await?,
    };
    let design_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.design (id, tenant_id, project_id, code, title, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(design_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(&code)
    .bind(&title)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            PmWriteError::Conflict("design code already exists in this project")
        } else {
            PmWriteError::Db(e)
        }
    })?;
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.design_revision \
         (id, tenant_id, project_id, design_id, version, content, state, content_hash, \
          source, created_by) \
         VALUES ($1, $2, $3, $4, 1, $5, 'draft', $6, $7, $8)",
    )
    .bind(revision_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(design_id)
    .bind(&content)
    .bind(&content_hash)
    .bind(source)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "design.created",
        "design",
        Some(design_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    design_detail(pool, tenant_id, design_id).await
}

/// 新修订草稿（版本 = max+1；基线指针 = 创建时的当前批准版本，§4.1）。
pub async fn create_design_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    req: DraftDesignRevisionRequest,
    actor: &AuditActor,
) -> Result<DesignRevisionView, PmWriteError> {
    if req.markdown.trim().is_empty() {
        return Err(PmWriteError::Validation("markdown is required"));
    }
    let content = build_revision_content(&req);
    let content_hash = content_hash_of(&content);

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, lifecycle, current, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    if lifecycle != "active" {
        return Err(PmWriteError::Conflict("design is not active"));
    }
    let source = if principal_is_human(&mut tx, actor.principal_id).await? {
        "human"
    } else {
        "ai"
    };
    let version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM design.design_revision \
         WHERE design_id = $1 AND tenant_id = $2",
    )
    .bind(design_id)
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.design_revision \
         (id, tenant_id, project_id, design_id, version, content, state, content_hash, \
          source, base_current_revision_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, 'draft', $7, $8, $9, $10)",
    )
    .bind(revision_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(design_id)
    .bind(version)
    .bind(&content)
    .bind(&content_hash)
    .bind(source)
    .bind(current)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "design_revision.created",
        "design_revision",
        Some(revision_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_revision_view(pool, tenant_id, design_id, revision_id).await
}

/// 改稿（仅 draft；正文由库级触发器双重保障，§4.2）。
pub async fn update_design_revision_draft(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
    req: DraftDesignRevisionRequest,
    actor: &AuditActor,
) -> Result<DesignRevisionView, PmWriteError> {
    if req.markdown.trim().is_empty() {
        return Err(PmWriteError::Validation("markdown is required"));
    }
    let content = build_revision_content(&req);
    let content_hash = content_hash_of(&content);

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    let (version, state, _, _) =
        lock_design_revision(&mut tx, tenant_id, design_id, revision_id).await?;
    let from = parse_state(&state)?;
    if !from.content_editable() {
        return Err(PmWriteError::Conflict("only draft revisions can be edited"));
    }
    sqlx::query(
        "UPDATE design.design_revision SET content = $1, content_hash = $2 \
         WHERE id = $3 AND design_id = $4 AND tenant_id = $5 AND state = 'draft'",
    )
    .bind(&content)
    .bind(&content_hash)
    .bind(revision_id)
    .bind(design_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "design_revision.updated",
        "design_revision",
        Some(revision_id),
        json!({ "version": version, "content_hash": content_hash }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_revision_view(pool, tenant_id, design_id, revision_id).await
}

/// 提交评审：draft → pending_confirmation（正文自此冻结；AI/人类皆可提交）。
pub async fn submit_design_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
    actor: &AuditActor,
) -> Result<DesignRevisionView, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    let (version, state, content_hash, content) =
        lock_design_revision(&mut tx, tenant_id, design_id, revision_id).await?;
    let from = parse_state(&state)?;
    transition_or_conflict(
        |f, t| f.can_transition_to(t),
        from,
        DesignRevisionState::PendingConfirmation,
        "illegal design revision transition",
    )?;
    let markdown = content
        .get("markdown")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if markdown.is_empty() {
        return Err(PmWriteError::Validation(
            "markdown is required before submit",
        ));
    }
    sqlx::query(
        "UPDATE design.design_revision \
         SET state = 'pending_confirmation', submitted_at = now() \
         WHERE id = $1 AND design_id = $2 AND tenant_id = $3 AND state = 'draft'",
    )
    .bind(revision_id)
    .bind(design_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "design_revision.submitted",
        "design_revision",
        Some(revision_id),
        json!({ "version": version, "content_hash": content_hash }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_revision_view(pool, tenant_id, design_id, revision_id).await
}

// ─────────────────────────────────────────── Design ↔ Requirement（Q1 多对多）

pub async fn list_design_requirements(
    pool: &PgPool,
    tenant_id: Uuid,
    _design_id: Uuid,
    revision_id: Uuid,
) -> Result<Vec<DesignRequirementLink>, PmWriteError> {
    sqlx::query_as(
        "SELECT dr.design_revision_id, dr.requirement_id, dr.requirement_revision_id, \
                r.code AS requirement_code, rv.version AS requirement_version, \
                rv.state AS requirement_revision_state, \
                (r.current_revision_id = dr.requirement_revision_id) AS is_current, \
                dr.created_at \
         FROM design.design_requirement dr \
         LEFT JOIN delivery.requirement r ON r.id = dr.requirement_id \
         LEFT JOIN delivery.requirement_revision rv ON rv.id = dr.requirement_revision_id \
         WHERE dr.tenant_id = $1 AND dr.design_revision_id = $2 \
         ORDER BY dr.created_at",
    )
    .bind(tenant_id)
    .bind(revision_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 绑定需求修订（§2.2：必须绑定具体且已批准的需求修订；仅 draft 可改关联）。
pub async fn link_design_requirement(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
    req: LinkDesignRequirementRequest,
    actor: &AuditActor,
) -> Result<DesignRequirementLink, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    let (_, state, _, _) = lock_design_revision(&mut tx, tenant_id, design_id, revision_id).await?;
    if !parse_state(&state)?.content_editable() {
        return Err(PmWriteError::Conflict(
            "requirement links are only editable on draft revisions",
        ));
    }
    // 需求必须属于本项目 + 本租户（跨项目越权拦截）。
    let (req_current,): (Option<Uuid>,) = sqlx::query_as(
        "SELECT current_revision_id FROM delivery.requirement \
         WHERE id = $1 AND tenant_id = $2 AND project_id = $3",
    )
    .bind(req.requirement_id)
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("requirement not found"))?;
    let revision_id_target =
        req.requirement_revision_id
            .unwrap_or(req_current.ok_or(PmWriteError::Validation(
                "requirement has no current revision to link",
            ))?);
    let (rev_state,): (String,) = sqlx::query_as(
        "SELECT state FROM delivery.requirement_revision \
         WHERE id = $1 AND requirement_id = $2 AND tenant_id = $3",
    )
    .bind(revision_id_target)
    .bind(req.requirement_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::Validation("requirement revision not found"))?;
    if rev_state != "approved" {
        // §2.2：设计只引用已经正式确认的需求修订。
        return Err(PmWriteError::Validation(
            "only approved requirement revisions can be linked",
        ));
    }
    sqlx::query(
        "INSERT INTO design.design_requirement \
         (design_revision_id, design_id, requirement_id, requirement_revision_id, \
          tenant_id, project_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (design_revision_id, requirement_id) DO NOTHING",
    )
    .bind(revision_id)
    .bind(design_id)
    .bind(req.requirement_id)
    .bind(revision_id_target)
    .bind(tenant_id)
    .bind(project_id)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "design_requirement.linked",
        "design_revision",
        Some(revision_id),
        json!({
            "requirement_id": req.requirement_id,
            "requirement_revision_id": revision_id_target,
        }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    let links = list_design_requirements(pool, tenant_id, design_id, revision_id).await?;
    links
        .into_iter()
        .find(|l| l.requirement_id == req.requirement_id)
        .ok_or(PmWriteError::Db(sqlx::Error::RowNotFound))
}

/// 解绑（仅 draft；关联变化全部留审计，§3.4 关键约束三）。
pub async fn unlink_design_requirement(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
    requirement_id: Uuid,
    actor: &AuditActor,
) -> Result<(), PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    let (_, state, _, _) = lock_design_revision(&mut tx, tenant_id, design_id, revision_id).await?;
    if !parse_state(&state)?.content_editable() {
        return Err(PmWriteError::Conflict(
            "requirement links are only editable on draft revisions",
        ));
    }
    let result = sqlx::query(
        "DELETE FROM design.design_requirement \
         WHERE tenant_id = $1 AND design_revision_id = $2 AND requirement_id = $3",
    )
    .bind(tenant_id)
    .bind(revision_id)
    .bind(requirement_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if result.rows_affected() == 0 {
        return Err(PmWriteError::NotFound("requirement link not found"));
    }
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "design_requirement.unlinked",
        "design_revision",
        Some(revision_id),
        json!({ "requirement_id": requirement_id }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    Ok(())
}

// ────────────────────────────────────────────────── 设计评审（问题登记/关闭）

pub async fn list_review_findings(
    pool: &PgPool,
    tenant_id: Uuid,
    revision_id: Uuid,
) -> Result<Vec<ReviewFindingView>, PmWriteError> {
    sqlx::query_as(
        "SELECT id, design_revision_id, finding_kind, severity, body, status, \
                resolution_note, raised_by, resolved_by, created_at, resolved_at \
         FROM design.review_finding \
         WHERE tenant_id = $1 AND design_revision_id = $2 \
         ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .bind(revision_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

pub async fn add_review_finding(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
    req: AddFindingRequest,
    actor: &AuditActor,
) -> Result<ReviewFindingView, PmWriteError> {
    let kind = req.finding_kind.as_deref().unwrap_or("issue");
    if !["issue", "question", "suggestion"].contains(&kind) {
        return Err(PmWriteError::Validation("invalid finding_kind"));
    }
    let severity = req.severity.as_deref().unwrap_or("warning");
    if !["info", "warning", "blocker"].contains(&severity) {
        return Err(PmWriteError::Validation("invalid severity"));
    }
    let body = req.body.trim().to_string();
    if body.is_empty() {
        return Err(PmWriteError::Validation("body is required"));
    }
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    let (_, state, _, _) = lock_design_revision(&mut tx, tenant_id, design_id, revision_id).await?;
    let parsed = parse_state(&state)?;
    if !matches!(
        parsed,
        DesignRevisionState::Draft | DesignRevisionState::PendingConfirmation
    ) {
        return Err(PmWriteError::Conflict(
            "findings can only be raised on draft or pending_confirmation revisions",
        ));
    }
    let finding_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.review_finding \
         (id, tenant_id, project_id, design_id, design_revision_id, finding_kind, severity, \
          body, raised_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(finding_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(design_id)
    .bind(revision_id)
    .bind(kind)
    .bind(severity)
    .bind(&body)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "review_finding.raised",
        "review_finding",
        Some(finding_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    sqlx::query_as(
        "SELECT id, design_revision_id, finding_kind, severity, body, status, \
                resolution_note, raised_by, resolved_by, created_at, resolved_at \
         FROM design.review_finding WHERE id = $1 AND tenant_id = $2",
    )
    .bind(finding_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 问题关闭：open → acknowledged → resolved（SC-05 审批前置：blocker 未关不许批准）。
pub async fn resolve_review_finding(
    pool: &PgPool,
    tenant_id: Uuid,
    finding_id: Uuid,
    req: ResolveFindingRequest,
    actor: &AuditActor,
) -> Result<ReviewFindingView, PmWriteError> {
    if !["acknowledged", "resolved"].contains(&req.status.as_str()) {
        return Err(PmWriteError::Validation(
            "status must be acknowledged or resolved",
        ));
    }
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, status, severity): (Uuid, String, String) = sqlx::query_as(
        "SELECT project_id, status, severity FROM design.review_finding \
         WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(finding_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("finding not found"))?;
    let legal = matches!(
        (status.as_str(), req.status.as_str()),
        ("open", "acknowledged") | ("open", "resolved") | ("acknowledged", "resolved")
    );
    if !legal {
        return Err(PmWriteError::Conflict("illegal finding status transition"));
    }
    if req.status == "resolved" && severity == "blocker" && req.resolution_note.is_none() {
        return Err(PmWriteError::Validation(
            "resolving a blocker finding requires a resolution note",
        ));
    }
    sqlx::query(
        "UPDATE design.review_finding \
         SET status = $1, resolution_note = COALESCE($2, resolution_note), \
             resolved_by = $3, resolved_at = CASE WHEN $1 = 'resolved' THEN now() ELSE resolved_at END \
         WHERE id = $4 AND tenant_id = $5",
    )
    .bind(&req.status)
    .bind(req.resolution_note.clone())
    .bind(actor.principal_id)
    .bind(finding_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "review_finding.closed",
        "review_finding",
        Some(finding_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    sqlx::query_as(
        "SELECT id, design_revision_id, finding_kind, severity, body, status, \
                resolution_note, raised_by, resolved_by, created_at, resolved_at \
         FROM design.review_finding WHERE id = $1 AND tenant_id = $2",
    )
    .bind(finding_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

// ───────────────────────────────────────────── 专表审批（批准 / 接受）

/// 请求批准设计修订（AI/人类皆可发起；每目标至多一条 pending）。
pub async fn request_design_revision_approval(
    pool: &PgPool,
    tenant_id: Uuid,
    design_id: Uuid,
    revision_id: Uuid,
    actor: &AuditActor,
) -> Result<ApprovalView, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_design(&mut tx, tenant_id, design_id).await?;
    let (_, state, content_hash, _) =
        lock_design_revision(&mut tx, tenant_id, design_id, revision_id).await?;
    if state != DesignRevisionState::PendingConfirmation.as_str() {
        return Err(PmWriteError::Conflict(
            "only pending_confirmation revisions can be requested for approval",
        ));
    }
    let approval_id = insert_design_approval(
        &mut tx,
        tenant_id,
        project_id,
        "design_revision",
        "design_id",
        design_id,
        revision_id,
        &content_hash,
        actor.principal_id,
        "an approval is already pending for this revision",
    )
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "design.approval.requested",
        "design_approval",
        Some(approval_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_approval_view(pool, tenant_id, approval_id).await
}

/// 请求接受 ADR（AI/人类皆可发起）。
pub async fn request_adr_approval(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    actor: &AuditActor,
) -> Result<ApprovalView, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, status, current_revision_id, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    if status != AdrStatus::UnderReview.as_str() {
        return Err(PmWriteError::Conflict(
            "only under_review adrs can be requested for acceptance",
        ));
    }
    let content_hash: String = sqlx::query_scalar(
        "SELECT content_hash FROM design.adr_revision \
         WHERE id = $1 AND tenant_id = $2 AND adr_id = $3",
    )
    .bind(current_revision_id)
    .bind(tenant_id)
    .bind(adr_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::Conflict(
        "adr has no submitted content revision",
    ))?;
    let approval_id = insert_design_approval(
        &mut tx,
        tenant_id,
        project_id,
        "adr_acceptance",
        "adr_id",
        adr_id,
        adr_id,
        &content_hash,
        actor.principal_id,
        "an approval is already pending for this adr",
    )
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "adr.approval.requested",
        "design_approval",
        Some(approval_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_approval_view(pool, tenant_id, approval_id).await
}

/// 项目级审批收件箱（S03-05 评审与批准；S03 工作空间「待办审批」）。
///
/// 覆盖三种审批 kind（design_revision / adr_acceptance / baseline_freeze），
/// 并带出目标的展示标识（设计 code / ADR code / 基线 code）——审批行本身
/// 只存 target_id，UI 需要一个列表能直接读的标题。
pub async fn list_approvals(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<ApprovalInboxRow>, PmWriteError> {
    sqlx::query_as(
        "SELECT a.id, a.kind, a.design_id, a.adr_id, a.baseline_id, a.target_id, \
                a.target_hash, a.status, a.requested_by, a.decided_by, \
                a.decision_reason, a.decided_at, a.created_at, \
                COALESCE(d.code, ad.code, b.code) AS target_code, \
                COALESCE(d.title, ad.title, b.code) AS target_title, \
                rb.display_name AS requested_by_name, db.display_name AS decided_by_name \
         FROM design.design_approval a \
         LEFT JOIN design.design d ON d.id = a.design_id \
         LEFT JOIN design.adr ad ON ad.id = a.adr_id \
         LEFT JOIN design.implementation_baseline b ON b.id = a.baseline_id \
         LEFT JOIN core.principal rb ON rb.id = a.requested_by \
         LEFT JOIN core.principal db ON db.id = a.decided_by \
         WHERE a.tenant_id = $1 AND a.project_id = $2 \
         ORDER BY (a.status = 'pending') DESC, a.created_at DESC",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// S03 工作空间聚合（S03 四页共用的项目级上下文 + 计数）。
///
/// 一次读出：设计/ADR/基线计数 + 待办审批数 + 待处理评审问题数 + 当前
/// 正式版本与草稿版本清单——S03 四页的页头与空态判据都从这里取，避免
/// 前端为每个计数各打一次请求。
pub async fn design_workspace(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<DesignWorkspace, PmWriteError> {
    let designs = list_designs(pool, tenant_id, project_id).await?;
    let adrs = list_adrs(pool, tenant_id, project_id).await?;
    let baselines = list_baselines(pool, tenant_id, project_id).await?;
    let approvals = list_approvals(pool, tenant_id, project_id).await?;

    let open_findings: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM design.review_finding \
         WHERE tenant_id = $1 AND project_id = $2 AND status = 'open'",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;

    // 需求覆盖：设计 ↔ 需求绑定按「当前正式版本」计数（被新版本取代的绑定
    // 不计入覆盖，避免把过期依据当成已覆盖）。
    let (linked_requirements, stale_requirements): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE r.current_revision_id = dr.requirement_revision_id), \
                count(*) FILTER (WHERE r.current_revision_id IS DISTINCT FROM dr.requirement_revision_id) \
         FROM design.design_requirement dr \
         LEFT JOIN delivery.requirement r ON r.id = dr.requirement_id \
         WHERE dr.tenant_id = $1 AND dr.project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let mut design_states: BTreeMap<String, i64> = BTreeMap::new();
    for design in &designs {
        let state = design.current_state.as_deref().unwrap_or("none");
        *design_states.entry(state.to_string()).or_insert(0) += 1;
    }
    let mut adr_states: BTreeMap<String, i64> = BTreeMap::new();
    for adr in &adrs {
        *adr_states.entry(adr.status.clone()).or_insert(0) += 1;
    }

    Ok(DesignWorkspace {
        design_count: designs.len() as i64,
        adr_count: adrs.len() as i64,
        baseline_count: baselines.len() as i64,
        pending_approval_count: approvals.iter().filter(|a| a.status == "pending").count() as i64,
        open_finding_count: open_findings,
        linked_requirement_count: linked_requirements,
        stale_requirement_count: stale_requirements,
        design_states,
        adr_states,
    })
}

/// 基线 → 任务交接事实（S03-06 交接 S04）。
///
/// 冻结基线时钉定的任务引用（`delivery.task.implementation_baseline_id`），
/// 供「交接 S04」区展示：哪些任务以本基线为实施依据。
pub async fn baseline_tasks(
    pool: &PgPool,
    tenant_id: Uuid,
    baseline_id: Uuid,
) -> Result<Vec<BaselineTaskRow>, PmWriteError> {
    sqlx::query_as(
        "SELECT t.id, t.title, t.status, t.priority, t.created_at \
         FROM delivery.task t \
         WHERE t.tenant_id = $1 AND t.implementation_baseline_id = $2 \
         ORDER BY t.created_at",
    )
    .bind(tenant_id)
    .bind(baseline_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// S03-01/02 分析页读模型：每个设计的**需求覆盖**与**影响关系**。
///
/// - 需求覆盖：设计 ↔ 需求绑定按修订锚定（§3.4 第一条）；`is_current=false`
///   表示需求已被新版本取代而设计仍引用旧版——这正是 §6.2「不允许的行为」
///   之一，也是影响分析页要标红的那一类。
/// - 影响关系：本设计被哪些 ADR 引用（`design.adr_design_revision`），以及
///   本设计涉及的需求（供「改一处影响哪些需求」的反向视图）。
pub async fn design_analysis(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<DesignAnalysisRow>, PmWriteError> {
    let designs = list_designs(pool, tenant_id, project_id).await?;

    let requirement_links: Vec<AnalysisRequirementLink> = sqlx::query_as(
        "SELECT dr.design_id, dr.design_revision_id, dr.requirement_id, \
                dr.requirement_revision_id, r.code AS requirement_code, \
                r.title AS requirement_title, rv.version AS requirement_version, \
                rv.state AS requirement_state, \
                (r.current_revision_id = dr.requirement_revision_id) AS is_current \
         FROM design.design_requirement dr \
         LEFT JOIN delivery.requirement r ON r.id = dr.requirement_id \
         LEFT JOIN delivery.requirement_revision rv ON rv.id = dr.requirement_revision_id \
         WHERE dr.tenant_id = $1 AND dr.project_id = $2 \
         ORDER BY r.code",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let adr_links: Vec<AnalysisAdrLink> = sqlx::query_as(
        "SELECT l.design_id, l.adr_revision_id, a.id AS adr_id, a.code AS adr_code, \
                a.title AS adr_title, a.status AS adr_status, rv.version AS adr_version \
         FROM design.adr_design_revision l \
         LEFT JOIN design.adr a ON a.id = l.adr_id \
         LEFT JOIN design.adr_revision rv ON rv.id = l.adr_revision_id \
         WHERE l.tenant_id = $1 AND l.project_id = $2 \
         ORDER BY a.code",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let finding_rows: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT design_id, count(*) FROM design.review_finding \
         WHERE tenant_id = $1 AND project_id = $2 AND status = 'open' \
         GROUP BY design_id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let mut rows = Vec::with_capacity(designs.len());
    for design in designs {
        let requirements: Vec<AnalysisRequirementLink> = requirement_links
            .iter()
            .filter(|link| link.design_id == design.id)
            .cloned()
            .collect();
        let adrs: Vec<AnalysisAdrLink> = adr_links
            .iter()
            .filter(|link| link.design_id == design.id)
            .cloned()
            .collect();
        let open_findings = finding_rows
            .iter()
            .find(|(design_id, _)| *design_id == design.id)
            .map(|(_, count)| *count)
            .unwrap_or(0);
        let stale_requirement_count = requirements
            .iter()
            .filter(|l| l.is_current != Some(true))
            .count() as i64;
        rows.push(DesignAnalysisRow {
            design_id: design.id,
            code: design.code,
            title: design.title,
            lifecycle_status: design.lifecycle_status,
            current_revision_id: design.current_revision_id,
            current_version: design.current_version,
            current_state: design.current_state,
            requirement_count: requirements.len() as i64,
            stale_requirement_count,
            open_finding_count: open_findings,
            requirements,
            adrs,
        });
    }
    Ok(rows)
}

/// 专表审批插入 + pending 唯一冲突映射（批 A4 单源：design_revision /
/// adr_acceptance / task_plan 三域同构 INSERT 收敛于此）。
/// `entity_col` ∈ {design_id, adr_id, plan_id}（与 kind 一一对应）。
#[allow(clippy::too_many_arguments)] // 审批插入行，列清单与 design_approval 对应
pub(crate) async fn insert_design_approval(
    tx: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    project_id: Uuid,
    kind: &str,
    entity_col: &str,
    entity_id: Uuid,
    target_id: Uuid,
    target_hash: &str,
    requested_by: Uuid,
    pending_conflict: &'static str,
) -> Result<Uuid, PmWriteError> {
    let approval_id = Uuid::new_v4();
    sqlx::query(&format!(
        "INSERT INTO design.design_approval \
         (id, tenant_id, project_id, kind, {entity_col}, target_id, target_hash, requested_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
    ))
    .bind(approval_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(kind)
    .bind(entity_id)
    .bind(target_id)
    .bind(target_hash)
    .bind(requested_by)
    .execute(tx)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            PmWriteError::Conflict(pending_conflict)
        } else {
            PmWriteError::Db(e)
        }
    })?;
    Ok(approval_id)
}

pub async fn fetch_approval_view(
    db: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
    approval_id: Uuid,
) -> Result<ApprovalView, PmWriteError> {
    sqlx::query_as(
        "SELECT id, kind, design_id, adr_id, baseline_id, target_id, target_hash, status, \
                requested_by, decided_by, decision_reason, decided_at, created_at \
         FROM design.design_approval WHERE id = $1 AND tenant_id = $2",
    )
    .bind(approval_id)
    .bind(tenant_id)
    .fetch_optional(db)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("approval not found"))
}

/// 审批裁决（**唯一批准入口**；真人主体 + REST 真人门双闸）。
///
/// - `design_revision` approve：修订 approved + 旧批准版本 superseded +
///   design.current 指针切换（SC-01/SC-02/SC-06）；
/// - `adr_acceptance` approve：ADR under_review → accepted（Q3）；
/// - AI 主体调用即 `Forbidden`（SC-05；命令通道红线之外的服务面闸）。
pub async fn decide_approval(
    pool: &PgPool,
    tenant_id: Uuid,
    approval_id: Uuid,
    req: DecideApprovalRequest,
    actor: &AuditActor,
) -> Result<ApprovalView, PmWriteError> {
    if !["approve", "reject"].contains(&req.decision.as_str()) {
        return Err(PmWriteError::Validation(
            "decision must be approve or reject",
        ));
    }
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    type ApprovalDecisionRow = (
        Uuid,
        String,
        Option<Uuid>,
        Option<Uuid>,
        Option<Uuid>,
        Option<Uuid>,
        Uuid,
        String,
    );
    let (project_id, kind, design_id, adr_id, _baseline_id, plan_id, target_id, target_hash): ApprovalDecisionRow =
        sqlx::query_as(
        "SELECT project_id, kind, design_id, adr_id, baseline_id, plan_id, target_id, target_hash \
         FROM design.design_approval WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(approval_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("approval not found"))?;
    let status: String = sqlx::query_scalar(
        "SELECT status FROM design.design_approval WHERE id = $1 AND tenant_id = $2",
    )
    .bind(approval_id)
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if status != "pending" {
        return Err(PmWriteError::Conflict("approval is already decided"));
    }

    match kind.as_str() {
        "design_revision" => {
            let design_id = design_id.ok_or(PmWriteError::Validation("approval missing design"))?;
            let (project_id, lifecycle, current, row_revision) =
                lock_design(&mut tx, tenant_id, design_id).await?;
            if lifecycle != "active" {
                return Err(PmWriteError::Conflict("design is not active"));
            }
            // SC-06：审批人核准时的当前版本已漂移 → 显式 409。
            if let Some(expected) = req.expected_current_revision_id {
                if Some(expected) != current {
                    return Err(PmWriteError::Conflict(
                        "design current revision changed since the approval was reviewed",
                    ));
                }
            }
            if let Some(expected) = req.expected_row_revision {
                if expected != row_revision {
                    return Err(PmWriteError::Conflict(
                        "design row_revision changed since the approval was reviewed",
                    ));
                }
            }
            let (version, state, content_hash, _) =
                lock_design_revision(&mut tx, tenant_id, design_id, target_id).await?;
            if content_hash != target_hash {
                return Err(PmWriteError::Conflict(
                    "content hash mismatch: the reviewed content is not the current content",
                ));
            }
            let from = parse_state(&state)?;
            let to = if req.decision == "approve" {
                DesignRevisionState::Approved
            } else {
                DesignRevisionState::Rejected
            };
            transition_or_conflict(|f, t| f.can_transition_to(t), from, to, "illegal design revision transition")?;
            if req.decision == "approve" {
                // SC-05 前置：存在未关闭的 blocker 评审问题 → 拒绝批准。
                let open_blockers: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM design.review_finding \
                     WHERE tenant_id = $1 AND design_revision_id = $2 \
                       AND severity = 'blocker' AND status <> 'resolved'",
                )
                .bind(tenant_id)
                .bind(target_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(PmWriteError::Db)?;
                if open_blockers > 0 {
                    return Err(PmWriteError::Conflict(
                        "open blocker review findings must be resolved before approval",
                    ));
                }
                // SC-02：新版本批准，旧批准版本退位（历史保留、可查询）。
                sqlx::query(
                    "UPDATE design.design_revision SET state = 'superseded' \
                     WHERE design_id = $1 AND tenant_id = $2 AND state = 'approved' AND id <> $3",
                )
                .bind(design_id)
                .bind(tenant_id)
                .bind(target_id)
                .execute(&mut *tx)
                .await
                .map_err(PmWriteError::Db)?;
                sqlx::query(
                    "UPDATE design.design_revision \
                     SET state = 'approved', approved_by = $1, approved_at = now() \
                     WHERE id = $2 AND design_id = $3 AND tenant_id = $4 AND state = 'pending_confirmation'",
                )
                .bind(actor.principal_id)
                .bind(target_id)
                .bind(design_id)
                .bind(tenant_id)
                .execute(&mut *tx)
                .await
                .map_err(PmWriteError::Db)?;
                sqlx::query(
                    "UPDATE design.design \
                     SET current_revision_id = $1, row_revision = row_revision + 1, updated_at = now() \
                     WHERE id = $2 AND tenant_id = $3",
                )
                .bind(target_id)
                .bind(design_id)
                .bind(tenant_id)
                .execute(&mut *tx)
                .await
                .map_err(PmWriteError::Db)?;
            } else {
                sqlx::query(
                    "UPDATE design.design_revision \
                     SET state = 'rejected' \
                     WHERE id = $1 AND design_id = $2 AND tenant_id = $3 AND state = 'pending_confirmation'",
                )
                .bind(target_id)
                .bind(design_id)
                .bind(tenant_id)
                .execute(&mut *tx)
                .await
                .map_err(PmWriteError::Db)?;
            }
            finalize_approval(
                &mut tx,
                project_id,
                approval_id,
                &req,
                actor,
                json!({ "version": version }),
            )
            .await?;
        }
        "adr_acceptance" => {
            let adr_id = adr_id.ok_or(PmWriteError::Validation("approval missing adr"))?;
            let (project_id, status_now, current_revision_id, _) =
                lock_adr(&mut tx, tenant_id, adr_id).await?;
            let from = AdrStatus::parse(&status_now)
                .ok_or_else(|| PmWriteError::Validation("unknown adr status"))?;
            let to = if req.decision == "approve" {
                AdrStatus::Accepted
            } else {
                AdrStatus::Rejected
            };
            transition_or_conflict(|f, t| f.can_transition_to(t), from, to, "illegal adr transition")?;
            // 内容锚定：当前内容版本哈希必须与审批人核准的哈希一致。
            let content_hash: String = sqlx::query_scalar(
                "SELECT content_hash FROM design.adr_revision \
                 WHERE id = $1 AND tenant_id = $2 AND adr_id = $3",
            )
            .bind(current_revision_id)
            .bind(tenant_id)
            .bind(adr_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(PmWriteError::Db)?
            .ok_or(PmWriteError::Conflict("adr has no content revision"))?;
            if content_hash != target_hash {
                return Err(PmWriteError::Conflict(
                    "content hash mismatch: the reviewed content is not the current content",
                ));
            }
            let to_str = to.as_str();
            sqlx::query(
                "UPDATE design.adr \
                 SET status = $1, row_revision = row_revision + 1, updated_at = now() \
                 WHERE id = $2 AND tenant_id = $3",
            )
            .bind(to_str)
            .bind(adr_id)
            .bind(tenant_id)
            .execute(&mut *tx)
            .await
            .map_err(PmWriteError::Db)?;
            finalize_approval(
                &mut tx,
                project_id,
                approval_id,
                &req,
                actor,
                json!({ "status": to_str }),
            )
            .await?;
        }
        "task_plan" => {
            // 计划裁决走独立事务（apply_plan_decision 内含 require_human +
            // 状态机校验）；随后在本事务内终局化审批行。
            let target_plan_id =
                plan_id.ok_or(PmWriteError::Validation("approval missing plan"))?;
            crate::task_plan::apply_plan_decision(
                pool,
                tenant_id,
                target_plan_id,
                req.decision == "approve",
                actor,
            )
            .await?;
            finalize_approval(
                &mut tx,
                project_id,
                approval_id,
                &req,
                actor,
                json!({ "kind": "task_plan" }),
            )
            .await?;
        }
        _ => {
            return Err(PmWriteError::Validation("unknown approval kind"));
        }
    }
    tx.commit().await.map_err(PmWriteError::Db)?;
    fetch_approval_view(pool, tenant_id, approval_id).await
}

async fn finalize_approval(
    tx: &mut sqlx::PgConnection,
    project_id: Uuid,
    approval_id: Uuid,
    req: &DecideApprovalRequest,
    actor: &AuditActor,
    after: Value,
) -> Result<(), PmWriteError> {
    let decision = if req.decision == "approve" {
        "approved"
    } else {
        "rejected"
    };
    sqlx::query(
        "UPDATE design.design_approval \
         SET status = $1, decided_by = $2, decision_reason = $3, decided_at = now() \
         WHERE id = $4 AND status = 'pending'",
    )
    .bind(decision)
    .bind(actor.principal_id)
    .bind(req.reason.clone())
    .bind(approval_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        tx,
        project_id,
        actor,
        "approval.decide",
        "design_approval",
        Some(approval_id),
        after,
    )
    .await?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────── ADR（Q3）

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

pub async fn list_adrs(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<AdrEntry>, PmWriteError> {
    sqlx::query_as(
        "SELECT a.id, a.project_id, a.code, a.title, a.status, a.blockers_implementation, \
                a.current_revision_id, rv.version AS current_version, \
                a.superseded_by_adr_id, a.row_revision, a.created_at, a.updated_at \
         FROM design.adr a \
         LEFT JOIN design.adr_revision rv ON rv.id = a.current_revision_id \
         WHERE a.tenant_id = $1 AND a.project_id = $2 \
         ORDER BY a.created_at DESC",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

#[derive(Debug, Serialize)]
pub struct AdrDetail {
    pub adr: AdrEntry,
    pub revisions: Vec<AdrRevisionView>,
    pub design_links: Vec<Value>,
    pub requirement_links: Vec<Value>,
    pub approvals: Vec<ApprovalView>,
}

pub async fn adr_detail(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
) -> Result<AdrDetail, PmWriteError> {
    let adr: AdrEntry = sqlx::query_as(
        "SELECT a.id, a.project_id, a.code, a.title, a.status, a.blockers_implementation, \
                a.current_revision_id, rv.version AS current_version, \
                a.superseded_by_adr_id, a.row_revision, a.created_at, a.updated_at \
         FROM design.adr a \
         LEFT JOIN design.adr_revision rv ON rv.id = a.current_revision_id \
         WHERE a.tenant_id = $1 AND a.id = $2",
    )
    .bind(tenant_id)
    .bind(adr_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("adr not found"))?;
    let revisions: Vec<AdrRevisionView> = sqlx::query_as(
        "SELECT id, adr_id, version, content, state, content_hash, created_by, created_at \
         FROM design.adr_revision WHERE tenant_id = $1 AND adr_id = $2 ORDER BY version DESC",
    )
    .bind(tenant_id)
    .bind(adr_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let design_links: Vec<Value> = sqlx::query(
        "SELECT l.adr_revision_id, l.design_id, l.design_revision_id, d.code, rv.version \
         FROM design.adr_design_revision l \
         LEFT JOIN design.design d ON d.id = l.design_id \
         LEFT JOIN design.design_revision rv ON rv.id = l.design_revision_id \
         WHERE l.tenant_id = $1 AND l.adr_id = $2 ORDER BY l.created_at",
    )
    .bind(tenant_id)
    .bind(adr_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?
    .into_iter()
    .map(|row| {
        use sqlx::Row;
        json!({
            "adr_revision_id": row.get::<Uuid, _>("adr_revision_id"),
            "design_id": row.get::<Uuid, _>("design_id"),
            "design_revision_id": row.get::<Uuid, _>("design_revision_id"),
            "design_code": row.get::<Option<String>, _>("code"),
            "design_revision_version": row.get::<Option<i32>, _>("version"),
        })
    })
    .collect();
    let requirement_links: Vec<Value> = sqlx::query(
        "SELECT l.adr_revision_id, l.requirement_id, l.requirement_revision_id, \
                r.code, rv.version \
         FROM design.adr_requirement l \
         LEFT JOIN delivery.requirement r ON r.id = l.requirement_id \
         LEFT JOIN delivery.requirement_revision rv ON rv.id = l.requirement_revision_id \
         WHERE l.tenant_id = $1 AND l.adr_id = $2 ORDER BY l.created_at",
    )
    .bind(tenant_id)
    .bind(adr_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?
    .into_iter()
    .map(|row| {
        use sqlx::Row;
        json!({
            "adr_revision_id": row.get::<Uuid, _>("adr_revision_id"),
            "requirement_id": row.get::<Uuid, _>("requirement_id"),
            "requirement_revision_id": row.get::<Uuid, _>("requirement_revision_id"),
            "requirement_code": row.get::<Option<String>, _>("code"),
            "requirement_revision_version": row.get::<Option<i32>, _>("version"),
        })
    })
    .collect();
    let approvals: Vec<ApprovalView> = sqlx::query_as(
        "SELECT id, kind, design_id, adr_id, baseline_id, target_id, target_hash, status, \
                requested_by, decided_by, decision_reason, decided_at, created_at \
         FROM design.design_approval \
         WHERE tenant_id = $1 AND adr_id = $2 ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .bind(adr_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    Ok(AdrDetail {
        adr,
        revisions,
        design_links,
        requirement_links,
        approvals,
    })
}

fn build_adr_content(req: &DraftAdrRevisionRequest) -> Value {
    let mut content = json!({ "decision_question": req.decision_question });
    let obj = content.as_object_mut().expect("object");
    if let Some(s) = req.context.as_deref().filter(|s| !s.trim().is_empty()) {
        obj.insert("context".into(), json!(s));
    }
    if !req.options.is_empty() {
        obj.insert("options".into(), json!(req.options));
    }
    if let Some(s) = req
        .chosen_option
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        obj.insert("chosen_option".into(), json!(s));
    }
    if let Some(s) = req.rationale.as_deref().filter(|s| !s.trim().is_empty()) {
        obj.insert("rationale".into(), json!(s));
    }
    if let Some(s) = req.consequences.as_deref().filter(|s| !s.trim().is_empty()) {
        obj.insert("consequences".into(), json!(s));
    }
    content
}

pub async fn create_adr(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    req: CreateAdrRequest,
    actor: &AuditActor,
) -> Result<AdrDetail, PmWriteError> {
    let title = req.title.trim().to_string();
    if title.is_empty() {
        return Err(PmWriteError::Validation("title is required"));
    }
    if req.decision_question.trim().is_empty() {
        return Err(PmWriteError::Validation("decision_question is required"));
    }
    let content = build_adr_content(&DraftAdrRevisionRequest {
        decision_question: req.decision_question.clone(),
        context: req.context.clone(),
        options: req.options.clone(),
        chosen_option: req.chosen_option.clone(),
        rationale: req.rationale.clone(),
        consequences: req.consequences.clone(),
    });
    let content_hash = content_hash_of(&content);

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let code = match req.code.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(c) => c.to_string(),
        None => next_code(&mut tx, tenant_id, project_id, "design.adr", "ADR").await?,
    };
    let adr_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.adr \
         (id, tenant_id, project_id, code, title, blockers_implementation, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(adr_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(&code)
    .bind(&title)
    .bind(req.blockers_implementation)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            PmWriteError::Conflict("adr code already exists in this project")
        } else {
            PmWriteError::Db(e)
        }
    })?;
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.adr_revision \
         (id, tenant_id, project_id, adr_id, version, content, state, content_hash, created_by) \
         VALUES ($1, $2, $3, $4, 1, $5, 'draft', $6, $7)",
    )
    .bind(revision_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(adr_id)
    .bind(&content)
    .bind(&content_hash)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "adr.created",
        "adr",
        Some(adr_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    adr_detail(pool, tenant_id, adr_id).await
}

/// 追加 ADR 内容修订（仅 proposed/under_review；已接受的决策不可改，走新 ADR）。
pub async fn create_adr_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    req: DraftAdrRevisionRequest,
    actor: &AuditActor,
) -> Result<AdrRevisionView, PmWriteError> {
    if req.decision_question.trim().is_empty() {
        return Err(PmWriteError::Validation("decision_question is required"));
    }
    let content = build_adr_content(&req);
    let content_hash = content_hash_of(&content);

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, status, _, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    if !matches!(status.as_str(), "proposed" | "under_review") {
        return Err(PmWriteError::Conflict(
            "accepted/rejected/superseded/deprecated adrs are immutable; create a new adr",
        ));
    }
    let version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM design.adr_revision \
         WHERE adr_id = $1 AND tenant_id = $2",
    )
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    let revision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.adr_revision \
         (id, tenant_id, project_id, adr_id, version, content, state, content_hash, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, 'draft', $7, $8)",
    )
    .bind(revision_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(adr_id)
    .bind(version)
    .bind(&content)
    .bind(&content_hash)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "adr_revision.created",
        "adr_revision",
        Some(revision_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    sqlx::query_as(
        "SELECT id, adr_id, version, content, state, content_hash, created_by, created_at \
         FROM design.adr_revision WHERE id = $1 AND tenant_id = $2",
    )
    .bind(revision_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 提交 ADR 内容（revision draft→submitted + adr proposed→under_review）。
pub async fn submit_adr_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    revision_id: Uuid,
    actor: &AuditActor,
) -> Result<AdrDetail, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, status, _, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    let (version, state): (i32, String) = sqlx::query_as(
        "SELECT version, state FROM design.adr_revision \
         WHERE id = $1 AND adr_id = $2 AND tenant_id = $3 FOR UPDATE",
    )
    .bind(revision_id)
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("adr revision not found"))?;
    let from = AdrRevisionState::parse(&state)
        .ok_or_else(|| PmWriteError::Validation("unknown adr revision state"))?;
    if !from.can_transition_to(AdrRevisionState::Submitted) {
        return Err(PmWriteError::Conflict("adr revision already submitted"));
    }
    sqlx::query(
        "UPDATE design.adr_revision SET state = 'submitted' \
         WHERE id = $1 AND adr_id = $2 AND tenant_id = $3 AND state = 'draft'",
    )
    .bind(revision_id)
    .bind(adr_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    let adr_from =
        AdrStatus::parse(&status).ok_or_else(|| PmWriteError::Validation("unknown adr status"))?;
    if adr_from == AdrStatus::Proposed {
        transition_or_conflict(
            |f, t| f.can_transition_to(t),
            adr_from,
            AdrStatus::UnderReview,
            "illegal adr transition",
        )?;
        sqlx::query(
            "UPDATE design.adr SET status = 'under_review', updated_at = now() \
             WHERE id = $1 AND tenant_id = $2",
        )
        .bind(adr_id)
        .bind(tenant_id)
        .execute(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
    }
    // 提交即成为当前内容版本（D-03：acceptance 锚定最新提交内容）。
    sqlx::query(
        "UPDATE design.adr SET current_revision_id = $1, updated_at = now() \
         WHERE id = $2 AND tenant_id = $3",
    )
    .bind(revision_id)
    .bind(adr_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "adr_revision.submitted",
        "adr_revision",
        Some(revision_id),
        json!({ "version": version }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    adr_detail(pool, tenant_id, adr_id).await
}

/// ADR 关联设计版本（§3.4 关键约束二：引用具体 Design Revision；仅 draft 修订可改）。
pub async fn link_adr_design_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    adr_revision_id: Uuid,
    req: LinkAdrDesignRequest,
    actor: &AuditActor,
) -> Result<(), PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, current, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    let (state,): (String,) = sqlx::query_as(
        "SELECT state FROM design.adr_revision \
         WHERE id = $1 AND adr_id = $2 AND tenant_id = $3",
    )
    .bind(adr_revision_id)
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("adr revision not found"))?;
    if !AdrRevisionState::parse(&state)
        .map(|s| s.content_editable())
        .unwrap_or(false)
    {
        return Err(PmWriteError::Conflict(
            "design links are only editable on draft adr revisions",
        ));
    }
    // 设计修订必须属于本项目（跨项目越权拦截）。
    let in_project: Option<Uuid> = sqlx::query_scalar(
        "SELECT rv.id FROM design.design_revision rv \
         WHERE rv.id = $1 AND rv.design_id = $2 AND rv.tenant_id = $3 AND rv.project_id = $4",
    )
    .bind(req.design_revision_id)
    .bind(req.design_id)
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if in_project.is_none() {
        return Err(PmWriteError::Validation(
            "design revision not found in this project",
        ));
    }
    let _ = current;
    sqlx::query(
        "INSERT INTO design.adr_design_revision \
         (adr_id, adr_revision_id, design_id, design_revision_id, tenant_id, project_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (adr_revision_id, design_revision_id) DO NOTHING",
    )
    .bind(adr_id)
    .bind(adr_revision_id)
    .bind(req.design_id)
    .bind(req.design_revision_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "adr_design_revision.linked",
        "adr_revision",
        Some(adr_revision_id),
        json!({
            "design_id": req.design_id,
            "design_revision_id": req.design_revision_id,
        }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    Ok(())
}

/// ADR 关联需求修订（与设计关联同规则：必须 approved；仅 draft 修订可改）。
pub async fn link_adr_requirement(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    adr_revision_id: Uuid,
    req: LinkAdrRequirementRequest,
    actor: &AuditActor,
) -> Result<(), PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    let (project_id, _, _, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    let (state,): (String,) = sqlx::query_as(
        "SELECT state FROM design.adr_revision \
         WHERE id = $1 AND adr_id = $2 AND tenant_id = $3",
    )
    .bind(adr_revision_id)
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("adr revision not found"))?;
    if !AdrRevisionState::parse(&state)
        .map(|s| s.content_editable())
        .unwrap_or(false)
    {
        return Err(PmWriteError::Conflict(
            "requirement links are only editable on draft adr revisions",
        ));
    }
    let (req_current,): (Option<Uuid>,) = sqlx::query_as(
        "SELECT current_revision_id FROM delivery.requirement \
         WHERE id = $1 AND tenant_id = $2 AND project_id = $3",
    )
    .bind(req.requirement_id)
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("requirement not found"))?;
    let revision_id_target =
        req.requirement_revision_id
            .unwrap_or(req_current.ok_or(PmWriteError::Validation(
                "requirement has no current revision to link",
            ))?);
    let (rev_state,): (String,) = sqlx::query_as(
        "SELECT state FROM delivery.requirement_revision \
         WHERE id = $1 AND requirement_id = $2 AND tenant_id = $3",
    )
    .bind(revision_id_target)
    .bind(req.requirement_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::Validation("requirement revision not found"))?;
    if rev_state != "approved" {
        return Err(PmWriteError::Validation(
            "only approved requirement revisions can be linked",
        ));
    }
    sqlx::query(
        "INSERT INTO design.adr_requirement \
         (adr_id, adr_revision_id, requirement_id, requirement_revision_id, \
          tenant_id, project_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (adr_revision_id, requirement_id) DO NOTHING",
    )
    .bind(adr_id)
    .bind(adr_revision_id)
    .bind(req.requirement_id)
    .bind(revision_id_target)
    .bind(tenant_id)
    .bind(project_id)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "adr_requirement.linked",
        "adr_revision",
        Some(adr_revision_id),
        json!({
            "requirement_id": req.requirement_id,
            "requirement_revision_id": revision_id_target,
        }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    Ok(())
}

/// 决策被新 ADR 取代（accepted → superseded；后继必须已接受）。
pub async fn supersede_adr(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    req: SupersedeAdrRequest,
    actor: &AuditActor,
) -> Result<AdrEntry, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    let (project_id, status, _, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    transition_or_conflict(
        |f, t| f.can_transition_to(t),
        AdrStatus::parse(&status).ok_or_else(|| PmWriteError::Validation("unknown adr status"))?,
        AdrStatus::Superseded,
        "illegal adr transition",
    )?;
    let (successor_status, successor_project): (String, Uuid) = sqlx::query_as(
        "SELECT status, project_id FROM design.adr \
         WHERE id = $1 AND tenant_id = $2",
    )
    .bind(req.superseded_by_adr_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::Validation("successor adr not found"))?;
    if successor_project != project_id {
        return Err(PmWriteError::Validation(
            "successor adr is in another project",
        ));
    }
    if successor_status != AdrStatus::Accepted.as_str() {
        return Err(PmWriteError::Validation(
            "successor adr must be accepted before it can supersede",
        ));
    }
    if req.superseded_by_adr_id == adr_id {
        return Err(PmWriteError::Validation("an adr cannot supersede itself"));
    }
    sqlx::query(
        "UPDATE design.adr \
         SET status = 'superseded', superseded_by_adr_id = $1, row_revision = row_revision + 1, updated_at = now() \
         WHERE id = $2 AND tenant_id = $3",
    )
    .bind(req.superseded_by_adr_id)
    .bind(adr_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "adr.superseded",
        "adr",
        Some(adr_id),
        json!({ "superseded_by_adr_id": req.superseded_by_adr_id, "reason": req.reason }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    sqlx::query_as(
        "SELECT a.id, a.project_id, a.code, a.title, a.status, a.blockers_implementation, \
                a.current_revision_id, rv.version AS current_version, \
                a.superseded_by_adr_id, a.row_revision, a.created_at, a.updated_at \
         FROM design.adr a \
         LEFT JOIN design.adr_revision rv ON rv.id = a.current_revision_id \
         WHERE a.id = $1 AND a.tenant_id = $2",
    )
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 废弃 ADR（proposed/under_review/accepted → deprecated；真人命令）。
pub async fn deprecate_adr(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    actor: &AuditActor,
) -> Result<AdrEntry, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    let (project_id, status, _, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    transition_or_conflict(
        |f, t| f.can_transition_to(t),
        AdrStatus::parse(&status).ok_or_else(|| PmWriteError::Validation("unknown adr status"))?,
        AdrStatus::Deprecated,
        "illegal adr transition",
    )?;
    sqlx::query(
        "UPDATE design.adr SET status = 'deprecated', row_revision = row_revision + 1, updated_at = now() \
         WHERE id = $1 AND tenant_id = $2",
    )
    .bind(adr_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "adr.deprecated",
        "adr",
        Some(adr_id),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    sqlx::query_as(
        "SELECT a.id, a.project_id, a.code, a.title, a.status, a.blockers_implementation, \
                a.current_revision_id, rv.version AS current_version, \
                a.superseded_by_adr_id, a.row_revision, a.created_at, a.updated_at \
         FROM design.adr a \
         LEFT JOIN design.adr_revision rv ON rv.id = a.current_revision_id \
         WHERE a.id = $1 AND a.tenant_id = $2",
    )
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

/// 设置实施阻塞标记（§6.2；真人命令——直接决定 Readiness Gate 判定输入）。
pub async fn set_adr_implementation_blocking(
    pool: &PgPool,
    tenant_id: Uuid,
    adr_id: Uuid,
    req: SetBlockingRequest,
    actor: &AuditActor,
) -> Result<AdrEntry, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    let (project_id, _, _, _) = lock_adr(&mut tx, tenant_id, adr_id).await?;
    sqlx::query(
        "UPDATE design.adr SET blockers_implementation = $1, updated_at = now() \
         WHERE id = $2 AND tenant_id = $3",
    )
    .bind(req.blockers_implementation)
    .bind(adr_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "adr.blocking_updated",
        "adr",
        Some(adr_id),
        json!({ "blockers_implementation": req.blockers_implementation }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    sqlx::query_as(
        "SELECT a.id, a.project_id, a.code, a.title, a.status, a.blockers_implementation, \
                a.current_revision_id, rv.version AS current_version, \
                a.superseded_by_adr_id, a.row_revision, a.created_at, a.updated_at \
         FROM design.adr a \
         LEFT JOIN design.adr_revision rv ON rv.id = a.current_revision_id \
         WHERE a.id = $1 AND a.tenant_id = $2",
    )
    .bind(adr_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)
}

// ───────────────────────────────────── Implementation Baseline（Q5 §6.3）

pub async fn list_baselines(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<BaselineEntry>, PmWriteError> {
    sqlx::query_as(
        "SELECT id, project_id, code, status, content, content_hash, readiness, \
                frozen_by, frozen_at, superseded_by_baseline_id, created_at \
         FROM design.implementation_baseline \
         WHERE tenant_id = $1 AND project_id = $2 ORDER BY frozen_at DESC",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

pub async fn baseline_detail(
    pool: &PgPool,
    tenant_id: Uuid,
    baseline_id: Uuid,
) -> Result<BaselineEntry, PmWriteError> {
    sqlx::query_as(
        "SELECT id, project_id, code, status, content, content_hash, readiness, \
                frozen_by, frozen_at, superseded_by_baseline_id, created_at \
         FROM design.implementation_baseline \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(baseline_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("baseline not found"))
}

/// 冻结实施基线（§6.3）：Readiness Gate 五条件全过 → 不可变快照 + 冻结审批行。
///
/// 冻结后设计/需求/ADR 的任何变化都不会改变本基线（SC-08）；基线变更 =
/// 重新冻结新基线 + 显式 supersede。
pub async fn freeze_baseline(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    req: FreezeBaselineRequest,
    actor: &AuditActor,
) -> Result<BaselineEntry, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    let gate = evaluate_readiness(pool, tenant_id, project_id, &req.design_revision_ids).await?;
    if !gate.ready {
        return Err(PmWriteError::Conflict(
            "readiness gate failed: resolve the failed conditions before freezing a baseline",
        ));
    }
    let revision_ids = req.design_revision_ids.clone();

    // 设计与修订快照。
    let design_rows: Vec<(Uuid, String, String, Uuid, i32, String)> = sqlx::query_as(
        "SELECT d.id, d.code, d.title, rv.id, rv.version, rv.content_hash \
         FROM design.design_revision rv \
         JOIN design.design d ON d.id = rv.design_id \
         WHERE rv.tenant_id = $1 AND rv.project_id = $2 AND rv.id = ANY($3) \
         ORDER BY d.code, rv.version",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&revision_ids)
    .fetch_all(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;

    // 需求依据快照（去重）。
    let requirement_rows: Vec<(Uuid, Uuid, i32)> = sqlx::query_as(
        "SELECT DISTINCT dr.requirement_id, dr.requirement_revision_id, rv.version \
         FROM design.design_requirement dr \
         JOIN delivery.requirement_revision rv ON rv.id = dr.requirement_revision_id \
         WHERE dr.tenant_id = $1 AND dr.project_id = $2 \
           AND dr.design_revision_id = ANY($3) \
         ORDER BY dr.requirement_id, dr.requirement_revision_id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&revision_ids)
    .fetch_all(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;

    // ADR 快照（经设计关联收集；去重）。
    let adr_rows: Vec<(Uuid, String, Uuid, String)> = sqlx::query_as(
        "SELECT DISTINCT l.adr_id, a.code, l.adr_revision_id, a.status \
         FROM design.adr_design_revision l \
         JOIN design.adr a ON a.id = l.adr_id \
         WHERE l.tenant_id = $1 AND l.project_id = $2 \
           AND l.design_revision_id = ANY($3) \
         ORDER BY l.adr_id, l.adr_revision_id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&revision_ids)
    .fetch_all(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;

    // 范围/验收合并（逐修订 content 结构化字段并集）。
    let mut included: Vec<Value> = Vec::new();
    let mut excluded: Vec<Value> = Vec::new();
    let mut modules: Vec<Value> = Vec::new();
    let mut acceptance: Vec<Value> = Vec::new();
    for rid in &revision_ids {
        let content: Option<Value> = sqlx::query_scalar(
            "SELECT content FROM design.design_revision \
             WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(rid)
        .fetch_optional(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
        let Some(content) = content else { continue };
        if let Some(scope) = content.get("implementation_scope") {
            for key in ["included", "excluded", "modules"] {
                if let Some(arr) = scope.get(key).and_then(Value::as_array) {
                    for item in arr {
                        let target = match key {
                            "included" => &mut included,
                            "excluded" => &mut excluded,
                            _ => &mut modules,
                        };
                        if !target.contains(item) {
                            target.push(item.clone());
                        }
                    }
                }
            }
        }
        if let Some(arr) = content.get("acceptance_criteria").and_then(Value::as_array) {
            for item in arr {
                if !acceptance.contains(item) {
                    acceptance.push(item.clone());
                }
            }
        }
    }

    // 仓库引用校验（§6.3 Repository Context 固定依据：必须属于本项目）。
    let unique_repo_ids: Vec<Uuid> = {
        let mut seen = std::collections::HashSet::new();
        req.repository_refs
            .iter()
            .filter(|r| seen.insert(r.repository_id))
            .map(|r| r.repository_id)
            .collect()
    };
    if !unique_repo_ids.is_empty() {
        let known: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM integration.project_repository \
             WHERE id = ANY($1) AND project_id = $2",
        )
        .bind(&unique_repo_ids)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
        if known != unique_repo_ids.len() as i64 {
            return Err(PmWriteError::Validation(
                "repository_refs contains repositories outside this project",
            ));
        }
    }

    let content = json!({
        "design_revision_ids": revision_ids,
        "designs": design_rows.iter().map(|(did, code, title, rid, ver, hash)| json!({
            "design_id": did, "code": code, "title": title,
            "design_revision_id": rid, "version": ver, "content_hash": hash,
        })).collect::<Vec<_>>(),
        "requirements": requirement_rows.iter().map(|(rid, rvid, ver)| json!({
            "requirement_id": rid, "requirement_revision_id": rvid, "version": ver,
        })).collect::<Vec<_>>(),
        "adrs": adr_rows.iter().map(|(aid, code, arid, status)| json!({
            "adr_id": aid, "code": code, "adr_revision_id": arid, "status": status,
        })).collect::<Vec<_>>(),
        "scope": { "included": included, "excluded": excluded, "modules": modules },
        "acceptance_criteria": acceptance,
        "repository_refs": req.repository_refs.iter().map(|r| json!({
            "repository_id": r.repository_id,
            "base_commit": r.base_commit,
        })).collect::<Vec<_>>(),
    });
    let content_hash = content_hash_of(&content);
    let gate_json = serde_json::to_value(&gate)
        .map_err(|e| PmWriteError::Db(sqlx::Error::Protocol(format!("gate serialization: {e}"))))?;

    let code = match req.code.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(c) => c.to_string(),
        None => {
            next_code(
                &mut tx,
                tenant_id,
                project_id,
                "design.implementation_baseline",
                "IB",
            )
            .await?
        }
    };
    let baseline_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO design.implementation_baseline \
         (id, tenant_id, project_id, code, content, content_hash, readiness, frozen_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(baseline_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(&code)
    .bind(&content)
    .bind(&content_hash)
    .bind(&gate_json)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            PmWriteError::Conflict("baseline code already exists in this project")
        } else {
            PmWriteError::Db(e)
        }
    })?;
    // R6：基线冻结的独立审批语义——专表审批行（created+approved 同事务，冻结人即批准人）。
    sqlx::query(
        "INSERT INTO design.design_approval \
         (id, tenant_id, project_id, kind, baseline_id, target_id, target_hash, \
          status, requested_by, decided_by, decision_reason, decided_at) \
         VALUES ($1, $2, $3, 'baseline_freeze', $4, $4, $5, 'approved', $6, $6, $7, now())",
    )
    .bind(Uuid::new_v4())
    .bind(tenant_id)
    .bind(project_id)
    .bind(baseline_id)
    .bind(&content_hash)
    .bind(actor.principal_id)
    .bind(req_reason_of_freeze(&gate))
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "baseline.frozen",
        "implementation_baseline",
        Some(baseline_id),
        json!({ "code": code, "content_hash": content_hash }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    baseline_detail(pool, tenant_id, baseline_id).await
}

fn req_reason_of_freeze(gate: &GateReport) -> Option<String> {
    let passed = gate
        .conditions
        .iter()
        .filter(|c| c.passed)
        .map(|c| c.condition)
        .collect::<Vec<_>>()
        .join(",");
    Some(format!("readiness gate passed: {passed}"))
}

/// 显式替代基线（SC-08 的另一半：新基线冻结后，旧基线由人显式退位）。
pub async fn supersede_baseline(
    pool: &PgPool,
    tenant_id: Uuid,
    baseline_id: Uuid,
    req: SupersedeBaselineRequest,
    actor: &AuditActor,
) -> Result<BaselineEntry, PmWriteError> {
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    require_human_conn(&mut tx, actor).await?;
    let (project_id, status): (Uuid, String) = sqlx::query_as(
        "SELECT project_id, status FROM design.implementation_baseline \
         WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(baseline_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("baseline not found"))?;
    if status != "frozen" {
        return Err(PmWriteError::Conflict("baseline is already superseded"));
    }
    if let Some(successor) = req.successor_baseline_id {
        let (s_project, s_status): (Uuid, String) = sqlx::query_as(
            "SELECT project_id, status FROM design.implementation_baseline \
             WHERE id = $1 AND tenant_id = $2",
        )
        .bind(successor)
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?
        .ok_or(PmWriteError::Validation("successor baseline not found"))?;
        if s_project != project_id {
            return Err(PmWriteError::Validation(
                "successor baseline is in another project",
            ));
        }
        if s_status != "frozen" {
            return Err(PmWriteError::Validation(
                "successor baseline must be frozen",
            ));
        }
        sqlx::query(
            "UPDATE design.implementation_baseline \
             SET status = 'superseded', superseded_by_baseline_id = $1 \
             WHERE id = $2 AND tenant_id = $3",
        )
        .bind(successor)
        .bind(baseline_id)
        .bind(tenant_id)
        .execute(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
    } else {
        sqlx::query(
            "UPDATE design.implementation_baseline SET status = 'superseded' \
             WHERE id = $1 AND tenant_id = $2",
        )
        .bind(baseline_id)
        .bind(tenant_id)
        .execute(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
    }
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "baseline.superseded",
        "implementation_baseline",
        Some(baseline_id),
        json!({ "superseded_by_baseline_id": req.successor_baseline_id, "reason": req.reason }),
    )
    .await?;
    tx.commit().await.map_err(PmWriteError::Db)?;
    baseline_detail(pool, tenant_id, baseline_id).await
}
