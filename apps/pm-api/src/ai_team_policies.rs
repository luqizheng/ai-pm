//! `/v1/pm/projects/{project_id}/ai-team/execution-policies*` 与任务级覆盖
//! `/v1/pm/tasks/{task_id}/policy-override`：Execution Policies 子页数据面
//! （UI-3，U-2=A 五子页最后一块；零迁移——0013/0010 两表既有，此前只有调度
//! 合成在读）。数据面见 `pm_application::ai_team_policies`。
//!
//! - GET    `/ai-team/execution-policies`          —— 四层现状 + 合成规则说明；
//! - PUT    `/ai-team/execution-policies/project`  —— body
//!   `{require_human_approval: bool|null}`（null = 删行恢复继承组织）；
//! - PUT    `/tasks/{task_id}/policy-override`     —— body
//!   `{require_human_approval|auto_accept|auto_merge|auto_publish_knowledge:
//!   bool|null}`（全 null 字段 = 继承；至少一个非 null，否则 400；清除走 DELETE）；
//! - DELETE `/tasks/{task_id}/policy-override`     —— 删行恢复全继承（行不存在
//!   为幂等成功）。
//!
//! **V1 写面限定**：organization（全局 '*'）与 task_type（跨项目）两层只读，
//! 写入口留待显式裁决——REST 层不挂对应写路由。
//!
//! 权限：读 = 项目可见（不存在 / 跨租户 / 非成员一律 404，T-022 口径）；
//! 写 = 租户 owner/admin 或项目 owner（`guard_policy_manager`，照 ai_roles
//! `guard_role_manager` / team_llm `guard_project_manager` 同款矩阵），
//! 否则 403。
//!
//! 请求体 JSON 字段用 snake_case（与 team_llm `provider_id` /
//! `expected_revision` 等既有 pm-api 契约同款；任务说明里的 camelCase 视为
//! 伪码）。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use pm_application::ai_team_policies::{
    clear_task_policy_override, get_project_execution_policies, set_project_execution_policy,
    set_task_policy_override, AiTeamPolicyError, ExecutionPoliciesView, ScopePolicyView,
    TaskPolicyOverrideInput,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, require_task_project, ApiError, ApiResult};

impl From<AiTeamPolicyError> for ApiError {
    fn from(e: AiTeamPolicyError) -> Self {
        match e {
            AiTeamPolicyError::ProjectNotFound | AiTeamPolicyError::TaskNotFound => {
                ApiError::NotFound("project not found")
            }
            AiTeamPolicyError::Validation(m) => ApiError::Text(StatusCode::BAD_REQUEST, m),
            AiTeamPolicyError::Db(err) => ApiError::Internal(err),
        }
    }
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/ai-team/execution-policies",
            get(list_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/ai-team/execution-policies/project",
            put(set_project_handler),
        )
        .route(
            "/v1/pm/tasks/{task_id}/policy-override",
            put(set_task_override_handler).delete(clear_task_override_handler),
        )
}

/// 项目边界：不存在 / 跨租户 → 404（不泄露存在性）。
async fn require_project_tenant(
    state: &AppState,
    project_id: Uuid,
    principal: &AuthedPrincipal,
) -> ApiResult<()> {
    let project_tenant: Option<Uuid> =
        sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(&state.pool)
            .await?;
    if project_tenant.as_ref() != Some(&principal.tenant_id) {
        return Err(ApiError::NotFound("project not found"));
    }
    Ok(())
}

/// 读守卫：项目可见（跨租户或调用者非成员一律 404）。
async fn require_read_access(
    state: &AppState,
    project_id: Uuid,
    principal: &AuthedPrincipal,
) -> ApiResult<()> {
    require_project_tenant(state, project_id, principal).await?;
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
}

/// 写守卫：租户 owner/admin 或项目 owner（§12「Team Owner/Admin ✓」同款矩阵）。
async fn guard_policy_manager(
    state: &AppState,
    principal: &AuthedPrincipal,
    project_id: Uuid,
) -> ApiResult<()> {
    require_project_tenant(state, project_id, principal).await?;
    if principal.is_admin {
        return Ok(());
    }
    match pm_application::project_member::is_project_manager(
        &state.pool,
        principal.tenant_id,
        project_id,
        principal.principal_id,
    )
    .await
    {
        Ok(true) => Ok(()),
        Ok(false) => Err(ApiError::Forbidden("project manager required")),
        Err(_) => Err(ApiError::NotFound("project not found")),
    }
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ExecutionPoliciesView>> {
    require_read_access(&state, project_id, &principal).await?;
    let view = get_project_execution_policies(&state.pool, principal.tenant_id, project_id).await?;
    Ok(Json(view))
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetProjectPolicyBody {
    /// null = 删除项目行（恢复继承组织）。
    #[serde(default)]
    pub require_human_approval: Option<bool>,
}

async fn set_project_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<SetProjectPolicyBody>,
) -> ApiResult<Json<Option<ScopePolicyView>>> {
    guard_policy_manager(&state, &principal, project_id).await?;
    let actor = principal.audit_actor();
    let row = set_project_execution_policy(
        &state.pool,
        &actor,
        principal.tenant_id,
        project_id,
        body.require_human_approval,
    )
    .await?;
    Ok(Json(row))
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetTaskOverrideBody {
    #[serde(default)]
    pub require_human_approval: Option<bool>,
    #[serde(default)]
    pub auto_accept: Option<bool>,
    #[serde(default)]
    pub auto_merge: Option<bool>,
    #[serde(default)]
    pub auto_publish_knowledge: Option<bool>,
}

impl From<SetTaskOverrideBody> for TaskPolicyOverrideInput {
    fn from(body: SetTaskOverrideBody) -> Self {
        TaskPolicyOverrideInput {
            auto_accept: body.auto_accept,
            auto_merge: body.auto_merge,
            auto_publish_knowledge: body.auto_publish_knowledge,
            require_human_approval: body.require_human_approval,
        }
    }
}

/// 任务级写端点共用前置：任务可见项目（404 口径）→ 该项目写守卫（403 口径）。
async fn guard_task_policy_manager(
    state: &AppState,
    principal: &AuthedPrincipal,
    task_id: Uuid,
) -> ApiResult<()> {
    let project_id = require_task_project(&state.pool, task_id, principal).await?;
    guard_policy_manager(state, principal, project_id).await
}

async fn set_task_override_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
    Json(body): Json<SetTaskOverrideBody>,
) -> ApiResult<StatusCode> {
    guard_task_policy_manager(&state, &principal, task_id).await?;
    let actor = principal.audit_actor();
    set_task_policy_override(
        &state.pool,
        &actor,
        principal.tenant_id,
        task_id,
        body.into(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn clear_task_override_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    guard_task_policy_manager(&state, &principal, task_id).await?;
    let actor = principal.audit_actor();
    clear_task_policy_override(&state.pool, &actor, principal.tenant_id, task_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
