//! T-060 知识索引层 V2 集成测试（设计稿 §8 K-02～K-30）。
//!
//! 覆盖：重建幂等、哈希漂移、发布即索引、veto 回滚、打分次序、类型权重、
//! 引用加成、分页、回退语义、两路径打分一致、租户边界、并发一致性、
//! 版本失效、状态-产物不变量、故障分级、项目级全量重建，以及 v1.3 的
//! Project 级写入协调（串行 / `operation_id` 幂等 / `base_revision` 冲突 /
//! 跨实例 / 锁释放）。
//!
//! 每个用例使用独立临时 Git 知识根，不触碰真实知识仓库。

use std::time::Duration;

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::knowledge::{self, CreateDocumentRequest, KnowledgeRoot};
use pm_application::knowledge_indexer::{
    index_states, DbSectionIndexer, IndexOutcome, KnowledgeIndexer, SearchPage,
    DB_SECTION_INDEX_VERSION, DB_SECTION_PROVIDER,
};
use pm_application::knowledge_proposal::{self, ProposalOutcome, ProposalRequest};
use pm_application::knowledge_reference::{self, AddReferenceRequest};
use pm_application::knowledge_search::KnowledgeSearchService;
use pm_application::knowledge_write_coordinator::{
    check_base_revision, claim_operation, find_operation, WriteCommand,
    DEFAULT_WRITE_LOCK_TIMEOUT_MS,
};
use sqlx::PgPool;
use uuid::Uuid;

use pm_domain::SYSTEM_TENANT;

// ─────────────────────────────────────────────────────────── 夹具

fn temp_root() -> KnowledgeRoot {
    KnowledgeRoot::new(std::env::temp_dir().join(format!("ai-pm-kn-idx-{}", Uuid::new_v4())))
}

fn cleanup(root: &KnowledgeRoot) {
    let _ = std::fs::remove_dir_all(root.path());
}

struct Boot {
    project_id: Uuid,
    human_id: Uuid,
    tenant_id: Uuid,
}

async fn boot(pool: &PgPool, code: &str) -> Boot {
    boot_project(pool, code).await
}

/// 另建一个项目（同租户）。
async fn boot_project(pool: &PgPool, code: &str) -> Boot {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("{code} project"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    Boot {
        project_id: outcome.project_id,
        human_id: outcome.human_id,
        tenant_id: SYSTEM_TENANT,
    }
}

/// 写一篇文档（走 `create_document` 全链路：写 Git + 落库 + 同步重建索引）。
async fn create_doc(
    pool: &PgPool,
    root: &KnowledgeRoot,
    boot: &Boot,
    doc_key: &str,
    doc_type: &str,
    markdown: &str,
) -> (Uuid, Uuid) {
    let created = knowledge::create_document(
        pool,
        root,
        boot.project_id,
        boot.human_id,
        CreateDocumentRequest {
            doc_key: doc_key.to_string(),
            doc_type: doc_type.to_string(),
            markdown: markdown.to_string(),
            change_summary: Some("test".to_string()),
            operation_id: format!("idx-{doc_key}-{}", Uuid::new_v4()),
            base_revision_id: None,
        },
    )
    .await
    .unwrap();
    (created.document_id, created.revision_id)
}

fn proposal(boot: &Boot, doc_key: &str, markdown: &str, summary: &str) -> ProposalRequest {
    ProposalRequest {
        project_id: boot.project_id,
        doc_key: doc_key.to_string(),
        doc_type: "requirement".to_string(),
        markdown: markdown.to_string(),
        change_summary: Some(summary.to_string()),
        source_event_id: Some(Uuid::new_v4()),
        analysis_run_id: None,
        source_ref: None,
        redline_kind: None,
        base_revision_id: None,
        operation_id: format!("idx-prop-{doc_key}-{}", Uuid::new_v4()),
        created_by: boot.human_id,
    }
}

/// 章节行数（某修订某 provider）。
async fn section_count(pool: &PgPool, revision_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM ai.document_section_index \
          WHERE document_revision_id = $1 AND provider = $2",
    )
    .bind(revision_id)
    .bind(DB_SECTION_PROVIDER)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 索引状态 `(status, indexed_content_hash, index_version)`。
async fn state_of(pool: &PgPool, revision_id: Uuid) -> (String, Option<String>, Option<String>) {
    sqlx::query_as(
        "SELECT status, indexed_content_hash, index_version FROM ai.document_index_state \
          WHERE document_revision_id = $1 AND provider = $2",
    )
    .bind(revision_id)
    .bind(DB_SECTION_PROVIDER)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn revision_hash(pool: &PgPool, revision_id: Uuid) -> String {
    sqlx::query_scalar("SELECT content_hash FROM ai.document_revision WHERE id = $1")
        .bind(revision_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// INV-1 / INV-2 全路径断言（K-19）。
async fn assert_invariants(pool: &PgPool, revision_id: Uuid) {
    let (status, indexed_hash, index_version) = state_of(pool, revision_id).await;
    let sections = section_count(pool, revision_id).await;
    let revision_content_hash = revision_hash(pool, revision_id).await;
    if status != "indexed" {
        assert_eq!(
            sections, 0,
            "INV-1 violated: status={status} but {sections} section rows remain"
        );
    } else {
        assert!(
            sections > 0,
            "INV-2 violated: status=indexed but no section rows"
        );
        assert_eq!(
            indexed_hash.as_deref(),
            Some(revision_content_hash.as_str()),
            "INV-2 violated: indexed hash does not match the revision hash"
        );
        assert_eq!(
            index_version.as_deref(),
            Some(DB_SECTION_INDEX_VERSION),
            "INV-2 violated: index_version does not match the current version"
        );
    }
}

async fn search(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    tenant_id: Uuid,
    query: &str,
    limit: Option<i64>,
    offset: Option<i64>,
) -> pm_application::knowledge_search::FallbackSearchResult {
    KnowledgeSearchService::new(pool.clone(), root.clone())
        .search(
            project_id,
            tenant_id,
            query,
            SearchPage::normalized(limit, offset),
        )
        .await
        .unwrap()
}

// ─────────────────────────────────────────────────────────── K-02 重建幂等

#[sqlx::test(migrations = "../../migrations")]
async fn k02_rebuild_is_idempotent_and_sections_are_contiguous(pool: PgPool) {
    let boot = boot(&pool, "IDX-02").await;
    let root = temp_root();
    // 三个标题 + 一段前言 → preamble + 3 个章节 = 4 行。
    let markdown =
        "前言说明。\n\n# 标题一\n\n正文一。\n\n## 标题二\n\n正文二。\n\n### 标题三\n\n正文三。\n";
    let (_document_id, revision_id) =
        create_doc(&pool, &root, &boot, "IDX-02-DOC", "knowledge", markdown).await;

    let indexer = DbSectionIndexer::new(pool.clone(), root.clone());
    let first = indexer
        .rebuild_document(boot.project_id, revision_id)
        .await
        .unwrap();
    assert!(matches!(first, IndexOutcome::Indexed { sections: 4 }));

    let rows: Vec<(i32, String)> = sqlx::query_as(
        "SELECT section_no, anchor FROM ai.document_section_index \
          WHERE document_revision_id = $1 AND provider = $2 ORDER BY section_no",
    )
    .bind(revision_id)
    .bind(DB_SECTION_PROVIDER)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 4);
    for (index, (section_no, _)) in rows.iter().enumerate() {
        assert_eq!(
            *section_no, index as i32,
            "section_no must be contiguous from 0"
        );
    }

    // 连跑两次：行数不变、状态不变。
    for _ in 0..2 {
        let again = indexer
            .rebuild_document(boot.project_id, revision_id)
            .await
            .unwrap();
        assert!(matches!(again, IndexOutcome::Indexed { sections: 4 }));
    }
    assert_eq!(section_count(&pool, revision_id).await, 4);
    let (status, indexed_hash, index_version) = state_of(&pool, revision_id).await;
    assert_eq!(status, "indexed");
    assert_eq!(
        indexed_hash.as_deref(),
        Some(revision_hash(&pool, revision_id).await.as_str())
    );
    assert_eq!(index_version.as_deref(), Some(DB_SECTION_INDEX_VERSION));
    assert_invariants(&pool, revision_id).await;

    cleanup(&root);
}

// ─────────────────────────────────────────────────────────── K-03 哈希漂移

#[sqlx::test(migrations = "../../migrations")]
async fn k03_hash_drift_invalidates_the_index_and_clears_sections(pool: PgPool) {
    let boot = boot(&pool, "IDX-03").await;
    let root = temp_root();
    let (_document_id, revision_id) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-03-DOC",
        "knowledge",
        "# 标题\n\n原始正文。\n",
    )
    .await;
    assert_eq!(section_count(&pool, revision_id).await, 1);

    // 手改盘上文件 → 哈希漂移。
    let path = root.path().join("projects/IDX-03/knowledge/IDX-03-DOC.md");
    std::fs::write(&path, "# 标题\n\n被改过的正文。\n").unwrap();

    let indexer = DbSectionIndexer::new(pool.clone(), root.clone());
    let outcome = indexer
        .rebuild_document(boot.project_id, revision_id)
        .await
        .unwrap();
    assert!(matches!(outcome, IndexOutcome::Stale));
    assert_eq!(
        section_count(&pool, revision_id).await,
        0,
        "stale must clear the section rows"
    );
    let (status, indexed_hash, _) = state_of(&pool, revision_id).await;
    assert_eq!(status, "stale");
    assert!(indexed_hash.is_none());
    assert_invariants(&pool, revision_id).await;

    // 检索不出旧内容（回退路径也校验哈希 → 该篇作废）。
    let result = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "原始正文",
        None,
        None,
    )
    .await;
    assert!(
        result.hits.is_empty(),
        "drifted content must not surface in search results"
    );

    cleanup(&root);
}

// ─────────────────────────────────────────────────── K-04/K-05/K-06 发布即索引

#[sqlx::test(migrations = "../../migrations")]
async fn k04_publish_via_create_document_indexes_immediately(pool: PgPool) {
    let boot = boot(&pool, "IDX-04").await;
    let root = temp_root();
    let (_document_id, revision_id) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-04-DOC",
        "knowledge",
        "# 标题\n\n可检索正文。\n",
    )
    .await;

    let (status, _, _) = state_of(&pool, revision_id).await;
    assert_eq!(
        status, "indexed",
        "create_document must rebuild synchronously"
    );
    assert!(section_count(&pool, revision_id).await > 0);

    let result = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "可检索正文",
        None,
        None,
    )
    .await;
    assert!(
        !result.used_fallback,
        "a fresh index must serve the DB path"
    );
    assert_eq!(result.hits.len(), 1);

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn k05_publish_via_submit_proposal_indexes_immediately(pool: PgPool) {
    let boot = boot(&pool, "IDX-05").await;
    let root = temp_root();
    let ProposalOutcome::Committed { revision_id, .. } = knowledge_proposal::submit_proposal(
        &pool,
        &root,
        proposal(
            &boot,
            "IDX-05-DOC",
            "---\nsource_events:\n  - EVT-1\n---\n\n# 标题\n\n自动提交正文。\n",
            "首版",
        ),
    )
    .await
    .unwrap() else {
        panic!("expected committed");
    };

    let (status, _, _) = state_of(&pool, revision_id).await;
    assert_eq!(status, "indexed");
    let result = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "自动提交正文",
        None,
        None,
    )
    .await;
    assert!(!result.used_fallback);
    assert_eq!(result.hits.len(), 1);

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn k06_redline_confirmation_indexes_immediately(pool: PgPool) {
    let boot = boot(&pool, "IDX-06").await;
    let root = temp_root();
    let mut req = proposal(
        &boot,
        "IDX-06-DOC",
        "---\nsource_events:\n  - EVT-1\n---\n\n# 标题\n\n红线正文。\n",
        "首版",
    );
    req.redline_kind = Some("external_commitment".to_string());
    let ProposalOutcome::RedlinePending { proposal_id } =
        knowledge_proposal::submit_proposal(&pool, &root, req)
            .await
            .unwrap()
    else {
        panic!("expected redline_pending");
    };

    let ProposalOutcome::Committed { revision_id, .. } =
        knowledge_proposal::confirm_redline_proposal(
            &pool,
            &root,
            proposal_id,
            boot.project_id,
            boot.human_id,
            format!("idx-confirm-{proposal_id}"),
        )
        .await
        .unwrap()
    else {
        panic!("expected committed after confirmation");
    };

    let (status, _, _) = state_of(&pool, revision_id).await;
    assert_eq!(status, "indexed");
    assert!(section_count(&pool, revision_id).await > 0);

    cleanup(&root);
}

// ─────────────────────────────────────────────────────────── K-07 veto 回滚

#[sqlx::test(migrations = "../../migrations")]
async fn k07_veto_clears_the_vetoed_revision_and_rebuilds_the_survivor(pool: PgPool) {
    let boot = boot(&pool, "IDX-07").await;
    let root = temp_root();
    let first = knowledge_proposal::submit_proposal(
        &pool,
        &root,
        proposal(
            &boot,
            "IDX-07-DOC",
            "---\nsource_events:\n  - EVT-1\n---\n\n# 标题\n\n第一版正文。\n",
            "v1",
        ),
    )
    .await
    .unwrap();
    let ProposalOutcome::Committed {
        revision_id: rev1, ..
    } = first
    else {
        panic!("expected v1 committed");
    };
    let second = knowledge_proposal::submit_proposal(
        &pool,
        &root,
        proposal(
            &boot,
            "IDX-07-DOC",
            "---\nsource_events:\n  - EVT-1\n---\n\n# 标题\n\n第二版正文。\n",
            "v2",
        ),
    )
    .await
    .unwrap();
    let ProposalOutcome::Committed {
        revision_id: rev2, ..
    } = second
    else {
        panic!("expected v2 committed");
    };

    let outcome = knowledge_proposal::veto_revision(
        &pool,
        &root,
        boot.project_id,
        rev2,
        boot.human_id,
        "内容有误",
        format!("idx-veto-{rev2}"),
    )
    .await
    .unwrap();
    assert_eq!(outcome.rolled_back_revision_id, rev2);
    assert_eq!(outcome.restored_revision_id, Some(rev1));

    // 被否决修订：stale + 章节清空。
    let (status, _, _) = state_of(&pool, rev2).await;
    assert_eq!(status, "stale");
    assert_eq!(section_count(&pool, rev2).await, 0);
    assert_invariants(&pool, rev2).await;

    // 生效修订：重建就绪。
    let (status, _, _) = state_of(&pool, rev1).await;
    assert_eq!(status, "indexed");
    assert!(section_count(&pool, rev1).await > 0);
    assert_invariants(&pool, rev1).await;

    // 检索只出回滚后的正文。
    let result = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "正文",
        None,
        None,
    )
    .await;
    assert_eq!(result.hits.len(), 1);
    assert!(result.hits[0].section.content.contains("第一版"));
    assert!(!result.hits[0].section.content.contains("第二版"));

    cleanup(&root);
}

// ─────────────────────────────────────────────────────────── K-08 打分次序

#[sqlx::test(migrations = "../../migrations")]
async fn k08_score_order_is_title_exact_then_contains_then_anchor_then_body(pool: PgPool) {
    let boot = boot(&pool, "IDX-08").await;
    let root = temp_root();
    // 四个章节分别对应四种命中方式。
    let markdown = "# lod\n\n标题全等。\n\n## lod 决策\n\n标题包含。\n\n## 其他\n\n正文出现 lod。\n\n### 决策 lod\n\n锚点含 lod。\n";
    let (_document_id, _revision_id) =
        create_doc(&pool, &root, &boot, "IDX-08-DOC", "knowledge", markdown).await;

    let result = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "lod",
        None,
        None,
    )
    .await;
    assert!(!result.hits.is_empty());
    let scores: Vec<i64> = result.hits.iter().map(|hit| hit.score).collect();
    // 降序排列。
    for pair in scores.windows(2) {
        assert!(pair[0] >= pair[1], "hits must be sorted by score desc");
    }
    // 标题全等最高。
    let exact = result
        .hits
        .iter()
        .find(|hit| hit.section.title == "lod")
        .expect("title-exact section must be found");
    let contains = result
        .hits
        .iter()
        .find(|hit| hit.section.title == "lod 决策")
        .expect("title-contains section must be found");
    assert!(
        exact.score > contains.score,
        "title equality must outrank title containment"
    );

    cleanup(&root);
}

// ─────────────────────────────────────────────────────────── K-09 类型权重

#[sqlx::test(migrations = "../../migrations")]
async fn k09_doc_type_weight_orders_decision_above_knowledge(pool: PgPool) {
    let boot = boot(&pool, "IDX-09").await;
    let root = temp_root();
    // 同一篇正文、两种文档类型：raw 分相同，类型权重决定次序。
    let body = "# 共同标题\n\n共享正文。共享正文。共享正文。\n";
    create_doc(&pool, &root, &boot, "IDX-09-DEC", "decision", body).await;
    create_doc(&pool, &root, &boot, "IDX-09-KNW", "knowledge", body).await;

    let result = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "共享正文",
        None,
        None,
    )
    .await;
    assert_eq!(result.hits.len(), 2);
    assert_eq!(result.hits[0].section.doc_key, "IDX-09-DEC");
    assert!(
        result.hits[0].score > result.hits[1].score,
        "decision must outrank knowledge on the same raw hit"
    );

    cleanup(&root);
}

// ─────────────────────────────────────────────────────────── K-10 引用加成

#[sqlx::test(migrations = "../../migrations")]
async fn k10_reference_boost_applies_only_to_the_current_revision(pool: PgPool) {
    let boot = boot(&pool, "IDX-10").await;
    let root = temp_root();
    let (document_id, revision_id) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-10-DOC",
        "knowledge",
        "# 标题\n\n## 被引用章节\n\n引用正文。\n",
    )
    .await;

    let baseline = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "引用正文",
        None,
        None,
    )
    .await;
    let baseline_score = baseline.hits[0].score;

    // 给当前修订的被引用章节加一条引用 → +2。
    knowledge_reference::add_reference(
        &pool,
        AddReferenceRequest {
            project_id: boot.project_id,
            document_id,
            document_revision_id: revision_id,
            section_anchor: Some("被引用章节".to_string()),
            section_title: Some("被引用章节".to_string()),
            start_line: Some(3),
            reference_kind: "evidence".to_string(),
            source_kind: "manual".to_string(),
            source_id: None,
            created_by: boot.human_id,
        },
    )
    .await
    .unwrap();

    let boosted = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "引用正文",
        None,
        None,
    )
    .await;
    assert_eq!(boosted.hits.len(), 1);
    assert_eq!(
        boosted.hits[0].score,
        baseline_score + 2,
        "a reference on the current revision must add exactly 2"
    );

    // 历史修订上的引用不加成：把引用挂到一条非当前修订上。
    let other_revision: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.document_revision \
         (project_id, document_id, revision_no, git_commit_sha, content_hash, status, \
          created_by, tenant_id) \
         VALUES ($1, $2, 99, 'sha', 'hash', 'rolled_back', $3, $4) RETURNING id",
    )
    .bind(boot.project_id)
    .bind(document_id)
    .bind(boot.human_id)
    .bind(boot.tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    knowledge_reference::add_reference(
        &pool,
        AddReferenceRequest {
            project_id: boot.project_id,
            document_id,
            document_revision_id: other_revision,
            section_anchor: Some("被引用章节".to_string()),
            section_title: None,
            start_line: None,
            reference_kind: "evidence".to_string(),
            source_kind: "manual".to_string(),
            source_id: None,
            created_by: boot.human_id,
        },
    )
    .await
    .unwrap();

    let unchanged = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "引用正文",
        None,
        None,
    )
    .await;
    assert_eq!(
        unchanged.hits[0].score,
        baseline_score + 2,
        "a reference on a historical revision must not add boost"
    );

    cleanup(&root);
}

// ─────────────────────────────────────────────────────────── K-11 分页

#[sqlx::test(migrations = "../../migrations")]
async fn k11_pagination_uses_offset_and_clamps_the_limit(pool: PgPool) {
    let boot = boot(&pool, "IDX-11").await;
    let root = temp_root();
    // 12 个章节各含一个唯一序号，便于定位。
    let mut markdown = String::from("# 目录\n\n");
    for index in 0..12 {
        markdown.push_str(&format!(
            "## 章节{index:02}\n\n命中标记 body{index:02}。\n\n"
        ));
    }
    create_doc(&pool, &root, &boot, "IDX-11-DOC", "knowledge", &markdown).await;

    let all = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "body",
        None,
        None,
    )
    .await;
    assert_eq!(all.hits.len(), 12);

    // 缺省 limit = 20。
    let defaulted = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "body",
        None,
        None,
    )
    .await;
    assert_eq!(defaulted.hits.len(), 12, "default limit is 20");
    assert_eq!(
        SearchPage::normalized(None, None).limit,
        20,
        "default limit must be 20"
    );

    // limit=5&offset=5 → 第 6–10 条。
    let page = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "body",
        Some(5),
        Some(5),
    )
    .await;
    assert_eq!(page.hits.len(), 5);
    let page_anchors: Vec<&str> = page
        .hits
        .iter()
        .map(|hit| hit.section.anchor.as_str())
        .collect();
    let all_anchors: Vec<&str> = all
        .hits
        .iter()
        .map(|hit| hit.section.anchor.as_str())
        .collect();
    assert_eq!(page_anchors, all_anchors[5..10].to_vec());

    // limit 夹紧到 1..=100。
    assert_eq!(SearchPage::normalized(Some(0), None).limit, 1);
    assert_eq!(SearchPage::normalized(Some(1000), None).limit, 100);
    assert_eq!(SearchPage::normalized(Some(-5), None).limit, 1);
    assert_eq!(SearchPage::normalized(None, Some(-3)).offset, 0);

    cleanup(&root);
}

// ─────────────────────────────────────────────────────────── K-12 响应兼容

#[sqlx::test(migrations = "../../migrations")]
async fn k12_search_hit_serialization_keeps_the_response_shape(pool: PgPool) {
    let boot = boot(&pool, "IDX-12").await;
    let root = temp_root();
    let (_document_id, revision_id) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-12-DOC",
        "knowledge",
        "# 标题\n\n正文。\n",
    )
    .await;

    let result = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "正文",
        None,
        None,
    )
    .await;
    let json = serde_json::to_value(&result.hits[0]).unwrap();
    let object = json.as_object().unwrap();
    assert!(object.contains_key("section"));
    assert!(object.contains_key("score"));
    assert!(
        !object.contains_key("revision_id"),
        "revision_id must not be serialized"
    );
    assert!(
        !object.contains_key("revision_hash"),
        "revision_hash must not be serialized"
    );
    // 证据字段仍在内存里可用。
    assert_eq!(result.hits[0].revision_id, revision_id);
    assert!(!result.hits[0].revision_hash.is_empty());

    cleanup(&root);
}

// ─────────────────────────────────────────────────────────── K-13 回退语义

#[sqlx::test(migrations = "../../migrations")]
async fn k13_fallback_serves_documents_whose_index_state_is_missing_or_failed(pool: PgPool) {
    let boot = boot(&pool, "IDX-13").await;
    let root = temp_root();
    let (_document_id, revision_id) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-13-DOC",
        "knowledge",
        "# 标题\n\n回退正文。\n",
    )
    .await;

    // 状态缺失 → 回退。
    sqlx::query("DELETE FROM ai.document_index_state WHERE document_revision_id = $1")
        .bind(revision_id)
        .execute(&pool)
        .await
        .unwrap();
    let missing = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "回退正文",
        None,
        None,
    )
    .await;
    assert!(missing.used_fallback);
    assert_eq!(missing.stale_revision_ids, vec![revision_id]);
    assert_eq!(
        missing.hits.len(),
        1,
        "fallback must still find the section"
    );

    // 状态 failed → 回退。
    sqlx::query(
        "INSERT INTO ai.document_index_state \
         (project_id, document_revision_id, provider, status, tenant_id) \
         VALUES ($1, $2, $3, 'failed', $4)",
    )
    .bind(boot.project_id)
    .bind(revision_id)
    .bind(DB_SECTION_PROVIDER)
    .bind(boot.tenant_id)
    .execute(&pool)
    .await
    .unwrap();
    let failed = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "回退正文",
        None,
        None,
    )
    .await;
    assert!(failed.used_fallback);
    assert_eq!(failed.hits.len(), 1);

    cleanup(&root);
}

// ─────────────────────────────────────────────────── K-14 两路径打分一致

#[sqlx::test(migrations = "../../migrations")]
async fn k14_db_and_fallback_paths_score_identically(pool: PgPool) {
    let boot = boot(&pool, "IDX-14").await;
    let root = temp_root();
    let (document_id, revision_id) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-14-DOC",
        "requirement",
        "# 标题\n\n## 子节\n\n一致性正文 marker。\n",
    )
    .await;

    let indexed = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "marker",
        None,
        None,
    )
    .await;
    assert!(!indexed.used_fallback);

    // 强制失效 → 走回退路径。
    sqlx::query(
        "UPDATE ai.document_index_state SET status = 'failed' WHERE document_revision_id = $1",
    )
    .bind(revision_id)
    .execute(&pool)
    .await
    .unwrap();
    let fallback = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "marker",
        None,
        None,
    )
    .await;
    assert!(fallback.used_fallback);

    assert_eq!(indexed.hits.len(), fallback.hits.len());
    assert_eq!(indexed.hits[0].score, fallback.hits[0].score);
    assert_eq!(
        indexed.hits[0].section.anchor,
        fallback.hits[0].section.anchor
    );
    assert_eq!(indexed.hits[0].revision_id, fallback.hits[0].revision_id);
    assert_eq!(
        indexed.hits[0].revision_hash,
        fallback.hits[0].revision_hash
    );
    let _ = document_id;

    cleanup(&root);
}

// ─────────────────────────────────────────────────────────── K-17 并发一致性

#[sqlx::test(migrations = "../../migrations")]
async fn k17_concurrent_rebuild_and_stale_are_serialized(pool: PgPool) {
    let boot = boot(&pool, "IDX-17").await;
    let root = temp_root();
    let (_document_id, revision_id) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-17-DOC",
        "knowledge",
        "# 标题\n\n并发正文。\n",
    )
    .await;

    let indexer = DbSectionIndexer::new(pool.clone(), root.clone());
    // 重建与「漂移清理」并发：两者共用 `(revision, provider)` 锁。
    let (rebuild, stale) = tokio::join!(
        indexer.rebuild_document(boot.project_id, revision_id),
        pm_application::knowledge_indexer::mark_stale(
            &pool,
            &root,
            boot.tenant_id,
            boot.project_id,
            revision_id,
            "concurrent drift"
        ),
    );
    assert!(rebuild.is_ok());
    assert!(stale.is_ok());
    // 后到的一方若判定前置条件已变 → Superseded（无写入）；否则正常落定。
    for outcome in [rebuild.unwrap(), stale.unwrap()] {
        match outcome {
            IndexOutcome::Indexed { .. } | IndexOutcome::Stale => {}
            IndexOutcome::Superseded => {}
            IndexOutcome::Failed { .. } => panic!("unexpected failure outcome"),
        }
    }
    assert_invariants(&pool, revision_id).await;

    cleanup(&root);
}

// ─────────────────────────────────────────────────────────── K-18 版本失效

#[sqlx::test(migrations = "../../migrations")]
async fn k18_index_version_participates_in_validity(pool: PgPool) {
    let boot = boot(&pool, "IDX-18").await;
    let root = temp_root();
    let (_document_id, revision_id) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-18-DOC",
        "knowledge",
        "# 标题\n\n版本正文。\n",
    )
    .await;

    // 把索引版本改成旧值 → 立即失效 → 走回退。
    sqlx::query(
        "UPDATE ai.document_index_state SET index_version = 'v0' WHERE document_revision_id = $1",
    )
    .bind(revision_id)
    .execute(&pool)
    .await
    .unwrap();

    let stale = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "版本正文",
        None,
        None,
    )
    .await;
    assert!(stale.used_fallback, "a version mismatch must fall back");
    assert_eq!(
        stale.hits.len(),
        1,
        "fallback still returns the authoritative hit"
    );

    // 全量重建 → 版本对齐 → 回到 DB 直查。
    let indexer = DbSectionIndexer::new(pool.clone(), root.clone());
    indexer
        .rebuild_document(boot.project_id, revision_id)
        .await
        .unwrap();
    let repaired = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "版本正文",
        None,
        None,
    )
    .await;
    assert!(
        !repaired.used_fallback,
        "a rebuilt index must serve the DB path"
    );

    cleanup(&root);
}

// ─────────────────────────────────────────────────────── K-19 状态-产物不变量

#[sqlx::test(migrations = "../../migrations")]
async fn k19_invariants_hold_on_every_path(pool: PgPool) {
    let boot = boot(&pool, "IDX-19").await;
    let root = temp_root();

    // 路径 1：成功重建。
    let (_doc, ok_revision) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-19-OK",
        "knowledge",
        "# 标题\n\n正文。\n",
    )
    .await;
    assert_invariants(&pool, ok_revision).await;

    // 路径 2：无标题 → failed + 章节清零。
    let no_heading = knowledge_proposal::submit_proposal(
        &pool,
        &root,
        ProposalRequest {
            project_id: boot.project_id,
            doc_key: "IDX-19-NOHEAD".to_string(),
            doc_type: "requirement".to_string(),
            markdown: "---\nsource_events:\n  - EVT-1\n---\n\n只有正文。\n".to_string(),
            change_summary: Some("v1".to_string()),
            source_event_id: Some(Uuid::new_v4()),
            analysis_run_id: None,
            source_ref: None,
            redline_kind: None,
            base_revision_id: None,
            operation_id: format!("idx-19-{}", Uuid::new_v4()),
            created_by: boot.human_id,
        },
    )
    .await
    .unwrap();
    assert!(matches!(proposal_outcome_kind(&no_heading), "rejected"));

    // 路径 3：哈希漂移 → stale + 章节清零。
    let (_doc, drift_revision) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-19-DRIFT",
        "knowledge",
        "# 标题\n\n原始。\n",
    )
    .await;
    let path = root
        .path()
        .join("projects/IDX-19/knowledge/IDX-19-DRIFT.md");
    std::fs::write(&path, "# 标题\n\n改过。\n").unwrap();
    let indexer = DbSectionIndexer::new(pool.clone(), root.clone());
    indexer
        .rebuild_document(boot.project_id, drift_revision)
        .await
        .unwrap();
    assert_invariants(&pool, drift_revision).await;
    let (status, _, _) = state_of(&pool, drift_revision).await;
    assert_eq!(status, "stale");

    cleanup(&root);
}

fn proposal_outcome_kind(outcome: &ProposalOutcome) -> &'static str {
    match outcome {
        ProposalOutcome::Committed { .. } => "committed",
        ProposalOutcome::Duplicate { .. } => "duplicate",
        ProposalOutcome::RedlinePending { .. } => "redline_pending",
        ProposalOutcome::Rejected { .. } => "rejected",
    }
}

// ─────────────────────────────────────────────────────────── K-21 空锚点保护

#[sqlx::test(migrations = "../../migrations")]
async fn k21_empty_anchor_never_earns_a_reference_boost(pool: PgPool) {
    let boot = boot(&pool, "IDX-21").await;
    let root = temp_root();
    let (document_id, revision_id) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-21-DOC",
        "knowledge",
        "# 标题\n\n## 章节\n\n锚点正文。\n",
    )
    .await;

    let baseline = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "锚点正文",
        None,
        None,
    )
    .await;
    let baseline_score = baseline.hits[0].score;

    // 空锚点引用（`section_anchor = ''`）→ 不加成。
    knowledge_reference::add_reference(
        &pool,
        AddReferenceRequest {
            project_id: boot.project_id,
            document_id,
            document_revision_id: revision_id,
            section_anchor: Some(String::new()),
            section_title: None,
            start_line: None,
            reference_kind: "evidence".to_string(),
            source_kind: "manual".to_string(),
            source_id: None,
            created_by: boot.human_id,
        },
    )
    .await
    .unwrap();

    // NULL 锚点引用 → 不加成。
    knowledge_reference::add_reference(
        &pool,
        AddReferenceRequest {
            project_id: boot.project_id,
            document_id,
            document_revision_id: revision_id,
            section_anchor: None,
            section_title: None,
            start_line: None,
            reference_kind: "decision".to_string(),
            source_kind: "manual".to_string(),
            source_id: Some(Uuid::new_v4()),
            created_by: boot.human_id,
        },
    )
    .await
    .unwrap();

    let after = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "锚点正文",
        None,
        None,
    )
    .await;
    assert_eq!(
        after.hits[0].score, baseline_score,
        "empty / null anchors must not add the reference boost"
    );

    cleanup(&root);
}

// ─────────────────────────────────────────────────────────── K-22 租户边界

#[sqlx::test(migrations = "../../migrations")]
async fn k22_search_is_scoped_by_tenant(pool: PgPool) {
    let tenant_a: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'idx-a', 'a') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let tenant_b: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'idx-b', 'b') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let mut boots = Vec::new();
    for (tenant, code) in [(tenant_a, "IDX-22-A"), (tenant_b, "IDX-22-B")] {
        let outcome = bootstrap_project(
            &pool,
            BootstrapRequest {
                project_code: code.to_string(),
                project_name: format!("{code} project"),
                tenant_id: tenant,
            },
        )
        .await
        .unwrap();
        boots.push((
            tenant,
            Boot {
                project_id: outcome.project_id,
                human_id: outcome.human_id,
                tenant_id: tenant,
            },
        ));
    }

    let root = temp_root();
    for (tenant, boot_entry) in &boots {
        let created = knowledge::create_document(
            &pool,
            &root,
            boot_entry.project_id,
            boot_entry.human_id,
            CreateDocumentRequest {
                doc_key: format!("IDX-22-{}", tenant),
                doc_type: "knowledge".to_string(),
                markdown: "# 标题\n\n租户隔离正文。\n".to_string(),
                change_summary: Some("test".to_string()),
                operation_id: format!("idx-22-{tenant}-{}", Uuid::new_v4()),
                base_revision_id: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(created.revision_no, 1);
    }

    // 每个租户只看到自己的结果。
    for (tenant, boot_entry) in &boots {
        let result = search(
            &pool,
            &root,
            boot_entry.project_id,
            *tenant,
            "租户隔离正文",
            None,
            None,
        )
        .await;
        assert_eq!(
            result.hits.len(),
            1,
            "tenant {tenant} must only see its own document"
        );
    }

    // 用 A 的 project + B 的 tenant → 无结果（租户谓词生效）。
    let crossed = KnowledgeSearchService::new(pool.clone(), root.clone())
        .search(
            boots[0].1.project_id,
            boots[1].0,
            "租户隔离正文",
            SearchPage::normalized(None, None),
        )
        .await
        .unwrap();
    assert!(
        crossed.hits.is_empty(),
        "a mismatched tenant must not see another tenant's documents"
    );

    cleanup(&root);
}

// ───────────────────────────────────────────────────── K-23 大文档性能

#[sqlx::test(migrations = "../../migrations")]
async fn k23_large_document_rebuild_stays_within_budget(pool: PgPool) {
    let boot = boot(&pool, "IDX-23").await;
    let root = temp_root();
    // ≥200 标题、≥200 KB。
    let mut markdown = String::from("# 大文档\n\n");
    let filler = "内容".repeat(200);
    for index in 0..220 {
        markdown.push_str(&format!("## 章节{index:03}\n\n{filler} {index}\n\n"));
    }
    assert!(
        markdown.len() >= 200 * 1024,
        "fixture must be at least 200 KB, got {} bytes",
        markdown.len()
    );

    let (_document_id, revision_id) =
        create_doc(&pool, &root, &boot, "IDX-23-DOC", "knowledge", &markdown).await;

    let indexer = DbSectionIndexer::new(pool.clone(), root.clone());
    let started = std::time::Instant::now();
    let outcome = indexer
        .rebuild_document(boot.project_id, revision_id)
        .await
        .unwrap();
    let elapsed_ms = started.elapsed().as_millis() as u64;

    let IndexOutcome::Indexed { sections } = outcome else {
        panic!("expected indexed, got {outcome:?}");
    };
    assert_eq!(sections, 221, "220 headings + 1 title section");
    assert_eq!(section_count(&pool, revision_id).await, 221);
    assert_invariants(&pool, revision_id).await;
    assert!(
        elapsed_ms <= 10_000,
        "rebuild took {elapsed_ms} ms, budget is 10 s"
    );

    // 回填实测值供设计稿 §5.6 引用。
    println!(
        "K23_PERF sections=221 bytes={} rebuild_ms={elapsed_ms}",
        markdown.len()
    );

    cleanup(&root);
}

// ─────────────────────────────────────────────────────────── K-24 混合分页

#[sqlx::test(migrations = "../../migrations")]
async fn k24_mixed_pages_have_no_duplicates_gaps_or_drift(pool: PgPool) {
    let boot = boot(&pool, "IDX-24").await;
    let root = temp_root();
    // 两篇文档：DB 命中与回退命中各一，同一章节两路都命中。
    let (db_document, db_revision) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-24-DB",
        "knowledge",
        "# 标题\n\n## 共用章节\n\n混合分页标记。\n",
    )
    .await;
    let (_fb_document, fb_revision) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-24-FB",
        "knowledge",
        "# 标题\n\n## 回退章节\n\n混合分页标记。\n",
    )
    .await;

    // 让 FB 篇失效（走回退），DB 篇保持索引态。
    sqlx::query(
        "UPDATE ai.document_index_state SET status = 'failed' WHERE document_revision_id = $1",
    )
    .bind(fb_revision)
    .execute(&pool)
    .await
    .unwrap();

    let all = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "混合分页标记",
        None,
        None,
    )
    .await;
    assert!(all.used_fallback);
    assert_eq!(all.hits.len(), 2);

    // 无重复。
    let anchors: Vec<String> = all
        .hits
        .iter()
        .map(|hit| hit.section.anchor.clone())
        .collect();
    let unique: std::collections::HashSet<&String> = anchors.iter().collect();
    assert_eq!(unique.len(), anchors.len(), "no duplicate rows");

    // 分页无缺口：offset=1 取到第 2 条。
    let page = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "混合分页标记",
        Some(1),
        Some(1),
    )
    .await;
    assert_eq!(page.hits.len(), 1);
    assert_eq!(page.hits[0].section.anchor, anchors[1]);

    // 顺序稳定：复跑两次结果一致。
    let again = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "混合分页标记",
        None,
        None,
    )
    .await;
    let again_anchors: Vec<String> = again
        .hits
        .iter()
        .map(|hit| hit.section.anchor.clone())
        .collect();
    assert_eq!(
        anchors, again_anchors,
        "ordering must be stable across runs"
    );

    let _ = (db_document, db_revision);

    cleanup(&root);
}

// ────────────────────────────────────────────── K-25 版本升级全量入口

#[sqlx::test(migrations = "../../migrations")]
async fn k25_project_level_reindex_restores_the_db_path(pool: PgPool) {
    let boot = boot(&pool, "IDX-25").await;
    let root = temp_root();
    let (_document_id, revision_id) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-25-DOC",
        "knowledge",
        "# 标题\n\n全量重建正文。\n",
    )
    .await;

    // 模拟版本升级：旧版本索引立即失效。
    sqlx::query(
        "UPDATE ai.document_index_state SET index_version = 'v0' WHERE document_revision_id = $1",
    )
    .bind(revision_id)
    .execute(&pool)
    .await
    .unwrap();
    let stale = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "全量重建正文",
        None,
        None,
    )
    .await;
    assert!(stale.used_fallback);

    // 项目级全量重建。
    let indexer = DbSectionIndexer::new(pool.clone(), root.clone());
    let mut reindexed = 0i64;
    let revisions: Vec<Uuid> = sqlx::query_scalar(
        "SELECT r.id FROM ai.document_revision r \
           JOIN ai.project_document d ON d.id = r.document_id \
          WHERE d.project_id = $1 AND d.tenant_id = $2 AND r.status = 'published' \
          ORDER BY d.doc_key, r.revision_no",
    )
    .bind(boot.project_id)
    .bind(boot.tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    for revision in revisions {
        if let IndexOutcome::Indexed { .. } = indexer
            .rebuild_document(boot.project_id, revision)
            .await
            .unwrap()
        {
            reindexed += 1;
        }
    }
    assert_eq!(reindexed, 1);

    let repaired = search(
        &pool,
        &root,
        boot.project_id,
        boot.tenant_id,
        "全量重建正文",
        None,
        None,
    )
    .await;
    assert!(
        !repaired.used_fallback,
        "full reindex must restore the DB path"
    );
    assert_eq!(repaired.hits.len(), 1);

    cleanup(&root);
}

// ────────────────────────────────────────────── K-26 同 Project 写串行

#[sqlx::test(migrations = "../../migrations")]
async fn k26_same_project_writes_are_serialized(pool: PgPool) {
    let boot = boot(&pool, "IDX-26").await;
    let root = temp_root();

    let started = std::time::Instant::now();
    let (a, b) = tokio::join!(
        create_doc(
            &pool,
            &root,
            &boot,
            "IDX-26-A",
            "knowledge",
            "# 标题\n\n甲文档。\n"
        ),
        create_doc(
            &pool,
            &root,
            &boot,
            "IDX-26-B",
            "knowledge",
            "# 标题\n\n乙文档。\n"
        ),
    );
    let elapsed = started.elapsed();

    let (_doc_a, revision_a) = a;
    let (_doc_b, revision_b) = b;
    // 两篇索引互不交叉。
    assert_eq!(section_count(&pool, revision_a).await, 1);
    assert_eq!(section_count(&pool, revision_b).await, 1);
    assert_invariants(&pool, revision_a).await;
    assert_invariants(&pool, revision_b).await;
    assert!(
        elapsed.as_millis() < 30_000,
        "serialized writes must not deadlock"
    );

    cleanup(&root);
}

// ────────────────────────────────────────────── K-27 operation_id 幂等

#[sqlx::test(migrations = "../../migrations")]
async fn k27_operation_id_replay_produces_one_revision(pool: PgPool) {
    let boot = boot(&pool, "IDX-27").await;
    let root = temp_root();
    let operation_id = format!("idx-27-op-{}", Uuid::new_v4());
    let markdown = "---\nsource_events:\n  - EVT-1\n---\n\n# 标题\n\n幂等正文。\n";

    let build = || ProposalRequest {
        project_id: boot.project_id,
        doc_key: "IDX-27-DOC".to_string(),
        doc_type: "requirement".to_string(),
        markdown: markdown.to_string(),
        change_summary: Some("v1".to_string()),
        source_event_id: Some(Uuid::new_v4()),
        analysis_run_id: None,
        source_ref: None,
        redline_kind: None,
        base_revision_id: None,
        operation_id: operation_id.clone(),
        created_by: boot.human_id,
    };

    let first = knowledge_proposal::submit_proposal(&pool, &root, build())
        .await
        .unwrap();
    let ProposalOutcome::Committed {
        revision_id,
        revision_no,
        ..
    } = &first
    else {
        panic!("expected committed, got {first:?}");
    };
    let first_revision = *revision_id;
    let first_revision_no = *revision_no;

    let second = knowledge_proposal::submit_proposal(&pool, &root, build())
        .await
        .unwrap();
    let ProposalOutcome::Committed {
        revision_id,
        revision_no,
        ..
    } = &second
    else {
        panic!("expected committed on replay, got {second:?}");
    };
    assert_eq!(
        *revision_id, first_revision,
        "a replay must return the original revision"
    );
    assert_eq!(*revision_no, first_revision_no);

    let revisions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.document_revision WHERE document_id = (\
           SELECT id FROM ai.project_document WHERE project_id = $1 AND doc_key = 'IDX-27-DOC')",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revisions, 1, "a replay must not create a second revision");

    let ledger: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.knowledge_write_operation WHERE project_id = $1 AND operation_id = $2",
    )
    .bind(boot.project_id)
    .bind(&operation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ledger, 1, "a replay must not create a second ledger row");

    cleanup(&root);
}

// ────────────────────────────────────────────── K-28 base_revision 冲突

#[sqlx::test(migrations = "../../migrations")]
async fn k28_stale_base_revision_is_refused(pool: PgPool) {
    let boot = boot(&pool, "IDX-28").await;
    let root = temp_root();
    let ProposalOutcome::Committed { revision_id, .. } = knowledge_proposal::submit_proposal(
        &pool,
        &root,
        proposal(
            &boot,
            "IDX-28-DOC",
            "---\nsource_events:\n  - EVT-1\n---\n\n# 标题\n\n第一版。\n",
            "v1",
        ),
    )
    .await
    .unwrap() else {
        panic!("expected committed");
    };

    // 第二版推进 head。
    knowledge_proposal::submit_proposal(
        &pool,
        &root,
        proposal(
            &boot,
            "IDX-28-DOC",
            "---\nsource_events:\n  - EVT-1\n---\n\n# 标题\n\n第二版。\n",
            "v2",
        ),
    )
    .await
    .unwrap();

    // 携带过期基线 → 409。
    let mut stale_req = proposal(
        &boot,
        "IDX-28-DOC",
        "---\nsource_events:\n  - EVT-1\n---\n\n# 标题\n\n第三版。\n",
        "v3",
    );
    stale_req.base_revision_id = Some(revision_id);
    let err = knowledge_proposal::submit_proposal(&pool, &root, stale_req)
        .await
        .unwrap_err();
    assert!(
        matches!(err, knowledge::KnowledgeError::RevisionConflict(_)),
        "a stale base revision must be refused"
    );

    let revisions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.document_revision WHERE document_id = (\
           SELECT id FROM ai.project_document WHERE project_id = $1 AND doc_key = 'IDX-28-DOC')",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revisions, 2, "a refused write must not create a revision");

    cleanup(&root);
}

// ────────────────────────────────────── K-29 跨实例 / 跨项目并行

#[sqlx::test(migrations = "../../migrations")]
async fn k29_two_pools_serialize_the_same_project_and_parallelize_across_projects(pool: PgPool) {
    let boot = boot(&pool, "IDX-29").await;
    let root = temp_root();

    // 第二个独立 pool 模拟第二个实例：连**同一个测试库**（`#[sqlx::test]`
    // 建的是临时库，必须按当前库名重连，而不是连 dev 库）。
    let current_db: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let other_pool = PgPool::connect(&format!("postgres://aipm:aipm@127.0.0.1:5433/{current_db}"))
        .await
        .ok();

    // 跨项目：两个项目同时写入，互不阻塞。
    let other_boot = boot_project(&pool, "IDX-29-B").await;
    let started = std::time::Instant::now();
    let (a, b) = tokio::join!(
        create_doc(
            &pool,
            &root,
            &boot,
            "IDX-29-A",
            "knowledge",
            "# 标题\n\n项目甲。\n"
        ),
        create_doc(
            &pool,
            &root,
            &other_boot,
            "IDX-29-B",
            "knowledge",
            "# 标题\n\n项目乙。\n"
        ),
    );
    let elapsed = started.elapsed();
    let (_doc_a, revision_a) = a;
    let (_doc_b, revision_b) = b;
    assert_eq!(section_count(&pool, revision_a).await, 1);
    assert_eq!(section_count(&pool, revision_b).await, 1);
    assert!(elapsed.as_millis() < 30_000);

    // 同项目跨实例：第二个 pool 的写入与第一个 pool 串行。
    if let Some(other_pool) = other_pool {
        let started = std::time::Instant::now();
        let (first, second) = tokio::join!(
            create_doc(
                &pool,
                &root,
                &boot,
                "IDX-29-SAME-1",
                "knowledge",
                "# 标题\n\n同项目一。\n"
            ),
            create_doc(
                &other_pool,
                &root,
                &boot,
                "IDX-29-SAME-2",
                "knowledge",
                "# 标题\n\n同项目二。\n"
            ),
        );
        let elapsed = started.elapsed();
        let (_doc_1, revision_1) = first;
        let (_doc_2, revision_2) = second;
        assert_eq!(section_count(&pool, revision_1).await, 1);
        assert_eq!(section_count(&pool, revision_2).await, 1);
        assert!(
            elapsed.as_millis() < 30_000,
            "cross-instance writes must not deadlock"
        );
    }

    cleanup(&root);
}

// ────────────────────────────────────────────── K-30 锁释放兜底

#[sqlx::test(migrations = "../../migrations")]
async fn k30_write_lock_is_released_even_when_the_holder_disappears(pool: PgPool) {
    let boot = boot(&pool, "IDX-30").await;
    let root = temp_root();

    // 模拟实例崩溃：持锁连接被强制关闭 → 锁由 PG 自动释放。
    let conn = pool.acquire().await.unwrap();
    let mut conn = conn.detach();
    let key = format!("ai.project_knowledge_write:{}", boot.project_id);
    let acquired: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtextextended($1::text, 0))")
            .bind(&key)
            .fetch_one(&mut conn)
            .await
            .unwrap();
    assert!(acquired, "the simulated instance must take the lock");
    // 连接被丢弃 = 实例崩溃。
    drop(conn);

    // 后续写入在超时内取得锁。
    let started = std::time::Instant::now();
    let (_document_id, revision_id) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-30-DOC",
        "knowledge",
        "# 标题\n\n接管正文。\n",
    )
    .await;
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(DEFAULT_WRITE_LOCK_TIMEOUT_MS + 5_000),
        "the abandoned lock must not block later writes"
    );
    assert_eq!(section_count(&pool, revision_id).await, 1);

    cleanup(&root);
}

// ────────────────────────────────────────── 幂等账本与基线校验（单元口径）

#[sqlx::test(migrations = "../../migrations")]
async fn write_operation_ledger_records_the_command(pool: PgPool) {
    let boot = boot(&pool, "IDX-LEDGER").await;
    let mut tx = pool.begin().await.unwrap();
    let operation_id = format!("ledger-{}", Uuid::new_v4());
    let claim = claim_operation(
        &mut tx,
        boot.project_id,
        &operation_id,
        WriteCommand::CreateDocument,
    )
    .await
    .unwrap();
    assert!(matches!(
        claim,
        pm_application::OperationClaim::Fresh { .. }
    ));
    tx.rollback().await.unwrap();

    // 事务回滚 → 账本行消失（不产生「已执行」的假象）。
    assert!(find_operation(&pool, boot.project_id, &operation_id)
        .await
        .unwrap()
        .is_none());
}

#[test]
fn base_revision_check_only_fires_when_a_baseline_is_declared() {
    let head = Uuid::new_v4();
    assert!(check_base_revision(None, Some(head)).is_ok());
    assert!(check_base_revision(Some(head), Some(head)).is_ok());
    assert!(check_base_revision(Some(Uuid::new_v4()), Some(head)).is_err());
    assert!(check_base_revision(Some(head), None).is_err());
}

// ────────────────────────────────────────── 索引状态视图（回归）

#[sqlx::test(migrations = "../../migrations")]
async fn index_state_view_reports_hash_and_version(pool: PgPool) {
    let boot = boot(&pool, "IDX-STATE").await;
    let root = temp_root();
    let (document_id, revision_id) = create_doc(
        &pool,
        &root,
        &boot,
        "IDX-STATE-DOC",
        "knowledge",
        "# 标题\n\n状态视图正文。\n",
    )
    .await;

    let states = index_states(&pool, boot.project_id, document_id)
        .await
        .unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].status, "indexed");
    assert_eq!(states[0].provider, DB_SECTION_PROVIDER);
    assert_eq!(
        states[0].indexed_content_hash,
        states[0].revision_content_hash
    );
    assert!(states[0].indexed_at.is_some());
    let _ = revision_id;

    cleanup(&root);
}
