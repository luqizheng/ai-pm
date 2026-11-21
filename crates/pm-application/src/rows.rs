//! `/v1/pm/*` API 行结构（Slice A 契约，字段源：`frontend/src/api/types.ts`，
//! 与 `migrations/0001-0004` 表列一一对应）。读端点与写端点返回共用。

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PrincipalRow {
    pub id: Uuid,
    pub kind: String,
    pub display_name: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// 客户主数据（0017）。`status`：`active` / `archived` / `placeholder`
/// （`placeholder` = 旧 bootstrap 以项目 code 隐式造的未归类客户，待人工合并）。
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CustomerRow {
    pub id: Uuid,
    pub code: Option<String>,
    pub name: String,
    pub short_name: Option<String>,
    pub industry: Option<String>,
    pub notes: Option<String>,
    pub parent_id: Option<Uuid>,
    pub status: String,
    /// 该客户下的项目数（列表端点按 customer 聚合统计）。
    pub project_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ProjectRow {
    pub id: Uuid,
    pub customer_id: Uuid,
    /// 冗余客户显示名，读端点 LEFT JOIN 得来；写端点插入后同样带出，避免前端二次请求。
    pub customer_name: String,
    /// 客户状态：`active` / `archived` / `placeholder`（未归类历史客户）。
    pub customer_status: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RequirementRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub code: String,
    pub title: String,
    /// 0039（S02D-04）：原 status 列改名；是否有正式版本由 current_revision_id 表达
    pub lifecycle_status: String,
    pub current_revision_id: Option<Uuid>,
    pub row_revision: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MilestoneRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub planned_at: Option<DateTime<Utc>>,
    pub status: String,
    pub revision: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DecisionRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub status: String,
    pub required_role: String,
    pub decided_by: Option<Uuid>,
    pub decision_reason: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub proposal_id: Option<Uuid>,
    pub proposal_status: Option<String>,
    pub target_type: Option<String>,
    pub command_type: Option<String>,
    pub rationale: Option<String>,
    pub risk_level: Option<String>,
    pub evidence: Option<serde_json::Value>,
    pub proposed_data: Option<serde_json::Value>,
    pub analysis_run_id: Option<Uuid>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TaskRow {
    pub id: Uuid,
    /// PA-07 任务身份锚（0050）：项目内业务编号 PM-NNN，UI 展示/引用用，非外键。
    pub code: String,
    pub project_id: Uuid,
    pub milestone_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub assignee_id: Option<Uuid>,
    pub revision: i64,
    /// 创建时锚定的 AI Role 版本（D-2 遗留补读面：0058 可空 FK，历史任务为 NULL）。
    pub ai_role_version_id: Option<Uuid>,
    /// 计划截止（0015 task_schedule；PA-07 列表「P0 · Due …」列数据源）。
    pub planned_end_at: Option<DateTime<Utc>>,
    /// 活跃关联产品 [{id,name,status}]（0044 task_product，withdrawn 不含）；无关联为 []。
    pub products: serde_json::Value,
    /// 最新一次 run（attempt_no 降序第一条：{id,attempt_no,status,review_verdict,created_at}）；无 run 为 null。
    pub latest_run: Option<serde_json::Value>,
    /// 依赖边计数：总数 / 已满足数（PA07 三谓词行级求值，PA07-IMP-003）。
    pub deps_total: i64,
    pub deps_met: i64,
    /// 任一关联产品处于 suspended/archived（执行门禁拒绝态）→「Blocked by Product Suspension」。
    pub product_blocked: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// PA-07 列表/单任务读面聚合列（0050 读面扩展）：关联产品 / 最新 run / 依赖满足
/// 计数 / 产品门禁态。调用方以 `t.planned_end_at, {TASK_AGGREGATE_COLUMNS},
/// t.created_at, ...` 拼接（`t` 为 delivery.task 别名）。依赖满足谓词与
/// task_dependency::dependencies_satisfied 逐条对齐（PA07-IMP-003，审查75/81）。
pub const TASK_AGGREGATE_COLUMNS: &str = "\
COALESCE((SELECT jsonb_agg(jsonb_build_object('id', p.id, 'name', p.name, 'status', p.status) \
             ORDER BY p.name) \
              FROM delivery.task_product tp \
              JOIN core.product p ON p.id = tp.product_id \
             WHERE tp.task_id = t.id AND tp.withdrawn_at IS NULL), \
          '[]'::jsonb) AS products, \
(SELECT jsonb_build_object('id', r.id, 'attempt_no', r.attempt_no, \
                           'status', r.status, 'review_verdict', r.review_verdict, \
                           'created_at', r.created_at) \
    FROM delivery.task_run r WHERE r.task_id = t.id \
   ORDER BY r.attempt_no DESC LIMIT 1) AS latest_run, \
(SELECT count(*) FROM delivery.task_dependency d \
  WHERE d.task_id = t.id AND d.tenant_id = t.tenant_id) AS deps_total, \
(SELECT count(*) FROM delivery.task_dependency d \
   JOIN delivery.task u ON u.project_id = d.project_id AND u.id = d.depends_on_task_id \
  WHERE d.task_id = t.id AND d.tenant_id = t.tenant_id \
    AND ((d.required_condition = 'completed' AND u.status = 'completed') \
      OR (d.required_condition = 'accepted' \
          AND u.status = 'completed' \
          AND EXISTS (SELECT 1 FROM governance.review_approval ra \
                       WHERE ra.task_id = u.id AND ra.status = 'approved')) \
      OR (d.required_condition = 'deliverable_approved' \
          AND u.status = 'completed' \
          AND EXISTS (SELECT 1 FROM governance.review_approval ra \
                       WHERE ra.task_id = u.id AND ra.status = 'approved') \
          AND EXISTS (SELECT 1 FROM delivery.task_revision tr \
                       WHERE tr.id = u.current_task_revision_id \
                         AND (tr.execution_contract->'deliverables') \
                             @> jsonb_build_array(d.required_deliverable_ref))))) AS deps_met, \
EXISTS (SELECT 1 FROM delivery.task_product tp \
         JOIN core.product p ON p.id = tp.product_id \
         WHERE tp.task_id = t.id AND tp.withdrawn_at IS NULL \
           AND p.status IN ('suspended', 'archived')) AS product_blocked";

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ApprovalRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub proposal_id: Uuid,
    pub status: String,
    pub required_role: String,
    pub decided_by: Option<Uuid>,
    pub decision_reason: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AuditLogRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub actor_id: Uuid,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RequirementRevisionRow {
    pub id: Uuid,
    pub requirement_id: Uuid,
    pub project_id: Uuid,
    pub version: i32,
    pub content: serde_json::Value,
    pub state: String,
    pub content_hash: String,
    pub created_by: Uuid,
    pub approved_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
