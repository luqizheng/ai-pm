//! Document Processing worker（TD-04 §3-4：表即队列 + SKIP LOCKED 租约）。
//!
//! 执行序（单 job）：claim → 拉原始字节 → adapter 解析（spawn_blocking）→
//! 产物/图片 upload file-service → 幂等落 artifact → 事务内（mappings +
//! job 终态 + outbox 事件）。at-least-once：产物以 (job, type, checksum)
//! 唯一约束吸收重复；事件以 outbox idempotency_key 吸收重复。

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use uuid::Uuid;

use pm_application::docproc_parse::{adapter_for, ParseCtx, ParseError, ParseOutcome};
use pm_application::docproc_service as svc;
use pm_application::docproc_service::ArtifactStore;

/// 事件类型（EventCatalog 登记于 pm_application::outbox，见 0016/TD-02 §4）。
pub const EVENT_JOB_COMPLETED: &str = "DocprocJobCompleted";
pub const EVENT_JOB_FAILED: &str = "DocprocJobFailed";
pub const AGGREGATE_TYPE: &str = "docproc_job";

pub struct DocprocWorker {
    pool: PgPool,
    store: Arc<dyn ArtifactStore>,
    worker_id: String,
    lease_secs: i64,
    max_attempts: i32,
    poll_interval: Duration,
    job_timeout: Duration,
}

impl DocprocWorker {
    /// `store` 为 None（未配置 file-service）时跳过启动——诚实降级。
    pub fn from_env(pool: PgPool, store: Option<Arc<dyn ArtifactStore>>) -> Option<Self> {
        let store = store?;
        Some(Self {
            pool,
            store,
            worker_id: format!("docproc-{}", uuid::Uuid::new_v4()),
            lease_secs: svc::lease_secs_from_env(),
            max_attempts: svc::max_attempts_from_env(),
            poll_interval: svc::poll_interval_from_env(),
            job_timeout: Duration::from_secs(
                std::env::var("DOCPROC_JOB_TIMEOUT_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(svc::DEFAULT_JOB_TIMEOUT_SECS),
            ),
        })
    }

    /// 长循环；panic 隔离在单 job 级（一个坏 job 不拖垮循环）。
    pub async fn run(&self) {
        tracing::info!(worker = %self.worker_id, "docproc worker loop started");
        loop {
            match self.run_once().await {
                Ok(true) => {} // 有任务，继续
                Ok(false) => tokio::time::sleep(self.poll_interval).await,
                Err(e) => {
                    tracing::warn!(error = %e, "docproc worker claim failed; backing off");
                    tokio::time::sleep(self.poll_interval).await;
                }
            }
        }
    }

    /// 领取并执行一个 job；返回是否有任务被领取。
    pub async fn run_once(&self) -> Result<bool, sqlx::Error> {
        let Some(job) = svc::claim_job(&self.pool, &self.worker_id, self.lease_secs).await? else {
            return Ok(false);
        };
        tracing::info!(job = %job.id, doc_version = %job.document_version_id,
                       r#type = %job.processing_type, attempt = job.attempts, "docproc job claimed");
        // 执行内超时（TD-04 §4：主动快速失败，不等租约被动过期）
        let execution = tokio::time::timeout(self.job_timeout, self.execute(&job)).await;
        match execution {
            Ok(Ok(())) => {}
            Ok(Err(DocprocJobError::Db(e))) => {
                // DB 故障：回 pending 由租约过期重领（保留 attempts 计数）
                tracing::warn!(job = %job.id, error = %e, "docproc job db failure");
                svc::fail_job(
                    &self.pool,
                    job.id,
                    "internal_db_error",
                    serde_json::json!({ "message": e.to_string() }),
                    i32::MAX,
                )
                .await?;
            }
            Ok(Err(e)) => {
                let code = job_error_code(&e);
                tracing::warn!(job = %job.id, code, error = %e, "docproc job failed");
                let retried = svc::fail_job(
                    &self.pool,
                    job.id,
                    code,
                    serde_json::json!({ "message": e.to_string() }),
                    self.max_attempts,
                )
                .await?;
                // 终态 failed → 失败事件（重试中不发，避免事件噪声）
                if !retried {
                    self.emit_failed(&job).await?;
                }
            }
            Err(_elapsed) => {
                tracing::warn!(job = %job.id, "docproc job timeout");
                let retried = svc::fail_job(
                    &self.pool,
                    job.id,
                    "parser_timeout",
                    serde_json::json!({ "timeout_secs": self.job_timeout.as_secs() }),
                    self.max_attempts,
                )
                .await?;
                if !retried {
                    self.emit_failed(&job).await?;
                }
            }
        }
        Ok(true)
    }

    async fn execute(&self, job: &svc::ClaimedJob) -> Result<(), DocprocJobError> {
        match job.processing_type.as_str() {
            "parse" => self.execute_parse(job).await,
            "compare" => self.execute_compare(job).await,
            "export" => self.execute_export(job).await,
            "ocr_detect" => self.execute_ocr_detect(job).await,
            other => Err(DocprocJobError::Bad(format!(
                "unknown processing_type {other}"
            ))),
        }
    }

    /// ocr_detect：页级 OCR 状态检测 → ocr_result artifact（规格 D-04 V1
    /// 五项能力；引擎 P1 经 OcrAdapter trait 替换，worker 无感）。
    async fn execute_ocr_detect(&self, job: &svc::ClaimedJob) -> Result<(), DocprocJobError> {
        let tenant_key: String = sqlx::query_scalar(
            "SELECT d.upstream_tenant_key FROM docproc.document d \
             JOIN docproc.document_version v ON v.document_id = d.id WHERE v.id = $1",
        )
        .bind(job.document_version_id)
        .fetch_one(&self.pool)
        .await
        .map_err(DocprocJobError::Db)?;
        let (bytes, _, _) = svc::version_bytes(
            &self.pool,
            self.store.as_ref(),
            job.tenant_id,
            &tenant_key,
            job.document_version_id,
        )
        .await
        .map_err(map_docproc_err)?;

        let detection = {
            let bytes = bytes.clone();
            tokio::task::spawn_blocking(move || {
                pm_application::docproc_ocr::ocr_adapter().detect(&bytes)
            })
            .await
            .map_err(|e| DocprocJobError::Bad(format!("ocr detect task join: {e}")))?
            .map_err(DocprocJobError::Parse)?
        };

        let pending = detection
            .pages
            .iter()
            .filter(|p| p.status == "pending")
            .count();
        let metadata = serde_json::json!({
            "engine": detection.engine,
            "engine_version": detection.engine_version,
            "pages": detection.pages,
            "pending_pages": pending,
            "note": "V1 detection only; OCR engine lands in P1 (D-04)",
        });
        let bytes_out = serde_json::to_vec_pretty(&metadata).expect("ocr metadata serialize");
        let cs = svc::checksum(&bytes_out);
        let key = self
            .store
            .put(&tenant_key, &format!("ocr-{}.json", job.id), bytes_out)
            .await
            .map_err(DocprocJobError::Store)?;
        svc::insert_artifact(
            &self.pool,
            svc::NewArtifact {
                tenant_id: job.tenant_id,
                job_id: job.id,
                artifact_type: "ocr_result",
                storage_key: &key,
                mime_type: "application/json",
                checksum: &cs,
                metadata,
            },
        )
        .await
        .map_err(DocprocJobError::Db)?;
        svc::finish_job(
            &self.pool,
            job.id,
            "completed",
            "ocr-detect",
            env!("CARGO_PKG_VERSION"),
            100,
        )
        .await
        .map_err(DocprocJobError::Db)?;
        self.emit_completed(
            job,
            "completed",
            vec![],
            serde_json::json!({ "pending_pages": pending }),
        )
        .await
        .map_err(DocprocJobError::Db)?;
        Ok(())
    }

    /// parse：取 tenant_key → 原始字节 → adapter → 产物落存。
    async fn execute_parse(&self, job: &svc::ClaimedJob) -> Result<(), DocprocJobError> {
        let tenant_key: String = sqlx::query_scalar(
            "SELECT d.upstream_tenant_key FROM docproc.document d \
             JOIN docproc.document_version v ON v.document_id = d.id WHERE v.id = $1",
        )
        .bind(job.document_version_id)
        .fetch_one(&self.pool)
        .await
        .map_err(DocprocJobError::Db)?;

        let (bytes, ext, filename) = svc::version_bytes(
            &self.pool,
            self.store.as_ref(),
            job.tenant_id,
            &tenant_key,
            job.document_version_id,
        )
        .await
        .map_err(map_docproc_err)?;

        let head = &bytes[..bytes.len().min(16)];
        let adapter = adapter_for(&ext, head)
            .ok_or_else(|| DocprocJobError::Bad(format!("no adapter for .{ext}")))?;

        // CPU 解析出 worker 线程（不阻塞 async runtime）
        let outcome = {
            let bytes = bytes.clone();
            let name = adapter.name();
            let version = adapter.version().to_string();
            tokio::task::spawn_blocking(
                move || -> Result<(ParseOutcome, String, String), ParseError> {
                    let outcome = adapter.parse(ParseCtx { bytes: &bytes })?;
                    Ok((outcome, name.to_string(), version))
                },
            )
            .await
            .map_err(|e| DocprocJobError::Bad(format!("parse task join: {e}")))?
            .map_err(DocprocJobError::Parse)?
        };
        let (outcome, parser_name, parser_version) = outcome;
        let status: &'static str = if outcome.warnings.is_empty() {
            "completed"
        } else {
            "partial_success"
        };

        // markdown 产物
        let md_bytes = outcome.markdown.as_bytes().to_vec();
        let md_cs = svc::checksum(&md_bytes);
        let md_key = self
            .store
            .put(&tenant_key, &format!("{filename}.md"), md_bytes.clone())
            .await
            .map_err(DocprocJobError::Store)?;
        let markdown_artifact_id = svc::insert_artifact(
            &self.pool,
            svc::NewArtifact {
                tenant_id: job.tenant_id,
                job_id: job.id,
                artifact_type: "markdown",
                storage_key: &md_key,
                mime_type: "text/markdown",
                checksum: &md_cs,
                metadata: serde_json::json!({ "filename": filename, "sections": outcome.report.sections }),
            },
        )
        .await
        .map_err(DocprocJobError::Db)?;

        // 图片产物（规格 §4.4：独立保存 + 唯一资源 ID）
        let mut artifact_ids = vec![markdown_artifact_id];
        for img in &outcome.images {
            let cs = svc::checksum(&img.data);
            let key = self
                .store
                .put(
                    &tenant_key,
                    &img.ref_path.replace('/', "_"),
                    img.data.clone(),
                )
                .await
                .map_err(DocprocJobError::Store)?;
            let id = svc::insert_artifact(
                &self.pool,
                svc::NewArtifact {
                    tenant_id: job.tenant_id,
                    job_id: job.id,
                    artifact_type: "image",
                    storage_key: &key,
                    mime_type: image_mime(&img.ext),
                    checksum: &cs,
                    metadata: serde_json::json!({ "ref_path": img.ref_path, "source_location": img.source_location, "alt": img.alt }),
                },
            )
            .await
            .map_err(DocprocJobError::Db)?;
            artifact_ids.push(id);
        }

        // 转换报告（规格 §8.7）
        let report_json = serde_json::json!({
            "sections": outcome.report.sections,
            "tables": outcome.report.tables,
            "images": outcome.report.images,
            "ocr_pages": outcome.report.ocr_pages,
            "warnings": outcome.warnings,
            "failures": outcome.report.failures,
            "ocr_required_pages": outcome.report.ocr_required_pages,
        });
        let report_bytes = serde_json::to_vec_pretty(&report_json).expect("report serialize");
        let report_cs = svc::checksum(&report_bytes);
        let report_key = self
            .store
            .put(
                &tenant_key,
                &format!("{filename}.conversion-report.json"),
                report_bytes,
            )
            .await
            .map_err(DocprocJobError::Store)?;
        let report_id = svc::insert_artifact(
            &self.pool,
            svc::NewArtifact {
                tenant_id: job.tenant_id,
                job_id: job.id,
                artifact_type: "conversion_report",
                storage_key: &report_key,
                mime_type: "application/json",
                checksum: &report_cs,
                metadata: report_json,
            },
        )
        .await
        .map_err(DocprocJobError::Db)?;
        artifact_ids.push(report_id);

        // 终态 + mappings + 事件（单事务：TD-04 §4）
        let mut tx = self.pool.begin().await.map_err(DocprocJobError::Db)?;
        svc::insert_mappings(
            &mut tx,
            job.tenant_id,
            markdown_artifact_id,
            &outcome.mapping,
        )
        .await
        .map_err(DocprocJobError::Db)?;
        let finished = svc::finish_job(
            &self.pool,
            job.id,
            status,
            &parser_name,
            &parser_version,
            100,
        )
        .await
        .map_err(DocprocJobError::Db)?;
        if !finished {
            // 已被取消/重领——不外发事件
            tx.commit().await.map_err(DocprocJobError::Db)?;
            return Ok(());
        }
        let payload = serde_json::json!({
            "job_id": job.id,
            "document_version_id": job.document_version_id,
            "status": status,
            "artifact_ids": artifact_ids,
            "report_summary": {
                "sections": outcome.report.sections,
                "tables": outcome.report.tables,
                "images": outcome.report.images,
                "warnings": outcome.warnings.len(),
            },
        });
        super::outbox::append_team(
            &mut tx,
            super::outbox::NewTeamOutboxEvent {
                tenant_id: job.tenant_id,
                event_type: EVENT_JOB_COMPLETED,
                aggregate_type: AGGREGATE_TYPE,
                aggregate_id: &job.id.to_string(),
                payload,
                idempotency_key: &format!("docproc:job:{}:{status}", job.id),
            },
        )
        .await
        .map_err(DocprocJobError::Db)?;
        tx.commit().await.map_err(DocprocJobError::Db)?;
        tracing::info!(job = %job.id, status, images = outcome.images.len(),
                       mappings = outcome.mapping.len(), "docproc parse finished");
        Ok(())
    }

    /// compare：两个 markdown 产物行级 diff → 报告 artifact（TD-02 §3.6）。
    async fn execute_compare(&self, job: &svc::ClaimedJob) -> Result<(), DocprocJobError> {
        let tenant_key: String = sqlx::query_scalar(
            "SELECT d.upstream_tenant_key FROM docproc.document d \
             JOIN docproc.document_version v ON v.document_id = d.id WHERE v.id = $1",
        )
        .bind(job.document_version_id)
        .fetch_one(&self.pool)
        .await
        .map_err(DocprocJobError::Db)?;
        let (left_id, right_id): (Uuid, Uuid) = parse_pair_config(&job.config)?;
        let (left, _) = svc::artifact_markdown(
            &self.pool,
            self.store.as_ref(),
            job.tenant_id,
            &tenant_key,
            left_id,
        )
        .await
        .map_err(map_docproc_err)?;
        let (right, _) = svc::artifact_markdown(
            &self.pool,
            self.store.as_ref(),
            job.tenant_id,
            &tenant_key,
            right_id,
        )
        .await
        .map_err(map_docproc_err)?;
        let blocks = pm_application::docproc_ops::diff_markdown(&left, &right);
        let report = serde_json::json!({
            "left_artifact_id": left_id,
            "right_artifact_id": right_id,
            "blocks": blocks,
            "summary": {
                "added": blocks.iter().filter(|b| b.kind == "added").count(),
                "removed": blocks.iter().filter(|b| b.kind == "removed").count(),
                "changed": blocks.iter().filter(|b| b.kind == "changed").count(),
            },
        });
        let bytes = serde_json::to_vec_pretty(&report).expect("diff serialize");
        let cs = svc::checksum(&bytes);
        let key = self
            .store
            .put(&tenant_key, &format!("compare-{}.json", job.id), bytes)
            .await
            .map_err(DocprocJobError::Store)?;
        svc::insert_artifact(
            &self.pool,
            svc::NewArtifact {
                tenant_id: job.tenant_id,
                job_id: job.id,
                artifact_type: "conversion_report",
                storage_key: &key,
                mime_type: "application/json",
                checksum: &cs,
                metadata: report.clone(),
            },
        )
        .await
        .map_err(DocprocJobError::Db)?;
        svc::finish_job(
            &self.pool,
            job.id,
            "completed",
            "compare",
            env!("CARGO_PKG_VERSION"),
            100,
        )
        .await
        .map_err(DocprocJobError::Db)?;
        let summary = report["summary"].clone();
        self.emit_completed(job, "completed", vec![], summary)
            .await
            .map_err(DocprocJobError::Db)?;
        Ok(())
    }

    /// export：markdown 产物 → HTML（TD-02 §3.7 V1 范围）。
    async fn execute_export(&self, job: &svc::ClaimedJob) -> Result<(), DocprocJobError> {
        let tenant_key: String = sqlx::query_scalar(
            "SELECT d.upstream_tenant_key FROM docproc.document d \
             JOIN docproc.document_version v ON v.document_id = d.id WHERE v.id = $1",
        )
        .bind(job.document_version_id)
        .fetch_one(&self.pool)
        .await
        .map_err(DocprocJobError::Db)?;
        let source_id: Uuid = job
            .config
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| DocprocJobError::Bad("config.artifact_id missing".into()))?;
        let target = job
            .config
            .get("target_format")
            .and_then(|v| v.as_str())
            .unwrap_or("html");
        if target != "html" {
            return Err(DocprocJobError::Bad(format!(
                "unsupported export target {target} (html only in V1)"
            )));
        }
        let (markdown, _) = svc::artifact_markdown(
            &self.pool,
            self.store.as_ref(),
            job.tenant_id,
            &tenant_key,
            source_id,
        )
        .await
        .map_err(map_docproc_err)?;
        let html =
            pm_application::docproc_ops::render_markdown_html(&markdown, &source_id.to_string());
        let bytes = html.into_bytes();
        let cs = svc::checksum(&bytes);
        let key = self
            .store
            .put(&tenant_key, &format!("export-{}.html", job.id), bytes)
            .await
            .map_err(DocprocJobError::Store)?;
        svc::insert_artifact(
            &self.pool,
            svc::NewArtifact {
                tenant_id: job.tenant_id,
                job_id: job.id,
                artifact_type: "export_file",
                storage_key: &key,
                mime_type: "text/html",
                checksum: &cs,
                metadata: serde_json::json!({ "source_artifact_id": source_id, "target_format": "html" }),
            },
        )
        .await
        .map_err(DocprocJobError::Db)?;
        svc::finish_job(
            &self.pool,
            job.id,
            "completed",
            "export",
            env!("CARGO_PKG_VERSION"),
            100,
        )
        .await
        .map_err(DocprocJobError::Db)?;
        self.emit_completed(
            job,
            "completed",
            vec![],
            serde_json::json!({
                "source_artifact_id": source_id,
                "target_format": "html",
            }),
        )
        .await
        .map_err(DocprocJobError::Db)?;
        Ok(())
    }

    /// 完成事件（TD-02 §4：所有 job 终态 completed/partial_success 均外发）。
    async fn emit_completed(
        &self,
        job: &svc::ClaimedJob,
        status: &str,
        artifact_ids: Vec<Uuid>,
        report_summary: serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        super::outbox::append_team(
            &mut tx,
            super::outbox::NewTeamOutboxEvent {
                tenant_id: job.tenant_id,
                event_type: EVENT_JOB_COMPLETED,
                aggregate_type: AGGREGATE_TYPE,
                aggregate_id: &job.id.to_string(),
                payload: serde_json::json!({
                    "job_id": job.id,
                    "document_version_id": job.document_version_id,
                    "status": status,
                    "artifact_ids": artifact_ids,
                    "report_summary": report_summary,
                }),
                idempotency_key: &format!("docproc:job:{}:{status}", job.id),
            },
        )
        .await?;
        tx.commit().await
    }

    async fn emit_failed(&self, job: &svc::ClaimedJob) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        super::outbox::append_team(
            &mut tx,
            super::outbox::NewTeamOutboxEvent {
                tenant_id: job.tenant_id,
                event_type: EVENT_JOB_FAILED,
                aggregate_type: AGGREGATE_TYPE,
                aggregate_id: &job.id.to_string(),
                payload: serde_json::json!({
                    "job_id": job.id,
                    "document_version_id": job.document_version_id,
                }),
                idempotency_key: &format!("docproc:job:{}:failed", job.id),
            },
        )
        .await?;
        tx.commit().await
    }
}

enum DocprocJobError {
    Db(sqlx::Error),
    Parse(ParseError),
    Store(pm_application::docproc_service::StoreError),
    Bad(String),
}

impl std::fmt::Display for DocprocJobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Db(e) => write!(f, "db: {e}"),
            Self::Parse(e) => write!(f, "parse: {e}"),
            Self::Store(e) => write!(f, "store: {e}"),
            Self::Bad(m) => write!(f, "{m}"),
        }
    }
}

/// 错误分类 → error_code（TD-02 §3.9）。
fn job_error_code(e: &DocprocJobError) -> &'static str {
    match e {
        DocprocJobError::Parse(ParseError::Encrypted(_)) => "encrypted_document",
        DocprocJobError::Parse(ParseError::ArchiveBomb(_)) => "archive_bomb",
        DocprocJobError::Parse(ParseError::MagicMismatch(_)) => "magic_mismatch",
        DocprocJobError::Parse(ParseError::UnsupportedFormat(_)) => "unsupported_format",
        DocprocJobError::Parse(ParseError::Corrupted(_)) => "corrupted_container",
        DocprocJobError::Store(_) => "storage_unavailable",
        DocprocJobError::Db(_) => "internal_db_error",
        DocprocJobError::Bad(_) => "bad_request",
    }
}

fn map_docproc_err(e: svc::DocprocError) -> DocprocJobError {
    match e {
        svc::DocprocError::Store(s) => DocprocJobError::Store(s),
        svc::DocprocError::Db(d) => DocprocJobError::Db(d),
        other => DocprocJobError::Bad(other.to_string()),
    }
}

fn image_mime(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "emf" | "wmf" => "application/octet-stream",
        _ => "application/octet-stream",
    }
}

fn parse_pair_config(config: &serde_json::Value) -> Result<(Uuid, Uuid), DocprocJobError> {
    let get = |k: &str| {
        config
            .get(k)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
    };
    match (get("left_artifact_id"), get("right_artifact_id")) {
        (Some(l), Some(r)) => Ok((l, r)),
        _ => Err(DocprocJobError::Bad(
            "config needs left_artifact_id + right_artifact_id".into(),
        )),
    }
}
