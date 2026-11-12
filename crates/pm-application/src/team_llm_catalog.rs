//! Team LLM Model Catalog / Authorization / Policy（Phase 3A）。
//!
//! 设计基线：docs/smark-pm-team-llm-provider-management-phase3-design-v1.md
//! §4（数据库）/§5（API 语义）/§6.1（写路径与解析同款严格校验）。
//!
//! 职责边界：
//! - 模型目录（AC-04）：手动登记/更新/启停；自动发现（sync）在 3C 接
//!   adapter `discover_models`，本模块只承载 upsert 面；
//! - 授权（AC-05）：`team_wide`/`revoked` 两态，一模型一行；撤销不删目录、
//!   不改 enabled、不静默改写项目配置（设计稿 §9），响应携带影响面计数；
//! - Team 按 Role 默认策略（AC-06）：七 Role（D-P3-1 词表，general 兜底）；
//! - 项目 Role 覆盖（AC-08）：删行 = 恢复继承 Team；现
//!   `project_ai_model_policy`（Provider 约束）语义零改动（见 team_llm.rs）。
//!
//! 写路径统一 [`ensure_model_usable`]：模型 enabled ∧ team_wide 授权 ∧
//! Provider enabled——与解析链（3B `resolve_model_for_role`）同款严格口径，
//! 禁止把不可用模型写进任何策略面（AC-07 的 API 侧防线）。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::team_llm::{decrypt_api_key_bytes, ResolvedProvider, TeamLlmError};

/// 模型策略 Role 词表（任务书 §3 裁决 D-P3-1：设计稿 §10.1 七 Role 的
/// snake_case；与项目执行职能词表 `ai_role.rs` 正交）。
pub const ROLE_KEYS: [&str; 7] = [
    "general",
    "ai_pm_lead",
    "requirement_analysis",
    "task_analysis",
    "risk_analysis",
    "architecture_analysis",
    "knowledge_management",
];

/// 模型能力词表（设计稿 §8.2 六能力；未知能力 = 空数组，禁止编造）。
pub const CAPABILITIES: [&str; 6] = [
    "text_generation",
    "reasoning",
    "structured_output",
    "tool_calling",
    "vision",
    "long_context",
];

// ─────────────────────────────────────────────────────────── 读取行

#[derive(Debug, Serialize)]
pub struct ModelRow {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub provider_name: String,
    pub model_id: String,
    pub display_name: String,
    pub capabilities: Vec<String>,
    pub context_window: Option<i32>,
    pub enabled: bool,
    pub source: String,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AuthorizationRow {
    pub model_id: Uuid,
    pub authorization: String,
    pub granted_at: DateTime<Utc>,
    pub revision: i64,
    /// 撤销影响面（§5.4：只报告不自动改写）：引用该模型的 Team 策略行数。
    pub team_policy_refs: i64,
    /// 引用该模型的项目覆盖行数。
    pub project_override_refs: i64,
}

#[derive(Debug, Serialize)]
pub struct PolicyRow {
    pub role_key: String,
    pub model_id: Option<Uuid>,
    pub model_display_name: Option<String>,
    pub provider_name: Option<String>,
    /// 未配置（无行）→ 解析将继承 general（Team general 行也缺失时解析报
    /// general_policy_missing，前端可提示配置缺口）。
    pub inherit_general: bool,
}

#[derive(Debug, Serialize)]
pub struct ProjectPolicyView {
    /// 项目 Provider 约束（现 ai.project_ai_model_policy 单跳，语义不变）。
    pub provider_constraint: Option<Uuid>,
    /// 约束 Provider 名（UI 03/05 显示）。
    pub provider_name: Option<String>,
    /// 约束行 CAS 版本（无行 = None；PUT provider_id 时作 expected_revision）。
    pub provider_constraint_revision: Option<i64>,
    pub overrides: Vec<PolicyRow>,
    /// Team 侧当前策略（覆盖继承的解析来源）。
    pub team_default: Vec<PolicyRow>,
}

// ─────────────────────────────────────────────────────────── 请求

#[derive(Debug, Clone, Deserialize)]
pub struct CreateModelRequest {
    pub provider_id: Uuid,
    pub model_id: String,
    pub display_name: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub context_window: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateModelRequest {
    pub expected_revision: i64,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub capabilities: Option<Vec<String>>,
    #[serde(default)]
    pub context_window: Option<Option<i32>>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthorizationItem {
    pub model_id: Uuid,
    /// grant | revoke
    pub action: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolicyItem {
    pub role_key: String,
    /// None = 删行（Team 策略：未配置 Role 继承 general；general 行删除 =
    /// 解析末级显式 Configuration Error）。
    pub model_id: Option<Uuid>,
    #[serde(default)]
    pub expected_revision: Option<i64>,
}

// ─────────────────────────────────────────────────────────── 审计

/// 目录/策略事件审计（SQL 单源：`pm_write::insert_tenant_audit`，批 A2 归并；
/// target_type 走通用列：`llm_model` / `llm_team_policy` / `llm_project_policy`）。
async fn insert_catalog_audit(
    tx: &mut Transaction<'static, Postgres>,
    tenant_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_id: Uuid,
    after: Option<serde_json::Value>,
) -> Result<(), sqlx::Error> {
    crate::pm_write::insert_tenant_audit(
        tx,
        tenant_id,
        actor,
        action,
        "llm_model",
        target_id,
        None,
        after,
    )
    .await
}

// ─────────────────────────────────────────────────────────── 读取

const MODEL_SELECT: &str = "SELECT m.id, m.provider_id, p.name AS provider_name, m.model_id, \
    m.display_name, m.capabilities, m.context_window, m.enabled, m.source, m.last_verified_at, \
    m.revision, m.created_by, m.created_at, m.updated_at \
    FROM ai.team_llm_model m JOIN ai.team_llm_provider p ON p.id = m.provider_id AND p.tenant_id = m.tenant_id";

// 14 列投影元组，行结构由 MODEL_SELECT 固定（定义见下方 ModelTuple）。
fn map_model_row(row: ModelTuple) -> ModelRow {
    ModelRow {
        id: row.0,
        provider_id: row.1,
        provider_name: row.2,
        model_id: row.3,
        display_name: row.4,
        capabilities: row.5,
        context_window: row.6,
        enabled: row.7,
        source: row.8,
        last_verified_at: row.9,
        revision: row.10,
        created_by: row.11,
        created_at: row.12,
        updated_at: row.13,
    }
}

type ModelTuple = (
    Uuid,
    Uuid,
    String,
    String,
    String,
    Vec<String>,
    Option<i32>,
    bool,
    String,
    Option<DateTime<Utc>>,
    i64,
    Uuid,
    DateTime<Utc>,
    DateTime<Utc>,
);

async fn get_model_row(
    executor: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
    model_row_id: Uuid,
) -> Result<ModelRow, TeamLlmError> {
    let row: ModelTuple = sqlx::query_as(&format!(
        "{MODEL_SELECT} WHERE m.tenant_id = $1 AND m.id = $2"
    ))
    .bind(tenant_id)
    .bind(model_row_id)
    .fetch_optional(executor)
    .await?
    .ok_or(TeamLlmError::NotFound)?;
    Ok(map_model_row(row))
}

/// 模型目录列表（provider_id 过滤可选；q 模糊匹配 display_name/model_id）。
pub async fn list_models(
    pool: &PgPool,
    tenant_id: Uuid,
    provider_id: Option<Uuid>,
) -> Result<Vec<ModelRow>, TeamLlmError> {
    let sql = match provider_id {
        Some(_) => format!(
            "{MODEL_SELECT} WHERE m.tenant_id = $1 AND m.provider_id = $2 ORDER BY p.name, m.model_id"
        ),
        None => format!("{MODEL_SELECT} WHERE m.tenant_id = $1 ORDER BY p.name, m.model_id"),
    };
    let mut query = sqlx::query_as::<_, ModelTuple>(&sql);
    if let Some(pid) = provider_id {
        query = query.bind(tenant_id).bind(pid);
    } else {
        query = query.bind(tenant_id);
    }
    let rows = query.fetch_all(pool).await?;
    Ok(rows.into_iter().map(map_model_row).collect())
}

// ─────────────────────────────────────────────────────────── 目录写路径

fn validate_capabilities(capabilities: &[String]) -> Result<(), TeamLlmError> {
    for cap in capabilities {
        if !CAPABILITIES.contains(&cap.as_str()) {
            return Err(TeamLlmError::Validation("unknown capability"));
        }
    }
    Ok(())
}

async fn ensure_provider_writable(
    tx: &mut Transaction<'static, Postgres>,
    tenant_id: Uuid,
    provider_id: Uuid,
) -> Result<(), TeamLlmError> {
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM ai.team_llm_provider WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(provider_id)
    .fetch_optional(&mut **tx)
    .await?;
    match status.as_deref() {
        None => Err(TeamLlmError::NotFound),
        Some("archived") => {
            Err(TeamLlmError::StatusTransition("provider is archived"))
        }
        Some(_) => Ok(()),
    }
}

/// 手动登记模型（AC-04；自动发现走 3C sync upsert）。
pub async fn create_model(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    req: CreateModelRequest,
) -> Result<ModelRow, TeamLlmError> {
    let model_id = req.model_id.trim();
    let display_name = req.display_name.trim();
    if model_id.is_empty() || display_name.is_empty() {
        return Err(TeamLlmError::Validation("model_id and display_name are required"));
    }
    if let Some(w) = req.context_window {
        if w <= 0 {
            return Err(TeamLlmError::Validation("context_window must be positive"));
        }
    }
    validate_capabilities(&req.capabilities)?;

    let mut tx = pool.begin().await?;
    ensure_provider_writable(&mut tx, tenant_id, req.provider_id).await?;

    // 重名显式拒绝（UNIQUE(tenant, provider, model_id) 的应用层前置）
    let exists: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM ai.team_llm_model \
         WHERE tenant_id = $1 AND provider_id = $2 AND model_id = $3",
    )
    .bind(tenant_id)
    .bind(req.provider_id)
    .bind(model_id)
    .fetch_optional(&mut *tx)
    .await?;
    if exists.is_some() {
        return Err(TeamLlmError::Validation(
            "model already registered for this provider",
        ));
    }

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.team_llm_model \
         (tenant_id, provider_id, model_id, display_name, capabilities, context_window, \
          enabled, source, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, false, 'manual', $7) RETURNING id",
    )
    .bind(tenant_id)
    .bind(req.provider_id)
    .bind(model_id)
    .bind(display_name)
    .bind(&req.capabilities)
    .bind(req.context_window)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;

    insert_catalog_audit(
        &mut tx,
        tenant_id,
        actor,
        "llm_model.created",
        id,
        Some(json!({ "provider_id": req.provider_id, "model_id": model_id })),
    )
    .await?;
    tx.commit().await?;

    get_model_row(pool, tenant_id, id).await
}

/// 更新模型（CAS；model_id 为身份键不可改）。
pub async fn update_model(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    model_row_id: Uuid,
    req: UpdateModelRequest,
) -> Result<ModelRow, TeamLlmError> {
    if let Some(caps) = &req.capabilities {
        validate_capabilities(caps)?;
    }
    if let Some(Some(w)) = req.context_window {
        if w <= 0 {
            return Err(TeamLlmError::Validation("context_window must be positive"));
        }
    }
    let mut tx = pool.begin().await?;

    let revision: Option<i64> = sqlx::query_scalar(
        "SELECT revision FROM ai.team_llm_model WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(model_row_id)
    .fetch_optional(&mut *tx)
    .await?;
    let revision = revision.ok_or(TeamLlmError::NotFound)?;
    if revision != req.expected_revision {
        return Err(TeamLlmError::RevisionConflict {
            expected: req.expected_revision,
            actual: revision,
        });
    }

    if let Some(display_name) = req.display_name.as_deref() {
        if display_name.trim().is_empty() {
            return Err(TeamLlmError::Validation("display_name cannot be empty"));
        }
        sqlx::query("UPDATE ai.team_llm_model SET display_name = $1 WHERE tenant_id = $2 AND id = $3")
            .bind(display_name.trim())
            .bind(tenant_id)
            .bind(model_row_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(caps) = &req.capabilities {
        sqlx::query("UPDATE ai.team_llm_model SET capabilities = $1 WHERE tenant_id = $2 AND id = $3")
            .bind(caps)
            .bind(tenant_id)
            .bind(model_row_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(ctx) = req.context_window {
        sqlx::query("UPDATE ai.team_llm_model SET context_window = $1 WHERE tenant_id = $2 AND id = $3")
            .bind(ctx)
            .bind(tenant_id)
            .bind(model_row_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(enabled) = req.enabled {
        sqlx::query("UPDATE ai.team_llm_model SET enabled = $1 WHERE tenant_id = $2 AND id = $3")
            .bind(enabled)
            .bind(tenant_id)
            .bind(model_row_id)
            .execute(&mut *tx)
            .await?;
        insert_catalog_audit(
            &mut tx,
            tenant_id,
            actor,
            if enabled { "llm_model.enabled" } else { "llm_model.disabled" },
            model_row_id,
            Some(json!({ "enabled": enabled })),
        )
        .await?;
    }
    sqlx::query(
        "UPDATE ai.team_llm_model SET revision = revision + 1, updated_at = now() \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(model_row_id)
    .execute(&mut *tx)
    .await?;

    let action = if req.enabled.is_some() {
        "llm_model.updated_with_status"
    } else {
        "llm_model.updated"
    };
    insert_catalog_audit(&mut tx, tenant_id, actor, action, model_row_id, None).await?;
    tx.commit().await?;

    get_model_row(pool, tenant_id, model_row_id).await
}

// ─────────────────────────────────────────────────────────── 授权（AC-05）

/// 批量授权/撤销。grant → `team_wide`（幂等 upsert）；revoke → `revoked`
/// （无行时也建行：授权历史连续）。撤销响应携带影响面计数（只报告，
/// 不自动改写策略/覆盖——设计稿 §9）。
pub async fn upsert_authorizations(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    items: Vec<AuthorizationItem>,
) -> Result<Vec<AuthorizationRow>, TeamLlmError> {
    let mut tx = pool.begin().await?;
    let mut rows = Vec::with_capacity(items.len());

    for item in items {
        let state = match item.action.as_str() {
            "grant" => "team_wide",
            "revoke" => "revoked",
            _ => return Err(TeamLlmError::Validation("action must be grant or revoke")),
        };
        let exists: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM ai.team_llm_model WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(item.model_id)
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_none() {
            return Err(TeamLlmError::NotFound);
        }

        let row: (String, DateTime<Utc>, i64) = sqlx::query_as(
            "INSERT INTO ai.team_llm_model_authorization \
             (tenant_id, model_id, state, granted_by) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (tenant_id, model_id) DO UPDATE SET \
             state = EXCLUDED.state, granted_by = EXCLUDED.granted_by, \
             granted_at = now(), revision = team_llm_model_authorization.revision + 1, \
             updated_at = now() \
             RETURNING state, granted_at, revision",
        )
        .bind(tenant_id)
        .bind(item.model_id)
        .bind(state)
        .bind(actor.principal_id)
        .fetch_one(&mut *tx)
        .await?;

        let (team_policy_refs, project_override_refs): (i64, i64) = sqlx::query_as(
            "SELECT \
             (SELECT count(*) FROM ai.team_llm_model_policy WHERE tenant_id = $1 AND model_id = $2), \
             (SELECT count(*) FROM ai.project_ai_model_override WHERE tenant_id = $1 AND model_id = $2)",
        )
        .bind(tenant_id)
        .bind(item.model_id)
        .fetch_one(&mut *tx)
        .await?;

        insert_catalog_audit(
            &mut tx,
            tenant_id,
            actor,
            "llm_model_authorization.changed",
            item.model_id,
            Some(json!({
                "action": item.action,
                "team_policy_refs": team_policy_refs,
                "project_override_refs": project_override_refs,
            })),
        )
        .await?;

        rows.push(AuthorizationRow {
            model_id: item.model_id,
            authorization: row.0,
            granted_at: row.1,
            revision: row.2,
            team_policy_refs,
            project_override_refs,
        });
    }

    tx.commit().await?;
    Ok(rows)
}

// ─────────────────────────────────────────────────────────── 严格可用性校验

/// 写路径与解析链同款口径（§6.1）：enabled ∧ team_wide ∧ Provider enabled。
/// 返回 provider_id 供约束校验（§6.2）。
async fn ensure_model_usable(
    tx: &mut Transaction<'static, Postgres>,
    tenant_id: Uuid,
    model_row_id: Uuid,
) -> Result<Uuid, TeamLlmError> {
    let row: Option<(bool, Option<String>, String)> = sqlx::query_as(
        "SELECT m.enabled, a.state, p.status \
         FROM ai.team_llm_model m \
         LEFT JOIN ai.team_llm_model_authorization a \
           ON a.tenant_id = m.tenant_id AND a.model_id = m.id \
         JOIN ai.team_llm_provider p ON p.id = m.provider_id AND p.tenant_id = m.tenant_id \
         WHERE m.tenant_id = $1 AND m.id = $2",
    )
    .bind(tenant_id)
    .bind(model_row_id)
    .fetch_optional(&mut **tx)
    .await?;
    let (enabled, authorization, provider_status) = row.ok_or(TeamLlmError::NotFound)?;
    if !enabled {
        return Err(TeamLlmError::Validation("model is disabled"));
    }
    if authorization.as_deref() != Some("team_wide") {
        return Err(TeamLlmError::Validation("model is not authorized for projects"));
    }
    if provider_status != "enabled" {
        return Err(TeamLlmError::Validation("model's provider is not enabled"));
    }
    let provider_id: Uuid = sqlx::query_scalar(
        "SELECT provider_id FROM ai.team_llm_model WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(model_row_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(provider_id)
}

// ─────────────────────────────────────────────────────────── Team 策略（AC-06）

const POLICY_SELECT: &str = "SELECT pol.role_key, m.id, m.display_name, p.name \
    FROM ai.team_llm_model_policy pol \
    JOIN ai.team_llm_model m ON m.id = pol.model_id AND m.tenant_id = pol.tenant_id \
    JOIN ai.team_llm_provider p ON p.id = m.provider_id AND p.tenant_id = m.tenant_id";

/// 七 Role 全量视图：无行 Role 的 `inherit_general = true`（general 行自身
/// 恒为 false）。
pub async fn get_team_policies(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Vec<PolicyRow>, TeamLlmError> {
    let configured: Vec<(String, Uuid, String, String)> = sqlx::query_as(&format!(
        "{POLICY_SELECT} WHERE pol.tenant_id = $1"
    ))
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let general_exists = configured.iter().any(|(role, ..)| role == "general");
    let mut rows: Vec<PolicyRow> = configured
        .into_iter()
        .map(|(role_key, id, display, provider)| PolicyRow {
            role_key,
            model_id: Some(id),
            model_display_name: Some(display),
            provider_name: Some(provider),
            inherit_general: false,
        })
        .collect();
    for role in ROLE_KEYS {
        if !rows.iter().any(|r| r.role_key == role) {
            rows.push(PolicyRow {
                role_key: role.to_string(),
                model_id: None,
                model_display_name: None,
                provider_name: None,
                inherit_general: role != "general" && !general_exists
                    || role != "general" && general_exists,
            });
        }
    }
    // 无行 Role：general 恒 false（自身无继承语义）；其余 = true（§6 解析继承）
    for row in rows.iter_mut() {
        if row.model_id.is_none() {
            row.inherit_general = row.role_key != "general";
        }
    }
    rows.sort_by_key(|r| {
        ROLE_KEYS
            .iter()
            .position(|k| *k == r.role_key)
            .unwrap_or(usize::MAX)
    });
    Ok(rows)
}

/// 批量设置 Team 策略（幂等 PUT；model_id=None 删行恢复继承）。
pub async fn set_team_policies(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    items: Vec<PolicyItem>,
) -> Result<Vec<PolicyRow>, TeamLlmError> {
    let mut tx = pool.begin().await?;
    for item in &items {
        if !ROLE_KEYS.contains(&item.role_key.as_str()) {
            return Err(TeamLlmError::Validation("unknown role_key"));
        }
        match item.model_id {
            Some(model_row_id) => {
                ensure_model_usable(&mut tx, tenant_id, model_row_id).await?;
                sqlx::query(
                    "INSERT INTO ai.team_llm_model_policy \
                     (tenant_id, role_key, model_id, updated_by) \
                     VALUES ($1, $2, $3, $4) \
                     ON CONFLICT (tenant_id, role_key) DO UPDATE SET \
                     model_id = EXCLUDED.model_id, updated_by = EXCLUDED.updated_by, \
                     revision = team_llm_model_policy.revision + 1, updated_at = now()",
                )
                .bind(tenant_id)
                .bind(&item.role_key)
                .bind(model_row_id)
                .bind(actor.principal_id)
                .execute(&mut *tx)
                .await?;
            }
            None => {
                if let Some(expected) = item.expected_revision {
                    let revision: Option<i64> = sqlx::query_scalar(
                        "SELECT revision FROM ai.team_llm_model_policy \
                         WHERE tenant_id = $1 AND role_key = $2",
                    )
                    .bind(tenant_id)
                    .bind(&item.role_key)
                    .fetch_optional(&mut *tx)
                    .await?;
                    if revision != Some(expected) {
                        return Err(TeamLlmError::RevisionConflict {
                            expected,
                            actual: revision.unwrap_or_default(),
                        });
                    }
                }
                sqlx::query(
                    "DELETE FROM ai.team_llm_model_policy WHERE tenant_id = $1 AND role_key = $2",
                )
                .bind(tenant_id)
                .bind(&item.role_key)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    insert_catalog_audit(
        &mut tx,
        tenant_id,
        actor,
        "llm_team_policy.changed",
        tenant_id,
        Some(json!({
            "roles": items.iter().map(|i| json!({
                "role_key": i.role_key,
                "model_id": i.model_id,
            })).collect::<Vec<_>>(),
        })),
    )
    .await?;
    tx.commit().await?;

    get_team_policies(pool, tenant_id).await
}

// ─────────────────────────────────────────────────────────── 项目覆盖（AC-08）

/// 项目策略视图：Provider 约束（现表）+ Role 覆盖 + Team 默认参照。
pub async fn get_project_ai_policies(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<ProjectPolicyView, TeamLlmError> {
    let constraint: Option<(Uuid, String, i64)> = sqlx::query_as(
        "SELECT pol.provider_id, p.name, pol.revision \
         FROM ai.project_ai_model_policy pol \
         JOIN ai.team_llm_provider p ON p.id = pol.provider_id AND p.tenant_id = pol.tenant_id \
         WHERE pol.tenant_id = $1 AND pol.project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let (provider_constraint, provider_name, provider_constraint_revision) = match constraint {
        Some((pid, name, revision)) => (Some(pid), Some(name), Some(revision)),
        None => (None, None, None),
    };
    let overrides = project_override_rows(pool, tenant_id, project_id).await?;
    let team_default = get_team_policies(pool, tenant_id).await?;
    Ok(ProjectPolicyView {
        provider_constraint,
        provider_name,
        provider_constraint_revision,
        overrides,
        team_default,
    })
}

async fn project_override_rows(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<PolicyRow>, TeamLlmError> {
    // 覆盖行直接 JOIN 模型（覆盖可指向未进 Team 策略的模型，不走策略表）
    let rows: Vec<(String, Uuid, String, String)> = sqlx::query_as(
        "SELECT o.role_key, m.id, m.display_name, p.name \
         FROM ai.project_ai_model_override o \
         JOIN ai.team_llm_model m ON m.id = o.model_id AND m.tenant_id = o.tenant_id \
         JOIN ai.team_llm_provider p ON p.id = m.provider_id AND p.tenant_id = m.tenant_id \
         WHERE o.tenant_id = $1 AND o.project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(role_key, id, display, provider)| PolicyRow {
            role_key,
            model_id: Some(id),
            model_display_name: Some(display),
            provider_name: Some(provider),
            inherit_general: false,
        })
        .collect())
}

/// 批量设置项目 Role 覆盖（model_id=None 删行恢复继承 Team）。
/// 项目有 Provider 约束行时，覆盖模型必须属于该 Provider（§6.2 严格校验）。
pub async fn set_project_overrides(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    project_id: Uuid,
    items: Vec<PolicyItem>,
) -> Result<ProjectPolicyView, TeamLlmError> {
    let mut tx = pool.begin().await?;

    let constraint: Option<Uuid> = sqlx::query_scalar(
        "SELECT provider_id FROM ai.project_ai_model_policy \
         WHERE tenant_id = $1 AND project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;

    for item in &items {
        if !ROLE_KEYS.contains(&item.role_key.as_str()) {
            return Err(TeamLlmError::Validation("unknown role_key"));
        }
        match item.model_id {
            Some(model_row_id) => {
                let provider_id =
                    ensure_model_usable(&mut tx, tenant_id, model_row_id).await?;
                if let Some(constrained) = constraint {
                    if provider_id != constrained {
                        return Err(TeamLlmError::Validation(
                            "model's provider violates the project provider constraint",
                        ));
                    }
                }
                sqlx::query(
                    "INSERT INTO ai.project_ai_model_override \
                     (tenant_id, project_id, role_key, model_id, updated_by) \
                     VALUES ($1, $2, $3, $4, $5) \
                     ON CONFLICT (tenant_id, project_id, role_key) DO UPDATE SET \
                     model_id = EXCLUDED.model_id, updated_by = EXCLUDED.updated_by, \
                     revision = project_ai_model_override.revision + 1, updated_at = now()",
                )
                .bind(tenant_id)
                .bind(project_id)
                .bind(&item.role_key)
                .bind(model_row_id)
                .bind(actor.principal_id)
                .execute(&mut *tx)
                .await?;
            }
            None => {
                sqlx::query(
                    "DELETE FROM ai.project_ai_model_override \
                     WHERE tenant_id = $1 AND project_id = $2 AND role_key = $3",
                )
                .bind(tenant_id)
                .bind(project_id)
                .bind(&item.role_key)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    insert_catalog_audit(
        &mut tx,
        tenant_id,
        actor,
        "llm_provider.project_policy_changed",
        tenant_id,
        Some(json!({
            "project_id": project_id,
            "overrides": items.iter().map(|i| json!({
                "role_key": i.role_key,
                "model_id": i.model_id,
            })).collect::<Vec<_>>(),
        })),
    )
    .await?;
    tx.commit().await?;

    get_project_ai_policies(pool, tenant_id, project_id).await
}
// ─────────────────────────────────────────────────────────── 解析链（AC-09/§6）

/// 模型解析失败（§6.1 reason 词表）：业务性 Configuration Error 与基础设施
/// 失败分轨——上层据此产出显式配置错误（禁止降级）或留痕告警。
#[derive(Debug, thiserror::Error)]
pub enum ModelResolutionError {
    #[error("{reason}: {detail}")]
    Configuration { reason: &'static str, detail: String },
    #[error(transparent)]
    Infrastructure(#[from] sqlx::Error),
}

/// 四级解析的行取数 JOIN（三级 source 表不同，JOIN 面同构）。
const RESOLVE_JOINS: &str = "JOIN ai.team_llm_model m ON m.id = o.model_id AND m.tenant_id = o.tenant_id \
    JOIN ai.team_llm_provider p ON p.id = m.provider_id AND p.tenant_id = m.tenant_id \
    LEFT JOIN ai.team_llm_model_authorization a ON a.tenant_id = m.tenant_id AND a.model_id = m.id \
    LEFT JOIN ai.team_llm_credential c ON c.id = p.credential_id";

type ResolveRowTuple = (
    Uuid,            // m.id（model_ref）
    Uuid,            // p.id（provider_id）
    String,          // p.endpoint
    String,          // m.model_id
    String,          // p.provider_type
    Option<Vec<u8>>, // c.api_key_ciphertext
    i32,             // p.timeout_secs
    bool,            // m.enabled
    Option<String>,  // a.state（授权状态）
    String,          // p.status
);

/// 四级解析链（任务书 §6.1，设计稿 §10.3 完整兑现）：
/// 项目 Role 覆盖 → Team Role 策略 → Team general → 显式 Configuration Error。
/// **命中即严格校验（enabled ∧ team_wide ∧ provider enabled ∧ 凭证可用 ∧
/// 项目 Provider 约束），任一失败即 Configuration Error，不回退下一级**
/// （DEC-LLM-004 Fail-Fast；reason 词表见 ModelResolutionError）。
pub async fn resolve_model_for_role(
    pool: &PgPool,
    project_id: Uuid,
    role_key: &str,
) -> Result<ResolvedProvider, ModelResolutionError> {
    if !ROLE_KEYS.contains(&role_key) {
        return Err(ModelResolutionError::Configuration {
            reason: "general_policy_missing",
            detail: format!("unknown role_key {role_key:?}"),
        });
    }
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await?
        .ok_or(ModelResolutionError::Configuration {
            reason: "no_model_for_role",
            detail: format!("project {project_id} not found"),
        })?;

    // 第 1 级：项目 Role 覆盖
    let override_row: Option<ResolveRowTuple> = sqlx::query_as(&format!(
        "SELECT m.id, p.id, p.endpoint, m.model_id, p.provider_type, c.api_key_ciphertext, \
         p.timeout_secs, m.enabled, a.state, p.status \
         FROM ai.project_ai_model_override o \
         {RESOLVE_JOINS} \
         WHERE o.tenant_id = $1 AND o.role_key = $2 AND o.project_id = $3"
    ))
    .bind(tenant_id)
    .bind(role_key)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    if let Some(row) = override_row {
        return validate_and_build_resolved(pool, tenant_id, project_id, row).await;
    }

    // 第 2 级：Team Role 策略
    let policy_row: Option<ResolveRowTuple> = sqlx::query_as(&format!(
        "SELECT m.id, p.id, p.endpoint, m.model_id, p.provider_type, c.api_key_ciphertext, \
         p.timeout_secs, m.enabled, a.state, p.status \
         FROM ai.team_llm_model_policy o \
         {RESOLVE_JOINS} \
         WHERE o.tenant_id = $1 AND o.role_key = $2"
    ))
    .bind(tenant_id)
    .bind(role_key)
    .fetch_optional(pool)
    .await?;
    if let Some(row) = policy_row {
        return validate_and_build_resolved(pool, tenant_id, project_id, row).await;
    }

    // 第 3 级：Team general（兜底）
    let general_row: Option<ResolveRowTuple> = sqlx::query_as(&format!(
        "SELECT m.id, p.id, p.endpoint, m.model_id, p.provider_type, c.api_key_ciphertext, \
         p.timeout_secs, m.enabled, a.state, p.status \
         FROM ai.team_llm_model_policy o \
         {RESOLVE_JOINS} \
         WHERE o.tenant_id = $1 AND o.role_key = 'general'"
    ))
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = general_row else {
        return Err(ModelResolutionError::Configuration {
            reason: "general_policy_missing",
            detail: format!(
                "no model override, no '{role_key}' policy, and no 'general' policy is configured; \
                 configure one in Team Settings → AI Configuration"
            ),
        });
    };
    validate_and_build_resolved(pool, tenant_id, project_id, row).await
}

/// 命中行的严格校验（§6.1）+ 凭证解密 + 项目 Provider 约束（§6.2）。
async fn validate_and_build_resolved(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    row: ResolveRowTuple,
) -> Result<ResolvedProvider, ModelResolutionError> {
    let (
        model_ref,
        provider_id,
        endpoint,
        model,
        provider_type,
        ciphertext,
        timeout_secs,
        enabled,
        authorization,
        provider_status,
    ) = row;
    if !enabled {
        return Err(ModelResolutionError::Configuration {
            reason: "model_disabled",
            detail: format!("model {model:?} is disabled"),
        });
    }
    if authorization.as_deref() != Some("team_wide") {
        return Err(ModelResolutionError::Configuration {
            reason: "authorization_revoked",
            detail: format!("model {model:?} is not authorized for projects"),
        });
    }
    if provider_status != "enabled" {
        return Err(ModelResolutionError::Configuration {
            reason: "provider_not_enabled",
            detail: format!("model {model:?}'s provider is not enabled"),
        });
    }
    let api_key = match ciphertext {
        Some(bytes) => {
            decrypt_api_key_bytes(&bytes).map_err(|_| ModelResolutionError::Configuration {
                reason: "credential_missing",
                detail: "api key could not be decrypted".to_string(),
            })?
        }
        None if matches!(provider_type.as_str(), "openai_compatible" | "ollama") => String::new(),
        None => {
            return Err(ModelResolutionError::Configuration {
                reason: "credential_missing",
                detail: "provider has no credential configured".to_string(),
            })
        }
    };
    // 项目 Provider 约束（§6.2：现 project_ai_model_policy 单跳降级为候选
    // 过滤器；约束行存在时解析结果的 Provider 必须一致，显式失败不静默换）
    let constraint: Option<Uuid> = sqlx::query_scalar(
        "SELECT provider_id FROM ai.project_ai_model_policy \
         WHERE tenant_id = $1 AND project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    if let Some(constrained) = constraint {
        if provider_id != constrained {
            return Err(ModelResolutionError::Configuration {
                reason: "provider_constraint_violation",
                detail: format!(
                    "resolved model {model:?} lives on a different provider than the project constraint"
                ),
            });
        }
    }
    Ok(ResolvedProvider {
        provider_id,
        endpoint,
        model,
        api_key,
        timeout_secs,
        model_ref: Some(model_ref),
    })
}