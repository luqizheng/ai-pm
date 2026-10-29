use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// 统一错误响应。不带 code 字段：@digua/http 对带 code 的错误体会先走
/// contracts 的错误码翻译，未知码会把 message 覆盖成码本身；只给 message
/// 才能原样透传给用户。
pub enum ApiError {
    Unauthorized(&'static str),
    BadRequest(&'static str),
    Conflict(&'static str),
    NotFound,
    Internal(sqlx::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found"),
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

pub type ApiResult<T> = Result<T, ApiError>;
