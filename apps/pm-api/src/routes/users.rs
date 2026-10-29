//! `GET /v1/iam/users/simple` —— 壳用户目录（principal → 昵称索引）。

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;

use super::auth::Authed;
use super::error::ApiResult;
use super::pagination::paging;

pub fn router() -> Router<PgPool> {
    Router::new().route("/v1/iam/users/simple", get(users_simple))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct UserSimpleRow {
    pub id: uuid::Uuid,
    pub nickname: String,
}

async fn users_simple(
    State(pool): State<PgPool>,
    _auth: Authed,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let items: Vec<UserSimpleRow> = sqlx::query_as(
        "SELECT id, display_name AS nickname FROM core.principal \
         WHERE is_active ORDER BY created_at LIMIT $1 OFFSET $2",
    )
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&pool)
    .await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM core.principal WHERE is_active")
        .fetch_one(&pool)
        .await?;

    Ok(Json(page.to_response(items, total)))
}
