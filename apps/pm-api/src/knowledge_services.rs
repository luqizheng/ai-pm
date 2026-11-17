//! `/v1/kb/*` 与 `/v1/pc/*`：KB-PC 服务化 API 契约路由（P2-2；V1.1 §12）。
//!
//! 骨架语义：鉴权链完整挂接（`require_bearer → resolve_principal →
//! require_active` 全局中间件 + handler 内 `require_project_visible` 同口径）。
//! **checkpoint-1 落真范围**：K2（kb 分析状态查询）与 P3（pc 分析状态查询）
//! 读 0053 最小表（kb.document\* / pc.cognition_object\*），其余受理/治理面
//! 仍走占位端口 `PortNotWired` 返回 **501 Not Implemented**（未接线 ≠ 404
//! 误导）；真实落点 Phase 4/KB 独立化与 Phase 5/PC-01C。骨架不做 wire 校验
//! ——受理契约解析归 Phase 4/5 的真实端口实现。
//!
//! 受理类入口（K1/P1/P2）的实际业务已在 Phase 3 由 Platform Intake 端点
//! 承担（同事务三写 + outbox），本骨架保留契约路由面供 Phase 7 transport
//! 替换（ADR-10 拆分预案第一步）。
//!
//! PC → KB 只读（PK1，§12.4）复用 K3/K5 同一路由；KB → PC 无任何路由
//! （§12.5 移除，附录 B.2）。
//!
//! **KP-3 落真**：K4（记忆审核转达，CAS）与 K5（来源只读查询）经
//! `pm_application::kb_service::KbServiceLive`（`KbServicePort` 进程内实现，调用方
//! 身份/项目作用域按请求绑定——差异说明 D-KP3-3）；K5 同路径追加 `POST` 来源登记
//! （§12.2 未列的 V0.5 管理面，见 `kb_memory_api.rs`，D-KP3-5）。K1/K3/K6 与 P 系
//! 受理/治理面维持 501。
//!
//! **KP-2（审阅流 v2）**：P4（提案详情）与 P5（审批转达：决定暂记 approved/rejected，
//! 报告关闭前可互改；正式版本由报告 close 批量发布）经 `pm_application::pc` 提案命令服务（V-6 唯一写入面）；P5 为治理
//! 动作，另挂可信交互上下文（A*：API Key 403），真人 + A3 门在服务层（AI/服务主体
//! 恒 403）；MCP 不暴露任何确认工具。P3 增提案分支（分析起草提案后 result_ref 锚
//! proposal）。P1/P2/P6/P7 维持 501。

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use kb_pc_contracts::ports::{KbMemoryReviewRequest, KbServicePort, PortError, PortNotWired};
use kb_pc_contracts::registry::TargetDomain;
use kb_pc_contracts::DomainRef;
use pm_application::kb_service::{KbCaller, KbServiceLive};
use pm_application::pc::{self, DecisionAction, PcError, ProposalDecisionInput};
use pm_application::projc::{self, ProjcError};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::kb_pc_query_repo;
use crate::support::{ensure_trusted_context, require_project_visible, ApiError, ApiResult};
use pm_application::repo::kb_pc_query::AnalysisTaskRow;
use pm_application::repo::kb_pc_query::KbPcQueryRepo;

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
        .route(
            "/v1/kb/sources",
            get(k5_list_sources).post(crate::kb_memory_api::register_source),
        )
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
        // PC 对象浏览（前端 KnowledgeView PC 页签；只读管理面；按 id 详情=全文）
        .route("/v1/pc/objects", get(list_pc_objects))
        .route("/v1/pc/objects/{object_id}", get(pc_object_detail))
        // PC 提案列表（前端审批页签；P4 按 id 详查 / P5 决策走既有端点）
        .route("/v1/pc/proposals", get(list_pc_proposals))
        // P3-11 并行端点（T-1/T-2/T-3 W6）：ProjC 提案 P5 决策（p5 同构；
        // 审阅流 v2 形态=范围确认点②，路由到 ProjC 独立域）
        .route(
            "/v1/pm/projc/proposals/{proposal_id}/decide",
            post(projc_p5_decide_proposal),
        )
}

impl From<PortError> for ApiError {
    fn from(e: PortError) -> Self {
        match e {
            PortError::NotWired(_) => ApiError::NotImplemented(
                "kb/pc service not wired (V1 in-process placeholder; landing = Phase 4/5)",
            ),
            PortError::NotFound(_) => ApiError::NotFound("resource not found"),
            PortError::Forbidden(_) => ApiError::Forbidden("caller lacks permission"),
            PortError::Invalid(m) => ApiError::Text(axum::http::StatusCode::BAD_REQUEST, m),
            // KP-3 补齐（差异说明 D-KP3-1）：CAS/状态冲突 409 带 KB 侧可读原因（stale 基线 /
            // 在途提案 / 项目已归档）；内部错误 500 不回显详情（KB 侧已记日志）。
            PortError::Conflict(m) => ApiError::Text(axum::http::StatusCode::CONFLICT, m),
            PortError::Internal(_) => ApiError::Text(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_string(),
            ),
        }
    }
}

/// KP-2：PC 服务层错误 → HTTP（动态原因透传；服务层查询全部项目作用域，404 不泄露
/// 跨项目存在性）。库层错误 500 不回显。
impl From<PcError> for ApiError {
    fn from(e: PcError) -> Self {
        match e {
            PcError::NotFound(m) => ApiError::Text(StatusCode::NOT_FOUND, m),
            PcError::Forbidden(m) => ApiError::Text(StatusCode::FORBIDDEN, m),
            PcError::Invalid(m) => ApiError::Text(StatusCode::BAD_REQUEST, m),
            PcError::Conflict(m) => ApiError::Text(StatusCode::CONFLICT, m),
            PcError::Db(err) => ApiError::Internal(err),
        }
    }
}

/// P3-11：ProjC 服务层错误 → HTTP（pc 同款映射口径）。
impl From<ProjcError> for ApiError {
    fn from(e: ProjcError) -> Self {
        match e {
            ProjcError::NotFound(m) => ApiError::Text(StatusCode::NOT_FOUND, m),
            ProjcError::Forbidden(m) => ApiError::Text(StatusCode::FORBIDDEN, m),
            ProjcError::Invalid(m) => ApiError::Text(StatusCode::BAD_REQUEST, m),
            ProjcError::Conflict(m) => ApiError::Text(StatusCode::CONFLICT, m),
            ProjcError::Db(err) => ApiError::Internal(err),
        }
    }
}

/// PC 域四元组引用（`pc/<type>/<id>`）。
fn pc_ref(target_type: &str, id: Uuid) -> DomainRef {
    DomainRef::for_domain(
        TargetDomain::ProductCognition,
        target_type,
        id.to_string(),
        None,
    )
}

/// ProjC 域四元组引用（`project_cognition/<type>/<id>`）。
fn projc_ref(target_type: &str, id: Uuid) -> DomainRef {
    DomainRef::for_domain(
        TargetDomain::ProjectCognition,
        target_type,
        id.to_string(),
        None,
    )
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
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await
}

/// 受理骨架统一落点：项目可见性校验通过后，占位端口答 501。
/// `body`/`path` 参数在此仅为签名占位（Phase 4/5 的真实实现按契约解析）。
async fn skeleton_501<T>(_opaque: T) -> ApiResult<Json<serde_json::Value>> {
    Err(ApiError::from(PortError::NotWired("skeleton")))
}

/// KP-3：请求级 KB 端口（调用方身份 + 租户 + 项目作用域绑定在实例上，D-KP3-3）。
fn kb_port(state: &AppState, caller: &AuthedPrincipal, project_id: Uuid) -> KbServiceLive {
    KbServiceLive::for_caller(
        state.pool.clone(),
        KbCaller {
            tenant_id: caller.tenant_id,
            project_id,
            actor: caller.audit_actor(),
        },
    )
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

/// K2（checkpoint-1 落真）：`{id}` = kb_analyze 任务 ID（intake_task）。
/// 返回任务状态 + result_ref；真实分析的 result_ref 指向 kb.document 修订，
/// 就地展开文档/修订明细（含 digest 正文）；桩产物（非 UUID ref）只回任务面。
async fn k2_analysis_status(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(kb_analysis_id): Path<Uuid>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let repo = kb_pc_query_repo(&state.pool);
    let task = repo
        .analysis_task(project_id, kb_analysis_id, "kb", "kb_analyze")
        .await?;
    let Some(AnalysisTaskRow {
        task_status,
        result_ref,
        last_error,
        created_at,
        updated_at,
        intake_id,
        intake_type,
        title,
    }) = task
    else {
        return Err(ApiError::NotFound("kb analysis task not found"));
    };
    let mut body = json!({
        "analysis_id": kb_analysis_id,
        "intake_id": intake_id,
        "intake_type": intake_type,
        "title": title,
        "task_status": task_status,
        "result_ref": result_ref,
        "last_error": last_error,
        "created_at": created_at,
        "updated_at": updated_at,
    });
    // 真实分析：result_ref.target_type == "document" → 展开文档与修订正文。
    let revision_id = result_ref
        .as_ref()
        .and_then(|r| r.get("target_type"))
        .and_then(|v| v.as_str())
        .filter(|t| *t == "document")
        .and_then(|_| {
            result_ref
                .as_ref()
                .and_then(|r| r.get("target_revision_id"))
                .and_then(|v| v.as_str())
                .and_then(|raw| Uuid::parse_str(raw).ok())
        });
    if let Some(revision_id) = revision_id {
        if let Some(rev) = repo.kb_revision(project_id, revision_id).await? {
            let content = repo.kb_revision_content(rev.rev_id).await?;
            body["document"] = json!({
                "document_id": rev.doc_id,
                "doc_key": rev.doc_key,
                "doc_type": rev.doc_type,
                "revision": {
                    "revision_id": rev.rev_id,
                    "revision_no": rev.revision_no,
                    "status": rev.revision_status,
                    "source_commit_sha": rev.source_commit_sha,
                    "content_hash": rev.content_hash,
                    "change_summary": rev.change_summary,
                    "content": content,
                    "created_at": rev.created_at,
                },
            });
        }
    }
    Ok(Json(body))
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

/// K4（KP-3 落真）：记忆审核转达（CAS：`base_memory_revision_id` = 当前待审提案）。
/// 响应按 §12.2 形状 `{ memory_id, review_status }`，附被审修订四元组 `revision_ref`
/// （端口返回 `KbAnalysisStatus` 的映射，差异说明 D-KP3-2）。
async fn k4_review_memory(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(memory_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let request: KbMemoryReviewRequest = serde_json::from_value(body).map_err(|e| {
        ApiError::Text(
            axum::http::StatusCode::BAD_REQUEST,
            format!("invalid review body: {e}"),
        )
    })?;
    let port = kb_port(&state, &caller, project_id);
    let status = KbServicePort::review_memory(&port, &memory_id, request).await?;
    Ok(Json(json!({
        "memory_id": memory_id,
        "review_status": status.status,
        "revision_ref": status.memory_draft_refs.first(),
    })))
}

/// K5 `evidence_of` wire 形态：`<memory|memory_revision>:<uuid>`（knowledge_base 域隐含，
/// 差异说明 D-KP3-4）；类型合法性由端口裁决。
fn parse_evidence_of(raw: &str) -> ApiResult<kb_pc_contracts::DomainRef> {
    let (target_type, target_id) = raw
        .split_once(':')
        .filter(|(t, i)| !t.trim().is_empty() && !i.trim().is_empty())
        .ok_or(ApiError::BadRequest(
            "evidence_of must be <memory|memory_revision>:<uuid>",
        ))?;
    Ok(kb_pc_contracts::DomainRef::for_domain(
        kb_pc_contracts::registry::TargetDomain::KnowledgeBase,
        target_type.trim(),
        target_id.trim(),
        None,
    ))
}

/// K5（KP-3 落真）：项目关联的 Original Sources（`?evidence_of=` 过滤被某记忆引用的来源）。
async fn k5_list_sources(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let evidence_of = query
        .get("evidence_of")
        .map(|raw| parse_evidence_of(raw))
        .transpose()?;
    let port = kb_port(&state, &caller, project_id);
    let items = KbServicePort::list_sources(&port, project_id, evidence_of.as_ref()).await?;
    Ok(Json(json!({ "items": items })))
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

/// P3（KP-1 起）：`{id}` = pc_analyze 任务 ID（intake_task）。
/// 返回任务状态 + result_ref；KP-2 起分析起草提案时 result_ref 锚 proposal（附
/// `conclusion=PROPOSED` + 提案摘要，经提案来源展开对象集）；NO_CHANGE 结论与 KP-1
/// 任务的 result_ref 锚 pc.source（展开 source → evidence → revision → object，含
/// pc.document_revision 正文）；checkpoint-1 旧任务的 legacy result_ref
/// （target_type=cognition_object）走直查对象分支。桩产物只回任务面。
///
/// 形状变化登记（KP-1 验收记录）：objects[].name→display_name 语义（键名不变）、
/// latest_revision.status 移除（版本化由 `versioned` 布尔派生——01B §5.3 不冗余
/// revision_status）、latest_revision.summary 改由 change_summary 承载、
/// 新增 analysis_mode（0062 列）。
async fn p3_analysis_status(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(pc_analysis_id): Path<Uuid>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let repo = kb_pc_query_repo(&state.pool);
    let task = repo
        .analysis_task(project_id, pc_analysis_id, "pc", "pc_analyze")
        .await?;
    let Some(AnalysisTaskRow {
        task_status,
        result_ref,
        last_error,
        created_at,
        updated_at,
        intake_id,
        intake_type,
        title,
    }) = task
    else {
        return Err(ApiError::NotFound("pc analysis task not found"));
    };
    let mut body = json!({
        "analysis_id": pc_analysis_id,
        "intake_id": intake_id,
        "intake_type": intake_type,
        "title": title,
        "task_status": task_status,
        "result_ref": result_ref,
        "last_error": last_error,
        "created_at": created_at,
        "updated_at": updated_at,
    });

    let ref_type = result_ref
        .as_ref()
        .and_then(|r| r.get("target_type"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let ref_id = result_ref
        .as_ref()
        .and_then(|r| r.get("target_id"))
        .and_then(|v| v.as_str())
        .and_then(|raw| Uuid::parse_str(raw).ok());

    // KP-2：分析产出未覆盖修订 → AI 起草提案（pc.change_proposed 回写 result_ref=
    // proposal 四元组）——结论 PROPOSED + 提案摘要，经提案来源展开本次分析对象集。
    if ref_type == "proposal" {
        if let Some(proposal_id) = ref_id {
            if let Some(head) = repo.pc_proposal_head(project_id, proposal_id).await? {
                body["conclusion"] = json!("PROPOSED");
                body["proposal_ref"] = json!(pc_ref("proposal", proposal_id));
                body["proposal"] = json!({
                    "proposal_id": proposal_id,
                    "status": head.status,
                    "governance_level": head.governance_level,
                });
                if let Some(source_id) = head.source_id {
                    body["objects"] =
                        json!(objects_of_source(&state, project_id, source_id).await?);
                }
            }
            return Ok(Json(body));
        }
    }

    // pc_analysis_ref 锚 pc.source（KP-1 链路；KP-2 起为 NO_CHANGE 结论的锚）——按来源
    // 展开本次分析产出的对象集（evidence.subject → revision → object + 最新正文）。
    if ref_type == "source" {
        if let Some(source_id) = ref_id {
            body["objects"] = json!(objects_of_source(&state, project_id, source_id).await?);
            return Ok(Json(body));
        }
    }

    // legacy：checkpoint-1 旧任务的 result_ref 锚主对象——收敛后对象行仍在，
    // 直查对象 + 当前修订（读面兼容，新分析不再产生此形状）。
    if ref_type == "cognition_object" {
        if let Some(object_id) = ref_id {
            let rows = repo.pc_object_legacy(project_id, object_id).await?;
            if !rows.is_empty() {
                body["objects"] = json!(rows.into_iter().map(pc_object_json).collect::<Vec<_>>());
            }
        }
    }
    Ok(Json(body))
}

async fn objects_of_source(
    state: &AppState,
    project_id: Uuid,
    source_id: Uuid,
) -> ApiResult<Vec<serde_json::Value>> {
    let rows = kb_pc_query_repo(&state.pool)
        .pc_objects_of_source(project_id, source_id)
        .await?;
    Ok(rows.into_iter().map(pc_object_json).collect())
}

/// P3 对象行投影（新/旧形状共用）：列序见两处查询。
type PcObjectRow = pm_application::repo::kb_pc_query::PcObjectRow;

fn pc_object_json(
    (
        id,
        object_type,
        object_key,
        display_name,
        lifecycle,
        revision_no,
        change_summary,
        content,
        analysis_mode,
        versioned,
    ): PcObjectRow,
) -> serde_json::Value {
    json!({
        "object_id": id,
        "object_type": object_type,
        "object_key": object_key,
        "name": display_name,
        "lifecycle_status": lifecycle,
        "latest_revision": revision_no.map(|no| json!({
            "revision_no": no,
            "summary": change_summary,
            "content": content,
            "analysis_mode": analysis_mode,
            "versioned": versioned.unwrap_or(false),
        })),
    })
}

/// P4（KP-2 落真）：提案详情——契约形状 `{proposal_id, status, governance_level,
/// members[], content_hash, approval_ref?}`，additive 携带提案头（title/rationale/
/// origin_kind/source_ref/expected_version_no…）、审批历史与 `published_version_ref`。
/// 项目成员可读（含 AI——读不改变正式认知）。
async fn p4_proposal(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(proposal_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    let proposal_id =
        Uuid::parse_str(&proposal_id).map_err(|_| ApiError::NotFound("proposal not found"))?;
    let view = pc::load_proposal(&state.pool, project_id, proposal_id).await?;
    let head = &view.head;
    Ok(Json(json!({
        "proposal_id": head.proposal_id,
        "status": head.status,
        "governance_level": head.governance_level,
        "members": view.members,
        "content_hash": head.proposal_content_hash,
        "approval_ref": view.approvals.last().map(|a| pc_ref("approval", a.approval_id)),
        "title": head.title,
        "rationale": head.rationale,
        "origin_kind": head.origin_kind,
        "source_ref": head.source_id.map(|id| pc_ref("source", id)),
        "risk_level": head.risk_level,
        "confidence": head.confidence,
        "expected_version_no": head.expected_version_no,
        "submitted_at": head.submitted_at,
        "created_by": head.created_by,
        "created_at": head.created_at,
        "updated_at": head.updated_at,
        "approvals": view.approvals,
        "published_version_ref": view
            .published_version
            .as_ref()
            .map(|v| pc_ref("cognition_version", v.version_id)),
        "published_version_no": view.published_version.as_ref().map(|v| v.version_no),
    })))
}

/// P5 请求体（契约 `{action, note?, expected_version_no}`；`reason` 为 note 别名）。
#[derive(serde::Deserialize)]
struct DecisionBody {
    action: String,
    #[serde(default, alias = "reason")]
    note: Option<String>,
    #[serde(default)]
    expected_version_no: Option<i64>,
}

/// P5（审阅流 v2）：审批转达（CAS：`expected_version_no` = 调用方确认的提案基线）。
/// approve/reject 只是暂记决定（报告关闭前可互改），`published_version_ref` 恒 null；
/// 正式版本在报告 close 批量发布。治理动作：可信交互上下文（A*：API Key 403）+ 服务层
/// 真人 + A3（AI/服务主体恒 403）；MCP 不暴露此入口。
async fn p5_decide_proposal(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(proposal_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    ensure_trusted_context(&caller)?;
    let proposal_id =
        Uuid::parse_str(&proposal_id).map_err(|_| ApiError::NotFound("proposal not found"))?;
    let body: DecisionBody = serde_json::from_value(body).map_err(|e| {
        ApiError::Text(
            StatusCode::BAD_REQUEST,
            format!("invalid decision body: {e}"),
        )
    })?;
    let action = DecisionAction::parse(body.action.trim())
        .ok_or(ApiError::BadRequest("action must be `approve` or `reject`"))?;
    let expected_version_no = body.expected_version_no.ok_or(ApiError::BadRequest(
        "expected_version_no is required (CAS)",
    ))?;
    let outcome = pc::decide_proposal(
        &state.pool,
        project_id,
        &caller.audit_actor(),
        proposal_id,
        ProposalDecisionInput {
            action,
            reason: body.note,
            expected_version_no,
        },
    )
    .await?;
    let version = outcome.published_version.as_ref();
    Ok(Json(json!({
        "proposal_id": outcome.proposal_id,
        "status": outcome.status,
        "approval_ref": outcome.approval_id.map(|id| pc_ref("approval", id)),
        "published_version_ref": version.map(|v| pc_ref("cognition_version", v.version_id)),
        "published_version_no": version.map(|v| v.version_no),
        "snapshot_hash": version.map(|v| v.snapshot_hash.clone()),
    })))
}

/// P3-11 W6 并行端点（P5 同构；范围确认点②）：ProjC 提案决策转达——决定
/// 终局（Q-02b/Q-11，GAP-35：终态不可跨态重决），正式版本在报告 close 双域分流
/// 批量发布（`projc-decision:{id}`，GAP-31-A 服务端派生）。门禁与 pc P5 一致：
/// 可信交互上下文（A*：API Key 403）+ 服务层真人 + A3；MCP 不暴露此入口。
async fn projc_p5_decide_proposal(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(proposal_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_project_param(&query)?;
    guard(&state, &caller, project_id).await?;
    ensure_trusted_context(&caller)?;
    let proposal_id =
        Uuid::parse_str(&proposal_id).map_err(|_| ApiError::NotFound("proposal not found"))?;
    let body: DecisionBody = serde_json::from_value(body).map_err(|e| {
        ApiError::Text(
            StatusCode::BAD_REQUEST,
            format!("invalid decision body: {e}"),
        )
    })?;
    let action = DecisionAction::parse(body.action.trim())
        .ok_or(ApiError::BadRequest("action must be `approve` or `reject`"))?;
    let expected_version_no = body.expected_version_no.ok_or(ApiError::BadRequest(
        "expected_version_no is required (CAS)",
    ))?;
    let outcome = projc::decide_proposal(
        &state.pool,
        project_id,
        &caller.audit_actor(),
        proposal_id,
        pm_application::projc::ProposalDecisionInput {
            action: match action {
                DecisionAction::Approve => pm_application::projc::DecisionAction::Approve,
                DecisionAction::Reject => pm_application::projc::DecisionAction::Reject,
            },
            reason: body.note,
            expected_version_no,
        },
    )
    .await?;
    Ok(Json(json!({
        "proposal_id": outcome.proposal_id,
        "status": outcome.status,
        "approval_ref": outcome.approval_id.map(|id| projc_ref("approval", id)),
        "published_version_ref": null,
        "published_version_no": null,
        "snapshot_hash": null,
    })))
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

/// `GET /v1/pc/objects/{object_id}?project_id=`：PC 对象详情（最新修订全文 +
/// analysis_mode/versioned）。前端 PC 页签行内展开消费；复用 `pc_object_legacy`
/// 查询与 P3 的 `pc_object_json` 投影（形状一致）。项目边界：非成员/其他项目
/// 404（不泄露跨项目存在性）；坏 uuid 404（同 p4_proposal 口径）。
async fn pc_object_detail(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(object_id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = query
        .get("project_id")
        .and_then(|v| uuid::Uuid::parse_str(v).ok())
        .ok_or(ApiError::BadRequest("project_id is required"))?;
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let object_id =
        Uuid::parse_str(&object_id).map_err(|_| ApiError::NotFound("pc object not found"))?;
    let mut rows = kb_pc_query_repo(&state.pool)
        .pc_object_legacy(project_id, object_id)
        .await?;
    if rows.len() != 1 {
        return Err(ApiError::NotFound("pc object not found"));
    }
    Ok(Json(pc_object_json(rows.pop().expect("len checked"))))
}

/// `GET /v1/pc/objects?project_id=`：项目 PC 认知对象列表（最新修订投影 + 正文
/// 摘要）。只读管理面（前端 PC 页签消费）；`require_project_visible` 同口径。
async fn list_pc_objects(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = query
        .get("project_id")
        .and_then(|v| uuid::Uuid::parse_str(v).ok())
        .ok_or(ApiError::BadRequest("project_id is required"))?;
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let rows: Vec<pm_application::repo::kb_pc_query::PcObjectListRow> =
        kb_pc_query_repo(&state.pool)
            .list_pc_objects(project_id)
            .await?;
    let rows: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "object_id": r.id,
                "object_type": r.object_type,
                "object_key": r.object_key,
                "display_name": r.display_name,
                "lifecycle_status": r.lifecycle_status,
                "revision_no": r.revision_no,
                "change_summary": r.change_summary,
                "excerpt": r.excerpt,
                "updated_at": r.updated_at,
            })
        })
        .collect();
    Ok(Json(json!({ "items": rows })))
}

/// `GET /v1/pc/proposals?project_id=&status=`：项目 PC 提案列表（审批页签供给）。
///
/// 只读管理面；`status` 可选过滤（审批页签默认拉 `submitted` + 全量两段由前端
/// 组合）；决策（approve/reject + CAS）走既有 P5 端点。
async fn list_pc_proposals(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = query
        .get("project_id")
        .and_then(|v| uuid::Uuid::parse_str(v).ok())
        .ok_or(ApiError::BadRequest("project_id is required"))?;
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let status_filter = query.get("status").map(|s| s.trim().to_string());
    let rows: Vec<pm_application::repo::kb_pc_query::PcProposalListRow> =
        kb_pc_query_repo(&state.pool)
            .list_pc_proposals(project_id, status_filter.as_deref())
            .await?;
    let rows: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "proposal_id": r.id,
                "title": r.title,
                "status": r.status,
                "risk_level": r.risk_level,
                "confidence": r.confidence,
                "origin_kind": r.origin_kind,
                "expected_version_no": r.expected_version_no,
                "member_count": r.member_count,
                "submitted_at": r.submitted_at,
                "created_at": r.created_at,
            })
        })
        .collect();
    Ok(Json(json!({ "items": rows })))
}
