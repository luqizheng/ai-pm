//! AI-PM Cognition 读写 port（任务书批 B 第三域，自 pm-api `pm_cognition.rs`
//! 收口）。
//!
//! 域语义（S01-04，见 pm_cognition 模块头）：认知 Run / 认知条目 / 项目知识
//! 三实体的建立与逐项确认；V1 派生是确定性的（结构化事实，无 LLM）——全部
//! 条目成立 → run completed。逐项确认（confirm）落项目知识（Edit & Confirm
//! 修订正文优先；重复确认 409）。
//!
//! 授权（成员可见性）与动作词汇翻译是 HTTP 层职责；派生条目的文案组装
//! （项目身份/仓库元数据）同样留 HTTP 层——它是产品文案不是数据访问。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

/// 认知 Run 行（overview 近序列表）。
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct CognitionRunRow {
    pub id: Uuid,
    pub scope: Value,
    pub item_count: i32,
    pub confirmed_count: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// 认知条目行。
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct CognitionItemRow {
    pub id: Uuid,
    pub domain: String,
    pub content: String,
    pub evidence: Value,
    pub confidence: String,
    pub has_conflict: bool,
    pub state: String,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    pub decision_note: Option<String>,
}

/// 已确认项目知识行。
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct KnowledgeRow {
    pub id: Uuid,
    pub domain: String,
    pub content: String,
    pub version: i32,
    pub source_item_id: Option<Uuid>,
    pub confirmed_by: Uuid,
    pub confirmed_at: DateTime<Utc>,
}

/// start_run 结果（run 头 + 条目计数）。
#[derive(Debug)]
pub struct RunStarted {
    pub run_id: Uuid,
    pub item_count: i32,
}

/// 逐项决策结果。
#[derive(Debug)]
pub struct CognitionDecision {
    pub knowledge_id: Option<Uuid>,
}

/// 域级语义错误：仓储把 SQL 行为翻译成这些，HTTP 层只映射状态码。
#[derive(Debug)]
pub enum CognitionRepoError {
    Db(sqlx::Error),
    Conflict(&'static str),
    NotFound(&'static str),
}

impl From<sqlx::Error> for CognitionRepoError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

/// 派生条目（start_run 内逐条落库的最小写入面）。
#[derive(Debug, Clone)]
pub struct DerivedItem<'a> {
    pub domain: &'a str,
    pub content: &'a str,
    pub evidence: &'a Value,
    pub confidence: &'a str,
}

/// 派生管线的输入行：项目身份 + 仓库元数据。
#[derive(Debug)]
pub struct DerivationInput {
    pub project_name: String,
    pub project_description: Option<String>,
    pub repositories: Vec<(Uuid, String, String, Option<String>, String)>,
}

/// 派生输入读取错误（项目不存在）。
#[derive(Debug)]
pub enum DerivationError {
    Db(sqlx::Error),
    NotFound,
}

impl From<sqlx::Error> for DerivationError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

#[async_trait]
pub trait CognitionRepo: Send + Sync {
    // ── 读 ──
    /// 派生管线输入：项目身份 + 仓库元数据（start_run 的文案原料）。
    async fn derivation_input(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<DerivationInput, DerivationError>;

    async fn recent_runs(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<CognitionRunRow>, CognitionRepoError>;

    async fn active_knowledge_count(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<i64, CognitionRepoError>;

    /// 非取消 run 中仍处 proposed 的条目数（overview 的「待确认」角标）。
    async fn proposed_item_count(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<i64, CognitionRepoError>;

    async fn run_items(
        &self,
        tenant_id: Uuid,
        run_id: Uuid,
    ) -> Result<Vec<CognitionItemRow>, CognitionRepoError>;

    async fn active_knowledge(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<KnowledgeRow>, CognitionRepoError>;

    // ── 写 ──
    /// 启动 Run：落 run 头（pending）+ 逐条派生条目 + 置 completed（单事务）。
    /// 项目不存在 → NotFound。
    #[allow(clippy::too_many_arguments)] // run 头 + 派生上下文一次传入
    async fn start_run(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        started_by: Uuid,
        scope: &Value,
        items: &[DerivedItem<'_>],
        repo_versions: &Value,
    ) -> Result<RunStarted, CognitionRepoError>;

    /// 逐项决策；confirm 时落项目知识（Edit & Confirm 修订正文优先）并回填
    /// run.confirmed_count（单事务）。条目不存在 → NotFound；已决策或已产出
    /// 知识 → Conflict。
    async fn decide_item(
        &self,
        tenant_id: Uuid,
        item_id: Uuid,
        next_state: &str,
        decided_by: Uuid,
        note: Option<&str>,
        edited_content: Option<&str>,
    ) -> Result<CognitionDecision, CognitionRepoError>;
}
