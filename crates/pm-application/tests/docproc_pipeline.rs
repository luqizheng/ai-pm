//! Document Processing 集成测试（0016 迁移 + 服务层 + worker 流水线）。
//!
//! 真库（sqlx::test）+ MemoryStore（file-service 的测试替身）。覆盖：
//! - 上传去重 / 新版本（D-06）
//! - 入队幂等（uq_job_active_config）与租约领取（SKIP LOCKED）
//! - parse 流水线：docx/pdf/txt → 产物 + 追溯 + 报告 + Team 级事件（U-06）
//! - 失败路径（损坏容器 → failed + attempts 上限）
//! - 取消 / append_team 的 NULL project_id 契约

use pm_application::docproc_parse::ParseCtx;
use pm_application::docproc_service as svc;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

/// 建测试租户（与 SYSTEM_TENANT 无关的独立租户，验租户隔离）。
async fn fixture(pool: &PgPool) -> (Uuid, Uuid) {
    let tenant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.tenant (id, provider, external_key, display_name) VALUES ($1, 'test', $2, 'docproc-t')")
        .bind(tenant_id)
        .bind(tenant_id.to_string())
        .execute(pool)
        .await
        .unwrap();
    let principal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'human', 'docproc-user') RETURNING id",
    )
    .bind(Uuid::new_v4())
    .fetch_one(pool)
    .await
    .unwrap();
    (tenant_id, principal_id)
}

fn store() -> std::sync::Arc<dyn svc::ArtifactStore> {
    std::sync::Arc::new(svc::MemoryStore::new())
}

async fn upload(
    pool: &PgPool,
    store: &std::sync::Arc<dyn svc::ArtifactStore>,
    tenant_id: Uuid,
    principal_id: Uuid,
    filename: &str,
    bytes: &[u8],
) -> svc::DocumentVersion {
    let ext = filename.rsplit('.').next().unwrap_or("bin");
    let mime = match ext {
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    };
    svc::create_document(
        store.as_ref(),
        pool,
        svc::CreateDocumentInput {
            tenant_id,
            project_id: None,
            filename,
            mime_type: mime,
            bytes,
            uploaded_by: principal_id,
            tenant_key: "1",
        },
        100 * 1024 * 1024,
    )
    .await
    .unwrap()
}

/* ── 合成夹具 ──────────────────────────────────────────────────────────── */

/// 最小合法 DOCX（zip 容器）：标题 1 + 正文 + 表格；outlineLvl 驱动标题。
fn build_docx(title: &str, body: &str) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
        let document_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>
<w:p><w:pPr><w:pStyle w:val="H1"/></w:pPr><w:r><w:t>{title}</w:t></w:r></w:p>
<w:p><w:r><w:t>{body}</w:t></w:r></w:p>
<w:tbl><w:tr><w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
</w:body></w:document>"#
        );
        let styles_xml = r#"<?xml version="1.0"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:style w:styleId="H1" w:type="paragraph"><w:outlineLvl w:val="0"/></w:style>
</w:styles>"#;
        zip.start_file("word/document.xml", opts).unwrap();
        std::io::Write::write_all(&mut zip, document_xml.as_bytes()).unwrap();
        zip.start_file("word/styles.xml", opts).unwrap();
        std::io::Write::write_all(&mut zip, styles_xml.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    buf.into_inner()
}

/// 最小合法 PDF：1 页 + 未压缩内容流 + WinAnsi 文本。
fn build_pdf(text: &str) -> Vec<u8> {
    let content = format!("BT /F1 12 Tf 72 720 Td ({text}) Tj ET");
    format!(
        "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
         2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
         3 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 4 0 R /MediaBox [0 0 612 792] >>\nendobj\n\
         4 0 obj\n<< /Length {len} >>\nstream\n{content}\nendstream\nendobj\n\
         trailer\n<< /Root 1 0 R >>\n%%EOF\n",
        len = content.len()
    )
    .into_bytes()
}

/* ── 用例 ──────────────────────────────────────────────────────────────── */

#[sqlx::test(migrations = "../../migrations")]
async fn upload_dedup_and_versions(pool: PgPool) {
    let (tenant_id, principal_id) = fixture(&pool).await;
    let st = store();
    let docx = build_docx("需求总纲", "系统应支持多租户");

    let v1 = upload(&pool, &st, tenant_id, principal_id, "需求.docx", &docx).await;
    assert_eq!(v1.version_number, 1);
    assert!(!v1.duplicate);

    // 同内容重传 → dedup
    let v1b = upload(&pool, &st, tenant_id, principal_id, "需求.docx", &docx).await;
    assert!(v1b.duplicate);
    assert_eq!(v1b.version_id, v1.version_id);

    // 新内容 → 新版本（D-06：版本只增）
    let docx2 = build_docx("需求总纲 V2", "系统应支持多租户与配额");
    let v2 = upload(&pool, &st, tenant_id, principal_id, "需求.docx", &docx2).await;
    assert!(!v2.duplicate);
    assert_eq!(v2.version_number, 2);

    // 白名单外扩展名
    let err = svc::create_document(
        st.as_ref(),
        &pool,
        svc::CreateDocumentInput {
            tenant_id,
            project_id: None,
            filename: "evil.exe",
            mime_type: "application/octet-stream",
            bytes: b"MZfake",
            uploaded_by: principal_id,
            tenant_key: "1",
        },
        100 * 1024 * 1024,
    )
    .await
    .err()
    .expect("must reject exe");
    assert!(matches!(err, svc::DocprocError::UnsupportedFormat(_)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn job_idempotency_claim_and_lease(pool: PgPool) {
    let (tenant_id, principal_id) = fixture(&pool).await;
    let st = store();
    let doc = upload(&pool, &st, tenant_id, principal_id, "a.txt", b"hello world").await;

    let mk_job = || svc::CreateJobInput {
        tenant_id,
        document_id: doc.document_id,
        processing_type: "parse",
        config: serde_json::json!({}),
        created_by: principal_id,
    };
    let j1 = svc::create_job(&pool, mk_job()).await.unwrap();
    assert!(!j1.existed);
    let j2 = svc::create_job(&pool, mk_job()).await.unwrap();
    assert!(j2.existed);
    assert_eq!(j1.job_id, j2.job_id);

    // 领取 → processing（租约期内不可再领）
    let claimed = svc::claim_job(&pool, "w1", 300)
        .await
        .unwrap()
        .expect("claim");
    assert_eq!(claimed.id, j1.job_id);
    assert!(svc::claim_job(&pool, "w2", 300).await.unwrap().is_none());

    // 状态可见
    let view = svc::job_view(&pool, tenant_id, j1.job_id).await.unwrap();
    assert_eq!(view.status, "processing");
    assert_eq!(view.attempts, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn parse_pipeline_docx_end_to_end(pool: PgPool) {
    let (tenant_id, principal_id) = fixture(&pool).await;
    let st = store();
    let docx = build_docx("产品需求", "支持邮件邀请");
    let doc = upload(&pool, &st, tenant_id, principal_id, "PRD.docx", &docx).await;
    let job = svc::create_job(
        &pool,
        svc::CreateJobInput {
            tenant_id,
            document_id: doc.document_id,
            processing_type: "parse",
            config: serde_json::json!({}),
            created_by: principal_id,
        },
    )
    .await
    .unwrap();

    let worker =
        pm_infrastructure::docproc_worker::DocprocWorker::from_env(pool.clone(), Some(st.clone()))
            .expect("worker");
    assert!(worker.run_once().await.unwrap(), "job executed");

    // 终态 + 产物
    let view = svc::job_view(&pool, tenant_id, job.job_id).await.unwrap();
    assert_eq!(view.status, "completed");
    assert_eq!(view.parser_name.as_deref(), Some("docx"));
    let types: Vec<&str> = view
        .artifacts
        .iter()
        .map(|a| a.artifact_type.as_str())
        .collect();
    assert!(types.contains(&"markdown"));
    assert!(types.contains(&"conversion_report"));
    assert_eq!(types.iter().filter(|t| **t == "image").count(), 0);

    // markdown 内容：outlineLvl → H1、正文、表格
    let md_artifact = view
        .artifacts
        .iter()
        .find(|a| a.artifact_type == "markdown")
        .unwrap();
    let (markdown, _) =
        svc::artifact_markdown(&pool, st.as_ref(), tenant_id, "1", md_artifact.artifact_id)
            .await
            .unwrap();
    assert!(
        markdown.contains("# 产品需求"),
        "heading from outlineLvl: {markdown}"
    );
    assert!(markdown.contains("支持邮件邀请"));
    assert!(markdown.contains("| A | B |"), "table rendered: {markdown}");

    // 追溯映射（heading → source paragraph + 表格映射）
    let mappings: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM docproc.source_mapping m \
         JOIN docproc.artifact a ON a.id = m.artifact_id WHERE a.id = $1",
    )
    .bind(md_artifact.artifact_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(mappings >= 2, "heading + table mappings, got {mappings}");

    // 处理报告：sections ≥ 1
    let report = view
        .artifacts
        .iter()
        .find(|a| a.artifact_type == "conversion_report")
        .unwrap()
        .metadata
        .clone();
    assert!(report["sections"].as_u64().unwrap() >= 1);

    // Team 级完成事件：outbox 行 + project_id IS NULL（0016 契约）
    let (event_type, project_id): (String, Option<Uuid>) = sqlx::query_as(
        "SELECT event_type::text, project_id FROM integration.outbox_event \
         WHERE aggregate_type = 'docproc_job' AND aggregate_id = $1",
    )
    .bind(job.job_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_type, "DocprocJobCompleted");
    assert!(
        project_id.is_none(),
        "team-level event must have NULL project_id"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn parse_pipeline_pdf_and_txt(pool: PgPool) {
    let (tenant_id, principal_id) = fixture(&pool).await;
    let st = store();

    // PDF：WinAnsi 直提取
    let pdf = build_pdf("Hello World");
    let doc = upload(&pool, &st, tenant_id, principal_id, "spec.pdf", &pdf).await;
    let job = svc::create_job(
        &pool,
        svc::CreateJobInput {
            tenant_id,
            document_id: doc.document_id,
            processing_type: "parse",
            config: serde_json::json!({}),
            created_by: principal_id,
        },
    )
    .await
    .unwrap();
    let worker =
        pm_infrastructure::docproc_worker::DocprocWorker::from_env(pool.clone(), Some(st.clone()))
            .expect("worker");
    worker.run_once().await.unwrap();
    let view = svc::job_view(&pool, tenant_id, job.job_id).await.unwrap();
    assert_eq!(view.parser_name.as_deref(), Some("pdf"));
    let md_id = view
        .artifacts
        .iter()
        .find(|a| a.artifact_type == "markdown")
        .unwrap()
        .artifact_id;
    let (markdown, _) = svc::artifact_markdown(&pool, st.as_ref(), tenant_id, "1", md_id)
        .await
        .unwrap();
    assert!(markdown.contains("## Page 1"), "page section: {markdown}");
    assert!(markdown.contains("Hello World"));

    // TXT：直接成功（partial_success 因 no_outline 警告）
    let doc2 = upload(
        &pool,
        &st,
        tenant_id,
        principal_id,
        "notes.txt",
        b"line one\nline two",
    )
    .await;
    let job2 = svc::create_job(
        &pool,
        svc::CreateJobInput {
            tenant_id,
            document_id: doc2.document_id,
            processing_type: "parse",
            config: serde_json::json!({}),
            created_by: principal_id,
        },
    )
    .await
    .unwrap();
    worker.run_once().await.unwrap();
    let view2 = svc::job_view(&pool, tenant_id, job2.job_id).await.unwrap();
    assert_eq!(view2.status, "partial_success", "txt warns no_outline");
}

#[sqlx::test(migrations = "../../migrations")]
async fn corrupted_docx_fails_with_bounded_attempts(pool: PgPool) {
    let (tenant_id, principal_id) = fixture(&pool).await;
    let st = store();
    // PK 头（魔数过检）但内容损坏
    let broken = b"PK\x03\x04 not a zip at all".to_vec();
    let doc = upload(&pool, &st, tenant_id, principal_id, "broken.docx", &broken).await;
    let job = svc::create_job(
        &pool,
        svc::CreateJobInput {
            tenant_id,
            document_id: doc.document_id,
            processing_type: "parse",
            config: serde_json::json!({}),
            created_by: principal_id,
        },
    )
    .await
    .unwrap();

    let worker =
        pm_infrastructure::docproc_worker::DocprocWorker::from_env(pool.clone(), Some(st.clone()))
            .expect("worker");
    // 三次执行：前两次回 pending，第三次落 failed（默认 max_attempts=3）
    worker.run_once().await.unwrap();
    let status = svc::job_view(&pool, tenant_id, job.job_id)
        .await
        .unwrap()
        .status;
    assert_eq!(status, "pending", "attempt 1 retries");
    worker.run_once().await.unwrap();
    let status = svc::job_view(&pool, tenant_id, job.job_id)
        .await
        .unwrap()
        .status;
    assert_eq!(status, "pending", "attempt 2 retries");
    worker.run_once().await.unwrap();
    let view = svc::job_view(&pool, tenant_id, job.job_id).await.unwrap();
    assert_eq!(view.status, "failed");
    assert_eq!(view.error_code.as_deref(), Some("corrupted_container"));
    // 原始文件未被破坏（规格 §5.3.6）
    let (bytes, _, _) = svc::version_bytes(&pool, st.as_ref(), tenant_id, "1", doc.version_id)
        .await
        .unwrap();
    assert_eq!(bytes, broken);
}

#[sqlx::test(migrations = "../../migrations")]
async fn cancel_pending_job_and_tenant_isolation(pool: PgPool) {
    let (tenant_id, principal_id) = fixture(&pool).await;
    let other_tenant = Uuid::new_v4();
    let st = store();
    let doc = upload(&pool, &st, tenant_id, principal_id, "a.md", b"# h1\ntext").await;
    let job = svc::create_job(
        &pool,
        svc::CreateJobInput {
            tenant_id,
            document_id: doc.document_id,
            processing_type: "parse",
            config: serde_json::json!({}),
            created_by: principal_id,
        },
    )
    .await
    .unwrap();

    // 取消 pending
    assert!(svc::cancel_job(&pool, tenant_id, job.job_id).await.unwrap());
    let view = svc::job_view(&pool, tenant_id, job.job_id).await.unwrap();
    assert_eq!(view.status, "cancelled");
    // 终态不可取消
    let err = svc::cancel_job(&pool, tenant_id, job.job_id)
        .await
        .err()
        .unwrap();
    assert!(matches!(err, svc::DocprocError::Conflict(_)));

    // 跨租户读 = NotFound（不泄露存在性）
    let err = svc::job_view(&pool, other_tenant, job.job_id)
        .await
        .err()
        .unwrap();
    assert!(matches!(err, svc::DocprocError::NotFound));
}

#[sqlx::test(migrations = "../../migrations")]
async fn compare_and_export_jobs(pool: PgPool) {
    let (tenant_id, principal_id) = fixture(&pool).await;
    let st = store();
    let worker =
        pm_infrastructure::docproc_worker::DocprocWorker::from_env(pool.clone(), Some(st.clone()))
            .expect("worker");

    // 两个 markdown 源（v1 与 v2 同名文档 → 两个 parse job）
    let mk_md = |text: &str| format!("# T\n\n{text}\n").into_bytes();
    let d1 = upload(&pool, &st, tenant_id, principal_id, "c.md", &mk_md("alpha")).await;
    let d2 = upload(&pool, &st, tenant_id, principal_id, "c2.md", &mk_md("beta")).await;
    let job1 = svc::create_job(
        &pool,
        svc::CreateJobInput {
            tenant_id,
            document_id: d1.document_id,
            processing_type: "parse",
            config: serde_json::json!({}),
            created_by: principal_id,
        },
    )
    .await
    .unwrap();
    let job2 = svc::create_job(
        &pool,
        svc::CreateJobInput {
            tenant_id,
            document_id: d2.document_id,
            processing_type: "parse",
            config: serde_json::json!({}),
            created_by: principal_id,
        },
    )
    .await
    .unwrap();
    worker.run_once().await.unwrap();
    worker.run_once().await.unwrap();
    let md1 = svc::job_view(&pool, tenant_id, job1.job_id)
        .await
        .unwrap()
        .artifacts
        .into_iter()
        .find(|a| a.artifact_type == "markdown")
        .unwrap()
        .artifact_id;
    let md2 = svc::job_view(&pool, tenant_id, job2.job_id)
        .await
        .unwrap()
        .artifacts
        .into_iter()
        .find(|a| a.artifact_type == "markdown")
        .unwrap()
        .artifact_id;

    // compare
    let cjob = svc::create_job(
        &pool,
        svc::CreateJobInput {
            tenant_id,
            document_id: d1.document_id,
            processing_type: "compare",
            config: serde_json::json!({ "left_artifact_id": md1, "right_artifact_id": md2 }),
            created_by: principal_id,
        },
    )
    .await
    .unwrap();
    worker.run_once().await.unwrap();
    let cv = svc::job_view(&pool, tenant_id, cjob.job_id).await.unwrap();
    assert_eq!(cv.status, "completed");
    let report = cv
        .artifacts
        .iter()
        .find(|a| a.artifact_type == "conversion_report")
        .unwrap();
    assert_eq!(report.metadata["summary"]["changed"].as_u64().unwrap(), 1);

    // export → html
    let ejob = svc::create_job(
        &pool,
        svc::CreateJobInput {
            tenant_id,
            document_id: d1.document_id,
            processing_type: "export",
            config: serde_json::json!({ "artifact_id": md1, "target_format": "html" }),
            created_by: principal_id,
        },
    )
    .await
    .unwrap();
    worker.run_once().await.unwrap();
    let ev = svc::job_view(&pool, tenant_id, ejob.job_id).await.unwrap();
    assert_eq!(ev.status, "completed");
    let export = ev
        .artifacts
        .iter()
        .find(|a| a.artifact_type == "export_file")
        .unwrap();
    assert_eq!(export.mime_type, "text/html");
}

#[sqlx::test(migrations = "../../migrations")]
async fn append_team_outbox_has_null_project(pool: PgPool) {
    // append_team 直测：Team 级事件落 NULL project_id（0016 契约变更）
    let (tenant_id, _principal_id) = fixture(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let id = pm_infrastructure::outbox::append_team(
        &mut tx,
        pm_infrastructure::outbox::NewTeamOutboxEvent {
            tenant_id,
            event_type: "DocprocJobCompleted",
            aggregate_type: "docproc_job",
            aggregate_id: "test",
            payload: serde_json::json!({ "job_id": "test" }),
            idempotency_key: "docproc:test:null-project",
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM integration.outbox_event WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(project_id.is_none());
}

/// 解析层直测兜底（无 DB）：docx adapter 的 outlineLvl 还原（中文样式名场景）。
#[test]
fn docx_adapter_outline_via_outline_lvl() {
    let docx = build_docx("中文标题一", "正文内容");
    let outcome = pm_application::docproc_parse::adapter_for("docx", b"PK")
        .unwrap()
        .parse(ParseCtx { bytes: &docx })
        .unwrap();
    assert!(outcome.markdown.contains("# 中文标题一"));
    assert!(outcome.markdown.contains("正文内容"));
    assert!(outcome.markdown.contains("| A | B |"));
    assert_eq!(outcome.report.tables, 1);
}
