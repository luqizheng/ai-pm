//! 系统级配置管理面（`/v1/admin/*`；`/admin/settings` 页的后端）。
//!
//! **门语义**：本面为**系统级单例**配置（platform schema，与租户无关）；写门 =
//! 调用方 `is_admin`（AuthedPrincipal，公司内部单栈部署下 tenant admin 即系统
//! 管理员——空库首管理员注册流）。若未来转多租户 SaaS，需引入平台级角色收紧
//！（handler 层登记）。
//!
//! **当前面**：embedding 供给配置（D-1 = bge-m3 @ Ollama 本地）——endpoint +
//! model + enabled。`enabled=true` 表示意图：Vector 通道本体属 G-3 门控，消费
//! 点在通道启用时读本表。
//!
//! **测试连接**：`POST /test` 用已存（或请求体覆盖的）endpoint/model 实调
//! `/embeddings`，返回 `{ok, dims, latency_ms, error}`——管理页"测试连接"按钮
//! 的供给面。出站请求 `.no_proxy()`（本机死代理先例）+ 10s 超时。

use axum::extract::{Extension, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use pm_application::repo::platform_ops::PlatformOpsRepo;
use crate::support::repos::platform_ops_repo;
use crate::support::{ApiError, ApiResult};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/admin/embedding-config",
            get(get_embedding_config).put(put_embedding_config),
        )
        .route(
            "/v1/admin/embedding-config/test",
            post(test_embedding_config),
        )
}

/// 系统级写门（is_admin；多租户 SaaS 化时此处收紧——见模块注释）。
fn require_system_admin(caller: &AuthedPrincipal) -> ApiResult<()> {
    if !caller.is_admin {
        return Err(ApiError::Forbidden(
            "embedding config requires a system administrator",
        ));
    }
    Ok(())
}

fn row_to_json(endpoint: &str, model: &str, enabled: bool, updated_at: chrono::DateTime<chrono::Utc>) -> serde_json::Value {
    json!({
        "endpoint": endpoint,
        "model": model,
        "enabled": enabled,
        "updated_at": updated_at,
    })
}

async fn load_config(pool: &sqlx::PgPool) -> Result<serde_json::Value, sqlx::Error> {
    let (endpoint, model, enabled, updated_at) =
        platform_ops_repo(pool).load_embedding_config().await?;
    Ok(row_to_json(&endpoint, &model, enabled, updated_at))
}

/// `GET /v1/admin/embedding-config`：读配置（管理员）。
async fn get_embedding_config(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
) -> ApiResult<Json<serde_json::Value>> {
    require_system_admin(&caller)?;
    let config = load_config(&state.pool).await.map_err(ApiError::Internal)?;
    Ok(Json(config))
}

#[derive(Debug, Deserialize)]
struct EmbeddingConfigBody {
    endpoint: String,
    model: String,
    #[serde(default)]
    enabled: bool,
}

/// `PUT /v1/admin/embedding-config`：保存配置（管理员；last-write，审计走
/// governance.audit_log 由调用方 tenant 派生——与 kb_service 同款）。
async fn put_embedding_config(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Json(body): Json<EmbeddingConfigBody>,
) -> ApiResult<Json<serde_json::Value>> {
    require_system_admin(&caller)?;
    let endpoint = body.endpoint.trim().to_string();
    let model = body.model.trim().to_string();
    if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
        return Err(ApiError::BadRequest(
            "endpoint must start with http:// or https://",
        ));
    }
    if model.is_empty() {
        return Err(ApiError::BadRequest("model must not be empty"));
    }
    let updated = platform_ops_repo(&state.pool)
        .update_embedding_config(&endpoint, &model, body.enabled, caller.principal_id)
        .await
        .map_err(ApiError::Internal)?;
    let (endpoint, model, enabled, updated_at) = updated;
    Ok(Json(json!({
        "endpoint": endpoint,
        "model": model,
        "enabled": enabled,
        "updated_at": updated_at,
    })))
}

#[derive(Debug, Deserialize)]
struct TestConfigBody {
    /// 留空 = 用已存配置测试（先保存再测试，或两者都传）。
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

/// `POST /v1/admin/embedding-config/test`：连通性测试（管理员）。
///
/// 用已存或覆盖的 endpoint/model 实调 `{endpoint}/embeddings`（input=固定探针），
/// 返回 ok/dims/latency_ms/error——不写库。
async fn test_embedding_config(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Json(body): Json<TestConfigBody>,
) -> ApiResult<Json<serde_json::Value>> {
    require_system_admin(&caller)?;
    let (endpoint, model) = match (body.endpoint.as_deref(), body.model.as_deref()) {
        (Some(ep), Some(m)) if !ep.trim().is_empty() && !m.trim().is_empty() => {
            (ep.trim().to_string(), m.trim().to_string())
        }
        _ => {
            let cfg = load_config(&state.pool).await.map_err(ApiError::Internal)?;
            (
                cfg["endpoint"].as_str().unwrap_or("").to_string(),
                cfg["model"].as_str().unwrap_or("").to_string(),
            )
        }
    };
    if endpoint.is_empty() || model.is_empty() {
        return Err(ApiError::BadRequest(
            "no config to test: save first or pass endpoint/model",
        ));
    }

    let client = reqwest::Client::builder().no_proxy().build().map_err(|e| {
        ApiError::Text(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("http client: {e}"),
        )
    })?;
    let started = std::time::Instant::now();
    // 探针超时必须短于网关转发超时（3s）：超过就会先吃网关 504，
    // 前端拿不到结构化失败（2026-11-13 UI 冒烟：2.10 无 GPU 冷启动
    // 触发 3s 边界）。2.5s 内出不来即按失败返回，body 仍是 200 JSON。
    let result = client
        .post(format!("{}/embeddings", endpoint.trim_end_matches('/')))
        .json(&json!({ "model": model, "input": "embedding-config-test" }))
        .timeout(std::time::Duration::from_millis(2500))
        .send()
        .await;

    let latency_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(resp) if resp.status().is_success() => {
            let payload: serde_json::Value = resp.json().await.map_err(|e| {
                ApiError::Text(
                    axum::http::StatusCode::BAD_GATEWAY,
                    format!("embedding response unreadable: {e}"),
                )
            })?;
            let dims = payload["data"][0]["embedding"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0);
            Ok(Json(json!({
                "ok": dims > 0,
                "dims": dims,
                "latency_ms": latency_ms,
                "error": if dims > 0 { serde_json::Value::Null } else { json!("empty embedding") },
            })))
        }
        Ok(resp) => Ok(Json(json!({
            "ok": false,
            "dims": 0,
            "latency_ms": latency_ms,
            "error": format!("provider returned HTTP {}", resp.status()),
        }))),
        Err(err) => Ok(Json(json!({
            "ok": false,
            "dims": 0,
            "latency_ms": latency_ms,
            "error": format!("request failed: {err}"),
        }))),
    }
}
