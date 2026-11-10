//! Document Processing 服务层（TD-02 §3 契约的实施核心）。
//!
//! 模块真源：docs/ai-pm-document-processing-td02-service-api-v1.md +
//! TD-01 数据模型。SQL 直书（仓内 kb_service 同惯例）；对象存储经
//! [`ArtifactStore`] 端口抽象（file-service 适配在 pm-infrastructure，
//! 测试用 [`MemoryStore`]）。

use sha2::{Digest, Sha256};
use uuid::Uuid;

/// 存储端口（TD-02 依赖边界：业务模块与 worker 只经此访问对象存储）。
#[async_trait::async_trait]
pub trait ArtifactStore: Send + Sync {
    /// 上传字节 → file-service 文件ID（storage_key）。
    async fn put(
        &self,
        tenant_key: &str,
        filename: &str,
        data: Vec<u8>,
    ) -> Result<String, StoreError>;
    /// 按 storage_key 取字节。
    async fn get(&self, tenant_key: &str, storage_key: &str) -> Result<Vec<u8>, StoreError>;
    /// 短时效预签 URL（浏览器免 JWT 直取）。
    async fn presign(&self, tenant_key: &str, storage_key: &str) -> Result<String, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("storage unavailable: {0}")]
    Unavailable(String),
    #[error("storage rejected: {0}")]
    Rejected(String),
}

/// 测试/离线内存实现（字节存进程内；storage_key = uuid）。
pub struct MemoryStore {
    inner: std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>,
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            inner: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl ArtifactStore for MemoryStore {
    async fn put(
        &self,
        _tenant_key: &str,
        _filename: &str,
        data: Vec<u8>,
    ) -> Result<String, StoreError> {
        let key = Uuid::new_v4().to_string();
        self.inner
            .lock()
            .expect("memory store lock")
            .insert(key.clone(), data);
        Ok(key)
    }

    async fn get(&self, _tenant_key: &str, storage_key: &str) -> Result<Vec<u8>, StoreError> {
        self.inner
            .lock()
            .expect("memory store lock")
            .get(storage_key)
            .cloned()
            .ok_or_else(|| StoreError::Rejected(format!("missing {storage_key}")))
    }

    async fn presign(&self, _tenant_key: &str, storage_key: &str) -> Result<String, StoreError> {
        Ok(format!("mem://{storage_key}"))
    }
}

/* ── 限制与环境配置（TD-04 §6） ─────────────────────────────────────────── */

pub const DEFAULT_MAX_UPLOAD_BYTES: usize = 100 * 1024 * 1024;
pub const DEFAULT_LEASE_SECS: i64 = 300;
pub const DEFAULT_JOB_TIMEOUT_SECS: u64 = 120;
pub const DEFAULT_MAX_ATTEMPTS: i32 = 3;
pub const DEFAULT_POLL_INTERVAL_MS: u64 = 1000;

/* ── 错误 ──────────────────────────────────────────────────────────────── */

#[derive(Debug, thiserror::Error)]
pub enum DocprocError {
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("magic mismatch: {0}")]
    MagicMismatch(String),
    #[error("empty file")]
    EmptyFile,
    #[error("file too large: {0} bytes")]
    TooLarge(usize),
    #[error("encrypted document: {0}")]
    Encrypted(String),
    #[error("corrupted container: {0}")]
    Corrupted(String),
    #[error("archive bomb rejected: {0}")]
    ArchiveBomb(String),
    #[error("document not found")]
    NotFound,
    #[error("storage: {0}")]
    Store(#[from] StoreError),
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("state conflict: {0}")]
    Conflict(String),
}

/* ── 输入契约 ──────────────────────────────────────────────────────────── */

pub struct CreateDocumentInput<'a> {
    pub tenant_id: Uuid,
    pub project_id: Option<Uuid>,
    pub filename: &'a str,
    pub mime_type: &'a str,
    pub bytes: &'a [u8],
    pub uploaded_by: Uuid,
    /// 调用方的上游租户键（file-service tid 镜像；"- "= dev 映射）
    pub tenant_key: &'a str,
}

#[derive(Debug)]
pub struct DocumentVersion {
    pub document_id: Uuid,
    pub version_id: Uuid,
    pub version_number: i32,
    pub checksum: String,
    pub file_size: i64,
    pub duplicate: bool,
}

pub struct CreateJobInput {
    pub tenant_id: Uuid,
    pub document_id: Uuid,
    pub processing_type: &'static str,
    pub config: serde_json::Value,
    pub created_by: Uuid,
}

#[derive(Debug)]
pub struct JobRef {
    pub job_id: Uuid,
    pub status: String,
    pub existed: bool,
}

/* ── 白名单与魔数（规格 §9.1/9.3；TD-03 §1 双检） ──────────────────────── */

/// sha256 hex。
pub fn checksum(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    format!("{:x}", h.finalize())
}

fn sniff_kind(ext: &str, bytes: &[u8]) -> Result<&'static str, DocprocError> {
    let ext = ext.to_ascii_lowercase();
    if !crate::docproc_parse::SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(DocprocError::UnsupportedFormat(ext));
    }
    let head = &bytes[..bytes.len().min(8)];
    let zip = head.starts_with(b"PK");
    let matches = match ext.as_str() {
        "docx" | "pptx" | "xlsx" => zip,
        "pdf" => head.starts_with(b"%PDF"),
        "html" | "htm" => bytes.contains(&b'<'),
        "md" | "markdown" | "txt" | "csv" => true,
        _ => false,
    };
    if !matches {
        return Err(DocprocError::MagicMismatch(ext));
    }
    Ok(match ext.as_str() {
        "docx" => "docx",
        "pptx" => "pptx",
        "xlsx" => "xlsx",
        "pdf" => "pdf",
        "html" | "htm" => "html",
        "md" | "markdown" => "md",
        "txt" => "txt",
        "csv" => "csv",
        _ => unreachable!("whitelist guard"),
    })
}

/* ── 上传（TD-02 §3.1） ────────────────────────────────────────────────── */

pub async fn create_document(
    store: &dyn ArtifactStore,
    pool: &sqlx::PgPool,
    input: CreateDocumentInput<'_>,
    max_upload: usize,
) -> Result<DocumentVersion, DocprocError> {
    if input.bytes.is_empty() {
        return Err(DocprocError::EmptyFile);
    }
    if input.bytes.len() > max_upload {
        return Err(DocprocError::TooLarge(input.bytes.len()));
    }
    let ext = input.filename.rsplit('.').next().unwrap_or("");
    let _kind = sniff_kind(ext, input.bytes)?;

    // file-service 先落字节（失败即中止，无半行）
    let checksum = checksum(input.bytes);
    let stored_name = format!("docproc:{}:{}", input.tenant_id, input.filename);
    let storage_key = store
        .put(input.tenant_key, &stored_name, input.bytes.to_vec())
        .await?;

    let mut tx = pool.begin().await?;

    // 文档身份：(tenant, project, filename) 归并——同名新内容 = 同一 document
    // 的新版本（D-06），同 checksum = dedup 复用现有版本。
    let document_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM docproc.document \
         WHERE tenant_id = $1 AND project_id IS NOT DISTINCT FROM $2 AND filename = $3",
    )
    .bind(input.tenant_id)
    .bind(input.project_id)
    .bind(input.filename)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(doc_id) = document_id {
        // 同 checksum → dedup 返回既有版本
        let existing: Option<(Uuid, i32)> = sqlx::query_as(
            "SELECT id, version_number FROM docproc.document_version \
             WHERE document_id = $1 AND checksum = $2 ORDER BY version_number DESC LIMIT 1",
        )
        .bind(doc_id)
        .bind(&checksum)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((version_id, version_number)) = existing {
            tx.commit().await?;
            return Ok(DocumentVersion {
                document_id: doc_id,
                version_id,
                version_number,
                checksum,
                file_size: input.bytes.len() as i64,
                duplicate: true,
            });
        }
        // 新版本：version_number = max+1（U-11 并发上传由 document.row_revision
        // 乐观锁仲裁——重试整体事务即可）
        let next: i32 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version_number), 0) + 1 FROM docproc.document_version WHERE document_id = $1",
        )
        .bind(doc_id)
        .fetch_one(&mut *tx)
        .await?;
        let (version_id, version_number): (Uuid, i32) = sqlx::query_as(
            "INSERT INTO docproc.document_version \
             (tenant_id, document_id, version_number, storage_key, file_size, checksum, uploaded_by) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id, version_number",
        )
        .bind(input.tenant_id)
        .bind(doc_id)
        .bind(next)
        .bind(&storage_key)
        .bind(input.bytes.len() as i64)
        .bind(&checksum)
        .bind(input.uploaded_by)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE docproc.document SET current_version_id = $2, updated_at = now() WHERE id = $1",
        )
        .bind(doc_id)
        .bind(version_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(DocumentVersion {
            document_id: doc_id,
            version_id,
            version_number,
            checksum,
            file_size: input.bytes.len() as i64,
            duplicate: false,
        });
    }

    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO docproc.document \
         (tenant_id, project_id, filename, mime_type, file_extension, upstream_tenant_key, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(input.tenant_id)
    .bind(input.project_id)
    .bind(input.filename)
    .bind(input.mime_type)
    .bind(ext.to_ascii_lowercase())
    .bind(input.tenant_key)
    .bind(input.uploaded_by)
    .fetch_one(&mut *tx)
    .await?;

    let (version_id, version_number): (Uuid, i32) = sqlx::query_as(
        "INSERT INTO docproc.document_version \
         (tenant_id, document_id, version_number, storage_key, file_size, checksum, uploaded_by) \
         VALUES ($1, $2, 1, $3, $4, $5, $6) RETURNING id, version_number",
    )
    .bind(input.tenant_id)
    .bind(document_id)
    .bind(&storage_key)
    .bind(input.bytes.len() as i64)
    .bind(&checksum)
    .bind(input.uploaded_by)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE docproc.document SET current_version_id = $2, updated_at = now() WHERE id = $1",
    )
    .bind(document_id)
    .bind(version_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(DocumentVersion {
        document_id,
        version_id,
        version_number,
        checksum,
        file_size: input.bytes.len() as i64,
        duplicate: false,
    })
}

/* ── 建任务（TD-02 §3.2；入队幂等 TD-01 §4.3） ─────────────────────────── */

pub async fn create_job(
    pool: &sqlx::PgPool,
    input: CreateJobInput,
) -> Result<JobRef, DocprocError> {
    // 版本归属校验（tenant 边界）
    let version_id: Uuid = sqlx::query_scalar(
        "SELECT v.id FROM docproc.document_version v \
         JOIN docproc.document d ON d.id = v.document_id \
         WHERE d.id = $1 AND d.tenant_id = $2 \
         ORDER BY v.version_number DESC LIMIT 1",
    )
    .bind(input.document_id)
    .bind(input.tenant_id)
    .fetch_optional(pool)
    .await?
    .ok_or(DocprocError::NotFound)?;

    let config_version = "1";
    let fingerprint_src = serde_json::json!({
        "type": input.processing_type,
        "config_version": config_version,
        "config": input.config,
    });
    let fingerprint = checksum(fingerprint_src.to_string().as_bytes());

    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO docproc.processing_job \
         (tenant_id, document_version_id, processing_type, config_version, \
          config_fingerprint, config, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (document_version_id, processing_type, config_fingerprint) \
           WHERE status <> 'cancelled' DO NOTHING \
         RETURNING id",
    )
    .bind(input.tenant_id)
    .bind(version_id)
    .bind(input.processing_type)
    .bind(config_version)
    .bind(&fingerprint)
    .bind(&input.config)
    .bind(input.created_by)
    .fetch_optional(pool)
    .await?;

    if let Some(job_id) = inserted {
        return Ok(JobRef {
            job_id,
            status: "pending".into(),
            existed: false,
        });
    }
    // 命中既有任务
    let existing: (Uuid, String) = sqlx::query_as(
        "SELECT id, status::text FROM docproc.processing_job \
         WHERE document_version_id = $1 AND processing_type = $2 \
           AND config_fingerprint = $3 AND status <> 'cancelled'",
    )
    .bind(version_id)
    .bind(input.processing_type)
    .bind(&fingerprint)
    .fetch_one(pool)
    .await?;
    Ok(JobRef {
        job_id: existing.0,
        status: existing.1,
        existed: true,
    })
}

/* ── Worker：领取 / 完成 / 失败（TD-04 §3-4） ──────────────────────────── */

#[derive(Debug, sqlx::FromRow)]
pub struct ClaimedJob {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub document_version_id: Uuid,
    pub processing_type: String,
    pub config: serde_json::Value,
    pub attempts: i32,
    pub config_version: String,
}

pub fn lease_secs_from_env() -> i64 {
    std::env::var("DOCPROC_JOB_LEASE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_LEASE_SECS)
}

pub fn max_attempts_from_env() -> i32 {
    std::env::var("DOCPROC_MAX_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_ATTEMPTS)
}

pub fn poll_interval_from_env() -> std::time::Duration {
    std::time::Duration::from_millis(
        std::env::var("DOCPROC_WORKER_POLL_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_POLL_INTERVAL_MS),
    )
}

/// SKIP LOCKED 领取（pending 或租约过期的 processing；at-least-once）。
pub async fn claim_job(
    pool: &sqlx::PgPool,
    worker_id: &str,
    lease_secs: i64,
) -> Result<Option<ClaimedJob>, sqlx::Error> {
    sqlx::query_as::<_, ClaimedJob>(
        "UPDATE docproc.processing_job SET \
           status = 'processing', lease_owner = $1, locked_until = now() + ($2 || ' seconds')::interval, \
           started_at = COALESCE(started_at, now()), attempts = attempts + 1, updated_at = now() \
         WHERE id = (SELECT id FROM docproc.processing_job \
                     WHERE status = 'pending' \
                        OR (status = 'processing' AND locked_until < now()) \
                     ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED) \
         RETURNING id, tenant_id, document_version_id, processing_type, config, attempts, config_version",
    )
    .bind(worker_id)
    .bind(lease_secs.to_string())
    .fetch_optional(pool)
    .await
}

/// 终态写入（乐观条件：仍处 processing 才落终态——取消竞争由条件兜底）。
pub async fn finish_job(
    pool: &sqlx::PgPool,
    job_id: Uuid,
    status: &'static str,
    parser_name: &str,
    parser_version: &str,
    progress: i32,
) -> Result<bool, sqlx::Error> {
    let n = sqlx::query(
        "UPDATE docproc.processing_job SET status = $2, parser_name = $3, parser_version = $4, \
         progress = $5, completed_at = now(), lease_owner = NULL, locked_until = NULL, updated_at = now() \
         WHERE id = $1 AND status = 'processing'",
    )
    .bind(job_id)
    .bind(status)
    .bind(parser_name)
    .bind(parser_version)
    .bind(progress)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n > 0)
}

/// 失败处置：attempts < max → 回 pending（重试），否则停留 failed。
pub async fn fail_job(
    pool: &sqlx::PgPool,
    job_id: Uuid,
    error_code: &str,
    error_detail: serde_json::Value,
    max_attempts: i32,
) -> Result<bool, sqlx::Error> {
    let retried = sqlx::query_scalar::<_, bool>(
        "UPDATE docproc.processing_job SET \
           status = CASE WHEN attempts < $2 THEN 'pending' ELSE 'failed' END, \
           error_code = $3, error_detail = $4, lease_owner = NULL, locked_until = NULL, updated_at = now() \
         WHERE id = $1 AND status = 'processing' \
         RETURNING status = 'pending'",
    )
    .bind(job_id)
    .bind(max_attempts)
    .bind(error_code)
    .bind(&error_detail)
    .fetch_optional(pool)
    .await?
    .unwrap_or(false);
    Ok(retried)
}

/// 取消（pending/processing → cancelled；终态不可取消）。
pub async fn cancel_job(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    job_id: Uuid,
) -> Result<bool, DocprocError> {
    let n = sqlx::query(
        "UPDATE docproc.processing_job SET status = 'cancelled', completed_at = now(), \
         lease_owner = NULL, locked_until = NULL, updated_at = now() \
         WHERE id = $1 AND tenant_id = $2 AND status IN ('pending', 'processing')",
    )
    .bind(job_id)
    .bind(tenant_id)
    .execute(pool)
    .await?
    .rows_affected();
    if n == 0 {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM docproc.processing_job WHERE id = $1 AND tenant_id = $2)",
        )
        .bind(job_id)
        .bind(tenant_id)
        .fetch_one(pool)
        .await?;
        if !exists {
            return Err(DocprocError::NotFound);
        }
        return Err(DocprocError::Conflict(
            "job not cancellable in current status".into(),
        ));
    }
    Ok(true)
}

/// 人工重试（仅 failed；重置 pending + attempts 清零）。
pub async fn retry_job(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    job_id: Uuid,
) -> Result<(), DocprocError> {
    let n = sqlx::query(
        "UPDATE docproc.processing_job SET status = 'pending', attempts = 0, error_code = NULL, \
         error_detail = NULL, updated_at = now() \
         WHERE id = $1 AND tenant_id = $2 AND status = 'failed'",
    )
    .bind(job_id)
    .bind(tenant_id)
    .execute(pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(DocprocError::NotFound);
    }
    Ok(())
}

/* ── 产物写入（worker 用；幂等 TD-04 §4） ──────────────────────────────── */

pub struct NewArtifact<'a> {
    pub tenant_id: Uuid,
    pub job_id: Uuid,
    pub artifact_type: &'static str,
    pub storage_key: &'a str,
    pub mime_type: &'a str,
    pub checksum: &'a str,
    pub metadata: serde_json::Value,
}

/// 幂等落产物：同 (job, type, checksum) 冲突 → 返回既有 id。
pub async fn insert_artifact(pool: &sqlx::PgPool, a: NewArtifact<'_>) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO docproc.artifact \
         (tenant_id, processing_job_id, artifact_type, storage_key, mime_type, checksum, metadata) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (processing_job_id, artifact_type, checksum) DO UPDATE SET tenant_id = docproc.artifact.tenant_id \
         RETURNING id",
    )
    .bind(a.tenant_id)
    .bind(a.job_id)
    .bind(a.artifact_type)
    .bind(a.storage_key)
    .bind(a.mime_type)
    .bind(a.checksum)
    .bind(&a.metadata)
    .fetch_one(pool)
    .await
}

pub async fn insert_mappings(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    artifact_id: Uuid,
    mappings: &[crate::docproc_parse::SourceMapping],
) -> Result<(), sqlx::Error> {
    for m in mappings {
        sqlx::query(
            "INSERT INTO docproc.source_mapping \
             (tenant_id, artifact_id, source_location, target_location, mapping_type) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(tenant_id)
        .bind(artifact_id)
        .bind(&m.source_location)
        .bind(&m.target_location)
        .bind(m.mapping_type.as_str())
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/* ── 查询（TD-02 §3.3-3.5/3.8） ────────────────────────────────────────── */

#[derive(Debug, serde::Serialize)]
pub struct JobArtifactView {
    pub artifact_id: Uuid,
    pub artifact_type: String,
    pub mime_type: String,
    pub checksum: String,
    pub metadata: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, serde::Serialize)]
pub struct JobView {
    pub job_id: Uuid,
    pub status: String,
    pub processing_type: String,
    pub progress: i32,
    pub parser_name: Option<String>,
    pub parser_version: Option<String>,
    pub error_code: Option<String>,
    pub error_detail: Option<serde_json::Value>,
    pub attempts: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub artifacts: Vec<JobArtifactView>,
}

/// sqlx 行元组（clippy type_complexity 拆分）。
type JobRow = (
    Uuid,
    String,
    String,
    i32,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<serde_json::Value>,
    i32,
    chrono::DateTime<chrono::Utc>,
    Option<chrono::DateTime<chrono::Utc>>,
);

pub async fn job_view(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    job_id: Uuid,
) -> Result<JobView, DocprocError> {
    let (
        job_id_,
        status,
        processing_type,
        progress,
        parser_name,
        parser_version,
        error_code,
        error_detail,
        attempts,
        created_at,
        completed_at,
    ): JobRow = sqlx::query_as(
        "SELECT id, status::text, processing_type::text, progress, parser_name, parser_version, \
         error_code, error_detail, attempts, created_at, completed_at \
         FROM docproc.processing_job WHERE id = $1 AND tenant_id = $2",
    )
    .bind(job_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?
    .ok_or(DocprocError::NotFound)?;

    let artifacts: Vec<JobArtifactView> = sqlx::query_as(
        "SELECT id, artifact_type::text, mime_type, checksum, metadata, created_at \
         FROM docproc.artifact WHERE processing_job_id = $1 ORDER BY created_at",
    )
    .bind(job_id_)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(
        |(artifact_id, artifact_type, mime_type, checksum, metadata, created_at): (
            Uuid,
            String,
            String,
            String,
            serde_json::Value,
            chrono::DateTime<chrono::Utc>,
        )| JobArtifactView {
            artifact_id,
            artifact_type,
            mime_type,
            checksum,
            metadata,
            created_at,
        },
    )
    .collect();

    Ok(JobView {
        job_id: job_id_,
        status,
        processing_type,
        progress,
        parser_name,
        parser_version,
        error_code,
        error_detail,
        attempts,
        created_at,
        completed_at,
        artifacts,
    })
}

#[derive(Debug, serde::Serialize)]
pub struct ArtifactView {
    pub artifact_id: Uuid,
    pub artifact_type: String,
    pub mime_type: String,
    pub checksum: String,
    pub metadata: serde_json::Value,
    pub storage_key: String,
    pub processing_job_id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn artifact_view(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    artifact_id: Uuid,
) -> Result<ArtifactView, DocprocError> {
    sqlx::query_as(
        "SELECT id, artifact_type::text, mime_type, checksum, metadata, storage_key, \
         processing_job_id, created_at \
         FROM docproc.artifact WHERE id = $1 AND tenant_id = $2",
    )
    .bind(artifact_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?
    .map(
        |(
            artifact_id,
            artifact_type,
            mime_type,
            checksum,
            metadata,
            storage_key,
            processing_job_id,
            created_at,
        ): (
            Uuid,
            String,
            String,
            String,
            serde_json::Value,
            String,
            Uuid,
            chrono::DateTime<chrono::Utc>,
        )| ArtifactView {
            artifact_id,
            artifact_type,
            mime_type,
            checksum,
            metadata,
            storage_key,
            processing_job_id,
            created_at,
        },
    )
    .ok_or(DocprocError::NotFound)
}

#[derive(Debug, serde::Serialize)]
pub struct DocumentListItem {
    pub document_id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub file_extension: String,
    pub project_id: Option<Uuid>,
    pub current_version_number: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_documents(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    project_id: Option<Uuid>,
    limit: i64,
) -> Result<Vec<DocumentListItem>, DocprocError> {
    type DocRow = (
        Uuid,
        String,
        String,
        String,
        Option<Uuid>,
        Option<i32>,
        chrono::DateTime<chrono::Utc>,
    );
    let rows: Vec<DocRow> = sqlx::query_as(
        "SELECT d.id, d.filename, d.mime_type, d.file_extension, d.project_id, \
             v.version_number, d.created_at \
             FROM docproc.document d \
             LEFT JOIN docproc.document_version v ON v.id = d.current_version_id \
             WHERE d.tenant_id = $1 AND ($2::uuid IS NULL OR d.project_id = $2) \
             ORDER BY d.created_at DESC LIMIT $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(limit.clamp(1, 200))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                document_id,
                filename,
                mime_type,
                file_extension,
                project_id,
                version,
                created_at,
            )| {
                DocumentListItem {
                    document_id,
                    filename,
                    mime_type,
                    file_extension,
                    project_id,
                    current_version_number: version,
                    created_at,
                }
            },
        )
        .collect())
}

#[derive(Debug, serde::Serialize)]
pub struct VersionItem {
    pub version_id: Uuid,
    pub version_number: i32,
    pub checksum: String,
    pub file_size: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_versions(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    document_id: Uuid,
) -> Result<Vec<VersionItem>, DocprocError> {
    let rows: Vec<(Uuid, i32, String, i64, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT v.id, v.version_number, v.checksum, v.file_size, v.created_at \
         FROM docproc.document_version v \
         JOIN docproc.document d ON d.id = v.document_id \
         WHERE d.id = $1 AND d.tenant_id = $2 ORDER BY v.version_number DESC",
    )
    .bind(document_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(version_id, version_number, checksum, file_size, created_at)| VersionItem {
                version_id,
                version_number,
                checksum,
                file_size,
                created_at,
            },
        )
        .collect())
}

/// 原始版本字节（worker / 下载用；tenant 边界校验）。
pub async fn version_bytes(
    pool: &sqlx::PgPool,
    store: &dyn ArtifactStore,
    tenant_id: Uuid,
    tenant_key: &str,
    version_id: Uuid,
) -> Result<(Vec<u8>, String, String), DocprocError> {
    let (storage_key, ext, filename): (String, String, String) = sqlx::query_as(
        "SELECT v.storage_key, d.file_extension, d.filename \
         FROM docproc.document_version v JOIN docproc.document d ON d.id = v.document_id \
         WHERE v.id = $1 AND v.tenant_id = $2",
    )
    .bind(version_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?
    .ok_or(DocprocError::NotFound)?;
    let bytes = store.get(tenant_key, &storage_key).await?;
    Ok((bytes, ext, filename))
}

/// markdown 产物内容 + checksum（TD-05 §2 端口的实现基础）。
pub async fn artifact_markdown(
    pool: &sqlx::PgPool,
    store: &dyn ArtifactStore,
    tenant_id: Uuid,
    tenant_key: &str,
    artifact_id: Uuid,
) -> Result<(String, String), DocprocError> {
    let view = artifact_view(pool, tenant_id, artifact_id).await?;
    if view.artifact_type != "markdown" {
        return Err(DocprocError::NotFound);
    }
    let bytes = store.get(tenant_key, &view.storage_key).await?;
    let cs = checksum(&bytes);
    if cs != view.checksum {
        return Err(DocprocError::Conflict("artifact checksum drift".into()));
    }
    Ok((String::from_utf8_lossy(&bytes).into_owned(), cs))
}

/// 某（文档最新）版本的最新 markdown 产物引用。
pub async fn latest_markdown_artifact(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    version_id: Uuid,
) -> Result<Option<(Uuid, String)>, DocprocError> {
    Ok(sqlx::query_as::<_, (Uuid, String)>(
        "SELECT a.id, a.checksum FROM docproc.artifact a \
         JOIN docproc.processing_job j ON j.id = a.processing_job_id \
         WHERE j.document_version_id = $1 AND j.tenant_id = $2 \
           AND a.artifact_type = 'markdown' AND j.status IN ('completed', 'partial_success') \
         ORDER BY a.created_at DESC LIMIT 1",
    )
    .bind(version_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?)
}
