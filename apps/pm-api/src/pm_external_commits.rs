//! P0 外部 Commit 采纳闭环 REST 面（迁移 0054）。
//!
//! - `POST /v1/pm/projects/{id}/external-commits/discover`  扫描外部新 Commit
//! - `GET  /v1/pm/projects/{id}/external-commits?status=`   列表
//! - `GET  /v1/pm/external-commits/{id}`                    详情（含澄清列表）
//! - `POST /v1/pm/external-commits/{id}/analyze`            AI 分析（人类触发）
//! - `POST /v1/pm/external-commits/{id}/clarifications/{cid}/answer`  澄清答复（human 门）
//! - `POST /v1/pm/external-commits/{id}/decision`           采纳决定（human 门）
//! - `POST /v1/pm/external-commits/{id}/close`              关闭（提案全部终态校验）
//! - `GET  /v1/pm/external-commits/{id}/diff`               变更差异（有界）
//!
//! 硬约束落点：发现 ≠ 采纳（服务层状态机 + CHECK）；采纳/澄清答复走
//! `require_human_principal`；知识更新只经 `state.knowledge` 根的提案管线。
//! 错误体沿用 `support` 的 `{message}` 约定（未知码不回译，message 原样透传）。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use pm_application::external_commit::{
    analyze_external_commit, answer_clarification, close_adoption, decide_adoption,
    discover_external_commits, get_external_commit, list_external_commits, read_commit_diff,
    AdoptionDecisionRequest, ExternalCommitError,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/external-commits/discover",
            post(discover_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/external-commits",
            get(list_handler),
        )
        .route("/v1/pm/external-commits/{commit_id}", get(detail_handler))
        .route(
            "/v1/pm/external-commits/{commit_id}/analyze",
            post(analyze_handler),
        )
        .route(
            "/v1/pm/external-commits/{commit_id}/clarifications/{clarification_id}/answer",
            post(answer_handler),
        )
        .route(
            "/v1/pm/external-commits/{commit_id}/decision",
            post(decision_handler),
        )
        .route(
            "/v1/pm/external-commits/{commit_id}/close",
            post(close_handler),
        )
        .route(
            "/v1/pm/external-commits/{commit_id}/diff",
            get(diff_handler),
        )
}

/// `ExternalCommitError` → `{message}` 响应。
fn external_commit_error(err: ExternalCommitError) -> Response {
    let status = match &err {
        ExternalCommitError::NotFound => StatusCode::NOT_FOUND,
        ExternalCommitError::Validation(_) => StatusCode::BAD_REQUEST,
        ExternalCommitError::Conflict(_) => StatusCode::CONFLICT,
        ExternalCommitError::Forbidden(_) => StatusCode::FORBIDDEN,
        ExternalCommitError::Git(detail) => {
            tracing::error!(detail = %detail, "external commit git access failed");
            StatusCode::BAD_GATEWAY
        }
        ExternalCommitError::Db(db) => {
            tracing::error!(error = %db, "external commit database error");
            StatusCode::INTERNAL_SERVER_ERROR
        }
        ExternalCommitError::Knowledge(detail) => {
            tracing::error!(error = %detail, "external commit knowledge pipeline error");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    (
        status,
        Json(json!({ "message": err.to_string(), "error": err.code() })),
    )
        .into_response()
}

/// 提交行 → 项目可见性（租户 + 成员边界，对外入口 T-022 同款）。
async fn commit_visible(
    pool: &sqlx::PgPool,
    commit_id: Uuid,
    tenant_id: Uuid,
    principal_id: Uuid,
) -> Result<Uuid, Response> {
    let project_id: Uuid = sqlx::query_scalar::<_, Uuid>(
        "SELECT project_id FROM integration.external_commit WHERE id = $1",
    )
    .bind(commit_id)
    .fetch_optional(pool)
    .await
    .map_err(|_err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "message": "internal error" })),
        )
            .into_response()
    })?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "message": "external commit not found" })),
        )
            .into_response()
    })?;
    match crate::support::project_visible(pool, project_id, tenant_id, principal_id).await {
        Ok(true) => Ok(project_id),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "message": "external commit not found" })),
        )
            .into_response()),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "message": format!("visibility check failed: {err}") })),
        )
            .into_response()),
    }
}

#[derive(Deserialize)]
struct DiscoverBody {
    #[serde(default)]
    repository_id: Option<Uuid>,
}

async fn discover_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    body: Option<Json<DiscoverBody>>,
) -> Response {
    match crate::support::project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "message": "project not found" })),
            )
                .into_response()
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "message": format!("visibility check failed: {err}") })),
            )
                .into_response()
        }
    }
    let actor = principal.audit_actor();
    match discover_external_commits(
        &state.pool,
        project_id,
        &actor,
        body.map(|Json(body)| body.repository_id).flatten(),
    )
    .await
    {
        Ok(outcome) => (StatusCode::OK, Json(outcome)).into_response(),
        Err(err) => external_commit_error(err),
    }
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    status: Option<String>,
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> Response {
    match crate::support::project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "message": "project not found" })),
            )
                .into_response()
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "message": format!("visibility check failed: {err}") })),
            )
                .into_response()
        }
    }
    match list_external_commits(&state.pool, project_id, query.status.as_deref()).await {
        Ok(items) => {
            let total = items.len();
            (
                StatusCode::OK,
                Json(json!({ "items": items, "total": total })),
            )
                .into_response()
        }
        Err(err) => external_commit_error(err),
    }
}

async fn detail_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(commit_id): Path<Uuid>,
) -> Response {
    let project_id = match commit_visible(
        &state.pool,
        commit_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        Ok(project_id) => project_id,
        Err(resp) => return resp,
    };
    let _ = project_id;
    match get_external_commit(&state.pool, commit_id).await {
        Ok((row, clarifications, repository_name)) => (
            StatusCode::OK,
            Json(json!({
                "commit": row,
                "clarifications": clarifications,
                "repository_name": repository_name,
            })),
        )
            .into_response(),
        Err(err) => external_commit_error(err),
    }
}

async fn analyze_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(commit_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = commit_visible(
        &state.pool,
        commit_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    let actor = principal.audit_actor();
    match analyze_external_commit(
        &state.pool,
        &state.knowledge,
        state.commit_model.as_ref(),
        commit_id,
        &actor,
    )
    .await
    {
        Ok(outcome) => (StatusCode::OK, Json(outcome)).into_response(),
        Err(err) => external_commit_error(err),
    }
}

#[derive(Deserialize)]
struct AnswerBody {
    answer: String,
}

async fn answer_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((commit_id, clarification_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<AnswerBody>,
) -> Response {
    if let Err(resp) = commit_visible(
        &state.pool,
        commit_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    // 澄清答复 = 人类正式输入（PA-06 人类面通道执法）。
    if let Err(err) = crate::support::require_human_principal(&state.pool, &principal).await {
        return err.into_response();
    }
    let actor = principal.audit_actor();
    match answer_clarification(
        &state.pool,
        commit_id,
        clarification_id,
        &actor,
        &body.answer,
    )
    .await
    {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
        Err(err) => external_commit_error(err),
    }
}

#[derive(Deserialize)]
struct DecisionBody {
    decision: String,
    #[serde(default)]
    note: Option<String>,
}

async fn decision_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(commit_id): Path<Uuid>,
    Json(body): Json<DecisionBody>,
) -> Response {
    if let Err(resp) = commit_visible(
        &state.pool,
        commit_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    // 采纳决定 = 人类正式决定（发现 ≠ 采纳；AI 主体一律 403）。
    if let Err(err) = crate::support::require_human_principal(&state.pool, &principal).await {
        return err.into_response();
    }
    let actor = principal.audit_actor();
    let request = AdoptionDecisionRequest {
        decision: body.decision,
        note: body.note,
    };
    match decide_adoption(&state.pool, &state.knowledge, commit_id, &actor, request).await {
        Ok(outcome) => (StatusCode::OK, Json(outcome)).into_response(),
        Err(err) => external_commit_error(err),
    }
}

async fn close_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(commit_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = commit_visible(
        &state.pool,
        commit_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    if let Err(err) = crate::support::require_human_principal(&state.pool, &principal).await {
        return err.into_response();
    }
    let actor = principal.audit_actor();
    match close_adoption(&state.pool, commit_id, &actor).await {
        Ok(outcome) => (StatusCode::OK, Json(outcome)).into_response(),
        Err(err) => external_commit_error(err),
    }
}

async fn diff_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(commit_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = commit_visible(
        &state.pool,
        commit_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    match read_commit_diff(&state.pool, commit_id).await {
        Ok((diff, truncated, sha)) => (
            StatusCode::OK,
            Json(json!({ "diff": diff, "truncated": truncated, "commit_sha": sha })),
        )
            .into_response(),
        Err(err) => external_commit_error(err),
    }
}
