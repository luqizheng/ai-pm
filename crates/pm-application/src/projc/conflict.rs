//! ProjC 冲突登记与人工处置（R-06：ProjC 自建 `projc.conflict`；pc.conflict =
//! Legacy / Not for New Use，本文件不触碰 pc.*）。
//!
//! 表结构 = pc.conflict 0001:3108 同构（0026 建表）；L3 冻结判定 =
//! [`super::frozen_by_conflict`]（未解决冲突经 conflict_object 圈定的对象/关系，
//! submit / approve / 发布三处同口径）。
//!
//! 门禁（fail-closed）：create / resolve 均为人工治理动作 = 真人主体 + A3
//! （ensure_formal_authorized；AI/服务主体恒 403）。
//! 状态机（库层 CHECK 兜底）：
//! `detected/under_review/awaiting_human → resolved/dismissed`；
//! `resolved/dismissed → reopened`（重新冻结）。
//! 审计口径：`projc_conflict.create` / `projc_conflict.resolve`，source_kind/
//! target_type 全部 projc_ 前缀。

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::append_audit_with_after;

use super::{ensure_formal_authorized, project_tenant, ProjcError};

const CONFLICT_TYPES: [&str; 8] = [
    "content_contradiction",
    "rule_contradiction",
    "scope_conflict",
    "relationship_conflict",
    "source_conflict",
    "revision_conflict",
    "reality_mismatch",
    "cross_domain_authority",
];
const SEVERITIES: [&str; 3] = ["critical", "major", "minor"];

/// 冲突登记入参（conflict_object 挂冲突双方：对象 / 关系至少其一）。
pub struct ConflictInput {
    /// [`CONFLICT_TYPES`] 之一。
    pub conflict_type: String,
    /// [`SEVERITIES`] 之一。
    pub severity: String,
    pub description: String,
    /// 检测来源描述（人工登记 / 分析发现等，可空）。
    pub detection_source: Option<String>,
    /// 被冲突圈定的 ProjC 对象身份。
    pub objects: Vec<Uuid>,
    /// 被冲突圈定的 ProjC 关系身份。
    pub relationships: Vec<Uuid>,
}

/// 处置动作：resolve（已解决）/ dismiss（不成立关闭）/ reopen（重开，重新冻结）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveAction {
    Resolve,
    Dismiss,
    Reopen,
}

impl ResolveAction {
    fn status(self) -> &'static str {
        match self {
            Self::Resolve => "resolved",
            Self::Dismiss => "dismissed",
            Self::Reopen => "reopened",
        }
    }
}

pub struct ResolveInput {
    pub action: ResolveAction,
    pub resolution_note: Option<String>,
}

/// 冲突登记（人工）：真人 + A3；登记即冻结——conflict_object 圈定的对象/关系
/// 对 submit / approve / 发布 403（L3），直至 resolved/dismissed 解冻。
pub async fn create_conflict(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    input: ConflictInput,
) -> Result<Uuid, ProjcError> {
    let tenant_id = project_tenant(pool, project_id).await?;
    ensure_formal_authorized(pool, project_id, actor).await?;

    if !CONFLICT_TYPES.contains(&input.conflict_type.as_str()) {
        return Err(ProjcError::Invalid(format!(
            "unknown conflict_type `{}`",
            input.conflict_type
        )));
    }
    if !SEVERITIES.contains(&input.severity.as_str()) {
        return Err(ProjcError::Invalid(format!(
            "unknown severity `{}`",
            input.severity
        )));
    }
    if input.description.trim().is_empty() {
        return Err(ProjcError::Invalid("description is required".into()));
    }
    if input.objects.is_empty() && input.relationships.is_empty() {
        return Err(ProjcError::Invalid(
            "a conflict needs at least one subject (object or relationship)".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    // 主体归属校验（对象/关系必须在本项目内；终态对象仍可被圈定——冲突处置
    // 正是对存量认知的争议登记）
    for object_id in &input.objects {
        let ok: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM projc.cognition_object \
              WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(object_id)
        .fetch_optional(&mut *tx)
        .await?;
        if ok.is_none() {
            return Err(ProjcError::NotFound(format!(
                "object {object_id} not found in project"
            )));
        }
    }
    for relationship_id in &input.relationships {
        let ok: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM projc.relationship \
              WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(relationship_id)
        .fetch_optional(&mut *tx)
        .await?;
        if ok.is_none() {
            return Err(ProjcError::NotFound(format!(
                "relationship {relationship_id} not found in project"
            )));
        }
    }

    let conflict_id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.conflict \
         (tenant_id, project_id, conflict_type, severity, description, detection_source, \
          status, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, 'detected', $7) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&input.conflict_type)
    .bind(&input.severity)
    .bind(input.description.trim())
    .bind(&input.detection_source)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;
    for object_id in &input.objects {
        sqlx::query(
            "INSERT INTO projc.conflict_object \
             (tenant_id, project_id, conflict_id, object_id) VALUES ($1, $2, $3, $4)",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(conflict_id)
        .bind(object_id)
        .execute(&mut *tx)
        .await?;
    }
    for relationship_id in &input.relationships {
        sqlx::query(
            "INSERT INTO projc.conflict_object \
             (tenant_id, project_id, conflict_id, relationship_id) VALUES ($1, $2, $3, $4)",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(conflict_id)
        .bind(relationship_id)
        .execute(&mut *tx)
        .await?;
    }
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "projc_conflict.create",
        "projc_conflict",
        Some(conflict_id),
        json!({
            "status": "detected",
            "conflict_type": input.conflict_type,
            "severity": input.severity,
            "objects": input.objects,
            "relationships": input.relationships,
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(conflict_id)
}

/// 冲突处置（人工）：真人 + A3。resolve/dismiss 解冻（终态需 resolution 落痕，
/// 库层 CHECK 强制 resolved_by/resolved_at）；reopen 解冻态重开（重新冻结）。
pub async fn resolve_conflict(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    conflict_id: Uuid,
    input: ResolveInput,
) -> Result<String, ProjcError> {
    let tenant_id = project_tenant(pool, project_id).await?;
    ensure_formal_authorized(pool, project_id, actor).await?;

    let mut tx = pool.begin().await?;
    let (status,): (String,) = sqlx::query_as(
        "SELECT status FROM projc.conflict \
          WHERE tenant_id = $1 AND project_id = $2 AND id = $3 FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(conflict_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ProjcError::NotFound("conflict not found".into()))?;
    let new_status = input.action.status();
    let legal = match input.action {
        ResolveAction::Resolve | ResolveAction::Dismiss => {
            !matches!(status.as_str(), "resolved" | "dismissed")
        }
        ResolveAction::Reopen => matches!(status.as_str(), "resolved" | "dismissed"),
    };
    if !legal {
        return Err(ProjcError::Conflict(format!(
            "conflict is `{status}`; {new_status} is not a legal transition"
        )));
    }
    let note = input
        .resolution_note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string);
    if matches!(
        input.action,
        ResolveAction::Resolve | ResolveAction::Dismiss
    ) && note.is_none()
    {
        return Err(ProjcError::Invalid(
            "resolution note is required for resolve/dismiss".into(),
        ));
    }
    sqlx::query(
        "UPDATE projc.conflict \
            SET status = $2, \
                resolution_note = COALESCE($3, resolution_note), \
                resolved_by = CASE WHEN $4 THEN $5 ELSE NULL END, \
                resolved_at = CASE WHEN $4 THEN now() ELSE NULL END, \
                updated_at = now() \
          WHERE id = $1",
    )
    .bind(conflict_id)
    .bind(new_status)
    .bind(&note)
    .bind(matches!(
        input.action,
        ResolveAction::Resolve | ResolveAction::Dismiss
    ))
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "projc_conflict.resolve",
        "projc_conflict",
        Some(conflict_id),
        json!({
            "status": new_status,
            "previous_status": status,
            "resolution_note": note,
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(new_status.to_string())
}
