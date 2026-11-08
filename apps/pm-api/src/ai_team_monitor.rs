//! `/v1/pm/projects/{project_id}/ai-team/{overview,monitor}`：AI Team 总览与
//! 执行监控（UI-2，U-1=C：先聚合现有 audit/run 落 UI，**不建事件表、零迁移**；
//! 裁决 `docs/project-ai-team-v2/ai-pm-ai-team-ui-monitor-adjudication-pack-v1.md`
//! §〇，事件表挂 Phase 4）。
//!
//! - GET `/ai-team/overview` —— 计数聚合：roles 启停计数 + 模板总数、活跃 Run
//!   （queued/claimed/running）与近 24h succeeded/failed、任务
//!   awaiting_decision/blocked、Provider 绑定态（复用 `resolve_repo_call_config`
//!   的解析语义，只报 bound + provider 名，**不读凭证/不解密**）。
//! - GET `/ai-team/monitor` —— 明细聚合：
//!   * `runs`：最近 50 条 `delivery.task_run`（created_at DESC）JOIN `delivery.task`
//!     取 title；
//!   * `events`：最近 100 条 `governance.audit_log`（`source_kind='run_dispatch'`，
//!     action ∈ {claim_task_run, skip_task_run, reclaim_task_run,
//!     task_ready_skipped, task_ready_assignee_resolved}）。
//!
//! 只读聚合：monitor 聚合本体恰 2 条 SQL（runs + events；守卫查询另计）；
//! overview 全部 COUNT 聚合 + 轻查询。权限：项目可见（不存在 / 跨租户 /
//! 非成员一律 404，沿用 `require_project_visible` T-022 口径）。

use axum::{
    extract::{Extension, Path, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, ApiError, ApiResult};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/ai-team/overview",
            get(overview_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/ai-team/monitor",
            get(monitor_handler),
        )
}

/// 项目边界：不存在 / 跨租户 / 调用者非成员一律 404（不泄露存在性）。
/// 返回项目租户 id（Provider 解析的 team 默认分支需要）。
async fn require_read_access(
    state: &AppState,
    project_id: Uuid,
    principal: &AuthedPrincipal,
) -> ApiResult<Uuid> {
    let project_tenant: Option<Uuid> =
        sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(&state.pool)
            .await?;
    let Some(tenant_id) = project_tenant else {
        return Err(ApiError::NotFound("project not found"));
    };
    if tenant_id != principal.tenant_id {
        return Err(ApiError::NotFound("project not found"));
    }
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    Ok(tenant_id)
}

// ── Overview ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct AiTeamOverviewView {
    pub roles: AiTeamRolesSummary,
    pub runs: AiTeamRunsSummary,
    pub tasks: AiTeamTasksSummary,
    pub provider: AiTeamProviderBinding,
}

#[derive(Debug, Serialize)]
pub(crate) struct AiTeamRolesSummary {
    /// 项目启用行中 enabled=true 的数量
    pub enabled: i64,
    /// 项目启用行中 enabled=false 的数量（未启用的模板不计入）
    pub disabled: i64,
    /// 全局模板总数（ai.ai_role_template；V1 固定 5）
    pub templates: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct AiTeamRunsSummary {
    /// 活跃 Run：status IN (queued, claimed, running)
    pub active: i64,
    /// 近 24h 成功终态（时间锚 = COALESCE(finished_at, created_at)）
    pub succeeded_24h: i64,
    /// 近 24h 失败终态（同上锚点）
    pub failed_24h: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct AiTeamTasksSummary {
    pub awaiting_decision: i64,
    pub blocked: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct AiTeamProviderBinding {
    /// true = 现行解析（项目指定严格 / 未指定回退 Team 首个 enabled+有模型）
    /// 能拿到可用 Provider
    pub bound: bool,
    /// 绑定到的 Provider 显示名；bound=false 且项目指定过 → 仍回填所指定名字
    /// （前端可提示「指定了但当前不可用」）；完全无绑定 → null
    pub provider_name: Option<String>,
}

async fn overview_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<AiTeamOverviewView>> {
    let tenant_id = require_read_access(&state, project_id, &principal).await?;

    // ① roles 计数（1 条标量聚合）
    let (enabled, disabled, templates): (i64, i64, i64) = sqlx::query_as(
        "SELECT \
           (SELECT count(*) FROM ai.project_ai_role \
             WHERE project_id = $1 AND enabled), \
           (SELECT count(*) FROM ai.project_ai_role \
             WHERE project_id = $1 AND NOT enabled), \
           (SELECT count(*) FROM ai.ai_role_template)",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;

    // ② runs / tasks 计数（2 条聚合）
    let (active, succeeded_24h, failed_24h): (i64, i64, i64) = sqlx::query_as(
        "SELECT \
           count(*) FILTER (WHERE status IN ('queued', 'claimed', 'running')), \
           count(*) FILTER (WHERE outcome = 'succeeded' \
             AND COALESCE(finished_at, created_at) >= now() - interval '24 hours'), \
           count(*) FILTER (WHERE outcome = 'failed' \
             AND COALESCE(finished_at, created_at) >= now() - interval '24 hours') \
         FROM delivery.task_run WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;

    let (awaiting_decision, blocked): (i64, i64) = sqlx::query_as(
        "SELECT \
           count(*) FILTER (WHERE status = 'awaiting_decision'), \
           count(*) FILTER (WHERE status = 'blocked') \
         FROM delivery.task WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;

    // ③ Provider 绑定态：复用 team_llm::resolve_repo_call_config 的解析语义，
    //    但只读名字与状态，不触碰凭证表（不查密）。项目指定分支严格不回退。
    let designated: Option<(String, String, String)> = sqlx::query_as(
        "SELECT p.name, p.status, COALESCE(p.configuration ->> 'model', '') \
         FROM ai.project_ai_model_policy pol \
         JOIN ai.team_llm_provider p ON p.id = pol.provider_id AND p.tenant_id = pol.tenant_id \
         WHERE pol.tenant_id = $1 AND pol.project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?;

    let provider = match designated {
        // 项目指定：enabled + 有模型才可用；否则严格失败不回退（名字仍回填）
        Some((name, status, model)) => {
            let usable = status == "enabled" && !model.is_empty();
            AiTeamProviderBinding {
                bound: usable,
                provider_name: Some(name),
            }
        }
        // 未指定：Team 首个 enabled + 有模型（创建序）
        None => {
            let team_default: Option<(String,)> = sqlx::query_as(
                "SELECT p.name FROM ai.team_llm_provider p \
                 WHERE p.tenant_id = $1 AND p.status = 'enabled' \
                   AND COALESCE(p.configuration ->> 'model', '') <> '' \
                 ORDER BY p.created_at, p.id LIMIT 1",
            )
            .bind(tenant_id)
            .fetch_optional(&state.pool)
            .await?;
            AiTeamProviderBinding {
                bound: team_default.is_some(),
                provider_name: team_default.map(|(name,)| name),
            }
        }
    };

    Ok(Json(AiTeamOverviewView {
        roles: AiTeamRolesSummary {
            enabled,
            disabled,
            templates,
        },
        runs: AiTeamRunsSummary {
            active,
            succeeded_24h,
            failed_24h,
        },
        tasks: AiTeamTasksSummary {
            awaiting_decision,
            blocked,
        },
        provider,
    }))
}

// ── Monitor ───────────────────────────────────────────────────────────

/// 聚合明细首屏上限（V1 不做分页：`before`/limit 参数挂后续批次）。
const RUN_LIMIT: i64 = 50;
const EVENT_LIMIT: i64 = 100;

#[derive(Debug, Serialize)]
pub(crate) struct AiTeamMonitorView {
    pub runs: Vec<AiTeamRunRow>,
    pub events: Vec<AiTeamEventRow>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AiTeamRunRow {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub task_title: String,
    pub status: String,
    /// 执行阶段（dispatch/started/…；queued Run 可空）
    pub stage: Option<String>,
    pub run_kind: Option<String>,
    pub attempt_no: i32,
    pub executor_kind: Option<String>,
    pub execution_mode: Option<String>,
    pub release_reason: Option<String>,
    /// N-1 调度保护（none/human_wait/start_wait/timed_out，0046）
    pub scheduling_shield: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    /// 终态结果（succeeded/failed/timed_out/cancelled；未终态为 null）
    pub outcome: Option<String>,
    /// 被替代终止时间（B3 隔离/围栏证据；一般终止为 null）
    pub terminated_at: Option<DateTime<Utc>>,
    /// 展示用完成时间锚：COALESCE(terminated_at, finished_at)
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AiTeamEventRow {
    pub action: String,
    /// 动作目标：claim/skip/reclaim = run_id；task_ready_* = task_id
    pub target_id: Uuid,
    /// JOIN 出的目标任务标题（run 目标经 task_run.task_id 二跳；解析不到为 null）
    pub target_title: Option<String>,
    /// actor 摘要 = principal.display_name（含 dispatch 服务主体；行缺失为 null）
    pub actor: Option<String>,
    /// 释放/跳过原因：COALESCE(after_data->>'release_reason', after_data->>'reason')
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

async fn monitor_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<AiTeamMonitorView>> {
    require_read_access(&state, project_id, &principal).await?;

    // 聚合本体恰 2 条 SQL（守卫查询另计）。
    // ① runs：最近 50 条 task_run JOIN task 取 title。
    let runs: Vec<AiTeamRunRow> = sqlx::query_as(
        "SELECT tr.id AS run_id, tr.task_id, t.title AS task_title, \
                tr.status, tr.stage, tr.run_kind, tr.attempt_no, \
                tr.executor_kind, tr.execution_mode, tr.release_reason, \
                tr.scheduling_shield, tr.claimed_at, tr.outcome, tr.terminated_at, \
                COALESCE(tr.terminated_at, tr.finished_at) AS finished_at, \
                tr.created_at \
         FROM delivery.task_run tr \
         JOIN delivery.task t ON t.id = tr.task_id \
         WHERE tr.project_id = $1 \
         ORDER BY tr.created_at DESC, tr.id DESC \
         LIMIT $2",
    )
    .bind(project_id)
    .bind(RUN_LIMIT)
    .fetch_all(&state.pool)
    .await?;

    // ② events：最近 100 条 run_dispatch 审计。actor 摘要 = display_name；
    //    reason 取 release_reason / reason（skip/reclaim 两种写法都收）；
    //    target 标题：run 目标二跳 task_run→task，task 目标直接 task。
    let events: Vec<AiTeamEventRow> = sqlx::query_as(
        "SELECT a.action, a.target_id, t.title AS target_title, \
                p.display_name AS actor, \
                COALESCE(a.after_data ->> 'release_reason', a.after_data ->> 'reason') AS reason, \
                a.created_at \
         FROM governance.audit_log a \
         LEFT JOIN core.principal p ON p.id = a.actor_id \
         LEFT JOIN delivery.task_run tr ON tr.id = a.target_id \
         LEFT JOIN delivery.task t \
           ON t.id = COALESCE(tr.task_id, CASE WHEN a.target_type = 'task' THEN a.target_id END) \
         WHERE a.project_id = $1 \
           AND a.source_kind = 'run_dispatch' \
           AND a.action IN ('claim_task_run', 'skip_task_run', 'reclaim_task_run', \
                            'task_ready_skipped', 'task_ready_assignee_resolved') \
         ORDER BY a.created_at DESC, a.id DESC \
         LIMIT $2",
    )
    .bind(project_id)
    .bind(EVENT_LIMIT)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(AiTeamMonitorView { runs, events }))
}
