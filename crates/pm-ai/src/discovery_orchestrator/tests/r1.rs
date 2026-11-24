//! R1（B03-01）对话主路径可靠性：编排轮失败对用户可见 + 人工复核退回后恢复消费。
//!
//! 覆盖四件事：
//! 1. 模型调用失败 → `last_round_error` 落库（会话详情/访谈面板可见）；
//! 2. 成功一轮 → 失败标记清除；
//! 3. `REVIEW_REQUIRED` 会话人工退回 `IN_PROGRESS` → 编排器恢复消费未处理消息；
//! 4. `record_round_error` 超长原因截断（错误文本可能极长）。

use super::{
    mount_reply, seed_project, seed_provider, seed_session, session_status,
    DiscoveryOrchestratorWorker,
};
use pm_application::discovery::{append_message, record_round_error, transition_session};
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use uuid::Uuid;

async fn last_round_error(pool: &sqlx::PgPool, session_id: Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT last_round_error FROM discovery.discovery_session WHERE id = $1")
        .bind(session_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// 模型端点不可达 → 轮次失败原因落库；随后成功一轮即清除。
#[sqlx::test(migrations = "../../migrations")]
async fn round_failure_persists_and_success_clears(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "我要做手机号登录",
    )
    .await
    .unwrap();

    // 无监听的端口：目录解析成功（provider enabled + 目录三件套），HTTP 调用失败。
    seed_provider(&pool, "http://127.0.0.1:1", "test-model").await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.failed, 1, "{report:?}");

    let error = last_round_error(&pool, session_id).await;
    assert!(
        error
            .as_deref()
            .is_some_and(|e| e.starts_with("model_error")),
        "failure reason must be persisted for UI: {error:?}"
    );
    assert_eq!(
        session_status(&pool, session_id).await,
        "IN_PROGRESS",
        "失败不推进状态机"
    );

    // 成功一轮（新 worker：冷却表是进程内存）→ 标记清除。
    // （provider 名称有唯一约束：改 endpoint 指向 mock，而非再种一行。）
    let server = wiremock::MockServer::start().await;
    sqlx::query("UPDATE ai.team_llm_provider SET endpoint = $1 WHERE tenant_id = $2")
        .bind(&server.uri())
        .bind(SYSTEM_TENANT)
        .execute(&pool)
        .await
        .unwrap();
    mount_reply(
        &server,
        &json!({
            "answer_markdown": "收到，已记录。",
            "findings": [{"content": "需求是手机号登录", "kind": "user_confirmed",
                          "quote": "手机号登录", "slot": "core_behavior"}],
            "questions": [],
            "slot_updates": []
        })
        .to_string(),
    )
    .await;
    let mut fresh = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = fresh.orchestrate_round().await.unwrap();
    assert_eq!(report.applied, 1, "{report:?}");
    assert_eq!(
        last_round_error(&pool, session_id).await,
        None,
        "成功一轮必须清除失败标记"
    );
}

/// `REVIEW_REQUIRED` 会话不在扫描集（前提不变）；人工退回 `IN_PROGRESS`
/// 后恢复消费——用户消息在退回前落库的内容不丢，由下一轮编排正常处理。
#[sqlx::test(migrations = "../../migrations")]
async fn resumed_review_required_session_rejoins_the_scan_set(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "补一点：要支持手机号登录",
    )
    .await
    .unwrap();

    // 人工推进到 REVIEW_REQUIRED（等待复核）：不在扫描集。
    transition_session(&pool, SYSTEM_TENANT, session_id, "REVIEW_REQUIRED")
        .await
        .unwrap();
    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(
        report.processed, 0,
        "REVIEW_REQUIRED 不是编排候选（前提不变）"
    );

    // 人工退回 → 恢复扫描，消息被消费。
    transition_session(&pool, SYSTEM_TENANT, session_id, "IN_PROGRESS")
        .await
        .unwrap();
    let server = wiremock::MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &json!({
            "answer_markdown": "收到补充。",
            "findings": [{"content": "需要支持手机号登录", "kind": "user_confirmed",
                          "quote": "手机号登录", "slot": "core_behavior"}],
            "questions": [],
            "slot_updates": []
        })
        .to_string(),
    )
    .await;
    let mut fresh = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = fresh.orchestrate_round().await.unwrap();
    assert_eq!(report.applied, 1, "{report:?}");
    // 消息已被消费：本轮落了 ai 回复（最新消息不再是 speaker=user，自然离开扫描集）。
    let last_speaker: String = sqlx::query_scalar(
        "SELECT speaker FROM discovery.discovery_message \
         WHERE session_id = $1 ORDER BY seq DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(last_speaker, "ai", "退回后编排器必须消费待处理消息");
}

/// 超长失败原因截断到 500 字符（错误文本可能极长，不允许无界写入）。
#[sqlx::test(migrations = "../../migrations")]
async fn round_error_is_truncated_to_a_bounded_length(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    let long_reason = "x".repeat(5000);
    record_round_error(&pool, SYSTEM_TENANT, session_id, &long_reason)
        .await
        .unwrap();
    let stored = last_round_error(&pool, session_id).await.unwrap();
    assert_eq!(stored.chars().count(), 500);
}
