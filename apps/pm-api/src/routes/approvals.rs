//! `POST /v1/pm/approvals/{id}/decide` —— 红线确认的人工决策。
//!
//! 契约：`{decision: "approved"|"rejected", reason}` →
//! `{id, status, decided_at}`；404 未知、409 已决策、400 非法入参。
//!
//! V1 语义：决策落 `governance.approval_request`，驳回把关联提案置
//! `vetoed` 并记 `human_intervention(veto)`；批准暂不推进提案状态——
//! 提案从 redline_pending 到执行的推进属于命令通道（PgCommandExecutor），
//! 端点落地时在此挂接。两类决策都写 `audit_log`，全在一个事务里。

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::post,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::auth::Authed;
use super::error::{ApiError, ApiResult};

pub fn router() -> Router<PgPool> {
    Router::new().route("/v1/pm/approvals/{id}/decide", post(decide))
}

#[derive(Deserialize)]
pub struct DecideRequest {
    decision: String,
    reason: String,
}

async fn decide(
    State(pool): State<PgPool>,
    auth: Authed,
    Path(id): Path<Uuid>,
    Json(body): Json<DecideRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    if body.decision != "approved" && body.decision != "rejected" {
        return Err(ApiError::BadRequest("decision must be approved or rejected"));
    }
    let reason = body.reason.trim();
    if reason.is_empty() {
        return Err(ApiError::BadRequest("reason is required"));
    }
    let approved = body.decision == "approved";

    let mut tx = pool.begin().await?;

    // 决策幂等防线：仅 pending 可决策（UPDATE ... WHERE status='pending' 原子抢锁）
    let updated = sqlx::query(
        "UPDATE governance.approval_request \
         SET status = $2, decided_by = $3, decision_reason = $4, decided_at = now() \
         WHERE id = $1 AND status = 'pending'",
    )
    .bind(id)
    .bind(&body.decision)
    .bind(auth.principal_id)
    .bind(reason)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        let exists: Option<String> =
            sqlx::query_scalar("SELECT status FROM governance.approval_request WHERE id = $1")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        return match exists {
            None => Err(ApiError::NotFound),
            Some(_) => Err(ApiError::Conflict("approval already decided")),
        };
    }

    let proposal_id: Uuid =
        sqlx::query_scalar("SELECT proposal_id FROM governance.approval_request WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    let project_id: Uuid =
        sqlx::query_scalar("SELECT project_id FROM governance.approval_request WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;

    if !approved {
        // 驳回 = 否决：提案终态 vetoed + 人工干预记录
        sqlx::query("UPDATE ai.change_proposal SET status = 'vetoed' WHERE id = $1")
            .bind(proposal_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO governance.human_intervention \
             (project_id, intervention_type, target_proposal_id, actor_id, reason, executed_at) \
             VALUES ($1, 'veto', $2, $3, $4, now())",
        )
        .bind(project_id)
        .bind(proposal_id)
        .bind(auth.principal_id)
        .bind(reason)
        .execute(&mut *tx)
        .await?;
    }
    // 批准：提案保持 redline_pending，待命令通道消费（见模块注释）

    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, action, target_type, target_id, created_at) \
         VALUES ($1, $2, $3, 'approval_request', $4, $5)",
    )
    .bind(project_id)
    .bind(auth.principal_id)
    .bind(if approved { "approval.approve" } else { "approval.reject" })
    .bind(id)
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let (status, decided_at): (String, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "SELECT status, decided_at FROM governance.approval_request WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(json!({ "id": id, "status": status, "decided_at": decided_at })))
}
