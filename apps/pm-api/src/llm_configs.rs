//! `/v1/pm/llm/*` 端点：全局模型供应商配置（deepseek / openai / glm / minimax / ollama）。
//!
//! 错误体沿用 `/v1/pm/*` 约定：只带 `{message}`，由 `@digua/http` 原样透传。
//! 连通性测试在服务端发起（不在浏览器侧直连供应商），避免浏览器 CORS 与
//! 密钥外泄；返回 `{ok, latency_ms, message}`，不返回任何密钥信息。

use std::time::{Duration, Instant};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde::Serialize;
use serde_json::json;

use crate::routes::AppState;
use pm_application::llm_config::{
    list_providers, load_provider_credentials, update_provider, LlmConfigError,
    UpdateLlmProviderRequest,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/llm/providers", get(list_handler))
        .route("/v1/pm/llm/providers/{provider}", put(update_handler))
        .route("/v1/pm/llm/providers/{provider}/test", post(test_handler))
}

fn error_response(err: LlmConfigError) -> Response {
    let (status, message) = match &err {
        LlmConfigError::UnknownProvider(_) => (StatusCode::NOT_FOUND, "unknown provider"),
        LlmConfigError::Validation(message) => (StatusCode::BAD_REQUEST, *message),
        LlmConfigError::SecretKeyMissing => (
            StatusCode::SERVICE_UNAVAILABLE,
            "AI_PM_SECRET_KEY is not configured; API key cannot be stored",
        ),
        LlmConfigError::Crypto => {
            tracing::error!(error = %err, "llm api key crypto failure");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
        LlmConfigError::Db(_) => {
            tracing::error!(error = %err, "llm provider db failure");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    };
    (status, Json(json!({ "message": message }))).into_response()
}

async fn list_handler(State(state): State<AppState>) -> Response {
    match list_providers(&state.pool).await {
        Ok(items) => {
            let total = items.len();
            (
                StatusCode::OK,
                Json(json!({ "items": items, "total": total })),
            )
                .into_response()
        }
        Err(err) => error_response(err),
    }
}

async fn update_handler(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Json(body): Json<UpdateLlmProviderRequest>,
) -> Response {
    match update_provider(&state.pool, &provider, body).await {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
        Err(err) => error_response(err),
    }
}

#[derive(Debug, Serialize)]
struct TestResult {
    ok: bool,
    latency_ms: u64,
    message: String,
}

async fn test_handler(State(state): State<AppState>, Path(provider): Path<String>) -> Response {
    let (row, api_key) = match load_provider_credentials(&state.pool, &provider).await {
        Ok(pair) => pair,
        Err(err) => return error_response(err),
    };
    if row.model.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "model is required before testing" })),
        )
            .into_response();
    }

    // Bound the probe: a misconfigured endpoint must not hold the request open
    // for the full production timeout.
    let timeout = Duration::from_secs(row.timeout_secs.min(30) as u64);
    let client = match reqwest::Client::builder()
        .no_proxy()
        .timeout(timeout)
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            tracing::error!(error = %err, "llm test client build failed");
            return error_response(LlmConfigError::Crypto);
        }
    };

    let url = format!("{}/chat/completions", row.base_url.trim_end_matches('/'));
    let payload = json!({
        "model": row.model,
        "messages": [{ "role": "user", "content": "ping" }],
        "max_tokens": 16,
        "stream": false
    });
    let mut request = client.post(&url).json(&payload);
    if let Some(key) = api_key.as_deref().filter(|key| !key.is_empty()) {
        request = request.bearer_auth(key);
    }

    let started = Instant::now();
    let result = match request.send().await {
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let latency = started.elapsed().as_millis() as u64;
            if status.is_success() {
                TestResult {
                    ok: true,
                    latency_ms: latency,
                    message: format!("HTTP {}", status.as_u16()),
                }
            } else {
                TestResult {
                    ok: false,
                    latency_ms: latency,
                    message: format!("HTTP {}: {}", status.as_u16(), brief(&body)),
                }
            }
        }
        Err(err) => TestResult {
            ok: false,
            latency_ms: started.elapsed().as_millis() as u64,
            message: if err.is_timeout() {
                "request timed out".to_string()
            } else {
                brief(&err.to_string())
            },
        },
    };

    (StatusCode::OK, Json(result)).into_response()
}

/// 截断上游/传输层错误，避免把整段响应体（可能含不可信模型输出）回显给前端。
fn brief(text: &str) -> String {
    text.chars().take(200).collect()
}
