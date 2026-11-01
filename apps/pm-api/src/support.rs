//! `/v1/pm/*` 端点共用件：统一错误响应与分页解析（Slice A）。
//!
//! 错误体刻意只带 `{message}` 不带 code：@digua/http 对带 code 的错误体会先走
//! contracts 的错误码翻译，未知码会把 message 覆盖成码本身；只给 message 才能
//! 原样透传（沿用 prism-shell-wip 分支的结论）。

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

pub enum ApiError {
    BadRequest(&'static str),
    Conflict(&'static str),
    NotFound(&'static str),
    Internal(sqlx::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            ApiError::Internal(e) => {
                tracing::error!(error = %e, "pm-api internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
            }
        };
        (status, Json(json!({ "message": message }))).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Internal(e)
    }
}

impl From<pm_application::pm_write::PmWriteError> for ApiError {
    fn from(e: pm_application::pm_write::PmWriteError) -> Self {
        match e {
            pm_application::pm_write::PmWriteError::NotFound(m) => ApiError::NotFound(m),
            pm_application::pm_write::PmWriteError::Conflict(m) => ApiError::Conflict(m),
            pm_application::pm_write::PmWriteError::Validation(m) => ApiError::BadRequest(m),
            pm_application::pm_write::PmWriteError::Db(err) => ApiError::Internal(err),
        }
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

pub const DEFAULT_PAGE: i64 = 1;
pub const DEFAULT_SIZE: i64 = 10;
pub const MAX_SIZE: i64 = 200;

pub struct Paging {
    pub page: i64,
    pub size: i64,
    pub offset: i64,
}

impl Paging {
    pub fn to_response<T: serde::Serialize>(self, items: Vec<T>, total: i64) -> serde_json::Value {
        json!({
            "items": items,
            "total": total,
            "page": self.page,
            "size": self.size,
        })
    }
}

/// 解析 page/size（非法或缺省回落默认值，size 截到 MAX_SIZE）。
pub fn paging(params: &HashMap<String, String>) -> Paging {
    let page = params
        .get("page")
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(DEFAULT_PAGE);
    let size = params
        .get("size")
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .map(|v| v.min(MAX_SIZE))
        .unwrap_or(DEFAULT_SIZE);
    Paging {
        page,
        size,
        offset: (page - 1) * size,
    }
}

/// 可选的 project_id 过滤参数（非法值视为未传）。
pub fn project_filter(params: &HashMap<String, String>) -> Option<Uuid> {
    params.get("project_id").and_then(|v| Uuid::parse_str(v).ok())
}

/// 可选的 customer_id 过滤参数（非法值视为未传）。项目列表按客户聚合时使用。
pub fn customer_filter(params: &HashMap<String, String>) -> Option<Uuid> {
    params.get("customer_id").and_then(|v| Uuid::parse_str(v).ok())
}
