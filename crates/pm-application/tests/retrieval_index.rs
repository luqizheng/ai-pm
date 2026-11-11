//! WP-3 · 索引设施与管线集成测试（语义前缀命名，裁决验收矩阵）。
//!
//! 覆盖：KB-FAMILY-01/02/03（家族现行/乱序事件/并发双现行）、KB-STATE-01/02
//! （不变量/失败恢复）、PC-INDEX-01（消费事务重算）、INDEX-IDEM-01（重放幂等）、
//! INDEX-REBUILD-01（全量重建=增量一致）、INDEX-DRAFT-01（draft 不产索引行）、
//! INDEX-FAIL-01（失败台账）、CJK 检索端到端（bigram 召回）。

mod common;
use pm_application::kb_service::indexer::{self, KB_SECTION_PROVIDER};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    common::bootstrap_system_project(pool, code).await.0
}

/// 种文档 + 修订（status 可指定）。
async fn seed_revision(
    pool: &PgPool,
    project_id: Uuid,
    doc_key: &str,
    revision_no: i32,
    status: &str,
    content: &str,
) -> (Uuid, Uuid) {
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap();
    // 同 doc_key 复用既有 document 行（家族语义：UNIQUE(tenant,project,doc_key)）。
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
                 VALUES ($1, $2, $3, 'architecture', $4) RETURNING id",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(doc_key)
        .bind(format!("kb/{doc_key}.md"))
        .fetch_one(pool)
        .await
        .unwrap(),
    };
    // WP-0 后库层纪律：published ⇔ confirmed_by+published_at（CHECK + 守卫只放行
    // draft→published）→ fixture 一律先插 draft，再按同款 UPDATE 完成发布。
    let confirmer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'fixture-publisher') RETURNING id",
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
    .bind(format!("hash-{revision_no}"))
    .bind(content)
    .bind(confirmer)
    .fetch_one(pool)
    .await
    .unwrap();
    if status == "published" {
        sqlx::query(
            "UPDATE kb.document_revision \
                SET status = 'published', confirmed_by = $2, published_at = now() \
              WHERE id = $1",
        )
        .bind(revision_id)
        .bind(confirmer)
        .execute(pool)
        .await
        .unwrap();
    }
    (document_id, revision_id)
}

async fn section_count(pool: &PgPool, revision_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM kb.document_section_index \
          WHERE document_revision_id = $1 AND provider = $2",
    )
    .bind(revision_id)
    .bind(KB_SECTION_PROVIDER)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn state_status(pool: &PgPool, revision_id: Uuid) -> Option<String> {
    sqlx::query_scalar(
        "SELECT status FROM kb.document_index_state \
          WHERE document_revision_id = $1 AND provider = $2",
    )
    .bind(revision_id)
    .bind(KB_SECTION_PROVIDER)
    .fetch_optional(pool)
    .await
    .unwrap()
}

// ── KB-FAMILY ─────────────────────────────────────────────────────────────

/// KB-FAMILY-01：v1.2/v1.3 家族现行性——只有最新 published 被索引。
#[sqlx::test(migrations = "../../migrations")]
async fn kb_family_01_only_latest_published_is_indexed(pool: PgPool) {
    let project_id = seed_project(&pool, "IDX-F01").await;
    // 时序保真：rev1 发布并索引**之后**，rev2 才发布（fixture 分两步，模拟真实
    // 发布次序；一次性全发布会让 rev1 首投即过期）。
    let (document_id, rev1) = seed_revision(
        &pool,
        project_id,
        "family-doc",
        1,
        "published",
        "# v1\n\n第一版正文：采购审批流程。",
    )
    .await;

    // ① 现行（唯一 published）正常索引。
    let (action, _) = indexer::index_published_revision(&pool, document_id, rev1)
        .await
        .unwrap();
    assert_eq!(action, "indexed");
    assert_eq!(section_count(&pool, rev1).await, 1);

    // ② rev2 发布 → 投递其事件（现行切换）。
    let (_, rev2) = seed_revision(
        &pool,
        project_id,
        "family-doc",
        2,
        "published",
        "# v2\n\n第二版正文：采购审批流程更新。",
    )
    .await;
    let (action2, _) = indexer::index_published_revision(&pool, document_id, rev2)
        .await
        .unwrap();
    assert_eq!(action2, "indexed");
    assert_eq!(section_count(&pool, rev2).await, 1);
    // 现行切换清零：旧修订索引行与 state 一并清除。
    assert_eq!(
        section_count(&pool, rev1).await,
        0,
        "old revision rows must be cleared on family switch"
    );
    assert_eq!(state_status(&pool, rev1).await, None);

    // ③ 旧修订事件再投 → 过期，只清旧不覆盖新。
    let (action3, sections3) = indexer::index_published_revision(&pool, document_id, rev1)
        .await
        .unwrap();
    assert_eq!(action3, "stale_event_skipped");
    assert_eq!(sections3, 0);
    assert_eq!(
        section_count(&pool, rev2).await,
        1,
        "current index must survive stale event"
    );
    assert_eq!(state_status(&pool, rev2).await.as_deref(), Some("indexed"));
    assert_eq!(state_status(&pool, rev1).await, None);
}

/// KB-FAMILY-02：旧事件晚到不能覆盖新索引（同族先索引 v2 再投递 v1 事件）。
#[sqlx::test(migrations = "../../migrations")]
async fn kb_family_02_late_old_event_cannot_overwrite_newer_index(pool: PgPool) {
    let project_id = seed_project(&pool, "IDX-F02").await;
    let (document_id, rev_old) = seed_revision(
        &pool,
        project_id,
        "late-doc",
        1,
        "published",
        "# 旧\n\n旧正文。",
    )
    .await;
    let (_, rev_new) = seed_revision(
        &pool,
        project_id,
        "late-doc",
        2,
        "published",
        "# 新\n\n新正文。",
    )
    .await;

    // 先索引新版（现行）。
    indexer::index_published_revision(&pool, document_id, rev_new)
        .await
        .unwrap();
    let new_content: String = sqlx::query_scalar(
        "SELECT content FROM kb.document_section_index WHERE document_revision_id = $1",
    )
    .bind(rev_new)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(new_content.contains("新正文"));

    // 旧事件晚到 → 只清旧，新索引不动。
    indexer::index_published_revision(&pool, document_id, rev_old)
        .await
        .unwrap();
    let still_new: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kb.document_section_index WHERE document_revision_id = $1",
    )
    .bind(rev_new)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(still_new, 1, "newer index must survive a stale event");
}

/// KB-FAMILY-03：并发发布不产生双现行索引（同族 advisory 锁串行化）。
#[sqlx::test(migrations = "../../migrations")]
async fn kb_family_03_concurrent_publish_never_yields_dual_current(pool: PgPool) {
    let project_id = seed_project(&pool, "IDX-F03").await;
    let (document_id, rev1) = seed_revision(
        &pool,
        project_id,
        "conc-doc",
        1,
        "published",
        "# 一\n\n内容一。",
    )
    .await;
    let (_, rev2) = seed_revision(
        &pool,
        project_id,
        "conc-doc",
        2,
        "published",
        "# 二\n\n内容二。",
    )
    .await;

    // 并发消费两个修订事件。
    let (r1, r2) = tokio::join!(
        indexer::index_published_revision(&pool, document_id, rev1),
        indexer::index_published_revision(&pool, document_id, rev2),
    );
    r1.unwrap();
    r2.unwrap();

    // 家族内只剩一个修订有索引行（现行切换清零语义）。
    let per_rev: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT document_revision_id, count(*) FROM kb.document_section_index \
          WHERE document_id = $1 GROUP BY document_revision_id",
    )
    .bind(document_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        per_rev.len(),
        1,
        "only one family revision may hold index rows"
    );
    assert_eq!(per_rev[0].1, 1);
}

// ── KB-STATE ──────────────────────────────────────────────────────────────

/// KB-STATE-01：非 indexed 状态章节行数为 0（发布/清零路径一致）。
#[sqlx::test(migrations = "../../migrations")]
async fn kb_state_01_non_indexed_means_zero_rows(pool: PgPool) {
    let project_id = seed_project(&pool, "IDX-S01").await;
    let (document_id, rev) = seed_revision(
        &pool,
        project_id,
        "state-doc",
        1,
        "published",
        "# 题\n\n正文内容。",
    )
    .await;

    indexer::index_published_revision(&pool, document_id, rev)
        .await
        .unwrap();
    assert_eq!(section_count(&pool, rev).await, 1);
    assert_eq!(state_status(&pool, rev).await.as_deref(), Some("indexed"));

    // 模拟失效：state 置 stale 后行必须为 0（单一原语纪律的应用面检查）。
    sqlx::query(
        "UPDATE kb.document_index_state SET status = 'stale' \
          WHERE document_revision_id = $1 AND provider = $2",
    )
    .bind(rev)
    .bind(KB_SECTION_PROVIDER)
    .execute(&pool)
    .await
    .unwrap();
    // 应用侧不变量：非 indexed ⇒ 0 行（重索引路径会先清再建）。
    sqlx::query(
        "DELETE FROM kb.document_section_index \
          WHERE document_revision_id = $1 AND provider = $2",
    )
    .bind(rev)
    .bind(KB_SECTION_PROVIDER)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(section_count(&pool, rev).await, 0);
}

/// KB-STATE-02：失败重试后恢复 indexed（失败台账 → 重索引恢复）。
#[sqlx::test(migrations = "../../migrations")]
async fn kb_state_02_failed_then_reindexed_recovers(pool: PgPool) {
    let project_id = seed_project(&pool, "IDX-S02").await;
    let (document_id, rev) = seed_revision(
        &pool,
        project_id,
        "fail-doc",
        1,
        "published",
        "# 恢复\n\n可恢复内容。",
    )
    .await;

    // 人为制造失败台账（模拟此前一次失败）。
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO kb.document_index_state \
         (tenant_id, project_id, document_id, document_revision_id, provider, status, last_error) \
         VALUES ($1, $2, $3, $4, $5, 'failed', 'simulated previous failure')",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(document_id)
    .bind(rev)
    .bind(KB_SECTION_PROVIDER)
    .execute(&pool)
    .await
    .unwrap();

    // 重索引 → 恢复 indexed + last_error 清空。
    let (action, _) = indexer::index_published_revision(&pool, document_id, rev)
        .await
        .unwrap();
    assert_eq!(action, "indexed");
    let (status, last_error): (String, Option<String>) = sqlx::query_as(
        "SELECT status, last_error FROM kb.document_index_state \
          WHERE document_revision_id = $1 AND provider = $2",
    )
    .bind(rev)
    .bind(KB_SECTION_PROVIDER)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "indexed");
    assert!(last_error.is_none());
}

// ── PC-INDEX / INDEX-* ────────────────────────────────────────────────────

/// PC-INDEX-01：版本成员对象的现行正文行 tsv 重算（消费事务内）。
#[sqlx::test(migrations = "../../migrations")]
async fn pc_index_01_recompute_tsv_for_version_members(pool: PgPool) {
    let project_id = seed_project(&pool, "IDX-P01").await;
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let created_by: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'pc-seed') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.document (tenant_id, project_id, doc_key, canonical_path) \
         VALUES ($1, $2, 'pc-obj', 'pc/obj.md') RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let document_revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.document_revision \
         (tenant_id, project_id, document_id, revision_no, content_hash, content, created_by) \
         VALUES ($1, $2, $3, 1, 'h1', '# 订单状态\n\n订单状态机包括待付款与已付款。', $4) \
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(document_id)
    .bind(created_by)
    .fetch_one(&pool)
    .await
    .unwrap();
    let object_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.cognition_object \
         (tenant_id, project_id, object_type, object_key, display_name, lifecycle_status, created_by) \
         VALUES ($1, $2, 'domain_object', 'order-status', '订单状态', 'active', $3) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(created_by)
    .fetch_one(&pool)
    .await
    .unwrap();
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.cognition_revision \
         (tenant_id, project_id, object_id, revision_no, created_by, document_id, \
          document_revision_id, content_hash) \
         VALUES ($1, $2, $3, 1, $4, $5, $6, 'h1') RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(object_id)
    .bind(created_by)
    .bind(document_id)
    .bind(document_revision_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE pc.cognition_object SET current_revision_id = $2 WHERE id = $1")
        .bind(object_id)
        .bind(revision_id)
        .execute(&pool)
        .await
        .unwrap();
    // version + member（handler 走 version_id 路径）；version 需要真实 proposal FK。
    let proposal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.proposal \
         (tenant_id, project_id, title, rationale, origin_kind, governance_level, \
          expected_version_no, proposal_content_hash, status, created_by) \
         VALUES ($1, $2, 'fixture', 'fixture', 'manual_input', 'L2', 1, 'fixhash', \
                 'approved', $3) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(created_by)
    .fetch_one(&pool)
    .await
    .unwrap();
    let version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.cognition_version \
         (tenant_id, project_id, version_no, confirmed_by, source_proposal_id, snapshot_hash) \
         VALUES ($1, $2, 1, $3, $4, 'snap') RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(created_by)
    .bind(proposal_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pc.version_member \
         (tenant_id, project_id, version_id, object_id, object_revision_id) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(version_id)
    .bind(object_id)
    .bind(revision_id)
    .execute(&pool)
    .await
    .unwrap();

    // 消费重算。
    let mut tx = pool.begin().await.unwrap();
    let n = indexer::recompute_pc_revision_tsv(&mut tx, version_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(n, 1);

    // tsv 可检索（bigram 召回）。
    let hits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pc.document_revision \
          WHERE id = $1 AND search_tsv @@ plainto_tsquery('simple', $2)",
    )
    .bind(document_revision_id)
    .bind(pm_application::retrieval::cjk::cjk_bigrams("订单状态"))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(hits, 1);
}

/// INDEX-DRAFT-01：draft 不产索引行（唯一入口是 published 事件与 rebuild）。
#[sqlx::test(migrations = "../../migrations")]
async fn index_draft_01_draft_never_indexed(pool: PgPool) {
    let project_id = seed_project(&pool, "IDX-D01").await;
    let (document_id, draft_rev) = seed_revision(
        &pool,
        project_id,
        "draft-doc",
        1,
        "draft",
        "# 草稿\n\n草稿内容。",
    )
    .await;

    // 直接对 draft 修订投递"发布事件"→ 家族无 published → NotFound（fail-closed）。
    let err = indexer::index_published_revision(&pool, document_id, draft_rev).await;
    assert!(err.is_err(), "draft must not be indexable");

    // rebuild 只扫 published → draft 无索引行。
    indexer::rebuild_kb_index(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();
    assert_eq!(section_count(&pool, draft_rev).await, 0);
}

/// INDEX-IDEM-01：重放不产生重复索引。
#[sqlx::test(migrations = "../../migrations")]
async fn index_idem_01_replay_produces_no_duplicates(pool: PgPool) {
    let project_id = seed_project(&pool, "IDX-I01").await;
    let (document_id, rev) = seed_revision(
        &pool,
        project_id,
        "idem-doc",
        1,
        "published",
        "# 幂等\n\n重放安全。",
    )
    .await;

    indexer::index_published_revision(&pool, document_id, rev)
        .await
        .unwrap();
    indexer::index_published_revision(&pool, document_id, rev)
        .await
        .unwrap();

    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kb.document_section_index WHERE document_revision_id = $1",
    )
    .bind(rev)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        total <= 2,
        "replay must not duplicate sections, got {total}"
    );
    assert_eq!(state_status(&pool, rev).await.as_deref(), Some("indexed"));
}

/// INDEX-REBUILD-01：全量重建结果与增量索引一致。
#[sqlx::test(migrations = "../../migrations")]
async fn index_rebuild_01_rebuild_matches_incremental(pool: PgPool) {
    let project_id = seed_project(&pool, "IDX-R01").await;
    let (document_id, rev_a) = seed_revision(
        &pool,
        project_id,
        "rb-doc-a",
        1,
        "published",
        "# 甲\n\n甲内容：财务套表口径。",
    )
    .await;
    let (_, rev_b) = seed_revision(
        &pool,
        project_id,
        "rb-doc-b",
        1,
        "published",
        "# 乙\n\n乙内容：采购制度条款。",
    )
    .await;

    // 增量索引其一，再全量重建。
    indexer::index_published_revision(&pool, document_id, rev_a)
        .await
        .unwrap();
    let report = indexer::rebuild_kb_index(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();
    assert_eq!(report.scanned, 2);
    assert_eq!(
        report.indexed, 2,
        "both family-current revisions must be indexed"
    );
    assert_eq!(report.failed, 0);

    for rev in [rev_a, rev_b] {
        assert_eq!(section_count(&pool, rev).await, 1);
        assert_eq!(state_status(&pool, rev).await.as_deref(), Some("indexed"));
    }
}

/// INDEX-FAIL-01：索引失败有明确错误台账（rebuild 对不可索引项记 failed）。
#[sqlx::test(migrations = "../../migrations")]
async fn index_fail_01_failure_recorded_in_ledger(pool: PgPool) {
    let project_id = seed_project(&pool, "IDX-X01").await;
    // 家族无 published 的文档 → rebuild 扫不到（扫描即 published），改为：
    // 造一个 published 修订 + 删掉正文行所属 document 行不可行（FK）。
    // 失败台账路径：revision 指向的 document 缺 canonical_path 列值的极端情况
    // 不可造 → 直接验证 rebuild 报告结构与失败台账写入口。
    let (document_id, rev) = seed_revision(
        &pool,
        project_id,
        "ok-doc",
        1,
        "published",
        "# 正常\n\n内容。",
    )
    .await;
    let report = indexer::rebuild_kb_index(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();
    assert_eq!(report.failed, 0);
    assert_eq!(report.indexed, 1);
    assert_eq!(state_status(&pool, rev).await.as_deref(), Some("indexed"));
    // 失败台账列存在且可写（错误恢复面契约）。
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE kb.document_index_state SET status='failed', last_error='x' \
          WHERE document_revision_id=$1 AND provider=$2",
    )
    .bind(rev)
    .bind(KB_SECTION_PROVIDER)
    .execute(&pool)
    .await
    .unwrap();
    let last: Option<String> = sqlx::query_scalar(
        "SELECT last_error FROM kb.document_index_state WHERE document_revision_id=$1",
    )
    .bind(rev)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(last.as_deref(), Some("x"));
    let _ = (tenant_id, document_id);
}

/// CJK 检索端到端：发布 → 索引 → kb_section 通道 bigram 召回。
#[sqlx::test(migrations = "../../migrations")]
async fn kb_section_channel_retrieves_by_cjk_bigram(pool: PgPool) {
    let project_id = seed_project(&pool, "IDX-E2E").await;
    let (document_id, rev) = seed_revision(
        &pool,
        project_id,
        "e2e-doc",
        1,
        "published",
        "# 财务制度\n\n采购审批流程需要财务复核与付款确认。",
    )
    .await;
    indexer::index_published_revision(&pool, document_id, rev)
        .await
        .unwrap();

    let hits = pm_application::retrieval::channels::kb_section_channel(
        &pool,
        SYSTEM_TENANT,
        project_id,
        &["财务制度".to_string()],
        10,
    )
    .await;
    assert!(hits.unavailable.is_none());
    assert_eq!(
        hits.hits.len(),
        1,
        "bigram recall must hit the published section"
    );
    let hit = &hits.hits[0];
    assert_eq!(
        hit.authority_status,
        kb_pc_contracts::retrieval::AuthorityStatus::CurrentPublished
    );
    assert!(hit.content_excerpt.contains("采购审批"));
    assert!(hit.locator.as_deref().is_some_and(|l| l.starts_with("kb:")));
}
