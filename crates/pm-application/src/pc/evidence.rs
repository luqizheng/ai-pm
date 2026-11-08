//! 来源与证据登记（PC-D-05；01B §9）。

use sqlx::PgPool;
use uuid::Uuid;

use kb_pc_contracts::registry::{parse_domain, DomainChannelState};

use crate::actor::AuditActor;

use super::PcError;

pub struct SourceInput {
    pub source_kind: String,
    pub target_domain: String,
    pub target_type: String,
    pub target_id: String,
    pub target_revision_id: Option<String>,
    pub revision_missing_reason: Option<String>,
    pub section_anchor: Option<String>,
    pub start_line: Option<i32>,
    pub locator: Option<String>,
    pub description: Option<String>,
    pub source_content_hash: Option<String>,
}

/// 来源登记（§9.2）：跨域通道状态校验 + kb_document 收窄由库层 CHECK 兜底。
pub async fn register_source(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    input: SourceInput,
) -> Result<Uuid, PcError> {
    let parsed = parse_domain(&input.target_domain)
        .map_err(|e| PcError::Invalid(format!("source domain: {e}")))?;
    match parsed.channel_state() {
        DomainChannelState::Open | DomainChannelState::ReadOnly => {}
        DomainChannelState::NotYetOpen => {
            return Err(PcError::Invalid(format!(
                "source domain `{}` channel is not open", input.target_domain
            )));
        }
        DomainChannelState::PermanentlyExcluded => {
            return Err(PcError::Invalid(format!(
                "source domain `{}` is permanently excluded", input.target_domain
            )));
        }
    }
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .map_err(|_| PcError::NotFound("project not found".into()))?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.source \
         (tenant_id, project_id, source_kind, target_domain, target_type, target_id, \
          target_revision_id, revision_missing_reason, section_anchor, start_line, \
          locator, description, source_content_hash, collected_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&input.source_kind)
    .bind(&input.target_domain)
    .bind(&input.target_type)
    .bind(&input.target_id)
    .bind(&input.target_revision_id)
    .bind(&input.revision_missing_reason)
    .bind(&input.section_anchor)
    .bind(input.start_line)
    .bind(&input.locator)
    .bind(&input.description)
    .bind(&input.source_content_hash)
    .bind(actor.principal_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

#[derive(Debug, Clone)]
pub enum EvidenceSubject {
    ObjectRevision(Uuid),
    RelationshipRevision(Uuid),
}

pub struct EvidenceInput {
    pub source_id: Uuid,
    pub subject: EvidenceSubject,
    pub evidence_type: String,
    pub target_domain: Option<String>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub target_revision_id: Option<String>,
    pub file_path: Option<String>,
    pub symbol: Option<String>,
    pub code_range: Option<String>,
    pub commit_sha: Option<String>,
    pub section_anchor: Option<String>,
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
    pub excerpt: Option<String>,
    pub description: Option<String>,
    pub evidence_hash: String,
    pub confidence: Option<String>,
}

/// 证据登记（§9.3）：主体二选一 + 类型判别器（库层 CHECK 兜底）。
/// document_section 的跨域完整性由服务层组合键探测 + 周期对账承担（R-13）。
pub async fn register_evidence(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    input: EvidenceInput,
) -> Result<Uuid, PcError> {
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .map_err(|_| PcError::NotFound("project not found".into()))?;
    let (object_revision_id, relationship_revision_id) = match &input.subject {
        EvidenceSubject::ObjectRevision(id) => {
            let ok: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM pc.cognition_revision \
                  WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(id)
            .fetch_optional(pool)
            .await?;
            if ok.is_none() {
                return Err(PcError::NotFound("object revision not found".into()));
            }
            (Some(*id), None)
        }
        EvidenceSubject::RelationshipRevision(id) => {
            let ok: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM pc.relationship_revision \
                  WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(id)
            .fetch_optional(pool)
            .await?;
            if ok.is_none() {
                return Err(PcError::NotFound("relationship revision not found".into()));
            }
            (None, Some(*id))
        }
    };
    // document_section 的 KB 引用组合键即时校验（写入时点；存续期对账另批）
    if input.evidence_type == "document_section" {
        if let (Some(domain), Some(rev)) = (&input.target_domain, &input.target_revision_id) {
            if domain == "knowledge_base" {
                let rev_uuid = Uuid::parse_str(rev)
                    .map_err(|_| PcError::Invalid("kb revision id must be uuid".into()))?;
                let ok: Option<Uuid> = sqlx::query_scalar(
                    "SELECT dr.id FROM kb.document_revision dr \
                       JOIN kb.document d ON d.tenant_id = dr.tenant_id \
                                         AND d.id = dr.document_id \
                      WHERE dr.tenant_id = $1 AND d.project_id = $2 AND dr.id = $3",
                )
                .bind(tenant_id)
                .bind(project_id)
                .bind(rev_uuid)
                .fetch_optional(pool)
                .await?;
                if ok.is_none() {
                    return Err(PcError::NotFound("evidence kb revision not found".into()));
                }
            }
        }
    }
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.evidence \
         (tenant_id, project_id, source_id, object_revision_id, relationship_revision_id, \
          evidence_type, target_domain, target_type, target_id, target_revision_id, \
          file_path, symbol, code_range, commit_sha, section_anchor, start_line, end_line, \
          excerpt, description, evidence_hash, confidence, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, \
                 $18, $19, $20, $21, $22) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(input.source_id)
    .bind(object_revision_id)
    .bind(relationship_revision_id)
    .bind(&input.evidence_type)
    .bind(&input.target_domain)
    .bind(&input.target_type)
    .bind(&input.target_id)
    .bind(&input.target_revision_id)
    .bind(&input.file_path)
    .bind(&input.symbol)
    .bind(&input.code_range)
    .bind(&input.commit_sha)
    .bind(&input.section_anchor)
    .bind(input.start_line)
    .bind(input.end_line)
    .bind(&input.excerpt)
    .bind(&input.description)
    .bind(&input.evidence_hash)
    .bind(input.confidence.unwrap_or_else(|| "medium".into()))
    .bind(actor.principal_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}
