//! 检索计划校验（WP-1 §五 契约二：Validator 是不可绕过的唯一执行入口）。
//!
//! 本模块只做 **IO 事实收集**——成员可见性、关系类型白名单、历史版本策略——然后
//! 交给契约层纯函数 [`kb_pc_contracts::retrieval::validate_plan`] 完成六项校验与
//! 夹紧。`ValidatedPlan` 的构造点在契约层（字段私有），本模块**无法**绕过。

use sqlx::PgPool;

use kb_pc_contracts::retrieval::{
    RetrievalCaller, RetrievalChannel, RetrievalMode, RetrievalPlan, ValidatedPlan,
    ValidationInputs, ValidationReport,
};

/// 收集校验所需的 IO 事实。
pub async fn collect_validation_inputs(
    pool: &PgPool,
    caller: &RetrievalCaller,
    plan: &RetrievalPlan,
) -> Result<ValidationInputs, sqlx::Error> {
    // ① 项目成员可见性（纵深防御：REST 层已做过一次，这里再做一次）。
    let project_visible: bool = sqlx::query_scalar(
        "SELECT EXISTS ( \
             SELECT 1 FROM core.project_member m \
             JOIN core.project p ON p.id = m.project_id AND p.tenant_id = m.tenant_id \
             WHERE m.project_id = $1 AND m.principal_id = $2 AND m.tenant_id = $3 \
         )",
    )
    .bind(caller.project_id)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_one(pool)
    .await?;

    // ② 跨域：V1 = **同项目内允许**（PC↔KB↔S02 关联检索）；跨项目永久禁止——
    //    跨项目由「项目上下文 = 调用方 project」这一根本边界保证（检索只查本项目
    //    数据，跨项目需要显式开关，本批不提供）。
    let cross_domain_allowed = true;

    // ③ 历史版本读策略：项目内成员可读**已发布**历史修订；draft 恒禁（通道级硬
    //    过滤 + 组装期权威终检双保险）。
    let allow_historical = project_visible;

    // ④ 关系类型白名单：`pc.relationship_type` 注册表的 active 集。
    let allowed_relationship_types: Vec<String> = if plan
        .retrieval_modes
        .contains(&RetrievalChannel::Relationship)
    {
        sqlx::query_scalar(
            "SELECT type_key FROM pc.relationship_type WHERE status = 'active' ORDER BY type_key",
        )
        .fetch_all(pool)
        .await?
    } else {
        Vec::new()
    };

    // ⑥ 敏感知识钩子：V1 无密级元数据 → 项目内可见（fail-closed 预留位）。
    let sensitive_blocked = false;

    Ok(ValidationInputs {
        project_visible,
        cross_domain_allowed,
        allow_historical,
        allowed_relationship_types,
        sensitive_blocked,
    })
}

/// 校验入口（收集 IO → 纯函数校验）。
pub async fn validate(
    pool: &PgPool,
    plan: RetrievalPlan,
    caller: &RetrievalCaller,
) -> Result<ValidatedPlan, ValidationReport> {
    let inputs = collect_validation_inputs(pool, caller, &plan)
        .await
        .map_err(|e| ValidationReport {
            checks: vec![kb_pc_contracts::retrieval::ValidationCheck {
                id: "io".into(),
                passed: false,
                reason: Some(format!("validation inputs unavailable: {e}")),
            }],
        })?;
    kb_pc_contracts::retrieval::validate_plan(plan, caller, &inputs)
}

/// 跨域模式判定（供编排层记录用；不代表放行——放行只由校验器裁决）。
pub fn wants_cross_domain(plan: &RetrievalPlan) -> bool {
    plan.mode == RetrievalMode::CrossDomain
        || plan
            .retrieval_modes
            .contains(&RetrievalChannel::Relationship)
}

/// 计划来源标签（wire 用）。
pub fn plan_source_label(source: kb_pc_contracts::retrieval::RetrievalPlanSource) -> String {
    match source {
        kb_pc_contracts::retrieval::RetrievalPlanSource::LlmPlanner => "llm_planner".to_string(),
        kb_pc_contracts::retrieval::RetrievalPlanSource::DeterministicFallback => {
            "deterministic_fallback".to_string()
        }
    }
}
