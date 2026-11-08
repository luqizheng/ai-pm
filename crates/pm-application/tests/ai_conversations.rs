//! Batch G：`ai_conversations::list_messages` 前向 keyset 游标 + 追加
//! metadata 落库（D-4 会话存储的查询面清偿——「>500 条新尾部不可达」）。
//!
//! 覆盖：after(+after_id) 组合键集的边界排序（同 created_at 由 id 决胜）、
//! after-only 退化、before/after 互斥 400、after_id 缺 after 400、
//! ASC+id 双键全序、append metadata 原样落库。
//! Batch H：`latest=true` 最新窗口（>limit 时返回**最新** limit 条、顺序仍
//! ASC）、latest 与 before/after 任一同给 400。

use pm_application::ai_conversations::{
    append_message, create_conversation, list_messages, ConversationError,
};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

/// 系统租户下的最小项目（会话 FK 需要 core.project 行）。
async fn seed_project(pool: &PgPool) -> Uuid {
    let customer_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.customer (id, name, tenant_id) VALUES ($1, 'Acme', $2)")
        .bind(customer_id)
        .bind(SYSTEM_TENANT)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.project (id, customer_id, code, name, tenant_id) \
         VALUES ($1, $2, 'CUR', 'Cursor Project', $3)",
    )
    .bind(project_id)
    .bind(customer_id)
    .bind(SYSTEM_TENANT)
    .execute(pool)
    .await
    .unwrap();
    project_id
}

/// 会话属主 principal。
async fn seed_owner(pool: &PgPool) -> Uuid {
    let owner = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'ai_pm', 'Owner')",
    )
    .bind(owner)
    .execute(pool)
    .await
    .unwrap();
    owner
}

/// 直插一条消息（created_at / id 可强制，测 keyset 边界用）。
async fn seed_message_at(
    pool: &PgPool,
    conversation_id: Uuid,
    id: Uuid,
    created_at: &str,
    content: &str,
) {
    sqlx::query(
        "INSERT INTO ai.conversation_message (id, conversation_id, role, content, created_at) \
         VALUES ($1, $2, 'user', $3, $4::timestamptz)",
    )
    .bind(id)
    .bind(conversation_id)
    .bind(content)
    .bind(created_at)
    .execute(pool)
    .await
    .unwrap();
}

/// m0 → m1 → m2，其中 m1/m2 同一 created_at（同微秒），id 随机——
/// 全序必须由 (created_at, id) 双键决定且稳定，前向游标按同一全序取增量。
#[sqlx::test(migrations = "../../migrations")]
async fn forward_cursor_orders_ties_by_id(pool: PgPool) {
    let project = seed_project(&pool).await;
    let owner = seed_owner(&pool).await;
    let conversation = create_conversation(&pool, SYSTEM_TENANT, project, owner, "cursor")
        .await
        .unwrap()
        .id;

    let m0 = Uuid::new_v4();
    let m1 = Uuid::new_v4();
    let m2 = Uuid::new_v4();
    seed_message_at(&pool, conversation, m0, "2026-11-08T10:00:00Z", "m0").await;
    seed_message_at(&pool, conversation, m1, "2026-11-08T10:01:00Z", "m1").await;
    seed_message_at(&pool, conversation, m2, "2026-11-08T10:01:00Z", "m2").await;

    // 基线：全量 ASC；同微秒对按 id 升序（顺序从返回行读取——id 是随机
    // UUID，测试不对两者谁大做假设，只锁「确定且可复算」这个契约）。
    let views = list_messages(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        None,
        None,
        None,
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(views.len(), 3, "{views:?}");
    assert_eq!(views[0].content, "m0");
    let (first_tie, second_tie) = if views[1].id < views[2].id {
        (&views[1], &views[2])
    } else {
        (&views[2], &views[1])
    };
    assert_eq!(
        first_tie.created_at, second_tie.created_at,
        "m1/m2 must share the same microsecond for this test to be a tie case"
    );

    // 游标 = 同微秒对的第一个：应恰取到 id 更大的另一个兄弟。
    let increment = list_messages(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        None,
        Some(first_tie.created_at),
        Some(first_tie.id),
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        increment
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>(),
        vec![second_tie.content.as_str()],
        "after(first_tie) must yield exactly the id-greater tie sibling"
    );

    // 游标 = 同微秒对的最后一个：无增量。
    let increment = list_messages(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        None,
        Some(second_tie.created_at),
        Some(second_tie.id),
        None,
        false,
    )
    .await
    .unwrap();
    assert!(increment.is_empty());
}

/// after-only（无 after_id）退化为纯时间游标；before 仍向后翻页。
#[sqlx::test(migrations = "../../migrations")]
async fn after_only_degenerates_to_timestamp_cursor(pool: PgPool) {
    let project = seed_project(&pool).await;
    let owner = seed_owner(&pool).await;
    let conversation = create_conversation(&pool, SYSTEM_TENANT, project, owner, "cursor")
        .await
        .unwrap()
        .id;
    seed_message_at(
        &pool,
        conversation,
        Uuid::new_v4(),
        "2026-11-08T10:00:00Z",
        "old",
    )
    .await;
    seed_message_at(
        &pool,
        conversation,
        Uuid::new_v4(),
        "2026-11-08T10:05:00Z",
        "new",
    )
    .await;

    let after = chrono::DateTime::parse_from_rfc3339("2026-11-08T10:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let views = list_messages(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        None,
        Some(after),
        None,
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        views.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(),
        vec!["new"]
    );

    let before = chrono::DateTime::parse_from_rfc3339("2026-11-08T10:05:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let views = list_messages(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        Some(before),
        None,
        None,
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        views.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(),
        vec!["old"],
        "before stays strictly exclusive (created_at < before)"
    );
}

/// before 与 after 同给 → 400（Validation）；after_id 无 after → 400。
#[sqlx::test(migrations = "../../migrations")]
async fn cursor_combination_validation_errors(pool: PgPool) {
    let project = seed_project(&pool).await;
    let owner = seed_owner(&pool).await;
    let conversation = create_conversation(&pool, SYSTEM_TENANT, project, owner, "cursor")
        .await
        .unwrap()
        .id;
    seed_message_at(
        &pool,
        conversation,
        Uuid::new_v4(),
        "2026-11-08T10:00:00Z",
        "only",
    )
    .await;

    let t = chrono::DateTime::parse_from_rfc3339("2026-11-08T10:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    let both = list_messages(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        Some(t),
        Some(t),
        None,
        None,
        false,
    )
    .await;
    assert!(
        matches!(both, Err(ConversationError::Validation(_))),
        "{both:?}"
    );

    let id_only = list_messages(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        None,
        None,
        Some(Uuid::new_v4()),
        None,
        false,
    )
    .await;
    assert!(
        matches!(id_only, Err(ConversationError::Validation(_))),
        "{id_only:?}"
    );
}

/// append_message 的 metadata 参数原样落库（assistant 观测面），REST 的
/// user 消息路径传 `{}`。
#[sqlx::test(migrations = "../../migrations")]
async fn append_message_persists_metadata(pool: PgPool) {
    let project = seed_project(&pool).await;
    let owner = seed_owner(&pool).await;
    let conversation = create_conversation(&pool, SYSTEM_TENANT, project, owner, "meta")
        .await
        .unwrap()
        .id;

    let empty = append_message(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        "user",
        "q",
        "markdown",
        serde_json::json!({}),
    )
    .await
    .unwrap();
    assert_eq!(empty.metadata, serde_json::json!({}));

    let observed = append_message(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        "assistant",
        "a",
        "markdown",
        serde_json::json!({"model": "test-model", "latency_ms": 42}),
    )
    .await
    .unwrap();
    assert_eq!(
        observed.metadata,
        serde_json::json!({"model": "test-model", "latency_ms": 42})
    );
}

/// Batch H：`latest=true` 最新窗口——消息数 > limit 时返回**最新**的
/// limit 条（不是最旧的 limit 条），且返回顺序仍是 `(created_at, id)` ASC；
/// limit 缺省回落 [`DEFAULT_MESSAGE_LIMIT`]（这里显式传小值控制窗口）。
#[sqlx::test(migrations = "../../migrations")]
async fn latest_window_returns_the_newest_tail_in_ascending_order(pool: PgPool) {
    let project = seed_project(&pool).await;
    let owner = seed_owner(&pool).await;
    let conversation = create_conversation(&pool, SYSTEM_TENANT, project, owner, "latest")
        .await
        .unwrap()
        .id;
    // 6 条消息，limit=2：默认分支取最旧 2 条（m0,m1），latest 取最新 2 条（m4,m5）。
    for (i, at) in [
        ("m0", "2026-11-08T10:00:00Z"),
        ("m1", "2026-11-08T10:01:00Z"),
        ("m2", "2026-11-08T10:02:00Z"),
        ("m3", "2026-11-08T10:03:00Z"),
        ("m4", "2026-11-08T10:04:00Z"),
        ("m5", "2026-11-08T10:05:00Z"),
    ] {
        seed_message_at(&pool, conversation, Uuid::new_v4(), at, i).await;
    }

    let default_window = list_messages(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        None,
        None,
        None,
        Some(2),
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        default_window
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>(),
        vec!["m0", "m1"],
        "default (latest=false) keeps the oldest-window behavior"
    );

    let latest_window = list_messages(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        None,
        None,
        None,
        Some(2),
        true,
    )
    .await
    .unwrap();
    assert_eq!(
        latest_window
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>(),
        vec!["m4", "m5"],
        "latest=true must return the NEWEST limit rows"
    );
    // 顺序仍 ASC：时间不减，末条是全表最新一条。
    let times: Vec<_> = latest_window.iter().map(|m| m.created_at).collect();
    assert!(
        times.windows(2).all(|w| w[0] <= w[1]),
        "latest window must be returned in ascending order"
    );
    assert_eq!(latest_window.last().unwrap().content, "m5");
}

/// Batch H：latest 消息数 ≤ limit 时即全量（顺序 ASC，与默认分支一致）；
/// latest 与 before / after 任一同给一律 400（从简裁决：组合语义不做）。
#[sqlx::test(migrations = "../../migrations")]
async fn latest_validation_and_small_window(pool: PgPool) {
    let project = seed_project(&pool).await;
    let owner = seed_owner(&pool).await;
    let conversation = create_conversation(&pool, SYSTEM_TENANT, project, owner, "latest")
        .await
        .unwrap()
        .id;
    seed_message_at(
        &pool,
        conversation,
        Uuid::new_v4(),
        "2026-11-08T10:00:00Z",
        "only",
    )
    .await;

    // ≤ limit：全量返回。
    let views = list_messages(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        None,
        None,
        None,
        None,
        true,
    )
    .await
    .unwrap();
    assert_eq!(
        views.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(),
        vec!["only"]
    );

    let t = chrono::DateTime::parse_from_rfc3339("2026-11-08T10:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let with_before = list_messages(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        Some(t),
        None,
        None,
        None,
        true,
    )
    .await;
    assert!(
        matches!(with_before, Err(ConversationError::Validation(_))),
        "{with_before:?}"
    );
    let with_after = list_messages(
        &pool,
        SYSTEM_TENANT,
        project,
        owner,
        conversation,
        None,
        Some(t),
        None,
        None,
        true,
    )
    .await;
    assert!(
        matches!(with_after, Err(ConversationError::Validation(_))),
        "{with_after:?}"
    );
}
