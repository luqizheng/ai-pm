//! `/v1/kb/*` 与 `/v1/pc/*`：KB-PC 服务化 API 契约路由骨架（P2-2；V1.1 §12）。
//!
//! 骨架语义：鉴权链完整挂接（`require_bearer → resolve_principal →
//! require_active` 全局中间件 + handler 内 `require_project_visible` 同口径），
//! 业务逻辑占位——端口 trait 落 `kb_pc_contracts::ports`（V1 进程内占位
//! `PortNotWired`），调用返回 **501 Not Implemented**（未接线 ≠ 404 误导；
//! 真实落点 Phase 4/KB 独立化与 Phase 5/PC-01C）。骨架不做 wire 校验——
//! 受理/查询的契约解析归 Phase 4/5 的真实端口实现。
//!
//! 受理类入口（K1/P1/P2）的实际业务已在 Phase 3 由 Platform Intake 端点
//! 承担（同事务三写 + outbox），本骨架保留契约路由面供 Phase 7 transport
//! 替换（ADR-10 拆分预案第一步）。
//!
//! PC → KB 只读（PK1，§12.4）复用 K3/K5 同一路由；KB → PC 无任何路由
//! （§12.5 移除，附录 B.2）。

use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    routing::{get, post},
};
use kb_pc_contracts::ports::{KbServicePort, PcServicePort, PortError, PortNotWired};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{ApiError, ApiResult, require_project_visible};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        // K1（受理骨架）/ K2 / K3（PK1 共用）/ K4 / K5（PK1 共用）/ K6
        .route("/v1/kb/analyses", post(k1_submit_analysis))
        .route("/v1/kb/analyses/{kb_analysis_id}", get(k2_analysis_status))
        .route(
            "/v1/kb/documents/{document_id}/revisions/{revision_id}",
            get(k3_document_revision),
        )
        .route("/v1/kb/memories/{memory_id}/review", post(k4_review_memory))
        .route("/v1/kb/sources", get(k5_list_sources))
        .route(
            "/v1/kb/supply-requests/{request_id}/fulfil",
            post(k6_fulfil_supply),
        )
        // P1~P7
        .route("/v1/pc/analyses", post(p1_submit_analysis))
        .route("/v1/pc/decisions", post(p2_submit_decision))
        .route("/v1/pc/analyses/{pc_analysis_id}", get(p3_analysis_status))
        .route("/v1/pc/proposals/{proposal_id}", get(p4_proposal))
        .route(
            "/v1/pc/proposals/{proposal_id}/decision",
            post(p5_decide_proposal),
        )
        .route("/v1/pc/conflicts/{conflict_id}", get(p6_conflict))
        .route("/v1/pc/versions/{version_id}", get(p7_version))
}

impl From<PortError> for ApiError {
    fn from(e: PortError) -> Self {
        match e {
            PortError::NotWired(_) => ApiError::NotImplemented(
                "kb/pc service not wired (V1 in-process placeholder; landing = Phase 4/5)",
            ),
            PortError::NotFound(_) => ApiError::NotFound("resource not found"),
            PortError::Forbidden(_) => ApiError::Forbidden("caller lacks permission"),
            PortError::Invalid(_) => ApiError::BadRequest("invalid request"),
        }
    }
}

/// 项目上下文：`?project_id=` 必填（同 outbox_ops 惯例——跨项目调用没有意义
/// 且必须落到租户边界内）。
fn require_project_param(query: &HashMap<String, String>) -> ApiResult<Uuid> {
    query
        .get("project_id")
        .and_then(|v| Uuid::parse_str(v).ok())
        .ok_or(ApiError::BadRequest("project_id is required"))
}

async fn guard(state: &AppState, caller: &AuthedPrincipal, project_id: Uuid) -> ApiResult<()> {
    require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id).await
}

/// 受理骨架统一落点：项目可见性校验通过后，占位端口答 501。
/// `body`/`path` 参数在此仅为签名占位（Phase 4/5 的真实实现按契约解析）。
async fn skeleton_501<T>(_opaque: T) -> ApiResult<Json<serde_json::Value>> {
    Err(ApiError::from(PortError::NotWired("skeleton")))
}

// ── Platform → KB（K1~K6）───────────────────────────────────────────────────

async fn k1_submit_analysis(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    skeleton_501(body).await
}

async fn k2_analysis_status(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(kb_analysis_id): Path<Uuid>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let port = PortNotWired;
    match KbServicePort::analysis_status(&port, kb_analysis_id).await {
        Ok(v) => Ok(Json(json!(v))),
        Err(e) => Err(ApiError::from(e)),
    }
}

async fn k3_document_revision(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((document_id, revision_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let port = PortNotWired;
    match KbServicePort::document_revision(&port, &document_id, &revision_id).await {
        Ok(v) => Ok(Json(json!(v))),
        Err(e) => Err(ApiError::from(e)),
    }
}

async fn k4_review_memory(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(memory_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let _ = memory_id;
    skeleton_501(body).await
}

async fn k5_list_sources(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let port = PortNotWired;
    let evidence_of = query.get("evidence_of").map(|raw| {
        // 只读查询骨架：evidence_of 以 knowledge_base 文档四元组表达（PK1 通道）。
        kb_pc_contracts::DomainRef::for_domain(
            kb_pc_contracts::registry::TargetDomain::KnowledgeBase,
            "document",
            raw.clone(),
            None,
        )
    });
    match KbServicePort::list_sources(&port, project_id, evidence_of.as_ref()).await {
        Ok(items) => Ok(Json(json!({ "items": items }))),
        Err(e) => Err(ApiError::from(e)),
    }
}

async fn k6_fulfil_supply(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(request_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let _ = request_id;
    skeleton_501(body).await
}

// ── Platform → PC（P1~P7）───────────────────────────────────────────────────

async fn p1_submit_analysis(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    skeleton_501(body).await
}

async fn p2_submit_decision(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    skeleton_501(body).await
}

async fn p3_analysis_status(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(pc_analysis_id): Path<Uuid>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let port = PortNotWired;
    match PcServicePort::analysis_status(&port, pc_analysis_id).await {
        Ok(v) => Ok(Json(json!(v))),
        Err(e) => Err(ApiError::from(e)),
    }
}

async fn p4_proposal(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(proposal_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let _ = proposal_id;
    skeleton_501(()).await
}

async fn p5_decide_proposal(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(proposal_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let _ = proposal_id;
    skeleton_501(body).await
}

async fn p6_conflict(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(conflict_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let _ = conflict_id;
    skeleton_501(()).await
}

async fn p7_version(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(version_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let _ = version_id;
    skeleton_501(()).await
}
