//! 文件代理（2026-11-07 file-service 收口配套）。
//!
//! lmms file-service 已收口为纯内部服务（`/file/*` 只认 service-jwt，
//! `aud=file-service`），网关不再暴露 `/v1/file/*` 用户路由。本模块在
//! pm-api 侧补上用户面入口：**同形状**复刻 file-service 的 REST 契约
//! （路径/请求/响应与 lmms `sdk/file-services` TS SDK 逐一对齐），前端
//! 只需把 baseUrl 从 `/gw` 换到 `/api`。
//!
//! 信任与归属模型（contracts/http-conventions.md）：
//! - 出站：单 token 服务 JWT。租户作用域由 token `tid` 驱动（file-service
//!   的 `and_tenant` 与 upload 的 `tenant_id` 均取自 tid），因此**每个请求**
//!   按 `caller.claims_tenant_key`（上游 tid 镜像）经
//!   `ServiceJwtCache::get_for_tenant` 铸造/取用 per-tenant token。
//!   pm-api 的鉴权门（`resolve_principal`）本就拒绝 cross-tenant token，
//!   故 v1 恒 `ct=false`。
//! - 归属：`user_id` 由本层强制覆写为 `caller.principal_id`——**不信任**
//!   客户端 metadata 里声明的 user_id（调用方对声明负责的“调用方”是
//!   pm-api，不是浏览器）。
//! - 客户端传入的 `tenant_id` 一律丢弃（server-derived，防越权写）。
//!
//! 复用 `vendor/digua-file-sdk`（源码快照，与 lmms 上游同源）。FileClient
//! 的 bearer 是进程级 supplier 闭包，而本层 token 按请求租户变化——为避免
//! 共享 bearer 槽的租户串号，每请求构建独立 FileClient（文件操作低频，
//! mesh 内明文 HTTP，代价可接受）。

use crate::auth::AuthedPrincipal;
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use digua_common_jwt::{ServiceJwtCache, ServiceJwtConfig, ServiceJwtError};
use digua_file_sdk::{CreatePresignUrl, FileClient, ListFilesQuery, SdkError, UploadFileMeta};
use std::sync::Arc;
use std::time::Duration;

/// file-service 的上传上限（services/file-service/src/config.rs:max_file_size，
/// 默认 100 MiB）。pm-api 作为代理必须放行同尺寸的 multipart 请求体
/// （axum 默认 2MB 会先 413）。
const MAX_BODY_BYTES: usize = 100 * 1024 * 1024;

/// The outbound file proxy handle. `None`（AppState.file）= 未配置
/// bootstrap token（裸机 dev 等），路由存在但返回 503。
pub(crate) struct FileProxy {
    file_service_url: String,
    shared_http: reqwest::Client,
    service_jwt: Arc<ServiceJwtCache>,
}

impl FileProxy {
    /// From env: `SERVICE_BOOTSTRAP_TOKEN`（必需）、`AI_PM_TOKEN_SERVICE_URL`
    /// （默认 http://token-service:8082）、`FILE_SERVICE_URL`
    /// （默认 http://file-service:8082）。缺 bootstrap ⇒ 代理禁用。
    pub fn from_env() -> Option<Arc<FileProxy>> {
        let bootstrap_token = match std::env::var("SERVICE_BOOTSTRAP_TOKEN") {
            Ok(t) if !t.trim().is_empty() => t,
            _ => {
                tracing::info!(
                    "SERVICE_BOOTSTRAP_TOKEN unset — file proxy disabled (/v1/file/* → 503)"
                );
                return None;
            }
        };
        let token_service_url = std::env::var("AI_PM_TOKEN_SERVICE_URL")
            .unwrap_or_else(|_| "http://token-service:8082".into());
        let file_service_url =
            std::env::var("FILE_SERVICE_URL").unwrap_or_else(|_| "http://file-service:8082".into());
        // 内网地址：禁系统代理（与 main.rs JWKS client 同理由）。
        let shared_http = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("file proxy reqwest build");
        let service_jwt = Arc::new(ServiceJwtCache::new(
            ServiceJwtConfig {
                token_service_url,
                bootstrap_token,
                caller_service_name: "pm-api".into(),
                // 仅 legacy get() 用；本模块一律走 get_for_tenant。
                caller_tenant: 1,
                refresh_skew: Duration::from_secs(60),
                request_timeout: Duration::from_secs(5),
            },
            shared_http.clone(),
            String::new(), // private key 只服务 test-only sign_local；生产走 HTTP 铸造
        ));
        Some(Arc::new(FileProxy {
            file_service_url,
            shared_http,
            service_jwt,
        }))
    }

    /// Per-request service-jwt：tid = 上游租户镜像（claims_tenant_key），
    /// ct 恒 false（pm-api 鉴权门拒绝 cross-tenant token）。
    async fn token(&self, caller: &AuthedPrincipal) -> Result<String, Response> {
        // dev-off principal 的 claims_tenant_key 是 "-"（off 模式无上游
        // JWT）；映射到 lmms dev 播种的租户 1，让本地无鉴权调试可用。
        let raw = caller.claims_tenant_key.trim();
        let tid: i64 = if raw == "-" {
            tracing::debug!("file proxy: dev-off principal mapped to upstream tenant 1");
            1
        } else {
            raw.parse().map_err(|_| {
                status_resp(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "file proxy: upstream tenant is not numeric",
                )
            })?
        };
        self.service_jwt
            .get_for_tenant("file-service", tid, false)
            .await
            .map_err(svc_jwt_err)
    }

    /// Per-request client bound to this request's tenant token.
    fn client_for(&self, token: String) -> FileClient {
        FileClient::new(self.file_service_url.clone(), move || Some(token.clone()))
    }
}

fn svc_jwt_err(e: ServiceJwtError) -> Response {
    tracing::warn!(error = %e, "file proxy: service-jwt mint failed");
    (
        StatusCode::BAD_GATEWAY,
        "file proxy: token mint failed".to_string(),
    )
        .into_response()
}

/// SdkError → HTTP status（file-service 的业务错误码透传，鉴权类降级 502：
/// 服务 JWT 被拒是部署配置问题，不应把 401 泄漏给前端引发重新登录循环）。
fn sdk_err(e: SdkError) -> Response {
    let (status, message) = match &e {
        SdkError::Validation {
            status, message, ..
        } => (
            StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_REQUEST),
            message.clone(),
        ),
        SdkError::RateLimit { message, .. } => (StatusCode::TOO_MANY_REQUESTS, message.clone()),
        SdkError::Auth { message, .. } | SdkError::Server { message, .. } => {
            tracing::warn!(error = %e, "file proxy: upstream rejected service call");
            (StatusCode::BAD_GATEWAY, message.clone())
        }
        SdkError::Network(m) => (StatusCode::BAD_GATEWAY, m.clone()),
    };
    (status, message).into_response()
}

pub(crate) fn router() -> Router<crate::routes::AppState> {
    Router::new()
        .route("/v1/file/upload", post(upload_handler))
        .route("/v1/file/list", get(list_handler))
        .route("/v1/file/presign", post(presign_handler))
        .route(
            "/v1/file/{id}",
            get(download_handler).delete(delete_handler),
        )
        .route("/v1/file/{id}/info", get(info_handler))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

/// AppState.file 的类型：`None` = 代理未配置。
pub(crate) type FileProxyHandle = Option<Arc<FileProxy>>;

pub(crate) fn proxy(state: Option<&Arc<FileProxy>>) -> Result<Arc<FileProxy>, Response> {
    state.cloned().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "file proxy not configured (SERVICE_BOOTSTRAP_TOKEN unset)".to_string(),
        )
            .into_response()
    })
}

/// 客户端上传 metadata（tenant_id 不存在——见模块注释；user_id 由本层覆写）。
#[derive(Debug, serde::Deserialize)]
struct IncomingUploadMeta {
    filename: String,
    #[serde(default)]
    content_type: Option<String>,
    #[serde(default)]
    is_temporary: Option<bool>,
    #[serde(default)]
    expires_in_seconds: Option<i64>,
}

async fn upload_handler(
    State(state): State<crate::routes::AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    mut multipart: Multipart,
) -> Result<Json<digua_file_sdk::FileInfo>, Response> {
    let px = proxy(state.file.as_ref())?;
    let mut meta: Option<IncomingUploadMeta> = None;
    let mut data: Option<Bytes> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| status_resp(StatusCode::BAD_REQUEST, "invalid multipart body"))?
    {
        match field.name() {
            Some("metadata") => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| status_resp(StatusCode::BAD_REQUEST, "metadata read failed"))?;
                meta = Some(serde_json::from_str(&text).map_err(|e| {
                    status_resp(
                        StatusCode::BAD_REQUEST,
                        format!("invalid metadata json: {e}"),
                    )
                })?);
            }
            Some("file") => {
                data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|_| status_resp(StatusCode::BAD_REQUEST, "file read failed"))?,
                );
            }
            _ => {}
        }
    }
    let meta =
        meta.ok_or_else(|| status_resp(StatusCode::BAD_REQUEST, "missing metadata field"))?;
    let data = data.ok_or_else(|| status_resp(StatusCode::BAD_REQUEST, "missing file field"))?;

    let token = px.token(&caller).await?;
    let out_meta = out_meta_from(meta, &caller);
    let client = px.client_for(token);
    let info = client
        .upload_file(&out_meta, &out_meta.filename, data.to_vec())
        .await
        .map_err(sdk_err)?;
    Ok(Json(info))
}

#[derive(Debug, serde::Deserialize)]
struct ListParams {
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    size: Option<u32>,
}

async fn list_handler(
    State(state): State<crate::routes::AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(q): Query<ListParams>,
) -> Result<Json<digua_file_sdk::PagedFiles>, Response> {
    let px = proxy(state.file.as_ref())?;
    let token = px.token(&caller).await?;
    let mut query = ListFilesQuery::new();
    if let Some(f) = q.filename {
        query = query.filename(f);
    }
    if let Some(u) = q.user_id {
        query = query.user_id(u);
    }
    // page/size: ListFilesQuery 无 builder 字段直接透传
    query.page = q.page;
    query.size = q.size;
    let client = px.client_for(token);
    let paged = client.list_files(&query).await.map_err(sdk_err)?;
    Ok(Json(paged))
}

async fn info_handler(
    State(state): State<crate::routes::AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(id): Path<String>,
) -> Result<Json<digua_file_sdk::FileInfo>, Response> {
    let px = proxy(state.file.as_ref())?;
    let token = px.token(&caller).await?;
    let client = px.client_for(token);
    let info = client.get_file_info(&id).await.map_err(sdk_err)?;
    Ok(Json(info))
}

async fn download_handler(
    State(state): State<crate::routes::AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(id): Path<String>,
) -> Result<Response, Response> {
    let px = proxy(state.file.as_ref())?;
    let token = px.token(&caller).await?;
    let client = px.client_for(token);
    let file = client.download_file(&id).await.map_err(sdk_err)?;
    let mut headers = HeaderMap::new();
    if let Some(ct) = &file.content_type {
        if let Ok(v) = ct.parse() {
            headers.insert(header::CONTENT_TYPE, v);
        }
    }
    if let Some(name) = &file.filename {
        if let Ok(v) = format!("attachment; filename=\"{}\"", name.replace('"', "")).parse() {
            headers.insert(header::CONTENT_DISPOSITION, v);
        }
    }
    Ok((StatusCode::OK, headers, file.bytes).into_response())
}

async fn delete_handler(
    State(state): State<crate::routes::AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, Response> {
    let px = proxy(state.file.as_ref())?;
    let token = px.token(&caller).await?;
    // file-service DELETE 无专用 SDK 方法——用共享 HTTP 客户端带 token 调。
    let url = format!("{}/file/{}", px.file_service_url.trim_end_matches('/'), id);
    let resp = px
        .shared_http
        .delete(&url)
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| status_resp(StatusCode::BAD_GATEWAY, format!("file delete: {e}")))?;
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(map_upstream_status(status, &body, "file delete"));
    }
    Ok(Json(serde_json::json!({ "success": true })))
}

async fn presign_handler(
    State(state): State<crate::routes::AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Json(req): Json<CreatePresignUrl>,
) -> Result<Json<digua_file_sdk::PresignUrlResponse>, Response> {
    let px = proxy(state.file.as_ref())?;
    let token = px.token(&caller).await?;
    let client = px.client_for(token);
    let resp = client.create_presign(&req).await.map_err(sdk_err)?;
    Ok(Json(resp))
}

fn status_resp(status: StatusCode, message: impl Into<String>) -> Response {
    (status, message.into()).into_response()
}

fn map_upstream_status(status: StatusCode, body: &str, op: &str) -> Response {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            tracing::warn!(op, %body, "file proxy: upstream rejected service call");
            (
                StatusCode::BAD_GATEWAY,
                format!("{op}: upstream auth failure"),
            )
                .into_response()
        }
        s if s.is_client_error() => (s, body.to_string()).into_response(),
        _ => (StatusCode::BAD_GATEWAY, format!("{op}: upstream failure")).into_response(),
    }
}

/// 归属覆写：以 pm-api 鉴权层的 principal 为准，不信客户端声明。
/// 客户端传入的 user_id 一律丢弃；tenant_id 不存在于 IncomingUploadMeta
/// （租户只认 token tid）。
fn out_meta_from(meta: IncomingUploadMeta, caller: &AuthedPrincipal) -> UploadFileMeta {
    UploadFileMeta {
        filename: meta.filename,
        content_type: meta.content_type,
        user_id: Some(caller.principal_id.to_string()),
        is_temporary: meta.is_temporary,
        expires_in_seconds: meta.expires_in_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use axum::http::StatusCode;
    
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_proxy(token_service_url: String, file_service_url: String) -> Arc<FileProxy> {
        let shared_http = reqwest::Client::builder().no_proxy().build().unwrap();
        let service_jwt = Arc::new(ServiceJwtCache::new(
            ServiceJwtConfig {
                token_service_url,
                bootstrap_token: "bt-pm-test".into(),
                caller_service_name: "pm-api".into(),
                caller_tenant: 1,
                refresh_skew: Duration::from_secs(60),
                request_timeout: Duration::from_secs(5),
            },
            shared_http.clone(),
            String::new(),
        ));
        Arc::new(FileProxy {
            file_service_url,
            shared_http,
            service_jwt,
        })
    }

    fn caller(tenant: &str) -> AuthedPrincipal {
        AuthedPrincipal {
            principal_id: uuid::Uuid::nil(),
            identity_id: None,
            display_name: "tester".into(),
            is_active: true,
            tenant_id: uuid::Uuid::nil(),
            tenant_role: pm_domain::TenantRole::Member,
            is_admin: false,
            auth_mode: "test".into(),
            provider: "test".into(),
            claims_tenant_key: tenant.into(),
            subject: "subject-1".into(),
            client: None,
        }
    }

    /// 全链：per-tenant 铸 token（tid=7）→ 携该 token 调 file-service 上传
    /// → user_id 被覆写为 principal_id、tenant_id 不出站。
    #[tokio::test]
    async fn upload_mints_per_tenant_token_and_overrides_attribution() {
        let token_svc = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/tokens/service-jwt"))
            .and(header("authorization", "Bearer bt-pm-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"access_token": "svc-jwt-t7", "expires_in": 3600}),
            ))
            .mount(&token_svc)
            .await;
        let file_svc = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/file/upload"))
            .and(header("authorization", "Bearer svc-jwt-t7"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "f-1", "filename": "a.pdf", "size": 3,
                "storage_path": "f-1", "tenant_id": "7", "is_temporary": false,
                "created_at": "2026-11-07T00:00:00Z", "updated_at": "2026-11-07T00:00:00Z"
            })))
            .mount(&file_svc)
            .await;

        let px = test_proxy(token_svc.uri(), file_svc.uri());
        let client = px.client_for(px.token(&caller("7")).await.unwrap());
        // 覆写逻辑走真实 out_meta_from（客户端伪造 user_id 的场景见下方单测）
        let meta = out_meta_from(
            IncomingUploadMeta {
                filename: "a.pdf".into(),
                content_type: Some("application/pdf".into()),
                is_temporary: None,
                expires_in_seconds: None,
            },
            &caller("7"),
        );
        let info = client
            .upload_file(&meta, "a.pdf", b"abc".to_vec())
            .await
            .unwrap();
        assert_eq!(info.id, "f-1");

        let received = &file_svc.received_requests().await.unwrap();
        let body = String::from_utf8_lossy(&received[0].body);
        assert!(
            body.contains(&uuid::Uuid::nil().to_string()),
            "user_id must be the overridden principal_id: {body}"
        );
        // token-service 收到的铸造请求必须带 tid=7
        let minted = &token_svc.received_requests().await.unwrap();
        let mint_body = String::from_utf8_lossy(&minted[0].body);
        assert!(
            mint_body.contains("\"tid\":7"),
            "mint must be per-tenant: {mint_body}"
        );
    }

    /// 非数字租户镜像 → 500（不外呼 file-service）。
    #[tokio::test]
    async fn token_rejects_non_numeric_tenant() {
        let px = test_proxy("http://127.0.0.1:1".into(), "http://127.0.0.1:1".into());
        let err = px.token(&caller("not-numeric")).await.unwrap_err();
        assert_eq!(err.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    /// 归属覆写纯逻辑：客户端伪造的 user_id 必须被丢弃。
    #[test]
    fn out_meta_overrides_forged_user_id() {
        let out = out_meta_from(
            IncomingUploadMeta {
                filename: "x".into(),
                content_type: None,
                is_temporary: None,
                expires_in_seconds: None,
            },
            &caller("7"),
        );
        assert_eq!(
            out.user_id.as_deref(),
            Some(uuid::Uuid::nil().to_string().as_str())
        );
    }
}
