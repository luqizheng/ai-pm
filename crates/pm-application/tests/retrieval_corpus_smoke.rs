//! 真实语料冒烟（WP-5 前置验证）：外部 Markdown 语料 → KB 发布（真人确认门）→
//! 索引（kb-section）→ FTS 通道英文召回。
//!
//! **语料门控**：默认读 `AI_PM_EVAL_CORPUS`（或 cadis 下载路径）；目录不存在则
//! 整文件跳过——CI / 无语料环境零影响。语料只读，产物全落在 sqlx 临时库。
//!
//! 验证面（真实内容分布下管线是否符合预期）：
//! - 发布链：draft → 真人 interactive 确认 → published（批量走确认命令）；
//! - 索引链：标题树解析 + 英文分词 + state 五联判定；
//! - 召回面：ADR 编号精确检索（RT-001 语料版）+ 普通关键词语义召回。

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::kb_service::document;
use pm_application::kb_service::indexer;
use pm_application::retrieval::channels;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;

/// 语料目录（环境变量优先；默认 = 用户下载的 cadis 文档）。
fn corpus_dir() -> Option<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("AI_PM_EVAL_CORPUS") {
        let p = std::path::PathBuf::from(dir);
        return p.is_dir().then_some(p);
    }
    let p = std::path::PathBuf::from(r"C:\Users\leo\Downloads\cadis-main\docs");
    p.is_dir().then_some(p)
}

/// 取最大的 N 个 .md（真实分布：优先长文档——检索价值密度高）。
fn pick_markdowns(dir: &std::path::Path, n: usize) -> Vec<std::path::PathBuf> {
    let mut files: Vec<(u64, std::path::PathBuf)> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .filter_map(|e| {
            let size = e.metadata().ok()?.len();
            Some((size, e.path()))
        })
        .collect();
    files.sort_by_key(|(size, _)| std::cmp::Reverse(*size));
    files.into_iter().take(n).map(|(_, p)| p).collect()
}

#[sqlx::test(migrations = "../../migrations")]
async fn real_corpus_ingest_publish_index_query(pool: PgPool) {
    let Some(corpus) = corpus_dir() else {
        eprintln!("[corpus-smoke] corpus dir not found; skipping");
        return;
    };
    let docs = pick_markdowns(&corpus, 12);
    assert!(
        docs.len() >= 5,
        "corpus should contain several markdown docs"
    );

    let project_id = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "CADIS-EVAL".to_string(),
            project_name: "cadis eval".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;

    // 真人（interactive）确认主体——WP-0 门。
    let human_pid: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'corpus-reviewer') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let identity_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal_identity (provider, tenant_id, subject, principal_id) \
         VALUES ('lmms', $1, $2, $3) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(Uuid::new_v4().to_string())
    .bind(human_pid)
    .fetch_one(&pool)
    .await
    .unwrap();
    let human = AuditActor::identified(
        human_pid,
        identity_id,
        Some("web".into()),
        Some("interactive".into()),
    );

    // 项目成员（确认门的成员校验面）。
    sqlx::query(
        "INSERT INTO core.project_member (project_id, principal_id, role, tenant_id) \
         VALUES ($1, $2, 'owner', $3)",
    )
    .bind(project_id)
    .bind(human_pid)
    .bind(SYSTEM_TENANT)
    .execute(&pool)
    .await
    .unwrap();

    // ── 摄入：draft → 确认发布（真链路）→ 索引 ─────────────────────────────
    let mut published = 0usize;
    for path in &docs {
        let doc_key = path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase()
            .replace([' ', '_'], "-");
        let content = std::fs::read_to_string(path).unwrap();
        let tenant_id: Uuid =
            sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
                .bind(project_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        let document_id: Uuid = sqlx::query_scalar(
            "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
             VALUES ($1, $2, $3, 'knowledge', $4) RETURNING id",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(&doc_key)
        .bind(format!("kb/{doc_key}.md"))
        .fetch_one(&pool)
        .await
        .unwrap();
        let revision_id: Uuid = sqlx::query_scalar(
            "INSERT INTO kb.document_revision \
             (tenant_id, document_id, revision_no, content_hash, status, content, created_by) \
             VALUES ($1, $2, 1, 'corpus-hash', 'draft', $3, $4) RETURNING id",
        )
        .bind(tenant_id)
        .bind(document_id)
        .bind(&content)
        .bind(human_pid)
        .fetch_one(&pool)
        .await
        .unwrap();

        // 真人确认门（WP-0 全路径：三重门 + 同事务事件）。
        document::confirm_revision(
            &pool,
            tenant_id,
            project_id,
            document_id,
            revision_id,
            &human,
        )
        .await
        .unwrap();

        // 索引（家族现行六步；管线同款入口）。
        let (action, sections) = indexer::index_published_revision(&pool, document_id, revision_id)
            .await
            .unwrap();
        assert_eq!(action, "indexed");
        assert!(sections > 0, "{doc_key} must yield sections");
        published += 1;
    }
    assert_eq!(published, docs.len());

    // ── 召回面 ──────────────────────────────────────────────────────────────
    // RT-001（语料版）：ADR 编号精确检索。
    let adr_hits = channels::kb_section_channel(
        &pool,
        SYSTEM_TENANT,
        project_id,
        &["ADR-001".to_string()],
        10,
    )
    .await;
    assert!(adr_hits.unavailable.is_none());
    assert!(
        adr_hits
            .hits
            .iter()
            .any(|h| h.content_excerpt.contains("monorepo")
                || h.title
                    .as_deref()
                    .is_some_and(|t| t.to_lowercase().contains("adr"))),
        "ADR-001 query must hit the decision log, got {:?}",
        adr_hits
            .hits
            .iter()
            .map(|h| h.title.clone())
            .collect::<Vec<_>>()
    );

    // 关键词语义召回：安全威胁模型。
    let sec_hits = channels::kb_section_channel(
        &pool,
        SYSTEM_TENANT,
        project_id,
        &["security threat model".to_string()],
        10,
    )
    .await;
    assert!(
        !sec_hits.hits.is_empty(),
        "security threat model query must hit security content"
    );

    // 关键词语义召回：风险登记册（plainto_tsquery 为 AND 语义——查询词都必须
    // 出现；'simple' 无词干还原，故选标题级稳定词对）。
    let plan_hits = channels::kb_section_channel(
        &pool,
        SYSTEM_TENANT,
        project_id,
        &["risk register".to_string()],
        10,
    )
    .await;
    assert!(!plan_hits.hits.is_empty(), "risk register query must hit");

    // 隔离：另一项目不可见（RT-010 语料版）。
    let other_project = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "CADIS-EVAL-OTHER".to_string(),
            project_name: "other".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;
    let empty = channels::kb_section_channel(
        &pool,
        SYSTEM_TENANT,
        other_project,
        &["ADR-001".to_string()],
        10,
    )
    .await;
    assert!(empty.hits.is_empty(), "corpus must be project-scoped");
}

use uuid::Uuid;
