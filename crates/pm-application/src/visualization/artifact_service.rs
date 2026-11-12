//! Visualization Artifact 生命周期与审批流程（V0-2）。
//!
//! 依据 13 号数据模型设计 §4/§6：状态机分离（artifact / approval 两台），
//! 乐观并发双锚（target_hash + expected_version_no），零业务写路径。
//!
//! ## 状态机（库级触发器兜底，本模块是入口）
//!
//! - `draft → published`：审批通过（或免审批槽位直接发布）；
//! - `published → stale`：stale 检测命中（不自动重渲染）；
//! - `draft → rejected`：审批拒绝（reason 必填）；
//! - **published 不可回退、spec 不可改写；stale 是终态**（触发器强制）。
//!
//! ## 错误语义（对齐仓库惯例）
//!
//! - 不存在 / 跨属主 → `NotFound`（404 同形，不泄露存在性）；
//! - 哈希漂移 / pending 冲突 / 终态重审 → `Conflict`（409）；
//! - 校验失败 → `Validation`（400）。

use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::audit::append_audit;

/// Artifact 记录（服务层视图）。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ArtifactRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    pub slot_key: String,
    pub version_no: i32,
    pub state: String,
    pub nature: String,
    pub spec: serde_json::Value,
    pub spec_hash: String,
    pub source_hash: String,
    pub renderer_id: Option<String>,
    pub renderer_version: Option<String>,
    pub created_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 服务层错误。
#[derive(Debug, thiserror::Error)]
pub enum VizError {
    /// 不存在或跨属主（404 同形，不泄露存在性）。
    #[error("visualization artifact not found")]
    NotFound,
    /// 乐观并发冲突（哈希漂移 / pending 撞单 / 终态重审）。
    #[error("visualization conflict: {0}")]
    Conflict(String),
    /// 输入校验失败。
    #[error("visualization validation failed: {0}")]
    Validation(String),
    /// 数据库错误。
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// 唯一约束冲突判定（对齐 `pm_write::support`）。
fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

/// 创建 draft 的输入（收敛参数——后续加字段不再扩散签名）。
pub struct CreateDraftInput<'a> {
    pub slot_key: &'a str,
    pub nature: &'a str,
    pub spec: &'a serde_json::Value,
    pub spec_hash: &'a str,
    pub source_hash: &'a str,
    pub provenance: &'a serde_json::Value,
}

/// 插入 draft Artifact（校验已由上游校验器完成；此处只做入库与审计）。
///
/// 版本号按槽位递增（重新生成 = 新版本，不覆盖历史——ADR-VIZ-006）。
pub async fn create_draft(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    input: CreateDraftInput<'_>,
) -> Result<ArtifactRecord, VizError> {
    let CreateDraftInput {
        slot_key,
        nature,
        spec,
        spec_hash,
        source_hash,
        provenance,
    } = input;
    if slot_key.trim().is_empty() {
        return Err(VizError::Validation("slot_key 不得为空".to_string()));
    }
    let mut tx = pool.begin().await?;

    // 项目归属校验（同一租户内；非成员/跨租户一律 404 同形）
    let tenant_id: Uuid = sqlx::query_scalar(
        "SELECT tenant_id FROM core.project WHERE id = $1 AND status <> 'deleted'",
    )
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(VizError::NotFound)?;

    let next_version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(max(version_no), 0) + 1 FROM viz.artifact \
          WHERE project_id = $1 AND slot_key = $2",
    )
    .bind(project_id)
    .bind(slot_key)
    .fetch_one(&mut *tx)
    .await?;

    let record: ArtifactRecord = sqlx::query_as(
        "INSERT INTO viz.artifact \
             (tenant_id, project_id, slot_key, version_no, state, nature, \
              spec, spec_hash, source_hash, provenance, created_by) \
          VALUES ($1, $2, $3, $4, 'draft', $5, $6, $7, $8, $9, $10) \
          RETURNING id, tenant_id, project_id, slot_key, version_no, state, nature, \
                    spec, spec_hash, source_hash, renderer_id, renderer_version, \
                    created_by, created_at",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(slot_key)
    .bind(next_version)
    .bind(nature)
    .bind(spec)
    .bind(spec_hash)
    .bind(source_hash)
    .bind(provenance)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            VizError::Conflict("并发提交同槽位新版本，请重试".to_string())
        } else {
            VizError::Db(e)
        }
    })?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "viz.artifact.created",
        "viz_artifact",
        Some(record.id),
    )
    .await?;

    tx.commit().await?;
    Ok(record)
}

/// 读取 Artifact（跨属主 404 同形）。
pub async fn get_artifact(pool: &PgPool, artifact_id: Uuid) -> Result<ArtifactRecord, VizError> {
    sqlx::query_as(
        "SELECT id, tenant_id, project_id, slot_key, version_no, state, nature, \
                spec, spec_hash, source_hash, renderer_id, renderer_version, \
                created_by, created_at \
           FROM viz.artifact WHERE id = $1",
    )
    .bind(artifact_id)
    .fetch_optional(pool)
    .await?
    .ok_or(VizError::NotFound)
}

/// 请求发布审批（draft → pending）。
///
/// 并发：同槽位第二条 pending 被部分唯一索引拒绝 → 409。
pub async fn request_approval(
    pool: &PgPool,
    artifact_id: Uuid,
    actor: &AuditActor,
) -> Result<Uuid, VizError> {
    let mut tx = pool.begin().await?;

    let artifact: (Uuid, Uuid, String, String, String, i32) = sqlx::query_as(
        "SELECT tenant_id, project_id, slot_key, nature, spec_hash, version_no \
           FROM viz.artifact WHERE id = $1",
    )
    .bind(artifact_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(VizError::NotFound)?;

    let (tenant_id, project_id, slot_key, nature, spec_hash, version_no) = artifact;

    // 解释图不可进入发布流程（DV-13；库级 CHECK 兜底，此处提前给可读错误）
    if nature == "explanatory" {
        return Err(VizError::Validation(
            "解释图（AI 生成 · 非正式认知）不得发布到业务槽位".to_string(),
        ));
    }

    // 只有 draft 可请求发布
    let state: String = sqlx::query_scalar("SELECT state FROM viz.artifact WHERE id = $1")
        .bind(artifact_id)
        .fetch_one(&mut *tx)
        .await?;
    if state != "draft" {
        return Err(VizError::Conflict(format!(
            "仅 draft 可请求发布（当前 {state}）"
        )));
    }

    let approval_id: Uuid = sqlx::query_scalar(
        "INSERT INTO viz.approval \
             (tenant_id, project_id, artifact_id, slot_key, target_hash, \
              expected_version_no, requested_by) \
          VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(artifact_id)
    .bind(slot_key)
    .bind(spec_hash)
    .bind(version_no)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            VizError::Conflict("该槽位已有待审发布请求".to_string())
        } else {
            VizError::Db(e)
        }
    })?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "viz.approval.requested",
        "viz_approval",
        Some(approval_id),
    )
    .await?;

    tx.commit().await?;
    Ok(approval_id)
}

/// 裁决审批（approve / reject / cancel）。
///
/// 乐观并发：target_hash 漂移（spec 被改过）或版本号漂移 → 409。
/// approve 同事务把 artifact 置 published。
pub async fn decide_approval(
    pool: &PgPool,
    approval_id: Uuid,
    actor: &AuditActor,
    action: &str,
    reason: Option<&str>,
) -> Result<(), VizError> {
    if action != "approve" && action != "reject" && action != "cancel" {
        return Err(VizError::Validation(format!("非法裁决动作：{action}")));
    }
    if action == "reject" && reason.map(str::trim).unwrap_or("").is_empty() {
        return Err(VizError::Validation("拒绝必须带理由".to_string()));
    }

    let mut tx = pool.begin().await?;

    // 行锁读取（并发裁决全序化）
    let approval: (Uuid, Uuid, Uuid, String, String, i32, String) = sqlx::query_as(
        "SELECT tenant_id, project_id, artifact_id, slot_key, target_hash, \
                expected_version_no, status \
           FROM viz.approval WHERE id = $1 FOR UPDATE",
    )
    .bind(approval_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(VizError::NotFound)?;

    let (_tenant_id, project_id, artifact_id, _slot_key, target_hash, expected_version, status) =
        approval;

    if status != "pending" {
        return Err(VizError::Conflict("该审批已裁决".to_string()));
    }

    // 哈希 / 版本漂移检测（与被审 artifact 当前值比对）
    let current: (String, i32) = sqlx::query_as(
        "SELECT spec_hash, version_no FROM viz.artifact WHERE id = $1",
    )
    .bind(artifact_id)
    .fetch_one(&mut *tx)
    .await?;
    if current.0 != target_hash {
        return Err(VizError::Conflict(
            "artifact 已在审批请求后变更（哈希漂移），请重新发起".to_string(),
        ));
    }
    if current.1 != expected_version {
        return Err(VizError::Conflict("artifact 版本号已漂移，请重新发起".to_string()));
    }

    let new_status = match action {
        "approve" => "approved",
        "reject" => "rejected",
        _ => "cancelled",
    };

    // 库级触发器会拒绝终态再改与锚点变更；这里显式写入裁决列
    sqlx::query(
        "UPDATE viz.approval SET status = $2, decided_by = $3, decided_at = now(), \
                decision_reason = $4 WHERE id = $1 AND status = 'pending'",
    )
    .bind(approval_id)
    .bind(new_status)
    .bind(actor.principal_id)
    .bind(reason)
    .execute(&mut *tx)
    .await?;

    // reject 同事务把 artifact 置 rejected（draft → rejected；13 号 §4.1）
    if action == "reject" {
        sqlx::query("UPDATE viz.artifact SET state = 'rejected' WHERE id = $1 AND state = 'draft'")
            .bind(artifact_id)
            .execute(&mut *tx)
            .await?;
    }

    // approve 同事务发布（库级触发器校验 nature / 不可变）
    if action == "approve" {
        let result = sqlx::query(
            "UPDATE viz.artifact SET state = 'published', published_by = $2, published_at = now() \
              WHERE id = $1 AND state = 'draft'",
        )
        .bind(artifact_id)
        .bind(actor.principal_id)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(VizError::Conflict(
                "artifact 不在 draft 状态，无法发布".to_string(),
            ));
        }
    }

    append_audit(
        &mut tx,
        project_id,
        actor,
        "viz.approval.decided",
        "viz_approval",
        Some(approval_id),
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

/// 标记 stale（source_hash 失配时由 stale 检测调用）。
///
/// 只对 published 生效；不自动重渲染（ADR-VIZ-006）。
pub async fn mark_stale(pool: &PgPool, artifact_id: Uuid, actor: &AuditActor) -> Result<(), VizError> {
    let mut tx = pool.begin().await?;

    let project_id: Uuid = sqlx::query_scalar(
        "SELECT project_id FROM viz.artifact WHERE id = $1",
    )
    .bind(artifact_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(VizError::NotFound)?;
    let state: String = sqlx::query_scalar("SELECT state FROM viz.artifact WHERE id = $1")
        .bind(artifact_id)
        .fetch_one(&mut *tx)
        .await?;
    if state != "published" {
        return Err(VizError::Conflict("仅 published 可标记 stale".to_string()));
    }

    sqlx::query("UPDATE viz.artifact SET state = 'stale' WHERE id = $1")
        .bind(artifact_id)
        .execute(&mut *tx)
        .await?;

    append_audit(&mut tx, project_id, actor, "viz.artifact.stale", "viz_artifact", Some(artifact_id))
        .await?;

    tx.commit().await?;
    Ok(())
}
