//! 关系草稿与端点解析（PC-I2；01B §6 + §13 跨域通道）。

use sqlx::PgPool;
use uuid::Uuid;

use kb_pc_contracts::registry::{parse_domain, DomainChannelState};

use crate::actor::AuditActor;

use super::{load_relationship_type, sha256_hex, type_set_allows, PcError};

/// 关系端点：PC 内对象（身份 + Revision 钉扎）或跨域四元组（§13）。
#[derive(Debug, Clone)]
pub enum PcEndpoint {
    Object {
        object_id: Uuid,
        revision_id: Option<Uuid>,
    },
    External {
        domain: String,
        target_type: String,
        target_id: String,
        revision_id: Option<String>,
    },
}

#[derive(Debug)]
struct ResolvedEndpoint {
    object_id: Option<Uuid>,
    object_type: Option<String>,
    object_revision_id: Option<Uuid>,
    ext_domain: Option<String>,
    ext_type: Option<String>,
    ext_id: Option<String>,
    ext_revision_id: Option<String>,
}

/// 跨域通道校验（§13.1 状态表 + §13.3 服务层组合键校验；V1 进程内共库，
/// 存在性探测直查外域表——服务化拆分后换 K3/PK1 只读端口，登记验收记录）。
async fn resolve_endpoint(
    exec: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    project_id: Uuid,
    endpoint: &PcEndpoint,
    side: &str,
) -> Result<ResolvedEndpoint, PcError> {
    match endpoint {
        PcEndpoint::Object {
            object_id,
            revision_id,
        } => {
            let row: Option<(String, Option<Uuid>, String)> = sqlx::query_as(
                "SELECT object_type, current_revision_id, lifecycle_status \
                   FROM pc.cognition_object \
                  WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(object_id)
            .fetch_optional(&mut *exec)
            .await?;
            let (object_type, current, lifecycle) =
                row.ok_or_else(|| PcError::NotFound(format!("{side} object not found")))?;
            if lifecycle == "deprecated" || lifecycle == "superseded" {
                return Err(PcError::Invalid(format!(
                    "{side} object is {lifecycle}; new relationships require an active object"
                )));
            }
            let revision = match revision_id {
                Some(r) => {
                    let ok: Option<Uuid> = sqlx::query_scalar(
                        "SELECT id FROM pc.cognition_revision \
                          WHERE tenant_id = $1 AND project_id = $2 AND object_id = $3 AND id = $4",
                    )
                    .bind(tenant_id)
                    .bind(project_id)
                    .bind(object_id)
                    .bind(r)
                    .fetch_optional(&mut *exec)
                    .await?;
                    Some(ok.ok_or_else(|| {
                        PcError::Invalid(format!("{side} revision does not belong to the object"))
                    })?)
                }
                None => Some(current.ok_or_else(|| {
                    PcError::Invalid(format!(
                        "{side} object has no revision yet (identity placeholder)"
                    ))
                })?),
            };
            Ok(ResolvedEndpoint {
                object_id: Some(*object_id),
                object_type: Some(object_type),
                object_revision_id: revision,
                ext_domain: None,
                ext_type: None,
                ext_id: None,
                ext_revision_id: None,
            })
        }
        PcEndpoint::External {
            domain,
            target_type,
            target_id,
            revision_id,
        } => {
            let parsed = parse_domain(domain)
                .map_err(|e| PcError::Invalid(format!("{side} endpoint domain: {e}")))?;
            match parsed.channel_state() {
                DomainChannelState::Open | DomainChannelState::ReadOnly => {}
                DomainChannelState::NotYetOpen => {
                    return Err(PcError::Invalid(format!(
                        "{side} endpoint domain `{domain}` channel is not open (design/adr/product_reality)"
                    )));
                }
                DomainChannelState::PermanentlyExcluded => {
                    return Err(PcError::Invalid(format!(
                        "{side} endpoint domain `{domain}` is permanently excluded (product_plan)"
                    )));
                }
            }
            // Revision 级引用纪律：requirement 必填（Q-PC-03）、knowledge_base 必填
            // （Q-PC-12 通道）；project_goal 允许空缺（§6.3 注 3）。
            let required = matches!(
                parsed,
                kb_pc_contracts::registry::TargetDomain::Requirement
                    | kb_pc_contracts::registry::TargetDomain::KnowledgeBase
            );
            if required && revision_id.is_none() {
                return Err(PcError::Invalid(format!(
                    "{side} endpoint domain `{domain}` requires a revision-level reference"
                )));
            }
            match parsed {
                kb_pc_contracts::registry::TargetDomain::Requirement => {
                    let rev = revision_id.as_deref().ok_or_else(|| {
                        PcError::Invalid("requirement endpoint requires revision".into())
                    })?;
                    let rev_uuid = Uuid::parse_str(rev).map_err(|_| {
                        PcError::Invalid("requirement revision id must be uuid".into())
                    })?;
                    let ok: Option<Uuid> = sqlx::query_scalar(
                        "SELECT rv.id FROM delivery.requirement_revision rv \
                           JOIN delivery.requirement r ON r.tenant_id = rv.tenant_id \
                                                      AND r.id = rv.requirement_id \
                          WHERE rv.tenant_id = $1 AND r.project_id = $2 AND rv.id = $3",
                    )
                    .bind(tenant_id)
                    .bind(project_id)
                    .bind(rev_uuid)
                    .fetch_optional(&mut *exec)
                    .await?;
                    if ok.is_none() {
                        return Err(PcError::NotFound(format!(
                            "{side} requirement revision not found in project"
                        )));
                    }
                }
                kb_pc_contracts::registry::TargetDomain::KnowledgeBase => {
                    let rev = revision_id.as_deref().ok_or_else(|| {
                        PcError::Invalid("knowledge_base endpoint requires revision".into())
                    })?;
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
                    .fetch_optional(&mut *exec)
                    .await?;
                    if ok.is_none() {
                        return Err(PcError::NotFound(format!(
                            "{side} kb document revision not found in project"
                        )));
                    }
                }
                // project_goal：01B §13.1 原文要求的「共享文档容器存在性探测」与
                // ADR-06a 守卫冲突（C-1：PC 禁止接入该容器，过渡期规约 §7.1-1）
                // ——守卫为准（用户裁决），本批仅通道校验，探测归后续 goal 域
                // 只读接口（偏差 j，验收记录同源）。
                // project_cognition（只读）/其余开放域：V1 仅通道校验，不做存在性探测
                _ => {}
            }
            Ok(ResolvedEndpoint {
                object_id: None,
                object_type: None,
                object_revision_id: None,
                ext_domain: Some(domain.clone()),
                ext_type: Some(target_type.clone()),
                ext_id: Some(target_id.clone()),
                ext_revision_id: revision_id.clone(),
            })
        }
    }
}

pub struct RelationshipDraftInput {
    pub relationship_type: String,
    pub source: PcEndpoint,
    pub target: PcEndpoint,
    /// 受控属性（relationship_type 声明范围内；服务层 V1 不做 schema 校验）。
    pub properties: serde_json::Value,
    pub change_summary: Option<String>,
}

/// 关系草稿：身份行 + 首 RR（端点语义快照自包含，§6.4）+ 指针 CAS。
/// 活跃身份唯一（uq_pc_relationship_identity）冲突 → Conflict。
pub async fn create_relationship_draft(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    input: RelationshipDraftInput,
) -> Result<(Uuid, Uuid), PcError> {
    let rel_type = load_relationship_type(pool, &input.relationship_type).await?;
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .map_err(|_| PcError::NotFound("project not found".into()))?;

    let mut tx = pool.begin().await?;
    let source =
        resolve_endpoint(tx.as_mut(), tenant_id, project_id, &input.source, "source").await?;
    let target =
        resolve_endpoint(tx.as_mut(), tenant_id, project_id, &input.target, "target").await?;

    // 端点类型合法性（§6.2 字典集合；跨域按 domain.type 形态）
    let src_key = source
        .object_type
        .clone()
        .or_else(|| {
            source
                .ext_domain
                .clone()
                .map(|d| format!("{d}.{}", source.ext_type.clone().unwrap_or_default()))
        })
        .ok_or_else(|| PcError::Invalid("source endpoint unresolved".into()))?;
    let tgt_key = target
        .object_type
        .clone()
        .or_else(|| {
            target
                .ext_domain
                .clone()
                .map(|d| format!("{d}.{}", target.ext_type.clone().unwrap_or_default()))
        })
        .ok_or_else(|| PcError::Invalid("target endpoint unresolved".into()))?;
    if !type_set_allows(&rel_type.source_object_types, &src_key) {
        return Err(PcError::Invalid(format!(
            "relationship `{}` does not allow source endpoint type `{src_key}`",
            rel_type.type_key
        )));
    }
    if !type_set_allows(&rel_type.target_object_types, &tgt_key) {
        return Err(PcError::Invalid(format!(
            "relationship `{}` does not allow target endpoint type `{tgt_key}`",
            rel_type.type_key
        )));
    }
    // 自引用：PC 内端点 = 同一对象身份；跨域端点 = 同一四元组定位
    let self_ref = (source.object_id.is_some() && source.object_id == target.object_id)
        || (source.ext_id.is_some()
            && source.ext_domain == target.ext_domain
            && source.ext_type == target.ext_type
            && source.ext_id == target.ext_id);
    if self_ref && !rel_type.allow_self_reference {
        return Err(PcError::Invalid(format!(
            "relationship `{}` forbids self reference",
            rel_type.type_key
        )));
    }

    // 活跃身份唯一预检（终态行不占位）
    let src_pin = endpoint_pin(
        source.object_id,
        source.ext_domain.as_deref(),
        source.ext_type.as_deref(),
        source.ext_id.as_deref(),
    );
    let tgt_pin = endpoint_pin(
        target.object_id,
        target.ext_domain.as_deref(),
        target.ext_type.as_deref(),
        target.ext_id.as_deref(),
    );
    let dup: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM pc.relationship \
          WHERE tenant_id = $1 AND project_id = $2 AND relationship_type_key = $3 \
            AND coalesce(source_object_id::text, source_ext_domain || ':' || source_ext_type || ':' || source_ext_id) = $4 \
            AND coalesce(target_object_id::text, target_ext_domain || ':' || target_ext_type || ':' || target_ext_id) = $5 \
            AND lifecycle_status IN ('identity','draft','active')",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&rel_type.type_key)
    .bind(&src_pin)
    .bind(&tgt_pin)
    .fetch_optional(&mut *tx)
    .await?;
    if dup.is_some() {
        return Err(PcError::Conflict(format!(
            "active relationship `{}` already exists for endpoints ({src_pin} → {tgt_pin})",
            rel_type.type_key
        )));
    }

    // RR content_hash = 规范序列化（type + directionality + 端点 + properties）
    let rr_canonical = format!(
        "rr:{}|{}|{}|{}|{}|{}|{}|{}",
        rel_type.type_key,
        rel_type.directionality,
        src_pin,
        source
            .object_revision_id
            .map(|r| r.to_string())
            .unwrap_or_default(),
        tgt_pin,
        target
            .object_revision_id
            .map(|r| r.to_string())
            .unwrap_or_default(),
        source.ext_revision_id.clone().unwrap_or_default(),
        serde_json::to_string(&input.properties).unwrap_or_default(),
    );
    let rr_hash = sha256_hex(&rr_canonical);

    let relationship_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.relationship \
         (tenant_id, project_id, relationship_type_key, lifecycle_status, \
          source_object_id, target_object_id, \
          source_ext_domain, source_ext_type, source_ext_id, source_ext_revision_id, \
          target_ext_domain, target_ext_type, target_ext_id, target_ext_revision_id, \
          created_by) \
         VALUES ($1, $2, $3, 'draft', $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) \
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&rel_type.type_key)
    .bind(source.object_id)
    .bind(target.object_id)
    .bind(source.ext_domain.clone())
    .bind(source.ext_type.clone())
    .bind(source.ext_id.clone())
    .bind(source.ext_revision_id.clone())
    .bind(target.ext_domain.clone())
    .bind(target.ext_type.clone())
    .bind(target.ext_id.clone())
    .bind(target.ext_revision_id.clone())
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;

    let rr_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.relationship_revision \
         (tenant_id, project_id, relationship_id, revision_no, relationship_type_key, \
          directionality, source_object_id, source_object_revision_id, \
          target_object_id, target_object_revision_id, \
          source_ext_domain, source_ext_type, source_ext_id, source_ext_revision_id, \
          target_ext_domain, target_ext_type, target_ext_id, target_ext_revision_id, \
          properties, change_summary, content_hash, created_by) \
         VALUES ($1, $2, $3, 1, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, \
                 $18, $19, $20, $21) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(relationship_id)
    .bind(&rel_type.type_key)
    .bind(&rel_type.directionality)
    .bind(source.object_id)
    .bind(source.object_revision_id)
    .bind(target.object_id)
    .bind(target.object_revision_id)
    .bind(source.ext_domain.clone())
    .bind(source.ext_type.clone())
    .bind(source.ext_id.clone())
    .bind(source.ext_revision_id.clone())
    .bind(target.ext_domain.clone())
    .bind(target.ext_type.clone())
    .bind(target.ext_id.clone())
    .bind(target.ext_revision_id.clone())
    .bind(&input.properties)
    .bind(&input.change_summary)
    .bind(&rr_hash)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE pc.relationship SET current_revision_id = $2, updated_at = now() WHERE id = $1",
    )
    .bind(relationship_id)
    .bind(rr_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok((relationship_id, rr_id))
}

/// 端点唯一身份钉（§6.3 uq_pc_relationship_identity 的函数式形态）：
/// PC 内端点 = object_id；跨域端点 = `domain:type:id`。
fn endpoint_pin(
    object_id: Option<Uuid>,
    ext_domain: Option<&str>,
    ext_type: Option<&str>,
    ext_id: Option<&str>,
) -> String {
    if let Some(o) = object_id {
        return o.to_string();
    }
    match (ext_domain, ext_type, ext_id) {
        (Some(d), Some(t), Some(i)) => format!("{d}:{t}:{i}"),
        _ => String::new(),
    }
}
