//! S01-04 · AI-PM Cognition V1（设计稿 docs/ui/s01/s01-04-ai-pm-cognition-ux-v1-1.md）。
//!
//! 职责边界（§1.3 / §15）：本模块只做认知 Run / 认知条目 / 项目知识三实体的
//! 建立与逐项确认；正式知识**发布**走知识空间既有管线，需求治理归 S02。
//!
//! V1 分析策略（诚实边界）：人工启动 Run 后，用**既有结构化事实**同步派生
//! 认知条目（项目记录 / 仓库元数据）——不虚构 LLM 输出；真模型编排由
//! ai-pm-worker 切片接管（届时写 collecting/analyzing 状态）。
//!
//! 授权：读写都要求 `core.project_member` 成员（DEF-01 教训：写侧不得只查租户）。
//! 逐项确认（Q3-B）：每条独立决策，不绑定 Run 完成态；确认即落项目知识并留
//! decided_by/at（§6.3），拒绝/暂缓/调查保留历史（§6.4），AI 无确认入口。
//!
//! 数据访问已收口到仓储（任务书批 B）：SQL 在
//! `pm_infrastructure::repo_cognition::PgCognitionRepo`，本模块零 SQL；
//! 派生条目的文案组装（项目身份/仓库元数据）是产品文案，留在本层。

use axum::{
    extract::{Extension, Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{ensure_trusted_context, require_project_visible, ApiError, ApiResult};
use pm_ai::identity_synthesis::LlmIdentitySynthesis;
use pm_application::identity::{apply_identity_drafts, assemble_identity_input, IdentityError};
use pm_application::knowledge::KnowledgeRoot;
use pm_application::repo::{CognitionRepo, CognitionRepoError};
use pm_infrastructure::repo_cognition::PgCognitionRepo;

fn repo(state: &AppState) -> PgCognitionRepo {
    PgCognitionRepo::new(state.pool.clone())
}

fn repo_err(e: CognitionRepoError) -> ApiError {
    match e {
        CognitionRepoError::Db(err) => ApiError::Internal(err),
        CognitionRepoError::Conflict(m) => ApiError::Conflict(m),
        CognitionRepoError::NotFound(m) => ApiError::NotFound(m),
    }
}

/// GET 认知概览：runs（近序）+ 最新 run 的 items + knowledge 统计。
async fn overview(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;

    let runs = repo(&state)
        .recent_runs(caller.tenant_id, project_id, 20)
        .await
        .map_err(repo_err)?;
    let knowledge_active = repo(&state)
        .active_knowledge_count(caller.tenant_id, project_id)
        .await
        .map_err(repo_err)?;
    let proposed = repo(&state)
        .proposed_item_count(caller.tenant_id, project_id)
        .await
        .map_err(repo_err)?;

    // 最新 run 的全部条目（逐项审核区数据源）。
    let latest_run_id = runs.first().map(|r| r.id);
    let items = match latest_run_id {
        Some(run_id) => repo(&state)
            .run_items(caller.tenant_id, run_id)
            .await
            .map_err(repo_err)?,
        None => Vec::new(),
    };

    Ok(Json(json!({
        "runs": runs,
        "latest_run_id": latest_run_id,
        "items": items,
        "knowledge_active": knowledge_active,
        "proposed": proposed,
    })))
}

/// 派生条目文案：项目身份 + 每仓库一条元数据认知。
/// 输入行经仓储 `derivation_input` 读取；文案是产品文案，留在本层组装。
async fn derive_items(
    state: &AppState,
    tenant_id: Uuid,
    project_id: Uuid,
) -> ApiResult<(
    Vec<Value>,
    Vec<pm_application::repo::cognition::DerivedItem<'static>>,
)> {
    let input = repo(state)
        .derivation_input(tenant_id, project_id)
        .await
        .map_err(|e| match e {
            pm_application::repo::cognition::DerivationError::Db(err) => ApiError::Internal(err),
            pm_application::repo::cognition::DerivationError::NotFound => {
                ApiError::NotFound("project not found")
            }
        })?;

    let mut items: Vec<pm_application::repo::cognition::DerivedItem<'static>> = Vec::new();
    let mut repo_versions = Vec::new();

    // 派生条目①：项目身份（无描述也是有效认知；§12 无仓库/缺资料不阻断）。
    let identity_content: String = match input.project_description.as_deref() {
        Some(desc) if !desc.trim().is_empty() => {
            format!("Project '{}' — {}", input.project_name, desc.trim())
        }
        _ => format!(
            "Project '{}' has no description yet; identity rests on the project record.",
            input.project_name
        ),
    };
    items.push(pm_application::repo::cognition::DerivedItem {
        domain: "identity",
        content: Box::leak(identity_content.into_boxed_str()),
        evidence: Box::leak(Box::new(
            json!({"type": "project_record", "source": "core.project"}),
        )),
        confidence: "high",
    });

    // 派生条目②：每仓库一条元数据认知（§9.1 独立认知；不推断仓库间关系）。
    for (id, name, url, branch, role) in &input.repositories {
        let branch_text = branch
            .as_deref()
            .filter(|b| !b.trim().is_empty())
            .unwrap_or("unknown");
        let content: String = format!(
            "Repository '{}' ({}): {} · default branch {}",
            name, role, url, branch_text
        );
        let evidence = json!({
            "type": "repository_metadata",
            "repository_id": id,
            "source": "integration.project_repository"
        });
        items.push(pm_application::repo::cognition::DerivedItem {
            domain: "repository",
            content: Box::leak(content.into_boxed_str()),
            evidence: Box::leak(Box::new(evidence)),
            confidence: "high",
        });
        repo_versions.push(json!({"repository_id": id, "revision_snapshot": "current"}));
    }

    Ok((repo_versions, items))
}

/// POST 启动认知 Run（Q2-C：人类启动）。V1 同步派生结构化事实条目。
/// D-4 · `POST .../cognition/identity-synthesis`：业务身份综合（人工按钮触发，
/// interactive 真人门）。LLM 汇聚访谈答案 + 仓库产物 → 三份草案 → 三提案
/// （PC×2 走 P4/P5，project-goal 走知识 8 态并自动强制红线）。开关
/// `AI_PM_IDENTITY_SYNTHESIS`（缺省 on，off → 501）。
async fn identity_synthesis(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    ensure_trusted_context(&caller)?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let actor = caller.audit_actor();

    let input = assemble_identity_input(&state.pool, project_id)
        .await
        .map_err(|err| {
            ApiError::Text(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("assemble failed: {err}"),
            )
        })?;
    let drafts = match LlmIdentitySynthesis::new(state.pool.clone())
        .synthesize(project_id, &input)
        .await
    {
        Ok(drafts) => drafts,
        Err(pm_ai::model::ModelError::Configuration(detail)) => {
            return Err(ApiError::NotImplemented(Box::leak(
                detail.clone().into_boxed_str(),
            )));
        }
        Err(err) => {
            return Err(ApiError::Text(
                axum::http::StatusCode::BAD_GATEWAY,
                format!("identity synthesis failed: {err}"),
            ));
        }
    };

    // Goal 提案需要知识根（agent 无直接 Git 写权，提案管线自持锁与红线）。
    let root = KnowledgeRoot::from_env();
    let outcome = apply_identity_drafts(&state.pool, Some(&root), project_id, &actor, &drafts)
        .await
        .map_err(|e| match e {
            IdentityError::Conflict(m) | IdentityError::Failed(m) => {
                ApiError::Text(axum::http::StatusCode::CONFLICT, m)
            }
            IdentityError::Forbidden(m) => ApiError::Text(axum::http::StatusCode::FORBIDDEN, m),
            IdentityError::Db(err) => ApiError::Internal(err),
        })?;

    Ok(Json(json!({
        "profile_proposal_id": outcome.profile_proposal_id,
        "vision_proposal_id": outcome.vision_proposal_id,
        "goal_proposal_id": outcome.goal_proposal_id,
        "goal_proposal_status": outcome.goal_proposal_status,
        "uncertainty_notes": drafts.uncertainty_notes,
    })))
}

async fn start_run(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;

    let (repo_versions, items) = derive_items(&state, caller.tenant_id, project_id).await?;
    let scope = json!({
        "sources": ["project_record", "repository_metadata"],
        "repository_count": repo_versions.len(),
        "mode": "structured_facts_v1",
    });

    let started = repo(&state)
        .start_run(
            caller.tenant_id,
            project_id,
            caller.principal_id,
            &scope,
            &items,
            &Value::Array(repo_versions),
        )
        .await
        .map_err(repo_err)?;

    Ok(Json(json!({
        "run_id": started.run_id,
        "status": "completed",
        "item_count": started.item_count,
    })))
}

#[derive(Deserialize)]
struct DecisionBody {
    /// confirm | reject | defer | investigate（§4.6；Edit 并入 confirm 的 content）
    action: String,
    /// Edit & Confirm（§6.1）：确认前修改正文
    content: Option<String>,
    note: Option<String>,
}

/// POST 逐项决策（Q3-B：每条独立确认；路由在成员人类层之下，AI 无此入口）。
async fn decide_item(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((project_id, item_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<DecisionBody>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;

    let next_state = match body.action.as_str() {
        "confirm" => "confirmed",
        "reject" => "rejected",
        "defer" => "deferred",
        "investigate" => "investigation",
        _ => return Err(ApiError::BadRequest("invalid action")),
    };

    let decision = repo(&state)
        .decide_item(
            caller.tenant_id,
            item_id,
            next_state,
            caller.principal_id,
            body.note.as_deref(),
            body.content.as_deref(),
        )
        .await
        .map_err(repo_err)?;

    Ok(Json(json!({
        "id": item_id,
        "state": next_state,
        "knowledge_id": decision.knowledge_id,
    })))
}

/// GET 已确认项目知识（Active）。
async fn knowledge(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;

    let items = repo(&state)
        .active_knowledge(caller.tenant_id, project_id)
        .await
        .map_err(repo_err)?;

    Ok(Json(json!({ "items": items })))
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/projects/{project_id}/cognition", get(overview))
        .route(
            "/v1/pm/projects/{project_id}/cognition/identity-synthesis",
            post(identity_synthesis),
        )
        .route(
            "/v1/pm/projects/{project_id}/cognition/runs",
            post(start_run),
        )
        .route(
            "/v1/pm/projects/{project_id}/cognition/items/{item_id}/decision",
            post(decide_item),
        )
        .route(
            "/v1/pm/projects/{project_id}/cognition/knowledge",
            get(knowledge),
        )
}
