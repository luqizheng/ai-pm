//! `KbServicePort` 的 V1 进程内实现（KP-3 落真 K4 / K5；其余端口维持未接线）。
//!
//! - **K4** `review_memory`：审核转达（CAS），落 [`super::memory::review`]；
//! - **K5** `list_sources`：来源只读查询（Platform / PC 共用，PK1 通道），落
//!   [`super::source`]；
//! - K1 / K2 / K3 / K6：`PortError::NotWired`（K2 由 REST 读面直查 0053 表，见
//!   `knowledge_services.rs`；K1/K3/K6 真实落点 Phase 4）。
//!
//! 调用方身份与项目作用域绑定在端口实例上（差异说明 D-KP3-3）：trait 方法签名不变，
//! 每请求经 [`KbServiceLive::for_caller`] 构造——Phase 7 HTTP transport 以请求头承载
//! 同一信息。K4 返回类型沿用骨架的 `KbAnalysisStatus`（差异说明 D-KP3-2）：
//! `status` = 审核结论，`memory_draft_refs[0]` = 被审修订四元组。

use kb_pc_contracts::ports::{
    Accepted, KbAnalysisRequest, KbAnalysisStatus, KbDocumentRevision, KbMemoryReviewRequest,
    KbServicePort, KbSupplyFulfilRequest, PortError, SourceSummary,
};
use kb_pc_contracts::{DomainRef, TargetDomain};
use sqlx::PgPool;
use uuid::Uuid;

use super::memory::{self, ReviewAction};
use super::source;
use super::{TYPE_MEMORY, TYPE_MEMORY_REVISION};
use crate::actor::AuditActor;

/// 端口调用方上下文（传输信封级：身份 + 租户 + 项目作用域）。
#[derive(Debug, Clone)]
pub struct KbCaller {
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    pub actor: AuditActor,
}

/// KB-Service 端口的进程内实现。
pub struct KbServiceLive {
    pool: PgPool,
    caller: KbCaller,
}

impl KbServiceLive {
    pub fn for_caller(pool: PgPool, caller: KbCaller) -> Self {
        Self { pool, caller }
    }
}

#[async_trait::async_trait]
impl KbServicePort for KbServiceLive {
    async fn submit_analysis(
        &self,
        _request: KbAnalysisRequest,
        _idempotency_key: &str,
    ) -> Result<Accepted, PortError> {
        Err(PortError::NotWired("KbServicePort::submit_analysis"))
    }

    async fn analysis_status(&self, _kb_analysis_id: Uuid) -> Result<KbAnalysisStatus, PortError> {
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
        memory_id: &str,
        request: KbMemoryReviewRequest,
    ) -> Result<KbAnalysisStatus, PortError> {
        // 非 UUID 的记忆 id 与不存在同口径（不泄露存在性）。
        let memory_id = Uuid::parse_str(memory_id)
            .map_err(|_| PortError::NotFound("memory not found".to_string()))?;
        let base = Uuid::parse_str(&request.base_memory_revision_id).map_err(|_| {
            PortError::Invalid("base_memory_revision_id must be a UUID".to_string())
        })?;
        let action = ReviewAction::parse(&request.action).ok_or_else(|| {
            PortError::Invalid("action must be one of approve | reject | revise".to_string())
        })?;
        let outcome = memory::review(
            &self.pool,
            self.caller.tenant_id,
            self.caller.project_id,
            memory_id,
            &self.caller.actor,
            action,
            request.note.as_deref(),
            base,
        )
        .await?;
        Ok(KbAnalysisStatus {
            status: outcome.review_status,
            source_id: None,
            document_ref: None,
            memory_draft_refs: vec![memory::revision_ref(outcome.revision_id)],
            failure: None,
        })
    }

    async fn list_sources(
        &self,
        project_id: Uuid,
        evidence_of: Option<&DomainRef>,
    ) -> Result<Vec<SourceSummary>, PortError> {
        if project_id != self.caller.project_id {
            return Err(PortError::Forbidden(
                "list_sources project outside the caller's scope".to_string(),
            ));
        }
        let rows = match evidence_of {
            None => source::list_project_sources(&self.pool, project_id).await?,
            Some(r) => {
                if r.target_domain != TargetDomain::KnowledgeBase.as_str()
                    || (r.target_type != TYPE_MEMORY && r.target_type != TYPE_MEMORY_REVISION)
                {
                    return Err(PortError::Invalid(
                        "evidence_of must reference knowledge_base memory | memory_revision"
                            .to_string(),
                    ));
                }
                let target = Uuid::parse_str(&r.target_id)
                    .map_err(|_| PortError::Invalid("evidence_of id must be a UUID".to_string()))?;
                source::sources_cited_by(&self.pool, project_id, &r.target_type, target).await?
            }
        };
        Ok(rows
            .into_iter()
            .map(|s| SourceSummary {
                source_id: s.source_id.to_string(),
                title: Some(s.title),
                content_hash: s.content_hash,
                evidence_of: evidence_of.cloned(),
            })
            .collect())
    }

    async fn fulfil_supply_request(
        &self,
        _request_id: &str,
        _request: KbSupplyFulfilRequest,
    ) -> Result<Accepted, PortError> {
        Err(PortError::NotWired("KbServicePort::fulfil_supply_request"))
    }
}
