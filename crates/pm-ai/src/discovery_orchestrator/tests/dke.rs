//! DK-E：三态提案分支（更正回流）验收面。
//!
//! 覆盖 §8.3 F 组 F1~F3（(b) 路径口径：零提案行 + 标记可见）、更正识别
//! fail-closed 两例、注入面「更正后下一轮已引用新事实」、C1 护栏与 F7
//! 人工裁决不可绕。

use super::{
    mount_reply, seed_project, seed_provider, seed_session, seed_slot_finding, session_status,
};
use pm_application::discovery::{
    add_finding, add_question, append_message, record_finding_verification, transition_session,
    DiscoveryError, FindingCreator, NewFinding,
};
use pm_domain::SYSTEM_TENANT;
use serde_json::{json, Value};
use uuid::Uuid;
use wiremock::MockServer;

use crate::discovery_orchestrator::DiscoveryOrchestratorWorker;

/// 按会话 kind 铺满槽位（更正轮的复核态需要槽位全绿；C1 用例仍可裁）。
async fn seed_all_slots(pool: &sqlx::PgPool, session_id: Uuid, kind: &str) {
    for slot in pm_application::discovery::slots_for_kind(kind) {
        seed_slot_finding(pool, session_id, slot.id, "observed").await;
    }
}

async fn finding_snapshot(pool: &sqlx::PgPool, finding_id: Uuid) -> (String, String, String) {
    sqlx::query_as(
        "SELECT kind, content, verification_status FROM discovery.discovery_finding WHERE id = $1",
    )
    .bind(finding_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seeded_old_finding(pool: &sqlx::PgPool, session_id: Uuid) -> Uuid {
    add_finding(
        pool,
        SYSTEM_TENANT,
        session_id,
        NewFinding {
            kind: "observed".to_string(),
            content: "认证使用 session cookie".to_string(),
            confidence: Some(0.7),
            evidence: json!([]),
            repository_ids: vec![],
            actor_role: None,
        },
        FindingCreator::AgentRelayed,
    )
    .await
    .unwrap()
    .id
}

/// 更正轮模型回复（contract 里多一个 correction 对象）。
fn correction_reply(
    target_finding_id: Uuid,
    corrected_fact: &str,
    quote: &str,
    answer: &str,
) -> String {
    json!({
        "answer_markdown": answer,
        "findings": [],
        "questions": [],
        "correction": {
            "intent": "correction",
            "target_finding_ref": target_finding_id.to_string(),
            "corrected_fact": corrected_fact,
            "quote": quote,
            "target_document": "repos/snapshot/repository-snapshot.md"
        }
    })
    .to_string()
}

async fn contradicted_count(pool: &sqlx::PgPool, session_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM discovery.discovery_finding \
         WHERE session_id = $1 AND kind = 'contradicted'",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn marker_count(pool: &sqlx::PgPool, session_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM discovery.discovery_question \
         WHERE session_id = $1 AND question LIKE '[%'",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// F1 + F2(b) + F3：更正落 `contradicted` + `user_confirmed` 双行、旧行零改；
/// 零提案行、KB/PC 零写入；`[更正待知识应用]` deferred 标记可见；会话进复核。
#[sqlx::test(migrations = "../../migrations")]
async fn correction_records_both_rows_and_keeps_kb_pc_untouched(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "INITIAL_ONBOARDING").await;
    seed_all_slots(&pool, session_id, "INITIAL_ONBOARDING").await;
    let old_id = seeded_old_finding(&pool, session_id).await;
    let before = finding_snapshot(&pool, old_id).await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "不对，认证不是 session，是 JWT（Bearer token）",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &correction_reply(
            old_id,
            "认证使用 JWT（Bearer token）",
            "认证不是 session，是 JWT",
            "已更正：认证走 JWT。",
        ),
    )
    .await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.corrections_applied, 1, "{report:?}");
    assert_eq!(report.correction_markers, 1, "{report:?}");
    assert_eq!(report.to_review_required, 1, "{report:?}");
    assert_eq!(session_status(&pool, session_id).await, "REVIEW_REQUIRED");

    // F1：旧 finding 零删改（kind / content / verification_status 三项全同）。
    assert_eq!(finding_snapshot(&pool, old_id).await, before);

    // F1：双行落库，且创建通道仍是 AgentRelayed → PENDING（F7）。
    let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        "SELECT id, kind, content, verification_status FROM discovery.discovery_finding \
         WHERE session_id = $1 AND kind IN ('contradicted', 'user_confirmed') \
         ORDER BY kind",
    )
    .bind(session_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2, "{rows:?}");
    let contradicted_id = rows[0].0;
    assert_eq!(rows[0].1, "contradicted");
    assert_eq!(rows[1].1, "user_confirmed");
    assert_eq!(rows[1].2, "认证使用 JWT（Bearer token）");
    assert_eq!(rows[0].3, "PENDING");
    assert_eq!(rows[1].3, "PENDING");
    assert!(
        rows[0].2.contains("认证使用 session cookie"),
        "contradicted row must name the negated fact: {}",
        rows[0].2
    );

    // 「指向旧 finding」的零迁移载体：system_observation description。
    let link: String = sqlx::query_scalar(
        "SELECT description FROM discovery.discovery_finding_evidence \
         WHERE finding_id = $1 AND evidence_type = 'system_observation'",
    )
    .bind(contradicted_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(link, format!("contradicts:{old_id}"));

    // 本轮回复原样落库（同轮回答已按用户原话更正；系统性「后续问答引用新事实」
    // 由 follow_up_round_is_answered_from_the_corrected_facts 钉住）。
    let ai_message: String = sqlx::query_scalar(
        "SELECT content FROM discovery.discovery_message \
         WHERE session_id = $1 AND speaker = 'ai' ORDER BY seq DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(ai_message.contains("JWT"), "{ai_message}");

    // 结构化标记：deferred + 指向 contradicted 行 + 两侧挂账可见。
    let (marker, marker_status, marker_finding): (String, String, Option<Uuid>) = sqlx::query_as(
        "SELECT question, status, finding_id FROM discovery.discovery_question \
         WHERE session_id = $1 AND question LIKE '[%' \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(marker_status, "deferred", "deferred 不阻塞跃迁（§4.3）");
    assert_eq!(marker_finding, Some(contradicted_id));
    assert!(marker.starts_with("[更正待知识应用]"), "{marker}");
    assert!(
        marker.contains("KP-3") && marker.contains("KP-2"),
        "{marker}"
    );

    // F2(b)：零提案行（不造影子文档行、不绕 FK 语义）。
    let proposals: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM discovery.discovery_proposal WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(proposals, 0);

    // F3：pc.* 零写入；KB 行零变化（本用例全程不产生的两个域同样是 0）。
    for table in [
        "pc.proposal",
        "pc.cognition_object",
        "pc.cognition_revision",
        "kb.document_revision",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "{table} must stay untouched");
    }
}

/// 更正后的注入面：下一轮提示词（S6）必须已含新事实与 contradicted 行——
/// 复核后人工「要求补充」把会话回退 `IN_PROGRESS`，编排器答复即引用新事实。
#[sqlx::test(migrations = "../../migrations")]
async fn follow_up_round_is_answered_from_the_corrected_facts(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "INITIAL_ONBOARDING").await;
    seed_all_slots(&pool, session_id, "INITIAL_ONBOARDING").await;
    let old_id = seeded_old_finding(&pool, session_id).await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "不对，认证不是 session，是 JWT（Bearer token）",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &correction_reply(
            old_id,
            "认证使用 JWT（Bearer token）",
            "认证不是 session，是 JWT",
            "已更正。",
        ),
    )
    .await;
    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    worker.orchestrate_round().await.unwrap();
    assert_eq!(session_status(&pool, session_id).await, "REVIEW_REQUIRED");

    // 人工复核「要求补充」→ REVIEW_REQUIRED → IN_PROGRESS（既有边，§4.4）。
    transition_session(&pool, SYSTEM_TENANT, session_id, "IN_PROGRESS")
        .await
        .unwrap();
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "那认证流程具体是怎么走的？",
    )
    .await
    .unwrap();
    mount_reply(
        &server,
        &json!({
            "answer_markdown": "认证走 JWT（Bearer token）。",
            "findings": [],
            "questions": []
        })
        .to_string(),
    )
    .await;
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.applied, 1, "{report:?}");

    // 第二轮请求的 system prompt（S6）必须同时含：更正后的新事实、被否定的旧事实。
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2, "one model call per round");
    let body: Value = serde_json::from_slice(&requests[1].body).unwrap();
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(
        system.contains("认证使用 JWT（Bearer token）"),
        "the corrected fact must be injected in the next round"
    );
    assert!(
        system.contains("用户更正：此前记录的事实「认证使用 session cookie」不再成立"),
        "the contradicted row must be visible to the model"
    );
}

/// 更正识别 fail-closed 一：模型给了 correction 但引文落不进本轮用户原话
/// （普通问答被误判为更正）→ 整条丢弃，零 contradicted、零标记。
#[sqlx::test(migrations = "../../migrations")]
async fn ordinary_question_is_not_misread_as_a_correction(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "INITIAL_ONBOARDING").await;
    let old_id =
        seed_slot_finding(&pool, session_id, "tech_stack_and_constraints", "observed").await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "顺便介绍一下这个项目的部署流程",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &correction_reply(
            old_id,
            "认证使用 JWT",
            "认证其实是 JWT", // 本轮用户消息里没有这句话
            "部署流程如下。",
        ),
    )
    .await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.corrections_applied, 0, "{report:?}");
    assert_eq!(report.correction_markers, 0, "{report:?}");
    assert_eq!(
        contradicted_count(&pool, session_id).await,
        0,
        "an unanchored correction must not produce a contradicted row"
    );
    assert_eq!(marker_count(&pool, session_id).await, 0);
    assert_eq!(session_status(&pool, session_id).await, "IN_PROGRESS");
}

/// 更正识别 fail-closed 二：目标不可定位 → 更正事实降级为 `observed`，
/// 不硬绑任何旧 finding（零 contradicted、零标记）。
#[sqlx::test(migrations = "../../migrations")]
async fn unresolvable_correction_target_degrades_to_observed(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "INITIAL_ONBOARDING").await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "更正一下：认证不是 session，是 JWT",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &correction_reply(
            Uuid::new_v4(), // 本 session 不存在
            "认证使用 JWT",
            "认证不是 session，是 JWT",
            "已记录。",
        ),
    )
    .await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.corrections_applied, 0, "{report:?}");
    assert_eq!(report.correction_markers, 0, "{report:?}");
    assert_eq!(contradicted_count(&pool, session_id).await, 0);

    let observed: String = sqlx::query_scalar(
        "SELECT content FROM discovery.discovery_finding \
         WHERE session_id = $1 AND kind = 'observed' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(observed, "认证使用 JWT", "fail-closed: observed, unbound");
}

/// C1 护栏：更正成立但仍有阻塞问题未答 → 会话留在 `NEEDS_USER_INPUT`，
/// 不得借更正通道绕过「有阻塞 open 问题就不进复核」。
#[sqlx::test(migrations = "../../migrations")]
async fn correction_cannot_bypass_open_blocking_questions(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "INITIAL_ONBOARDING").await;
    seed_all_slots(&pool, session_id, "INITIAL_ONBOARDING").await;
    let old_id = seeded_old_finding(&pool, session_id).await;
    // 用户这条消息不是对任何问题的回答；阻塞问题在它之后才出现（并发 REST
    // 追加的真实形态）——`pick_consumed_question` 只消费「早于本轮消息」的
    // 最新问题时，这一条仍算未答。
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "先更正：认证不是 session，是 JWT",
    )
    .await
    .unwrap();
    add_question(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "数据库迁移策略是什么？",
        Some("迁移策略不明会让部署方案落空"),
        None,
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &correction_reply(
            old_id,
            "认证使用 JWT",
            "认证不是 session，是 JWT",
            "已更正。",
        ),
    )
    .await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.corrections_applied, 1, "{report:?}");
    let status = session_status(&pool, session_id).await;
    assert_ne!(
        status, "REVIEW_REQUIRED",
        "C1: a blocking open question must keep the session out of review"
    );
    // DK-B 既有语义：本轮没有**新**阻塞问题时留在 IN_PROGRESS（不是
    // NEEDS_USER_INPUT——那是「本轮产出了阻塞问题」的落点）。
    assert_eq!(status, "IN_PROGRESS");
}

/// F7：编排器落下的更正两行都是 PENDING；人工裁决面只认真人
/// （AI principal 直调 `record_finding_verification` 被服务层拒绝）。
#[sqlx::test(migrations = "../../migrations")]
async fn correction_verification_requires_a_human_decision(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, human) = seed_session(&pool, project_id, "INITIAL_ONBOARDING").await;
    seed_all_slots(&pool, session_id, "INITIAL_ONBOARDING").await;
    let old_id = seeded_old_finding(&pool, session_id).await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "不对，认证不是 session，是 JWT",
    )
    .await
    .unwrap();
    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &correction_reply(
            old_id,
            "认证使用 JWT",
            "认证不是 session，是 JWT",
            "已更正。",
        ),
    )
    .await;
    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    worker.orchestrate_round().await.unwrap();

    let confirmed_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM discovery.discovery_finding \
         WHERE session_id = $1 AND kind = 'user_confirmed' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let ai_principal: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('ai_pm', 'orchestrator') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let error = record_finding_verification(
        &pool,
        SYSTEM_TENANT,
        session_id,
        confirmed_id,
        "CONFIRMED",
        "我确认",
        ai_principal,
        None,
        None,
    )
    .await
    .expect_err("AI principals must not decide findings");
    assert!(matches!(error, DiscoveryError::Validation(_)), "{error:?}");
    let status: String = sqlx::query_scalar(
        "SELECT verification_status FROM discovery.discovery_finding WHERE id = $1",
    )
    .bind(confirmed_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "PENDING", "failed decision must not change the row");

    // 真人裁决照常可用（同一行 → CONFIRMED），证明闸门只拦非人类。
    record_finding_verification(
        &pool,
        SYSTEM_TENANT,
        session_id,
        confirmed_id,
        "CONFIRMED",
        "用户已明确",
        human,
        None,
        None,
    )
    .await
    .unwrap();
    let status: String = sqlx::query_scalar(
        "SELECT verification_status FROM discovery.discovery_finding WHERE id = $1",
    )
    .bind(confirmed_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "CONFIRMED");
}
