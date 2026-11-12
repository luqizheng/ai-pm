//! Project Baseline 读写 port（任务书批 B 第二域，自 pm-api `pm_baseline.rs`
//! 收口）。
//!
//! 域语义（S01-05，见 pm_baseline 模块头）：
//! - 提案从已确认知识 + 仓库快照派生；pending 提案唯一（重复创建 Conflict）；
//! - 条目逐项决策（confirm/reject/defer/investigate；已决策 409）；
//! - 确认要求无 proposed 条目（deferred/investigation 作为 documented
//!   uncertainties 保留）；已确认版本不可变。
//!
//! 授权（真人三重门 / 项目可见性）与动作词汇翻译是 HTTP 层职责，不进仓储。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 基线条目行（overview 的 items 与决策回读）。
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct BaselineItemRow {
    pub id: Uuid,
    pub category: String,
    pub content: String,
    pub evidence: serde_json::Value,
    pub confidence: String,
    pub state: String,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    pub decision_note: Option<String>,
}

/// 当前已确认版本头。
#[derive(Debug, Clone, Copy)]
pub struct BaselineCurrent {
    pub version: i32,
    pub proposal_id: Uuid,
    pub confirmed_by: Uuid,
    pub confirmed_at: DateTime<Utc>,
}

/// 待确认提案头。
#[derive(Debug, Clone)]
pub struct PendingProposal {
    pub id: Uuid,
    pub version: i32,
    pub status: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

/// 历史版本行。
#[derive(Debug, Clone, Copy)]
pub struct BaselineHistoryRow {
    pub version: i32,
    pub proposal_id: Uuid,
    pub confirmed_at: DateTime<Utc>,
}

/// 提案生成结果（create_proposal 响应体）。
#[derive(Debug)]
pub struct ProposalCreated {
    pub proposal_id: Uuid,
    pub version: i32,
    pub item_count: i64,
}

/// 条目决策结果。
#[derive(Debug)]
pub struct ItemDecision {
    pub item_id: Uuid,
    pub next_state: String,
}

/// 基线确认结果。
#[derive(Debug)]
pub struct BaselineConfirmed {
    pub version: i32,
}

/// 域级语义错误：仓储把 SQL 行为翻译成这些，HTTP 层只映射状态码。
#[derive(Debug)]
pub enum BaselineRepoError {
    Db(sqlx::Error),
    /// 唯一性/状态类冲突（pending 提案已存在、条目已决策、有未决条目）。
    Conflict(&'static str),
    /// 目标不存在（条目不存在或提案非 pending；无 pending 提案）。
    NotFound(&'static str),
}

impl From<sqlx::Error> for BaselineRepoError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

#[async_trait]
pub trait BaselineRepo: Send + Sync {
    async fn current_version(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<Option<BaselineCurrent>, BaselineRepoError>;

    async fn pending_proposal(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<Option<PendingProposal>, BaselineRepoError>;

    async fn proposal_items(
        &self,
        tenant_id: Uuid,
        proposal_id: Uuid,
    ) -> Result<Vec<BaselineItemRow>, BaselineRepoError>;

    async fn version_history(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<BaselineHistoryRow>, BaselineRepoError>;

    /// 从已确认知识 + 仓库快照派生并落提案与条目（单事务；先清 pending 冲突）。
    async fn create_proposal(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        created_by: Uuid,
    ) -> Result<ProposalCreated, BaselineRepoError>;

    /// 条目逐项决策（content 可选更正）；提案非 pending 或条目不存在 → NotFound。
    #[allow(clippy::too_many_arguments)] // 决策行字段齐写，列清单与 baseline_item 对应
    async fn decide_item(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        item_id: Uuid,
        next_state: &str,
        decided_by: Uuid,
        note: Option<&str>,
        content: Option<&str>,
    ) -> Result<ItemDecision, BaselineRepoError>;

    /// 确认最新 pending 提案：无 pending → NotFound；有未决条目 → Conflict；
    /// 成功落版本头 + 提案转 confirmed（单事务）。
    async fn confirm_pending(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        confirmed_by: Uuid,
    ) -> Result<BaselineConfirmed, BaselineRepoError>;
}
