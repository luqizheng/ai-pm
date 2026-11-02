//! `/v1/pm/outbox/*`：Outbox 运维端点（T-061 §4）。
//!
//! 三个端点把「既有死信查询与重放能力」暴露成项目级可用的运维入口——此前
//! 它们只躺在基础设施层，运维发现死信只能直连 DB。
//!
//! 权限模型（V1 取舍）：沿用 `require_project_visible`（同租户 + 项目成员）。
//! **V1 是「项目成员级别的死信运维能力」，不是独立运维角色授权**——项目内
//! 没有权限码体系，不新造体系（YAGNI）。弥补追溯需要的方式是**重放记审计**：
//! 每次调用都写一条 `governance.audit_log`（含操作者、项目、事件 ID 与结果），
//! 成功与失败**均记**——失败的探测尝试本身就是排障信息。
//!
//! 项目级隔离是**双层防线**：API 层校验项目可见性，底层 SQL 再带
//! `project_id` 条件（调用方可构造别项目的 `event_id`）。

use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    routing::{get, post},
};
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{ApiResult, require_project_visible};
use pm_application::outbox_ops::{self, ReplayOutcome};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/outbox/dead-letters", get(dead_letters_handler))
        .route(
            "/v1/pm/outbox/events/{event_id}/replay",
            post(replay_handler),
        )
        .route("/v1/pm/outbox/stats", get(stats_handler))
}

/// 死信列表查询参数。`project_id` 必填（跨项目的运维视图没有意义，且必须
/// 落到一个租户边界内）。
#[derive(Debug, serde::Deserialize)]
struct DeadLetterQuery {
    project_id: Option<Uuid>,
    limit: Option<i64>,
}

/// 重放查询参数——`project_id` 同样必填。
#[derive(Debug, serde::Deserialize)]
struct ReplayQuery {
    project_id: Option<Uuid>,
}

/// 统计查询参数。
#[derive(Debug, serde::Deserialize)]
struct StatsQuery {
    project_id: Option<Uuid>,
}

async fn dead_letters_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<DeadLetterQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = query
        .project_id
        .ok_or_else(|| crate::support::ApiError::BadRequest("project_id is required"))?;
    require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id).await?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let listing = outbox_ops::dead_letters(&state.pool, project_id, limit).await?;
    Ok(Json(serde_json::json!({
        "items": listing.items,
        "total": listing.total,
    })))
}

async fn replay_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(event_id): Path<Uuid>,
    Query(query): Query<ReplayQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = query
        .project_id
        .ok_or_else(|| crate::support::ApiError::BadRequest("project_id is required"))?;
    require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id).await?;

    let outcome = outbox_ops::replay(&state.pool, project_id, event_id).await?;
    // 重放审计：成功与失败均记（失败尝试也是排障信息）。
    audit_replay(
        &state,
        &caller.audit_actor(),
        project_id,
        event_id,
        match &outcome {
            ReplayOutcome::Replayed => "replayed",
            ReplayOutcome::NotFound => "not_found",
            ReplayOutcome::NotDeadLetter => "not_dead_letter",
        },
    )
    .await?;

    Ok(match outcome {
        ReplayOutcome::Replayed => Json(serde_json::json!({
            "replayed": true,
            "event_id": event_id,
        })),
        // 跨项目事件与不存在事件**统一 404**——不让调用方区分「存在但无权」
        // 与「不存在」，避免事件 ID 探测。
        ReplayOutcome::NotFound => {
            return Err(crate::support::ApiError::NotFound(
                "outbox event not found",
            ));
        }
        ReplayOutcome::NotDeadLetter => {
            return Err(crate::support::ApiError::Conflict(
                "outbox event is not dead-lettered",
            ));
        }
    })
}

async fn stats_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<StatsQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = query
        .project_id
        .ok_or_else(|| crate::support::ApiError::BadRequest("project_id is required"))?;
    require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id).await?;
    let stats = outbox_ops::stats(&state.pool, project_id).await?;
    Ok(Json(serde_json::to_value(stats).unwrap_or_else(|_| serde_json::json!({}))))
}

/// 写一条重放审计（`action = 'outbox_dead_letter_replay'`）。
///
/// 审计写失败**不阻断**重放结果返回——重放本身已落库，审计是留痕；把它
/// 记为 error 日志即可（严格来说这也是一种「留痕缺失」，但让运维操作因
/// 审计失败而不可用更糟）。
async fn audit_replay(
    state: &AppState,
    actor: &pm_application::actor::AuditActor,
    project_id: Uuid,
    event_id: Uuid,
    result: &str,
) -> ApiResult<()> {
    if let Err(e) = sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, after_data, source_kind, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, 'outbox_dead_letter_replay', 'outbox_event', $7, $8, 'pm_api', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(event_id)
    .bind(serde_json::json!({ "result": result }))
    .execute(&state.pool)
    .await
    {
        tracing::error!(error = %e, %event_id, "outbox replay audit write failed");
    }
    Ok(())
}
