//! WP-5 · 负向隔离矩阵（裁决：安全边界不是平均分问题——任何一项失败不得被
//! 高分抵消；六场景逐项独立验收，全部必须 100%）。
//!
//! 场景（裁决原文映射）：
//! - NEG-01 非成员尝试检索其他项目知识（403/404，不泄漏存在性）；
//! - NEG-02 Agent/API Key 读取 Draft（draft 恒不进检索结果）；
//! - NEG-03 历史版本在未声明历史策略时进入现行结果（include_history=false）；
//! - NEG-04 已 superseded 的 Revision 经索引残留进入当前检索（家族切换清零）；
//! - NEG-05 跨项目关系端点污染（FK + endpoint_in_scope 双防线）；
//! - NEG-06 历史版本错误标注 current_published（通道产物 authority 词表纪律）。

use kb_pc_contracts::retrieval::{
    AuthorityStatus, RetrievalCaller, RetrievalChannel, RetrievalMode, RetrievalPlan,
};
use kb_pc_contracts::RetrievalPort;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::kb_service::indexer;
use pm_application::retrieval::service::RetrievalServiceLive;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

async fn principal(pool: &PgPool, kind: &str, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ($1, $2) RETURNING id",
    )
    .bind(kind)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 项目 + owner 成员 + 可选第二项目。
async fn seed_project(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
    let project_id = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;
    let human_id = principal(pool, "human", "member").await;
    sqlx::query(
        "INSERT INTO core.project_member (project_id, principal_id, role, tenant_id) \
         VALUES ($1, $2, 'owner', $3)",
    )
    .bind(project_id)
    .bind(human_id)
    .bind(SYSTEM_TENANT)
    .execute(pool)
    .await
    .unwrap();
    (project_id, human_id)
}

fn caller(project_id: Uuid, principal_id: Uuid) -> RetrievalCaller {
    RetrievalCaller {
        principal_id,
        tenant_id: SYSTEM_TENANT,
        project_id,
        auth_mode: "interactive".into(),
    }
}

/// KB 文档 + 修订（draft 先插，published 走守卫允许的 UPDATE）。
async fn seed_kb(
    pool: &PgPool,
    project_id: Uuid,
    doc_key: &str,
    revision_no: i32,
    publish: bool,
    content: &str,
) -> (Uuid, Uuid) {
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap();
    let document_id: Uuid = match sqlx::query_scalar(
        "SELECT id FROM kb.document WHERE tenant_id = $1 AND project_id = $2 AND doc_key = $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(doc_key)
    .fetch_optional(pool)
    .await
    .unwrap()
    {
        Some(id) => id,
        None => sqlx::query_scalar(
            "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
             VALUES ($1, $2, $3, 'knowledge', $4) RETURNING id",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(doc_key)
        .bind(format!("kb/{doc_key}.md"))
        .fetch_one(pool)
        .await
        .unwrap(),
    };
    let publisher: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'neg-publisher') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document_revision \
         (tenant_id, document_id, revision_no, content_hash, status, content, created_by) \
         VALUES ($1, $2, $3, $4, 'draft', $5, $6) RETURNING id",
    )
    .bind(tenant_id)
    .bind(document_id)
    .bind(revision_no)
    .bind(format!("neg-hash-{revision_no}"))
    .bind(content)
    .bind(publisher)
    .fetch_one(pool)
    .await
    .unwrap();
    if publish {
        sqlx::query(
            "UPDATE kb.document_revision \
                SET status = 'published', confirmed_by = $2, published_at = now() \
              WHERE id = $1",
        )
        .bind(revision_id)
        .bind(publisher)
        .execute(pool)
        .await
        .unwrap();
    }
    (document_id, revision_id)
}

async fn query_kb(pool: &PgPool, project_id: Uuid, principal_id: Uuid, query: &str) -> Vec<String> {
    let svc = RetrievalServiceLive::new(pool.clone(), None);
    let resp = svc
        .query(query, &caller(project_id, principal_id))
        .await
        .unwrap();
    resp.evidence
        .evidence
        .iter()
        .map(|e| {
            format!(
                "{}|{}",
                e.title.clone().unwrap_or_default(),
                e.content_excerpt
            )
        })
        .collect()
}

/// NEG-01：非成员跨项目检索被拒。
#[sqlx::test(migrations = "../../migrations")]
async fn neg_01_non_member_cross_project_rejected(pool: PgPool) {
    let (project_id, _) = seed_project(&pool, "NEG-01").await;
    let outsider = principal(&pool, "human", "outsider").await;
    let svc = RetrievalServiceLive::new(pool.clone(), None);
    let err = svc
        .query("任意", &caller(project_id, outsider))
        .await
        .expect_err("non-member must be rejected");
    assert!(matches!(
        err,
        pm_application::retrieval::service::RetrievalError::Forbidden(_)
    ));
}

/// NEG-02：Draft 不进检索结果（KB draft 未发布 → 无索引行 → 通道不可达）。
#[sqlx::test(migrations = "../../migrations")]
async fn neg_02_draft_never_in_results(pool: PgPool) {
    let (project_id, human_id) = seed_project(&pool, "NEG-02").await;
    // 同族：v1 published + v2 draft（含独特关键词"机密草案"）。
    seed_kb(
        &pool,
        project_id,
        "neg-doc",
        1,
        true,
        "# 已发布\n\n公开内容包含财务流程。",
    )
    .await;
    seed_kb(
        &pool,
        project_id,
        "neg-doc",
        2,
        false,
        "# 草稿\n\n机密草案内容绝不可见。",
    )
    .await;
    indexer::rebuild_kb_index(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();

    let hits = query_kb(&pool, project_id, human_id, "机密草案").await;
    assert!(
        hits.is_empty(),
        "draft content must never be retrievable, got {hits:?}"
    );
    let hits = query_kb(&pool, project_id, human_id, "公开内容").await;
    assert_eq!(hits.len(), 1, "published content stays retrievable");
}

/// NEG-03：未声明历史策略时，历史版本内容不入现行结果（include_history=false
/// 是所有 V1 通道的既定口径——通道只出家族现行）。
#[sqlx::test(migrations = "../../migrations")]
async fn neg_03_history_not_in_current_results_without_policy(pool: PgPool) {
    let (project_id, human_id) = seed_project(&pool, "NEG-03").await;
    // v1（含独特旧词"旧费率千分之三"）→ v2 发布（现行）。
    seed_kb(
        &pool,
        project_id,
        "rate-spec",
        1,
        true,
        "# 费率 v1\n\n旧费率千分之三。",
    )
    .await;
    seed_kb(
        &pool,
        project_id,
        "rate-spec",
        2,
        true,
        "# 费率 v2\n\n新费率千分之五。",
    )
    .await;
    indexer::rebuild_kb_index(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();

    // OR 语义（WP-5 基线修正）下部分词命中现行文档是正确行为——NEG-03 的核心
    // 不变量是**旧修订内容（千分之三）绝不出现**（keyword 级判定，与 F-11 同口径）。
    let hits = query_kb(&pool, project_id, human_id, "旧费率千分之三").await;
    assert!(
        hits.iter().all(|h| !h.contains("千分之三")),
        "superseded revision content must not surface without explicit history policy, got {hits:?}"
    );
    let hits = query_kb(&pool, project_id, human_id, "新费率千分之五").await;
    assert_eq!(hits.len(), 1, "current content retrievable");
    assert!(hits[0].contains("千分之五"));
}

/// NEG-04：家族切换清零后，旧修订索引行不存在（残留不可能经通道渗入）。
#[sqlx::test(migrations = "../../migrations")]
async fn neg_04_no_stale_index_rows_after_family_switch(pool: PgPool) {
    let (project_id, _) = seed_project(&pool, "NEG-04").await;
    let (document_id, rev1) =
        seed_kb(&pool, project_id, "switch-doc", 1, true, "# 旧\n\n旧正文。").await;
    let (_, rev2) = seed_kb(&pool, project_id, "switch-doc", 2, true, "# 新\n\n新正文。").await;
    indexer::index_published_revision(&pool, document_id, rev1)
        .await
        .unwrap();
    indexer::index_published_revision(&pool, document_id, rev2)
        .await
        .unwrap();

    let old_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kb.document_section_index WHERE document_revision_id = $1",
    )
    .bind(rev1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(old_rows, 0, "family switch must clear old index rows");
}

/// NEG-05：跨项目端点污染——FK 拒绝对象端点；ext 端点经 endpoint_in_scope 过滤。
#[sqlx::test(migrations = "../../migrations")]
async fn neg_05_cross_project_endpoint_pollution_blocked(pool: PgPool) {
    let (project_id, member_id) = seed_project(&pool, "NEG-05").await;
    let (other_project, _) = seed_project(&pool, "NEG-05-OTHER").await;
    let created_by = principal(&pool, "human", "neg-seed").await;

    let module: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.cognition_object \
         (tenant_id, project_id, object_type, object_key, display_name, lifecycle_status, created_by) \
         VALUES ($1, $2, 'feature_module', 'neg-module', '负向模块', 'active', $3) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(created_by)
    .fetch_one(&pool)
    .await
    .unwrap();
    let foreign_doc: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
         VALUES ($1, $2, 'foreign-doc', 'knowledge', 'kb/f.md') RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(other_project)
    .fetch_one(&pool)
    .await
    .unwrap();

    // ext 端点跨项目：插入成功（无物理 FK），但 endpoint_in_scope 必须过滤。
    sqlx::query(
        "INSERT INTO pc.relationship \
         (tenant_id, project_id, relationship_type_key, lifecycle_status, source_object_id, \
          target_ext_domain, target_ext_type, target_ext_id, created_by) \
         VALUES ($1, $2, 'module-covered-by-design', 'active', $3, 'knowledge_base', 'doc', $4, $5)",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(module)
    .bind(foreign_doc)
    .bind(created_by)
    .execute(&pool)
    .await
    .unwrap();

    let svc = RetrievalServiceLive::new(pool.clone(), None);
    let plan = RetrievalPlan {
        intent: kb_pc_contracts::retrieval::RetrievalIntent::ImpactAnalysis,
        mode: RetrievalMode::CrossDomain,
        domains: vec!["PC".into()],
        retrieval_modes: vec![RetrievalChannel::Relationship],
        entities: vec![kb_pc_contracts::retrieval::EntityAnchor {
            kind: "pc_object".into(),
            type_hint: None,
            key_or_name: "neg-module".into(),
        }],
        rewritten_queries: vec!["负向模块".into()],
        traversal: Some(kb_pc_contracts::retrieval::TraversalSpec {
            enabled: true,
            max_hops: 3,
        }),
        analysis: None,
        include_history: false,
        budget: None,
    };
    let validated =
        pm_application::retrieval::validator::validate(&pool, plan, &caller(project_id, member_id))
            .await
            .expect("plan must validate");
    let result = svc.expand(&validated).await.unwrap();
    assert!(
        result.edges.is_empty(),
        "polluted endpoint must be filtered, got {:?}",
        result.edges
    );
}

/// NEG-06：通道产物 authority 词表纪律——现行通道永不产出 historical_published。
#[sqlx::test(migrations = "../../migrations")]
async fn neg_06_no_historical_authority_in_current_channels(pool: PgPool) {
    let (project_id, human_id) = seed_project(&pool, "NEG-06").await;
    seed_kb(
        &pool,
        project_id,
        "auth-doc",
        1,
        true,
        "# 权威\n\n正式内容。",
    )
    .await;
    indexer::rebuild_kb_index(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();

    let svc = RetrievalServiceLive::new(pool.clone(), None);
    let resp = svc
        .query("正式内容", &caller(project_id, human_id))
        .await
        .unwrap();
    for e in &resp.evidence.evidence {
        assert_ne!(
            e.authority_status,
            AuthorityStatus::HistoricalPublished,
            "current channels must never emit historical_published"
        );
        assert_ne!(
            e.authority_status,
            AuthorityStatus::Speculative,
            "speculative must never appear in current results"
        );
    }
}
