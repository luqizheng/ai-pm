//! D-5 · 仓库分析报告读面 + 关闭（知识审阅「分析报告」页签 / review-center 卡片）。
//!
//! - 列表/详情：项目成员可读（`require_project_visible`；非成员 404 不泄露存在性）。
//! - 关闭：D-5 人工显式动作——三重门同 KB confirm（interactive 真人 + 成员）；
//!   事务内自动处置未决项（PC 提案 withdrawn / KB 草稿 rejected）并固化 D-1 锚点。
//! - 决策动作本身不在此（KB confirm / PC P4-P5 既有端点）——单一决策源。

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Extension, Json, Router,
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, ApiError, ApiResult};

use pm_application::analysis_report::{
    self, AnalysisReportError, CloseOutcome, ReportDetail, ReportListRow,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/analysis-reports", get(list_reports))
        .route("/v1/pm/analysis-reports/{report_id}", get(get_report))
        .route(
            "/v1/pm/analysis-reports/{report_id}/close",
            post(close_report),
        )
}

impl From<AnalysisReportError> for ApiError {
    fn from(e: AnalysisReportError) -> Self {
        match e {
            AnalysisReportError::NotFound => ApiError::NotFound("analysis report not found"),
            AnalysisReportError::Conflict(m) => ApiError::Text(axum::http::StatusCode::CONFLICT, m),
            AnalysisReportError::Forbidden(m) => {
                ApiError::Text(axum::http::StatusCode::FORBIDDEN, m)
            }
            // G-1 硬失败：projc 源未登记（状态机不一致 = 服务端故障）
            AnalysisReportError::ProjcSourceMissing(_) => ApiError::Text(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            ),
            AnalysisReportError::Db(err) => ApiError::Internal(err),
        }
    }
}

async fn project_scope(
    state: &AppState,
    caller: &AuthedPrincipal,
    query: &HashMap<String, String>,
) -> ApiResult<Uuid> {
    let project_id = query
        .get("project_id")
        .and_then(|v| Uuid::parse_str(v).ok())
        .ok_or(ApiError::BadRequest("project_id is required"))?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    Ok(project_id)
}

/// `GET /v1/pm/analysis-reports?project_id=&repository_id=&status=`：项目分析报告
/// 列表（状态分区供给：analyzing / analyzed 待审理 / closed 历史 / failed）。
async fn list_reports(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = project_scope(&state, &caller, &query).await?;
    let repository_id = query
        .get("repository_id")
        .and_then(|v| Uuid::parse_str(v).ok());
    let status = query.get("status").map(|s| s.trim().to_string());
    let rows: Vec<ReportListRow> =
        analysis_report::list_reports(&state.pool, project_id, repository_id, status.as_deref())
            .await?;
    Ok(Json(json!({ "items": rows })))
}

/// `GET /v1/pm/analysis-reports/{report_id}?project_id=`：报告详情聚合
/// （头 + KB 修订全文 + PC 提案集 + PC 修订集）。
async fn get_report(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(report_id): Path<Uuid>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<ReportDetail>> {
    let project_id = project_scope(&state, &caller, &query).await?;
    let detail = analysis_report::report_detail(&state.pool, project_id, report_id).await?;
    Ok(Json(detail))
}

/// `POST /v1/pm/analysis-reports/{report_id}/close?project_id=`：关闭报告
/// （D-5 门释放动作）。三重门：interactive 真人（服务层 ensure_closable）+
/// 项目成员 + 报告属项目。
async fn close_report(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(report_id): Path<Uuid>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<CloseOutcome>> {
    let project_id = project_scope(&state, &caller, &query).await?;
    let actor = caller.audit_actor();
    analysis_report::ensure_closable(&actor)?;
    let outcome = analysis_report::close_report(&state.pool, project_id, report_id, &actor).await?;
    Ok(Json(outcome))
}
