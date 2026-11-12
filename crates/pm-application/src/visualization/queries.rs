//! 可视化查询服务（V0-2 API 支撑）——**SQL 只住在这里**。
//!
//! 依据 refactor/backend 引入的 pm-api 零 SQL 门禁（`guard-pm-api-sql.sh`）：
//! handler 层不得直写 SQL，一律下沉到 pm-application 服务模块。
//!
//! 覆盖 viz_handlers 的四条查询：
//! - 槽位摘要（DISTINCT ON 每槽最新版本）；
//! - Artifact 分页列表（count + 页数据）；
//! - 审批 → 项目归属解析（权限链前置）。

use sqlx::PgPool;
use uuid::Uuid;

/// 槽位摘要行：每槽位当前版本的核心元信息。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct SlotSummaryRow {
    pub slot_key: String,
    pub view_kind: String,
    pub state: String,
    pub nature: String,
    pub version_no: i32,
    pub artifact_id: Uuid,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 槽位摘要：每槽位取最新一条（DISTINCT ON + created_at DESC）。
pub async fn list_slot_summaries(
    pool: &PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
) -> Result<Vec<SlotSummaryRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT DISTINCT ON (a.slot_key) a.slot_key, \
                COALESCE(a.spec->'view'->>'kind', 'unknown') AS view_kind, \
                a.state, a.nature, a.version_no, a.id AS artifact_id, a.updated_at \
           FROM viz.artifact a \
          WHERE a.project_id = $1 AND a.tenant_id = $2 \
          ORDER BY a.slot_key, a.created_at DESC",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

/// Artifact 列表行。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ArtifactListRow {
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

/// 列表过滤条件。
#[derive(Debug, Clone, Default)]
pub struct ArtifactFilter<'a> {
    pub slot_key: Option<&'a str>,
    pub state: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

/// 列表总数（与 [`list_artifacts_page`] 同一组过滤）。
pub async fn count_artifacts(
    pool: &PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
    filter: &ArtifactFilter<'_>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*) FROM viz.artifact \
          WHERE project_id = $1 AND tenant_id = $2 \
            AND ($3::text IS NULL OR slot_key = $3) \
            AND ($4::text IS NULL OR state = $4)",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(filter.slot_key)
    .bind(filter.state)
    .fetch_one(pool)
    .await
}

/// 分页列表（created_at DESC）。
pub async fn list_artifacts_page(
    pool: &PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
    filter: &ArtifactFilter<'_>,
) -> Result<Vec<ArtifactListRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, tenant_id, project_id, slot_key, version_no, state, nature, \
                spec, spec_hash, source_hash, renderer_id, renderer_version, \
                created_by, created_at \
           FROM viz.artifact \
          WHERE project_id = $1 AND tenant_id = $2 \
            AND ($3::text IS NULL OR slot_key = $3) \
            AND ($4::text IS NULL OR state = $4) \
          ORDER BY created_at DESC LIMIT $5 OFFSET $6",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(filter.slot_key)
    .bind(filter.state)
    .bind(filter.limit)
    .bind(filter.offset)
    .fetch_all(pool)
    .await
}

/// 审批 → 项目归属（权限链前置：approval → project → 成员可见性）。
pub async fn approval_project(
    pool: &PgPool,
    approval_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT project_id FROM viz.approval WHERE id = $1")
        .bind(approval_id)
        .fetch_optional(pool)
        .await
}
