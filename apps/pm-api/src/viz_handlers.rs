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
use pm_application::visualization::live_graph::{
    build_architecture_spec, build_task_dag_spec, build_trace_spec, LIVE_EDGE_LIMIT,
};
use pm_application::visualization::projection::{
    project_spec, ReadStrategy,
};
use pm_application::repo::viz_read::VizReadRepo;
use pm_application::visualization::queries::{
    approval_project, count_artifacts, list_artifacts_page, list_slot_summaries, ArtifactFilter,
};
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
        .route(
            "/v1/pm/visualizations/projects/{project_id}/live/{view_kind}",
            get(live_graph),
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

async fn slots(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id).await?;

    let rows = list_slot_summaries(&state.pool, project_id, caller.tenant_id).await?;

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

    let filter = ArtifactFilter {
        slot_key: slot_key.map(|s| s.as_str()),
        state: artifact_state.map(|s| s.as_str()),
        limit: page.size,
        offset: page.offset,
    };
    let total = count_artifacts(&state.pool, project_id, caller.tenant_id, &filter).await?;
    let items = list_artifacts_page(&state.pool, project_id, caller.tenant_id, &filter).await?;

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
    let project_id = approval_project(&state.pool, approval_id)
        .await?
        .ok_or(ApiError::NotFound("visualization approval not found"))?;
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


// ───────────────────────────────────────────── Live 图查询

/// Live 图：实时聚合既有关系表 → 投影裁剪 → 返回。
///
/// 一致性：单条 SQL 取数（repo 层保证）→ `consistency=current`。
/// 权限：项目可见性前置 + 投影层二次裁剪（节点/边/计数/provenance）。
async fn live_graph(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((project_id, view_kind)): Path<(Uuid, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id).await?;

    let repo = crate::support::repos::viz_read_repo(&state.pool);
    let (spec, truncated) = match view_kind.as_str() {
        "trace-graph" => {
            let rows = repo
                .trace_edges(caller.tenant_id, project_id, LIVE_EDGE_LIMIT)
                .await?;
            let truncated = rows.len() as i64 >= LIVE_EDGE_LIMIT;
            (build_trace_spec(project_id, &state.viz_registry, &rows, truncated), truncated)
        }
        "task-dag" => {
            let rows = repo
                .task_dependencies(caller.tenant_id, project_id, LIVE_EDGE_LIMIT)
                .await?;
            let truncated = rows.len() as i64 >= LIVE_EDGE_LIMIT;
            (build_task_dag_spec(project_id, &state.viz_registry, &rows, truncated), truncated)
        }
        "architecture-graph" => {
            let rows = repo
                .cognition_edges(caller.tenant_id, project_id, LIVE_EDGE_LIMIT)
                .await?;
            let truncated = rows.len() as i64 >= LIVE_EDGE_LIMIT;
            (
                build_architecture_spec(project_id, &state.viz_registry, &rows, truncated),
                truncated,
            )
        }
        _ => {
            return Err(ApiError::NotFound("unknown visualization view kind"));
        }
    };

    // 投影层：一致性标记 + 计数重算。
    //
    // ⚠ 对象级裁剪**不在此处做**：Live 读侧 SQL 已带项目过滤与端点双端隔离
    // （`repo_viz_read` 的 project_id = $2 条件），此时再用"空可见集合"的
    // resolver 二次裁剪会把全部边裁掉（曾因此返回 0 边）。
    // 细粒度对象级裁剪（成员只可见部分对象）随 14 号 §3.2 的 resolver 预取
    // 接线补齐——届时用 probe_visible_objects 预取真实可见集合。
    let resolver = AllVisibleResolver;
    let report = project_spec(&spec, &resolver, ReadStrategy::SingleStatement);

    Ok(Json(serde_json::json!({
        "data": {
            "spec_version": report.spec.spec_version,
            "diagram_type": report.spec.view.kind,
            "nodes": report.spec.nodes,
            "edges": report.spec.edges
        },
        "meta": {
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "consistency": report.consistency.as_str(),
            "truncated": truncated
        }
    })))
}

/// Live 读侧的投影判定器：SQL 已按项目 + 端点双端隔离过滤，
/// 此处置为"全部可见"（**不是**默认空集合——空集合会把边全裁掉）。
///
/// 对象级细粒度裁剪（14 号 §3.2）待 resolver 预取接线后替换本实现。
struct AllVisibleResolver;

impl pm_domain::visualization::ObjectResolver for AllVisibleResolver {
    fn exists_and_visible(&self, _domain: &str, _object_type: &str, _id: &str) -> bool {
        true
    }
    fn relation_exists(&self, _from: &str, _to: &str, _relation: &str) -> bool {
        true
    }
}
