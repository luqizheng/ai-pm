//! L2 安全测试基础设施（阶段 B，交付物 05 §2 / §3 三分类口径）。
//!
//! 本模块只服务测试：把 `security_p0_regression.rs` 的临时夹具固化为可复用件，
//! 供阶段 C 的 V1-T01~T12、V2-T01~T10 与 P0 回归组共用。
//!
//! 组成（任务书「阶段 B」四条）：
//! 1. **双租户夹具**——tenantA = `SYSTEM_TENANT`（lmms:1 identity 映射），
//!    tenantB 独立 tenant（lmms:2），各自 bootstrap 一个项目与 owner 真人；
//! 2. **凭据签发器**——Human interactive JWT / Service JWT / Agent(dev_agent)
//!    principal JWT / cpk 设备凭据（devenv.device + device_credential，三码）/
//!    未认证；
//! 3. **断言库**——写：HTTP 4xx + DB 快照无变化 + 无副作用；
//!    读：HTTP 拒绝 + 响应逐字段不含目标租户数据；
//! 4. **FakeModel Fixture 接口预留**（阶段 D 业务链使用；本阶段仅版本化占位）。
//!
//! 纪律：测试数据全部 `L2-*`/`test` 前缀；只读审计——本模块不修改业务代码。

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::routes::router_with_auth;
use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::device_credential;
use pm_domain::SYSTEM_TENANT;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const PRIVATE_PEM: &str = include_str!("../keys/private.pem");
const PUBLIC_PEM: &str = include_str!("../keys/public.pem");

// ───────────────────────────────────────────────────────────── 凭据签发器

pub fn test_jwks() -> Arc<JWKSCache> {
    Arc::new(JWKSCache::from_pem(PUBLIC_PEM).expect("test public key must parse"))
}

pub fn sign_claims(claims: Value) -> String {
    sign_token(PRIVATE_PEM, &claims, Duration::from_secs(300)).expect("test token must sign")
}

/// Human interactive JWT（`aud` 存在 → auth_mode=interactive）。
pub fn sign_human(sub: &str, tid: i64) -> String {
    sign_claims(json!({
        "sub": sub,
        "tid": tid,
        "username": sub,
        "roles": [],
        "permissions": [],
        "aud": "coder-micro-services",
    }))
}

/// Service JWT：`roles` 含 `service` → `resolve_principal` 恒 403（IAM_PERM_3003）。
pub fn sign_service(sub: &str, tid: i64) -> String {
    sign_claims(json!({
        "sub": sub,
        "tid": tid,
        "username": sub,
        "roles": ["service"],
        "permissions": [],
        "aud": "coder-micro-services",
    }))
}

/// API Key 形态 JWT（`purpose=api_key`）：业务可达，但 `ensure_trusted_context`
/// 拒绝确认类动作。
pub fn sign_api_key(sub: &str, tid: i64, permissions: &[&str]) -> String {
    sign_claims(json!({
        "sub": sub,
        "tid": tid,
        "username": sub,
        "roles": [],
        "permissions": permissions,
        "purpose": "api_key",
        "client_id": "l2-api-key",
    }))
}

// ───────────────────────────────────────────────────────────── 双租户夹具

/// 一个租户的完整上下文：项目 + owner 真人 + 可用 JWT + 共享 router。
#[derive(Clone)]
pub struct TenantCtx {
    pub tenant_id: Uuid,
    pub tid: i64,
    pub project_id: Uuid,
    pub human_id: Uuid,
    /// dev_agent（developer）principal——Agent 主体测试用。
    pub agent_id: Uuid,
    pub token: String,
    pub app: Router,
}

/// 双租户夹具：A = SYSTEM_TENANT（lmms:1），B = 独立 tenant（lmms:2）。
pub async fn dual_tenants(pool: &PgPool) -> (TenantCtx, TenantCtx) {
    let app = router_with_auth(pool.clone(), test_jwks());
    let a = seed_tenant_a(pool, app.clone()).await;
    let b = seed_tenant_b(pool, app.clone()).await;
    (a, b)
}

async fn insert_identity(pool: &PgPool, tenant_id: Uuid, subject: &str, principal_id: Uuid) {
    sqlx::query(
        "INSERT INTO core.principal_identity (provider, tenant_id, subject, principal_id) \
         VALUES ('lmms', $1, $2, $3)",
    )
    .bind(tenant_id)
    .bind(subject)
    .bind(principal_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_tenant_a(pool: &PgPool, app: Router) -> TenantCtx {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: "L2-A1".to_string(),
            project_name: "l2 tenant a".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    // 迁移只种 id=0x10 的 system 行；改标 lmms:1，使 JWT tid=1 解析到项目租户。
    sqlx::query("UPDATE core.tenant SET provider = 'lmms', external_key = '1' WHERE id = $1")
        .bind(SYSTEM_TENANT)
        .execute(pool)
        .await
        .unwrap();
    insert_identity(
        pool,
        SYSTEM_TENANT,
        &boot.human_id.to_string(),
        boot.human_id,
    )
    .await;
    TenantCtx {
        tenant_id: SYSTEM_TENANT,
        tid: 1,
        project_id: boot.project_id,
        human_id: boot.human_id,
        agent_id: boot.dev_agent_id,
        token: sign_human(&boot.human_id.to_string(), 1),
        app,
    }
}

async fn seed_tenant_b(pool: &PgPool, app: Router) -> TenantCtx {
    let tenant_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', '2', 'lmms:2-l2') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: "L2-B1".to_string(),
            project_name: "l2 tenant b".to_string(),
            tenant_id,
        },
    )
    .await
    .unwrap();
    insert_identity(pool, tenant_id, &boot.human_id.to_string(), boot.human_id).await;
    TenantCtx {
        tenant_id,
        tid: 2,
        project_id: boot.project_id,
        human_id: boot.human_id,
        agent_id: boot.dev_agent_id,
        token: sign_human(&boot.human_id.to_string(), 2),
        app,
    }
}

/// Agent（dev_agent）主体 JWT：身份映射到 bootstrap 造的 developer principal。
/// 该 principal 本身是 `project_member`（role=developer）——正是 V1-T02 要验证的
/// 「Agent 是否被主体门拒绝」的输入。
pub async fn agent_token(pool: &PgPool, tenant: &TenantCtx) -> String {
    insert_identity(
        pool,
        tenant.tenant_id,
        &tenant.agent_id.to_string(),
        tenant.agent_id,
    )
    .await;
    sign_human(&tenant.agent_id.to_string(), tenant.tid)
}

/// 同租户、无项目成员资格的真人（V2-T09 用）。
pub async fn tenant_member_without_project(
    pool: &PgPool,
    tenant: &TenantCtx,
    sub: &str,
) -> (Uuid, String) {
    let principal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', $1, true) RETURNING id",
    )
    .bind(sub)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(tenant.tenant_id)
    .bind(principal_id)
    .execute(pool)
    .await
    .unwrap();
    insert_identity(pool, tenant.tenant_id, sub, principal_id).await;
    (principal_id, sign_human(sub, tenant.tid))
}

// ───────────────────────────────────────────────────────────── cpk 设备凭据

pub struct CpkCredential {
    pub device_id: Uuid,
    pub plaintext: String,
}

/// 签发一台 owner 的设备凭据（granted_permissions 默认三码）。
pub async fn issue_cpk(pool: &PgPool, tenant_id: Uuid, owner: Uuid) -> CpkCredential {
    // fk_device_tenant_owner：owner 必须是租户成员。
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) \
         VALUES ($1, $2, 'owner') ON CONFLICT (tenant_id, principal_id) DO NOTHING",
    )
    .bind(tenant_id)
    .bind(owner)
    .execute(pool)
    .await
    .unwrap();
    let device_id: Uuid = sqlx::query_scalar(
        "INSERT INTO devenv.device (id, tenant_id, owner_principal_id, name, platform, status) \
         VALUES ($1, $2, $3, 'l2-runner', 'linux', 'active') RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(tenant_id)
    .bind(owner)
    .fetch_one(pool)
    .await
    .unwrap();
    let issued = device_credential::issue_device_credential(
        pool,
        tenant_id,
        device_id,
        &AuditActor::principal(owner),
    )
    .await
    .expect("cpk issue must succeed");
    CpkCredential {
        device_id,
        plaintext: issued.plaintext,
    }
}

/// 撤销设备凭据（R3「凭据撤销后任意调用 → 401」）。
pub async fn revoke_cpk(pool: &PgPool, tenant_id: Uuid, device_id: Uuid, owner: Uuid) {
    device_credential::revoke_device_credential(
        pool,
        tenant_id,
        device_id,
        &AuditActor::principal(owner),
    )
    .await
    .expect("revoke must succeed");
}

// ───────────────────────────────────────────────────────────── 请求构造

pub fn req_json(
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    builder
        .body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
        .unwrap()
}

/// 带附加头的 JSON 请求（V1-T05/T10/T11 的伪造声明字段/头用）。
pub fn req_json_headers(
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
    extra_headers: &[(&str, &str)],
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    for (name, value) in extra_headers {
        builder = builder.header(*name, *value);
    }
    builder
        .body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
        .unwrap()
}

/// `/mcp` 请求（Host 固定 loopback，命中 McpHosts::default 白名单）。
pub fn mcp_request(token: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("host", "127.0.0.1:8090")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(Body::from(body.to_string()))
        .unwrap()
}

pub fn mcp_tool_call(token: &str, tool: &str, arguments: Value) -> Request<Body> {
    mcp_request(
        token,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        }),
    )
}

pub async fn status_body(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

pub async fn status_text(response: axum::response::Response) -> (StatusCode, String) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

// ───────────────────────────────────────────────────────────── DB 快照断言库

/// 项目级 DB 快照：覆盖 P0 与 V-1/V-2 写路径可能触碰的全部表。
/// 拒绝请求必须满足「HTTP 4xx + 快照逐字相等」。
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSnapshot {
    pub cognition_items: Value,
    pub cognition_runs: Value,
    pub knowledge: Value,
    pub baseline_proposals: Value,
    pub baseline_items: Value,
    pub tasks: Value,
    pub approvals: Value,
    pub proposals: Value,
    pub inbound_events: Value,
    pub jobs: Value,
    pub interventions: Value,
    pub notifications: Value,
    pub audit_logs: Value,
    pub ai_config: Value,
}

impl ProjectSnapshot {
    pub async fn capture(pool: &PgPool, project_id: Uuid) -> Self {
        Self {
            cognition_items: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb) FROM ( \
                   SELECT id, state, decided_by, decided_at, decision_note, knowledge_id, content \
                     FROM ai.cognition_item WHERE project_id = $1) x",
                project_id,
            )
            .await,
            cognition_runs: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb) FROM ( \
                   SELECT id, status, item_count, confirmed_count \
                     FROM ai.cognition_run WHERE project_id = $1) x",
                project_id,
            )
            .await,
            knowledge: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb) FROM ( \
                   SELECT id, status, content, confirmed_by, version \
                     FROM ai.project_knowledge WHERE project_id = $1) x",
                project_id,
            )
            .await,
            tasks: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb) FROM ( \
                   SELECT id, status, revision FROM delivery.task WHERE project_id = $1) x",
                project_id,
            )
            .await,
            baseline_proposals: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb) FROM ( \
                   SELECT id, version, status, confirmed_by FROM ai.baseline_proposal \
                    WHERE project_id = $1) x",
                project_id,
            )
            .await,
            baseline_items: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb) FROM ( \
                   SELECT id, proposal_id, state, decided_by, decision_note, content \
                     FROM ai.baseline_item WHERE project_id = $1) x",
                project_id,
            )
            .await,
            approvals: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb) FROM ( \
                   SELECT id, status, decided_by FROM governance.approval_request \
                    WHERE project_id = $1) x",
                project_id,
            )
            .await,
            proposals: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb) FROM ( \
                   SELECT id, status FROM ai.change_proposal WHERE project_id = $1) x",
                project_id,
            )
            .await,
            inbound_events: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb) FROM ( \
                   SELECT id, event_type, actor_id FROM integration.inbound_event \
                    WHERE project_id = $1) x",
                project_id,
            )
            .await,
            jobs: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb) FROM ( \
                   SELECT id, job_type, status FROM integration.job WHERE project_id = $1) x",
                project_id,
            )
            .await,
            interventions: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb) FROM ( \
                   SELECT id, intervention_type, actor_id FROM governance.human_intervention \
                    WHERE project_id = $1) x",
                project_id,
            )
            .await,
            notifications: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb) FROM ( \
                   SELECT id, kind, payload FROM integration.notification WHERE project_id = $1) x",
                project_id,
            )
            .await,
            audit_logs: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x) ORDER BY x.id), '[]'::jsonb) FROM ( \
                   SELECT id, action, actor_id, target_id FROM governance.audit_log \
                    WHERE project_id = $1) x",
                project_id,
            )
            .await,
            ai_config: rows(
                pool,
                "SELECT COALESCE(jsonb_agg(to_jsonb(x)), '[]'::jsonb) FROM ( \
                   SELECT automation_mode, enabled FROM ai.project_ai_config \
                    WHERE project_id = $1) x",
                project_id,
            )
            .await,
        }
    }
}

async fn rows(pool: &PgPool, sql: &str, project_id: Uuid) -> Value {
    sqlx::query_scalar::<_, Value>(sql)
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// 写操作跨租户拒绝断言：状态码命中 `expected` **且** 目标项目快照逐字不变。
pub async fn expect_cross_tenant_denied(
    pool: &PgPool,
    target_project: Uuid,
    before: &ProjectSnapshot,
    response: axum::response::Response,
    expected: &[StatusCode],
) -> Value {
    let (status, body) = status_body(response).await;
    assert!(
        expected.contains(&status),
        "expected a denial status, got {status}: {body}"
    );
    let after = ProjectSnapshot::capture(pool, target_project).await;
    assert_eq!(
        &after, before,
        "a denied write must not change the target tenant's DB state"
    );
    body
}

/// 读操作跨租户拒绝断言：状态码命中 `expected` **且** 响应体不含任何目标租户标记。
pub async fn expect_cross_tenant_read_denied(
    response: axum::response::Response,
    expected: &[StatusCode],
    forbidden_markers: &[&str],
) -> String {
    let (status, text) = status_text(response).await;
    assert!(
        expected.contains(&status),
        "expected a denial status, got {status}: {text}"
    );
    for marker in forbidden_markers {
        assert!(
            !text.contains(marker),
            "cross-tenant payload `{marker}` must not leak in the rejection body"
        );
    }
    text
}

// ───────────────────────────────────────────────────────────── 业务种子

/// 启动一次认知 Run 并返回派生条目 id 列表（走真实 HTTP 入口）。
pub async fn start_cognition_items(app: &Router, token: &str, project_id: Uuid) -> Vec<Uuid> {
    let response = app
        .clone()
        .oneshot(req_json(
            "POST",
            &format!("/v1/pm/projects/{project_id}/cognition/runs"),
            Some(token),
            Some(json!({})),
        ))
        .await
        .unwrap();
    let (status, body) = status_body(response).await;
    assert_eq!(status, StatusCode::OK, "start run failed: {body}");

    let response = app
        .clone()
        .oneshot(req_json(
            "GET",
            &format!("/v1/pm/projects/{project_id}/cognition"),
            Some(token),
            None,
        ))
        .await
        .unwrap();
    let (status, body) = status_body(response).await;
    assert_eq!(status, StatusCode::OK, "cognition overview failed: {body}");
    body["items"]
        .as_array()
        .expect("items array")
        .iter()
        .map(|item| Uuid::parse_str(item["id"].as_str().unwrap()).unwrap())
        .collect()
}

/// 造一条任务（V2-T07 批量 / V1-T12 MCP 成员校验收用）。
pub async fn seed_task(pool: &PgPool, project_id: Uuid, title: &str, status: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id) \
         VALUES ($1, $2, $3, 1, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 读响应中的 MCP `isError` 标志（JSON-RPC result）。
pub fn mcp_is_error(body: &Value) -> bool {
    body["result"]["isError"].as_bool().unwrap_or(false)
}

/// MCP 工具结果的文本载荷（`result.content[0].text`）。
pub fn mcp_text(body: &Value) -> String {
    body["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

// ───────────────────────────────────────────────────────────── FakeModel 预留

/// 阶段 D 业务链（交付物 05 §2.3）的确定性模型夹具接口预留。
///
/// 本阶段只固定版本号与字段形状——业务链执行时以该版本为 fixture 版本锚点，
/// 保证输入/输出可复现（失败可重放）。不做任何网络调用。
pub mod fake_model {
    /// fixture 版本锚点（阶段 D 变更须递增并同步 D04）。
    pub const FIXTURE_VERSION: &str = "fake-model-v1";

    #[derive(Debug, Clone)]
    pub struct FakeModelFixture {
        pub version: &'static str,
        pub model_name: &'static str,
        pub provider: &'static str,
    }

    pub fn v1() -> FakeModelFixture {
        FakeModelFixture {
            version: FIXTURE_VERSION,
            model_name: "fake-model",
            provider: "test",
        }
    }
}
