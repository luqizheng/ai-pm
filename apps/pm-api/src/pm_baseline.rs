//! S01-05 · Project Baseline V1（AI 提案 → 人类逐项确认 → 版本不可变）。
//!
//! 提案条目从 S01-04 已确认知识 + 仓库快照派生（认知 → 基线的唯一管线）。
//! 确认规则（与 S01-05 边界裁决一致）：逐项独立决策；**Confirm Baseline 要求
//! 无 proposed 条目**（未决更正阻塞），deferred/investigation 允许作为
//! documented uncertainties 随基线保留。已确认提案不可变；变更走新提案。
//! 授权同 S01-04：读写都过 require_project_visible（DEF-01 教训）。

use axum::{
    extract::{Extension, Path, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, ApiError, ApiResult};

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

    let current: Option<(i32, Uuid, Uuid, DateTime<Utc>)> = sqlx::query_as(
        "SELECT version, proposal_id, confirmed_by, confirmed_at FROM ai.project_baseline \
         WHERE tenant_id = $1 AND project_id = $2 ORDER BY version DESC LIMIT 1",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::Internal)?;

    let proposal: Option<(Uuid, i32, String, Uuid, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, version, status, created_by, created_at FROM ai.baseline_proposal \
         WHERE tenant_id = $1 AND project_id = $2 AND status = 'pending'",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::Internal)?;

    let items: Vec<BaselineItemRow> = match &proposal {
        Some((pid, ..)) => sqlx::query_as(
            "SELECT id, category, content, evidence, confidence, state, \
                    decided_by, decided_at, decision_note \
             FROM ai.baseline_item WHERE tenant_id = $1 AND proposal_id = $2 \
             ORDER BY created_at",
        )
        .bind(caller.tenant_id)
        .bind(pid)
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::Internal)?,
        None => Vec::new(),
    };

    let history: Vec<(i32, Uuid, DateTime<Utc>)> = sqlx::query_as(
        "SELECT version, proposal_id, confirmed_at FROM ai.project_baseline \
         WHERE tenant_id = $1 AND project_id = $2 ORDER BY version DESC",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::Internal)?;

    Ok(Json(json!({
        "current_version": current.as_ref().map(|c| c.0),
        "current_confirmed_at": current.as_ref().map(|c| c.3),
        "history": history.iter().map(|h| json!({
            "version": h.0, "proposal_id": h.1, "confirmed_at": h.2,
        })).collect::<Vec<_>>(),
        "proposal": proposal.as_ref().map(|p| json!({
            "id": p.0, "version": p.1, "status": p.2, "created_at": p.4,
        })),
        "items": items,
    })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct BaselineItemRow {
    id: Uuid,
    category: String,
    content: String,
    evidence: serde_json::Value,
    confidence: String,
    state: String,
    decided_by: Option<Uuid>,
    decided_at: Option<DateTime<Utc>>,
    decision_note: Option<String>,
}

/// POST 生成提案（人类动作）：从已确认知识 + 仓库快照派生条目。
/// 已有 pending 提案 → 409（变更走新提案，须先处理当前提案）。
async fn create_proposal(
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

    let mut tx = state.pool.begin().await.map_err(ApiError::Internal)?;

    let pending: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM ai.baseline_proposal \
         WHERE tenant_id = $1 AND project_id = $2 AND status = 'pending'",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;
    if pending.is_some() {
        return Err(ApiError::Conflict(
            "a pending baseline proposal already exists",
        ));
    }

    let version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM ai.project_baseline \
         WHERE tenant_id = $1 AND project_id = $2",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;

    let proposal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.baseline_proposal (tenant_id, project_id, version, created_by) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .bind(version)
    .bind(caller.principal_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;

    // 条目来源①：S01-04 已确认知识（认知 → 基线的管线）。
    // 类别沿用知识的领域映射（V1 一一对应；scope/standards 等待知识细分后启用）。
    let knowledge: Vec<(String, String, Uuid)> = sqlx::query_as(
        "SELECT domain, content, id FROM ai.project_knowledge \
         WHERE tenant_id = $1 AND project_id = $2 AND status = 'active' \
         ORDER BY confirmed_at",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;
    for (domain, content, kid) in &knowledge {
        sqlx::query(
            "INSERT INTO ai.baseline_item \
             (tenant_id, project_id, proposal_id, category, content, evidence, confidence) \
             VALUES ($1, $2, $3, $4, $5, $6, 'high')",
        )
        .bind(caller.tenant_id)
        .bind(project_id)
        .bind(proposal_id)
        .bind(domain)
        .bind(content)
        .bind(json!({"type": "knowledge", "knowledge_id": kid}))
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Internal)?;
    }

    // 条目来源②：仓库快照（role + url；primary=main 角色记录，不自动认定主仓）。
    let repos: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT name, url, role FROM integration.project_repository \
         WHERE tenant_id = $1 AND project_id = $2 ORDER BY created_at",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;
    for (name, url, role) in &repos {
        sqlx::query(
            "INSERT INTO ai.baseline_item \
             (tenant_id, project_id, proposal_id, category, content, evidence, confidence) \
             VALUES ($1, $2, $3, 'repository', $4, $5, 'high')",
        )
        .bind(caller.tenant_id)
        .bind(project_id)
        .bind(proposal_id)
        .bind(format!("Repository '{}' ({}): {}", name, role, url))
        .bind(json!({"type": "repository_snapshot", "role": role}))
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Internal)?;
    }

    let item_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.baseline_item \
         WHERE tenant_id = $1 AND proposal_id = $2",
    )
    .bind(caller.tenant_id)
    .bind(proposal_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;

    tx.commit().await.map_err(ApiError::Internal)?;

    Ok(Json(json!({
        "proposal_id": proposal_id,
        "version": version,
        "item_count": item_count,
    })))
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

    let next_state = match body.action.as_str() {
        "confirm" => "confirmed",
        "reject" => "rejected",
        "defer" => "deferred",
        "investigate" => "investigation",
        _ => return Err(ApiError::BadRequest("invalid action")),
    };

    let mut tx = state.pool.begin().await.map_err(ApiError::Internal)?;

    // 提案必须是 pending（confirmed 即不可变，§核心原则 6）。
    let item: (String, Uuid) = sqlx::query_as(
        "SELECT bi.state, bi.proposal_id FROM ai.baseline_item bi \
         JOIN ai.baseline_proposal bp ON bp.id = bi.proposal_id \
         WHERE bi.tenant_id = $1 AND bi.project_id = $2 AND bi.id = $3 \
           AND bp.status = 'pending' FOR UPDATE OF bi",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .bind(item_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::Internal)?
    .ok_or_else(|| ApiError::NotFound("baseline item not found or proposal not pending"))?;

    if item.0 != "proposed" {
        return Err(ApiError::Conflict("baseline item is already decided"));
    }

    sqlx::query(
        "UPDATE ai.baseline_item SET state = $3, decided_by = $4, decided_at = now(), \
                decision_note = $5, content = COALESCE($6, content) \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(caller.tenant_id)
    .bind(item_id)
    .bind(next_state)
    .bind(caller.principal_id)
    .bind(body.note.as_deref().filter(|n| !n.trim().is_empty()))
    .bind(body.content.as_deref().filter(|c| !c.trim().is_empty()))
    .execute(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;

    tx.commit().await.map_err(ApiError::Internal)?;

    Ok(Json(json!({ "id": item_id, "state": next_state })))
}

/// POST 确认基线（显式人类动作）：要求提案存在、无 proposed 条目
/// （deferred/investigation 作为 documented uncertainties 保留）；
/// 成功 → 版本头落库 + 提案转 confirmed（条目即不可变）。
async fn confirm_baseline(
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

    let mut tx = state.pool.begin().await.map_err(ApiError::Internal)?;

    let proposal: (Uuid, i32) = sqlx::query_as(
        "SELECT id, version FROM ai.baseline_proposal \
         WHERE tenant_id = $1 AND project_id = $2 AND status = 'pending' \
         ORDER BY created_at DESC LIMIT 1 FOR UPDATE",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::Internal)?
    .ok_or_else(|| ApiError::NotFound("no pending baseline proposal"))?;

    let proposed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.baseline_item \
         WHERE tenant_id = $1 AND proposal_id = $2 AND state = 'proposed'",
    )
    .bind(caller.tenant_id)
    .bind(proposal.0)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;
    if proposed > 0 {
        return Err(ApiError::Conflict(
            "baseline proposal still has undecided items",
        ));
    }

    sqlx::query(
        "INSERT INTO ai.project_baseline \
         (tenant_id, project_id, version, proposal_id, confirmed_by) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .bind(proposal.1)
    .bind(proposal.0)
    .bind(caller.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;

    sqlx::query(
        "UPDATE ai.baseline_proposal SET status = 'confirmed', confirmed_by = $3, \
                confirmed_at = now() WHERE tenant_id = $1 AND id = $2",
    )
    .bind(caller.tenant_id)
    .bind(proposal.0)
    .bind(caller.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;

    tx.commit().await.map_err(ApiError::Internal)?;

    Ok(Json(
        json!({ "version": proposal.1, "status": "confirmed" }),
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
