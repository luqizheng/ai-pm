//! DK-B 访谈编排器测试：纯函数（开关/预算/槽位/校验）+ 数据库端到端
//! （C1~C6、越权四类、记账、降级纪律、注入面）。

use super::*;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::discovery::{
    add_finding, add_question, append_message, apply_orchestrator_round, FindingCreator,
    NewFinding, RoundDraft, RoundFinding as DraftFinding, RoundQuestion as DraftQuestion,
};
use pm_domain::SYSTEM_TENANT;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── 纯函数用例 ─────────────────────────────────────────────────────────

#[test]
fn switch_defaults_off_and_typos_stay_off() {
    assert_eq!(
        DiscoveryOrchestratorMode::parse(None),
        DiscoveryOrchestratorMode::Off
    );
    assert_eq!(
        DiscoveryOrchestratorMode::parse(Some("off")),
        DiscoveryOrchestratorMode::Off
    );
    assert_eq!(
        DiscoveryOrchestratorMode::parse(Some("on")),
        DiscoveryOrchestratorMode::On
    );
    for typo in ["ture", "true", "ON", "On", "enable", "1", ""] {
        assert_eq!(
            DiscoveryOrchestratorMode::parse(Some(typo)),
            DiscoveryOrchestratorMode::Off,
            "typo {typo:?} must stay off"
        );
    }
}

fn slot_finding(slot: &str, kind: &str, status: &str) -> SlotFinding {
    SlotFinding {
        slot: Some(slot.to_string()),
        kind: kind.to_string(),
        verification_status: status.to_string(),
    }
}

#[test]
fn slot_coverage_needs_kind_and_a_live_verdict() {
    let report = evaluate_slots(
        "TASK_DISCOVERY",
        &[
            slot_finding("core_behavior", "observed", "PENDING"),
            slot_finding("users_and_roles", "user_confirmed", "CONFIRMED"),
            // speculation 不算覆盖
            slot_finding("data_and_state", "speculation", "PENDING"),
            // 人类否决的不算覆盖
            slot_finding("acceptance_criteria", "observed", "REJECTED"),
            // CONFLICT 也不算
            slot_finding("trigger_scenario", "user_confirmed", "CONFLICT"),
        ],
    );
    assert_eq!(report.green, vec!["users_and_roles", "core_behavior"]);
    assert!(report.missing.contains(&"data_and_state"));
    assert!(report.missing.contains(&"acceptance_criteria"));
    assert!(report.missing.contains(&"trigger_scenario"));
    assert_eq!(report.missing.len(), 4);

    // 槽位换了 kind 就不认
    let report = evaluate_slots(
        "UPDATE",
        &[slot_finding("core_behavior", "observed", "PENDING")],
    );
    assert!(report.missing.contains(&"change_point"));
    assert_eq!(report.missing.len(), 3, "UPDATE table has three slots");
}

fn long_sections() -> Vec<PromptSection> {
    [
        (SectionId::S1Project, "S1"),
        (SectionId::S2Cognition, "S2"),
        (SectionId::S3Digest, "S3"),
        (SectionId::S4Requirements, "S4"),
        (SectionId::S5Tasks, "S5"),
        (SectionId::S6Interview, "S6"),
    ]
    .into_iter()
    .map(|(id, marker)| PromptSection {
        id,
        text: marker.repeat(id.cap()),
    })
    .collect()
}

#[test]
fn budget_keeps_everything_when_under_the_cap() {
    let prompt = assemble_prompt(&long_sections(), Vec::new());
    assert!(prompt.dropped.is_empty(), "caps sum to the budget");
    assert_eq!(prompt.included.len(), 6);
    assert!(prompt.chars_total <= TOTAL_PROMPT_CHARS + 2_000);
    assert!(!prompt.system.contains("本节因预算被省略"));
}

#[test]
fn budget_drops_sections_in_the_ruling_order_and_flags_them() {
    let turns: Vec<ConversationTurn> = (0..RECENT_TURNS_K)
        .map(|index| {
            ConversationTurn::new("user", format!("{index:0>width$}", width = MAX_TURN_CHARS))
        })
        .collect();
    let prompt = assemble_prompt(&long_sections(), turns);
    assert_eq!(
        prompt.dropped,
        vec!["S5_tasks", "S4_requirements", "S2_cognition"],
        "drop order is S5 -> S4 -> S2 -> S3 (D-DK-7)"
    );
    assert_eq!(
        prompt.included,
        vec!["S1_project", "S3_kb_digest", "S6_interview"]
    );
    assert!(prompt.chars_total <= TOTAL_PROMPT_CHARS);
    for key in &prompt.dropped {
        assert!(
            prompt.system.contains(&format!("本节因预算被省略：{key}")),
            "each dropped section must be labelled in the prompt"
        );
    }
}

#[test]
fn absent_sections_are_not_reported_as_dropped() {
    let prompt = assemble_prompt(
        &[PromptSection {
            id: SectionId::S6Interview,
            text: "本次访谈".to_string(),
        }],
        Vec::new(),
    );
    assert!(prompt.dropped.is_empty());
    assert_eq!(prompt.included, vec!["S6_interview"]);
    assert_eq!(
        prompt.absent,
        vec![
            "S1_project",
            "S2_cognition",
            "S3_kb_digest",
            "S4_requirements",
            "S5_tasks"
        ],
        "DK-C has not projected S1~S5 yet; they are absent, not budget-dropped"
    );
}

#[test]
fn json_parsing_tolerates_fences_and_counts_forbidden_fields() {
    let body = "```json\n{\"answer_markdown\":\"hi\",\"findings\":[{\"content\":\"a\",\"verification_status\":\"CONFIRMED\"}],\"questions\":[]}\n```";
    let (output, forbidden) = parse_model_output(body).expect("fenced JSON must parse");
    assert_eq!(output.answer_markdown, "hi");
    assert_eq!(forbidden, 1);
    assert!(parse_model_output("不是 JSON").is_err());
    assert!(
        parse_model_output("{}").is_err(),
        "answer_markdown is required"
    );
}

fn finding_view(
    content: &str,
    kind: &str,
    status: &str,
    slot: Option<&str>,
) -> DiscoveryFindingView {
    DiscoveryFindingView {
        id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        kind: kind.to_string(),
        content: content.to_string(),
        confidence: None,
        verification_status: status.to_string(),
        evidence: json!([]),
        actor_role: slot.map(|slot| format!("slot:{slot}")),
        repository_ids: Vec::new(),
        created_at: chrono::Utc::now(),
        display_state: status.to_string(),
    }
}

fn llm_output(value: Value) -> LlmOutput {
    serde_json::from_value(value).expect("test fixtures must build LlmOutput")
}

/// 四类越权（增槽位 / 伪造 finding_ref / 自标 user_confirmed / 写
/// verification_status）+ 证据编造 + 超额截断：全部拒绝并计数，合法项照落。
#[test]
fn validation_rejects_the_four_overreach_classes() {
    let existing = vec![finding_view("既有事实", "observed", "PENDING", None)];
    let output = llm_output(json!({
        "answer_markdown": "好的",
        "findings": [
            {"content": "用户可以手机号登录", "kind": "user_confirmed", "quote": "手机号登录",
             "slot": "core_behavior", "evidence_refs": ["context 里没有的引用"]},
            {"content": "第二事实", "kind": "observed", "verification_status": "CONFIRMED"}
        ],
        "questions": [
            {"question": "越权槽位问题", "why_blocking": "x", "slot": "made_up_slot"},
            {"question": "伪造引用的问题", "why_blocking": "x", "slot": "users_and_roles",
             "finding_ref": "00000000-0000-0000-0000-000000000001"},
            {"question": "Q3", "why_blocking": "x", "slot": "core_behavior"},
            {"question": "Q4", "slot": "data_and_state"},
            {"question": "Q5", "slot": "acceptance_criteria"}
        ],
        "slot_updates": [{"slot": "also_made_up"}]
    }));
    let validated = validate_output(
        ValidationInputs {
            kind: "TASK_DISCOVERY",
            user_message: "我们要做手机号登录，短信验证码",
            whitelist: "context 内容",
            existing_findings: &existing,
            open_question_texts: &[],
        },
        output,
        1,
    )
    .expect("item-level overreach must not kill the round");

    assert_eq!(
        validated.overreach.verification_status_attempts, 1,
        "写 verification_status"
    );
    assert_eq!(
        validated.overreach.user_confirmed_claims, 1,
        "自标 user_confirmed"
    );
    assert_eq!(
        validated.overreach.unknown_slots, 2,
        "finding 槽位 + question 槽位 + slot_updates 越权"
    );
    assert_eq!(
        validated.overreach.forged_finding_refs, 1,
        "伪造 finding_ref"
    );
    assert_eq!(validated.overreach.fabricated_evidence, 1, "编造证据");
    assert_eq!(
        validated.overreach.truncated_questions, 1,
        "第 4 个问题被截断"
    );
    assert_eq!(validated.questions.len(), MAX_QUESTIONS_PER_ROUND);
    // 用户原话里有「手机号登录」→ 服务端判定归属 user_confirmed（不是采信模型）。
    assert_eq!(validated.findings[0].kind, "user_confirmed");
    assert!(
        validated.findings[0].evidence_quotes.is_empty(),
        "编造证据必须被丢弃"
    );
    // 合法槽位保留，越权槽位被摘掉（不是整条事实丢失）。
    assert_eq!(validated.findings[0].slot.as_deref(), Some("core_behavior"));
    assert_eq!(validated.findings[1].kind, "observed");
    assert!(validated.findings[1].slot.is_none());
    // 第一个问题因槽位越权被丢 → 合法问题里不含它；伪造引用被摘成 None。
    assert!(validated.questions.iter().all(|q| q.slot != "made_up_slot"));
    assert!(validated.questions.iter().any(|q| q.finding_ref.is_none()));
}

#[test]
fn user_confirmed_is_derived_from_the_users_own_words_only() {
    let output = llm_output(json!({
        "answer_markdown": "收到",
        "findings": [
            {"content": "用户提到要做审批流", "kind": "observed", "quote": "要做审批流"},
            {"content": "用户自述是 B 端产品", "kind": "user_confirmed", "quote": "我们其实是内部工具"}
        ],
        "questions": []
    }));
    let validated = validate_output(
        ValidationInputs {
            kind: "TASK_DISCOVERY",
            user_message: "我们要做审批流，另外这是给内部用的",
            whitelist: "",
            existing_findings: &[],
            open_question_texts: &[],
        },
        output,
        0,
    )
    .unwrap();
    assert_eq!(
        validated.findings[0].kind, "user_confirmed",
        "原话在用户消息里"
    );
    // 模型自标 user_confirmed 但 quote 不在用户消息里 → 归为 observed。
    assert_eq!(validated.findings[1].kind, "observed");
    assert_eq!(validated.overreach.user_confirmed_claims, 1);
}

#[test]
fn duplicate_questions_and_findings_are_dropped() {
    let existing = vec![finding_view("已存在的事实。", "observed", "PENDING", None)];
    let output = llm_output(json!({
        "answer_markdown": "好",
        "findings": [{"content": "已存在的事实", "kind": "observed"}],
        "questions": [{"question": "这个需求为什么要做？", "slot": "core_behavior"}]
    }));
    let validated = validate_output(
        ValidationInputs {
            kind: "TASK_DISCOVERY",
            user_message: "随便",
            whitelist: "",
            existing_findings: &existing,
            open_question_texts: &["这个需求为什么要做".to_string()],
        },
        output,
        0,
    )
    .unwrap();
    assert_eq!(validated.findings_deduped, 1);
    assert_eq!(validated.questions_deduped, 1);
    assert!(validated.findings.is_empty());
    assert!(validated.questions.is_empty());
}

// ── 数据库用例（sqlx::test）───────────────────────────────────────────

async fn seed_project(pool: &sqlx::PgPool) -> Uuid {
    bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: "DKB".to_string(),
            project_name: "dk-b".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id
}

async fn seed_human(pool: &sqlx::PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'owner') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_session(pool: &sqlx::PgPool, project_id: Uuid, kind: &str) -> (Uuid, Uuid) {
    let human = seed_human(pool).await;
    let session = pm_application::discovery::create_session(
        pool,
        SYSTEM_TENANT,
        project_id,
        "WEB",
        kind,
        human,
    )
    .await
    .unwrap();
    (session.id, human)
}

async fn seed_provider(pool: &sqlx::PgPool, endpoint: &str, model: &str) -> Uuid {
    let principal_id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'ai_pm', 'Seed')")
        .bind(principal_id)
        .execute(pool)
        .await
        .unwrap();
    let provider_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO ai.team_llm_provider \
         (id, tenant_id, name, provider_type, endpoint, configuration, status, created_by) \
         VALUES ($1, $2, 'seed-provider', 'openai_compatible', $3, $4, 'enabled', $5)",
    )
    .bind(provider_id)
    .bind(SYSTEM_TENANT)
    .bind(endpoint)
    .bind(json!({ "model": model }))
    .bind(principal_id)
    .execute(pool)
    .await
    .unwrap();
    provider_id
}

fn json_reply(content: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "choices": [{"message": {"content": content}}]
    }))
}

async fn mount_reply(server: &MockServer, content: &str) {
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(json_reply(content))
        .mount(server)
        .await;
}

async fn seed_slot_finding(pool: &sqlx::PgPool, session_id: Uuid, slot: &str, kind: &str) -> Uuid {
    add_finding(
        pool,
        SYSTEM_TENANT,
        session_id,
        NewFinding {
            kind: kind.to_string(),
            content: format!("{slot} 的事实（{kind}）"),
            confidence: Some(0.8),
            evidence: json!([]),
            repository_ids: vec![],
            actor_role: Some(format!("slot:{slot}")),
        },
        FindingCreator::AgentRelayed,
    )
    .await
    .unwrap()
    .id
}

async fn seed_all_task_slots(pool: &sqlx::PgPool, session_id: Uuid) {
    for slot in pm_application::discovery::TASK_DISCOVERY_SLOTS {
        seed_slot_finding(pool, session_id, slot.id, "observed").await;
    }
}

async fn message_count(pool: &sqlx::PgPool, session_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM discovery.discovery_message WHERE session_id = $1")
        .bind(session_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn finding_count(pool: &sqlx::PgPool, session_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM discovery.discovery_finding WHERE session_id = $1")
        .bind(session_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn question_count(pool: &sqlx::PgPool, session_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM discovery.discovery_question WHERE session_id = $1")
        .bind(session_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn session_status(pool: &sqlx::PgPool, session_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM discovery.discovery_session WHERE id = $1")
        .bind(session_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// C1 + 记账：LLM 提一个阻塞问题 → NEEDS_USER_INPUT，问题落库为 open，
/// ai 消息落库，`llm_call_record` 的 purpose/延迟可见。
#[sqlx::test(migrations = "../../migrations")]
async fn blocking_question_moves_to_needs_user_input_and_is_accounted(pool: sqlx::PgPool) {
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

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &json!({
            "answer_markdown": "明白。为了确认范围，我需要先问清验收口径。",
            "findings": [{"content": "需求是手机号登录", "kind": "user_confirmed", "quote": "手机号登录",
                          "slot": "core_behavior"}],
            "questions": [{"question": "登录成功后的失败提示要覆盖哪些情况？",
                           "why_blocking": "不确认会导致验收口径缺失", "slot": "acceptance_criteria"}],
            "slot_updates": []
        })
        .to_string(),
    )
    .await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.applied, 1, "{report:?}");
    assert_eq!(report.to_needs_user_input, 1);
    assert_eq!(report.questions_added, 1);
    assert_eq!(session_status(&pool, session_id).await, "NEEDS_USER_INPUT");

    let (status, why): (String, Option<String>) = sqlx::query_as(
        "SELECT status, why_blocking FROM discovery.discovery_question WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "open");
    assert!(why.is_some());

    // 归属由服务端按用户原话复算：kind=user_confirmed 且槽位标签在 actor_role。
    let (kind, actor_role): (String, Option<String>) = sqlx::query_as(
        "SELECT kind, actor_role FROM discovery.discovery_finding WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kind, "user_confirmed");
    assert_eq!(actor_role.as_deref(), Some("slot:core_behavior"));

    // 消息：user + ai 各一条（编排器只追加一条）。
    assert_eq!(message_count(&pool, session_id).await, 2);

    let record: (String, String, Option<i64>) = sqlx::query_as(
        "SELECT purpose, status, latency_ms FROM ai.llm_call_record WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(record.0, "discovery_orchestration");
    assert_eq!(record.1, "ok");
    assert!(record.2.is_some(), "latency must be recorded");

    // 回复后最新消息是 ai → 自动离开扫描集。
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.processed, 0);
}

/// C2：模型说「问完了」但槽位没全绿 → 不跃迁（留 IN_PROGRESS），不采信自述。
#[sqlx::test(migrations = "../../migrations")]
async fn model_claims_completion_but_missing_slots_keep_it_in_progress(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    seed_slot_finding(&pool, session_id, "core_behavior", "observed").await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "差不多就这些",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &json!({"answer_markdown": "我觉得差不多了。", "findings": [], "questions": []})
            .to_string(),
    )
    .await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.applied, 1);
    assert_eq!(report.to_review_required, 0);
    assert_eq!(session_status(&pool, session_id).await, "IN_PROGRESS");
}

/// C3：deferred 问题不阻塞跃迁，但必须出现在注入里（复核清单）。
#[sqlx::test(migrations = "../../migrations")]
async fn deferred_questions_do_not_block_but_reach_the_prompt(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    seed_all_task_slots(&pool, session_id).await;
    let question = add_question(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "老系统的数据要不要迁移？",
        Some("不确认会影响实施范围"),
        None,
    )
    .await
    .unwrap();
    pm_application::discovery::defer_question(&pool, SYSTEM_TENANT, question.id)
        .await
        .unwrap();
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "先按新系统做",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &json!({"answer_markdown": "好，先把待定项挂账。", "findings": [], "questions": []})
            .to_string(),
    )
    .await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.to_review_required, 1);
    assert_eq!(session_status(&pool, session_id).await, "REVIEW_REQUIRED");

    let requests = server.received_requests().await.unwrap();
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(
        system.contains("老系统的数据要不要迁移"),
        "deferred question must be visible in S6"
    );
}

/// 「三条件全绿」端到端：阻塞问题被用户这一轮答掉 + 槽位全绿 + 本轮无新问题
/// → REVIEW_REQUIRED；回答落 discovery_answer 且归属 session 创建者。
#[sqlx::test(migrations = "../../migrations")]
async fn three_conditions_green_reach_review_required(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, human) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    seed_all_task_slots(&pool, session_id).await;
    let question = add_question(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "验收标准如何判定？",
        Some("没有验收口径就无法生成任务"),
        None,
    )
    .await
    .unwrap();
    // 问题必须先于用户这一条消息（跳答匹配的时间前提）。
    sqlx::query(
        "UPDATE discovery.discovery_question SET created_at = now() - interval '1 minute' \
         WHERE id = $1",
    )
    .bind(question.id)
    .execute(&pool)
    .await
    .unwrap();
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "验收就是登录成功率和短信到达率都要有监控",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &json!({
            "answer_markdown": "已记录验收口径。",
            "findings": [{"content": "验收口径=登录成功率与短信到达率监控", "kind": "user_confirmed",
                          "quote": "登录成功率和短信到达率都要有监控",
                          "slot": "acceptance_criteria"}],
            "questions": []
        })
        .to_string(),
    )
    .await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.answers_consumed, 1, "{report:?}");
    assert_eq!(report.to_review_required, 1, "{report:?}");
    assert_eq!(session_status(&pool, session_id).await, "REVIEW_REQUIRED");

    let (status, answered_by): (String, Uuid) = sqlx::query_as(
        "SELECT q.status, a.answered_by FROM discovery.discovery_question q \
         JOIN discovery.discovery_answer a ON a.question_id = q.id WHERE q.id = $1",
    )
    .bind(question.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "answered");
    assert_eq!(answered_by, human, "回答归属会话创建者（用户自己说的话）");
}

/// C4：同一 (session_id, last_message_id) 重放 → StaleRound，零新增行。
#[sqlx::test(migrations = "../../migrations")]
async fn replaying_the_same_last_message_is_a_no_op(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    seed_all_task_slots(&pool, session_id).await;
    let message = append_message(&pool, SYSTEM_TENANT, session_id, "user", "WEB", "就这样")
        .await
        .unwrap();

    let draft = RoundDraft {
        expected_last_message_id: message.id,
        answer_markdown: "好的".to_string(),
        findings: vec![DraftFinding {
            kind: "observed".to_string(),
            content: "一次性事实".to_string(),
            confidence: None,
            slot: Some("core_behavior".to_string()),
            evidence_quotes: vec![],
        }],
        questions: vec![DraftQuestion {
            question: "还需要确认吗？".to_string(),
            why_blocking: None,
            finding_ref: Some(FindingRef::Round(0)),
            slot: "core_behavior".to_string(),
        }],
        consume_question: None,
        answer_text: "就这样".to_string(),
        next_status: "IN_PROGRESS".to_string(),
        correction: None,
    };
    apply_orchestrator_round(&pool, SYSTEM_TENANT, session_id, draft.clone())
        .await
        .unwrap();
    let findings_after_first = finding_count(&pool, session_id).await;
    let questions_after_first = question_count(&pool, session_id).await;
    let messages_after_first = message_count(&pool, session_id).await;

    let error = apply_orchestrator_round(&pool, SYSTEM_TENANT, session_id, draft)
        .await
        .expect_err("a replay must be refused");
    assert!(
        matches!(error, DiscoveryError::StaleRound { .. }),
        "unexpected error: {error:?}"
    );
    assert_eq!(finding_count(&pool, session_id).await, findings_after_first);
    assert_eq!(
        question_count(&pool, session_id).await,
        questions_after_first
    );
    assert_eq!(message_count(&pool, session_id).await, messages_after_first);
    assert_eq!(messages_after_first, 2, "user + exactly one ai reply");
}

/// C6：同一缺口连续 3 轮 → NEEDS_USER_INPUT（不死循环烧 token）。
#[sqlx::test(migrations = "../../migrations")]
async fn the_same_missing_slots_for_three_rounds_escalate_to_human(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &json!({"answer_markdown": "继续吧。", "findings": [], "questions": []}).to_string(),
    )
    .await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    for round in 1..=2 {
        append_message(
            &pool,
            SYSTEM_TENANT,
            session_id,
            "user",
            "WEB",
            &format!("第 {round} 轮补充"),
        )
        .await
        .unwrap();
        let report = worker.orchestrate_round().await.unwrap();
        assert_eq!(report.applied, 1);
        assert_eq!(
            report.to_needs_user_input, 0,
            "round {round} must not escalate yet"
        );
        assert_eq!(session_status(&pool, session_id).await, "IN_PROGRESS");
    }

    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "第三轮补充",
    )
    .await
    .unwrap();
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(
        report.to_needs_user_input, 1,
        "third identical gap escalates"
    );
    assert_eq!(session_status(&pool, session_id).await, "NEEDS_USER_INPUT");
}

/// 无 provider：零写入、零记账、状态不变（绝不写假内容）。
#[sqlx::test(migrations = "../../migrations")]
async fn no_provider_writes_nothing(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "要做手机号登录",
    )
    .await
    .unwrap();

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.skipped_no_provider, 1);
    assert_eq!(report.applied, 0);
    assert_eq!(message_count(&pool, session_id).await, 1, "no ai message");
    assert_eq!(finding_count(&pool, session_id).await, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM ai.llm_call_record")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(session_status(&pool, session_id).await, "IN_PROGRESS");

    // 冷却：第二轮不再重复扫描该会话。
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.processed, 0, "cooldown must keep the session out");
}

/// 模型回复不是 JSON：丢弃本轮，零写入，会话不动；调用仍记账（ok）。
#[sqlx::test(migrations = "../../migrations")]
async fn non_json_reply_writes_nothing(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "要做审批流",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(&server, "我建议我们先聊聊需求").await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.invalid_rounds, 1);
    assert_eq!(report.applied, 0);
    assert_eq!(message_count(&pool, session_id).await, 1, "no ai message");
    assert_eq!(finding_count(&pool, session_id).await, 0);
    assert_eq!(question_count(&pool, session_id).await, 0);
    assert_eq!(session_status(&pool, session_id).await, "IN_PROGRESS");
    // 模型调用确实发生过 → 账本有 ok 记录（解析失败不抹掉真实调用，F21 先例）。
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM ai.llm_call_record WHERE purpose = 'discovery_orchestration'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

/// 服务层硬闸：编排器给 COMPLETED 一律 Validation 拒（AI 无确认权）。
#[sqlx::test(migrations = "../../migrations")]
async fn the_service_refuses_ai_completion(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    let message = append_message(&pool, SYSTEM_TENANT, session_id, "user", "WEB", "好了")
        .await
        .unwrap();
    let draft = RoundDraft {
        expected_last_message_id: message.id,
        answer_markdown: "完成".to_string(),
        findings: vec![],
        questions: vec![],
        consume_question: None,
        answer_text: "好了".to_string(),
        next_status: "COMPLETED".to_string(),
        correction: None,
    };
    let error = apply_orchestrator_round(&pool, SYSTEM_TENANT, session_id, draft)
        .await
        .expect_err("AI must not complete a session");
    assert!(matches!(error, DiscoveryError::Validation(_)), "{error:?}");
    assert_eq!(session_status(&pool, session_id).await, "IN_PROGRESS");
    assert_eq!(message_count(&pool, session_id).await, 1);
}

/// 注入面：S6-only + 最近 K=6 轮（不用 20 轮窗口）；早期事实仍在注入里。
#[sqlx::test(migrations = "../../migrations")]
async fn interview_context_keeps_structured_memory_and_six_turns(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    seed_slot_finding(&pool, session_id, "core_behavior", "observed").await;
    sqlx::query(
        "UPDATE discovery.discovery_finding SET content = '早期事实：核心行为是审批' \
         WHERE session_id = $1",
    )
    .bind(session_id)
    .execute(&pool)
    .await
    .unwrap();
    for index in 0..25 {
        append_message(
            &pool,
            SYSTEM_TENANT,
            session_id,
            "user",
            "WEB",
            &format!("turn-{index:02}"),
        )
        .await
        .unwrap();
    }

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &json!({"answer_markdown": "ok", "findings": [], "questions": []}).to_string(),
    )
    .await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    worker.orchestrate_round().await.unwrap();

    let requests = server.received_requests().await.unwrap();
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(
        messages.len(),
        1 + RECENT_TURNS_K,
        "system + last 6 turns only"
    );
    let system = messages[0]["content"].as_str().unwrap();
    assert!(
        system.contains("早期事实：核心行为是审批"),
        "structured memory must survive long sessions (no 20-turn window)"
    );
    assert!(system.contains("[槽位状态]"));
    assert!(system.contains("core_behavior"));
    assert_eq!(messages[messages.len() - 1]["content"], "turn-24");
}

/// 连续 3 轮契约不合规 → NEEDS_USER_INPUT + 停扫直到用户再说话。
#[sqlx::test(migrations = "../../migrations")]
async fn three_unusable_rounds_escalate_and_stop_rescanning(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(&server, "我建议先聊聊").await;

    // 冷却置零：本用例验证「连续 3 次不合规 → 升级」，不是验证 30s 退避。
    let mut worker =
        DiscoveryOrchestratorWorker::new(pool.clone()).with_retry_cooldown(Duration::ZERO);
    for round in 1..=3 {
        append_message(
            &pool,
            SYSTEM_TENANT,
            session_id,
            "user",
            "WEB",
            &format!("第 {round} 轮"),
        )
        .await
        .unwrap();
        let report = worker.orchestrate_round().await.unwrap();
        assert_eq!(report.invalid_rounds, 1, "round {round}");
        assert_eq!(report.applied, 0);
    }
    assert_eq!(session_status(&pool, session_id).await, "NEEDS_USER_INPUT");
    assert_eq!(message_count(&pool, session_id).await, 3, "user rows only");

    // 停扫：同一条消息不再重试（不烧 token）。
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.processed, 0, "halted until the user speaks again");

    // 用户再说话 → 解除停扫并继续处理。
    append_message(&pool, SYSTEM_TENANT, session_id, "user", "WEB", "新的补充")
        .await
        .unwrap();
    mount_reply(
        &server,
        &json!({"answer_markdown": "好", "findings": [], "questions": []}).to_string(),
    )
    .await;
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.processed, 1, "a new user message resumes the loop");
}

// ── DK-C 集成：逐会话投影进入提示词；投影失败 fail-open 降级 S6-only ────────

use std::sync::Arc as StdArc;

use wiremock::matchers::body_string_contains;

use crate::discovery_context::{
    assemble as assemble_context, InterviewContext, InterviewContextBuilder, InterviewContextError,
    InterviewKind, SectionDraft, KEY_PROJECT_PROFILE,
};

/// 可注入的投影器桩：`fail=true` 恒败（验证读侧 fail-open），否则回带 marker 的 S1。
struct StubContextBuilder {
    fail: bool,
}

#[async_trait::async_trait]
impl InterviewContextBuilder for StubContextBuilder {
    async fn build(
        &self,
        _tenant_id: Uuid,
        _project_id: Uuid,
        _session_id: Uuid,
        _kind: InterviewKind,
    ) -> Result<InterviewContext, InterviewContextError> {
        if self.fail {
            return Err(InterviewContextError::ProjectNotFound(_project_id));
        }
        Ok(assemble_context(vec![SectionDraft::new(
            KEY_PROJECT_PROFILE,
            "项目概览",
            "MARKER-S1-PROFILE：来自投影器桩的项目画像。",
        )]))
    }
}

/// DK-C 接线（正）：注入的投影器 S1 逐会话进入提示词——LLM 请求体里必须
/// 出现投影 marker，否则本 mock 不匹配（404）→ 本轮失败 → applied=0。
#[sqlx::test(migrations = "../../migrations")]
async fn context_builder_sections_flow_into_the_prompt(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    seed_slot_finding(&pool, session_id, "core_behavior", "observed").await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "手机号登录，先这些",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_string_contains("MARKER-S1-PROFILE"))
        .respond_with(json_reply(
            &json!({"answer_markdown": "收到。", "findings": [], "questions": []}).to_string(),
        ))
        .mount(&server)
        .await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone())
        .with_context_builder(StdArc::new(StubContextBuilder { fail: false }));
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.applied, 1, "{report:?}");
}

/// DK-C 接线（负）：投影器失败 → WARN 后按 S6-only 继续（读侧 fail-open，
/// §2.4），访谈轮次不被投影故障阻塞。
#[sqlx::test(migrations = "../../migrations")]
async fn context_builder_failure_degrades_to_s6_only(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    seed_slot_finding(&pool, session_id, "core_behavior", "observed").await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "手机号登录，先这些",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &json!({"answer_markdown": "收到。", "findings": [], "questions": []}).to_string(),
    )
    .await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone())
        .with_context_builder(StdArc::new(StubContextBuilder { fail: true }));
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(
        report.applied, 1,
        "read-side fail-open: round still applies without S1~S5"
    );
}

// ── DK-D 集成：写回钩子在轮次路径上的调用纪律 ──────────────────────────────

use crate::discovery_writeback::{DecomposeOutcome, DraftOutcome, WriteBackError};

/// 记录调用序列的钩子桩：证明「谁在何时被调」而不假装做写回。
struct RecordingHook {
    calls: StdArc<std::sync::Mutex<Vec<&'static str>>>,
    fail: bool,
}

#[async_trait::async_trait]
impl crate::discovery_writeback::WriteBackHook for RecordingHook {
    async fn on_review_required(
        &self,
        _tenant_id: Uuid,
        _session: &pm_application::discovery::DiscoverySessionView,
    ) -> Result<DraftOutcome, WriteBackError> {
        self.calls.lock().unwrap().push("on_review_required");
        if self.fail {
            return Err(WriteBackError::Disabled);
        }
        Ok(DraftOutcome::skipped("stub"))
    }

    async fn decompose_if_approved(
        &self,
        _tenant_id: Uuid,
        _session: &pm_application::discovery::DiscoverySessionView,
    ) -> Result<DecomposeOutcome, WriteBackError> {
        self.calls.lock().unwrap().push("decompose");
        if self.fail {
            return Err(WriteBackError::Disabled);
        }
        Ok(DecomposeOutcome::NothingToDo { reason: "stub" })
    }
}

/// REVIEW_REQUIRED 轮：on_review_required → decompose 依序各调一次。
#[sqlx::test(migrations = "../../migrations")]
async fn write_back_hook_fires_in_order_on_review_required(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    seed_all_task_slots(&pool, session_id).await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "验收就是登录成功率和短信到达率都要有监控",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &json!({"answer_markdown": "已记录。", "findings": [], "questions": []}).to_string(),
    )
    .await;

    let calls = StdArc::new(std::sync::Mutex::new(Vec::new()));
    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone()).with_write_back(StdArc::new(
        RecordingHook {
            calls: calls.clone(),
            fail: false,
        },
    ));
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.to_review_required, 1, "{report:?}");
    assert_eq!(
        *calls.lock().unwrap(),
        vec!["on_review_required", "decompose"],
        "hook order must be draft-then-decompose on a REVIEW_REQUIRED round"
    );
}

/// 非 REVIEW_REQUIRED 轮（槽位未绿 → IN_PROGRESS）：只调 decompose。
#[sqlx::test(migrations = "../../migrations")]
async fn write_back_hook_skips_draft_when_not_review_required(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    seed_slot_finding(&pool, session_id, "core_behavior", "observed").await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "手机号登录，先这些",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &json!({"answer_markdown": "收到。", "findings": [], "questions": []}).to_string(),
    )
    .await;

    let calls = StdArc::new(std::sync::Mutex::new(Vec::new()));
    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone()).with_write_back(StdArc::new(
        RecordingHook {
            calls: calls.clone(),
            fail: false,
        },
    ));
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.applied, 1, "{report:?}");
    assert_eq!(
        *calls.lock().unwrap(),
        vec!["decompose"],
        "draft hook must not fire on a non-REVIEW_REQUIRED round"
    );
}

/// 钩子失败 fail-soft：主链七步已提交，轮次照常 applied，不因写回故障回滚。
#[sqlx::test(migrations = "../../migrations")]
async fn write_back_hook_failure_does_not_fail_the_round(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    seed_all_task_slots(&pool, session_id).await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "验收就是登录成功率和短信到达率都要有监控",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    mount_reply(
        &server,
        &json!({"answer_markdown": "已记录。", "findings": [], "questions": []}).to_string(),
    )
    .await;

    let calls = StdArc::new(std::sync::Mutex::new(Vec::new()));
    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone()).with_write_back(StdArc::new(
        RecordingHook {
            calls: calls.clone(),
            fail: true,
        },
    ));
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.applied, 1, "{report:?}");
    assert_eq!(report.drafts_submitted, 0);
    assert_eq!(report.task_batches_generated, 0);
    assert_eq!(
        *calls.lock().unwrap(),
        vec!["on_review_required", "decompose"]
    );
    assert_eq!(session_status(&pool, session_id).await, "REVIEW_REQUIRED");
}

/// DK-F 真栈缺陷的回归用例：**同事务内写入的消息必须按 `seq` 定序**。
///
/// 真栈实测（场景 2 首条访谈消息）：载体升级在**同一个事务**里写了
/// 「交接摘要」(ai) 与用户原话 (user) 两条消息——它们共享同一个
/// `created_at`（事务时间戳）。而扫描面用 `ORDER BY created_at DESC, id DESC`
/// 取「最新消息」，次级键是随机 UUID → 约一半概率选中 ai 那条 →
/// 「最新消息不是 user」→ 会话不进编排扫描集 → 访谈静默卡死。
///
/// 本用例把 id 的字典序安排成「ai 更大」，使旧的 `id DESC` 次级排序**必然**
/// 选中 ai（即必然复现缺陷），而 `seq DESC`（会话内权威顺序）必然选中 user。
/// 于是用例是确定性的：修好 → `processed == 1`；没修 → `processed == 0`。
#[sqlx::test(migrations = "../../migrations")]
async fn latest_message_is_ordered_by_seq_not_by_timestamp_tiebreak(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;

    // id 刻意让 ai 行更大（uuid 字典序）——旧排序（id DESC）会选中 ai。
    let ai_id = Uuid::parse_str("ffffffff-ffff-4fff-8fff-ffffffffffff").unwrap();
    let user_id = Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();

    // 同一事务里写两条消息 → created_at 相同，seq 递增（ai=1, user=2）。
    let mut tx = pool.begin().await.unwrap();
    for (id, speaker, content, seq) in [
        (ai_id, "ai", "【交接摘要】", 1_i64),
        (user_id, "user", "我要做手机号登录", 2_i64),
    ] {
        sqlx::query(
            "INSERT INTO discovery.discovery_message \
             (id, tenant_id, session_id, speaker, source, content, seq) \
             VALUES ($1, $2, $3, $4, 'WEB', $5, $6)",
        )
        .bind(id)
        .bind(SYSTEM_TENANT)
        .bind(session_id)
        .bind(speaker)
        .bind(content)
        .bind(seq)
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();

    // 两条消息的 created_at 确实相同（前提成立）。
    let (distinct_timestamps,): (i64,) = sqlx::query_as(
        "SELECT count(DISTINCT created_at) FROM discovery.discovery_message WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        distinct_timestamps, 1,
        "fixture must reproduce the same-transaction timestamp tie"
    );

    // 扫描面：最新消息必须是 user（seq=2），会话才进编排候选集。
    // 用例只断言「被扫描到」（模型回包不在本用例的断言面内）。
    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(
        report.processed, 1,
        "the user message must be seen as the latest message: {report:?}"
    );
    assert_eq!(
        report.stale_rounds, 0,
        "the round must not be dropped as stale: {report:?}"
    );
}

/// DK-F 真栈缺陷的回归用例：**既有 finding 的 id 必须出现在提示词里**。
///
/// 真栈实测（场景 2 更正轮）：输出契约要求 `target_finding_ref` 照抄
/// 「[既有发现]」里的 **uuid**，但 S6 渲染只写了正文没写 id → 模型只能照抄
/// 正文 → 服务端解析不出 uuid → 每轮更正都降级为「observed, unbound」，
/// F1（contradicted + user_confirmed 双行）在真栈永不成立。
///
/// 用例把「提示词正文含该 finding 的 id」钉住。
#[sqlx::test(migrations = "../../migrations")]
async fn existing_finding_ids_are_visible_in_the_prompt(pool: sqlx::PgPool) {
    let project_id = seed_project(&pool).await;
    let (session_id, _) = seed_session(&pool, project_id, "TASK_DISCOVERY").await;
    let finding_id = seed_slot_finding(&pool, session_id, "core_behavior", "observed").await;
    append_message(
        &pool,
        SYSTEM_TENANT,
        session_id,
        "user",
        "WEB",
        "手机号登录，先这些",
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    seed_provider(&pool, &server.uri(), "test-model").await;
    // 提示词必须带得上这个 id（body 断言）。
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_string_contains(&finding_id.to_string()))
        .respond_with(json_reply(
            &json!({"answer_markdown": "收到。", "findings": [], "questions": []}).to_string(),
        ))
        .mount(&server)
        .await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(report.applied, 1, "{report:?}");
    assert_eq!(
        report.failed, 0,
        "the model call must have matched: {report:?}"
    );
}

/// DK-E：三态提案分支（更正回流）验收面（独立文件，避免与 DK-B 用例纠缠）。
mod dke;
