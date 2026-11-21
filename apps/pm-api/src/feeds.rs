//! 统一信息流三视角与已读状态（PA-05-08/09 批次 C；只读面 + Read 写面）。
//!
//! - `GET /v1/pm/feeds/my`：Project Membership ∪ Product Membership 的去重
//!   活动集（My Feed 根据 Product Membership 和事项路由生成）。
//! - `GET /v1/pm/feeds/projects/{project_id}`：Project Feed（成员可见性守卫
//!   同总控台口径——不可见一律 404，不泄露存在性）。
//! - `GET /v1/pm/feeds/products/{product_id}`：Product Feed（仅 Product
//!   成员可读，与 list_product_members 同口径；非成员 404）。
//! - `POST /v1/pm/feeds/read-state`：批量标记/清除 Read；不碰
//!   Notified/Processed/Approved（PA-05-09 四概念互不等同）。
//!
//! Feed 是统一 Activity 数据的不同视图，不另建消息系统；已读是用户个人
//! 状态，按调用者主体读写。

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, ApiError, ApiResult};

use pm_application::work_item::{
    mark_all_my_feed_read, mark_read_state, my_feed, my_feed_counts, product_feed,
    project_feed, MyFeedFilter, WorkItemError,
};

/// WorkItemError → ApiError（NotFound 保持 404 语义；Validation → 400）。
fn api_err(err: WorkItemError) -> ApiError {
    match err {
        WorkItemError::Db(e) => ApiError::Internal(e),
        WorkItemError::NotFound => ApiError::NotFound("work item not found"),
        WorkItemError::Validation(m) => ApiError::BadRequest(m),
    }
}

async fn mark_read(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    principal_id: Uuid,
    item_ids: &[Uuid],
    read: bool,
) -> ApiResult<u64> {
    mark_read_state(pool, tenant_id, principal_id, item_ids, read)
        .await
        .map_err(api_err)
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/feeds/my", get(my_feed_handler))
        .route(
            "/v1/pm/feeds/projects/{project_id}",
            get(project_feed_handler),
        )
        .route(
            "/v1/pm/feeds/products/{product_id}",
            get(product_feed_handler),
        )
        .route("/v1/pm/feeds/read-state", post(mark_read_state_handler))
        .route("/v1/pm/feeds/mark-all-read", post(mark_all_read_handler))
}

#[derive(Deserialize)]
struct FeedQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
    /// 视图：unread（仅未读）/ attention（需关注）；缺省 All activity。
    #[serde(default)]
    view: Option<String>,
    #[serde(default)]
    actor_type: Option<String>,
    #[serde(default)]
    activity_type: Option<String>,
    #[serde(default)]
    project_id: Option<Uuid>,
}

fn default_limit() -> i64 {
    50
}

fn feed_json_with_counts(
    rows: Vec<pm_application::work_item::FeedRow>,
    counts: Option<pm_application::work_item::FeedCounts>,
) -> Json<serde_json::Value> {
    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "id": row.id,
                "item_id": row.item_id,
                "target_type": row.target_type,
                "target_id": row.target_id,
                "item_title": row.item_title,
                "activity_type": row.activity_type,
                "actor_type": row.actor_type,
                "actor_id": row.actor_id,
                "summary": row.summary,
                "detail": row.detail,
                "project_id": row.project_id,
                "product_id": row.product_id,
                "created_at": row.created_at,
                "read_at": row.read_at,
            })
        })
        .collect();
    let mut body = serde_json::json!({ "items": items });
    if let Some(c) = counts {
        body["counts"] = serde_json::json!({
            "total": c.total,
            "unread": c.unread,
            "attention": c.attention,
        });
    }
    Json(body)
}

fn feed_json(rows: Vec<pm_application::work_item::FeedRow>) -> Json<serde_json::Value> {
    feed_json_with_counts(rows, None)
}

#[derive(Deserialize)]
struct MarkAllBody {}

async fn mark_all_read_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
) -> ApiResult<Json<serde_json::Value>> {
    let updated =
        mark_all_my_feed_read(&state.pool, principal.tenant_id, principal.principal_id)
            .await
            .map_err(api_err)?;
    Ok(Json(serde_json::json!({ "updated": updated })))
}

async fn my_feed_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Query(query): Query<FeedQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let filter = MyFeedFilter {
        unread_only: query.view.as_deref() == Some("unread"),
        attention_only: query.view.as_deref() == Some("attention"),
        actor_type: query.actor_type.clone(),
        activity_type: query.activity_type.clone(),
        project_id: query.project_id,
    };
    let rows = my_feed(
        &state.pool,
        principal.tenant_id,
        principal.principal_id,
        Some(&filter),
        query.limit.clamp(1, 200),
        query.offset.max(0),
    )
    .await
    .map_err(api_err)?;
    let counts = my_feed_counts(&state.pool, principal.tenant_id, principal.principal_id)
        .await
        .map_err(api_err)?;
    Ok(feed_json(rows))
}

async fn project_feed_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<FeedQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let rows = project_feed(
        &state.pool,
        principal.tenant_id,
        principal.principal_id,
        project_id,
        query.limit.clamp(1, 200),
        query.offset.max(0),
    )
    .await
    .map_err(api_err)?;
    Ok(feed_json(rows))
}

async fn product_feed_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(product_id): Path<Uuid>,
    Query(query): Query<FeedQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    // 与 list_product_members 同口径：仅 Product 成员可读（非成员 404）。
    let is_member: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.product_member \
          WHERE product_id = $1 AND principal_id = $2)",
    )
    .bind(product_id)
    .bind(principal.principal_id)
    .fetch_one(&state.pool)
    .await?;
    if !is_member {
        return Err(crate::support::ApiError::NotFound("product not found"));
    }
    let rows = product_feed(
        &state.pool,
        principal.tenant_id,
        principal.principal_id,
        product_id,
        query.limit.clamp(1, 200),
        query.offset.max(0),
    )
    .await
    .map_err(api_err)?;
    Ok(feed_json(rows))
}

#[derive(Deserialize)]
struct ReadStateBody {
    item_ids: Vec<Uuid>,
    #[serde(default = "default_read")]
    read: bool,
}

fn default_read() -> bool {
    true
}

#[derive(Serialize)]
struct ReadStateResult {
    updated: u64,
}

async fn mark_read_state_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Json(body): Json<ReadStateBody>,
) -> ApiResult<Json<ReadStateResult>> {
    let updated = mark_read(
        &state.pool,
        principal.tenant_id,
        principal.principal_id,
        &body.item_ids,
        body.read,
    )
    .await?;
    Ok(Json(ReadStateResult { updated }))
}
