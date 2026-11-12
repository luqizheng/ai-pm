//! S03 · 设计与技术决策 REST 适配层。
//!
//! 统一服务（`pm_application::design`）的薄适配：业务裁决零实现，只做
//! （a）`require_project_visible` 成员闸（DEF-01 六点的 REST 面）、
//! （b）确认类命令的可信交互上下文判定（`ensure_trusted_context`：api_key
//! 鉴权拒绝，§7.2 A*——AI 主体在此被拦，批准无旁路）、（c）错误映射。
//!
//! 设计修改与设计审批是两件事（总体设计 §2/§5）：草稿/提交/评审/请求批准
//! 不限主体（AI 允许，来源留痕 `source='ai'`）；decide / freeze / supersede /
//! deprecate / blocking 是确认类命令，一律真人门。

use std::collections::HashMap;

use axum::{
    extract::{Extension, Path, Query, State},
    routing::{get, patch, post},
    Json, Router,
};
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::design_guard_repo;
use pm_application::repo::DesignGuardRepo;
use crate::support::{ensure_trusted_context, require_project_visible, ApiError, ApiResult};
use pm_application::design::{
    self, AddFindingRequest, CreateAdrRequest, CreateDesignRequest, DecideApprovalRequest,
    DraftAdrRevisionRequest, DraftDesignRevisionRequest, FreezeBaselineRequest,
    LinkAdrDesignRequest, LinkAdrRequirementRequest, LinkDesignRequirementRequest,
    ResolveFindingRequest, SetBlockingRequest, SupersedeAdrRequest, SupersedeBaselineRequest,
};

/// 设计级端点公共前置：租户边界（越权/不存在一律 404）+ 项目成员可见性。
async fn require_design_visible(
    state: &AppState,
    design_id: Uuid,
    principal: &AuthedPrincipal,
) -> ApiResult<Uuid> {
    let project_id = design_guard_repo(&state.pool)
        .design_project_id(principal.tenant_id, design_id)
        .await?
        .ok_or(ApiError::NotFound("design not found"))?;
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    Ok(project_id)
}

async fn require_adr_visible(
    state: &AppState,
    adr_id: Uuid,
    principal: &AuthedPrincipal,
) -> ApiResult<Uuid> {
    let project_id = design_guard_repo(&state.pool)
        .adr_project_id(principal.tenant_id, adr_id)
        .await?
        .ok_or(ApiError::NotFound("adr not found"))?;
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    Ok(project_id)
}

async fn require_revision_visible(
    state: &AppState,
    design_id: Uuid,
    revision_id: Uuid,
    principal: &AuthedPrincipal,
) -> ApiResult<()> {
    require_design_visible(state, design_id, principal).await?;
    if !design_guard_repo(&state.pool)
        .revision_exists(principal.tenant_id, design_id, revision_id)
        .await?
    {
        return Err(ApiError::NotFound("design revision not found"));
    }
    Ok(())
}

// ────────────────────────────────────────────────── Technical Design

async fn list_designs(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let items = design::list_designs(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn create_design(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateDesignRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let detail = design::create_design(
        &state.pool,
        caller.tenant_id,
        project_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!(detail)),
    ))
}

async fn get_design(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(design_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_design_visible(&state, design_id, &caller).await?;
    let detail = design::design_detail(&state.pool, caller.tenant_id, design_id).await?;
    Ok(Json(serde_json::json!(detail)))
}

async fn create_revision(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(design_id): Path<Uuid>,
    Json(body): Json<DraftDesignRevisionRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_design_visible(&state, design_id, &caller).await?;
    let revision = design::create_design_revision(
        &state.pool,
        caller.tenant_id,
        design_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "revision": revision })),
    ))
}

async fn update_revision_draft(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((design_id, revision_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<DraftDesignRevisionRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    require_revision_visible(&state, design_id, revision_id, &caller).await?;
    let revision = design::update_design_revision_draft(
        &state.pool,
        caller.tenant_id,
        design_id,
        revision_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "revision": revision })))
}

async fn submit_revision(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((design_id, revision_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_revision_visible(&state, design_id, revision_id, &caller).await?;
    let revision = design::submit_design_revision(
        &state.pool,
        caller.tenant_id,
        design_id,
        revision_id,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "revision": revision })))
}

// ─────────────────────────────────────────── Design ↔ Requirement 关联

async fn list_revision_requirements(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((design_id, revision_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_revision_visible(&state, design_id, revision_id, &caller).await?;
    let items =
        design::list_design_requirements(&state.pool, caller.tenant_id, design_id, revision_id)
            .await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn link_requirement(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((design_id, revision_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<LinkDesignRequirementRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_revision_visible(&state, design_id, revision_id, &caller).await?;
    let link = design::link_design_requirement(
        &state.pool,
        caller.tenant_id,
        design_id,
        revision_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!(link)),
    ))
}

async fn unlink_requirement(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((design_id, revision_id, requirement_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<axum::http::StatusCode> {
    require_revision_visible(&state, design_id, revision_id, &caller).await?;
    design::unlink_design_requirement(
        &state.pool,
        caller.tenant_id,
        design_id,
        revision_id,
        requirement_id,
        &caller.audit_actor(),
    )
    .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─────────────────────────────────────────────── 设计评审（问题登记）

async fn list_findings(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((design_id, revision_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_revision_visible(&state, design_id, revision_id, &caller).await?;
    let items = design::list_review_findings(&state.pool, caller.tenant_id, revision_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn add_finding(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((design_id, revision_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<AddFindingRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_revision_visible(&state, design_id, revision_id, &caller).await?;
    let finding = design::add_review_finding(
        &state.pool,
        caller.tenant_id,
        design_id,
        revision_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "finding": finding })),
    ))
}

async fn close_finding(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((design_id, revision_id, finding_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(body): Json<ResolveFindingRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    require_revision_visible(&state, design_id, revision_id, &caller).await?;
    let finding = design::resolve_review_finding(
        &state.pool,
        caller.tenant_id,
        finding_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "finding": finding })))
}

// ───────────────────────────────────────────── 专表审批（真人门）

async fn request_design_approval(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((design_id, revision_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_revision_visible(&state, design_id, revision_id, &caller).await?;
    let approval = design::request_design_revision_approval(
        &state.pool,
        caller.tenant_id,
        design_id,
        revision_id,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "approval": approval })),
    ))
}

async fn request_adr_approval(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(adr_id): Path<Uuid>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_adr_visible(&state, adr_id, &caller).await?;
    let approval =
        design::request_adr_approval(&state.pool, caller.tenant_id, adr_id, &caller.audit_actor())
            .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "approval": approval })),
    ))
}

/// 审批裁决——确认类命令：真人主体 + 可信交互上下文双闸（A*；SC-05）。
async fn decide_approval(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(approval_id): Path<Uuid>,
    Json(body): Json<DecideApprovalRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // 审批行的项目可见性先行（防跨项目探测：不存在与不可见同形 404）。
    let project_id = design_guard_repo(&state.pool)
        .approval_project_id(caller.tenant_id, approval_id)
        .await?
        .ok_or(ApiError::NotFound("approval not found"))?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    ensure_trusted_context(&caller)?;
    let approval = design::decide_approval(
        &state.pool,
        caller.tenant_id,
        approval_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "approval": approval })))
}

// ─────────────────────────────────────────────────────────────── ADR

async fn list_adrs(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let items = design::list_adrs(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn create_adr(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateAdrRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let detail = design::create_adr(
        &state.pool,
        caller.tenant_id,
        project_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!(detail)),
    ))
}

async fn get_adr(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(adr_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_adr_visible(&state, adr_id, &caller).await?;
    let detail = design::adr_detail(&state.pool, caller.tenant_id, adr_id).await?;
    Ok(Json(serde_json::json!(detail)))
}

async fn create_adr_revision(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(adr_id): Path<Uuid>,
    Json(body): Json<DraftAdrRevisionRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_adr_visible(&state, adr_id, &caller).await?;
    let revision = design::create_adr_revision(
        &state.pool,
        caller.tenant_id,
        adr_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "revision": revision })),
    ))
}

async fn submit_adr_revision(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((adr_id, revision_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_adr_visible(&state, adr_id, &caller).await?;
    let detail = design::submit_adr_revision(
        &state.pool,
        caller.tenant_id,
        adr_id,
        revision_id,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!(detail)))
}

async fn link_adr_design(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((adr_id, revision_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<LinkAdrDesignRequest>,
) -> ApiResult<axum::http::StatusCode> {
    require_adr_visible(&state, adr_id, &caller).await?;
    design::link_adr_design_revision(
        &state.pool,
        caller.tenant_id,
        adr_id,
        revision_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn link_adr_requirement(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((adr_id, revision_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<LinkAdrRequirementRequest>,
) -> ApiResult<axum::http::StatusCode> {
    require_adr_visible(&state, adr_id, &caller).await?;
    design::link_adr_requirement(
        &state.pool,
        caller.tenant_id,
        adr_id,
        revision_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// 决策替代——确认类命令：真人门。
async fn supersede_adr(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(adr_id): Path<Uuid>,
    Json(body): Json<SupersedeAdrRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    require_adr_visible(&state, adr_id, &caller).await?;
    ensure_trusted_context(&caller)?;
    let entry = design::supersede_adr(
        &state.pool,
        caller.tenant_id,
        adr_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "adr": entry })))
}

/// 废弃 ADR——确认类命令：真人门。
async fn deprecate_adr(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(adr_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_adr_visible(&state, adr_id, &caller).await?;
    ensure_trusted_context(&caller)?;
    let entry =
        design::deprecate_adr(&state.pool, caller.tenant_id, adr_id, &caller.audit_actor()).await?;
    Ok(Json(serde_json::json!({ "adr": entry })))
}

/// 设置实施阻塞标记——影响 Readiness Gate 判定输入，真人门。
async fn set_implementation_blocking(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(adr_id): Path<Uuid>,
    Json(body): Json<SetBlockingRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    require_adr_visible(&state, adr_id, &caller).await?;
    ensure_trusted_context(&caller)?;
    let entry = design::set_adr_implementation_blocking(
        &state.pool,
        caller.tenant_id,
        adr_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "adr": entry })))
}

// ───────────────────────────────────── Implementation Baseline（Q5）

/// Readiness Gate 五条件 dry-run 判定（**不改任何状态**）。
///
/// 纯读语义（POST 不当用）：查询参数 `design_revision_ids` 为逗号分隔的
/// 设计版本 id 列表。缺省 = 本项目全部已批准的设计版本，即「当前整体是否
/// 可进入 S04」的默认判定。
async fn implementation_readiness(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let requested: Vec<Uuid> = params
        .get("design_revision_ids")
        .map(|raw| {
            raw.split(',')
                .filter_map(|part| Uuid::parse_str(part.trim()).ok())
                .collect()
        })
        .unwrap_or_default();
    // 缺省 = 项目内全部已批准的设计版本（与「当前整体就绪」口径一致）。
    let design_revision_ids = if requested.is_empty() {
        design_guard_repo(&state.pool)
            .approved_revision_ids(caller.tenant_id, project_id)
            .await?
    } else {
        requested
    };
    if design_revision_ids.is_empty() {
        // 无已批准版本 = 门禁必然不通过；返回空判定而非报错（前端据此显示空态）。
        return Ok(Json(serde_json::json!(design::GateReport {
            ready: false,
            design_revision_ids: Vec::new(),
            conditions: Vec::new(),
        })));
    }
    let report = design::evaluate_readiness(
        &state.pool,
        caller.tenant_id,
        project_id,
        &design_revision_ids,
    )
    .await?;
    Ok(Json(serde_json::json!(report)))
}

/// 冻结实施基线——确认类命令：真人门 + 门禁五条件全过。
async fn freeze_baseline(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<FreezeBaselineRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    ensure_trusted_context(&caller)?;
    let baseline = design::freeze_baseline(
        &state.pool,
        caller.tenant_id,
        project_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "baseline": baseline })),
    ))
}

async fn list_baselines(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let items = design::list_baselines(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

/// 项目级审批收件箱（S03-05）：待办 + 历史，按「待办优先、时间倒序」排。
async fn list_approvals(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let items = design::list_approvals(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

/// S03 工作空间项目级聚合（S03 四页共用上下文与计数）。
async fn design_workspace(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let workspace = design::design_workspace(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(serde_json::json!(workspace)))
}

/// S03-01/02 分析页：每个设计的需求覆盖与影响关系。
async fn design_analysis(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let items = design::design_analysis(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

/// 基线 → 任务交接事实（S03-06 交接 S04）。
async fn baseline_tasks(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(baseline_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = design_guard_repo(&state.pool)
        .baseline_project_id(caller.tenant_id, baseline_id)
        .await?
        .ok_or(ApiError::NotFound("baseline not found"))?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let items = design::baseline_tasks(&state.pool, caller.tenant_id, baseline_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn get_baseline(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(baseline_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = design_guard_repo(&state.pool)
        .baseline_project_id(caller.tenant_id, baseline_id)
        .await?
        .ok_or(ApiError::NotFound("baseline not found"))?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let baseline = design::baseline_detail(&state.pool, caller.tenant_id, baseline_id).await?;
    Ok(Json(serde_json::json!({ "baseline": baseline })))
}

/// 显式替代基线——确认类命令：真人门。
async fn supersede_baseline(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(baseline_id): Path<Uuid>,
    Json(body): Json<SupersedeBaselineRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = design_guard_repo(&state.pool)
        .baseline_project_id(caller.tenant_id, baseline_id)
        .await?
        .ok_or(ApiError::NotFound("baseline not found"))?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    ensure_trusted_context(&caller)?;
    let baseline = design::supersede_baseline(
        &state.pool,
        caller.tenant_id,
        baseline_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "baseline": baseline })))
}

// ─────────────────────────────────────────────────────────────── 路由

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/designs",
            get(list_designs).post(create_design),
        )
        .route("/v1/pm/designs/{design_id}", get(get_design))
        .route(
            "/v1/pm/designs/{design_id}/revisions",
            post(create_revision),
        )
        .route(
            "/v1/pm/designs/{design_id}/revisions/{revision_id}",
            patch(update_revision_draft),
        )
        .route(
            "/v1/pm/designs/{design_id}/revisions/{revision_id}/submit",
            post(submit_revision),
        )
        .route(
            "/v1/pm/designs/{design_id}/revisions/{revision_id}/requirements",
            get(list_revision_requirements).post(link_requirement),
        )
        .route(
            "/v1/pm/designs/{design_id}/revisions/{revision_id}/requirements/{requirement_id}",
            axum::routing::delete(unlink_requirement),
        )
        .route(
            "/v1/pm/designs/{design_id}/revisions/{revision_id}/findings",
            get(list_findings).post(add_finding),
        )
        .route(
            "/v1/pm/designs/{design_id}/revisions/{revision_id}/findings/{finding_id}/close",
            post(close_finding),
        )
        .route(
            "/v1/pm/designs/{design_id}/revisions/{revision_id}/approvals",
            post(request_design_approval),
        )
        .route(
            "/v1/pm/design-approvals/{approval_id}/decide",
            post(decide_approval),
        )
        .route(
            "/v1/pm/projects/{project_id}/adrs",
            get(list_adrs).post(create_adr),
        )
        .route("/v1/pm/adrs/{adr_id}", get(get_adr))
        .route("/v1/pm/adrs/{adr_id}/revisions", post(create_adr_revision))
        .route(
            "/v1/pm/adrs/{adr_id}/revisions/{revision_id}/submit",
            post(submit_adr_revision),
        )
        .route(
            "/v1/pm/adrs/{adr_id}/revisions/{revision_id}/designs",
            post(link_adr_design),
        )
        .route(
            "/v1/pm/adrs/{adr_id}/revisions/{revision_id}/requirements",
            post(link_adr_requirement),
        )
        .route("/v1/pm/adrs/{adr_id}/approvals", post(request_adr_approval))
        .route("/v1/pm/adrs/{adr_id}/supersede", post(supersede_adr))
        .route("/v1/pm/adrs/{adr_id}/deprecate", post(deprecate_adr))
        .route(
            "/v1/pm/adrs/{adr_id}/implementation-blocking",
            post(set_implementation_blocking),
        )
        .route(
            "/v1/pm/projects/{project_id}/implementation-readiness",
            get(implementation_readiness),
        )
        .route(
            "/v1/pm/projects/{project_id}/implementation-baselines",
            get(list_baselines).post(freeze_baseline),
        )
        .route(
            "/v1/pm/implementation-baselines/{baseline_id}",
            get(get_baseline),
        )
        .route(
            "/v1/pm/implementation-baselines/{baseline_id}/tasks",
            get(baseline_tasks),
        )
        // S03 工作空间读面（前端四页共用；2026-11-10 接线批）。
        .route(
            "/v1/pm/projects/{project_id}/design-approvals",
            get(list_approvals),
        )
        .route(
            "/v1/pm/projects/{project_id}/design-workspace",
            get(design_workspace),
        )
        .route(
            "/v1/pm/projects/{project_id}/design-analysis",
            get(design_analysis),
        )
        .route(
            "/v1/pm/implementation-baselines/{baseline_id}/supersede",
            post(supersede_baseline),
        )
}
