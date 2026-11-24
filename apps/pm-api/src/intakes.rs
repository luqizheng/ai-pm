//! `/v1/pm/intakes`：Intake 受理与运维端点（P3-2/P3-5；V1.1 §9.3/§12.1/§14.6）。
//!
//! 受理语义（D-3）：**202 仅代表「请求已登记、任务已创建」**——不代表分析
//! 完成、不代表业务结果已产生；可追踪标识（intake_id/task_id）随响应返回，
//! 业务结果经状态查询或事件获取。幂等（D-12）：`Idempotency-Key` 重复提交
//! 返回既有 Intake（replayed=true），零副作用。
//!
//! 权限模型：`require_project_visible`（同租户 + 项目成员，同 outbox_ops
//! 口径）；人工重试写审计（成功与失败均记，同 outbox_ops 纪律）。

use axum::{
    extract::{Extension, Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use std::collections::HashMap;
use uuid::Uuid;

use pm_application::intake::{self, SubmitIntake, TaskSpec};

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::platform_ops_repo;
use crate::support::{require_project_visible, ApiError, ApiResult};
use pm_application::repo::platform_ops::PlatformOpsRepo;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/intakes", post(submit_handler))
        .route("/v1/pm/intakes/{intake_id}", get(state_handler))
        .route(
            "/v1/pm/intakes/{intake_id}/tasks/{task_id}/retry",
            post(retry_handler),
        )
}

#[derive(Debug, serde::Deserialize)]
struct SubmitIntakeBody {
    intake_type: String,
    channel: String,
    source_ref: serde_json::Value,
    #[serde(default)]
    content_hash: Option<String>,
    #[serde(default)]
    title: Option<String>,
    /// 显式任务覆盖；缺省按 intake_type 路由矩阵派生（§10.1 主路由）。
    #[serde(default)]
    tasks: Option<Vec<SubmitTaskBody>>,
}

#[derive(Debug, serde::Deserialize)]
struct SubmitTaskBody {
    target_service: String,
    task_type: String,
    #[serde(default = "default_phase")]
    phase: String,
}

fn default_phase() -> String {
    "required".to_string()
}

async fn submit_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    Json(body): Json<SubmitIntakeBody>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    // 项目上下文三通道（与 `/v1/pm/*` 惯例一致，query 优先）：
    // ?project_id= → X-Project-Id header → source_ref.project_id。
    let project_id = query
        .get("project_id")
        .and_then(|v| Uuid::parse_str(v).ok())
        .or_else(|| {
            headers
                .get("x-project-id")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| Uuid::parse_str(v).ok())
        })
        .or_else(|| {
            body.source_ref
                .get("project_id")
                .and_then(|v| v.as_str())
                .and_then(|v| Uuid::parse_str(v).ok())
        })
        .ok_or_else(|| {
            ApiError::BadRequest(
                "project context required (?project_id= or X-Project-Id or source_ref.project_id)",
            )
        })?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;

    // 幂等键：HTTP `Idempotency-Key` header（D-12）。
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let tasks = body.tasks.map(|list| {
        list.into_iter()
            .map(|t| TaskSpec {
                target_service: t.target_service,
                task_type: t.task_type,
                phase: if t.phase == "optional" {
                    "optional".to_string()
                } else {
                    "required".to_string()
                },
            })
            .collect::<Vec<_>>()
    });

    let receipt = intake::submit_intake(
        &state.pool,
        SubmitIntake {
            project_id,
            tenant_id: caller.tenant_id,
            submitted_by: caller.principal_id,
            intake_type: body.intake_type,
            channel: body.channel,
            source_ref: body.source_ref,
            content_hash: body.content_hash,
            title: body.title,
            idempotency_key,
            tasks,
        },
    )
    .await?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(serde_json::to_value(&receipt).unwrap_or_else(|_| serde_json::json!({}))),
    ))
}

/// 状态校正查询（P3-4 聚合 + P3-5）：聚合状态 = 查询时派生（§9.4.2）。
async fn state_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(intake_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let view = intake_view(&state, intake_id, &caller).await?;
    Ok(Json(
        serde_json::to_value(view).unwrap_or_else(|_| serde_json::json!({})),
    ))
}

/// 人工重试（P3-5，ADR-12）：FAILED / NEEDS_HUMAN_REVIEW 任务复用原行
/// attempt+1 重置 PENDING 并重写派发事件；禁止新建任务行重试。
async fn retry_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((intake_id, task_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    let view = intake_view(&state, intake_id, &caller).await?;
    let retry = intake::retry_task(&state.pool, intake_id, task_id).await?;
    // 重试审计：留痕（操作者/任务/结果）。
    audit_retry(&state, &caller, view.intake_id, task_id).await;
    Ok(Json(
        serde_json::to_value(retry).unwrap_or_else(|_| serde_json::json!({})),
    ))
}

/// 归属校验 + Intake 视图（跨项目 intake 统一 404，不泄露存在性）。
async fn intake_view(
    state: &AppState,
    intake_id: Uuid,
    caller: &AuthedPrincipal,
) -> ApiResult<intake::IntakeState> {
    // 先取 intake 归属项目再过可见性（intake_state 本身不做租户过滤）。
    let project_id = platform_ops_repo(&state.pool)
        .intake_project_id(caller.tenant_id, intake_id)
        .await?;
    let project_id = project_id.ok_or(ApiError::NotFound("intake not found"))?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    intake::intake_state(&state.pool, intake_id)
        .await
        .map_err(|e| match e {
            intake::IntakeError::NotFound => ApiError::NotFound("intake not found"),
            other => ApiError::from(other),
        })
}

async fn audit_retry(state: &AppState, caller: &AuthedPrincipal, intake_id: Uuid, task_id: Uuid) {
    let actor = caller.audit_actor();
    if let Err(e) = platform_ops_repo(&state.pool)
        .audit_intake_retry(
            intake_id,
            &actor,
            task_id,
            serde_json::json!({ "retry": "attempt+1", "reuse_row": true }),
        )
        .await
    {
        tracing::error!(error = %e, %task_id, "intake retry audit write failed");
    }
}

impl From<intake::IntakeError> for ApiError {
    fn from(e: intake::IntakeError) -> Self {
        match e {
            intake::IntakeError::NotFound => ApiError::NotFound("intake not found"),
            intake::IntakeError::TaskNotFound(_) => ApiError::NotFound("intake task not found"),
            intake::IntakeError::NotRetryable(_, _) => {
                ApiError::Conflict("intake task is not retryable")
            }
            intake::IntakeError::Validation(m) => ApiError::BadRequest(
                // &'static str 限制下的静态化：动态消息截断进日志，响应给固定语。
                {
                    tracing::warn!(message = %m, "intake validation rejected");
                    "invalid intake request"
                },
            ),
            intake::IntakeError::Db(err) => ApiError::Internal(err),
        }
    }
}
