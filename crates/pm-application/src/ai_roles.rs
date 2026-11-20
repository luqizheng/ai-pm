//! AI Role 项目启用面（V2 D-1=A，`docs/project-ai-team-v2/
//! ai-pm-ai-team-kickoff-adjudication-pack-v1.md` §二）。
//!
//! 模型（迁移 0058）：全局模板 `ai.ai_role_template` → 项目启用行
//! `ai.project_ai_role` → 不可变版本 `ai.ai_role_version`（append-only）。
//!
//! 语义：
//! - list 是**模板 LEFT JOIN 项目行**的合并视图：存量项目没有启用行时，
//!   视图呈现 `enabled=false, current_version=None`（懒生效，不回填数据）；
//! - enable 且项目行不存在时，同事务建行 + 从模板 `default_definition` 派生
//!   版本 `1.0`（created_by = 操作者）+ `current_version_id` 指向它；
//! - `project_manager` 不允许项目禁用（AT-D12/稿 §3.3，上层映射 409）；
//! - 启停落 `governance.audit_log` 审计行；低频管理动作，不进 outbox（仓库惯例）。
//!
//! V2.0-D Capability Registry V1（迁移 0061，Batch E）：承接解析
//! （[`resolve_role_assignee`]）**registry-only**——候选 = `ai.ai_executor_capability`
//! （capability 词表 = `ai_role_template.role_key`）∩ `ai.project_agent_member`
//! （P4-ORG Phase 4 删 legacy 成员角色映射）；bootstrap 播种见
//! `bootstrap::ensure_executor_capabilities`。

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::append_audit;
use pm_domain::{AiRoleCategory, AI_ROLE_TEMPLATE_SEED_KEYS, AI_ROLE_UNDISABLEABLE_KEYS};

#[derive(Debug, thiserror::Error)]
pub enum AiRoleError {
    /// 项目行不存在（启停目标未启用）。
    #[error("ai role not found")]
    NotFound,
    /// 项目不存在或跨租户（上层 404，不泄露存在性）。
    #[error("project not found")]
    ProjectNotFound,
    /// role_key 在不可禁用清单里（AT-D12；上层映射 409）。
    #[error("ai role `{0}` is not disableable")]
    Undisableable(&'static str),
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// 合并视图：模板行 + 项目启用状态 + 当前版本摘要。
#[derive(Debug, Clone, Serialize)]
pub struct ProjectAiRoleView {
    pub role_id: Uuid,
    pub role_key: String,
    pub name: String,
    pub category: AiRoleCategory,
    /// 项目行缺失时恒为 false（懒生效语义）。
    pub enabled: bool,
    pub current_version: Option<AiRoleVersionSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiRoleVersionSummary {
    pub id: Uuid,
    pub version: String,
    pub created_at: DateTime<Utc>,
}

/// 不可变版本快照（全量 definition；历史只增不改）。
#[derive(Debug, Clone, Serialize)]
pub struct AiRoleVersionView {
    pub id: Uuid,
    pub version: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub definition: serde_json::Value,
}

/// 项目 AI Role 目录（模板 LEFT JOIN 项目行）。
///
/// 不在这里做项目可见性校验（REST 层守卫负责）；跨租户的项目行只是查不到
/// 启用状态，视图退化为全 false，不泄露数据。
pub async fn list_project_roles(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<ProjectAiRoleView>, AiRoleError> {
    fetch_role_views(pool, tenant_id, project_id, None).await
}

/// 某个 Role 的版本历史（append-only，按创建序返回）。
pub async fn list_role_versions(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    role_id: Uuid,
) -> Result<Vec<AiRoleVersionView>, AiRoleError> {
    let rows = sqlx::query(
        "SELECT v.id, v.version, v.created_by, v.created_at, v.definition \
           FROM ai.ai_role_version v \
           JOIN ai.project_ai_role pr ON pr.id = v.project_role_id \
          WHERE pr.tenant_id = $1 AND pr.project_id = $2 AND pr.role_id = $3 \
          ORDER BY v.created_at, v.version",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(role_id)
    .fetch_all(pool)
    .await?;

    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        views.push(AiRoleVersionView {
            id: row.try_get("id")?,
            version: row.try_get("version")?,
            created_by: row.try_get("created_by")?,
            created_at: row.try_get("created_at")?,
            definition: row.try_get("definition")?,
        });
    }
    Ok(views)
}

/// 启停一个项目的 Role（归属与权限校验在 REST 层；这里只保证数据一致）。
///
/// - `enabled=false` 且目标行不存在 = 已是关闭态的幂等 no-op（不建行、不落审计）；
/// - `enabled=true` 且目标行不存在 = 懒生效：同事务建行 + 版本 1.0；
/// - `project_manager` 无论行是否存在都不允许禁用。
pub async fn set_project_role_enabled(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    project_id: Uuid,
    role_id: Uuid,
    enabled: bool,
) -> Result<ProjectAiRoleView, AiRoleError> {
    // 项目必须属于本租户（组合 FK 兜底，这里给可读错误）。
    let project_tenant: Option<Uuid> =
        sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    if project_tenant != Some(tenant_id) {
        return Err(AiRoleError::ProjectNotFound);
    }

    // 模板必须存在，且先于行操作做不可禁用裁决。
    let template: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, role_key FROM ai.ai_role_template WHERE id = $1")
            .bind(role_id)
            .fetch_optional(pool)
            .await?;
    let (template_id, role_key) = template.ok_or(AiRoleError::NotFound)?;
    if !enabled && AI_ROLE_UNDISABLEABLE_KEYS.contains(&role_key.as_str()) {
        // role_key 来自预置目录，收敛到 &'static。
        let key = AI_ROLE_UNDISABLEABLE_KEYS
            .iter()
            .find(|k| **k == role_key)
            .copied()
            .ok_or_else(|| {
                AiRoleError::Validation("undisableable key missing from catalog".into())
            })?;
        return Err(AiRoleError::Undisableable(key));
    }

    let mut tx = pool.begin().await?;

    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM ai.project_ai_role \
         WHERE tenant_id = $1 AND project_id = $2 AND role_id = $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(template_id)
    .fetch_optional(&mut *tx)
    .await?;

    let row_id = match existing {
        Some(id) => {
            sqlx::query(
                "UPDATE ai.project_ai_role SET enabled = $1, updated_at = now() WHERE id = $2",
            )
            .bind(enabled)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            id
        }
        None if enabled => {
            create_project_role_with_initial_version(&mut tx, tenant_id, project_id, template_id)
                .await?
        }
        // 关闭一个从未启用的 Role = 幂等 no-op。
        None => {
            tx.rollback().await?;
            return fetch_role_views(pool, tenant_id, project_id, Some(template_id))
                .await?
                .into_iter()
                .next()
                .ok_or(AiRoleError::NotFound);
        }
    };

    append_audit(
        &mut tx,
        project_id,
        actor,
        if enabled {
            "ai_role.enabled"
        } else {
            "ai_role.disabled"
        },
        "ai_project_role",
        Some(row_id),
    )
    .await?;

    tx.commit().await?;

    fetch_role_views(pool, tenant_id, project_id, Some(template_id))
        .await?
        .into_iter()
        .next()
        .ok_or(AiRoleError::NotFound)
}

/// 建项目启用行 + 从模板 `default_definition` 派生版本 1.0 并指向它
/// （bootstrap 与懒生效共用；调用方负责事务）。
pub(crate) async fn create_project_role_with_initial_version(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    template_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    let row_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.project_ai_role (tenant_id, project_id, role_id, enabled) \
         VALUES ($1, $2, $3, TRUE) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(template_id)
    .fetch_one(&mut **tx)
    .await?;
    let version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.ai_role_version (project_role_id, version, definition) \
         SELECT $1, '1.0', t.default_definition \
           FROM ai.ai_role_template t WHERE t.id = $2 RETURNING id",
    )
    .bind(row_id)
    .bind(template_id)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE ai.project_ai_role SET current_version_id = $1, updated_at = now() WHERE id = $2",
    )
    .bind(version_id)
    .bind(row_id)
    .execute(&mut **tx)
    .await?;
    Ok(row_id)
}

/// bootstrap 种子：为项目补齐 Catalog 首批 5 个 Role（幂等；缺哪行补哪行）。
pub(crate) async fn ensure_project_ai_roles(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<(), sqlx::Error> {
    let existing: Vec<Uuid> = sqlx::query_scalar(
        "SELECT role_id FROM ai.project_ai_role WHERE tenant_id = $1 AND project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(&mut **tx)
    .await?;
    let existing: std::collections::HashSet<Uuid> = existing.into_iter().collect();

    let templates: Vec<(String, Uuid)> =
        sqlx::query_as("SELECT role_key, id FROM ai.ai_role_template WHERE role_key = ANY($1)")
            .bind(&AI_ROLE_TEMPLATE_SEED_KEYS[..])
            .fetch_all(&mut **tx)
            .await?;

    for (_, template_id) in templates {
        if existing.contains(&template_id) {
            continue;
        }
        create_project_role_with_initial_version(tx, tenant_id, project_id, template_id).await?;
    }
    Ok(())
}

/// 合并视图查询（模板 LEFT JOIN 项目行 + 当前版本摘要）。
async fn fetch_role_views<'e, E>(
    executor: E,
    tenant_id: Uuid,
    project_id: Uuid,
    role_id: Option<Uuid>,
) -> Result<Vec<ProjectAiRoleView>, AiRoleError>
where
    E: sqlx::PgExecutor<'e>,
{
    let rows = sqlx::query(
        "SELECT t.id AS role_id, t.role_key, t.name, t.category, \
                pr.enabled AS enabled, \
                v.id AS version_id, v.version AS version_label, \
                v.created_at AS version_created_at \
           FROM ai.ai_role_template t \
           LEFT JOIN ai.project_ai_role pr \
                  ON pr.role_id = t.id AND pr.tenant_id = $1 AND pr.project_id = $2 \
           LEFT JOIN ai.ai_role_version v ON v.id = pr.current_version_id \
          WHERE ($3::uuid IS NULL OR t.id = $3) \
          ORDER BY t.created_at, t.role_key",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(role_id)
    .fetch_all(executor)
    .await?;

    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        let category: String = row.try_get("category")?;
        let category = AiRoleCategory::from_str(&category).ok_or_else(|| {
            AiRoleError::Validation(format!("unknown ai role category `{category}`"))
        })?;
        let enabled: Option<bool> = row.try_get("enabled")?;
        let version_id: Option<Uuid> = row.try_get("version_id")?;
        let version_label: Option<String> = row.try_get("version_label")?;
        let version_created_at: Option<DateTime<Utc>> = row.try_get("version_created_at")?;
        views.push(ProjectAiRoleView {
            role_id: row.try_get("role_id")?,
            role_key: row.try_get("role_key")?,
            name: row.try_get("name")?,
            category,
            enabled: enabled.unwrap_or(false),
            current_version: match (version_id, version_label, version_created_at) {
                (Some(id), Some(label), Some(created_at)) => Some(AiRoleVersionSummary {
                    id,
                    version: label,
                    created_at,
                }),
                _ => None,
            },
        });
    }
    Ok(views)
}

// ---------------------------------------------------------------------------
// D-2=A 调度承接解析（`docs/project-ai-team-v2/
// ai-pm-ai-team-kickoff-adjudication-pack-v1.md` §〇/§三，迁移 0059）
// ---------------------------------------------------------------------------

/// 按 Role 版本解析任务的承接 principal（TaskReadyHandler 调度决策的解析步）。
///
/// **P4-ORG Phase 4（DR-PA-04③ / DR-ORG-08③）**：legacy `ROLE_EXECUTOR_MAP`
/// 与 `core.project_member.role` 职能定向派发已废除——候选只来自
/// `ai.ai_executor_capability`（租户级能力）∩ `ai.project_agent_member`
/// （项目授权行，原 project_member agent 行承载迁移于此）。
///
/// **Registry-only**：候选 = `ai.ai_executor_capability`
/// （tenant_id + capability = role_key + `revoked_at IS NULL`）JOIN
/// `core.principal`（`kind='dev_agent'` 且 active）JOIN `ai.project_agent_member`
/// （该项目存在授权行即可——能力与项目授权两层解耦）。
/// 版本链闸门（版本存在 / 本项目 / enabled）统一前置。
///
/// 选取规则（两条路径一致）：优先「空闲」（无以其为 `delivery.task.assignee_id`
/// 的活跃 delivery run，status IN ('claimed','running')；`task_run.agent_id`
/// 预留列 V1 无生产写入，活跃归因以 task.assignee_id 为准），都忙时取
/// principal_id 最小者（确定性）。无候选 / 版本不存在 / 跨项目 / role 未启用 →
/// `None`（调用方落 `no_executor_available` 等待）。
///
/// 单查询实现（executor 泛型 + 事务内复用，sqlx 的 Executor 是消费式的，两次
/// fetch 会 move）：`role`（版本链闸门）与 `registry`（能力 ∩ 项目授权）两个
/// CTE 在**一条语句**内完成判定与选取。
pub async fn resolve_role_assignee<'e, E>(
    executor: E,
    tenant_id: Uuid,
    project_id: Uuid,
    role_version_id: Uuid,
) -> Result<Option<Uuid>, AiRoleError>
where
    E: sqlx::PgExecutor<'e>,
{
    // 版本不存在、挂到了别的项目、项目行 enabled=false、无合格 dev_agent，
    // 全部收敛为同一结果 None——调度语义里统一是「无合格执行者」，不区分
    // 细分原因（审计 payload 里给恢复 hint）。
    let candidate: Option<Uuid> = sqlx::query_scalar(
        "WITH role AS ( \
             SELECT pr.project_id, t.role_key \
               FROM ai.ai_role_version v \
               JOIN ai.project_ai_role pr ON pr.id = v.project_role_id \
               JOIN ai.ai_role_template t ON t.id = pr.role_id \
              WHERE v.id = $1 AND pr.tenant_id = $2 AND pr.project_id = $3 \
                AND pr.enabled \
         ), registry AS ( \
             SELECT cap.principal_id \
               FROM role r \
               JOIN ai.ai_executor_capability cap \
                 ON cap.tenant_id = $2 AND cap.capability = r.role_key \
                AND cap.revoked_at IS NULL \
               JOIN core.principal p \
                 ON p.id = cap.principal_id AND p.kind = 'dev_agent' AND p.is_active \
               JOIN ai.project_agent_member am \
                 ON am.project_id = r.project_id AND am.principal_id = cap.principal_id \
         ) \
         SELECT registry.principal_id \
           FROM registry \
          ORDER BY EXISTS ( \
               SELECT 1 FROM delivery.task_run tr \
                 JOIN delivery.task tt ON tt.id = tr.task_id \
                WHERE tt.assignee_id = registry.principal_id \
                  AND tr.project_id = $3 \
                  AND tr.run_kind = 'delivery' \
                  AND tr.status IN ('claimed', 'running') \
           ) ASC, \
           registry.principal_id ASC \
          LIMIT 1"
    )
    .bind(role_version_id)
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(executor)
    .await?;

    Ok(candidate)
}
