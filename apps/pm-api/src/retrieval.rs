//! 统一检索 REST 面（WP-1）。
//!
//! **唯一端点**：`POST /v1/retrieval/query`——`plan` / `execute` / `expand` /
//! `evidence` 四能力是**内部模块边界**，不暴露为独立 REST（裁决 §十：V1 不过早拆
//! 微服务 API；`expand` 公开端点有绕过计划校验直接触发遍历的风险）。
//!
//! **Planner 开关**：`AI_PM_RETRIEVAL_PLANNER`（默认 **off**）——关闭时**不调用
//! LLM**，走确定性双域直查（降级原因 `planner_disabled`）；端点**始终可用**。
//!
//! **授权**：`scope()` 强制 `?project_id=` 必填 + `require_project_visible`——
//! 非成员一律 404（不泄漏项目存在性）。检索是**只读**操作，**不适用** WP-0 的
//! 真人交互式确认门（AI Agent / API Key 可检索；正式确认知识版本仍只允许可信
//! 真人交互）。

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;

use kb_pc_contracts::retrieval::{PlannerModel, RetrievalCaller};
use pm_ai::retrieval_planner::LlmRetrievalPlanner;
use pm_application::retrieval::service::{RetrievalError, RetrievalServiceLive};

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, ApiError, ApiResult};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/v1/retrieval/query", post(retrieval_query))
}

/// 检索请求体。
#[derive(Debug, Deserialize)]
struct RetrievalQueryBody {
    /// 自然语言查询。
    query: String,
}

/// Planner 开关（默认 off；`.10` 栈显式开启）。
fn planner_enabled() -> bool {
    std::env::var("AI_PM_RETRIEVAL_PLANNER")
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "on" | "yes"
            )
        })
        .unwrap_or(false)
}

/// `POST /v1/retrieval/query`：统一检索入口。
async fn retrieval_query(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<RetrievalQueryBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = query
        .get("project_id")
        .and_then(|v| uuid::Uuid::parse_str(v).ok())
        .ok_or(ApiError::BadRequest("project_id is required"))?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;

    let text = body.query.trim();
    if text.is_empty() {
        return Err(ApiError::BadRequest("query must not be empty"));
    }

    // Planner 未启用 → 不注入模型 → 确定性降级（不调用 LLM）。
    let planner: Option<Arc<dyn PlannerModel>> = if planner_enabled() {
        Some(Arc::new(LlmRetrievalPlanner::new(state.pool.clone())))
    } else {
        None
    };
    let service = RetrievalServiceLive::new(state.pool.clone(), planner);

    let retrieval_caller = RetrievalCaller {
        principal_id: caller.principal_id,
        tenant_id: caller.tenant_id,
        project_id,
        auth_mode: caller.auth_mode.clone(),
    };
    let response = service
        .query(text, &retrieval_caller)
        .await
        .map_err(map_error)?;
    Ok(Json(serde_json::json!(response)))
}

/// 错误映射（403/400/404/500）。
fn map_error(err: RetrievalError) -> ApiError {
    use axum::http::StatusCode;
    match err {
        RetrievalError::Forbidden(m) => ApiError::Text(StatusCode::FORBIDDEN, m),
        RetrievalError::Invalid(m) => ApiError::Text(StatusCode::BAD_REQUEST, m),
        RetrievalError::NotFound(m) => ApiError::Text(StatusCode::NOT_FOUND, m),
        RetrievalError::Db(err) => ApiError::Internal(err),
    }
}
