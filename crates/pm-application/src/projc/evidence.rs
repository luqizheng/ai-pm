//! 来源与证据登记（projc.*；pc::evidence 同款机制）。
//!
//! DomainRef target 侧契约（R-07 V1.1）：`target_domain/target_type/target_id/
//! target_revision_id(+revision_missing_reason)`，与 `pc.relationship_type.owner_domain`
//! 的「关系确认权威域」语义分离（裁决稿 §3）。`project_cognition` 域的通道状态
//! 见 kb-pc-contracts::registry（V1 只读 → 随 T-2 评估开放）。

use sqlx::PgPool;
use uuid::Uuid;

use kb_pc_contracts::registry::{parse_domain, DomainChannelState};

use crate::actor::AuditActor;

use super::{project_tenant, ProjcError};

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

/// 来源登记：跨域通道状态校验 + SEC-P2-2 目标存在性/同租户同项目探测
/// （pc::register_source 同款口径；kb_document 收窄由库层 CHECK 兜底）。
pub async fn register_source(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    input: SourceInput,
) -> Result<Uuid, ProjcError> {
    let parsed = parse_domain(&input.target_domain)
        .map_err(|e| ProjcError::Invalid(format!("source domain: {e}")))?;
    match parsed.channel_state() {
        DomainChannelState::Open | DomainChannelState::ReadOnly => {}
        DomainChannelState::NotYetOpen => {
            return Err(ProjcError::Invalid(format!(
                "source domain `{}` channel is not open",
                input.target_domain
            )));
        }
        DomainChannelState::PermanentlyExcluded => {
            return Err(ProjcError::Invalid(format!(
                "source domain `{}` is permanently excluded",
                input.target_domain
            )));
        }
    }
    let tenant_id = project_tenant(pool, project_id).await?;

    // SEC-P2-2：按 source_kind 对目标做「对象存在性 + 同 project / 同租户」校验。
    // `manual_input` / `external_reference` / `repo_analysis` 的 target_id 是
    // 外部或生产者自述标识，不做对象探测。
    if let Some(table) =
        crate::reference_targets::source_kind_target(&input.source_kind, &input.target_type)
    {
        let target_uuid = Uuid::parse_str(&input.target_id).map_err(|_| {
            ProjcError::Invalid(format!(
                "source target_id must be a uuid for `{}` source_kind",
                input.source_kind
            ))
        })?;
        let probe =
            crate::reference_targets::probe(pool, table, target_uuid, tenant_id, Some(project_id))
                .await?;
        if !probe.in_scope {
            return Err(ProjcError::NotFound(format!(
                "source target `{}` not found in this project/tenant (SEC-P2-2)",
                input.target_id
            )));
        }
        // kb_document 必须钉一份属于该文档、且在本项目内的 KB 修订
        if input.source_kind == "kb_document" {
            let rev = input.target_revision_id.as_deref().ok_or_else(|| {
                ProjcError::Invalid("kb_document source must pin a KB revision".into())
            })?;
            let rev_uuid = Uuid::parse_str(rev)
                .map_err(|_| ProjcError::Invalid("kb revision id must be uuid".into()))?;
            let matched: Option<Uuid> = sqlx::query_scalar(
                "SELECT dr.id FROM kb.document_revision dr \
                   JOIN kb.document d ON d.tenant_id = dr.tenant_id AND d.id = dr.document_id \
                  WHERE dr.tenant_id = $1 AND d.project_id = $2 \
                    AND dr.document_id = $3 AND dr.id = $4",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(target_uuid)
            .bind(rev_uuid)
            .fetch_optional(pool)
            .await?;
            if matched.is_none() {
                return Err(ProjcError::NotFound(
                    "source kb revision not found in this project (SEC-P2-2)".into(),
                ));
            }
        }
    }

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.source \
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

/// 证据主体（Stage 1 仅对象修订；关系修订主体随 relationship 表 T-2 落地）。
#[derive(Debug, Clone)]
pub enum EvidenceSubject {
    ObjectRevision(Uuid),
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

/// 证据登记：主体归属 + 类型判别器（库层 CHECK 兜底）；
/// document_section 的 KB 引用组合键即时校验（pc R-13 同款）。
pub async fn register_evidence(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    input: EvidenceInput,
) -> Result<Uuid, ProjcError> {
    let tenant_id = project_tenant(pool, project_id).await?;
    let object_revision_id = match &input.subject {
        EvidenceSubject::ObjectRevision(id) => {
            let ok: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM projc.cognition_revision \
                  WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(id)
            .fetch_optional(pool)
            .await?;
            if ok.is_none() {
                return Err(ProjcError::NotFound("object revision not found".into()));
            }
            *id
        }
    };
    // document_section 的 KB 引用组合键即时校验（写入时点；存续期对账另批）
    if input.evidence_type == "document_section" {
        if let (Some(domain), Some(rev)) = (&input.target_domain, &input.target_revision_id) {
            if domain == "knowledge_base" {
                let rev_uuid = Uuid::parse_str(rev)
                    .map_err(|_| ProjcError::Invalid("kb revision id must be uuid".into()))?;
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
                    return Err(ProjcError::NotFound(
                        "evidence kb revision not found".into(),
                    ));
                }
            }
        }
    }
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.evidence \
         (tenant_id, project_id, source_id, object_revision_id, relationship_revision_id, \
          evidence_type, target_domain, target_type, target_id, target_revision_id, \
          file_path, symbol, code_range, commit_sha, section_anchor, start_line, end_line, \
          excerpt, description, evidence_hash, confidence, created_by) \
         VALUES ($1, $2, $3, $4, NULL, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, \
                 $17, $18, $19, $20, $21) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(input.source_id)
    .bind(object_revision_id)
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
