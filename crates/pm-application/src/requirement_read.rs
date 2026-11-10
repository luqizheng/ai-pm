//! S02 · 需求域读模型（评审队列 / 需求基线 / 变更影响分析）。
//!
//! S02 已有需求 CRUD 与修订生命周期（draft → submitted → approved/rejected），
//! 但缺三块**页面级读面**：
//!
//! - `project_review_queue`：待人工裁决的**修订提案**队列（S02-UI-03）。
//!   评审对象是**修订**（不是需求）——需求是稳定身份，评审的是它的一版正文。
//! - `project_requirement_baseline`：项目当前需求基线（S02-UI-04）。按 S02
//!   裁决 R3：**需求基线 = 全部已确认需求的当前正式修订集合**——它是**派生
//!   视图**，不新建基线表（避免与 S03 的实施基线混淆）。
//! - `requirement_impact`：单个需求的变更影响分析（S02-UI-07）：谁绑定了它
//!   （设计 / 任务）、绑的是哪一版、是否已被新版本取代。
//!
//! 领域纪律：
//! - **修订五态**：draft / pending_confirmation / approved / rejected /
//!   superseded；只有 approved 是正式依据。
//! - **确认 ≠ 评审**：proposal 队列是「待确认修订」；已确认历史是另一张读面。
//! - **影响分析不造结论**：只列事实（谁绑了哪一版、是否过期），不下判断。

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::pm_write::PmWriteError;

// ───────────────────────────────────────────── 评审队列（S02-UI-03）

/// 待评审修订行。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct ReviewProposalRow {
    pub revision_id: Uuid,
    pub requirement_id: Uuid,
    pub requirement_code: Option<String>,
    pub requirement_title: Option<String>,
    pub requirement_type: Option<String>,
    pub priority: Option<String>,
    pub version: i32,
    pub state: String,
    pub content: Value,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub created_by_name: Option<String>,
    /// 前一正式版本的版本号（供并排比较）。
    pub base_version: Option<i32>,
    pub base_content_hash: Option<String>,
    /// 绑定到本修订的来源（源文档 / 发现）。
    pub source_count: i64,
}

/// 评审队列读模型。
#[derive(Debug, Serialize)]
pub struct RequirementReviewQueue {
    pub proposals: Vec<ReviewProposalRow>,
    /// 按类型分桶（functional / non_functional）。
    pub by_type: Vec<(String, i64)>,
    /// 按优先级分桶（low / normal / high）。
    pub by_priority: Vec<(String, i64)>,
}

/// 项目级需求评审队列（待确认修订）。
pub async fn project_review_queue(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<RequirementReviewQueue, PmWriteError> {
    let proposals: Vec<ReviewProposalRow> = sqlx::query_as(
        "SELECT rv.id AS revision_id, rv.requirement_id, r.code AS requirement_code, \
                r.title AS requirement_title, r.requirement_type, r.priority, \
                rv.version, rv.state, rv.content, rv.content_hash, rv.created_at, \
                rv.submitted_at, rv.created_by, p.display_name AS created_by_name, \
                base.version AS base_version, base.content_hash AS base_content_hash, \
                (SELECT count(*) FROM delivery.requirement_source_input si \
                  WHERE si.requirement_id = r.id) \
                    AS source_count \
         FROM delivery.requirement_revision rv \
         JOIN delivery.requirement r ON r.id = rv.requirement_id \
         LEFT JOIN core.principal p ON p.id = rv.created_by \
         LEFT JOIN delivery.requirement_revision base ON base.id = rv.base_current_revision_id \
         WHERE rv.tenant_id = $1 AND rv.project_id = $2 \
           AND rv.state = 'pending_confirmation' \
         ORDER BY rv.submitted_at NULLS LAST, rv.created_at",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let by_type: Vec<(String, i64)> = sqlx::query_as(
        "SELECT r.requirement_type, count(*) FROM delivery.requirement_revision rv \
         JOIN delivery.requirement r ON r.id = rv.requirement_id \
         WHERE rv.tenant_id = $1 AND rv.project_id = $2 \
           AND rv.state = 'pending_confirmation' \
         GROUP BY r.requirement_type ORDER BY r.requirement_type",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let by_priority: Vec<(String, i64)> = sqlx::query_as(
        "SELECT r.priority, count(*) FROM delivery.requirement_revision rv \
         JOIN delivery.requirement r ON r.id = rv.requirement_id \
         WHERE rv.tenant_id = $1 AND rv.project_id = $2 \
           AND rv.state = 'pending_confirmation' \
         GROUP BY r.priority ORDER BY r.priority",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    Ok(RequirementReviewQueue {
        proposals,
        by_type,
        by_priority,
    })
}

// ───────────────────────────────────────────── 需求基线（S02-UI-04）

/// 需求基线行（一个需求在基线中的当前正式版本）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct RequirementBaselineRow {
    pub requirement_id: Uuid,
    pub code: Option<String>,
    pub title: Option<String>,
    pub requirement_type: Option<String>,
    pub priority: Option<String>,
    pub lifecycle_status: Option<String>,
    pub revision_id: Option<Uuid>,
    pub version: Option<i32>,
    pub state: Option<String>,
    pub content_hash: Option<String>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub confirmed_by: Option<String>,
    pub acceptance_criteria_count: i64,
    pub source_count: i64,
}

/// 需求基线读模型（派生视图，非存储实体）。
#[derive(Debug, Serialize)]
pub struct RequirementBaseline {
    pub items: Vec<RequirementBaselineRow>,
    /// 已确认需求数 / 项目需求总数。
    pub confirmed_count: i64,
    pub total_count: i64,
    /// 存在「有需求但未确认」缺口时的提示（不造结论，只陈述数量）。
    pub unconfirmed_count: i64,
}

/// 项目当前需求基线 = 全部已确认需求的当前正式修订集合。
pub async fn project_requirement_baseline(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<RequirementBaseline, PmWriteError> {
    let items: Vec<RequirementBaselineRow> = sqlx::query_as(
        "SELECT r.id AS requirement_id, r.code, r.title, r.requirement_type, r.priority, \
                r.lifecycle_status, rv.id AS revision_id, rv.version, rv.state, \
                rv.content_hash, c.confirmed_at, hp.display_name AS confirmed_by, \
                (SELECT count(*) FROM delivery.requirement_acceptance_criterion ac \
                  WHERE ac.requirement_id = r.id \
                    AND ac.established_in_revision_id = rv.id) AS acceptance_criteria_count, \
                (SELECT count(*) FROM delivery.requirement_source_input si \
                  WHERE si.requirement_id = r.id) AS source_count \
         FROM delivery.requirement r \
         LEFT JOIN delivery.requirement_revision rv ON rv.id = r.current_revision_id \
         LEFT JOIN delivery.requirement_confirmation c \
                ON c.requirement_id = r.id AND c.revision_id = rv.id AND c.action = 'confirm' \
         LEFT JOIN core.principal hp ON hp.id = c.human_principal_id \
         WHERE r.tenant_id = $1 AND r.project_id = $2 \
           AND r.lifecycle_status = 'active' \
         ORDER BY r.code",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let total_count = items.len() as i64;
    let confirmed_count = items
        .iter()
        .filter(|item| item.state.as_deref() == Some("approved"))
        .count() as i64;

    Ok(RequirementBaseline {
        items,
        confirmed_count,
        total_count,
        unconfirmed_count: total_count - confirmed_count,
    })
}

// ───────────────────────────────────────────── 变更影响分析（S02-UI-07）

/// 影响面行：谁绑定了本需求（设计 / 任务），绑的是哪一版，是否过期。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct RequirementImpactRow {
    /// `design` / `task`。
    pub kind: String,
    pub target_id: Uuid,
    pub code: Option<String>,
    pub title: Option<String>,
    /// 绑定的需求修订版本号。
    pub bound_version: Option<i32>,
    /// 本需求当前正式版本号。
    pub current_version: Option<i32>,
    /// 绑定的是否为当前正式版本（false = 已过期，需重新评估）。
    pub is_current: Option<bool>,
    /// 对端自身状态（设计修订状态 / 任务状态）。
    pub target_status: Option<String>,
}

/// 需求变更影响分析读模型。
#[derive(Debug, Serialize)]
pub struct RequirementImpact {
    pub requirement_id: Uuid,
    pub code: Option<String>,
    pub title: Option<String>,
    pub current_version: Option<i32>,
    /// 设计绑定（design_requirement）。
    pub designs: Vec<RequirementImpactRow>,
    /// 任务绑定（task_requirement）。
    pub tasks: Vec<RequirementImpactRow>,
    /// 过期绑定数（绑了已被取代的修订）。
    pub stale_binding_count: i64,
}

/// 单个需求的变更影响分析。
pub async fn requirement_impact(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
) -> Result<RequirementImpact, PmWriteError> {
    let (code, title, _current_revision_id, current_version): (
        Option<String>,
        Option<String>,
        Option<Uuid>,
        Option<i32>,
    ) = sqlx::query_as(
        "SELECT r.code, r.title, r.current_revision_id, rv.version \
         FROM delivery.requirement r \
         LEFT JOIN delivery.requirement_revision rv ON rv.id = r.current_revision_id \
         WHERE r.tenant_id = $1 AND r.id = $2",
    )
    .bind(tenant_id)
    .bind(requirement_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("requirement not found"))?;

    let designs: Vec<RequirementImpactRow> = sqlx::query_as(
        "SELECT 'design' AS kind, dr.design_id AS target_id, d.code, d.title, rv.version AS bound_version, \
                cur.version AS current_version, \
                (r.current_revision_id = dr.requirement_revision_id) AS is_current, \
                rv.state AS target_status \
         FROM design.design_requirement dr \
         LEFT JOIN design.design d ON d.id = dr.design_id \
         LEFT JOIN design.design_revision rv ON rv.id = dr.design_revision_id \
         LEFT JOIN delivery.requirement r ON r.id = dr.requirement_id \
         LEFT JOIN delivery.requirement_revision cur ON cur.id = r.current_revision_id \
         WHERE dr.tenant_id = $1 AND dr.requirement_id = $2 \
         ORDER BY d.code",
    )
    .bind(tenant_id)
    .bind(requirement_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let tasks: Vec<RequirementImpactRow> = sqlx::query_as(
        "SELECT 'task' AS kind, tr.task_id AS target_id, NULL::text AS code, t.title, \
                rv.version AS bound_version, cur.version AS current_version, \
                (r.current_revision_id = tr.revision_id) AS is_current, \
                t.status AS target_status \
         FROM delivery.task_requirement tr \
         LEFT JOIN delivery.task t ON t.id = tr.task_id \
         LEFT JOIN delivery.requirement r ON r.id = tr.requirement_id \
         LEFT JOIN delivery.requirement_revision rv ON rv.id = tr.revision_id \
         LEFT JOIN delivery.requirement_revision cur ON cur.id = r.current_revision_id \
         WHERE tr.tenant_id = $1 AND tr.requirement_id = $2 \
         ORDER BY t.title",
    )
    .bind(tenant_id)
    .bind(requirement_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let stale_binding_count = designs
        .iter()
        .chain(tasks.iter())
        .filter(|row| row.is_current != Some(true))
        .count() as i64;

    Ok(RequirementImpact {
        requirement_id,
        code,
        title,
        current_version,
        designs,
        tasks,
        stale_binding_count,
    })
}
