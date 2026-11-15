//! REST routes: `POST /api/v1/events` (Task 9) over the idempotent ingestion
//! service, plus the Task 15 supervision endpoints over the three human
//! powers (veto / red-line decision / freeze-unfreeze) and the notification
//! feed.
//!
//! `POST /api/v1/events` response mapping:
//!
//! | outcome                | status | body                                                        |
//! |------------------------|--------|-------------------------------------------------------------|
//! | `Accepted`             | 202    | `{event_id, processing_status: "queued"}`                   |
//! | `Duplicate`            | 200    | `{event_id, status, processing_status: "duplicate"}`        |
//! | `IdempotencyConflict`  | 409    | `{error: "idempotency_conflict", event_id}`                 |
//! | `UnknownProject`       | 404    | `{error: "unknown_project"}`                                |
//! | `Db`                   | 500    | `{error: "internal"}` (details only in the log)             |
//!
//! Supervision response mapping (Task 15):
//!
//! | call     | outcome              | status | body                                       |
//! |----------|----------------------|--------|--------------------------------------------|
//! | veto     | `RolledBack`         | 200    | `{outcome: "rolled_back", new_audit_id}`   |
//! | veto     | `AlreadyCompensated` | 200    | `{outcome: "already_compensated", already}`|
//! | veto     | `ProposalClosed`     | 200    | `{outcome: "proposal_closed"}`             |
//! | veto     | `RevisionConflict`   | 409    | `{error: "rollback_failed", ...}`          |
//! | veto     | `NotFound`           | 404    | `{error: "proposal_not_found"}`            |
//! | decision | `Executed`/`Rejected`| 200    | `{outcome: "executed"/"rejected", ...}`    |
//! | decision | `AlreadyDecided`     | 200    | `{outcome: "already_decided", already: true}` |
//! | decision | `NotFound`           | 404    | `{error: "approval_not_found"}`            |
//! | freeze   | ok                   | 200    | `{outcome: "frozen"}`                      |
//! | unfreeze | ok                   | 200    | `{outcome: "unfrozen"}`                    |
//! | freeze   | unknown config       | 404    | `{error: "unknown_project"}`               |
//! | feed     | ok                   | 200    | `[NotificationRow...]` newest-first        |

use std::sync::Arc;

use crate::support::require_project_visible;
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use pm_application::supervision::{
    decide_approval, freeze, notifications_feed, unfreeze, veto, DecideRequest, DecisionOutcome,
    SupervisionError, VetoOutcome, VetoRequest,
};
use pm_application::{CommandError, PgCommandExecutor, PolicyEngine};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::support::repos;
use pm_application::repo::GuardQueryRepo;

/// Shared handler state: the pool and the process-wide command executor. The
/// executor's red-line set comes from the one canonical policy file — the
/// single source of truth, loaded once per process here.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) pool: PgPool,
    pub(crate) executor: Arc<PgCommandExecutor>,
    pub(crate) audit_feed: crate::audit_stream::AuditFeed,
    /// S02-I2：需求投影的知识库根（submit 绑定点写 Git）。测试经
    /// [`router_with_feed_and_knowledge`] 注入独立临时根，避免并行用例的
    /// Git index 竞态；生产 = `KnowledgeRoot::from_env()`。
    pub(crate) knowledge: pm_application::knowledge::KnowledgeRoot,
    /// 文件代理（2026-11-07 file-service 收口配套）。`None` = 未配置
    /// bootstrap token（裸机 dev），/v1/file/* 返回 503。
    pub(crate) file: crate::files::FileProxyHandle,
    /// Document Processing 存储（0016/TD-02）。`None` = 未配置 file-service，
    /// /v1/document-processing/* 返回 503。
    pub(crate) docproc_store:
        Option<std::sync::Arc<dyn pm_application::docproc_service::ArtifactStore>>,
    /// DK-A：对话载体判定（`AI_PM_CONVERSATION_ROUTER` + 意图分类端口）。
    pub(crate) conversation_routing: ConversationRouting,
    /// Visualization Relation Registry（编译期嵌入，版本随二进制——审计可追溯）。
    ///
    /// 关系未 active 即**不可投影**（专家裁决：未会签 ≠ 默认允许）。
    pub(crate) viz_registry: Arc<pm_domain::visualization::RelationRegistry>,
}

/// DK-A 对话载体判定装配：开关 + 意图分类器。生产 = [`ConversationRouting::from_env`]
/// （LLM 分类器，开关读环境、默认 off）；测试经
/// [`router_with_auth_config_and_routing`] 注入固定开关与夹具分类器。
#[derive(Clone)]
pub struct ConversationRouting {
    pub mode: pm_application::conversation_router::RouterMode,
    pub classifier: Arc<dyn pm_application::conversation_router::IntentClassifier>,
}

impl ConversationRouting {
    pub fn from_env(pool: &PgPool) -> Self {
        let mode = pm_application::conversation_router::RouterMode::from_env();
        tracing::info!(
            ?mode,
            "conversation router mode selected (AI_PM_CONVERSATION_ROUTER)"
        );
        Self {
            mode,
            classifier: Arc::new(pm_ai::LlmIntentClassifier::new(pool.clone())),
        }
    }

    /// 开关 off + 无分类器（只有规则 1/2）。
    pub fn off() -> Self {
        Self {
            mode: pm_application::conversation_router::RouterMode::Off,
            classifier: Arc::new(pm_application::conversation_router::DisabledClassifier),
        }
    }
}

/// The pm-api route table, stateful over the shared connection pool and
/// executor. Used by `main.rs` and by `tests/routes.rs` so tests exercise the
/// real routes. The audit feed is process-local here; `main.rs` uses
/// [`router_with_feed`] so it can also spawn the PG listener.
pub fn router(pool: PgPool) -> Router {
    router_with_feed_and_hosts(
        pool,
        crate::audit_stream::AuditFeed::new(),
        crate::mcp_hosts::McpHosts::default(),
    )
}

/// Same route table over an explicit audit feed. `main.rs` calls this with the
/// feed whose PG listener it owns. The `off` mode (local development) keeps the
/// MCP service unguarded — permissions only exist once bearer auth is on.
///
/// The `/mcp` Host allow-list comes from the environment (T-081): cross-machine
/// access is a deployment decision, not a code change.
pub fn router_with_feed(pool: PgPool, audit_feed: crate::audit_stream::AuditFeed) -> Router {
    router_with_feed_and_hosts(pool, audit_feed, crate::mcp_hosts::McpHosts::from_env())
}

/// [`router_with_feed`] with an explicit `/mcp` Host allow-list (T-081).
pub fn router_with_feed_and_hosts(
    pool: PgPool,
    audit_feed: crate::audit_stream::AuditFeed,
    mcp_hosts: crate::mcp_hosts::McpHosts,
) -> Router {
    router_with_feed_and_knowledge(
        pool,
        audit_feed,
        mcp_hosts,
        pm_application::knowledge::KnowledgeRoot::from_env(),
    )
}

/// [`router_with_feed_and_hosts`] with an explicit knowledge root（S02-I2）：
/// 测试注入每用例独立临时根。
pub fn router_with_feed_and_knowledge(
    pool: PgPool,
    audit_feed: crate::audit_stream::AuditFeed,
    mcp_hosts: crate::mcp_hosts::McpHosts,
    knowledge: pm_application::knowledge::KnowledgeRoot,
) -> Router {
    // `off` 模式不再拆掉身份门：注入固定的 `dev-off` principal
    // （写请求时落库），handlers 与审计仍有 actor（v1 §6 / C42）。
    let auth = crate::auth::AuthState {
        jwks: None,
        pool: pool.clone(),
        config: crate::auth::AuthConfig::default(),
    };
    build_router(pool, audit_feed, false, mcp_hosts, knowledge, None).layer(
        axum::middleware::from_fn_with_state(auth, crate::auth::inject_dev_principal),
    )
}

fn build_router(
    pool: PgPool,
    audit_feed: crate::audit_stream::AuditFeed,
    guard_mcp: bool,
    mcp_hosts: crate::mcp_hosts::McpHosts,
    knowledge: pm_application::knowledge::KnowledgeRoot,
    conversation_routing: Option<ConversationRouting>,
) -> Router {
    let policy = Arc::new(
        PolicyEngine::from_yaml(include_str!("../../../policies/ai-pm-ai-lead-v2.yaml"))
            .expect("canonical policy yaml must parse"),
    );
    let executor = Arc::new(PgCommandExecutor::new(pool.clone(), policy));
    let conversation_routing =
        conversation_routing.unwrap_or_else(|| ConversationRouting::from_env(&pool));
    // 与 policy 同款：编译期嵌入 + 启动期解析失败即 panic（fail-fast，
    // 词表坏了要在启动时炸，而不是在出图时静默放过）。
    let viz_registry = Arc::new(
        pm_application::visualization::registry_loader::parse_registry(include_str!(
            "../../../policies/viz-relation-registry.yaml"
        ))
        .expect("viz relation registry yaml must parse"),
    );
    let state = AppState {
        pool,
        executor,
        audit_feed,
        knowledge,
        file: crate::files::FileProxy::from_env(),
        docproc_store: pm_infrastructure::docproc_store::FileServiceStore::from_env("pm-api")
            .map(|s| s as std::sync::Arc<dyn pm_application::docproc_service::ArtifactStore>),
        conversation_routing,
        viz_registry,
    };
    // The MCP service clones the pool inside its own state factory; it cannot
    // share `state` (different service type), hence a separate handle.
    let mcp_pool = state.pool.clone();
    let mcp_service =
        pm_mcp::PmMcpServer::streamable_http_service(mcp_pool, mcp_hosts.extra_allowed);
    // `/mcp` is permission-gated when auth is on: the outer layer verifies
    // claims, this inner layer demands PM_MCP_READ plus the tool's write
    // permission. In `off` mode there are no claims, so the gate is skipped.
    //
    // T-088：`attach_device_claim` 是 `X-AI-PM-Device-Id` 的**唯一**校验点，挂在
    // 最内层（紧贴 rmcp）——权限拒绝的请求不触发设备查询；两种模式都挂（off 模式
    // 仍有 dev-off principal）。它只注入 `DeviceClaim`，绝不改变鉴权结果。
    let device_layer = axum::middleware::from_fn_with_state(
        state.clone(),
        crate::device_claim::attach_device_claim,
    );
    let mcp_router: Router<AppState> = if guard_mcp {
        Router::new()
            .fallback_service(mcp_service)
            .layer(device_layer)
            .layer(axum::middleware::from_fn(
                crate::auth::require_mcp_permission,
            ))
    } else {
        Router::new()
            .fallback_service(mcp_service)
            .layer(device_layer)
    };
    Router::new()
        .route("/health", get(health_handler))
        // 首管理员初始化探测（空库强制注册）：公开，状态来自启动检查 + 记号文件。
        .route("/v1/pm/setup/status", get(crate::setup::status_handler))
        .route("/api/v1/events", post(submit_event_handler))
        .route("/api/v1/supervision/veto", post(veto_handler))
        .route(
            "/api/v1/supervision/approvals/{id}/decision",
            post(decide_handler),
        )
        .route("/api/v1/supervision/freeze", post(freeze_handler))
        .route("/api/v1/supervision/unfreeze", post(unfreeze_handler))
        .route(
            "/api/v1/projects/{project_id}/notifications",
            get(notifications_handler),
        )
        // `/v1/pm/*` 业务端点（Slice A，前端经典页契约；无 /api 前缀——vite
        // 代理剥掉 /api 后直达这里）。
        .merge(crate::pm_lists::router())
        .merge(crate::customers::router())
        // 主体自查与管理端激活/停用（T-021a）。
        .merge(crate::principals::router())
        // 租户成员与角色管理（T-021b）：撤权即时生效的唯一写入路径。
        .merge(crate::tenant_members::router())
        // 项目成员授权（S01-W13）：core.project_member 的唯一写入路径。
        .merge(crate::project_members::router())
        // 归档与 Run 取消（S01 · I1-D/P2）：归档事务收口 + cancel_run 外部入口。
        .merge(crate::project_lifecycle::router())
        // 项目总控台聚合（S01 · I5-C）：六数据块纯读模型，按块降级。
        .merge(crate::command_center::router())
        // 任务执行 Run（AI 执行中心数据源；见 workspace-execution-artifacts plan）。
        .merge(crate::pm_runs::router())
        // 交付清单（Slice D4：基线 → 结果 commit → 证据；只读）。
        .merge(crate::pm_delivery::router())
        // Review 审批（Slice D5：pending 审批列表 + 裁决单事务；T-043）。
        .merge(crate::pm_reviews::router())
        // 交付验收工作台（T-070）：项目级聚合读模型 + 任务级验收证据（findings /
        // 逐条裁决 / 审批历史）；全部只读，验收动作复用上面的 decision 端点。
        .merge(crate::pm_acceptance::router())
        // 产物（需求修订）列表——AI 产物验收中心数据源。
        .merge(crate::pm_artifacts::router())
        .merge(crate::viz_handlers::router())
        // Team LLM Provider 管理面（0041；owner/admin 守卫，取代旧全局 /v1/pm/llm/*）。
        .merge(crate::team_llm::router())
        // Team 模型目录/授权/Role 策略（Phase 3A；owner/admin 守卫）。
        .merge(crate::team_llm_catalog::router())
        // Team Onboarding（0044）：建团认领 / 团队档案 / 邮箱邀请 / 邀请链接。
        .merge(crate::team_onboarding::router())
        // 项目 ↔ 代码仓库关系 + 项目级 SSH 凭据（P0 CRUD）。
        .merge(crate::pm_repositories::router())
        // D-5 · 仓库分析报告：列表 / 详情 / 关闭（知识审阅报告页签 + review-center 卡片）。
        .merge(crate::analysis_reports::router())
        // 冷启动导入：启动 / 轮询 / 取消 / apply（提案链）。
        .merge(crate::pm_imports::router())
        // 项目认知访谈（T-084）：DiscoverySession 读写 + 发现卡片确认（source=WEB）。
        .merge(crate::pm_discovery::router())
        // Task 前置依赖（T-024）：建边 / 删边 / 上下游查询。
        .merge(crate::task_dependencies::router())
        .merge(crate::pm_tasks::router())
        .merge(crate::pm_execution::router())
        .merge(crate::pm_requirement_read::router())
        .merge(crate::task_repositories::router())
        // 项目 AI Role 目录与启停（V2 D-1=A）：模板合并视图 / 启停 / 版本历史。
        .merge(crate::ai_roles::router())
        // AI Team 总览与执行监控（UI-2，U-1=C）：现有表只读聚合（零迁移）。
        .merge(crate::ai_team_monitor::router())
        // Execution Policies 读写面（UI-3，U-2=A）：四层策略现状 + 项目层/任务
        // 覆盖层写入（org/task_type V1 只读，零迁移）。
        .merge(crate::ai_team_policies::router())
        // 最小对话存储（V2 D-4=A）：AI PM 对话记忆（会话 + 消息，属主隔离；
        // REST 恒 role=user，assistant 生产者是后续 AI 链批次）。
        .merge(crate::ai_conversations::router())
        .merge(crate::ai_context_items::router())
        // Outbox 运维（T-061）：死信列表 / 死信重放 / 事件状态监控。
        .merge(crate::outbox_ops::router())
        .merge(crate::intakes::router())
        .merge(crate::knowledge_services::router())
        // KP-3 KB 管理面（0065）：来源详情 / 记忆提案 / 记忆列表与详情（审核只走 K4）。
        .merge(crate::kb_memory_api::router())
        .merge(crate::retrieval::router())
        .merge(crate::admin_config::router())
        // 执行端接入引导（T-080）：公开的 `/install`、`/install/{tool}`。
        .merge(crate::onboarding::router())
        // 技能包同源分发（T-089）：manifest + zip 下载（公开，`/install*` 已放行）。
        .merge(crate::skills_package::router())
        // 开发环境目录（T-088）：设备登记 / 绑定 / 自检 / 撤销。
        .merge(crate::pm_devices::router())
        // S01-04 认知（Run/Item/Knowledge）：人工启动 + 逐项确认。
        .merge(crate::pm_cognition::router())
        // S01-05 项目基线：提案派生（知识+仓库快照）→ 逐项确认 → 版本不可变。
        .merge(crate::pm_baseline::router())
        .merge(crate::pm_requirements::router())
        .merge(crate::pm_designs::router())
        .merge(crate::pm_plans::router())
        // 文件代理（2026-11-07）：file-service 收口 mesh-only 后的用户面
        // 入口，同形状复刻其 REST 契约，出站走 per-tenant 服务 JWT。
        .merge(crate::files::router())
        // Document Processing（0016）：平台级文档上传/解析/产物/比较/导出。
        .merge(crate::docproc::router())
        .merge(crate::pm_approvers::router())
        .merge(crate::pm_interactions::router())
        // 审计实时流（SSE）。EventSource 无法带 Authorization，token 走
        // `?access_token=`；auth 层只对本路径放行 query token。
        .route(
            "/v1/pm/events/stream",
            get(crate::audit_stream::stream_handler),
        )
        // 知识空间（只读 + 树检索）：正文来自 Git 知识仓库，见 knowledge-space plan。
        .route("/v1/pm/documents", get(crate::documents::list_handler))
        .route(
            "/v1/pm/documents/search",
            get(crate::documents::search_handler),
        )
        // 项目正式 Goal（S01 · I4：Q1 随创建建立 / Q2 Agent 只能提案）。
        .route(
            "/v1/pm/projects/{project_id}/goal",
            get(crate::documents::goal_handler),
        )
        .route(
            "/v1/pm/documents/{document_id}",
            get(crate::documents::get_handler),
        )
        // 知识空间 V2：提案 / 红线确认 / 否决回滚 / 索引 / 引用 / 需求迁移。
        .route(
            "/v1/pm/documents/proposals",
            get(crate::documents::list_proposals_handler)
                .post(crate::documents::submit_proposal_handler),
        )
        .route(
            "/v1/pm/documents/proposals/{proposal_id}/confirm",
            post(crate::documents::confirm_redline_handler),
        )
        .route(
            "/v1/pm/documents/revisions/{revision_id}/veto",
            post(crate::documents::veto_revision_handler),
        )
        .route(
            "/v1/pm/documents/revisions/{revision_id}/reindex",
            post(crate::documents::reindex_handler),
        )
        // 项目级全量重建（T-060 §5.5；索引版本升级后恢复 DB 直查）。
        .route(
            "/v1/pm/documents/reindex",
            post(crate::documents::reindex_project_handler),
        )
        .route(
            "/v1/pm/documents/{document_id}/references",
            get(crate::documents::list_references_handler)
                .post(crate::documents::add_reference_handler),
        )
        .route(
            "/v1/pm/documents/{document_id}/index-state",
            get(crate::documents::index_state_handler),
        )
        .route(
            "/v1/pm/requirements/revisions/{revision_id}/migrate",
            post(crate::documents::migrate_requirement_handler),
        )
        .nest("/mcp", mcp_router)
        .with_state(state)
}

/// The same route table guarded by the bearer-auth layer (`auth::require_bearer`).
/// Served by `main.rs` under `AI_PM_AUTH_MODE=jwks` (the default, fail-closed);
/// the bare [`router`] stays the `off`-mode/test shape so existing tests keep
/// exercising the unauthenticated contract. The layer wraps everything routed
/// above, including the nested `/mcp` service.
pub fn router_with_auth(pool: PgPool, jwks: Arc<digua_common_jwt::JWKSCache>) -> Router {
    router_with_auth_config_and_feed_and_hosts(
        pool,
        jwks,
        crate::auth::AuthConfig::from_env(),
        crate::audit_stream::AuditFeed::new(),
        crate::mcp_hosts::McpHosts::default(),
    )
}

/// [`router_with_auth`] over an explicit audit feed (see [`router_with_feed`]).
///
/// The `/mcp` Host allow-list comes from the environment (T-081), like the
/// auth config.
pub fn router_with_auth_and_feed(
    pool: PgPool,
    jwks: Arc<digua_common_jwt::JWKSCache>,
    audit_feed: crate::audit_stream::AuditFeed,
) -> Router {
    router_with_auth_config_and_feed_and_hosts(
        pool,
        jwks,
        crate::auth::AuthConfig::from_env(),
        audit_feed,
        crate::mcp_hosts::McpHosts::from_env(),
    )
}

/// [`router_with_auth`] with an explicit first-admin pair (tests pass the pair
/// directly instead of mutating process-wide env vars).
pub fn router_with_auth_config(
    pool: PgPool,
    jwks: Arc<digua_common_jwt::JWKSCache>,
    config: crate::auth::AuthConfig,
) -> Router {
    router_with_auth_config_and_feed(pool, jwks, config, crate::audit_stream::AuditFeed::new())
}

/// [`router_with_auth_and_feed`] with an explicit auth config, over an explicit
/// audit feed.
///
/// Layer order (outermost first): `require_bearer` → `resolve_principal`
/// → `require_active` → routes (v1 §6).
pub fn router_with_auth_config_and_feed(
    pool: PgPool,
    jwks: Arc<digua_common_jwt::JWKSCache>,
    config: crate::auth::AuthConfig,
    audit_feed: crate::audit_stream::AuditFeed,
) -> Router {
    router_with_auth_config_and_feed_and_hosts(
        pool,
        jwks,
        config,
        audit_feed,
        crate::mcp_hosts::McpHosts::default(),
    )
}

/// [`router_with_auth_config_and_feed`] with an explicit `/mcp` Host
/// allow-list (T-081). Tests pass the default (loopback only) so the ambient
/// environment cannot change what the suite exercises.
pub fn router_with_auth_config_and_feed_and_hosts(
    pool: PgPool,
    jwks: Arc<digua_common_jwt::JWKSCache>,
    config: crate::auth::AuthConfig,
    audit_feed: crate::audit_stream::AuditFeed,
    mcp_hosts: crate::mcp_hosts::McpHosts,
) -> Router {
    authed_router(pool, jwks, config, audit_feed, mcp_hosts, None)
}

/// [`router_with_auth_config`] with an explicit DK-A conversation routing
/// (tests pin the `AI_PM_CONVERSATION_ROUTER` mode and inject a fixture
/// classifier instead of mutating process-wide env vars).
pub fn router_with_auth_config_and_routing(
    pool: PgPool,
    jwks: Arc<digua_common_jwt::JWKSCache>,
    config: crate::auth::AuthConfig,
    routing: ConversationRouting,
) -> Router {
    authed_router(
        pool,
        jwks,
        config,
        crate::audit_stream::AuditFeed::new(),
        crate::mcp_hosts::McpHosts::default(),
        Some(routing),
    )
}

fn authed_router(
    pool: PgPool,
    jwks: Arc<digua_common_jwt::JWKSCache>,
    config: crate::auth::AuthConfig,
    audit_feed: crate::audit_stream::AuditFeed,
    mcp_hosts: crate::mcp_hosts::McpHosts,
    routing: Option<ConversationRouting>,
) -> Router {
    let auth = crate::auth::AuthState {
        jwks: Some(jwks),
        pool: pool.clone(),
        config,
    };
    build_router(
        pool,
        audit_feed,
        true,
        mcp_hosts,
        pm_application::knowledge::KnowledgeRoot::from_env(),
        routing,
    )
    .layer(axum::middleware::from_fn_with_state(
        auth.clone(),
        crate::auth::require_active,
    ))
    .layer(axum::middleware::from_fn_with_state(
        auth.clone(),
        crate::auth::resolve_principal,
    ))
    .layer(axum::middleware::from_fn_with_state(
        auth,
        crate::auth::require_bearer,
    ))
}

/// SEC-P0（DEC-SEC-P0-01）：legacy 监督/接入端点的统一项目守卫。
///
/// `off` 模式（`inject_dev_principal`）是本地开发/测试信任域，保持直通；
/// 生产（`AI_PM_AUTH_MODE=jwks`，默认 fail-closed）一律走
/// [`require_project_visible`]（租户 + 项目成员，不可见即 404 不泄露存在性）。
async fn require_legacy_project_access(
    pool: &PgPool,
    project_id: Uuid,
    caller: &AuthedPrincipal,
) -> Result<(), Response> {
    if caller.auth_mode == "off" {
        return Ok(());
    }
    match require_project_visible(pool, project_id, caller.tenant_id, caller.principal_id).await {
        Ok(()) => Ok(()),
        Err(_) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "project_not_found" })),
        )
            .into_response()),
    }
}

// ---------------------------------------------------------------- ingestion

use pm_application::ingestion::{submit_event, IngestionError, SubmitEventRequest, SubmitOutcome};

/// `GET /health`：就绪探针（公开路径，见 `auth::is_public_path`）。
///
/// runbook §二 把它当作部署校验与执行端安装验证的第一步——路由此前只存在于
/// `auth.rs` 的放行名单里，实际请求会 404。
async fn health_handler() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "service": "pm-api" }))
}

async fn submit_event_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Json(mut req): Json<SubmitEventRequest>,
) -> Response {
    // SEC-P0-1（DEC-SEC-P0-01）：project 归属校验 + actor 只来自认证主体
    // （不再信任 body 声明）。
    if let Err(resp) =
        require_legacy_project_access(&state.pool, req.project_id.into(), &caller).await
    {
        return resp;
    }
    req.actor_id = caller.principal_id.into();
    match submit_event(&state.pool, req).await {
        Ok(SubmitOutcome::Accepted { event_id }) => (
            StatusCode::ACCEPTED,
            Json(json!({ "event_id": event_id, "processing_status": "queued" })),
        )
            .into_response(),
        Ok(SubmitOutcome::Duplicate { event_id, status }) => (
            StatusCode::OK,
            Json(json!({
                "event_id": event_id,
                "status": status,
                "processing_status": "duplicate",
            })),
        )
            .into_response(),
        Err(IngestionError::IdempotencyConflict { existing_event_id }) => (
            StatusCode::CONFLICT,
            Json(json!({ "error": "idempotency_conflict", "event_id": existing_event_id })),
        )
            .into_response(),
        Err(IngestionError::UnknownProject) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "unknown_project" })),
        )
            .into_response(),
        Err(IngestionError::Db(err)) => {
            tracing::error!(error = ?err, "ingestion failed with a database error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
                .into_response()
        }
    }
}

// --------------------------------------------------------------- supervision

async fn veto_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Json(mut req): Json<VetoRequest>,
) -> Response {
    // SEC-P0-3（DEC-SEC-P0-01）：caller 必须是该 project 成员；服务层的
    // `WHERE id AND project_id` 配对保证 proposal 确属该 project，二者合取
    // 即"caller 是 proposal 所属 project 的成员"（A-03）。actor 只来自认证主体。
    if let Err(resp) =
        require_legacy_project_access(&state.pool, req.project_id.into(), &caller).await
    {
        return resp;
    }
    req.actor_id = caller.principal_id.into();
    req.actor = Some(caller.audit_actor());
    match veto(&state.pool, &state.executor, req).await {
        Ok(VetoOutcome::RolledBack { new_audit_id }) => (
            StatusCode::OK,
            Json(json!({ "outcome": "rolled_back", "new_audit_id": new_audit_id })),
        )
            .into_response(),
        // Idempotent repeat: still a success for the caller.
        Ok(VetoOutcome::AlreadyCompensated) => (
            StatusCode::OK,
            Json(json!({ "outcome": "already_compensated", "already": true })),
        )
            .into_response(),
        Ok(VetoOutcome::ProposalClosed) => (
            StatusCode::OK,
            Json(json!({ "outcome": "proposal_closed" })),
        )
            .into_response(),
        // The rollback was refused (the target moved on); the service already
        // recorded intervention('rollback_failed').
        Err(SupervisionError::Command(CommandError::RevisionConflict { expected, actual })) => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "rollback_failed",
                "expected_revision": expected,
                "actual_revision": actual,
            })),
        )
            .into_response(),
        // An irreversible proposal (no stored compensation spec) refuses the
        // rollback; the service already recorded intervention('rollback_failed').
        Err(SupervisionError::Command(CommandError::Validation(detail))) => (
            StatusCode::CONFLICT,
            Json(json!({ "error": "rollback_failed", "detail": detail })),
        )
            .into_response(),
        Err(SupervisionError::NotFound(detail)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "proposal_not_found", "detail": detail })),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "veto failed");
            internal_error()
        }
    }
}

async fn decide_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(approval_id): Path<Uuid>,
    Json(req): Json<DecideRequest>,
) -> Response {
    // The path names the approval; the body's field (if any) is ignored.
    let mut req = DecideRequest { approval_id, ..req };
    // SEC-P0-2（DEC-SEC-P0-01）：project 从 approval 对象反查（tenant 锚定，
    // A-03 不信任客户端 id），再验 caller 成员可见性；否则 404 不泄露存在性。
    let project_id = match repos::guard_query_repo(&state.pool)
        .approval_head(caller.tenant_id, approval_id)
        .await
    {
        Ok(Some((project_id, _))) => project_id,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "approval_not_found" })),
            )
                .into_response()
        }
        Err(err) => {
            tracing::error!(error = %err, "approval lookup failed");
            return internal_error();
        }
    };
    if let Err(resp) = require_legacy_project_access(&state.pool, project_id, &caller).await {
        return resp;
    }
    req.actor_id = caller.principal_id.into();
    req.actor = Some(caller.audit_actor());
    match decide_approval(&state.pool, &state.executor, req).await {
        Ok(DecisionOutcome::Executed { receipt }) => (
            StatusCode::OK,
            Json(json!({
                "outcome": "executed",
                "audit_id": receipt.audit_id,
                "command_kind": receipt.command_kind,
                "target_id": receipt.target_id,
                "resulting_revision": receipt.resulting_revision,
            })),
        )
            .into_response(),
        Ok(DecisionOutcome::Rejected) => {
            (StatusCode::OK, Json(json!({ "outcome": "rejected" }))).into_response()
        }
        // Idempotent repeat: HTTP 200 with `already: true`.
        Err(SupervisionError::Command(CommandError::AlreadyDecided { .. })) => (
            StatusCode::OK,
            Json(json!({ "outcome": "already_decided", "already": true })),
        )
            .into_response(),
        Err(SupervisionError::NotFound(detail)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "approval_not_found", "detail": detail })),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "red-line decision failed");
            internal_error()
        }
    }
}

/// Freeze / unfreeze request body.
#[derive(Debug, Deserialize)]
struct FreezeRequest {
    project_id: Uuid,
    // SEC-P0-4：actor_id 不再从 body 采信（actor = AuthedPrincipal）。
    // 字段保留以兼容旧客户端 payload（serde 默认忽略未知字段也可，这里显式
    // 接收后丢弃，避免 deny_unknown_fields 类破坏）。
    #[allow(dead_code)]
    actor_id: Uuid,
    reason: String,
}

async fn freeze_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Json(req): Json<FreezeRequest>,
) -> Response {
    // The brake flips the automation mode, records the intervention and the
    // notification, and answers 200 (404 when the project has no AI config).
    // SEC-P0-4（DEC-SEC-P0-01）：归属校验 + actor 只来自认证主体（body 的
    // actor_id 仅保留反序列化兼容，不再采信）。
    if let Err(resp) = require_legacy_project_access(&state.pool, req.project_id, &caller).await {
        return resp;
    }
    match freeze(
        &state.pool,
        req.project_id,
        caller.principal_id,
        &req.reason,
    )
    .await
    {
        Ok(()) => (StatusCode::OK, Json(json!({ "outcome": "frozen" }))).into_response(),
        Err(SupervisionError::NotFound(detail)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "unknown_project", "detail": detail })),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "freeze failed");
            internal_error()
        }
    }
}

async fn unfreeze_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Json(req): Json<FreezeRequest>,
) -> Response {
    // SEC-P0-4（同 freeze）：归属校验 + actor 只来自认证主体。
    if let Err(resp) = require_legacy_project_access(&state.pool, req.project_id, &caller).await {
        return resp;
    }
    match unfreeze(
        &state.pool,
        req.project_id,
        caller.principal_id,
        &req.reason,
    )
    .await
    {
        Ok(()) => (StatusCode::OK, Json(json!({ "outcome": "unfrozen" }))).into_response(),
        Err(SupervisionError::NotFound(detail)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "unknown_project", "detail": detail })),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "unfreeze failed");
            internal_error()
        }
    }
}

/// Feed query string: `?since=<RFC3339>&limit=<n>`; both optional.
#[derive(Debug, Deserialize)]
struct FeedQuery {
    since: Option<String>,
    limit: Option<i64>,
}

async fn notifications_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<FeedQuery>,
) -> Response {
    // SEC-P0-5（DEC-SEC-P0-01）：跨租户读修复——先验归属再查 feed。
    if let Err(resp) = require_legacy_project_access(&state.pool, project_id, &caller).await {
        return resp;
    }
    let since = match query.since {
        Some(text) => match chrono::DateTime::parse_from_rfc3339(&text) {
            Ok(t) => Some(t.with_timezone(&chrono::Utc)),
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "invalid_since", "detail": "since must be RFC3339" })),
                )
                    .into_response();
            }
        },
        None => None,
    };
    match notifications_feed(&state.pool, project_id, since, query.limit.unwrap_or(50)).await {
        Ok(rows) => (StatusCode::OK, Json(rows)).into_response(),
        Err(err) => {
            tracing::error!(error = ?err, "notification feed failed");
            internal_error()
        }
    }
}

fn internal_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "internal" })),
    )
        .into_response()
}
