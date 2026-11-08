//! Execution Policies 读写面（UI-3，U-2=A 五子页最后一块；零迁移——两表
//! `delivery.policy_scope`（0013 建表；0016 增能力列 auto_accept/auto_merge/
//! auto_publish_knowledge；0019 增 `tenant_id NOT NULL` 且主键改为
//! `(tenant_id, scope, scope_key)`）与 `delivery.task_policy_override`
//! （0010 建表；0019 增 tenant_id）均已存在，此前只有调度合成在读，本模块
//! 补齐 REST 读写的数据面。
//!
//! 0016 的 scope 行能力列本端点**不读不写**（V1 写面只管 require_human_
//! approval；能力列仅调度合成 `load_scope_layer` 消费），如实留给后续裁决。
//!
//! 语义（照 `run_dispatch::policy` / `policy_composition` 代码事实）：
//! - 读取 = 四层现状 + 合成规则说明。organization（scope_key='*'）与
//!   project（scope_key=project_id）行可缺省（None = 该层不表态/继承上层）；
//!   task_type 行按租户全域（scope_key = 任务类型文本，跨项目口径）；任务
//!   覆盖按项目列出。
//! - 合成规则（[`policy_composition::EffectivePolicy::compose`]）：
//!   * 能力类（auto_accept/auto_merge/auto_publish_knowledge）各表态层取交集，
//!     缺层/NULL = 不表态（放行），全空表 = 默认允许；
//!   * 约束类 require_human_approval 任一层 true 即 true（高层强制审批、
//!     低层不得放宽），默认 false；
//!   * require_human_approval=true 时否决 auto_accept（auto_merge 与
//!     auto_publish_knowledge 独立判定，不受牵连）；
//!   * additional_checks 各层并集去重（规范化 JSON 排序）。
//! - **V1 写面限定**：只有 project 层与 task 覆盖层可写；organization 影响
//!   租户全局 '*'、task_type 跨项目，写入口留待显式裁决（REST 层不挂写路由）。
//! - 写路径同事务落 `governance.audit_log`（action 见各函数文档；低频管理
//!   动作，不进 outbox——仓库惯例，同 `ai_roles`）。

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::append_audit_with_after;
use crate::policy_composition::{EffectivePolicy, PolicyLayers};

#[derive(Debug, thiserror::Error)]
pub enum AiTeamPolicyError {
    /// 项目不存在或跨租户（上层 404，不泄露存在性）。
    #[error("project not found")]
    ProjectNotFound,
    /// 任务不存在或跨租户（上层 404）。
    #[error("task not found")]
    TaskNotFound,
    /// 请求体不满足写前置条件（上层 400）。
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

// ── 读取 ──────────────────────────────────────────────────────────────

/// scope 行视图（organization / project 两层共用；`require_human_approval`
/// 在 policy_scope 里 NOT NULL，无「部分表态」形态）。
#[derive(Debug, Clone, Serialize)]
pub struct ScopePolicyView {
    pub require_human_approval: bool,
    pub updated_by: Uuid,
    pub updated_at: DateTime<Utc>,
}

/// task_type 层行（跨项目全局口径）。
#[derive(Debug, Clone, Serialize)]
pub struct TaskTypePolicyView {
    /// `delivery.policy_scope.scope_key` = 任务类型文本。
    pub task_type: String,
    pub require_human_approval: bool,
    pub updated_by: Uuid,
    pub updated_at: DateTime<Utc>,
}

/// 任务级覆盖行（`delivery.task_policy_override`；NULL 字段 = 继承上层）。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TaskPolicyOverrideView {
    pub task_id: Uuid,
    pub task_title: String,
    pub auto_accept: Option<bool>,
    pub auto_merge: Option<bool>,
    pub auto_publish_knowledge: Option<bool>,
    pub require_human_approval: Option<bool>,
    pub updated_at: DateTime<Utc>,
}

/// 合成规则说明（文档字段：默认值由 [`EffectivePolicy::compose`] 对空层实时
/// 计算得出，不手抄常量，保证与调度语义永远一致）。
#[derive(Debug, Clone, Serialize)]
pub struct PolicyCompositionDoc {
    /// 叠加顺序：organization → project → task_type → task。
    pub layer_order: Vec<&'static str>,
    /// 无任何层表态时的有效策略（代码事实：能力类默认允许、审批默认不要求）。
    pub defaults: EffectivePolicyDefaultsDoc,
    /// 逐条规则文本（key 稳定、text 人类可读；UI 原样渲染）。
    pub rules: Vec<PolicyRuleDoc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EffectivePolicyDefaultsDoc {
    pub auto_accept: bool,
    pub auto_merge: bool,
    pub auto_publish_knowledge: bool,
    pub require_human_approval: bool,
    pub additional_checks: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicyRuleDoc {
    pub key: &'static str,
    pub text: &'static str,
}

/// GET 响应：四层现状 + 合成规则说明。容器键 `taskTypes` / `taskOverrides`
/// 按任务说明用驼峰；行内字段沿用仓库 snake_case 惯例（同 monitor 契约）。
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionPoliciesView {
    /// scope='organization', scope_key='*'；行缺失 = None（未设置，走默认）。
    pub organization: Option<ScopePolicyView>,
    /// scope='project', scope_key=project_id；行缺失 = None（继承组织）。
    pub project: Option<ScopePolicyView>,
    /// scope='task_type' 全部行（**租户内跨项目全局**，非本项目专属）。
    #[serde(rename = "taskTypes")]
    pub task_types: Vec<TaskTypePolicyView>,
    /// 本项目任务覆盖（`delivery.task_policy_override` JOIN task 取 title，
    /// updated_at DESC 取前 100——该表无 created_at，updated_at 是唯一时间锚）。
    #[serde(rename = "taskOverrides")]
    pub task_overrides: Vec<TaskPolicyOverrideView>,
    pub composition: PolicyCompositionDoc,
}

/// 合成规则文档（每次读取实时构建；默认值照 compose 代码计算）。
pub fn composition_doc() -> PolicyCompositionDoc {
    let defaults = EffectivePolicy::compose(&PolicyLayers::default());
    PolicyCompositionDoc {
        layer_order: vec!["organization", "project", "task_type", "task"],
        defaults: EffectivePolicyDefaultsDoc {
            auto_accept: defaults.auto_accept,
            auto_merge: defaults.auto_merge,
            auto_publish_knowledge: defaults.auto_publish_knowledge,
            require_human_approval: defaults.require_human_approval,
            additional_checks: defaults.additional_checks,
        },
        rules: vec![
            PolicyRuleDoc {
                key: "layer_order",
                text: "四层依次叠加：organization → project → task_type → task；行缺失或字段为 NULL 表示该层不表态（继承上层）。",
            },
            PolicyRuleDoc {
                key: "capabilities",
                text: "能力类字段（auto_accept / auto_merge / auto_publish_knowledge）取所有表态层的交集：全部允许才允许；无任何层表态时默认允许。",
            },
            PolicyRuleDoc {
                key: "constraints",
                text: "约束类字段 require_human_approval 只能收紧：任一层为 true 即要求人工审批；低层不得放宽高层（任务级 false 无法覆盖组织/项目层的 true）；无表态时默认不要求。",
            },
            PolicyRuleDoc {
                key: "auto_accept_veto",
                text: "人工审批否决自动验收：require_human_approval = true 时 auto_accept 强制为 false；auto_merge 与 auto_publish_knowledge 独立判定，不受审批要求牵连。",
            },
            PolicyRuleDoc {
                key: "additional_checks",
                text: "additional_checks 取各层并集并去重（按规范化 JSON 排序，顺序稳定）。",
            },
        ],
    }
}

async fn fetch_scope_row(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: &str,
    scope_key: &str,
) -> Result<Option<ScopePolicyView>, AiTeamPolicyError> {
    let row = sqlx::query_as::<_, (bool, Uuid, DateTime<Utc>)>(
        "SELECT require_human_approval, updated_by, updated_at \
           FROM delivery.policy_scope \
          WHERE tenant_id = $1 AND scope = $2 AND scope_key = $3",
    )
    .bind(tenant_id)
    .bind(scope)
    .bind(scope_key)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(require_human_approval, updated_by, updated_at)| ScopePolicyView {
        require_human_approval,
        updated_by,
        updated_at,
    }))
}

/// 项目 Execution Policies 聚合读取（可见性校验在 REST 层；这里保证归属）。
pub async fn get_project_execution_policies(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<ExecutionPoliciesView, AiTeamPolicyError> {
    let project_tenant: Option<Uuid> =
        sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    if project_tenant.as_ref() != Some(&tenant_id) {
        return Err(AiTeamPolicyError::ProjectNotFound);
    }

    let organization = fetch_scope_row(pool, tenant_id, "organization", "*").await?;
    let project = fetch_scope_row(pool, tenant_id, "project", &project_id.to_string()).await?;

    let task_type_rows = sqlx::query_as::<_, (String, bool, Uuid, DateTime<Utc>)>(
        "SELECT scope_key, require_human_approval, updated_by, updated_at \
           FROM delivery.policy_scope \
          WHERE tenant_id = $1 AND scope = 'task_type' \
          ORDER BY scope_key",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let task_types = task_type_rows
        .into_iter()
        .map(|(task_type, require_human_approval, updated_by, updated_at)| TaskTypePolicyView {
            task_type,
            require_human_approval,
            updated_by,
            updated_at,
        })
        .collect();

    // 覆盖行上限 100（V1 不分页）；排序锚 updated_at DESC（表无 created_at）。
    let task_overrides = sqlx::query_as::<_, TaskPolicyOverrideView>(
        "SELECT o.task_id, t.title AS task_title, \
                o.auto_accept, o.auto_merge, o.auto_publish_knowledge, \
                o.require_human_approval, o.updated_at \
           FROM delivery.task_policy_override o \
           JOIN delivery.task t ON t.id = o.task_id \
          WHERE o.project_id = $1 \
          ORDER BY o.updated_at DESC, o.task_id DESC \
          LIMIT 100",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    Ok(ExecutionPoliciesView {
        organization,
        project,
        task_types,
        task_overrides,
        composition: composition_doc(),
    })
}

// ── 写入（V1 限定：project 层 + task 覆盖层）──────────────────────────

/// 设置项目层 require_human_approval；`None` = 删除项目行（恢复继承组织）。
///
/// 同事务审计：action=`execution_policy.project_updated`，`after_data` 为
/// 写后快照（`require_human_approval: null` 即清除）。
pub async fn set_project_execution_policy(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    project_id: Uuid,
    require_human_approval: Option<bool>,
) -> Result<Option<ScopePolicyView>, AiTeamPolicyError> {
    let project_tenant: Option<Uuid> =
        sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    if project_tenant.as_ref() != Some(&tenant_id) {
        return Err(AiTeamPolicyError::ProjectNotFound);
    }

    let mut tx = pool.begin().await?;
    match require_human_approval {
        Some(value) => {
            // 主键 = (tenant_id, scope, scope_key)（0019）。
            sqlx::query(
                "INSERT INTO delivery.policy_scope \
                     (tenant_id, scope, scope_key, require_human_approval, updated_by) \
                 VALUES ($1, 'project', $2, $3, $4) \
                 ON CONFLICT (tenant_id, scope, scope_key) DO UPDATE SET \
                   require_human_approval = EXCLUDED.require_human_approval, \
                   updated_by = EXCLUDED.updated_by, \
                   updated_at = now()",
            )
            .bind(tenant_id)
            .bind(project_id.to_string())
            .bind(value)
            .bind(actor.principal_id)
            .execute(&mut *tx)
            .await?;
        }
        None => {
            sqlx::query(
                "DELETE FROM delivery.policy_scope \
                  WHERE tenant_id = $1 AND scope = 'project' AND scope_key = $2",
            )
            .bind(tenant_id)
            .bind(project_id.to_string())
            .execute(&mut *tx)
            .await?;
        }
    }

    let after = serde_json::json!({
        "scope": "project",
        "scope_key": project_id.to_string(),
        "require_human_approval": require_human_approval,
    });
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "execution_policy.project_updated",
        "policy_scope",
        None,
        after,
    )
    .await?;
    tx.commit().await?;

    fetch_scope_row(pool, tenant_id, "project", &project_id.to_string()).await
}

/// 任务覆盖写输入；`None` 字段 = 继承（不表态）。
#[derive(Debug, Clone, Default)]
pub struct TaskPolicyOverrideInput {
    pub auto_accept: Option<bool>,
    pub auto_merge: Option<bool>,
    pub auto_publish_knowledge: Option<bool>,
    pub require_human_approval: Option<bool>,
}

impl TaskPolicyOverrideInput {
    /// 全部字段为 None = 无差异可写（清除走 [`clear_task_policy_override`]，
    /// 本端点只做 upsert）。
    fn is_empty(&self) -> bool {
        self.auto_accept.is_none()
            && self.auto_merge.is_none()
            && self.auto_publish_knowledge.is_none()
            && self.require_human_approval.is_none()
    }

    fn after_json(&self, task_id: Uuid) -> serde_json::Value {
        serde_json::json!({
            "task_id": task_id,
            "auto_accept": self.auto_accept,
            "auto_merge": self.auto_merge,
            "auto_publish_knowledge": self.auto_publish_knowledge,
            "require_human_approval": self.require_human_approval,
        })
    }
}

/// UPSERT 任务覆盖（至少一个字段非 None，否则 [`AiTeamPolicyError::Validation`]）。
///
/// 行内未出现的字段一律写 NULL（= 继承）；`additional_checks` V1 写面不碰
/// （插入走表默认 `'[]'`，更新保持原值）。任务归属校验：任务必须存在于调用
/// 者租户（`delivery.task.tenant_id`，0019）；项目 id 从任务行派生（组合 FK 兜底）。
/// 审计 action=`execution_policy.task_override_updated`。
pub async fn set_task_policy_override(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    task_id: Uuid,
    input: TaskPolicyOverrideInput,
) -> Result<(), AiTeamPolicyError> {
    if input.is_empty() {
        return Err(AiTeamPolicyError::Validation(
            "at least one field must be non-null; use DELETE to restore full inheritance".into(),
        ));
    }
    let project_id = task_project_id(pool, tenant_id, task_id).await?;

    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO delivery.task_policy_override \
             (task_id, project_id, tenant_id, auto_accept, auto_merge, \
              auto_publish_knowledge, require_human_approval, updated_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (task_id) DO UPDATE SET \
           auto_accept = EXCLUDED.auto_accept, \
           auto_merge = EXCLUDED.auto_merge, \
           auto_publish_knowledge = EXCLUDED.auto_publish_knowledge, \
           require_human_approval = EXCLUDED.require_human_approval, \
           updated_by = EXCLUDED.updated_by, \
           updated_at = now()",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(tenant_id)
    .bind(input.auto_accept)
    .bind(input.auto_merge)
    .bind(input.auto_publish_knowledge)
    .bind(input.require_human_approval)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "execution_policy.task_override_updated",
        "task_policy_override",
        Some(task_id),
        input.after_json(task_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 删除任务覆盖（恢复全继承）。行不存在 = 已是全继承态，幂等 no-op
/// （不落审计，同 `ai_roles` 关闭未启用 Role 的惯例）。
/// 审计 action=`execution_policy.task_override_cleared`。
pub async fn clear_task_policy_override(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    task_id: Uuid,
) -> Result<(), AiTeamPolicyError> {
    let project_id = task_project_id(pool, tenant_id, task_id).await?;

    let mut tx = pool.begin().await?;
    let deleted: Option<Uuid> = sqlx::query_scalar(
        "DELETE FROM delivery.task_policy_override \
          WHERE task_id = $1 RETURNING task_id",
    )
    .bind(task_id)
    .fetch_optional(&mut *tx)
    .await?;
    if deleted.is_some() {
        let after = TaskPolicyOverrideInput::default().after_json(task_id);
        append_audit_with_after(
            &mut tx,
            project_id,
            actor,
            "execution_policy.task_override_cleared",
            "task_policy_override",
            Some(task_id),
            after,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// 任务在本租户内的归属项目（不存在/跨租户一律 TaskNotFound，不泄露存在性）。
async fn task_project_id(
    pool: &PgPool,
    tenant_id: Uuid,
    task_id: Uuid,
) -> Result<Uuid, AiTeamPolicyError> {
    let project_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT project_id FROM delivery.task WHERE id = $1 AND tenant_id = $2",
    )
    .bind(task_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?;
    project_id.ok_or(AiTeamPolicyError::TaskNotFound)
}
