//! `/v1/pm/acceptance-overview` 与 `/v1/pm/tasks/{task_id}/{review-findings,criterion-verdicts,review-approvals}`：
//! 项目交付验收工作台的只读端点（T-070 / `docs/ai-pm-delivery-acceptance-workbench-v1.md` §5.1 / §5.3）。
//!
//! - 第一屏：项目级聚合读模型——Q1 状态过滤 + Q2 后端统一判定徽标 + 动作区状态。
//! - 第二层：验收证据（findings / 逐条裁决 / 审批历史）。多仓交付清单复用
//!   `/v1/pm/tasks/{task_id}/delivery`（零改动）；验收动作复用
//!   `POST /v1/pm/review-approvals/{approval_id}/decision`（Q3，零新增写路径）。
//!
//! 可见性沿用 T-022：项目 / 任务不存在、跨租户或调用者非项目成员一律 404。

use axum::{
    extract::{Extension, Path, Query, State},
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::pm_reviews::ApprovalRow;
use crate::routes::AppState;
use crate::support::{require_project_visible, require_task_project, ApiError, ApiResult};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/acceptance-overview", get(overview_handler))
        .route(
            "/v1/pm/tasks/{task_id}/review-findings",
            get(review_findings_handler),
        )
        .route(
            "/v1/pm/tasks/{task_id}/criterion-verdicts",
            get(criterion_verdicts_handler),
        )
        .route(
            "/v1/pm/tasks/{task_id}/review-approvals",
            get(review_approvals_handler),
        )
}

/// `GET /v1/pm/acceptance-overview?project_id=`：待验收总表（§5.1）。
async fn overview_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<pm_application::AcceptanceOverview>> {
    let project_id = params
        .get("project_id")
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(ApiError::BadRequest("project_id is required"))?;
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let overview =
        pm_application::load_overview(&state.pool, project_id, principal.tenant_id).await?;
    Ok(Json(overview))
}

/// 一条 review finding（`delivery.review_finding` ⋈ 所属 review run）。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub(crate) struct ReviewFindingRow {
    pub id: Uuid,
    pub review_run_id: Uuid,
    pub review_attempt_no: i32,
    pub review_run_status: String,
    pub criterion_id: Option<Uuid>,
    pub kind: String,
    pub severity: String,
    pub code_location: Option<String>,
    pub message: String,
    pub evidence_ref: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// `GET /v1/pm/tasks/{task_id}/review-findings`：该任务全部 findings，`blocker` 置顶。
async fn review_findings_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_task_project(&state.pool, task_id, &principal).await?;
    let rows: Vec<ReviewFindingRow> = sqlx::query_as(
        "SELECT f.id, f.review_run_id, r.attempt_no AS review_attempt_no, \
                r.status AS review_run_status, f.criterion_id, f.finding_kind AS kind, \
                f.severity, f.code_location, f.message, f.evidence_ref, f.created_at \
           FROM delivery.review_finding f \
           JOIN delivery.task_run r ON r.id = f.review_run_id \
          WHERE r.task_id = $1 AND r.project_id = $2 AND r.run_kind = 'review' \
          ORDER BY CASE f.severity WHEN 'blocker' THEN 0 WHEN 'warning' THEN 1 ELSE 2 END, \
                   r.attempt_no DESC, f.created_at DESC, f.id",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({
        "task_id": task_id,
        "project_id": project_id,
        "items": rows,
        "total": rows.len(),
    })))
}

/// 验收标准逐条裁决（当前快照）：标准正文 + 该标准**最新一次成功 review run** 的裁决。
///
/// 从未被任何成功 review run 覆盖的标准 `evaluated=false`、`verdict=null`——
/// 不造 `awaiting_manual` 假值（§5.3 口径裁定）。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub(crate) struct CriterionVerdictRow {
    pub criterion_id: Uuid,
    pub ordinal: i32,
    pub text: String,
    pub verification_kind: String,
    pub evaluated: bool,
    pub review_run_id: Option<Uuid>,
    pub review_attempt_no: Option<i32>,
    pub verdict: Option<String>,
    pub evidence: Option<serde_json::Value>,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<chrono::DateTime<chrono::Utc>>,
    pub verdict_created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// `GET /v1/pm/tasks/{task_id}/criterion-verdicts`。
async fn criterion_verdicts_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_task_project(&state.pool, task_id, &principal).await?;
    let rows: Vec<CriterionVerdictRow> = sqlx::query_as(
        "SELECT c.id AS criterion_id, c.ordinal, c.text, c.verification_kind, \
                (v.id IS NOT NULL) AS evaluated, \
                v.review_run_id, v.review_attempt_no, v.verdict, v.evidence, \
                v.decided_by, v.decided_at, v.created_at AS verdict_created_at \
           FROM delivery.task_acceptance_criterion c \
           LEFT JOIN LATERAL ( \
                SELECT v.id, v.review_run_id, r.attempt_no AS review_attempt_no, v.verdict, \
                       v.evidence, v.decided_by, v.decided_at, v.created_at \
                  FROM delivery.acceptance_criterion_verdict v \
                  JOIN delivery.task_run r ON r.id = v.review_run_id \
                 WHERE v.criterion_id = c.id \
                   AND r.run_kind = 'review' AND r.status = 'succeeded' \
                 ORDER BY r.attempt_no DESC, r.created_at DESC \
                 LIMIT 1 \
           ) v ON true \
          WHERE c.task_id = $1 AND c.project_id = $2 \
          ORDER BY c.ordinal",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({
        "task_id": task_id,
        "project_id": project_id,
        "items": rows,
        "total": rows.len(),
    })))
}

/// `GET /v1/pm/tasks/{task_id}/review-approvals`：该任务审批历史（含 pending），
/// 与 `/v1/pm/review-approvals` 同一读模型的 task 级投影（§5.3）。
async fn review_approvals_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_task_project(&state.pool, task_id, &principal).await?;
    let rows: Vec<ApprovalRow> = sqlx::query_as(
        "SELECT a.id, a.task_id, a.review_run_id, a.delivery_run_id, a.kind, a.status, \
                a.decided_by, a.decision_reason, a.decided_at, a.created_at, t.title AS task_title \
           FROM governance.review_approval a \
           JOIN delivery.task t ON t.id = a.task_id \
          WHERE a.task_id = $1 AND a.project_id = $2 \
          ORDER BY a.created_at DESC, a.id",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({
        "task_id": task_id,
        "project_id": project_id,
        "items": rows,
        "total": rows.len(),
    })))
}
