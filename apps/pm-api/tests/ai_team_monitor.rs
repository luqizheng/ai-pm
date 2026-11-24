//! `/v1/pm/projects/{project_id}/ai-team/{overview,monitor}` REST 契约
//! （UI-2，U-1=C 现有表只读聚合，零迁移；
//! `docs/project-ai-team-v2/ai-pm-ai-team-ui-monitor-adjudication-pack-v1.md` §〇）。
//!
//! 覆盖：monitor 200 字段结构（runs 排序 / events 过滤与 reason 归并）、
//! overview 计数口径（roles / 活跃与近 24h Run / awaiting_decision+blocked /
//! Provider 指定与继承）、空项目零值结构、非成员 404。
//!
//! 夹具沿用 `ai_roles.rs` 的种子 harness（tenant external_key = JWT tid）。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::auth::AuthConfig;
use pm_api::routes::router_with_auth_config;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

const PRIVATE_PEM: &str = include_str!("keys/private.pem");
const PUBLIC_PEM: &str = include_str!("keys/public.pem");

fn test_jwks() -> Arc<JWKSCache> {
    Arc::new(JWKSCache::from_pem(PUBLIC_PEM).expect("test public key must parse"))
}

fn token_for(sub: &str, tid: i64) -> String {
    sign_token(
        PRIVATE_PEM,
        &json!({
            "sub": sub, "tid": tid, "username": sub, "roles": [],
            "permissions": [], "aud": "coder-micro-services",
        }),
        // 1h：本机 sqlx::test 迁移慢，长链路用例会超过 300s 默认（ai_roles 同款）。
        Duration::from_secs(3600),
    )
    .expect("test token must sign")
}

fn request(method: &str, uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

async fn call(app: &Router, uri: &str, token: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(request("GET", uri, token))
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn app(pool: PgPool) -> Router {
    router_with_auth_config(pool, test_jwks(), AuthConfig::default())
}

async fn seed_tenant(pool: &PgPool, external_key: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', $1, $1) RETURNING id",
    )
    .bind(external_key)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// principal + identity + tenant_member 一条龙。
async fn seed_tenant_member(pool: &PgPool, tenant_id: Uuid, sub: &str) -> Uuid {
    let principal: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', $1, true) RETURNING id",
    )
    .bind(sub)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal_identity (provider, tenant_id, subject, principal_id) \
         VALUES ('lmms', $1, $2, $3)",
    )
    .bind(tenant_id)
    .bind(sub)
    .bind(principal)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) \
         VALUES ($1, $2, 'member')",
    )
    .bind(tenant_id)
    .bind(principal)
    .execute(pool)
    .await
    .unwrap();
    principal
}

async fn seed_project_member(pool: &PgPool, tenant_id: Uuid, project_id: Uuid, principal_id: Uuid) {
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'owner')",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_project(pool: &PgPool, tenant_id: Uuid, code: &str) -> Uuid {
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, 'c') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, $3, $3, 'planning') RETURNING id",
    )
    .bind(tenant_id)
    .bind(customer_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_task(pool: &PgPool, project_id: Uuid, tenant_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
         VALUES ($1, $2, 'ready', $3) RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 造 Run；`extras` 为 (列名, 原始 SQL 片段)（字符串字面量自带引号，时间用
/// `now() - interval` 表达式）——均为测试常量，非用户输入。
async fn seed_run(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    attempt_no: i32,
    status: &str,
    created_at: &str,
    extras: &[(&str, &str)],
) -> Uuid {
    let mut extra_cols = String::new();
    let mut extra_vals = String::new();
    for (column, value) in extras {
        extra_cols.push_str(&format!(", {column}"));
        extra_vals.push_str(&format!(", {value}"));
    }
    sqlx::query_scalar(&format!(
        "INSERT INTO delivery.task_run \
           (project_id, task_id, attempt_no, status, created_at, tenant_id{extra_cols}) \
         VALUES ($1, $2, $3, $4, {created_at}, \
                 (SELECT tenant_id FROM core.project WHERE id = $1){extra_vals}) RETURNING id"
    ))
    .bind(project_id)
    .bind(task_id)
    .bind(attempt_no)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
async fn seed_audit(
    pool: &PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
    actor_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    after_data: Value,
    created_at: &str,
    source_kind: Option<&str>,
) {
    sqlx::query(&format!(
        "INSERT INTO governance.audit_log \
           (project_id, actor_id, action, target_type, target_id, after_data, \
            source_kind, created_at, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, {created_at}, $8)"
    ))
    .bind(project_id)
    .bind(actor_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(after_data)
    .bind(source_kind)
    .bind(tenant_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_provider(
    pool: &PgPool,
    tenant_id: Uuid,
    creator: Uuid,
    name: &str,
    status: &str,
    model: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO ai.team_llm_provider \
           (tenant_id, name, provider_type, endpoint, configuration, status, created_by) \
         VALUES ($1, $2, 'deepseek', 'https://api.example.com', $3, $4, $5) RETURNING id",
    )
    .bind(tenant_id)
    .bind(name)
    .bind(json!({ "model": model }))
    .bind(status)
    .bind(creator)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn bind_policy(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    provider_id: Uuid,
    by: Uuid,
) {
    sqlx::query(
        "INSERT INTO ai.project_ai_model_policy \
           (tenant_id, project_id, provider_id, updated_by) VALUES ($1, $2, $3, $4)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(provider_id)
    .bind(by)
    .execute(pool)
    .await
    .unwrap();
}

// ── Monitor ───────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn monitor_returns_runs_and_events_with_structure(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_tenant_member(&pool, tenant, "alice").await;
    let project = seed_project(&pool, tenant, "p-mon").await;
    seed_project_member(&pool, tenant, project, alice).await;
    let task = seed_task(&pool, project, tenant, "Implement login").await;

    // 三个 Run：created_at DESC 期望顺序 running(新) → failed → queued(旧)。
    let run_running = seed_run(
        &pool,
        project,
        task,
        2,
        "running",
        "now() - interval '10 minutes'",
        &[
            ("stage", "'started'"),
            ("run_kind", "'delivery'"),
            ("executor_kind", "'codex'"),
            ("execution_mode", "'spawn'"),
            ("claimed_at", "now() - interval '5 minutes'"),
        ],
    )
    .await;
    let run_failed = seed_run(
        &pool,
        project,
        task,
        1,
        "failed",
        "now() - interval '2 hours'",
        &[
            ("outcome", "'failed'"),
            ("finished_at", "now() - interval '1 hour'"),
        ],
    )
    .await;
    seed_run(
        &pool,
        project,
        task,
        3,
        "cancelled",
        "now() - interval '3 hours'",
        &[
            ("release_reason", "'dependency_unmet'"),
            ("scheduling_shield", "'human_wait'"),
        ],
    )
    .await;

    // 事件：run_dispatch 三动作 + task_ready 补充动作；一条非 run_dispatch 应被过滤。
    seed_audit(
        &pool,
        project,
        tenant,
        alice,
        "claim_task_run",
        "task_run",
        run_running,
        json!({ "task_id": task, "mode": "Normal", "model": "worker" }),
        "now() - interval '5 minutes'",
        Some("run_dispatch"),
    )
    .await;
    seed_audit(
        &pool,
        project,
        tenant,
        alice,
        "reclaim_task_run",
        "task_run",
        run_failed,
        json!({ "reason": "lease_expired_unstarted" }),
        "now() - interval '40 minutes'",
        Some("run_dispatch"),
    )
    .await;
    seed_audit(
        &pool,
        project,
        tenant,
        alice,
        "skip_task_run",
        "task_run",
        run_failed,
        json!({ "release_reason": "scheduling_shield", "scheduling_shield": "human_wait" }),
        "now() - interval '50 minutes'",
        Some("run_dispatch"),
    )
    .await;
    seed_audit(
        &pool,
        project,
        tenant,
        alice,
        "task_ready_skipped",
        "task",
        task,
        json!({ "reason": "no_executor_available" }),
        "now() - interval '3 hours'",
        Some("run_dispatch"),
    )
    .await;
    seed_audit(
        &pool,
        project,
        tenant,
        alice,
        "task.create",
        "task",
        task,
        json!({}),
        "now() - interval '4 hours'",
        None,
    )
    .await;

    let app = app(pool);
    let token = token_for("alice", 1);
    let (status, body) = call(
        &app,
        &format!("/v1/pm/projects/{project}/ai-team/monitor"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // runs：全字段结构 + created_at DESC 排序
    let runs = body["runs"].as_array().expect("runs array");
    assert_eq!(runs.len(), 3, "{body}");
    assert_eq!(runs[0]["task_title"], json!("Implement login"));
    assert_eq!(runs[0]["status"], json!("running"));
    assert_eq!(runs[0]["stage"], json!("started"));
    assert_eq!(runs[0]["run_kind"], json!("delivery"));
    assert_eq!(runs[0]["attempt_no"], json!(2));
    assert_eq!(runs[0]["executor_kind"], json!("codex"));
    assert_eq!(runs[0]["execution_mode"], json!("spawn"));
    assert_eq!(runs[0]["scheduling_shield"], json!("none"));
    assert!(runs[0]["run_id"].is_string() && runs[0]["task_id"].is_string());
    assert_eq!(runs[0]["run_id"], json!(run_running));
    assert_eq!(runs[1]["status"], json!("failed"));
    assert_eq!(runs[1]["outcome"], json!("failed"));
    // finished 锚：COALESCE(terminated_at, finished_at) 取有值
    assert!(runs[1]["finished_at"].is_string());
    assert_eq!(runs[1]["terminated_at"], json!(null));
    assert_eq!(runs[2]["status"], json!("cancelled"));
    assert_eq!(runs[2]["release_reason"], json!("dependency_unmet"));
    assert_eq!(runs[2]["scheduling_shield"], json!("human_wait"));
    assert_eq!(runs[2]["outcome"], json!(null));

    // events：只收 run_dispatch 五动作，created_at DESC；reason 双写归并
    let events = body["events"].as_array().expect("events array");
    assert_eq!(events.len(), 4, "task.create must be filtered out: {body}");
    assert_eq!(events[0]["action"], json!("claim_task_run"));
    assert_eq!(events[1]["action"], json!("reclaim_task_run"));
    assert_eq!(events[1]["reason"], json!("lease_expired_unstarted"));
    assert_eq!(events[2]["action"], json!("skip_task_run"));
    assert_eq!(events[2]["reason"], json!("scheduling_shield"));
    assert_eq!(events[3]["action"], json!("task_ready_skipped"));
    assert_eq!(events[3]["reason"], json!("no_executor_available"));
    // actor 摘要 = display_name；target 标题两跳可解析
    assert_eq!(events[0]["actor"], json!("alice"));
    assert_eq!(events[0]["target_title"], json!("Implement login"));
    assert_eq!(events[0]["target_id"], json!(run_running));
}

#[sqlx::test(migrations = "../../migrations")]
async fn monitor_empty_project_zero_collections(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_tenant_member(&pool, tenant, "alice").await;
    let project = seed_project(&pool, tenant, "p-empty").await;
    seed_project_member(&pool, tenant, project, alice).await;

    let app = app(pool);
    let token = token_for("alice", 1);
    let (status, body) = call(
        &app,
        &format!("/v1/pm/projects/{project}/ai-team/monitor"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["runs"], json!([]));
    assert_eq!(body["events"], json!([]));
}

#[sqlx::test(migrations = "../../migrations")]
async fn monitor_non_member_and_cross_tenant_404(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_tenant_member(&pool, tenant, "alice").await;
    let project = seed_project(&pool, tenant, "p-guard").await;
    seed_project_member(&pool, tenant, project, alice).await;
    // 同租户非成员
    seed_tenant_member(&pool, tenant, "mallory").await;
    // 跨租户
    let tenant2 = seed_tenant(&pool, "2").await;
    seed_tenant_member(&pool, tenant2, "bob").await;

    let app = app(pool);
    let (status, _) = call(
        &app,
        &format!("/v1/pm/projects/{project}/ai-team/monitor"),
        &token_for("mallory", 1),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &app,
        &format!("/v1/pm/projects/{project}/ai-team/monitor"),
        &token_for("bob", 2),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &app,
        &format!("/v1/pm/projects/{}/ai-team/monitor", Uuid::new_v4()),
        &token_for("alice", 1),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── Overview ──────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn overview_counts_roles_runs_tasks_and_provider(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_tenant_member(&pool, tenant, "alice").await;
    let project = seed_project(&pool, tenant, "p-ov").await;
    seed_project_member(&pool, tenant, project, alice).await;
    let task = seed_task(&pool, project, tenant, "T").await;

    // roles：1 启用 + 1 停用；模板 5（迁移种子）
    let template_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM ai.ai_role_template ORDER BY created_at LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO ai.project_ai_role (tenant_id, project_id, role_id, enabled) \
         VALUES ($1, $2, $3, true), ($1, $2, $4, false)",
    )
    .bind(tenant)
    .bind(project)
    .bind(template_ids[0])
    .bind(template_ids[1])
    .execute(&pool)
    .await
    .unwrap();

    // runs：1 活跃 + 24h 内 succeeded/failed 各 1 + 24h 外 failed 1（不计）
    seed_run(
        &pool,
        project,
        task,
        1,
        "queued",
        "now() - interval '1 hour'",
        &[],
    )
    .await;
    seed_run(
        &pool,
        project,
        task,
        2,
        "succeeded",
        "now() - interval '2 hours'",
        &[
            ("outcome", "'succeeded'"),
            ("finished_at", "now() - interval '2 hours'"),
        ],
    )
    .await;
    seed_run(
        &pool,
        project,
        task,
        3,
        "failed",
        "now() - interval '1 hour'",
        &[
            ("outcome", "'failed'"),
            ("finished_at", "now() - interval '1 hour'"),
        ],
    )
    .await;
    seed_run(
        &pool,
        project,
        task,
        4,
        "failed",
        "now() - interval '30 hours'",
        &[
            ("outcome", "'failed'"),
            ("finished_at", "now() - interval '30 hours'"),
        ],
    )
    .await;

    // tasks：awaiting_decision 1 + blocked 1
    sqlx::query("UPDATE delivery.task SET status = 'awaiting_decision' WHERE id = $1")
        .bind(task)
        .execute(&pool)
        .await
        .unwrap();
    let task2 = seed_task(&pool, project, tenant, "T2").await;
    sqlx::query("UPDATE delivery.task SET status = 'blocked' WHERE id = $1")
        .bind(task2)
        .execute(&pool)
        .await
        .unwrap();

    // provider：项目指定 + enabled + 有模型 → bound=true
    let provider = seed_provider(
        &pool,
        tenant,
        alice,
        "DeepSeek Main",
        "enabled",
        "deepseek-chat",
    )
    .await;
    bind_policy(&pool, tenant, project, provider, alice).await;

    let app = app(pool);
    let token = token_for("alice", 1);
    let (status, body) = call(
        &app,
        &format!("/v1/pm/projects/{project}/ai-team/overview"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["roles"]["enabled"], json!(1));
    assert_eq!(body["roles"]["disabled"], json!(1));
    assert_eq!(body["roles"]["templates"], json!(5));
    assert_eq!(body["runs"]["active"], json!(1));
    assert_eq!(body["runs"]["succeeded_24h"], json!(1));
    assert_eq!(body["runs"]["failed_24h"], json!(1));
    assert_eq!(body["tasks"]["awaiting_decision"], json!(1));
    assert_eq!(body["tasks"]["blocked"], json!(1));
    assert_eq!(body["provider"]["bound"], json!(true));
    assert_eq!(body["provider"]["provider_name"], json!("DeepSeek Main"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn overview_empty_project_zero_and_unbound(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_tenant_member(&pool, tenant, "alice").await;
    let project = seed_project(&pool, tenant, "p-ov-empty").await;
    seed_project_member(&pool, tenant, project, alice).await;

    let app = app(pool);
    let token = token_for("alice", 1);
    let (status, body) = call(
        &app,
        &format!("/v1/pm/projects/{project}/ai-team/overview"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["roles"],
        json!({ "enabled": 0, "disabled": 0, "templates": 5 })
    );
    assert_eq!(
        body["runs"],
        json!({ "active": 0, "succeeded_24h": 0, "failed_24h": 0 })
    );
    assert_eq!(
        body["tasks"],
        json!({ "awaiting_decision": 0, "blocked": 0 })
    );
    // 无任何 Provider → 未绑定且无名字
    assert_eq!(
        body["provider"],
        json!({ "bound": false, "provider_name": null })
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn overview_provider_inheritance_and_strict_designation(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_tenant_member(&pool, tenant, "alice").await;
    let project = seed_project(&pool, tenant, "p-ov-prov").await;
    seed_project_member(&pool, tenant, project, alice).await;

    // 未指定时的候选：enabled 但无模型 / disabled 有模型 / enabled+有模型（创建序第三个）
    seed_provider(&pool, tenant, alice, "No Model", "enabled", "").await;
    let disabled_one = seed_provider(&pool, tenant, alice, "Disabled One", "disabled", "m").await;
    seed_provider(&pool, tenant, alice, "Inherited One", "enabled", "m1").await;

    let app = app(pool.clone());
    let token = token_for("alice", 1);

    // 场景 1：未指定 → 继承 Team 首个 enabled+有模型（bound=true + Inherited One）
    let (status, body) = call(
        &app,
        &format!("/v1/pm/projects/{project}/ai-team/overview"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["provider"]["bound"], json!(true));
    assert_eq!(body["provider"]["provider_name"], json!("Inherited One"));

    // 场景 2：指定但 Provider 已停用 → 严格不回退，bound=false 且保留名字
    bind_policy(&pool, tenant, project, disabled_one, alice).await;
    let (status, body) = call(
        &app,
        &format!("/v1/pm/projects/{project}/ai-team/overview"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["provider"]["bound"], json!(false));
    assert_eq!(body["provider"]["provider_name"], json!("Disabled One"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn overview_non_member_404(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_tenant_member(&pool, tenant, "alice").await;
    let project = seed_project(&pool, tenant, "p-ov-guard").await;
    seed_project_member(&pool, tenant, project, alice).await;
    seed_tenant_member(&pool, tenant, "mallory").await;

    let tenant2 = seed_tenant(&pool, "2").await;
    seed_tenant_member(&pool, tenant2, "bob").await;

    let app = app(pool);
    let (status, _) = call(
        &app,
        &format!("/v1/pm/projects/{project}/ai-team/overview"),
        &token_for("mallory", 1),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &app,
        &format!("/v1/pm/projects/{project}/ai-team/overview"),
        &token_for("bob", 2),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── R3（B01-01）：AI 就绪探针 ───────────────────────────────────────────

/// 探针 = DB 四项数据条件（provider/catalog/team_wide 授权/general 策略）：
/// 默认部署全零 → ready=false；四项齐备 → ready=true；跨租户项目 404。
#[sqlx::test(migrations = "../../migrations")]
async fn ai_readiness_reflects_llm_data_plane(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_tenant_member(&pool, tenant, "alice").await;
    let project = seed_project(&pool, tenant, "RDY").await;
    seed_project_member(&pool, tenant, project, alice).await;
    let app = app(pool.clone());
    let alice_token = token_for("alice", 1);

    // ① 全零：默认部署事实（不伪造就绪）。
    let (status, body) = call(
        &app,
        &format!("/v1/pm/projects/{project}/ai-readiness"),
        &alice_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["provider_enabled_count"], 0);
    assert_eq!(body["catalog_model_enabled_count"], 0);
    assert_eq!(body["team_wide_authorization_count"], 0);
    assert_eq!(body["general_policy_set"], false);
    assert_eq!(body["ready"], false);

    // ② 四项齐备（直接种子数据面；openai_compatible 允许无凭证）。
    let provider_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.team_llm_provider \
         (tenant_id, name, provider_type, endpoint, status, created_by) \
         VALUES ($1, 'rdy-provider', 'openai_compatible', 'http://127.0.0.1:9', 'enabled', $2) \
         RETURNING id",
    )
    .bind(tenant)
    .bind(alice)
    .fetch_one(&pool)
    .await
    .unwrap();
    let model_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.team_llm_model \
         (tenant_id, provider_id, model_id, display_name, enabled, created_by) \
         VALUES ($1, $2, 'test-model', 'Test Model', true, $3) RETURNING id",
    )
    .bind(tenant)
    .bind(provider_id)
    .bind(alice)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai.team_llm_model_authorization (tenant_id, model_id, state, granted_by) \
         VALUES ($1, $2, 'team_wide', $3)",
    )
    .bind(tenant)
    .bind(model_id)
    .bind(alice)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai.team_llm_model_policy \
         (tenant_id, role_key, model_id, updated_by) \
         VALUES ($1, 'general', $2, $3)",
    )
    .bind(tenant)
    .bind(model_id)
    .bind(alice)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = call(
        &app,
        &format!("/v1/pm/projects/{project}/ai-readiness"),
        &alice_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["provider_enabled_count"], 1);
    assert_eq!(body["catalog_model_enabled_count"], 1);
    assert_eq!(body["team_wide_authorization_count"], 1);
    assert_eq!(body["general_policy_set"], true);
    assert_eq!(body["ready"], true);

    // ③ 跨租户项目：404（不泄露存在性）。
    let tenant2 = seed_tenant(&pool, "2").await;
    let bob = seed_tenant_member(&pool, tenant2, "bob").await;
    let project2 = seed_project(&pool, tenant2, "RDY2").await;
    seed_project_member(&pool, tenant2, project2, bob).await;
    let (status, _) = call(
        &app,
        &format!("/v1/pm/projects/{project2}/ai-readiness"),
        &alice_token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
