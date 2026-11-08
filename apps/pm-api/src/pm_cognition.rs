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

use axum::{
    extract::{Extension, Path, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, ApiError, ApiResult};

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

    let runs: Vec<RunRow> = sqlx::query_as(
        "SELECT id, scope, item_count, confirmed_count, status, created_at \
         FROM ai.cognition_run WHERE tenant_id = $1 AND project_id = $2 \
         ORDER BY created_at DESC LIMIT 20",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;

    let knowledge_active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.project_knowledge \
         WHERE tenant_id = $1 AND project_id = $2 AND status = 'active'",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;

    let proposed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.cognition_item ci \
         JOIN ai.cognition_run cr ON cr.id = ci.run_id \
         WHERE ci.tenant_id = $1 AND ci.project_id = $2 AND ci.state = 'proposed' \
           AND cr.status <> 'cancelled'",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;

    // 最新 run 的全部条目（逐项审核区数据源）。
    let latest_run_id = runs.first().map(|r| r.id);
    let items: Vec<CognitionItemRow> = match latest_run_id {
        Some(run_id) => {
            sqlx::query_as(
                "SELECT id, domain, content, evidence, confidence, has_conflict, \
                    state, decided_by, decided_at, decision_note \
             FROM ai.cognition_item WHERE tenant_id = $1 AND run_id = $2 \
             ORDER BY created_at",
            )
            .bind(caller.tenant_id)
            .bind(run_id)
            .fetch_all(&state.pool)
            .await?
        }
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

#[derive(sqlx::FromRow, serde::Serialize)]
struct RunRow {
    id: Uuid,
    scope: serde_json::Value,
    item_count: i32,
    confirmed_count: i32,
    status: String,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct CognitionItemRow {
    id: Uuid,
    domain: String,
    content: String,
    evidence: serde_json::Value,
    confidence: String,
    has_conflict: bool,
    state: String,
    decided_by: Option<Uuid>,
    decided_at: Option<DateTime<Utc>>,
    decision_note: Option<String>,
}

/// POST 启动认知 Run（Q2-C：人类启动）。V1 同步派生结构化事实条目。
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

    let mut tx = state.pool.begin().await.map_err(ApiError::Internal)?;

    let project: (String, Option<String>) = sqlx::query_as(
        "SELECT name, description FROM core.project \
         WHERE id = $1 AND tenant_id = $2",
    )
    .bind(project_id)
    .bind(caller.tenant_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| ApiError::NotFound("project not found"))?;

    let repos: Vec<(Uuid, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT id, name, url, default_branch, role \
         FROM integration.project_repository \
         WHERE tenant_id = $1 AND project_id = $2 ORDER BY created_at",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;

    let scope = json!({
        "sources": ["project_record", "repository_metadata"],
        "repository_count": repos.len(),
        "mode": "structured_facts_v1",
    });
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.cognition_run (tenant_id, project_id, status, scope, started_by) \
         VALUES ($1, $2, 'pending', $3, $4) RETURNING id",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .bind(&scope)
    .bind(caller.principal_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;

    // 派生条目①：项目身份（无描述也是有效认知；§12 无仓库/缺资料不阻断）。
    let identity_content = match project.1.as_deref() {
        Some(desc) if !desc.trim().is_empty() => {
            format!("Project '{}' — {}", project.0, desc.trim())
        }
        _ => format!(
            "Project '{}' has no description yet; identity rests on the project record.",
            project.0
        ),
    };
    insert_item(
        &mut tx,
        caller.tenant_id,
        project_id,
        run_id,
        "identity",
        &identity_content,
        &json!({"type": "project_record", "source": "core.project"}),
        "high",
    )
    .await?;

    // 派生条目②：每仓库一条元数据认知（§9.1 独立认知；不推断仓库间关系）。
    let mut repo_versions = Vec::new();
    for (id, name, url, branch, role) in &repos {
        let branch_text = branch
            .as_deref()
            .filter(|b| !b.trim().is_empty())
            .unwrap_or("unknown");
        let content = format!(
            "Repository '{}' ({}): {} · default branch {}",
            name, role, url, branch_text
        );
        insert_item(
            &mut tx,
            caller.tenant_id,
            project_id,
            run_id,
            "repository",
            &content,
            &json!({
                "type": "repository_metadata",
                "repository_id": id,
                "source": "integration.project_repository"
            }),
            "high",
        )
        .await?;
        repo_versions.push(json!({"repository_id": id, "revision_snapshot": "current"}));
    }
    let repo_versions = serde_json::Value::Array(repo_versions);

    let item_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.cognition_item WHERE tenant_id = $1 AND run_id = $2",
    )
    .bind(caller.tenant_id)
    .bind(run_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;
    let item_count = item_count as i32;

    // V1 派生是确定性的：全部条目成立 → completed（编排切片引入部分失败语义）。
    sqlx::query(
        "UPDATE ai.cognition_run SET status = 'completed', item_count = $2, \
                repo_versions = $3, finished_at = now() \
         WHERE tenant_id = $1 AND id = $4",
    )
    .bind(caller.tenant_id)
    .bind(item_count)
    .bind(&repo_versions)
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;

    tx.commit().await.map_err(ApiError::Internal)?;

    Ok(Json(json!({
        "run_id": run_id,
        "status": "completed",
        "item_count": item_count,
    })))
}

#[allow(clippy::too_many_arguments)] // 认知对象落库行写入面，列清单与 pc 表对应
async fn insert_item(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    run_id: Uuid,
    domain: &str,
    content: &str,
    evidence: &serde_json::Value,
    confidence: &str,
) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO ai.cognition_item \
         (tenant_id, project_id, run_id, domain, content, evidence, confidence) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(run_id)
    .bind(domain)
    .bind(content)
    .bind(evidence)
    .bind(confidence)
    .execute(&mut **tx)
    .await
    .map_err(ApiError::Internal)?;
    Ok(())
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

    let mut tx = state.pool.begin().await.map_err(ApiError::Internal)?;

    let item: (String, String, Option<Uuid>) = sqlx::query_as(
        "SELECT state, content, knowledge_id FROM ai.cognition_item \
         WHERE tenant_id = $1 AND project_id = $2 AND id = $3 FOR UPDATE",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .bind(item_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::Internal)?
    .ok_or_else(|| ApiError::NotFound("cognition item not found"))?;

    if item.0 != "proposed" {
        return Err(ApiError::Conflict("cognition item is already decided"));
    }

    // Confirm：落项目知识（Edit & Confirm 的修订正文优先；重复确认 409）。
    let mut knowledge_id = item.2;
    let mut final_content: Option<String> = None;
    if next_state == "confirmed" {
        if knowledge_id.is_some() {
            return Err(ApiError::Conflict(
                "cognition item already produced knowledge",
            ));
        }
        let content = match body.content.as_deref() {
            Some(edited) if !edited.trim().is_empty() && edited.trim() != item.1 => {
                final_content = Some(edited.trim().to_string());
                edited.trim().to_string()
            }
            _ => item.1.clone(),
        };
        knowledge_id = Some(
            sqlx::query_scalar(
                "INSERT INTO ai.project_knowledge \
                 (tenant_id, project_id, domain, content, source_item_id, confirmed_by) \
                 SELECT tenant_id, project_id, domain, $3, id, $4 FROM ai.cognition_item \
                 WHERE tenant_id = $1 AND id = $2 RETURNING id",
            )
            .bind(caller.tenant_id)
            .bind(item_id)
            .bind(&content)
            .bind(caller.principal_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(ApiError::Internal)?,
        );
    }

    sqlx::query(
        "UPDATE ai.cognition_item SET state = $3, decided_by = $4, decided_at = now(), \
                decision_note = $5, knowledge_id = COALESCE($6, knowledge_id), \
                content = COALESCE($7, content) \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(caller.tenant_id)
    .bind(item_id)
    .bind(next_state)
    .bind(caller.principal_id)
    .bind(body.note.as_deref().filter(|n| !n.trim().is_empty()))
    .bind(knowledge_id)
    .bind(&final_content)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::Internal)?;

    if next_state == "confirmed" {
        sqlx::query(
            "UPDATE ai.cognition_run SET confirmed_count = confirmed_count + 1 \
             WHERE tenant_id = $1 AND id = \
               (SELECT run_id FROM ai.cognition_item WHERE tenant_id = $1 AND id = $2)",
        )
        .bind(caller.tenant_id)
        .bind(item_id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Internal)?;
    }

    tx.commit().await.map_err(ApiError::Internal)?;

    Ok(Json(json!({
        "id": item_id,
        "state": next_state,
        "knowledge_id": knowledge_id,
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

    let rows: Vec<KnowledgeRow> = sqlx::query_as(
        "SELECT id, domain, content, version, source_item_id, confirmed_by, confirmed_at \
         FROM ai.project_knowledge \
         WHERE tenant_id = $1 AND project_id = $2 AND status = 'active' \
         ORDER BY confirmed_at DESC",
    )
    .bind(caller.tenant_id)
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(json!({ "items": rows })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct KnowledgeRow {
    id: Uuid,
    domain: String,
    content: String,
    version: i32,
    source_item_id: Option<Uuid>,
    confirmed_by: Uuid,
    confirmed_at: DateTime<Utc>,
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/projects/{project_id}/cognition", get(overview))
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
