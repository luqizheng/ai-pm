//! `/v1/pm/visualizations`：可视化产物 REST 面（V0-2）。
//!
//! 契约：docs/visualization/ai-pm-viz-13-v0-2-data-model-design-v1.md §6.2。
//!
//! 端点（13 号 §6.2 的读 + 审批面；Live 查询属 V0-3 后续接线）：
//! - `GET    .../projects/{pid}/slots`                 槽位摘要列表
//! - `GET    .../artifacts/{aid}`                      产物详情（含 spec）
//! - `GET    .../projects/{pid}/artifacts`             槽位历史（分页）
//! - `POST   .../projects/{pid}/artifacts`             提交 spec → draft
//! - `POST   .../artifacts/{aid}/approval-requests`    请求发布审批
//! - `POST   .../approvals/{aid}/decision`             裁决（approve/reject/cancel）
//! - `POST   .../artifacts/{aid}/refresh`              stale 重算（同步）
//!
//! 纪律：
//! - 全端点先 `require_project_visible`（或 artifact→project→成员链）；
//! - 写侧 `ensure_trusted_context`（机器凭据 403）；
//! - 错误体只带 `{message}`（仓库惯例），`VizError` 映射 404/409/400/500。

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{ensure_trusted_context, paging, require_project_visible, ApiError, ApiResult};
use pm_application::actor::AuditActor;
use pm_application::visualization::artifact_service::{
    create_draft, decide_approval, get_artifact as svc_get_artifact, mark_stale, request_approval,
    CreateDraftInput, VizError,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/visualizations/projects/{project_id}/slots",
            get(slots),
        )
        .route(
            "/v1/pm/visualizations/projects/{project_id}/artifacts",
            get(list_artifacts).post(create_artifact),
        )
        .route(
            "/v1/pm/visualizations/artifacts/{artifact_id}",
            get(artifact_detail),
        )
        .route(
            "/v1/pm/visualizations/artifacts/{artifact_id}/approval-requests",
            post(approval_request),
        )
        .route(
            "/v1/pm/visualizations/approvals/{approval_id}/decision",
            post(approval_decision),
        )
        .route(
            "/v1/pm/visualizations/artifacts/{artifact_id}/refresh",
            post(artifact_refresh),
        )
}

/// 服务层错误 → HTTP（错误体只带 message，仓库惯例）。
///
/// `Conflict` / `Validation` 的消息是**运行时动态文本**（哈希漂移、pending 撞单、
/// 解释图禁发布等），静态串变体装不下——走 `ApiError::Text`（先例：ACP-03
/// InteractionError，support.rs :22-24）。
fn map_error(err: VizError) -> ApiError {
    match err {
        VizError::NotFound => ApiError::NotFound("visualization artifact not found"),
        VizError::Conflict(message) => ApiError::Text(StatusCode::CONFLICT, message),
        VizError::Validation(message) => ApiError::Text(StatusCode::BAD_REQUEST, message),
        VizError::Db(e) => ApiError::Internal(e),
    }
}

fn audit_actor(caller: &AuthedPrincipal) -> AuditActor {
    AuditActor {
        principal_id: caller.principal_id,
        identity_id: caller.identity_id,
        client: None,
        auth_mode: Some("password".to_string()),
        claimed_device_id: None,
    }
}

// ───────────────────────────────────────────── 槽位摘要

/// 槽位摘要行：每槽位当前版本的核心元信息。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
struct SlotSummaryRow {
    slot_key: String,
    view_kind: String,
    state: String,
    nature: String,
    version_no: i32,
    artifact_id: Uuid,
    updated_at: chrono::DateTime<chrono::Utc>,
}

async fn slots(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id).await?;

    // 每槽位取最新一条（按 created_at DESC 取 top-1）；view_kind 从 spec 读。
    let rows: Vec<SlotSummaryRow> = sqlx::query_as(
        "SELECT DISTINCT ON (a.slot_key) a.slot_key, \
                COALESCE(a.spec->'view'->>'kind', 'unknown') AS view_kind, \
                a.state, a.nature, a.version_no, a.id AS artifact_id, a.updated_at \
           FROM viz.artifact a \
          WHERE a.project_id = $1 AND a.tenant_id = $2 \
          ORDER BY a.slot_key, a.created_at DESC",
    )
    .bind(project_id)
    .bind(caller.tenant_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "items": rows })))
}

// ───────────────────────────────────────────── 产物详情

async fn artifact_detail(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(artifact_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    // 跨属主 404 同形：先取产物 → 再验项目可见性
    let artifact = svc_get_artifact(&state.pool, artifact_id).await.map_err(map_error)?;
    require_project_visible(
        &state.pool,
        artifact.project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;

    Ok(Json(serde_json::to_value(&artifact).expect("serialize")))
}

// ───────────────────────────────────────────── 列表（分页）

async fn list_artifacts(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id).await?;

    let page = paging(&params);
    let slot_key = params.get("slot_key").filter(|s| !s.trim().is_empty());
    let artifact_state = params.get("state").filter(|s| !s.trim().is_empty());

    // 计数 + 页数据（同一组过滤）
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM viz.artifact \
          WHERE project_id = $1 AND tenant_id = $2 \
            AND ($3::text IS NULL OR slot_key = $3) \
            AND ($4::text IS NULL OR state = $4)",
    )
    .bind(project_id)
    .bind(caller.tenant_id)
    .bind(slot_key)
    .bind(artifact_state)
    .fetch_one(&state.pool)
    .await?;

    #[derive(Debug, sqlx::FromRow, serde::Serialize)]
    struct ArtifactRow {
        id: Uuid,
        tenant_id: Uuid,
        project_id: Uuid,
        slot_key: String,
        version_no: i32,
        state: String,
        nature: String,
        spec: serde_json::Value,
        spec_hash: String,
        source_hash: String,
        renderer_id: Option<String>,
        renderer_version: Option<String>,
        created_by: Uuid,
        created_at: chrono::DateTime<chrono::Utc>,
    }
    let items: Vec<ArtifactRow> = sqlx::query_as(
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
    .bind(caller.tenant_id)
    .bind(slot_key)
    .bind(artifact_state)
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": page.page,
        "size": page.size
    })))
}

// ───────────────────────────────────────────── 创建 draft

#[derive(Debug, serde::Deserialize)]
struct CreateArtifactBody {
    slot_key: String,
    nature: String,
    spec: serde_json::Value,
    spec_hash: String,
    source_hash: String,
    #[serde(default)]
    provenance: serde_json::Value,
}

async fn create_artifact(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateArtifactBody>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id).await?;
    ensure_trusted_context(&caller)?;

    let record = create_draft(
        &state.pool,
        project_id,
        &audit_actor(&caller),
        CreateDraftInput {
            slot_key: &body.slot_key,
            nature: &body.nature,
            spec: &body.spec,
            spec_hash: &body.spec_hash,
            source_hash: &body.source_hash,
            provenance: &if body.provenance.is_null() {
                serde_json::json!({})
            } else {
                body.provenance
            },
        },
    )
    .await
    .map_err(map_error)?;

    Ok(Json(serde_json::to_value(&record).expect("serialize")))
}

// ───────────────────────────────────────────── 审批

async fn approval_request(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(artifact_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    // artifact → project → 成员可见性
    let artifact = svc_get_artifact(&state.pool, artifact_id).await.map_err(map_error)?;
    require_project_visible(
        &state.pool,
        artifact.project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    ensure_trusted_context(&caller)?;

    let approval_id =
        request_approval(&state.pool, artifact_id, &audit_actor(&caller))
            .await
            .map_err(map_error)?;

    Ok(Json(
        serde_json::json!({ "approvalId": approval_id, "status": 201 }),
    ))
}

#[derive(Debug, serde::Deserialize)]
struct DecisionBody {
    action: String,
    #[serde(default)]
    reason: Option<String>,
}

async fn approval_decision(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(approval_id): Path<Uuid>,
    Json(body): Json<DecisionBody>,
) -> ApiResult<StatusCode> {
    // approval → project → 成员可见性
    let project_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT project_id FROM viz.approval WHERE id = $1",
    )
    .bind(approval_id)
    .fetch_optional(&state.pool)
    .await?;
    let project_id = project_id.ok_or(ApiError::NotFound("visualization approval not found"))?;
    require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id).await?;
    ensure_trusted_context(&caller)?;

    decide_approval(
        &state.pool,
        approval_id,
        &audit_actor(&caller),
        &body.action,
        body.reason.as_deref(),
    )
    .await
    .map_err(map_error)?;

    Ok(StatusCode::OK)
}

// ───────────────────────────────────────────── stale 重算

async fn artifact_refresh(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(artifact_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let artifact = svc_get_artifact(&state.pool, artifact_id).await.map_err(map_error)?;
    require_project_visible(
        &state.pool,
        artifact.project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;

    // 当前实现：比对 source_hash 与源对象现状（重算），失配则置 stale。
    // 源对象重算属 V0-3 后续接线（needs DB aggregation）；此处先落标记路径。
    mark_stale(&state.pool, artifact_id, &audit_actor(&caller))
        .await
        .map_err(map_error)?;

    Ok(StatusCode::OK)
}

