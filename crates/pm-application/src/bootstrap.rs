//! Project bootstrap (Task 8): idempotent project creation plus the full
//! principal / AI-configuration / skill-binding set a project needs before it
//! can receive events.
//!
//! One call creates, in a single transaction:
//!
//! - a `core.customer` — **only** on the legacy path
//!   ([`bootstrap_project`] / [`bootstrap_project_with_status`]), where the
//!   customer is looked up by `name = project code` and created when missing.
//!   [`bootstrap_project_for_customer`] instead requires an existing customer
//!   id maintained through `/v1/pm/customers` (0017 客户主数据),
//! - five `core.principal` rows — the human owner, the AI PM and three dev
//!   agents (architect / developer / tester) — with code-suffixed display
//!   names. On the owned path ([`bootstrap_project_owned`] /
//!   [`bootstrap_project_for_customer_owned`], T-022) the synthetic human is
//!   skipped and the real caller takes the owner slot,
//! - the `core.project` row (`status = 'planning'`, `code` UNIQUE),
//! - five `core.project_member` rows (human → `owner`, ai_pm → `ai_pm`,
//!   dev_agent → `architect` / `developer` / `tester`),
//! - the `ai.project_ai_config` row (fake model, `ai_lead`, policy version 1)
//!   bound to the AI PM principal, and
//! - five `ai.project_skill_binding` rows pinning every 0003-seeded skill at
//!   `v1`.
//!
//! A second call with the same project code fails with
//! [`BootstrapError::Duplicate`] — `core.project.code` is UNIQUE; bootstrap
//! never upserts and never produces a second project.

use pm_domain::{PrincipalId, ProjectId};
use sqlx::PgPool;
use uuid::Uuid;

/// 项目创建时的归属主体：真实调用者（T-022）或兼容路径的合成 owner。
#[derive(Debug, Clone, Copy)]
enum ProjectOwner {
    /// 兼容路径：新建 `owner-<code>` 合成 principal（测试与旧调用方）。
    Synthetic,
    /// 真实调用者：不造合成 principal，直接写 `project_member(role='owner')`。
    Principal(PrincipalId),
}

/// Everything [`bootstrap_project`] needs: the unique project code and its
/// display name, plus the owning tenant (T-021b：根表由调用方显式给租户).
pub struct BootstrapRequest {
    pub project_code: String,
    pub project_name: String,
    pub tenant_id: Uuid,
}

/// The ids a successful bootstrap created (or reused, for the customer).
#[derive(Debug, Clone)]
pub struct BootstrapOutcome {
    pub customer_id: Uuid,
    pub project_id: ProjectId,
    pub ai_principal_id: PrincipalId,
    /// 兼容旧字段名：P0 起指向 developer principal。
    pub dev_agent_id: PrincipalId,
    pub developer_id: PrincipalId,
    pub architect_id: PrincipalId,
    pub tester_id: PrincipalId,
    pub human_id: PrincipalId,
}

#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    #[error("project code `{0}` already exists")]
    Duplicate(String),
    /// 指定客户不存在，或已归档（0017 起建项必须挂在一个可用的客户上）。
    #[error("customer `{0}` not found or archived")]
    CustomerNotFound(Uuid),
    #[error("database error: {0}")]
    Db(sqlx::Error),
}

/// 客户归属来源：主数据（0017 起）或隐式占位（0017 之前的兼容路径）。
enum CustomerTarget {
    Existing(Uuid),
    Implicit,
}

/// The five skill definitions seeded by `migrations/0003_platform.sql`. Every
/// bootstrap binds all of them at their seed version.
const SKILL_IDS: [&str; 5] = [
    "requirement_analysis",
    "task_analysis",
    "risk_detection",
    "architecture_consistency",
    "scheduling_orchestration",
];

/// **兼容路径**：客户按 `name = project code` 隐式创建（建为 `placeholder`）。
///
/// 0017 起新代码应改用 [`bootstrap_project_for_customer`]，把项目挂到经
/// `/v1/pm/customers` 维护的客户主数据上；保留本函数只为既有调用方与测试。
pub async fn bootstrap_project(
    pool: &PgPool,
    req: BootstrapRequest,
) -> Result<BootstrapOutcome, BootstrapError> {
    bootstrap_project_with_status(pool, req, None).await
}

/// 同 [`bootstrap_project`]，可指定初始项目状态（导入用 `initializing`）。
pub async fn bootstrap_project_with_status(
    pool: &PgPool,
    req: BootstrapRequest,
    initial_status: Option<&str>,
) -> Result<BootstrapOutcome, BootstrapError> {
    bootstrap_inner(
        pool,
        req,
        CustomerTarget::Implicit,
        initial_status,
        ProjectOwner::Synthetic,
    )
    .await
}

/// T-022：建项时把**真实调用者**写成 `project_member(role='owner')`，不再造
/// `owner-<code>` 合成 principal。可见性按 `project_member` 过滤后，创建者必须
/// 在成员表里，否则新项目对他自己不可见。
pub async fn bootstrap_project_owned(
    pool: &PgPool,
    req: BootstrapRequest,
    owner: PrincipalId,
) -> Result<BootstrapOutcome, BootstrapError> {
    bootstrap_inner(
        pool,
        req,
        CustomerTarget::Implicit,
        None,
        ProjectOwner::Principal(owner),
    )
    .await
}

/// 把项目挂到**已存在**的客户上（0017 客户主数据路径）。
///
/// 校验客户存在且未归档，不再隐式创建客户——一个客户对多个项目由调用方
/// 传入同一个 `customer_id` 即可。客户不可用时报
/// [`BootstrapError::CustomerNotFound`]，事务不留下任何行。
pub async fn bootstrap_project_for_customer(
    pool: &PgPool,
    req: BootstrapRequest,
    customer_id: Uuid,
    initial_status: Option<&str>,
) -> Result<BootstrapOutcome, BootstrapError> {
    bootstrap_inner(
        pool,
        req,
        CustomerTarget::Existing(customer_id),
        initial_status,
        ProjectOwner::Synthetic,
    )
    .await
}

/// [`bootstrap_project_for_customer`] 的归属版本：真实调用者写为 owner 成员（T-022）。
pub async fn bootstrap_project_for_customer_owned(
    pool: &PgPool,
    req: BootstrapRequest,
    customer_id: Uuid,
    owner: PrincipalId,
    initial_status: Option<&str>,
) -> Result<BootstrapOutcome, BootstrapError> {
    bootstrap_inner(
        pool,
        req,
        CustomerTarget::Existing(customer_id),
        initial_status,
        ProjectOwner::Principal(owner),
    )
    .await
}

/// Bootstraps one project: customer resolution, the five principals, the
/// project, its members, the AI configuration and the five skill bindings —
/// all inside one transaction. Fails with [`BootstrapError::Duplicate`] when
/// the project code is already taken.
async fn bootstrap_inner(
    pool: &PgPool,
    req: BootstrapRequest,
    customer: CustomerTarget,
    initial_status: Option<&str>,
    owner: ProjectOwner,
) -> Result<BootstrapOutcome, BootstrapError> {
    let mut tx = pool.begin().await.map_err(BootstrapError::Db)?;
    let tenant_id = req.tenant_id;

    // Friendly duplicate check before any inserts; the UNIQUE(tenant_id, code)
    // violation on the insert below is the race-safe backstop.
    let existing: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM core.project WHERE tenant_id = $1 AND code = $2")
            .bind(tenant_id)
            .bind(&req.project_code)
            .fetch_optional(&mut *tx)
            .await
            .map_err(BootstrapError::Db)?;
    if existing.is_some() {
        return Err(BootstrapError::Duplicate(req.project_code));
    }

    let customer_id: Uuid = match customer {
        CustomerTarget::Existing(id) => {
            // 租户谓词：跨租户的 customer_id 视为不存在，不泄露存在性。
            let found: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM core.customer \
                  WHERE id = $1 AND tenant_id = $2 AND status <> 'archived'",
            )
            .bind(id)
            .bind(tenant_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(BootstrapError::Db)?;
            found.ok_or(BootstrapError::CustomerNotFound(id))?
        }
        // 兼容路径：客户按 name（= project code）幂等，name 无唯一约束故先查后插。
        CustomerTarget::Implicit => {
            match sqlx::query_scalar(
                "SELECT id FROM core.customer WHERE tenant_id = $1 AND name = $2",
            )
            .bind(tenant_id)
            .bind(&req.project_code)
            .fetch_optional(&mut *tx)
            .await
            .map_err(BootstrapError::Db)?
            {
                Some(id) => id,
                None => sqlx::query_scalar(
                    "INSERT INTO core.customer (tenant_id, name, status) \
                     VALUES ($1, $2, 'placeholder') RETURNING id",
                )
                .bind(tenant_id)
                .bind(&req.project_code)
                .fetch_one(&mut *tx)
                .await
                .map_err(BootstrapError::Db)?,
            }
        }
    };

    // 真实调用者路径不造合成 human principal：调用者本身就是 owner。
    let human_id = match owner {
        ProjectOwner::Synthetic => {
            insert_principal(&mut tx, "human", &format!("owner-{}", req.project_code)).await?
        }
        ProjectOwner::Principal(principal_id) => principal_id,
    };
    let ai_principal_id =
        insert_principal(&mut tx, "ai_pm", &format!("ai-pm-{}", req.project_code)).await?;
    // `dev_agent` 是**租户级**实例（ADR-0001 D3）：同一租户的多个项目复用同一组
    // Agent，项目授权仍写 `project_member`，不另造 grant 表。
    let (architect_id, developer_id, tester_id) = ensure_tenant_agents(&mut tx, tenant_id).await?;

    let status = match initial_status {
        None | Some("planning") => "planning",
        Some("initializing") => "initializing",
        Some(other) => {
            return Err(BootstrapError::Db(sqlx::Error::Protocol(format!(
                "unsupported initial project status `{other}`"
            ))))
        }
    };
    let project_id: ProjectId = match sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(tenant_id)
    .bind(customer_id)
    .bind(&req.project_code)
    .bind(&req.project_name)
    .bind(status)
    .fetch_one(&mut *tx)
    .await
    {
        Ok(id) => id,
        Err(sqlx::Error::Database(db)) if db.is_unique_violation() => {
            return Err(BootstrapError::Duplicate(req.project_code));
        }
        Err(e) => return Err(BootstrapError::Db(e)),
    };

    insert_member(&mut tx, tenant_id, project_id, human_id, "owner").await?;
    insert_member(&mut tx, tenant_id, project_id, ai_principal_id, "ai_pm").await?;
    insert_member(&mut tx, tenant_id, project_id, architect_id, "architect").await?;
    insert_member(&mut tx, tenant_id, project_id, developer_id, "developer").await?;
    insert_member(&mut tx, tenant_id, project_id, tester_id, "tester").await?;

    sqlx::query(
        "INSERT INTO ai.project_ai_config \
         (project_id, tenant_id, ai_principal_id, model_provider, model_name, model_config, automation_mode, policy_version) \
         VALUES ($1, (SELECT tenant_id FROM core.project WHERE id = $1), $2, 'fake', 'fake', '{}'::jsonb, 'ai_lead', 1)",
    )
    .bind(project_id)
    .bind(ai_principal_id)
    .execute(&mut *tx)
    .await
    .map_err(BootstrapError::Db)?;

    for skill_id in SKILL_IDS {
        sqlx::query(
            "INSERT INTO ai.project_skill_binding (project_id, tenant_id, skill_id, skill_version) \
             VALUES ($1, (SELECT tenant_id FROM core.project WHERE id = $1), $2, 'v1')",
        )
        .bind(project_id)
        .bind(skill_id)
        .execute(&mut *tx)
        .await
        .map_err(BootstrapError::Db)?;
    }

    tx.commit().await.map_err(BootstrapError::Db)?;

    Ok(BootstrapOutcome {
        customer_id,
        project_id,
        ai_principal_id,
        dev_agent_id: developer_id,
        developer_id,
        architect_id,
        tester_id,
        human_id,
    })
}

/// 按 `core.project_member.role` 解析承接任务的 principal；未知角色返回 None。
pub async fn resolve_assignee(
    pool: &PgPool,
    project_id: ProjectId,
    role: &str,
) -> Result<Option<PrincipalId>, BootstrapError> {
    sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member \
         WHERE project_id = $1 AND role = $2 LIMIT 1",
    )
    .bind(project_id)
    .bind(role)
    .fetch_optional(pool)
    .await
    .map_err(BootstrapError::Db)
}

async fn insert_principal(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    kind: &str,
    display_name: &str,
) -> Result<PrincipalId, BootstrapError> {
    sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ($1, $2) RETURNING id",
    )
    .bind(kind)
    .bind(display_name)
    .fetch_one(&mut **tx)
    .await
    .map_err(BootstrapError::Db)
}

/// 取（或建）该租户的三个 `dev_agent` 实例，返回 (architect, developer, tester)。
///
/// 租户级而非项目级：第二次及以后建项目时直接复用，不重复造 principal——
/// 这正是 ADR-0001 D3 的三层结构（`agent_definition` 全局 → `tenant_agent` 租户
/// → `project_member` 项目授权）。
async fn ensure_tenant_agents(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
) -> Result<(PrincipalId, PrincipalId, PrincipalId), BootstrapError> {
    let found: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT d.code, ta.principal_id \
           FROM core.tenant_agent ta \
           JOIN ai.agent_definition d ON d.id = ta.agent_definition_id \
          WHERE ta.tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(BootstrapError::Db)?;

    let mut ids = std::collections::HashMap::new();
    for (code, principal_id) in found {
        ids.insert(code, principal_id);
    }

    for code in ["architect", "developer", "tester"] {
        if ids.contains_key(code) {
            continue;
        }
        let principal_id =
            insert_principal(tx, "dev_agent", &format!("{code}-{tenant_id}")).await?;
        sqlx::query(
            "INSERT INTO core.tenant_agent (tenant_id, agent_definition_id, principal_id) \
             SELECT $1, d.id, $2 FROM ai.agent_definition d WHERE d.code = $3",
        )
        .bind(tenant_id)
        .bind(principal_id)
        .bind(code)
        .execute(&mut **tx)
        .await
        .map_err(BootstrapError::Db)?;
        ids.insert(code.to_string(), principal_id);
    }

    Ok((ids["architect"], ids["developer"], ids["tester"]))
}

async fn insert_member(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    project_id: ProjectId,
    principal_id: PrincipalId,
    role: &str,
) -> Result<(), BootstrapError> {
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .bind(role)
    .execute(&mut **tx)
    .await
    .map_err(BootstrapError::Db)?;
    Ok(())
}
