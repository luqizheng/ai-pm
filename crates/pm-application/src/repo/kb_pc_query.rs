//! KB/PC 知识服务查询 port（任务书批 B 域 7，自 pm-api `knowledge_services.rs`
//! 的直查 SQL 收口）。
//!
//! 域语义（KP-1/KP-2/checkpoint-1，见 knowledge_services 模块头）：KB/PC 分析
//! 任务状态展开（intake_task + result_ref 分派）、PC 对象投影与列表。
//! 端口未接线的桩端点（PortNotWired）不走本仓储。
//!
//! 投影 JSON 的组装（pc_object_json 等）是响应契约，留 HTTP 层。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

/// KB/PC 分析任务行（intake_task ⋈ intake_record）。
#[derive(Debug, Clone)]
pub struct AnalysisTaskRow {
    pub task_status: String,
    pub result_ref: Option<Value>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub intake_id: Uuid,
    pub intake_type: String,
    pub title: String,
}

/// KB 文档修订行（document ⋈ document_revision）。
#[derive(Debug, Clone)]
pub struct KbRevisionRow {
    pub rev_id: Uuid,
    pub doc_id: Uuid,
    pub doc_key: String,
    pub doc_type: String,
    pub revision_no: i32,
    pub revision_status: String,
    pub source_commit_sha: Option<String>,
    pub content_hash: String,
    pub change_summary: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// PC 对象投影行（对象 + 最新修订 + 正文摘要 + 是否已入正式版本）。
pub type PcObjectRow = (
    Uuid,
    String,
    String,
    String,
    String,
    Option<i32>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<bool>,
);

/// PC 提案头（P3 PROPOSED 结论展开）。
#[derive(Debug, Clone)]
pub struct PcProposalHead {
    pub source_id: Option<Uuid>,
    pub status: String,
    pub governance_level: String,
}

/// PC 对象列表行（管理面列表投影）。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct PcObjectListRow {
    pub id: Uuid,
    pub object_type: String,
    pub object_key: Option<String>,
    pub display_name: String,
    pub lifecycle_status: String,
    pub updated_at: DateTime<Utc>,
    pub revision_no: Option<i32>,
    pub change_summary: Option<String>,
    pub excerpt: Option<String>,
}

/// PC 提案列表行。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct PcProposalListRow {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub risk_level: String,
    pub confidence: String,
    pub origin_kind: String,
    pub expected_version_no: i64,
    pub member_count: i64,
    pub submitted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// 对象 → 来源分析报告追溯行（version_member → cognition_version → proposal →
/// repo_analysis_report 反查；report 侧 LEFT JOIN——非报告通道发布的版本
/// report 字段为 NULL）。pc/projc 两域共用形状。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ObjectLineageRow {
    pub report_id: Option<Uuid>,
    pub repository_name: Option<String>,
    pub commit_sha: Option<String>,
    pub version_no: i64,
    pub confirmed_at: DateTime<Utc>,
}

#[async_trait]
pub trait KbPcQueryRepo: Send + Sync {
    /// KB/PC 分析任务行（`target_service`/`task_type` 区分 kb_analyze / pc_analyze）。
    async fn analysis_task(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        target_service: &str,
        task_type: &str,
    ) -> Result<Option<AnalysisTaskRow>, sqlx::Error>;

    /// KB 文档修订行（按 revision id + 项目边界）。
    async fn kb_revision(
        &self,
        project_id: Uuid,
        revision_id: Uuid,
    ) -> Result<Option<KbRevisionRow>, sqlx::Error>;

    /// KB 修订正文。
    async fn kb_revision_content(
        &self,
        revision_id: Uuid,
    ) -> Result<Option<String>, sqlx::Error>;

    /// PC 提案头（P3 PROPOSED 展开）。
    async fn pc_proposal_head(
        &self,
        project_id: Uuid,
        proposal_id: Uuid,
    ) -> Result<Option<PcProposalHead>, sqlx::Error>;

    /// P3 来源展开：证据钉扎的对象集 + 各对象最新修订。
    async fn pc_objects_of_source(
        &self,
        project_id: Uuid,
        source_id: Uuid,
    ) -> Result<Vec<PcObjectRow>, sqlx::Error>;

    /// P3 legacy：直查单对象 + 当前修订（checkpoint-1 旧任务形状）。
    async fn pc_object_legacy(
        &self,
        project_id: Uuid,
        object_id: Uuid,
    ) -> Result<Vec<PcObjectRow>, sqlx::Error>;

    async fn list_pc_objects(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<PcObjectListRow>, sqlx::Error>;

    async fn list_pc_proposals(
        &self,
        project_id: Uuid,
        status_filter: Option<&str>,
    ) -> Result<Vec<PcProposalListRow>, sqlx::Error>;

    /// ProjC 对象列表（管理面列表投影；形状与 [`Self::list_pc_objects`] 一致）。
    async fn list_projc_objects(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<PcObjectListRow>, sqlx::Error>;

    /// ProjC 对象详情（最新修订投影；形状与 [`Self::pc_object_legacy`] 一致；
    /// versioned 经 version_member→published version 内联判定——projc 域无
    /// v_revision_versioned 视图，0025 移交 P3-10）。
    async fn projc_object_legacy(
        &self,
        project_id: Uuid,
        object_id: Uuid,
    ) -> Result<Vec<PcObjectRow>, sqlx::Error>;

    /// PC 对象 → 来源分析报告追溯（最近 3 条已发布版本）。
    async fn pc_object_lineage(
        &self,
        project_id: Uuid,
        object_id: Uuid,
    ) -> Result<Vec<ObjectLineageRow>, sqlx::Error>;

    /// ProjC 对象 → 来源分析报告追溯（最近 3 条已发布版本）。
    async fn projc_object_lineage(
        &self,
        project_id: Uuid,
        object_id: Uuid,
    ) -> Result<Vec<ObjectLineageRow>, sqlx::Error>;
}
