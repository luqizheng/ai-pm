//! `/install` 与 `/install/{tool}`：执行端接入引导（T-080）。
//!
//! **公开端点**（免鉴权）——agent 在拿到 API Key 之前就要拉取它。
//!
//! 两种形态，同一个 URL：
//!
//! - 默认 Markdown（人打开就能照着做）；
//! - `Accept: application/json` 或 `?format=json` → JSON（agent 读）。
//!
//! 两者来自同一份 `Guide`（`pm-application::onboarding`），不存在两套内容源。
//! **任何凭据都不出现在响应里**：key 只以 `<API_KEY>` 占位符出现。

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::routes::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/install", get(catalog_handler))
        .route("/install/{tool}", get(guide_handler))
}

/// `?format=json`（`Accept` 优先；两者不冲突）。
#[derive(Debug, Deserialize)]
struct FormatQuery {
    format: Option<String>,
}

/// 平台地址与来源：`AI_PM_PUBLIC_BASE_URL` 缺省时回落回环地址，并标记来源。
fn platform_base_url() -> (String, pm_application::onboarding::BaseUrlSource) {
    match std::env::var("AI_PM_PUBLIC_BASE_URL") {
        Ok(raw) if !raw.trim().is_empty() => {
            let trimmed = raw.trim().trim_end_matches('/').to_string();
            if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
                tracing::warn!(
                    base_url = %trimmed,
                    "AI_PM_PUBLIC_BASE_URL is not an http(s) URL; falling back to the default"
                );
                return (
                    pm_application::onboarding::DEFAULT_BASE_URL.to_string(),
                    pm_application::onboarding::BaseUrlSource::Default,
                );
            }
            (trimmed, pm_application::onboarding::BaseUrlSource::Env)
        }
        _ => (
            pm_application::onboarding::DEFAULT_BASE_URL.to_string(),
            pm_application::onboarding::BaseUrlSource::Default,
        ),
    }
}

/// 响应形态：Markdown（默认）还是 JSON。
enum Format {
    Markdown,
    Json,
}

/// 协商失败：只带一个可读原因，响应在调用处构造。
enum NegotiateError {
    UnsupportedFormat(String),
}

impl IntoResponse for NegotiateError {
    fn into_response(self) -> Response {
        match self {
            NegotiateError::UnsupportedFormat(raw) => (
                StatusCode::BAD_REQUEST,
                Json(json!({ "message": format!("unsupported format: {raw}") })),
            )
                .into_response(),
        }
    }
}

/// 形态判定：显式 `?format=` 优先于 `Accept`；非法值不静默回落。
fn negotiate(headers: &HeaderMap, query: &FormatQuery) -> Result<Format, NegotiateError> {
    if let Some(raw) = query.format.as_deref() {
        return match raw {
            "json" => Ok(Format::Json),
            "markdown" | "md" => Ok(Format::Markdown),
            other => Err(NegotiateError::UnsupportedFormat(other.to_string())),
        };
    }
    let accepts_json = headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.contains("application/json") && !value.contains("text/markdown"))
        .unwrap_or(false);
    Ok(if accepts_json {
        Format::Json
    } else {
        Format::Markdown
    })
}

/// `GET /install`：工具清单。
async fn catalog_handler(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<FormatQuery>,
) -> Response {
    let format = match negotiate(&headers, &query) {
        Ok(format) => format,
        Err(err) => return err.into_response(),
    };
    let (base_url, source) = platform_base_url();
    let catalog = pm_application::onboarding::tool_catalog(&base_url, source);
    match format {
        Format::Json => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            serde_json::to_string(&catalog).unwrap_or_else(|_| "{}".to_string()),
        )
            .into_response(),
        Format::Markdown => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
            pm_application::onboarding::render::catalog_to_markdown(&catalog),
        )
            .into_response(),
    }
}

/// `GET /install/{tool}`：分工具接入指引。未知 slug 落通用条目（**不 404**）。
async fn guide_handler(
    State(_state): State<AppState>,
    Path(tool): Path<String>,
    headers: HeaderMap,
    Query(query): Query<FormatQuery>,
) -> Response {
    if !pm_application::onboarding::is_valid_slug(&tool) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "invalid tool slug" })),
        )
            .into_response();
    }
    let format = match negotiate(&headers, &query) {
        Ok(format) => format,
        Err(err) => return err.into_response(),
    };
    let (base_url, source) = platform_base_url();
    let guide = pm_application::onboarding::install_guide(&tool, &base_url, source);
    match format {
        Format::Json => {
            // T-089：JSON 形态增 `bootstrap` 段——技能包地址、release manifest
            // 摘要与 Bootstrap 端点清单（Codex Bootstrap V1.3 §〇-B 裁清点 5：
            // `/install/{tool}` 是唯一稳定入口）。Markdown 形态不加（给人看的
            // 指引由 T-090 按七步 lifecycle 重写）。
            let manifest = crate::skills_package::release_manifest();
            let bootstrap = serde_json::json!({
                "package_url": format!(
                    "{}/install/packages/pm-ai-skills-{}.zip",
                    guide.platform_base_url,
                    crate::skills_package::PACKAGE_VERSION
                ),
                "release_manifest": manifest,
                "endpoints": {
                    "devices": format!("{}/v1/pm/devices", guide.platform_base_url),
                    "bindings": format!(
                        "{}/v1/pm/devices/{{device_id}}/bindings",
                        guide.platform_base_url
                    ),
                    "checks": format!(
                        "{}/v1/pm/devices/{{device_id}}/checks",
                        guide.platform_base_url
                    ),
                    "revoke": format!(
                        "{}/v1/pm/devices/{{device_id}}/revoke",
                        guide.platform_base_url
                    ),
                },
                "key_handling": "API Key 由用户当面提供；只写入本工具的 MCP 配置，不进入安装指令、URL 或日志。",
            });
            let mut value = serde_json::to_value(&guide).unwrap_or_else(|_| serde_json::json!({}));
            value["bootstrap"] = bootstrap;
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string()),
            )
                .into_response()
        }
        Format::Markdown => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
            pm_application::onboarding::render::to_markdown(&guide),
        )
            .into_response(),
    }
}
