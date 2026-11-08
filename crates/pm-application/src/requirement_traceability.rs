//! S02-I3 · Source & Traceability（docs/s02-i3-source-traceability-implementation-v1.md）。
//!
//! 契约（S02-04 V1.2 §9/§10/§5.4/§7.4；Q-4/Q-18/Q-20/S02D-15/19 已裁可冻结）：
//! - `requirement_finding`（R5）：Finding↔Requirement 多对多，relation 单值域
//!   `supports`（§9.2-2）；不复制 Finding 内容进正文。
//! - `source.source_input` + `requirement_source_input`（Q-18 选项 A）：来源材料
//!   为输入侧实体；来源关联属 **Requirement 级、修订继承**（S02D-19）。
//! - 职责（§7.4）：Human Member/Lead/Approver 可登记/关联；AI-PM 仅建议——
//!   登记与关联命令 `require_human`（AI 建议走提案池既有路径，不直接落关联）。
//! - Decision 治理关联 = `ai.change_proposal.target_type='requirement'`（§10.3，
//!   列无 CHECK 零迁移；decisions 读模型已通用 join——本切片零代码，仅登记语义）。
//! - `repository_change_intake` 候选延后 I7（本切片裁决：无消费方不建空表）。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::{append_audit, PmWriteError};
use crate::requirement::require_human;

/// §5.4 来源五类（R-13/C-1）；`intake` 类随 S02-I7 启用（枚举先占位）。
pub const INPUT_KINDS: [&str; 5] = [
    "human_input",
    "discovery",
    "external_feedback",
    "intake",
    "import",
];

#[derive(Debug, Deserialize)]
pub struct RegisterSourceInputRequest {
    pub input_kind: String,
    pub title: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub source_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LinkSourceInputRequest {
    pub input_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct LinkFindingRequest {
    pub finding_id: Uuid,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct SourceInputEntry {
    pub id: Uuid,
    pub project_id: Uuid,
    pub input_kind: String,
    pub title: String,
    pub content: Option<String>,
    pub source_ref: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct LinkEntry {
    pub requirement_id: Uuid,
    pub linked_id: Uuid,
    pub relation: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

/// 命令前置：真人主体（§7.4）+ 需求可见且 `lifecycle_status='active'`。
/// 成员闸由 REST 适配层 `require_requirement_visible` 把门（I1 既有分层）。
/// 在调用方事务内执行（与后续写入同事务，避免校验/写入竞态）。
async fn require_active_requirement(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    requirement_id: Uuid,
) -> Result<Uuid, PmWriteError> {
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT project_id, lifecycle_status FROM delivery.requirement \
         WHERE id = $1 AND tenant_id = $2",
    )
    .bind(requirement_id)
    .bind(tenant_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(PmWriteError::Db)?;
    let (project_id, lifecycle) = row.ok_or(PmWriteError::NotFound("requirement not found"))?;
    if lifecycle != "active" {
        return Err(PmWriteError::Conflict(
            "requirement is not active (withdrawn/archived)",
        ));
    }
    Ok(project_id)
}

// ────────────────────────────────────────────────── 来源材料（source_input）

pub async fn register_source_input(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    req: RegisterSourceInputRequest,
    actor: &AuditActor,
) -> Result<SourceInputEntry, PmWriteError> {
    require_human(pool, actor).await?;
    let kind = req.input_kind.trim();
    if !INPUT_KINDS.contains(&kind) {
        return Err(PmWriteError::Validation("invalid input_kind"));
    }
    let title = req.title.trim().to_string();
    if title.is_empty() {
        return Err(PmWriteError::Validation("title is required"));
    }
    let exists: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM core.project WHERE id = $1 AND tenant_id = $2")
            .bind(project_id)
            .bind(tenant_id)
            .fetch_optional(pool)
            .await
            .map_err(PmWriteError::Db)?;
    if exists.is_none() {
        return Err(PmWriteError::NotFound("unknown project"));
    }

    let mut tx = pool.begin().await?;
    let input_id = Uuid::new_v4();
    let entry = sqlx::query_as::<_, SourceInputEntry>(
        "INSERT INTO source.source_input \
         (id, tenant_id, project_id, input_kind, title, content, source_ref, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         RETURNING id, project_id, input_kind, title, content, source_ref, created_by, created_at",
    )
    .bind(input_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(kind)
    .bind(&title)
    .bind(
        req.content
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty()),
    )
    .bind(
        req.source_ref
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "source.registered",
        "source_input",
        Some(input_id),
    )
    .await?;
    tx.commit().await?;
    Ok(entry)
}

pub async fn link_source_input(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    input_id: Uuid,
    actor: &AuditActor,
) -> Result<LinkEntry, PmWriteError> {
    require_human(pool, actor).await?;
    let mut tx = pool.begin().await?;
    let project_id = require_active_requirement(&mut tx, tenant_id, requirement_id).await?;
    let source_project: Option<Uuid> = sqlx::query_scalar(
        "SELECT project_id FROM source.source_input WHERE id = $1 AND tenant_id = $2",
    )
    .bind(input_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    // 跨项目来源按不存在处理（与 negative_4 同语义：不泄露存在性）。
    if source_project != Some(project_id) {
        return Err(PmWriteError::NotFound("source input not found"));
    }
    let entry = sqlx::query_as::<_, LinkEntry>(
        "INSERT INTO delivery.requirement_source_input \
         (requirement_id, input_id, tenant_id, project_id, created_by) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (requirement_id, input_id) DO NOTHING \
         RETURNING requirement_id, input_id AS linked_id, relation, created_by, created_at",
    )
    .bind(requirement_id)
    .bind(input_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(actor.principal_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    let entry = entry.ok_or(PmWriteError::Conflict("source input already linked"))?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.source_linked",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;
    Ok(entry)
}

pub async fn unlink_source_input(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    input_id: Uuid,
    actor: &AuditActor,
) -> Result<(), PmWriteError> {
    require_human(pool, actor).await?;
    let mut tx = pool.begin().await?;
    let project_id = require_active_requirement(&mut tx, tenant_id, requirement_id).await?;
    let deleted = sqlx::query(
        "DELETE FROM delivery.requirement_source_input \
         WHERE requirement_id = $1 AND input_id = $2",
    )
    .bind(requirement_id)
    .bind(input_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if deleted.rows_affected() == 0 {
        return Err(PmWriteError::NotFound("source input link not found"));
    }
    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.source_unlinked",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn list_requirement_sources(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
) -> Result<serde_json::Value, PmWriteError> {
    let items = sqlx::query_as::<_, SourceInputEntry>(
        "SELECT si.id, si.project_id, si.input_kind, si.title, si.content, si.source_ref, \
                si.created_by, si.created_at \
         FROM delivery.requirement_source_input rsi \
         JOIN source.source_input si ON si.project_id = rsi.project_id AND si.id = rsi.input_id \
         JOIN delivery.requirement r ON r.project_id = rsi.project_id AND r.id = rsi.requirement_id \
         WHERE rsi.requirement_id = $1 AND r.tenant_id = $2 \
         ORDER BY si.created_at DESC",
    )
    .bind(requirement_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    Ok(serde_json::json!({ "items": items }))
}

// ────────────────────────────────────────────────── Finding 支撑（§9）

pub async fn link_finding(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    finding_id: Uuid,
    actor: &AuditActor,
) -> Result<LinkEntry, PmWriteError> {
    require_human(pool, actor).await?;
    let mut tx = pool.begin().await?;
    let project_id = require_active_requirement(&mut tx, tenant_id, requirement_id).await?;
    // Finding 必须同项目（discovery 复合键 tenant+project+id；跨项目按不存在处理）。
    let finding: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM discovery.discovery_finding \
         WHERE id = $1 AND tenant_id = $2 AND project_id = $3",
    )
    .bind(finding_id)
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if finding.is_none() {
        return Err(PmWriteError::NotFound("finding not found in this project"));
    }
    let entry = sqlx::query_as::<_, LinkEntry>(
        "INSERT INTO delivery.requirement_finding \
         (requirement_id, finding_id, tenant_id, project_id, created_by) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (requirement_id, finding_id) DO NOTHING \
         RETURNING requirement_id, finding_id AS linked_id, relation, created_by, created_at",
    )
    .bind(requirement_id)
    .bind(finding_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(actor.principal_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    let entry = entry.ok_or(PmWriteError::Conflict("finding already linked"))?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.finding_linked",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;
    Ok(entry)
}

pub async fn unlink_finding(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    finding_id: Uuid,
    actor: &AuditActor,
) -> Result<(), PmWriteError> {
    require_human(pool, actor).await?;
    let mut tx = pool.begin().await?;
    let project_id = require_active_requirement(&mut tx, tenant_id, requirement_id).await?;
    let deleted = sqlx::query(
        "DELETE FROM delivery.requirement_finding WHERE requirement_id = $1 AND finding_id = $2",
    )
    .bind(requirement_id)
    .bind(finding_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if deleted.rows_affected() == 0 {
        return Err(PmWriteError::NotFound("finding link not found"));
    }
    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.finding_unlinked",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn list_requirement_findings(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
) -> Result<serde_json::Value, PmWriteError> {
    let items: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT f.id, f.kind::text \
         FROM delivery.requirement_finding rf \
         JOIN discovery.discovery_finding f \
           ON f.tenant_id = rf.tenant_id AND f.project_id = rf.project_id AND f.id = rf.finding_id \
         JOIN delivery.requirement r ON r.project_id = rf.project_id AND r.id = rf.requirement_id \
         WHERE rf.requirement_id = $1 AND r.tenant_id = $2 \
         ORDER BY rf.created_at DESC",
    )
    .bind(requirement_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let items: Vec<serde_json::Value> = items
        .into_iter()
        .map(|(id, kind)| json!({ "finding_id": id, "kind": kind }))
        .collect();
    Ok(serde_json::json!({ "items": items }))
}

// ─────────────────────────────────────── 按 basis 清单一键物化（DK-0 §3.3）

#[derive(Debug, Deserialize)]
pub struct MaterializeBasisRequest {
    /// 显式子集（UI 上被取消勾选的条目）；缺省 = 修订 `content.basis_finding_ids`
    /// 的全量清单。**不允许**超出清单——点击是"物化 AI 的建议"，不是"新增关联"。
    #[serde(default)]
    pub finding_ids: Option<Vec<Uuid>>,
    /// 是否同时登记并关联访谈来源材料（`source_input(kind='discovery')`）。
    /// 缺省 `true`——设计稿 §3.3 的物化面包含"从哪来"的来源材料。
    #[serde(default)]
    pub register_source: Option<bool>,
    /// 来源材料标题（缺省 `访谈会话 {session_id}`）。
    #[serde(default)]
    pub source_title: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MaterializeBasisOutcome {
    pub requirement_id: Uuid,
    /// 读取 basis 的修订（最新修订：人类正在复核的那一条）。
    pub revision_id: Uuid,
    /// 修订里登记的提案级标记（AI 写的，非权威）。
    pub basis_finding_ids: Vec<Uuid>,
    pub linked_findings: Vec<Uuid>,
    pub already_linked_findings: Vec<Uuid>,
    pub source_input_id: Option<Uuid>,
    pub source_linked: bool,
}

/// 「按 basis 清单一键物化」（D-DK-5 的**人类**半步）。
///
/// 语义：把 AI 写在 `content.basis_finding_ids` 里的提案级标记，一次点击物化成
/// 正式溯源关联——`requirement_finding`（逐条 `link_finding`）+
/// `source.source_input(kind='discovery')` + `requirement_source_input`
/// （`register_source_input` / `link_source_input`）。
///
/// **不绕过 S02D-20**：本函数第一行就是 `require_human`，下面调用的每个既有
/// 服务各自也有真人闸；AI 永远走不到这里（走即 403）。这里只是把"人点 N 次"
/// 压成"人点 1 次"，判定与写入全部复用既有单点服务。
///
/// 幂等：已关联的 finding / 已关联的来源材料分别进 `already_*`，重放 no-op。
pub async fn materialize_basis(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    req: MaterializeBasisRequest,
    actor: &AuditActor,
) -> Result<MaterializeBasisOutcome, PmWriteError> {
    require_human(pool, actor).await?;

    let row: Option<(Uuid, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT rv.id, rv.content->'basis_finding_ids' \
           FROM delivery.requirement r \
           JOIN LATERAL ( \
               SELECT id, content FROM delivery.requirement_revision v \
                WHERE v.requirement_id = r.id ORDER BY v.version DESC LIMIT 1 \
           ) rv ON TRUE \
          WHERE r.id = $1 AND r.tenant_id = $2",
    )
    .bind(requirement_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let (revision_id, basis_json) = row.ok_or(PmWriteError::NotFound("requirement not found"))?;
    let basis: Vec<Uuid> = basis_json
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    if basis.is_empty() {
        return Err(PmWriteError::Validation(
            "requirement revision has no basis_finding_ids to materialize",
        ));
    }

    let requested = match req.finding_ids {
        Some(ids) if !ids.is_empty() => {
            // 超出清单 → 拒绝（fail-closed：UI 与数据不同步时不猜意图）。
            if let Some(unknown) = ids.iter().find(|id| !basis.contains(id)) {
                let _ = unknown;
                return Err(PmWriteError::Validation(
                    "finding_ids contains an id outside the revision basis",
                ));
            }
            ids
        }
        _ => basis.clone(),
    };

    let mut linked_findings = Vec::new();
    let mut already_linked_findings = Vec::new();
    for finding_id in &requested {
        match link_finding(pool, tenant_id, requirement_id, *finding_id, actor).await {
            Ok(_) => linked_findings.push(*finding_id),
            Err(PmWriteError::Conflict(_)) => already_linked_findings.push(*finding_id),
            Err(other) => return Err(other),
        }
    }

    // 来源材料：一个访谈会话一条 `source_input`（按 project + source_ref 复用）。
    let mut source_input_id = None;
    let mut source_linked = false;
    if req.register_source.unwrap_or(true) {
        let session_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT f.session_id FROM discovery.discovery_finding f \
               JOIN delivery.requirement r ON r.id = $2 \
              WHERE f.id = $1 AND f.tenant_id = $3 AND f.project_id = r.project_id",
        )
        .bind(requested[0])
        .bind(requirement_id)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .map_err(PmWriteError::Db)?;
        if let Some(session_id) = session_id {
            let source_ref = session_id.to_string();
            let project_id: Uuid =
                sqlx::query_scalar("SELECT project_id FROM delivery.requirement WHERE id = $1")
                    .bind(requirement_id)
                    .fetch_one(pool)
                    .await
                    .map_err(PmWriteError::Db)?;
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM source.source_input \
                  WHERE tenant_id = $1 AND project_id = $2 AND input_kind = 'discovery' \
                    AND source_ref = $3 ORDER BY created_at LIMIT 1",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(&source_ref)
            .fetch_optional(pool)
            .await
            .map_err(PmWriteError::Db)?;
            let input_id = match existing {
                Some(id) => id,
                None => {
                    let entry = register_source_input(
                        pool,
                        tenant_id,
                        project_id,
                        RegisterSourceInputRequest {
                            input_kind: "discovery".to_string(),
                            title: req
                                .source_title
                                .as_deref()
                                .map(str::trim)
                                .filter(|t| !t.is_empty())
                                .map(str::to_string)
                                .unwrap_or_else(|| format!("访谈会话 {source_ref}")),
                            content: None,
                            source_ref: Some(source_ref.clone()),
                        },
                        actor,
                    )
                    .await?;
                    entry.id
                }
            };
            match link_source_input(pool, tenant_id, requirement_id, input_id, actor).await {
                Ok(_) => source_linked = true,
                Err(PmWriteError::Conflict(_)) => source_linked = true,
                Err(other) => return Err(other),
            }
            source_input_id = Some(input_id);
        }
    }

    Ok(MaterializeBasisOutcome {
        requirement_id,
        revision_id,
        basis_finding_ids: basis,
        linked_findings,
        already_linked_findings,
        source_input_id,
        source_linked,
    })
}
