//! P3-11 T-1/T-2/T-3 阶段二 · W4/W7 切换矩阵集成测试（真实 DB）。
//!
//! | 用例 | 断言焦点（T-3 验收矩阵映射） |
//! |---|---|
//! | 1 | 技术栈/仓库结构/Repository Module 进 ProjC：`derive_projc_cognition` 后 projc.cognition_object 3 类有行（derived 归因不变）、pc.cognition_object snapshot 类恒 0 行；G-1 双源（pc.source + projc.source）同链登记 |
//! | 2 | 旧 snapshot 路径不能绕过新规则：pc.object_type snapshot 四类 status='deprecated' → pc::upsert_object_draft 拒（fail-closed）；identity 链 product_* 不受影响；projc 字典不受波及 |
//! | 3 | Finding 仅候选：派生只产生 draft + 提案（repo_analysis），无 version 行直至人工 approve |

use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::projc::{
    propose_from_analysis, AnalysisProposalInput, AnalysisProposalOutcome,
};
use pm_application::repo_facts::{CommitFact, HotFile, LanguageStat, RepoFacts};
use pm_domain::SYSTEM_TENANT;
use pm_infrastructure::kb_pc_analysis::derive_projc_cognition;

const LEAD: Uuid = Uuid::from_u128(0x7072_0000_0000_0000_0000_0000_0000_0003);

async fn seed_principal(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'derive-lead', true) ON CONFLICT (id) DO NOTHING",
    )
    .bind(LEAD)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("c-{code}"))
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(code)
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_kb_revision(pool: &PgPool, project: Uuid) -> (Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project)
        .fetch_one(pool)
        .await
        .unwrap();
    let doc: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
         VALUES ($1, $2, $3, 'knowledge', $3) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(format!("repo-fixture-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let rev: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document_revision \
         (tenant_id, document_id, revision_no, content_hash, status, content, created_by) \
         VALUES ($1, $2, 1, 'hash-derive', 'draft', '# snapshot digest', $3) RETURNING id",
    )
    .bind(tenant)
    .bind(doc)
    .bind(LEAD)
    .fetch_one(pool)
    .await
    .unwrap();
    (doc, rev)
}

fn facts() -> RepoFacts {
    RepoFacts {
        head_commit_sha: "abcdef1234567890abcdef1234567890abcdef12".to_string(),
        default_branch: "main".to_string(),
        commits: vec![CommitFact {
            sha: "abcdef1234567890abcdef1234567890abcdef12".to_string(),
            author: "alice".to_string(),
            authored_at: "2026-11-16T10:00:00Z".to_string(),
            subject: "feat: switch".to_string(),
        }],
        hot_files: vec![
            HotFile {
                path: "crates/pm-api/src/routes.rs".to_string(),
                touches: 12,
            },
            HotFile {
                path: "docs/architecture/x.md".to_string(),
                touches: 5,
            },
        ],
        languages: vec![LanguageStat {
            extension: "rs".to_string(),
            files: 120,
        }],
        file_count: 121,
        docs: vec![],
        truncated: false,
        size_bytes: 1024 * 1024,
    }
}

// ── 用例 1：派生域归属（T-3：技术认知进 ProjC，pc snapshot 恒 0 行）──────────

#[sqlx::test(migrations = "../../migrations")]
async fn derive_routes_technical_cognition_to_projc(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "PD-A").await;
    let repository_id = Uuid::new_v4();
    let (kb_doc, kb_rev) = seed_kb_revision(&pool, project).await;
    let lead = AuditActor::principal(LEAD);

    let (projc_source, primary_revision) = derive_projc_cognition(
        &pool,
        project,
        Uuid::new_v4(),
        Uuid::new_v4(),
        repository_id,
        kb_doc,
        kb_rev,
        "hash-derive",
        &facts(),
        None,
        false,
        None,
    )
    .await
    .unwrap();

    // ProjC：3 类技术认知草稿有行（repository / tech_stack / repository_module×2）
    let objects: Vec<(String, String)> = sqlx::query_as(
        "SELECT object_type, lifecycle_status FROM projc.cognition_object \
          WHERE project_id = $1 ORDER BY object_type, object_key",
    )
    .bind(project)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(objects.len(), 4, "snapshot+tech_stack+2 modules");
    assert!(objects
        .iter()
        .all(|(_, s)| s == "identity" || s == "draft"));
    for expected in ["repository", "tech_stack"] {
        assert!(
            objects.iter().any(|(t, _)| t == expected),
            "missing {expected}"
        );
    }
    assert_eq!(
        objects
            .iter()
            .filter(|(t, _)| t == "repository_module")
            .count(),
        2
    );

    // T-3：pc.cognition_object snapshot 类恒 0 行（错误域草稿源头不可达）
    let pc_snapshot_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pc.cognition_object \
          WHERE project_id = $1 \
            AND object_type IN ('repository','tech_stack','delivery_activity','module')",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pc_snapshot_rows, 0);

    // LLM 归因语义保留（D-3=A：无 provider → derived）
    let modes: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT analysis_mode FROM projc.cognition_revision WHERE project_id = $1",
    )
    .bind(project)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(modes, vec![("derived".to_string(),)]);

    // G-1 双源：pc.source（identity 通道）+ projc.source（快照通道）同链登记
    let pc_sources: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.source WHERE project_id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pc_sources, 1);
    let projc_sources: i64 =
        sqlx::query_scalar("SELECT count(*) FROM projc.source WHERE project_id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(projc_sources, 1);
    let (projc_rev_pin,): (Option<String>,) = sqlx::query_as(
        "SELECT target_revision_id FROM projc.source WHERE project_id = $1",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(projc_rev_pin.as_deref(), Some(kb_rev.to_string().as_str()));

    // Finding 仅候选：派生 → 提案（repo_analysis、submitted），无正式版本行
    let outcome = propose_from_analysis(
        &pool,
        project,
        AnalysisProposalInput {
            source_id: projc_source,
            task_id: Uuid::new_v4(),
            title: "仓库认知更新".into(),
            rationale: "derive switch".into(),
        },
    )
    .await
    .unwrap();
    let AnalysisProposalOutcome::Proposed(receipt) = outcome else {
        panic!("candidates expected");
    };
    assert_eq!(receipt.member_count, 4);
    let (origin, status): (String, String) = sqlx::query_as(
        "SELECT origin_kind, status FROM projc.proposal WHERE id = $1",
    )
    .bind(receipt.proposal_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((origin.as_str(), status.as_str()), ("repo_analysis", "submitted"));
    let version_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM projc.cognition_version WHERE project_id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(version_rows, 0, "no formal version without human approve");
    let _ = lead;
    let _ = primary_revision;
}

// ── 用例 2：字典废弃守卫（W7 fail-closed；identity 链不受影响）───────────────

#[sqlx::test(migrations = "../../migrations")]
async fn snapshot_dictionary_deprecated_fails_closed(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "PD-B").await;
    let lead = AuditActor::principal(LEAD);

    // 字典状态四类全 deprecated
    let deprecated: Vec<(String,)> = sqlx::query_as(
        "SELECT type_key FROM pc.object_type \
          WHERE status = 'deprecated' ORDER BY type_key",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let keys: Vec<String> = deprecated.into_iter().map(|(k,)| k).collect();
    assert_eq!(
        keys,
        vec![
            "delivery_activity".to_string(),
            "module".to_string(),
            "repository".to_string(),
            "tech_stack".to_string(),
        ]
    );

    // pc 草稿面拒收（load_object_type 即拒 = 错误域草稿源头不可达）
    for object_type in ["repository", "tech_stack", "delivery_activity", "module"] {
        let rejected = pm_application::pc::upsert_object_draft(
            &pool,
            project,
            &lead,
            pm_application::pc::UpsertObjectDraft {
                object_type: object_type.to_string(),
                object_key: Some(format!("legacy-{object_type}")),
                display_name: "x".into(),
                content_markdown: "x".into(),
                change_summary: None,
                source_context: None,
                analysis_mode: None,
            },
        )
        .await;
        assert!(
            matches!(rejected, Err(pm_application::pc::PcError::Invalid(_))),
            "{object_type} must be rejected by the deprecated dictionary"
        );
    }

    // identity 链 product_* 类型不受影响（可正常起草）
    let identity_ok = pm_application::pc::upsert_object_draft(
        &pool,
        project,
        &lead,
        pm_application::pc::UpsertObjectDraft {
            object_type: "product_profile".into(),
            object_key: Some("product-profile".into()),
            display_name: "产品画像".into(),
            content_markdown: "# 画像".into(),
            change_summary: None,
            source_context: None,
            analysis_mode: None,
        },
    )
    .await;
    assert!(identity_ok.is_ok(), "identity chain untouched: {identity_ok:?}");

    // projc 字典不受 pc 废弃波及（独立字典，R-02）
    let projc_ok = pm_application::projc::upsert_object_draft(
        &pool,
        project,
        &lead,
        pm_application::projc::UpsertObjectDraft {
            object_type: "repository".into(),
            object_key: Some("projc-repo".into()),
            display_name: "仓库".into(),
            content_markdown: "# repo".into(),
            change_summary: None,
            source_context: None,
            analysis_mode: None,
        },
    )
    .await;
    assert!(projc_ok.is_ok(), "projc dictionary independent: {projc_ok:?}");
}
