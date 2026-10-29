//! 壳认证端点（IAM 上游服务就绪前的本地实现）。
//!
//! 契约来自前端壳 `frontend/src/shell/api/client.ts`：login/refresh 返回
//! `{access_token, refresh_token, token_type, expires_in, user}`。
//! 当前策略：任意口令可登录（与前端 mock 一致），按 `display_name` 找或建
//! 人类 principal；角色固定 project_owner 占位。真实认证/角色/权限由 IAM
//! 接管后整块替换。

use axum::{
    Json, Router,
    extract::{FromRequestParts, State},
    routing::{get, post},
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::error::{ApiError, ApiResult};

const SESSION_TTL_HOURS: i64 = 24;

pub fn router() -> Router<PgPool> {
    Router::new()
        .route("/v1/iam/auth/login", post(login))
        .route("/v1/iam/auth/refresh", post(refresh))
        .route("/v1/iam/auth/logout", post(logout))
        .route("/v1/iam/auth/setup-status", get(setup_status))
}

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    #[allow(dead_code)]
    password: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    refresh_token: String,
}

/// Bearer 会话校验：token 即 core.auth_session.id。
pub struct Authed {
    pub principal_id: Uuid,
    pub session_id: Uuid,
}

impl FromRequestParts<PgPool> for Authed {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        pool: &PgPool,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized("missing bearer token"))?;
        let session_id: Uuid = token
            .parse()
            .map_err(|_| ApiError::Unauthorized("invalid token"))?;
        let principal_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT principal_id FROM core.auth_session WHERE id = $1 AND expires_at > now()",
        )
        .bind(session_id)
        .fetch_optional(pool)
        .await?;
        principal_id
            .map(|principal_id| Authed {
                principal_id,
                session_id,
            })
            .ok_or(ApiError::Unauthorized("session expired"))
    }
}

/// 找或建人类 principal，签发新会话。
async fn login(
    State(pool): State<PgPool>,
    Json(body): Json<LoginRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let username = body.username.trim();
    if username.is_empty() {
        return Err(ApiError::BadRequest("username is required"));
    }

    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM core.principal \
         WHERE kind = 'human' AND display_name = $1 AND is_active \
         ORDER BY created_at LIMIT 1",
    )
    .bind(username)
    .fetch_optional(&pool)
    .await?;

    let principal_id = match existing {
        Some(id) => id,
        None => {
            let id: Uuid = sqlx::query_scalar(
                "INSERT INTO core.principal (kind, display_name) VALUES ('human', $1) RETURNING id",
            )
            .bind(username)
            .fetch_one(&pool)
            .await?;
            tracing::info!(principal = %id, username, "created principal on first login");
            id
        }
    };

    issue_session(&pool, principal_id, &username).await
}

async fn refresh(
    State(pool): State<PgPool>,
    Json(body): Json<RefreshRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let refresh_token: Uuid = body
        .refresh_token
        .parse()
        .map_err(|_| ApiError::Unauthorized("invalid refresh token"))?;

    let row: Option<(Uuid, Uuid, Uuid, String)> = sqlx::query_as(
        "UPDATE core.auth_session s \
         SET refresh_token = gen_random_uuid(), expires_at = $2 \
         FROM core.principal p \
         WHERE s.refresh_token = $1 AND s.principal_id = p.id AND s.expires_at > now() \
         RETURNING s.id, s.refresh_token, s.principal_id, p.display_name",
    )
    .bind(refresh_token)
    .bind(Utc::now() + Duration::hours(SESSION_TTL_HOURS))
    .fetch_optional(&pool)
    .await?;

    let (session_id, new_refresh, principal_id, display_name) =
        row.ok_or(ApiError::Unauthorized("refresh token expired"))?;
    Ok(Json(session_body(
        principal_id,
        session_id,
        new_refresh,
        &display_name,
    )))
}

async fn logout(
    State(pool): State<PgPool>,
    auth: Authed,
) -> ApiResult<Json<serde_json::Value>> {
    sqlx::query("DELETE FROM core.auth_session WHERE id = $1")
        .bind(auth.session_id)
        .execute(&pool)
        .await?;
    Ok(Json(json!({})))
}

async fn setup_status() -> Json<serde_json::Value> {
    Json(json!({ "setup_required": false }))
}

/// 签发会话并组装壳的 LoginResult（user.id 是 principal_id，不是 token）。
async fn issue_session(
    pool: &PgPool,
    principal_id: Uuid,
    display_name: &str,
) -> ApiResult<Json<serde_json::Value>> {
    let (session_id, refresh_token): (Uuid, Uuid) = sqlx::query_as(
        "INSERT INTO core.auth_session (principal_id, expires_at) VALUES ($1, $2) \
         RETURNING id, refresh_token",
    )
    .bind(principal_id)
    .bind(Utc::now() + Duration::hours(SESSION_TTL_HOURS))
    .fetch_one(pool)
    .await?;
    Ok(Json(session_body(
        principal_id,
        session_id,
        refresh_token,
        display_name,
    )))
}

fn session_body(
    principal_id: Uuid,
    access_token: Uuid,
    refresh_token: Uuid,
    display_name: &str,
) -> serde_json::Value {
    json!({
        "access_token": access_token,
        "refresh_token": refresh_token,
        "token_type": "Bearer",
        "expires_in": SESSION_TTL_HOURS * 3600,
        "user": {
            "id": principal_id,
            "username": display_name,
            "nickname": display_name,
            "email": null,
            "phone": null,
            "avatar": null,
            "roles": ["project_owner"],
            "permissions": [],
        }
    })
}
