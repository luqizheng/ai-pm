//! `/v1/pm/*` 人工写服务（Slice A）：需求修订的起草/定稿与任务的创建/编辑。
//!
//! 与命令通道（`command_executor`，走提案/红线治理）不同，这些是人在 UI 上的
//! 直接操作：本地事务落地 + `governance.audit_log` 行（动作名对齐前端审计页
//! 词汇）。操作者由调用方显式传入（T-021a：HTTP handler 取 `AuthedPrincipal`），
//! 应用层不再自行猜测 actor。

#![allow(unused_imports)]

pub(crate) use serde::Deserialize;
pub(crate) use serde::Serialize;
pub(crate) use serde_json::json;
pub(crate) use sha2::{Digest, Sha256};
pub(crate) use sqlx::PgPool;
pub(crate) use uuid::Uuid;

pub(crate) use crate::actor::AuditActor;
pub(crate) use crate::rows::{RequirementRevisionRow, TaskRow};

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

pub(crate) mod audit;
mod revision;
mod support;
mod task;
pub(crate) mod task_revision;

pub use audit::*;
pub use revision::*;
pub use support::*;
pub use task::*;

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
    /// 钉版修订号（S05-02，迁移 0014）：NULL=取该文档最新 published 修订
    /// （默认，不落后）；非空=人工确认时钉定的版本（可复现）。
    #[serde(default)]
    pub pinned_revision_no: Option<i32>,
}

#[derive(Debug)]
pub enum PmWriteError {
    NotFound(&'static str),
    Conflict(&'static str),
    Validation(&'static str),
    /// 有身份但无权限（S02-I1：AI 主体提交确认类命令 / 非 Lead 治理命令，
    /// S02D-02 #2 + §7.4 职责表）。
    Forbidden(&'static str),
    /// 动态校验消息（S04 计划草案校验：带任务键定位的 400 文案）。
    ValidationMsg(String),
    Db(sqlx::Error),
}

impl From<sqlx::Error> for PmWriteError {
    fn from(e: sqlx::Error) -> Self {
        PmWriteError::Db(e)
    }
}

// PA-05 接线：统一事项服务错误并入 pm_write 错误面（Validation 保持静态
// 文案口径；NotFound 挂统一事项语义）。
impl From<crate::work_item::WorkItemError> for PmWriteError {
    fn from(err: crate::work_item::WorkItemError) -> Self {
        match err {
            crate::work_item::WorkItemError::Db(e) => PmWriteError::Db(e),
            crate::work_item::WorkItemError::NotFound => {
                PmWriteError::NotFound("work item not found")
            }
            crate::work_item::WorkItemError::Validation(m) => PmWriteError::Validation(m),
        }
    }
}

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
    /// D-1 遗留①（V2 D-2=A 前置）：Task 创建时的 AI Role 版本锚（稿 §17.2
    /// 「Task 保留创建时版本引用」）。可空 = 历史 Task 允许无 Role 锚；非空时
    /// 校验该版本所属项目行必须指向本任务的目标 project（跨项目 → Validation）。
    #[serde(default)]
    pub ai_role_version_id: Option<Uuid>,
    /// S03→S04 交接（总体设计 §6.3）：Task 钉定不可变实施基线（SC-08）。
    /// 非空时校验：基线存在、属于目标项目、状态 frozen（superseded 拒绝）。
    #[serde(default)]
    pub implementation_baseline_id: Option<Uuid>,
    /// DK-D（设计稿 §7.2/E4）：创建时即落验收口径，与任务行**同事务**。
    /// 提供即按 `task_acceptance_criterion` 逐条写入（ordinal 缺省按 `max+10`）。
    /// 放在 create 而不是再调一次 `update_task`：任务从诞生起就必须可验收，
    /// 且避免"建了任务但还没验收口径"的中间态（AI 生成的任务尤其如此）。
    #[serde(default)]
    pub acceptance_criteria: Option<Vec<AcceptanceCriterionInput>>,
}

/// 整批 `draft → ready` 请求（DK-0 §7.4 / D-DK-8：整批预览 + 一次确认）。
#[derive(Debug, Deserialize)]
pub struct MarkTasksReadyRequest {
    pub task_ids: Vec<Uuid>,
}

/// 整批确认结果（E5 的读数面：`ready.len()` 即本批 TaskReady 事件数）。
#[derive(Debug, Default, Serialize)]
pub struct MarkTasksReadyOutcome {
    /// 本批真正迁到 `ready` 的任务（每条一个 `TaskReady` 事件）。
    pub ready: Vec<Uuid>,
    /// 已是 `ready` 的任务（幂等 no-op，不重复发事件）。
    pub already_ready: Vec<Uuid>,
    /// 非 draft/ready（执行中/终态）而不接受整批确认的任务。
    pub skipped: Vec<Uuid>,
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
