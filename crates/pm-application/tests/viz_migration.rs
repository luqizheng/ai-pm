//! `viz` schema 迁移与约束验证（V0-2）。
//!
//! `#[sqlx::test]` 会对每个用例**重放全量迁移链**（含 0061）——迁移不可应用时
//! 所有用例直接失败。本文件额外锁定关键不变量（13 号 §3/§4）：
//! - 解释图不可发布（`viz_artifact_nature_publish_check`，DV-13）；
//! - 发布版 spec 不可改写、published 只能向 stale 迁移（不可变守卫触发器）；
//! - 每槽位至多一条 pending 审批（部分唯一索引，并发保障）；
//! - 拒绝必须带理由；审批锚点列不可变（R-04 状态机分离）。

use sqlx::PgPool;

/// 插入一份最小 artifact（绕过应用层，直接验证库级约束）。
async fn insert_artifact(
    pool: &PgPool,
    state: &str,
    nature: &str,
) -> Result<uuid::Uuid, sqlx::Error> {
    // version_no 按 (project, slot) 内递增——槽位版本唯一键是另一条约束，
    // 本文件的用例只针对状态机 / 审批 / 触发器，需绕开它。
    let next: (i32,) = sqlx::query_as(
        "SELECT COALESCE(max(version_no), 0) + 1 FROM viz.artifact \
          WHERE project_id = '00000000-0000-0000-0000-000000000002' \
            AND slot_key = 'req-trace:R-042'",
    )
    .fetch_one(pool)
    .await?;
    let row: (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO viz.artifact \
             (tenant_id, project_id, slot_key, version_no, state, nature, \
              spec, spec_hash, source_hash, provenance, created_by) \
          VALUES ('00000000-0000-0000-0000-000000000001', \
                  '00000000-0000-0000-0000-000000000002', \
                  'req-trace:R-042', $3, $1, $2, \
                  '{\"k\":\"v\"}'::jsonb, 'spec-hash', 'source-hash', \
                  '{\"source_domain\":\"delivery\"}'::jsonb, \
                  '00000000-0000-0000-0000-000000000003') \
          RETURNING id",
    )
    .bind(state)
    .bind(nature)
    .bind(next.0)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

#[sqlx::test(migrations = "../../migrations")]
async fn tables_exist_with_expected_columns(pool: PgPool) {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns \
          WHERE table_schema = 'viz' \
            AND table_name IN ('artifact','approval','render_output','slot_config')",
    )
    .fetch_one(&pool)
    .await
    .expect("query");
    // artifact 22 + approval 14 + render_output 12 + slot_config 13 = 61 列
    assert!(
        count >= 55,
        "viz 四表列数异常：{count}（预期 ≥55，实际按迁移定义）"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn explanatory_artifact_cannot_be_published(pool: PgPool) {
    let id = insert_artifact(&pool, "draft", "explanatory")
        .await
        .expect("插入解释图草稿");
    let result = sqlx::query(
        "UPDATE viz.artifact SET state = 'published', published_by = '00000000-0000-0000-0000-000000000003', published_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&pool)
    .await;
    assert!(
        result.is_err(),
        "解释图不得进入 published（viz_artifact_nature_publish_check）"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn published_spec_is_immutable(pool: PgPool) {
    let id = insert_artifact(&pool, "draft", "business")
        .await
        .expect("插入业务图草稿");
    sqlx::query(
        "UPDATE viz.artifact SET state = 'published', published_by = '00000000-0000-0000-0000-000000000003', published_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .expect("先发布");
    let result =
        sqlx::query("UPDATE viz.artifact SET spec = '{\"hacked\":true}'::jsonb WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await;
    assert!(result.is_err(), "发布版 spec 不可改写（不可变守卫触发器）");
}

#[sqlx::test(migrations = "../../migrations")]
async fn published_can_only_transition_to_stale(pool: PgPool) {
    let id = insert_artifact(&pool, "draft", "business")
        .await
        .expect("插入");
    sqlx::query(
        "UPDATE viz.artifact SET state = 'published', published_by = '00000000-0000-0000-0000-000000000003', published_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .expect("先发布");
    // published → stale 合法
    sqlx::query("UPDATE viz.artifact SET state = 'stale' WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .expect("published → stale 应合法");
    // stale → draft 非法（离开 published/stale）
    let result = sqlx::query("UPDATE viz.artifact SET state = 'draft' WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await;
    assert!(result.is_err(), "published/stale 不得回到 draft");
}

#[sqlx::test(migrations = "../../migrations")]
async fn only_one_pending_approval_per_slot(pool: PgPool) {
    let artifact_id = insert_artifact(&pool, "draft", "business")
        .await
        .expect("插入");
    let insert = "INSERT INTO viz.approval \
              (tenant_id, project_id, artifact_id, slot_key, target_hash, \
               expected_version_no, requested_by) \
          VALUES ('00000000-0000-0000-0000-000000000001', \
                  '00000000-0000-0000-0000-000000000002', \
                  $1, 'req-trace:R-042', 'spec-hash', 1, \
                  '00000000-0000-0000-0000-000000000003')";
    sqlx::query(insert)
        .bind(artifact_id)
        .execute(&pool)
        .await
        .expect("第一条 pending 应成功");
    // 同槽位第二条 pending（即便指向不同 artifact）必须被唯一索引拒绝
    let another = insert_artifact(&pool, "draft", "business")
        .await
        .expect("插入第二个 draft");
    let result = sqlx::query(insert).bind(another).execute(&pool).await;
    assert!(
        result.is_err(),
        "同槽位第二条 pending 必须被部分唯一索引拒绝（R-04）"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn rejection_requires_reason(pool: PgPool) {
    let artifact_id = insert_artifact(&pool, "draft", "business")
        .await
        .expect("插入");
    let approval: (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO viz.approval \
             (tenant_id, project_id, artifact_id, slot_key, target_hash, \
              expected_version_no, requested_by) \
          VALUES ('00000000-0000-0000-0000-000000000001', \
                  '00000000-0000-0000-0000-000000000002', \
                  $1, 'req-trace:R-042', 'spec-hash', 1, \
                  '00000000-0000-0000-0000-000000000003') \
          RETURNING id",
    )
    .bind(artifact_id)
    .fetch_one(&pool)
    .await
    .expect("插入审批");
    let result = sqlx::query(
        "UPDATE viz.approval SET status = 'rejected', decided_by = '00000000-0000-0000-0000-000000000003', decided_at = now() WHERE id = $1",
    )
    .bind(approval.0)
    .execute(&pool)
    .await;
    assert!(
        result.is_err(),
        "拒绝必须带 decision_reason（viz_approval_reject_reason_check）"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn approval_anchor_columns_are_immutable(pool: PgPool) {
    let artifact_id = insert_artifact(&pool, "draft", "business")
        .await
        .expect("插入");
    let approval: (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO viz.approval \
             (tenant_id, project_id, artifact_id, slot_key, target_hash, \
              expected_version_no, requested_by) \
          VALUES ('00000000-0000-0000-0000-000000000001', \
                  '00000000-0000-0000-0000-000000000002', \
                  $1, 'req-trace:R-042', 'spec-hash', 1, \
                  '00000000-0000-0000-0000-000000000003') \
          RETURNING id",
    )
    .bind(artifact_id)
    .fetch_one(&pool)
    .await
    .expect("插入审批");
    let result = sqlx::query("UPDATE viz.approval SET target_hash = 'drifted' WHERE id = $1")
        .bind(approval.0)
        .execute(&pool)
        .await;
    assert!(
        result.is_err(),
        "审批锚点列不可变（裁决守卫触发器，乐观并发前提）"
    );
}
