//! `/v1/pm/task-runs`：任务执行 Run 的只读列表与单 Run 详情（spec §19.5 / §20；T-070 §5.2）。
//!
//! 自 Slice C 起 `TaskReady` handler 会为可调度任务创建 `delivery.task_run`；
//! 这里按项目列出 Run 及其失败摘要，供「AI 执行中心」展示真实执行数据。
//!
//! T-070 增补（纯读扩展，不改既有字段）：
//! - 列表补执行上下文列（`run_kind` / `agent_id` / `executor_kind` / `executor_session` /
//!   `lease_expires_at`）与两个可选过滤参数 `task_id`、`run_kind`（attempt_no 自 0026
//!   起按 run_kind 分轨编号，`delivery` / `review` 分轨展示）。
//! - `GET /v1/pm/task-runs/{run_id}`：租约 / 执行器 / 阶段时间线 + 本 Run 锁定的各仓基线
//!   （`task_run_repository`）。**不读日志内容**：日志在执行端磁盘，V1 只给
//!   `executor_session` 与 `workspace_path` 定位信息（设计稿 §二非目标）。
//!
//! 工具调用 / 日志 / 代码变化等执行细节需要 Codex/MCP 执行器（Slice D），当前
//! 无数据源，前端对这些区域显示空态而非虚构数据。

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{paging, project_filter, require_project_visible, ApiError, ApiResult};
use serde_json::json;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/task-runs", get(task_runs))
        .route("/v1/pm/task-runs/{run_id}", get(task_run_detail))
        // ACP-07（0054）：Deadline 过期重授权（UI 快捷入口；Lead/Approver 守卫
        // = 租户 admin 或项目 owner；独立审计 + 独立有效期，不改原 deadline）。
        .route(
            "/v1/pm/task-runs/{run_id}/execution-authorization",
            axum::routing::post(grant_authorization_handler).delete(revoke_authorization_handler),
        )
}

/// ACP-07 守卫：run 存在于调用者租户 + 授权者为其租户 admin 或项目 owner。
async fn ensure_authorization_manager(
    state: &AppState,
    principal: &AuthedPrincipal,
    run_id: Uuid,
) -> Result<(Uuid, Uuid, bool), Response> {
    let row: Option<(Uuid, Uuid, Option<String>)> = sqlx::query_as(
        "SELECT r.project_id, r.tenant_id, tm.role            FROM delivery.task_run r            LEFT JOIN core.tenant_member tm              ON tm.tenant_id = r.tenant_id AND tm.principal_id = $2           WHERE r.id = $1 AND r.tenant_id = $2",
    )
    .bind(run_id)
    .bind(principal.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "code": "internal", "message": e.to_string() })),
        )
            .into_response()
    })?;
    let Some((project_id, _tenant, role)) = row else {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({ "code": "run_not_found", "message": "run not found" })),
        )
            .into_response());
    };
    // 项目 owner 判定：core.project_member owner 行。
    let is_project_owner: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.project_member           WHERE project_id = $1 AND principal_id = $2 AND role = 'owner')",
    )
    .bind(project_id)
    .bind(principal.principal_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "code": "internal", "message": e.to_string() })),
        )
            .into_response()
    })?;
    let is_admin = role.as_deref() == Some("owner") || role.as_deref() == Some("admin");
    if !(is_admin || is_project_owner) {
        return Err((
            axum::http::StatusCode::FORBIDDEN,
            Json(json!({ "code": "FORBIDDEN", "message": "lead or approver required" })),
        )
            .into_response());
    }
    Ok((project_id, run_id, is_admin))
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct GrantAuthorizationBody {
    pub reason: String,
    pub new_deadline_at: chrono::DateTime<chrono::Utc>,
}

async fn grant_authorization_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(run_id): Path<Uuid>,
    Json(body): Json<GrantAuthorizationBody>,
) -> Response {
    let (project_id, _, _is_admin) =
        match ensure_authorization_manager(&state, &principal, run_id).await {
            Ok(v) => v,
            Err(resp) => return resp,
        };
    let actor = principal.audit_actor();
    let id = pm_application::acp06_07::grant_execution_authorization(
        &state.pool,
        principal.tenant_id,
        project_id,
        run_id,
        None,
        principal.principal_id,
        &body.reason,
        body.new_deadline_at,
        &actor,
    )
    .await;
    match id {
        Ok(id) => (
            StatusCode::CREATED,
            Json(json!({ "authorization_id": id, "run_id": run_id, "new_deadline_at": body.new_deadline_at })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "code": "internal", "message": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RevokeAuthorizationPath {
    pub authorization_id: Uuid,
}

async fn revoke_authorization_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(run_id): Path<Uuid>,
    Json(body): Json<RevokeAuthorizationPath>,
) -> Response {
    let (project_id, _, _) = match ensure_authorization_manager(&state, &principal, run_id).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let actor = principal.audit_actor();
    match pm_application::acp06_07::revoke_execution_authorization(
        &state.pool,
        principal.tenant_id,
        project_id,
        run_id,
        body.authorization_id,
        &actor,
    )
    .await
    {
        Ok(revoked) => {
            Json(json!({ "authorization_id": body.authorization_id, "revoked": revoked }))
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "code": "internal", "message": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub(crate) struct TaskRunRow {
    pub id: Uuid,
    pub task_id: Uuid,
    pub task_title: String,
    pub attempt_no: i32,
    pub run_kind: String,
    pub status: String,
    pub outcome: Option<String>,
    pub agent_id: Option<Uuid>,
    pub executor_kind: Option<String>,
    pub executor_session: Option<String>,
    pub lease_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub failure_category: Option<String>,
    pub failure_reason: Option<String>,
}

/// `run_kind` 过滤参数：只认 CHECK 枚举两值，非法值视为未传（与 `project_filter` 同口径）。
fn run_kind_filter(params: &HashMap<String, String>) -> Option<String> {
    params
        .get("run_kind")
        .filter(|v| matches!(v.as_str(), "delivery" | "review"))
        .cloned()
}

async fn task_runs(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let project_id = project_filter(&params);
    let task_id = params.get("task_id").and_then(|v| Uuid::parse_str(v).ok());
    let run_kind = run_kind_filter(&params);
    if let Some(project_id) = project_id {
        require_project_visible(
            &state.pool,
            project_id,
            caller.tenant_id,
            caller.principal_id,
        )
        .await?;
    }
    let items: Vec<TaskRunRow> = sqlx::query_as(
        "SELECT r.id, r.task_id, t.title AS task_title, r.attempt_no, r.run_kind, r.status, \
                r.outcome, r.agent_id, r.executor_kind, r.executor_session, r.lease_expires_at, \
                r.created_at, r.started_at, r.finished_at, \
                f.category AS failure_category, f.reason_code AS failure_reason \
         FROM delivery.task_run r \
         JOIN delivery.task t ON t.id = r.task_id \
         LEFT JOIN delivery.task_run_failure f ON f.run_id = r.id \
         WHERE r.tenant_id = $5 \
           AND EXISTS (SELECT 1 FROM core.project_member m \
                        WHERE m.project_id = r.project_id AND m.principal_id = $4) \
           AND ($1::uuid IS NULL OR r.project_id = $1) \
           AND ($6::uuid IS NULL OR r.task_id = $6) \
           AND ($7::text IS NULL OR r.run_kind = $7) \
         ORDER BY r.created_at DESC, r.attempt_no DESC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .bind(task_id)
    .bind(run_kind.as_deref())
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run r \
          WHERE r.tenant_id = $3 \
            AND EXISTS (SELECT 1 FROM core.project_member m \
                         WHERE m.project_id = r.project_id AND m.principal_id = $2) \
            AND ($1::uuid IS NULL OR r.project_id = $1) \
            AND ($4::uuid IS NULL OR r.task_id = $4) \
            AND ($5::text IS NULL OR r.run_kind = $5)",
    )
    .bind(project_id)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .bind(task_id)
    .bind(run_kind.as_deref())
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

/// 单 Run 详情头：列表字段 + 租约 / 执行器 / 阶段时间线 / 集成与审查结论。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub(crate) struct TaskRunDetailRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub task_title: String,
    pub attempt_no: i32,
    pub run_kind: String,
    pub status: String,
    pub outcome: Option<String>,
    pub stage: Option<String>,
    pub agent_id: Option<Uuid>,
    pub executor_kind: Option<String>,
    pub executor_session: Option<String>,
    pub lease_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub claimed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub heartbeat_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_agent_heartbeat_at: Option<chrono::DateTime<chrono::Utc>>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deadline_at: Option<chrono::DateTime<chrono::Utc>>,
    pub release_reason: Option<String>,
    pub integration_status: String,
    pub review_verdict: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub failure_category: Option<String>,
    pub failure_reason: Option<String>,
}

/// 本 Run 锁定的一仓基线（`task_run_repository`）；`workspace_path` 即「日志定位」文本。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub(crate) struct RunRepositoryRow {
    pub repository_id: Uuid,
    pub repository_name: String,
    pub base_ref: String,
    pub base_commit: String,
    pub write_access: bool,
    pub workspace_path: String,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct TaskRunDetail {
    #[serde(flatten)]
    pub run: TaskRunDetailRow,
    pub repositories: Vec<RunRepositoryRow>,
}

/// `GET /v1/pm/task-runs/{run_id}`：run 不存在 / 跨租户 / 非成员 → 404（与 `/delivery` 同口径）。
async fn task_run_detail(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(run_id): Path<Uuid>,
) -> ApiResult<Json<TaskRunDetail>> {
    let run: Option<TaskRunDetailRow> = sqlx::query_as(
        "SELECT r.id, r.project_id, r.task_id, t.title AS task_title, r.attempt_no, r.run_kind, \
                r.status, r.outcome, r.stage, r.agent_id, r.executor_kind, r.executor_session, \
                r.lease_expires_at, r.claimed_at, r.heartbeat_at, r.last_agent_heartbeat_at, \
                r.started_at, r.finished_at, r.deadline_at, r.release_reason, \
                r.integration_status, r.review_verdict, r.created_at, \
                f.category AS failure_category, f.reason_code AS failure_reason \
           FROM delivery.task_run r \
           JOIN delivery.task t ON t.id = r.task_id \
           LEFT JOIN delivery.task_run_failure f ON f.run_id = r.id \
          WHERE r.id = $1 AND r.tenant_id = $2 \
            AND EXISTS (SELECT 1 FROM core.project_member m \
                         WHERE m.project_id = r.project_id AND m.principal_id = $3)",
    )
    .bind(run_id)
    .bind(caller.tenant_id)
    .bind(caller.principal_id)
    .fetch_optional(&state.pool)
    .await?;
    let run = run.ok_or(ApiError::NotFound("run_not_found"))?;
    let repositories: Vec<RunRepositoryRow> = sqlx::query_as(
        "SELECT trr.repository_id, pr.name AS repository_name, trr.base_ref, trr.base_commit, \
                trr.write_access, trr.workspace_path \
           FROM delivery.task_run_repository trr \
           JOIN integration.project_repository pr \
             ON pr.id = trr.repository_id AND pr.project_id = trr.project_id \
          WHERE trr.run_id = $1 \
          ORDER BY pr.name, trr.repository_id",
    )
    .bind(run_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(TaskRunDetail { run, repositories }))
}
