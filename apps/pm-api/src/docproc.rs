//! Document Processing 用户面 API（TD-02 §3 全端点；统一前缀
//! `/v1/document-processing`，网关侧为 `/api/v1/document-processing`）。
//!
//! 信任与边界：tenant 从 `AuthedPrincipal` 投影（不信任请求体）；可选
//! project 经 `project_visible` 成员校验；存储经 `AppState.docproc_store`
//! （未配置 file-service → 503，同 files.rs 诚实降级）。

use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use pm_application::docproc_service as svc;
use pm_application::docproc_service::{DocprocError, StoreError};

/// 上传上限（file-service 100MiB 对齐，TD-02 §3.1）。
const MAX_BODY_BYTES: usize = 100 * 1024 * 1024;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/document-processing/documents",
            post(upload_handler).get(list_documents_handler),
        )
        .route(
            "/v1/document-processing/documents/{document_id}/versions",
            get(list_versions_handler),
        )
        .route(
            "/v1/document-processing/documents/{document_id}/process",
            post(create_job_handler),
        )
        .route("/v1/document-processing/jobs/{job_id}", get(job_handler))
        .route(
            "/v1/document-processing/jobs/{job_id}/report",
            get(job_report_handler),
        )
        .route(
            "/v1/document-processing/jobs/{job_id}/cancel",
            post(cancel_job_handler),
        )
        .route(
            "/v1/document-processing/jobs/{job_id}/retry",
            post(retry_job_handler),
        )
        .route(
            "/v1/document-processing/artifacts/{artifact_id}",
            get(artifact_handler),
        )
        .route("/v1/document-processing/compare", post(compare_handler))
        .route("/v1/document-processing/export", post(export_handler))
        // 上传路由 100MiB（axum 默认 2MB 会先 413）
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

/* ── 错误映射（TD-02 §3.9） ────────────────────────────────────────────── */

fn err_resp(e: &DocprocError) -> Response {
    let (status, code) = match e {
        DocprocError::UnsupportedFormat(_) => (StatusCode::BAD_REQUEST, "unsupported_format"),
        DocprocError::MagicMismatch(_) => (StatusCode::BAD_REQUEST, "magic_mismatch"),
        DocprocError::EmptyFile => (StatusCode::BAD_REQUEST, "empty_file"),
        DocprocError::TooLarge(_) => (StatusCode::PAYLOAD_TOO_LARGE, "file_too_large"),
        DocprocError::Encrypted(_) => (StatusCode::CONFLICT, "encrypted_document"),
        DocprocError::Corrupted(_) => (StatusCode::CONFLICT, "corrupted_container"),
        DocprocError::ArchiveBomb(_) => (StatusCode::CONFLICT, "archive_bomb"),
        DocprocError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
        DocprocError::Conflict(_) => (StatusCode::CONFLICT, "state_conflict"),
        DocprocError::Store(StoreError::Unavailable(_)) => {
            (StatusCode::BAD_GATEWAY, "storage_unavailable")
        }
        DocprocError::Store(_) => (StatusCode::BAD_GATEWAY, "storage_rejected"),
        DocprocError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_db_error"),
    };
    tracing::warn!(code, error = %e, "docproc api error");
    (
        status,
        Json(json!({ "error": { "code": code, "message": e.to_string() } })),
    )
        .into_response()
}

fn store_of(state: &AppState) -> Result<Arc<dyn svc::ArtifactStore>, Response> {
    state.docproc_store.clone().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": { "code": "storage_unconfigured",
                "message": "file-service not configured (missing SERVICE_BOOTSTRAP_TOKEN)" } })),
        )
            .into_response()
    })
}

/// 可选 project 的成员可见性（跨租户/非成员按不存在处理）。
async fn check_project(
    pool: &sqlx::PgPool,
    caller: &AuthedPrincipal,
    project_id: Option<Uuid>,
) -> Result<(), Response> {
    let Some(pid) = project_id else { return Ok(()) };
    match crate::support::project_visible(pool, pid, caller.tenant_id, caller.principal_id).await {
        Ok(true) => Ok(()),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "not_found", "message": "project not found" } })),
        )
            .into_response()),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": { "code": "visibility_check_failed", "message": err.to_string() } })),
        )
            .into_response()),
    }
}

/* ── 上传（TD-02 §3.1） ────────────────────────────────────────────────── */

fn mime_for_ext(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pdf" => "application/pdf",
        "md" | "markdown" => "text/markdown",
        "html" | "htm" => "text/html",
        "csv" => "text/csv",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

async fn upload_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, Response> {
    let store = store_of(&state)?;
    let mut file: Option<(String, axum::body::Bytes)> = None;
    let mut project_id: Option<Uuid> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| bad_request("invalid multipart body"))?
    {
        match field.name() {
            Some("project_id") => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| bad_request("project_id read failed"))?;
                if !text.trim().is_empty() {
                    project_id = Some(
                        text.trim()
                            .parse()
                            .map_err(|_| bad_request("project_id must be uuid"))?,
                    );
                }
            }
            Some("file") => {
                let filename = field.file_name().unwrap_or("unnamed").to_string();
                let data = field
                    .bytes()
                    .await
                    .map_err(|_| bad_request("file read failed"))?;
                file = Some((filename, data));
            }
            _ => {}
        }
    }
    let (filename, data) = file.ok_or_else(|| bad_request("missing file field"))?;
    check_project(&state.pool, &caller, project_id).await?;

    let ext = filename.rsplit('.').next().unwrap_or("");
    let result = svc::create_document(
        store.as_ref(),
        &state.pool,
        svc::CreateDocumentInput {
            tenant_id: caller.tenant_id,
            project_id,
            filename: &filename,
            mime_type: mime_for_ext(ext),
            bytes: &data,
            uploaded_by: caller.principal_id,
            tenant_key: &caller.claims_tenant_key,
        },
        MAX_BODY_BYTES,
    )
    .await
    .map_err(|e| err_resp(&e))?;

    Ok(Json(json!({
        "document_id": if result.duplicate { lookup_document_id(&state, &caller, &result).await } else { result.document_id },
        "version_id": result.version_id,
        "version_number": result.version_number,
        "checksum": result.checksum,
        "file_size": result.file_size,
        "duplicate": result.duplicate,
    })))
}

/// dedup 命中时 create_document 未回 document_id——按 checksum 反查一次。
async fn lookup_document_id(
    state: &AppState,
    caller: &AuthedPrincipal,
    v: &svc::DocumentVersion,
) -> Uuid {
    sqlx::query_scalar(
        "SELECT d.id FROM docproc.document d \
         JOIN docproc.document_version ver ON ver.document_id = d.id \
         WHERE ver.id = $1 AND d.tenant_id = $2",
    )
    .bind(v.version_id)
    .bind(caller.tenant_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or_else(|_| Uuid::nil())
}

fn bad_request(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": { "code": "bad_request", "message": message } })),
    )
        .into_response()
}

/* ── 建任务（TD-02 §3.2/3.6/3.7） ──────────────────────────────────────── */

#[derive(Deserialize)]
struct ProcessBody {
    target_format: Option<String>,
    #[serde(default)]
    options: Option<serde_json::Value>,
}

async fn create_job_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(document_id): Path<Uuid>,
    Json(body): Json<ProcessBody>,
) -> Result<Json<serde_json::Value>, Response> {
    let target = body.target_format.as_deref().unwrap_or("markdown");
    let (processing_type, config): (&'static str, serde_json::Value) = match target {
        "markdown" => ("parse", body.options.clone().unwrap_or_else(|| json!({}))),
        other => {
            return Err(bad_request(&format!(
                "unsupported target_format {other} (markdown only via /process)"
            )))
        }
    };
    let job = svc::create_job(
        &state.pool,
        svc::CreateJobInput {
            tenant_id: caller.tenant_id,
            document_id,
            processing_type,
            config,
            created_by: caller.principal_id,
        },
    )
    .await
    .map_err(|e| err_resp(&e))?;
    Ok(Json(json!({
        "job_id": job.job_id,
        "status": job.status,
        "existed": job.existed,
    })))
}

/// compare/export：从源 artifact 反查 document_version 绑定 job。
async fn job_for_artifact(
    state: &AppState,
    caller: &AuthedPrincipal,
    artifact_id: Uuid,
) -> Result<Uuid, Response> {
    let view = svc::artifact_view(&state.pool, caller.tenant_id, artifact_id)
        .await
        .map_err(|e| err_resp(&e))?;
    let version_id: Uuid =
        sqlx::query_scalar("SELECT document_version_id FROM docproc.processing_job WHERE id = $1")
            .bind(view.processing_job_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| {
                (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "not_found", "message": "source job not found" } })),
        )
            .into_response()
            })?;
    let document_id: Uuid =
        sqlx::query_scalar("SELECT document_id FROM docproc.document_version WHERE id = $1")
            .bind(version_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| not_found())?;
    Ok(document_id)
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": { "code": "not_found", "message": "not found" } })),
    )
        .into_response()
}

async fn compare_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, Response> {
    let left: Uuid = body
        .get("left_artifact_id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| bad_request("left_artifact_id required"))?;
    let right: Uuid = body
        .get("right_artifact_id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| bad_request("right_artifact_id required"))?;
    let document_id = job_for_artifact(&state, &caller, left).await?;
    let job = svc::create_job(
        &state.pool,
        svc::CreateJobInput {
            tenant_id: caller.tenant_id,
            document_id,
            processing_type: "compare",
            config: json!({ "left_artifact_id": left, "right_artifact_id": right }),
            created_by: caller.principal_id,
        },
    )
    .await
    .map_err(|e| err_resp(&e))?;
    Ok(Json(
        json!({ "job_id": job.job_id, "status": job.status, "existed": job.existed }),
    ))
}

#[derive(Deserialize)]
struct ExportBody {
    artifact_id: Uuid,
    target_format: Option<String>,
}

async fn export_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Json(body): Json<ExportBody>,
) -> Result<Json<serde_json::Value>, Response> {
    let document_id = job_for_artifact(&state, &caller, body.artifact_id).await?;
    let job = svc::create_job(
        &state.pool,
        svc::CreateJobInput {
            tenant_id: caller.tenant_id,
            document_id,
            processing_type: "export",
            config: json!({
                "artifact_id": body.artifact_id,
                "target_format": body.target_format.unwrap_or_else(|| "html".into()),
            }),
            created_by: caller.principal_id,
        },
    )
    .await
    .map_err(|e| err_resp(&e))?;
    Ok(Json(
        json!({ "job_id": job.job_id, "status": job.status, "existed": job.existed }),
    ))
}

/* ── 查询（TD-02 §3.3-3.5/3.8） ────────────────────────────────────────── */

async fn job_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Response> {
    let view = svc::job_view(&state.pool, caller.tenant_id, job_id)
        .await
        .map_err(|e| err_resp(&e))?;
    Ok(Json(
        serde_json::to_value(&view).expect("job view serialize"),
    ))
}

async fn job_report_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Response> {
    let view = svc::job_view(&state.pool, caller.tenant_id, job_id)
        .await
        .map_err(|e| err_resp(&e))?;
    let report = view
        .artifacts
        .iter()
        .find(|a| a.artifact_type == "conversion_report")
        .map(|a| a.metadata.clone())
        .unwrap_or_else(|| json!({ "note": "no conversion report yet" }));
    Ok(Json(
        json!({ "job_id": job_id, "status": view.status, "report": report }),
    ))
}

/// markdown 产物 ≤1MiB 内联正文（TD-02 §3.4，免二次请求）。
const INLINE_CONTENT_LIMIT: usize = 1024 * 1024;

async fn artifact_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(artifact_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Response> {
    let store = store_of(&state)?;
    let view = svc::artifact_view(&state.pool, caller.tenant_id, artifact_id)
        .await
        .map_err(|e| err_resp(&e))?;
    let mut out = serde_json::to_value(&view).expect("artifact view serialize");
    match store
        .presign(&caller.claims_tenant_key, &view.storage_key)
        .await
    {
        Ok(url) => {
            out["download_url"] = json!(url);
        }
        Err(e) => {
            tracing::warn!(error = %e, "docproc presign failed");
        }
    }
    if view.artifact_type == "markdown" {
        if let Ok((content, _)) = svc::artifact_markdown(
            &state.pool,
            store.as_ref(),
            caller.tenant_id,
            &caller.claims_tenant_key,
            artifact_id,
        )
        .await
        {
            if content.len() <= INLINE_CONTENT_LIMIT {
                out["content"] = json!(content);
            }
        }
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    project_id: Option<Uuid>,
    #[serde(default)]
    limit: Option<i64>,
}

async fn list_documents_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(q): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, Response> {
    check_project(&state.pool, &caller, q.project_id).await?;
    let items = svc::list_documents(
        &state.pool,
        caller.tenant_id,
        q.project_id,
        q.limit.unwrap_or(50),
    )
    .await
    .map_err(|e| err_resp(&e))?;
    Ok(Json(json!({ "items": items })))
}

async fn list_versions_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(document_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Response> {
    let items = svc::list_versions(&state.pool, caller.tenant_id, document_id)
        .await
        .map_err(|e| err_resp(&e))?;
    Ok(Json(json!({ "items": items })))
}

/* ── 任务控制（TD-04 §3：取消 / 人工重试） ─────────────────────────────── */

async fn cancel_job_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Response> {
    svc::cancel_job(&state.pool, caller.tenant_id, job_id)
        .await
        .map_err(|e| err_resp(&e))?;
    Ok(Json(json!({ "job_id": job_id, "status": "cancelled" })))
}

async fn retry_job_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Response> {
    svc::retry_job(&state.pool, caller.tenant_id, job_id)
        .await
        .map_err(|e| err_resp(&e))?;
    Ok(Json(json!({ "job_id": job_id, "status": "pending" })))
}
