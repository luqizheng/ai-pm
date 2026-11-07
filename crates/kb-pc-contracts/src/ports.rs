//! 端口 trait（V1.1 §12 API 契约的类型化承载）。
//!
//! V1 形态 = 进程内端口调用（ADR-10 方案 A）；Phase 7 后同一 trait 换 HTTP
//! transport，签名不变。本文件只承载**契约类型与 trait**：
//!
//! - `KbServicePort`：Platform → KB（K1~K6）。
//! - `PcServicePort`：Platform → PC（P1~P7）。
//! - PC → KB 的只读查询（PK1）复用 `KbServicePort::document_revision` /
//!   `list_sources`（§12.4 保留点对点）。
//! - KB → PC 全部移除（§12.5）——不提供任何 trait 方法。
//!
//! V1 业务落点归 Phase 4（KB 独立化）/ Phase 5（PC-01C），本批次提供
//! [`PortNotWired`] 占位实现：任何调用返回 [`PortError::NotWired`]，让路由
//! 骨架可编译可测试（受理语义 202 由 Platform 侧 Intake 端点承担，不经端口）。

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain_ref::DomainRef;

/// 端口调用错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PortError {
    /// V1 占位：业务实现未接线（Phase 4/5 落点）。
    #[error("port not wired: {0} (V1 in-process placeholder; real landing = Phase 4/5)")]
    NotWired(&'static str),
    #[error("resource not found: {0}")]
    NotFound(String),
    #[error("caller lacks permission for the requested action: {0}")]
    Forbidden(String),
    #[error("invalid request: {0}")]
    Invalid(String),
}

/// 纯受理响应（D-3：202 仅代表「请求已登记、任务已创建」）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Accepted {
    pub task_id: Uuid,
    pub intake_id: Uuid,
    pub status: String,
}

impl Accepted {
    pub fn pending(task_id: Uuid, intake_id: Uuid) -> Self {
        Self {
            task_id,
            intake_id,
            status: "PENDING".to_string(),
        }
    }
}

/// 原始文件引用（§7.4 Source Reference；`content_hash` 为跨服务对账锚）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceReference {
    pub kind: String,
    /// 二选一：预签名 URI 或内联引用（文本类先例 source_input.content）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_ref: Option<String>,
    pub content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
}

// ── K1~K6：Platform → KB ────────────────────────────────────────────────────

/// K1 请求：提交资料分析。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KbAnalysisRequest {
    pub intake_ref: DomainRef,
    pub intake_type: String,
    pub source_ref: SourceReference,
    pub content_hash: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
}

/// K2 响应：分析状态查询。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KbAnalysisStatus {
    pub status: String,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub document_ref: Option<DomainRef>,
    #[serde(default)]
    pub memory_draft_refs: Vec<DomainRef>,
    #[serde(default)]
    pub failure: Option<String>,
}

/// K3 响应：文档修订元数据。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KbDocumentRevision {
    pub document_id: String,
    pub revision_id: String,
    pub git_commit_sha: String,
    pub content_hash: String,
    pub index_state: String,
    pub created_at: Option<DateTime<Utc>>,
}

/// K4 请求：记忆审核转达。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KbMemoryReviewRequest {
    pub action: String,
    #[serde(default)]
    pub note: Option<String>,
    pub base_memory_revision_id: String,
}

/// K5 来源摘要（Platform/PC 共用只读）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SourceSummary {
    pub source_id: String,
    pub title: Option<String>,
    pub content_hash: String,
    #[serde(default)]
    pub evidence_of: Option<DomainRef>,
}

/// K6 请求：资料补充任务执行转达。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KbSupplyFulfilRequest {
    pub result_ref: DomainRef,
}

/// KB-Service 端口（K1~K6；PK1 只读复用 K3/K5）。
#[async_trait::async_trait]
pub trait KbServicePort: Send + Sync {
    /// K1 提交资料分析（幂等 `kb_analyze:{intake_id}`）。
    async fn submit_analysis(
        &self,
        request: KbAnalysisRequest,
        idempotency_key: &str,
    ) -> Result<Accepted, PortError>;
    /// K2 查询分析状态。
    async fn analysis_status(&self, kb_analysis_id: Uuid)
        -> Result<KbAnalysisStatus, PortError>;
    /// K3 查询文档修订元数据（PC→KB 只读 PK1 共用）。
    async fn document_revision(
        &self,
        document_id: &str,
        revision_id: &str,
    ) -> Result<KbDocumentRevision, PortError>;
    /// K4 审核转达（CAS）。
    async fn review_memory(
        &self,
        memory_id: &str,
        request: KbMemoryReviewRequest,
    ) -> Result<KbAnalysisStatus, PortError>;
    /// K5 来源查询（Platform/PC 共用只读）。
    async fn list_sources(
        &self,
        project_id: Uuid,
        evidence_of: Option<&DomainRef>,
    ) -> Result<Vec<SourceSummary>, PortError>;
    /// K6 资料补充执行转达（幂等 `kb_supply:{request_id}`）。
    async fn fulfil_supply_request(
        &self,
        request_id: &str,
        request: KbSupplyFulfilRequest,
    ) -> Result<Accepted, PortError>;
}

// ── P1~P7：Platform → PC ────────────────────────────────────────────────────

/// KB 认知候选信号（§10.2 信号契约；payload 最小字段）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CognitionSignal {
    pub relevance: String,
    pub confidence: String,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub subject_refs: Vec<DomainRef>,
    #[serde(default)]
    pub evidence_refs: Vec<DomainRef>,
}

/// P1 请求：提交认知分析任务（纯受理）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PcAnalysisRequest {
    pub intake_ref: DomainRef,
    pub task_ref: DomainRef,
    pub analysis_kind: String,
    #[serde(default)]
    pub signal: Option<CognitionSignal>,
    #[serde(default)]
    pub content_refs: Vec<DomainRef>,
}

/// P2 请求：用户明确产品决策受理（纯受理；proposal 引用经 P4/事件获取）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PcDecisionRequest {
    pub intake_ref: DomainRef,
    pub task_ref: DomainRef,
    /// 决策摘要（不内联正文；四元组引用 + 摘要 ≤256 + hash）。
    pub decision: serde_json::Value,
    pub proposed_by: String,
}

/// P3 响应：分析状态/结论。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PcAnalysisStatus {
    pub status: String,
    /// PROPOSED / NO_CHANGE / CONFLICT。
    #[serde(default)]
    pub conclusion: Option<String>,
    #[serde(default)]
    pub proposal_ref: Option<DomainRef>,
    #[serde(default)]
    pub conflict_ref: Option<DomainRef>,
}

/// P4 响应：Proposal 详情。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PcProposal {
    pub proposal_id: String,
    pub status: String,
    pub governance_level: String,
    #[serde(default)]
    pub members: Vec<serde_json::Value>,
    pub content_hash: String,
    #[serde(default)]
    pub approval_ref: Option<DomainRef>,
}

/// P5 请求：审批转达（CAS）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PcProposalDecisionRequest {
    pub action: String,
    #[serde(default)]
    pub note: Option<String>,
    pub expected_version_no: i64,
}

/// P5 响应。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PcProposalDecision {
    pub proposal_id: String,
    pub status: String,
    #[serde(default)]
    pub published_version_ref: Option<DomainRef>,
}

/// P6 响应：冲突详情。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PcConflict {
    pub conflict_id: String,
    pub status: String,
    #[serde(default)]
    pub frozen: Vec<serde_json::Value>,
}

/// P7 响应：版本引用。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PcVersion {
    pub version_id: String,
    pub version_no: i64,
    pub snapshot_hash: String,
    #[serde(default)]
    pub predecessor_version_id: Option<String>,
}

/// PC-Service 端口（P1~P7）。
#[async_trait::async_trait]
pub trait PcServicePort: Send + Sync {
    /// P1 提交认知分析任务受理（幂等 `pc_analyze:{intake_id}`）。
    async fn submit_analysis(
        &self,
        request: PcAnalysisRequest,
        idempotency_key: &str,
    ) -> Result<Accepted, PortError>;
    /// P2 提交产品决策受理（幂等 `pc_decide:{intake_id}`；纯受理语义）。
    async fn submit_decision(
        &self,
        request: PcDecisionRequest,
        idempotency_key: &str,
    ) -> Result<Accepted, PortError>;
    /// P3 分析状态/结论。
    async fn analysis_status(
        &self,
        pc_analysis_id: Uuid,
    ) -> Result<PcAnalysisStatus, PortError>;
    /// P4 Proposal 详情。
    async fn proposal(&self, proposal_id: &str) -> Result<PcProposal, PortError>;
    /// P5 审批转达（CAS）。
    async fn decide_proposal(
        &self,
        proposal_id: &str,
        request: PcProposalDecisionRequest,
    ) -> Result<PcProposalDecision, PortError>;
    /// P6 冲突详情。
    async fn conflict(&self, conflict_id: &str) -> Result<PcConflict, PortError>;
    /// P7 版本引用。
    async fn version(&self, version_id: &str) -> Result<PcVersion, PortError>;
}

// ── V1 占位实现 ─────────────────────────────────────────────────────────────

/// V1 进程内占位：全部方法返回 [`PortError::NotWired`]。
///
/// 受理类入口的实际业务在 Phase 3 已由 Platform Intake 端点承担（同事务
/// 三写 + outbox），端口接线（消费侧分析本体）归 Phase 4/5。
pub struct PortNotWired;

#[async_trait::async_trait]
impl KbServicePort for PortNotWired {
    async fn submit_analysis(
        &self,
        _request: KbAnalysisRequest,
        _idempotency_key: &str,
    ) -> Result<Accepted, PortError> {
        Err(PortError::NotWired("KbServicePort::submit_analysis"))
    }
    async fn analysis_status(
        &self,
        _kb_analysis_id: Uuid,
    ) -> Result<KbAnalysisStatus, PortError> {
        Err(PortError::NotWired("KbServicePort::analysis_status"))
    }
    async fn document_revision(
        &self,
        _document_id: &str,
        _revision_id: &str,
    ) -> Result<KbDocumentRevision, PortError> {
        Err(PortError::NotWired("KbServicePort::document_revision"))
    }
    async fn review_memory(
        &self,
        _memory_id: &str,
        _request: KbMemoryReviewRequest,
    ) -> Result<KbAnalysisStatus, PortError> {
        Err(PortError::NotWired("KbServicePort::review_memory"))
    }
    async fn list_sources(
        &self,
        _project_id: Uuid,
        _evidence_of: Option<&DomainRef>,
    ) -> Result<Vec<SourceSummary>, PortError> {
        Err(PortError::NotWired("KbServicePort::list_sources"))
    }
    async fn fulfil_supply_request(
        &self,
        _request_id: &str,
        _request: KbSupplyFulfilRequest,
    ) -> Result<Accepted, PortError> {
        Err(PortError::NotWired("KbServicePort::fulfil_supply_request"))
    }
}

#[async_trait::async_trait]
impl PcServicePort for PortNotWired {
    async fn submit_analysis(
        &self,
        _request: PcAnalysisRequest,
        _idempotency_key: &str,
    ) -> Result<Accepted, PortError> {
        Err(PortError::NotWired("PcServicePort::submit_analysis"))
    }
    async fn submit_decision(
        &self,
        _request: PcDecisionRequest,
        _idempotency_key: &str,
    ) -> Result<Accepted, PortError> {
        Err(PortError::NotWired("PcServicePort::submit_decision"))
    }
    async fn analysis_status(
        &self,
        _pc_analysis_id: Uuid,
    ) -> Result<PcAnalysisStatus, PortError> {
        Err(PortError::NotWired("PcServicePort::analysis_status"))
    }
    async fn proposal(&self, _proposal_id: &str) -> Result<PcProposal, PortError> {
        Err(PortError::NotWired("PcServicePort::proposal"))
    }
    async fn decide_proposal(
        &self,
        _proposal_id: &str,
        _request: PcProposalDecisionRequest,
    ) -> Result<PcProposalDecision, PortError> {
        Err(PortError::NotWired("PcServicePort::decide_proposal"))
    }
    async fn conflict(&self, _conflict_id: &str) -> Result<PcConflict, PortError> {
        Err(PortError::NotWired("PcServicePort::conflict"))
    }
    async fn version(&self, _version_id: &str) -> Result<PcVersion, PortError> {
        Err(PortError::NotWired("PcServicePort::version"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn placeholder_ports_are_fail_closed() {
        let port = PortNotWired;
        let err = port
            .proposal("p1")
            .await
            .unwrap_err();
        assert!(matches!(err, PortError::NotWired(_)));
        assert!(err.to_string().contains("Phase 4/5"));
    }

    #[test]
    fn accepted_is_pure_receipt() {
        let a = Accepted::pending(Uuid::new_v4(), Uuid::new_v4());
        assert_eq!(a.status, "PENDING");
    }
}
