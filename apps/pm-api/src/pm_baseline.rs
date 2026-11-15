//! S01-05 · Project Baseline V1（AI 提案 → 人类逐项确认 → 版本不可变）。
//!
//! 提案条目从 S01-04 已确认知识 + 仓库快照派生（认知 → 基线的唯一管线）。
//! 确认规则（与 S01-05 边界裁决一致）：逐项独立决策；**Confirm Baseline 要求
//! 无 proposed 条目**（未决更正阻塞），deferred/investigation 允许作为
//! documented uncertainties 随基线保留。已确认提案不可变；变更走新提案。
//! 授权同 S01-04：读写都过 require_project_visible（DEF-01 教训）。
//!
//! 数据访问已收口到仓储（任务书批 B）：SQL 在
//! `pm_infrastructure::repo_baseline::PgBaselineRepo`，本模块零 SQL；
//! NotFound/Conflict 域语义由仓储返回，本层只映射状态码与授权门。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{
    require_human_decision_context, require_project_visible, ApiError, ApiResult,
};
use pm_application::repo::{BaselineRepo, BaselineRepoError};
use pm_infrastructure::repo_baseline::PgBaselineRepo;

fn repo(state: &AppState) -> PgBaselineRepo {
    PgBaselineRepo::new(state.pool.clone())
}

fn repo_err(e: BaselineRepoError) -> ApiError {
    match e {
        BaselineRepoError::Db(err) => ApiError::Internal(err),
        BaselineRepoError::Conflict(m) => ApiError::Conflict(m),
        BaselineRepoError::NotFound(m) => ApiError::NotFound(m),
    }
}

/// GET 基线概览：当前已确认版本 + 待确认提案（含条目）+ 历史。
async fn overview(
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

    let current = repo(&state)
        .current_version(caller.tenant_id, project_id)
        .await
        .map_err(repo_err)?;
    let proposal = repo(&state)
        .pending_proposal(caller.tenant_id, project_id)
        .await
        .map_err(repo_err)?;
    let items = match &proposal {
        Some(p) => repo(&state)
            .proposal_items(caller.tenant_id, p.id)
            .await
            .map_err(repo_err)?,
        None => Vec::new(),
    };
    let history = repo(&state)
        .version_history(caller.tenant_id, project_id)
        .await
        .map_err(repo_err)?;

    Ok(Json(json!({
        "current_version": current.as_ref().map(|c| c.version),
        "current_confirmed_at": current.as_ref().map(|c| c.confirmed_at),
        "history": history.iter().map(|h| json!({
            "version": h.version, "proposal_id": h.proposal_id, "confirmed_at": h.confirmed_at,
        })).collect::<Vec<_>>(),
        "proposal": proposal.as_ref().map(|p| json!({
            "id": p.id, "version": p.version, "status": p.status, "created_at": p.created_at,
        })),
        "items": items,
    })))
}

/// POST 生成提案（人类动作）：从已确认知识 + 仓库快照派生条目。
/// 已有 pending 提案 → 409（变更走新提案，须先处理当前提案）。
/// 创建语义 → 201（规格 19 号 R86/TC-P1-06-01 定版；与本仓 create→201 惯例对齐）。
async fn create_proposal(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(err) = require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await
    {
        return err.into_response();
    }

    let created = match repo(&state)
        .create_proposal(caller.tenant_id, project_id, caller.principal_id)
        .await
    {
        Ok(created) => created,
        Err(err) => return repo_err(err).into_response(),
    };

    (
        StatusCode::CREATED,
        Json(json!({
            "proposal_id": created.proposal_id,
            "version": created.version,
            "item_count": created.item_count,
        })),
    )
        .into_response()
}

#[derive(Deserialize)]
struct DecisionBody {
    action: String,
    content: Option<String>,
    note: Option<String>,
}

/// POST 条目逐项决策（同 S01-04 动作集；已决策 409）。
async fn decide_item(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((project_id, item_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<DecisionBody>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    // SEC-P1-1：逐项决策与 cognition 同门（交互式真人 + 查库 kind='human'）；
    // `confirm_baseline` 的三重门继续独立生效（版本正式化）。
    require_human_decision_context(&state.pool, &caller).await?;

    // 动作词汇 → 状态词汇（HTTP 契约；非法动作 400）。
    let next_state = match body.action.as_str() {
        "confirm" => "confirmed",
        "reject" => "rejected",
        "defer" => "deferred",
        "investigate" => "investigation",
        _ => return Err(ApiError::BadRequest("invalid action")),
    };

    let decision = repo(&state)
        .decide_item(
            caller.tenant_id,
            project_id,
            item_id,
            next_state,
            caller.principal_id,
            body.note.as_deref(),
            body.content.as_deref(),
        )
        .await
        .map_err(repo_err)?;

    Ok(Json(json!({ "id": decision.item_id, "state": decision.next_state })))
}

/// POST 确认基线（显式人类动作）：要求提案存在、无 proposed 条目
/// （deferred/investigation 作为 documented uncertainties 保留）；
/// 成功 → 版本头落库 + 提案转 confirmed（条目即不可变）。
///
/// **确认权三重门**（DEF-P0-02 · DR-C2「受保护操作必须真人审批」）：
///
/// 1. **可信人类交互上下文**——`auth_mode='interactive'` 放行；`api_key`
///    （Agent / Service Key）与 `unknown` 一律拒绝：**携带真人 JWT 不等于
///    真人操作**，无法区分真人与 Agent 代持凭据时拒绝确认。`off`（本地开发
///    免鉴权兜底）放行——该模式的主体按构造即真人
///    （`DEV_OFF_PRINCIPAL`，`core.principal.kind='human'`），且生产不进入该模式；
/// 2. **真人主体**——`core.principal.kind='human'`（AI 主体 `ai_pm` / `dev_agent`
///    / `service` 不得确认基线，更不得确认自己产出的提案）；
/// 3. **项目可见性**——非成员 404（不泄漏项目存在性）。
///
/// 该门**不可绕过**：后端强制，不依赖前端按钮是否禁用。
async fn confirm_baseline(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    // ① 身份真实性——纯内存 fail-fast，不开事务（fail-closed）。
    //    interactive = 真人交互会话；off = 本地开发兜底（主体按构造为真人）。
    //    api_key（Agent / Service Key）与 unknown 一律拒绝。
    match caller.auth_mode.as_str() {
        "interactive" | "off" => {}
        _ => {
            return Err(ApiError::Forbidden(
                "baseline confirmation requires an interactive human session",
            ))
        }
    }

    // ② 真人主体——AI 主体（ai_pm / dev_agent / service）不得确认正式基线。
    pm_application::requirement::require_human(&state.pool, &caller.audit_actor())
        .await
        .map_err(|e| match e {
            pm_application::pm_write::PmWriteError::Forbidden(_)
            | pm_application::pm_write::PmWriteError::NotFound(_) => {
                ApiError::Forbidden("baseline confirmation requires a human principal")
            }
            pm_application::pm_write::PmWriteError::Db(err) => ApiError::Internal(err),
            pm_application::pm_write::PmWriteError::ValidationMsg(m) => {
                ApiError::Text(axum::http::StatusCode::INTERNAL_SERVER_ERROR, m)
            }
            pm_application::pm_write::PmWriteError::Validation(m)
            | pm_application::pm_write::PmWriteError::Conflict(m) => {
                ApiError::Text(axum::http::StatusCode::INTERNAL_SERVER_ERROR, m.to_string())
            }
        })?;

    // ③ 项目可见性（非成员 404）。
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;

    let confirmed = repo(&state)
        .confirm_pending(caller.tenant_id, project_id, caller.principal_id)
        .await
        .map_err(repo_err)?;

    Ok(Json(
        json!({ "version": confirmed.version, "status": "confirmed" }),
    ))
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/projects/{project_id}/baseline", get(overview))
        .route(
            "/v1/pm/projects/{project_id}/baseline/proposals",
            post(create_proposal),
        )
        .route(
            "/v1/pm/projects/{project_id}/baseline/items/{item_id}/decision",
            post(decide_item),
        )
        .route(
            "/v1/pm/projects/{project_id}/baseline/confirm",
            post(confirm_baseline),
        )
}
