//! DK-A 对话载体判定 + 单向升级 + 撤销（设计
//! `docs/product/ai-pm-dialog-orchestration-design-v1.md` §2.3/§2.4/§8.3 A1~A6）。
//!
//! 走真实 REST（`POST /conversations/{cid}/messages` 唯一发送入口 +
//! `POST /conversations/{cid}/handoff/undo`），夹具分类器注入
//! [`ConversationRouting`]（开关不读环境）。夹具 harness 沿用 `ai_conversations.rs`。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::auth::AuthConfig;
use pm_api::routes::{router_with_auth_config_and_routing, ConversationRouting};
use pm_application::conversation_router::{
    ClassifierError, Intent, IntentClassification, IntentClassifier, RouterMode,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const PRIVATE_PEM: &str = include_str!("keys/private.pem");
const PUBLIC_PEM: &str = include_str!("keys/public.pem");

const REQUIREMENT: &str = "本项目需要手机登录";
const QUERY: &str = "这个项目现在进度怎么样？";

// ── 夹具分类器 ────────────────────────────────────────────────────────

struct FixedClassifier {
    result: Result<IntentClassification, ClassifierError>,
    calls: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl IntentClassifier for FixedClassifier {
    async fn classify(
        &self,
        _project_id: Uuid,
        _content: &str,
    ) -> Result<IntentClassification, ClassifierError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.clone()
    }
}

fn routing(
    mode: RouterMode,
    result: Result<IntentClassification, ClassifierError>,
) -> (ConversationRouting, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    (
        ConversationRouting {
            mode,
            classifier: Arc::new(FixedClassifier {
                result,
                calls: calls.clone(),
            }),
        },
        calls,
    )
}

fn hit(intent: Intent, confidence: f64) -> Result<IntentClassification, ClassifierError> {
    Ok(IntentClassification { intent, confidence })
}

// ── harness ──────────────────────────────────────────────────────────

fn test_jwks() -> Arc<JWKSCache> {
    Arc::new(JWKSCache::from_pem(PUBLIC_PEM).expect("test public key must parse"))
}

fn token_for(sub: &str, tid: i64) -> String {
    sign_token(
        PRIVATE_PEM,
        &json!({
            "sub": sub, "tid": tid, "username": sub, "roles": [],
            "permissions": ["PM_MCP_READ"], "aud": "coder-micro-services",
        }),
        Duration::from_secs(300),
    )
    .expect("test token must sign")
}

fn app(pool: PgPool, routing: ConversationRouting) -> axum::Router {
    router_with_auth_config_and_routing(pool, test_jwks(), AuthConfig::default(), routing)
}

async fn call(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {}", token_for("alice", 7)));
    let body = match body {
        Some(v) => {
            builder = builder.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let response = app.oneshot(builder.body(body).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

struct Fixture {
    tenant: Uuid,
    project: Uuid,
    alice: Uuid,
}

async fn seed(pool: &PgPool) -> Fixture {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', '7', '7') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let alice: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', 'alice', true) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal_identity (provider, tenant_id, subject, principal_id) \
         VALUES ('lmms', $1, 'alice', $2)",
    )
    .bind(tenant)
    .bind(alice)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(tenant)
    .bind(alice)
    .execute(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, code, name, status) \
         VALUES ($1, 'C', 'C', 'active') RETURNING id",
    )
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let project = seed_project(pool, tenant, customer, "P1", alice).await;
    Fixture {
        tenant,
        project,
        alice,
    }
}

async fn seed_project(
    pool: &PgPool,
    tenant: Uuid,
    customer: Uuid,
    code: &str,
    member: Uuid,
) -> Uuid {
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, $3, $3, 'planning') RETURNING id",
    )
    .bind(tenant)
    .bind(customer)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'owner')",
    )
    .bind(tenant)
    .bind(project)
    .bind(member)
    .execute(pool)
    .await
    .unwrap();
    project
}

async fn new_conversation(app: axum::Router, project: Uuid) -> Uuid {
    let (status, v) = call(
        app,
        "POST",
        &format!("/v1/pm/projects/{project}/conversations"),
        Some(json!({"title": "手机登录讨论"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    v["id"].as_str().unwrap().parse().unwrap()
}

async fn send(app: axum::Router, project: Uuid, cid: Uuid, body: Value) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        &format!("/v1/pm/projects/{project}/conversations/{cid}/messages"),
        Some(body),
    )
    .await
}

async fn undo(app: axum::Router, project: Uuid, cid: Uuid) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        &format!("/v1/pm/projects/{project}/conversations/{cid}/handoff/undo"),
        None,
    )
    .await
}

async fn link_of(pool: &PgPool, cid: Uuid) -> Option<Uuid> {
    sqlx::query_scalar("SELECT discovery_session_id FROM ai.conversation WHERE id = $1")
        .bind(cid)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn chat_rows(pool: &PgPool, cid: Uuid) -> Vec<(String, String, Value)> {
    sqlx::query_as(
        "SELECT role, content, metadata FROM ai.conversation_message \
         WHERE conversation_id = $1 ORDER BY created_at, id",
    )
    .bind(cid)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn interview_rows(pool: &PgPool, sid: Uuid) -> Vec<(String, String, String, i64)> {
    sqlx::query_as(
        "SELECT speaker, source, content, seq FROM discovery.discovery_message \
         WHERE session_id = $1 ORDER BY seq",
    )
    .bind(sid)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn count(pool: &PgPool, sql: &str, project: Uuid) -> i64 {
    sqlx::query_scalar(sql)
        .bind(project)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn session_count(pool: &PgPool, project: Uuid) -> i64 {
    count(
        pool,
        "SELECT count(*) FROM discovery.discovery_session WHERE project_id = $1",
        project,
    )
    .await
}

async fn finding_count(pool: &PgPool, project: Uuid) -> i64 {
    count(
        pool,
        "SELECT count(*) FROM discovery.discovery_finding f \
         JOIN discovery.discovery_session s ON s.id = f.session_id WHERE s.project_id = $1",
        project,
    )
    .await
}

async fn session_status(pool: &PgPool, sid: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM discovery.discovery_session WHERE id = $1")
        .bind(sid)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn sid_of(v: &Value) -> Uuid {
    v["discovery_session_id"].as_str().unwrap().parse().unwrap()
}

// ── 升级路径 3 例 ────────────────────────────────────────────────────

/// 升级例 1（显式，规则 2）+ A1：一条 system 边界、零重复行、原子四步。
#[sqlx::test(migrations = "../../migrations")]
async fn explicit_interview_upgrade_is_atomic_and_single_written(pool: PgPool) {
    let fx = seed(&pool).await;
    // 开关 off：显式进入不依赖开关。
    let (r, calls) = routing(RouterMode::Off, hit(Intent::RequirementIntake, 0.99));
    let cid = new_conversation(app(pool.clone(), r.clone()), fx.project).await;

    let (status, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": "你好"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(v["landed"], "chat");
    assert!(v["discovery_session_id"].is_null());
    assert_eq!(v["role"], "user", "chat 落点保持旧契约形状: {v}");

    let (status, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "mode": "interview"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(v["landed"], "interview");
    let sid = sid_of(&v);
    assert_eq!(v["speaker"], "user");
    assert_eq!(v["content"], REQUIREMENT);
    assert_eq!(v["handoff"]["trigger"], "explicit");
    assert_eq!(v["handoff"]["created_session"], true);
    assert_eq!(v["handoff"]["notice"], "已切换到需求澄清");
    assert!(v["handoff"]["undo_until"].is_string());
    assert_eq!(calls.load(Ordering::SeqCst), 0, "显式路径不调用分类器");

    // ④ 链接已置。
    assert_eq!(link_of(&pool, cid).await, Some(sid));
    // ① 会话：WEB / TASK_DISCOVERY / IN_PROGRESS / 本人创建。
    let (source, kind, status_s, created_by): (String, String, String, Uuid) = sqlx::query_as(
        "SELECT source, kind, status, created_by FROM discovery.discovery_session WHERE id = $1",
    )
    .bind(sid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (source.as_str(), kind.as_str(), status_s.as_str()),
        ("WEB", "TASK_DISCOVERY", "IN_PROGRESS")
    );
    assert_eq!(created_by, fx.alice);

    // A1：chat 段 = [你好, system 边界]，只有一条 system，需求句零重复行。
    let rows = chat_rows(&pool, cid).await;
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].0, "user");
    assert_eq!(rows[1].0, "system");
    assert_eq!(
        rows[1].2["handoff"]["discovery_session_id"],
        json!(sid.to_string())
    );
    // Batch 3B：边界消息附 informational 信封（场景驱动生产）；
    // handoff 事实键原样保留——移植后链接态滤行使不进面板，信封对其他 AI 卡照常生效。
    assert_eq!(rows[1].2["envelope"]["type"], json!("informational"));
    assert_eq!(rows[1].2["envelope"]["status"], json!("presented"));
    assert_eq!(rows[1].2["envelope"]["title"], json!("已切换到需求澄清"));
    assert_eq!(
        rows[1].2["envelope"]["payload"]["contentMarkdown"].is_string(),
        json!(true)
    );
    assert_eq!(rows.iter().filter(|r| r.0 == "system").count(), 1);
    assert!(rows.iter().all(|r| r.1 != REQUIREMENT), "需求句不得落 chat");

    // ② 访谈段 = [ai/SYSTEM 交接摘要, user/WEB 需求句]；摘要不复制旧轮次原文。
    let msgs = interview_rows(&pool, sid).await;
    assert_eq!(msgs.len(), 2, "{msgs:?}");
    assert_eq!((msgs[0].0.as_str(), msgs[0].1.as_str()), ("ai", "SYSTEM"));
    assert!(msgs[0].2.contains("交接摘要"));
    assert!(!msgs[0].2.contains("你好"), "摘要禁逐条复制: {}", msgs[0].2);
    assert_eq!((msgs[1].0.as_str(), msgs[1].1.as_str()), ("user", "WEB"));
    assert_eq!(msgs[1].2, REQUIREMENT);
    assert_eq!(finding_count(&pool, fx.project).await, 0);
}

/// 升级例 2（隐式，开关 on，规则 4）：分类命中 → 新建会话 + 可见提示。
#[sqlx::test(migrations = "../../migrations")]
async fn implicit_intent_upgrade_when_router_on(pool: PgPool) {
    let fx = seed(&pool).await;
    let (r, calls) = routing(RouterMode::On, hit(Intent::KbCorrection, 0.9));
    let cid = new_conversation(app(pool.clone(), r.clone()), fx.project).await;

    let (status, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": "数据库写错了，应该是 PostgreSQL"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(v["landed"], "interview", "{v}");
    assert_eq!(v["handoff"]["trigger"], "implicit_intent");
    assert_eq!(v["handoff"]["intent"], "kb_correction");
    assert_eq!(
        v["handoff"]["notice"], "已切换到需求澄清",
        "A4 静默升级禁止"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let sid = sid_of(&v);
    let kind: String =
        sqlx::query_scalar("SELECT kind FROM discovery.discovery_session WHERE id = $1")
            .bind(sid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(kind, "UPDATE", "kb_correction → UPDATE");
    assert_eq!(link_of(&pool, cid).await, Some(sid));
}

/// 升级例 3（分类失败，开关 on）= A3：每种失败都落 chat、不建 session、不写 finding。
#[sqlx::test(migrations = "../../migrations")]
async fn classifier_failure_lands_in_chat_fail_closed(pool: PgPool) {
    let fx = seed(&pool).await;
    let failures = [
        Err(ClassifierError::NoProvider("provider disabled".into())),
        Err(ClassifierError::Timeout),
        Err(ClassifierError::Unavailable("503".into())),
        Err(ClassifierError::BadOutput("not json".into())),
        // 低于阈值 / 非升级型意图同样落 chat。
        hit(Intent::RequirementIntake, 0.3),
        hit(Intent::ProjectQuery, 0.99),
    ];
    for result in failures {
        let (r, calls) = routing(RouterMode::On, result.clone());
        let cid = new_conversation(app(pool.clone(), r.clone()), fx.project).await;
        let (status, v) = send(
            app(pool.clone(), r),
            fx.project,
            cid,
            json!({"content": REQUIREMENT}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{result:?}: {v}");
        assert_eq!(v["landed"], "chat", "{result:?}: {v}");
        assert!(v["handoff"].is_null());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(link_of(&pool, cid).await, None);
        let rows = chat_rows(&pool, cid).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (rows[0].0.as_str(), rows[0].1.as_str()),
            ("user", REQUIREMENT)
        );
    }
    assert_eq!(session_count(&pool, fx.project).await, 0, "A3：零 session");
    assert_eq!(finding_count(&pool, fx.project).await, 0, "A3：零 finding");
}

// ── 开关 / 本地闸 ─────────────────────────────────────────────────────

/// 开关 off（默认）：需求句也落 chat，分类器零调用（只有规则 1/2）。
/// 开关 on 但本地规则未命中：不调用分类器（省 token），落 chat。
#[sqlx::test(migrations = "../../migrations")]
async fn router_off_and_local_miss_never_call_classifier(pool: PgPool) {
    let fx = seed(&pool).await;
    let (off, off_calls) = routing(RouterMode::Off, hit(Intent::RequirementIntake, 0.99));
    let cid = new_conversation(app(pool.clone(), off.clone()), fx.project).await;
    let (_, v) = send(
        app(pool.clone(), off),
        fx.project,
        cid,
        json!({"content": REQUIREMENT}),
    )
    .await;
    assert_eq!(v["landed"], "chat");
    assert_eq!(off_calls.load(Ordering::SeqCst), 0);

    let (on, on_calls) = routing(RouterMode::On, hit(Intent::RequirementIntake, 0.99));
    let (_, v) = send(
        app(pool.clone(), on),
        fx.project,
        cid,
        json!({"content": QUERY}),
    )
    .await;
    assert_eq!(v["landed"], "chat");
    assert_eq!(on_calls.load(Ordering::SeqCst), 0);
    assert_eq!(session_count(&pool, fx.project).await, 0);
}

/// 规则 3（开关 on）：本人有未完结访谈 → 续接该访谈、不建新会话、不调分类器；
/// 撤销只回退链接、不暂停用户此前已在进行的会话。
#[sqlx::test(migrations = "../../migrations")]
async fn rule3_continues_own_open_interview(pool: PgPool) {
    let fx = seed(&pool).await;
    let open = pm_application::discovery::create_session(
        &pool,
        fx.tenant,
        fx.project,
        "WEB",
        "TASK_DISCOVERY",
        fx.alice,
    )
    .await
    .unwrap();
    let (r, calls) = routing(RouterMode::On, hit(Intent::RequirementIntake, 0.99));
    let cid = new_conversation(app(pool.clone(), r.clone()), fx.project).await;
    let (_, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT}),
    )
    .await;
    assert_eq!(v["landed"], "interview", "{v}");
    assert_eq!(sid_of(&v), open.id);
    assert_eq!(v["handoff"]["trigger"], "implicit_continue");
    assert_eq!(v["handoff"]["created_session"], false);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(session_count(&pool, fx.project).await, 1);

    let (status, v) = undo(app(pool.clone(), r), fx.project, cid).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["session_status"], "IN_PROGRESS");
    assert_eq!(session_status(&pool, open.id).await, "IN_PROGRESS");
    assert_eq!(link_of(&pool, cid).await, None);
}

// ── A2 / A4 / A5 / A6 ────────────────────────────────────────────────

/// A2：升级后同线程纯问答（开关 off 也一样）仍落访谈，不回落 chat。
#[sqlx::test(migrations = "../../migrations")]
async fn upgraded_thread_keeps_landing_in_interview(pool: PgPool) {
    let fx = seed(&pool).await;
    let (r, _) = routing(RouterMode::Off, hit(Intent::ChitChat, 0.99));
    let cid = new_conversation(app(pool.clone(), r.clone()), fx.project).await;
    let (_, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "mode": "interview"}),
    )
    .await;
    let sid = sid_of(&v);
    let chat_before = chat_rows(&pool, cid).await.len();

    let (status, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": QUERY}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(v["landed"], "interview");
    assert_eq!(sid_of(&v), sid);
    assert!(v["handoff"].is_null(), "续接不产生第二次升级");
    assert_eq!(chat_rows(&pool, cid).await.len(), chat_before);
    let msgs = interview_rows(&pool, sid).await;
    assert_eq!(msgs.last().unwrap().2, QUERY);
    assert_eq!(msgs.iter().map(|m| m.3).collect::<Vec<_>>(), vec![1, 2, 3]);
}

/// A4：撤销 = 本次新建会话置 PAUSED + 链接回退 + 撤销后回 chat；二次撤销 409；
/// 窗口过期 409 且不改任何状态。
#[sqlx::test(migrations = "../../migrations")]
async fn undo_handoff_pauses_and_unlinks_within_window(pool: PgPool) {
    let fx = seed(&pool).await;
    let (r, _) = routing(RouterMode::Off, hit(Intent::ChitChat, 0.99));
    let cid = new_conversation(app(pool.clone(), r.clone()), fx.project).await;
    let (_, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "mode": "interview"}),
    )
    .await;
    let sid = sid_of(&v);

    let (status, v) = undo(app(pool.clone(), r.clone()), fx.project, cid).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["session_status"], "PAUSED");
    assert_eq!(session_status(&pool, sid).await, "PAUSED");
    assert_eq!(link_of(&pool, cid).await, None);
    let rows = chat_rows(&pool, cid).await;
    assert!(
        rows.last().unwrap().2.get("handoff_undone").is_some(),
        "{rows:?}"
    );

    let (_, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": QUERY}),
    )
    .await;
    assert_eq!(v["landed"], "chat");

    let (status, v) = undo(app(pool.clone(), r.clone()), fx.project, cid).await;
    assert_eq!(status, StatusCode::CONFLICT, "{v}");

    // 窗口过期：再升级一次，把边界消息拨回 31 分钟前。
    let (_, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "mode": "interview"}),
    )
    .await;
    let sid2 = sid_of(&v);
    sqlx::query(
        "UPDATE ai.conversation_message SET created_at = created_at - interval '31 minutes' \
         WHERE conversation_id = $1 AND metadata ? 'handoff'",
    )
    .bind(cid)
    .execute(&pool)
    .await
    .unwrap();
    let (status, v) = undo(app(pool.clone(), r), fx.project, cid).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(v["message"], "undo_window_expired", "{v}");
    assert_eq!(session_status(&pool, sid2).await, "IN_PROGRESS");
    assert_eq!(link_of(&pool, cid).await, Some(sid2));
}

/// A5：会话列表只有 `ai.conversation` 一套；访谈不出现为列表项，仅以列暴露链接。
#[sqlx::test(migrations = "../../migrations")]
async fn conversation_list_is_the_only_thread_directory(pool: PgPool) {
    let fx = seed(&pool).await;
    let (r, _) = routing(RouterMode::Off, hit(Intent::ChitChat, 0.99));
    let cid = new_conversation(app(pool.clone(), r.clone()), fx.project).await;
    let (_, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "mode": "interview"}),
    )
    .await;
    let sid = sid_of(&v);
    let (status, list) = call(
        app(pool.clone(), r),
        "GET",
        &format!("/v1/pm/projects/{}/conversations", fx.project),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = list.as_array().unwrap();
    assert_eq!(items.len(), 1, "{list}");
    assert_eq!(items[0]["id"], json!(cid.to_string()));
    assert_eq!(items[0]["discovery_session_id"], json!(sid.to_string()));
}

/// A6：访谈 COMPLETED 后同线程纯问答 → landed=chat，不新建 session、不写 finding
/// （规则 1 须看状态，不能只查列存在）。PAUSED 同理。
#[sqlx::test(migrations = "../../migrations")]
async fn completed_interview_falls_back_to_chat(pool: PgPool) {
    let fx = seed(&pool).await;
    let (r, calls) = routing(RouterMode::On, hit(Intent::RequirementIntake, 0.99));
    let cid = new_conversation(app(pool.clone(), r.clone()), fx.project).await;
    let (_, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "mode": "interview"}),
    )
    .await;
    let sid = sid_of(&v);
    pm_application::discovery::transition_session(&pool, fx.tenant, sid, "COMPLETED")
        .await
        .unwrap();
    let sessions_before = session_count(&pool, fx.project).await;
    let interview_before = interview_rows(&pool, sid).await.len();

    let (status, v) = send(
        app(pool.clone(), r),
        fx.project,
        cid,
        json!({"content": QUERY}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(v["landed"], "chat", "{v}");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "纯问答本地闸不过，不调分类器"
    );
    assert_eq!(session_count(&pool, fx.project).await, sessions_before);
    assert_eq!(interview_rows(&pool, sid).await.len(), interview_before);
    assert_eq!(finding_count(&pool, fx.project).await, 0);
    let rows = chat_rows(&pool, cid).await;
    assert_eq!(rows.last().unwrap().1, QUERY);
}

// ── 不变量 / 边界 ─────────────────────────────────────────────────────

/// 不变量：升级后线程脱离第七循环 `conversation_reply` 的 pending 扫描集
/// （访谈消息不在 ai.conversation_message，最新行为 system 边界）。用真实
/// worker 的扫描（无 provider → 每个 pending 线程计一次 skipped_no_provider）。
#[sqlx::test(migrations = "../../migrations")]
async fn upgraded_thread_leaves_conversation_reply_scan_set(pool: PgPool) {
    let fx = seed(&pool).await;
    let (r, _) = routing(RouterMode::Off, hit(Intent::ChitChat, 0.99));
    let cid = new_conversation(app(pool.clone(), r.clone()), fx.project).await;
    send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": "你好"}),
    )
    .await;

    let mut worker = pm_ai::ConversationReplyWorker::new(pool.clone());
    let report = worker.reply_round().await.unwrap();
    assert_eq!(report.skipped_no_provider, 1, "升级前 chat 线程 pending");

    send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "mode": "interview"}),
    )
    .await;
    send(
        app(pool.clone(), r),
        fx.project,
        cid,
        json!({"content": QUERY}),
    )
    .await;
    let mut fresh = pm_ai::ConversationReplyWorker::new(pool.clone());
    let report = fresh.reply_round().await.unwrap();
    assert_eq!(
        report.skipped_no_provider, 0,
        "升级后线程不在 pending 集: {report:?}"
    );
}

/// 显式指定会话的边界：跨项目 → 404；已完结 → 409；mode 非法 → 400；
/// 失败请求零写入（不留半升级状态）。
#[sqlx::test(migrations = "../../migrations")]
async fn explicit_session_boundaries(pool: PgPool) {
    let fx = seed(&pool).await;
    let customer: Uuid = sqlx::query_scalar("SELECT customer_id FROM core.project WHERE id = $1")
        .bind(fx.project)
        .fetch_one(&pool)
        .await
        .unwrap();
    let other = seed_project(&pool, fx.tenant, customer, "P2", fx.alice).await;
    let foreign = pm_application::discovery::create_session(
        &pool,
        fx.tenant,
        other,
        "WEB",
        "TASK_DISCOVERY",
        fx.alice,
    )
    .await
    .unwrap();
    let done = pm_application::discovery::create_session(
        &pool,
        fx.tenant,
        fx.project,
        "WEB",
        "TASK_DISCOVERY",
        fx.alice,
    )
    .await
    .unwrap();
    pm_application::discovery::transition_session(&pool, fx.tenant, done.id, "COMPLETED")
        .await
        .unwrap();

    let (r, _) = routing(RouterMode::Off, hit(Intent::ChitChat, 0.99));
    let cid = new_conversation(app(pool.clone(), r.clone()), fx.project).await;
    let (status, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "discovery_session_id": foreign.id}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{v}");
    let (status, _) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "discovery_session_id": done.id}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "mode": "chatty"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(chat_rows(&pool, cid).await.is_empty());
    assert_eq!(link_of(&pool, cid).await, None);
    assert!(interview_rows(&pool, foreign.id).await.is_empty());

    // 显式指定本项目活跃会话 → 升级续接（不新建）。
    let live = pm_application::discovery::create_session(
        &pool, fx.tenant, fx.project, "WEB", "UPDATE", fx.alice,
    )
    .await
    .unwrap();
    let (status, v) = send(
        app(pool.clone(), r),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "discovery_session_id": live.id}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(sid_of(&v), live.id);
    assert_eq!(v["handoff"]["created_session"], false);
}

// ── 补充不变量（验收记录 §二追加）────────────────────────────────────

/// A1 强化：升级后重发显式指定同一会话 → 规则 1 短路续接，handoff=null，
/// 边界 system 消息仍只有一条（不堆叠）。
#[sqlx::test(migrations = "../../migrations")]
async fn repeat_explicit_send_does_not_stack_boundaries(pool: PgPool) {
    let fx = seed(&pool).await;
    let (r, _) = routing(RouterMode::Off, hit(Intent::ChitChat, 0.99));
    let cid = new_conversation(app(pool.clone(), r.clone()), fx.project).await;
    let (_, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "mode": "interview"}),
    )
    .await;
    let sid = sid_of(&v);
    let boundaries_after_first = chat_rows(&pool, cid)
        .await
        .iter()
        .filter(|m| m.0 == "system")
        .count();
    assert_eq!(boundaries_after_first, 1);

    let (status, v) = send(
        app(pool.clone(), r),
        fx.project,
        cid,
        json!({"content": QUERY, "discovery_session_id": sid}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(v["landed"], "interview");
    assert_eq!(sid_of(&v), sid);
    assert!(v["handoff"].is_null(), "续接不得再发边界: {v}");
    let boundaries = chat_rows(&pool, cid)
        .await
        .iter()
        .filter(|m| m.0 == "system")
        .count();
    assert_eq!(boundaries, 1);
}

/// 撤销冲突纪律：本次升级新建的会话已被推进（NEEDS_USER_INPUT）→ 撤销 409、
/// 链接不动、会话状态不被改写（不给状态机外行为留后门）。
#[sqlx::test(migrations = "../../migrations")]
async fn undo_blocked_once_session_advances(pool: PgPool) {
    let fx = seed(&pool).await;
    let (r, _) = routing(RouterMode::Off, hit(Intent::ChitChat, 0.99));
    let cid = new_conversation(app(pool.clone(), r.clone()), fx.project).await;
    let (_, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "mode": "interview"}),
    )
    .await;
    let sid = sid_of(&v);
    pm_application::discovery::transition_session(&pool, fx.tenant, sid, "NEEDS_USER_INPUT")
        .await
        .unwrap();

    let (status, v) = undo(app(pool.clone(), r), fx.project, cid).await;
    assert_eq!(status, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["message"], "discovery_session_not_undoable", "{v}");
    assert_eq!(link_of(&pool, cid).await, Some(sid));
    assert_eq!(session_status(&pool, sid).await, "NEEDS_USER_INPUT");
    let boundaries = chat_rows(&pool, cid)
        .await
        .iter()
        .filter(|m| m.2.get("handoff_undone").is_some())
        .count();
    assert_eq!(boundaries, 0);
}

/// C7（DK-A 入口）：两条线程续接同一访谈并发发送 → 会话行锁串行，`seq` 连续
/// 1..=N 零重复、零丢失、全部 201（无锁时 `MAX(seq)+1` 必撞 UNIQUE → 500）。
#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_threads_on_one_session_never_collide_on_seq(pool: PgPool) {
    let fx = seed(&pool).await;
    let (r, _) = routing(RouterMode::Off, hit(Intent::ChitChat, 0.99));
    let a = new_conversation(app(pool.clone(), r.clone()), fx.project).await;
    let b = new_conversation(app(pool.clone(), r.clone()), fx.project).await;
    let (_, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        a,
        json!({"content": REQUIREMENT, "mode": "interview"}),
    )
    .await;
    let sid = sid_of(&v);
    let (status, _) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        b,
        json!({"content": REQUIREMENT, "discovery_session_id": sid}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let before = interview_rows(&pool, sid).await.len();

    const ROUNDS: usize = 12;
    let mut tasks = tokio::task::JoinSet::new();
    for i in 0..ROUNDS {
        for cid in [a, b] {
            let (pool, r, project) = (pool.clone(), r.clone(), fx.project);
            tasks.spawn(async move {
                send(
                    app(pool, r),
                    project,
                    cid,
                    json!({"content": format!("并发 {i}")}),
                )
                .await
            });
        }
    }
    while let Some(joined) = tasks.join_next().await {
        let (status, v) = joined.unwrap();
        assert_eq!(status, StatusCode::CREATED, "{v}");
        assert_eq!(v["landed"], "interview", "{v}");
    }
    let seqs: Vec<i64> = interview_rows(&pool, sid)
        .await
        .iter()
        .map(|m| m.3)
        .collect();
    assert_eq!(seqs.len(), before + 2 * ROUNDS, "零丢失");
    assert_eq!(
        seqs,
        (1..=seqs.len() as i64).collect::<Vec<_>>(),
        "seq 连续无重复"
    );
}

// ── B03-01（R1）：REVIEW_REQUIRED 会话不再是死信 ──────────────────────

/// 续接进 `REVIEW_REQUIRED` 会话的发送 201 且回执如实携带 `interview_status`：
/// 消息照常落访谈（记录不丢），调用方据此提示「等待人工复核」；人工退回
/// `IN_PROGRESS` 后会话回到编排扫描集（恢复消费未处理消息）。
#[sqlx::test(migrations = "../../migrations")]
async fn send_into_review_required_interview_reports_status(pool: PgPool) {
    let fx = seed(&pool).await;
    let (r, _) = routing(RouterMode::Off, hit(Intent::ChitChat, 0.99));
    let cid = new_conversation(app(pool.clone(), r.clone()), fx.project).await;

    // 升级成访谈：回执即带访谈状态（此时 IN_PROGRESS）。
    let (_, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": REQUIREMENT, "mode": "interview"}),
    )
    .await;
    assert_eq!(v["landed"], "interview");
    assert_eq!(v["interview_status"], "IN_PROGRESS", "{v}");
    let sid = sid_of(&v);

    // 人工推进到 REVIEW_REQUIRED（编排器三条件全绿时的同一状态）。
    pm_application::discovery::transition_session(&pool, fx.tenant, sid, "REVIEW_REQUIRED")
        .await
        .unwrap();

    // 等待复核期间再发送：201、消息落访谈（记录不丢）、回执如实告知状态。
    let (status, v) = send(
        app(pool.clone(), r.clone()),
        fx.project,
        cid,
        json!({"content": QUERY}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(v["landed"], "interview", "{v}");
    assert_eq!(v["interview_status"], "REVIEW_REQUIRED", "{v}");
    let msgs = interview_rows(&pool, sid).await;
    assert_eq!(msgs.last().unwrap().2, QUERY);

    // 编排器不消费该消息（扫描谓词不含 REVIEW_REQUIRED——前提不变）。
    let mut worker = pm_ai::ConversationReplyWorker::new(pool.clone());
    let report = worker.reply_round().await.unwrap();
    assert_eq!(report.skipped_no_provider, 0, "线程不在 chat 扫描集");
    assert_eq!(
        interview_rows(&pool, sid).await.last().unwrap().2,
        QUERY,
        "消息仍在，无人消费"
    );

    // 人工退回 IN_PROGRESS：会话回编排扫描集（此后编排器恢复消费）。
    pm_application::discovery::transition_session(&pool, fx.tenant, sid, "IN_PROGRESS")
        .await
        .unwrap();
    assert_eq!(session_status(&pool, sid).await, "IN_PROGRESS");
}
