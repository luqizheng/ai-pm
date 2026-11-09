//! 检索编排（plan → validate → execute → evidence）+ 降级链。
//!
//! **降级链**（裁决 §5.3）：Planner 关闭 / 无模型策略 / Provider 不可用 / 超时 /
//! 坏 JSON / 执行错误 → 统一走**确定性降级计划**（PC+KB 双域直查），但各自带
//! `degradation_reason`；降级**不得**让 Retrieval API 不可用。
//!
//! **单一执行路径**：降级产出的仍是同一个 `RetrievalPlan` 形态，经**同一个校验器**
//! 校验后进**同一个执行器**——不因 Planner 失败出现两套执行分支。

use std::sync::Arc;

use kb_pc_contracts::retrieval::{
    BudgetReport, ChannelHits, EvidenceBundle, PlannerModel, PlannerModelError, RetrievalCaller,
    RetrievalChannel, RetrievalDegradationReason, RetrievalEvidence, RetrievalPlan,
    RetrievalPlanSource, RetrievalPort, TraversalResult, ValidatedPlan, ValidationReport,
};
use sqlx::PgPool;

use super::channels;
use super::validator;
use super::RetrievalResponse;

/// 检索编排错误。
#[derive(Debug, thiserror::Error)]
pub enum RetrievalError {
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("invalid request: {0}")]
    Invalid(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

/// 检索服务（进程内实现；Phase 7 换 HTTP 时端口形状不变）。
pub struct RetrievalServiceLive {
    pool: PgPool,
    /// 规划模型（None = 未注入 → Planner 关闭，走确定性降级）。
    planner: Option<Arc<dyn PlannerModel>>,
}

impl RetrievalServiceLive {
    pub fn new(pool: PgPool, planner: Option<Arc<dyn PlannerModel>>) -> Self {
        Self { pool, planner }
    }

    /// 统一检索入口（唯一 REST 端点的实现体）。
    pub async fn query(
        &self,
        query: &str,
        caller: &RetrievalCaller,
    ) -> Result<RetrievalResponse, RetrievalError> {
        // ① 计划（LLM 或确定性降级）。
        let (plan, source, degradation) = self.build_plan(caller.project_id, query).await;

        // ② 校验（不可绕过的唯一执行入口）。
        let validated = validator::validate(&self.pool, plan, caller)
            .await
            .map_err(|report| {
                RetrievalError::Forbidden(format!(
                    "retrieval plan rejected: {}",
                    render_report(&report)
                ))
            })?;

        // ③ 执行（通道级硬过滤已在此完成）。
        let hits = self.execute_channels(&validated).await?;

        // ④ 证据组装（WP-4 完整形态；本批为骨架：条数/字符预算在 WP-4 落地）。
        let mut degradations: Vec<String> = degradation
            .map(|r| r.as_str().to_string())
            .into_iter()
            .collect();
        degradations.extend(hits.unavailable_codes.iter().cloned());

        let evidence = EvidenceBundle {
            evidence: hits.hits,
            traversal: TraversalResult::default(),
            gaps: Vec::new(),
            degradations,
            budget_report: BudgetReport::default(),
        };

        Ok(RetrievalResponse {
            plan_source: validator::plan_source_label(source),
            degraded: degradation.is_some(),
            degradation_reason: degradation.map(|r| r.as_str().to_string()),
            validation: validated.report().clone(),
            evidence,
        })
    }

    /// 计划构建（LLM 或降级）。
    async fn build_plan(
        &self,
        project_id: uuid::Uuid,
        query: &str,
    ) -> (
        RetrievalPlan,
        RetrievalPlanSource,
        Option<RetrievalDegradationReason>,
    ) {
        let Some(planner) = self.planner.as_ref() else {
            // Planner 关闭（未注入 / 开关 off）——**不调用 LLM**，端点仍可用。
            return (
                kb_pc_contracts::retrieval::deterministic_fallback_plan(query),
                RetrievalPlanSource::DeterministicFallback,
                Some(RetrievalDegradationReason::PlannerDisabled),
            );
        };

        match planner.plan(project_id, query).await {
            Ok(raw) => match serde_json::from_str::<RetrievalPlan>(&raw) {
                Ok(plan) => (plan, RetrievalPlanSource::LlmPlanner, None),
                Err(_) => (
                    kb_pc_contracts::retrieval::deterministic_fallback_plan(query),
                    RetrievalPlanSource::DeterministicFallback,
                    Some(RetrievalDegradationReason::InvalidPlannerOutput),
                ),
            },
            Err(err) => {
                let reason = match err {
                    PlannerModelError::NoProvider => {
                        RetrievalDegradationReason::ProjectModelPolicyMissing
                    }
                    PlannerModelError::ProviderUnavailable(_) => {
                        RetrievalDegradationReason::ProviderUnavailable
                    }
                    PlannerModelError::Timeout => RetrievalDegradationReason::PlannerTimeout,
                    PlannerModelError::InvalidOutput(_) => {
                        RetrievalDegradationReason::InvalidPlannerOutput
                    }
                    PlannerModelError::Execution(_) => {
                        RetrievalDegradationReason::PlannerExecutionError
                    }
                };
                (
                    kb_pc_contracts::retrieval::deterministic_fallback_plan(query),
                    RetrievalPlanSource::DeterministicFallback,
                    Some(reason),
                )
            }
        }
    }

    /// 通道执行（已过硬过滤的候选才进入这里）。
    async fn execute_channels(
        &self,
        validated: &ValidatedPlan,
    ) -> Result<ChannelHits, RetrievalError> {
        let plan = validated.plan();
        let queries = &plan.rewritten_queries;
        let limit = plan
            .budget
            .map(|b| b.max_candidates)
            .unwrap_or(kb_pc_contracts::retrieval::MAX_CANDIDATES_LIMIT);

        let mut hits: Vec<RetrievalEvidence> = Vec::new();
        let mut unavailable_codes: Vec<String> = Vec::new();

        // PC 实体通道（真实现）。
        if plan.retrieval_modes.contains(&RetrievalChannel::PcEntity)
            && channels::mode_includes_pc(plan.mode)
        {
            let result = channels::pc_entity_channel(
                &self.pool,
                validated.tenant_id(),
                validated.project_id(),
                queries,
                limit,
            )
            .await?;
            hits.extend(result.hits);
            if let Some(code) = result.unavailable {
                unavailable_codes.push(code);
            }
        }

        // 其余通道（V1 占位，WP-3 接真索引后替换）。
        if plan.retrieval_modes.contains(&RetrievalChannel::PcFts) {
            let result = channels::pc_fts_channel().await;
            hits.extend(result.hits);
            if let Some(code) = result.unavailable {
                unavailable_codes.push(code);
            }
        }
        if plan.retrieval_modes.contains(&RetrievalChannel::KbSection)
            && channels::mode_includes_kb(plan.mode)
        {
            let result = channels::kb_section_channel().await;
            hits.extend(result.hits);
            if let Some(code) = result.unavailable {
                unavailable_codes.push(code);
            }
        }

        Ok(ChannelHits {
            hits,
            unavailable_codes,
        })
    }
}

/// 计划报告渲染（错误消息用；非机器消费）。
fn render_report(report: &ValidationReport) -> String {
    report
        .checks
        .iter()
        .filter(|c| !c.passed)
        .map(|c| format!("{}: {}", c.id, c.reason.as_deref().unwrap_or("denied")))
        .collect::<Vec<_>>()
        .join("; ")
}

#[async_trait::async_trait]
impl RetrievalPort for RetrievalServiceLive {
    async fn plan(
        &self,
        query: &str,
        caller: &RetrievalCaller,
    ) -> Result<RetrievalPlan, kb_pc_contracts::PortError> {
        let (plan, _, _) = self.build_plan(caller.project_id, query).await;
        Ok(plan)
    }

    async fn validate(
        &self,
        plan: RetrievalPlan,
        caller: RetrievalCaller,
    ) -> Result<ValidatedPlan, ValidationReport> {
        validator::validate(&self.pool, plan, &caller).await
    }

    async fn execute(
        &self,
        plan: ValidatedPlan,
    ) -> Result<ChannelHits, kb_pc_contracts::PortError> {
        self.execute_channels(&plan)
            .await
            .map_err(|e| kb_pc_contracts::PortError::Internal(e.to_string()))
    }

    async fn expand(
        &self,
        plan: &ValidatedPlan,
    ) -> Result<TraversalResult, kb_pc_contracts::PortError> {
        // WP-2 接真遍历引擎：种子解析 → 受控遍历（双端隔离 + 环检测 + 预算）。
        let p = plan.plan();
        let budget = p
            .budget
            .unwrap_or(kb_pc_contracts::retrieval::RetrievalBudget {
                max_candidates: kb_pc_contracts::retrieval::MAX_CANDIDATES_LIMIT,
                max_hops: 3,
                timeout_ms: kb_pc_contracts::retrieval::MAX_TIMEOUT_MS,
            });
        let seed_nodes = super::traversal::resolve_seed_nodes(
            &self.pool,
            plan.tenant_id(),
            plan.project_id(),
            &p.entities,
            budget.max_candidates,
        )
        .await;
        let request = super::traversal::TraversalRequest {
            seed_nodes,
            max_hops: budget.max_hops,
            max_candidates: budget.max_candidates,
            timeout_ms: budget.timeout_ms,
        };
        Ok(
            super::traversal::traverse(&self.pool, plan.tenant_id(), plan.project_id(), &request)
                .await,
        )
    }

    async fn evidence(
        &self,
        hits: &ChannelHits,
        traversal: &TraversalResult,
    ) -> Result<EvidenceBundle, kb_pc_contracts::PortError> {
        Ok(EvidenceBundle {
            evidence: hits.hits.clone(),
            traversal: traversal.clone(),
            gaps: Vec::new(),
            degradations: hits.unavailable_codes.clone(),
            budget_report: BudgetReport::default(),
        })
    }
}
