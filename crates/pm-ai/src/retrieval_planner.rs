//! 检索规划模型（WP-1 Q-A 供给线）。
//!
//! **供给线纪律**（WP-1 裁决 Q-A）：使用项目级 `ai.project_ai_model_policy` +
//! Team LLM Provider，与 PC-I6 分析模型**同一套供给与路由机制**（复用
//! `select_analysis_model_config`）——**不新增独立 Provider 管理体系**，也不允许
//! 通过环境变量绕过 Team/Project 模型授权。
//!
//! **降级映射**：无模型策略 → `project_model_policy_missing`；Provider 不可用 →
//! `provider_unavailable`；超时 → `planner_timeout`；输出不合规 →
//! `invalid_planner_output`；其他 → `planner_execution_error`。四种情形统一落到
//! 确定性降级计划（应用层），**端点不得不可用**。
//!
//! **成本归因**：每次调用落 `ai.llm_call_record`，`purpose='retrieval_planner'`
//! （WP-1 验收 RT-025：模型/项目/结果/耗时/成本可归因）。
//!
//! **纪律**：模型只产出**计划**，不产出数据、不决定访问权限——权限由契约层
//! `validate_plan` 裁决。

use kb_pc_contracts::retrieval::{PlannerModel, PlannerModelError};
use sqlx::PgPool;
use uuid::Uuid;

use crate::analysis_model::{select_model_for_role, ModelSelection};
use crate::call_record::{record_call, PURPOSE_RETRIEVAL_PLANNER};
use crate::conversation_model::{self, ConversationTurn};
use crate::model::ModelError;

/// 规划器输入长度上限（字符；对话分类器同款纪律）。
pub const MAX_PLANNER_INPUT_CHARS: usize = 2_000;
/// 规划器超时上限（秒）。
pub const MAX_PLANNER_TIMEOUT_SECS: i64 = 8;

/// 规划系统提示（只要求 JSON 计划；不含任何权限语义）。
pub const PLANNER_SYSTEM_PROMPT: &str = r#"你是检索规划器。只输出一个 JSON 对象，不要解释、不要 markdown 代码块。

字段：
- intent: "factual_lookup" | "impact_analysis" | "completeness_check" | "summary" | "comparison"
- mode: "pc_only" | "kb_only" | "pc_kb" | "cross_domain"
- domains: 字符串数组，取值 "PC" / "KB"
- retrieval_modes: 字符串数组，取值 "pc_entity" | "pc_fts" | "kb_section" | "relationship"
- entities: 对象数组，每项 { "kind": "pc_object"|"kb_document", "type_hint": 可选字符串, "key_or_name": 字符串 }
- rewritten_queries: 字符串数组（1~3 条检索改写）
- traversal: 可选 { "enabled": 布尔, "max_hops": 整数(1~5) }
- analysis: 可选 { "kind": "coverage_gap"|"impact"|"none", "require_evidence": 布尔 }
- include_history: 布尔（是否需要已发布的历史版本）
- budget: { "max_candidates": 整数(1~60), "max_hops": 整数(0~5), "timeout_ms": 整数(1000~8000) }

规则：只输出 JSON；不得输出 SQL、不得引用内部表名；跨域关联分析用 cross_domain；
其余情况用 pc_only / kb_only / pc_kb。"#;

/// LLM 规划器（复用既有模型供给线）。
pub struct LlmRetrievalPlanner {
    pool: PgPool,
}

impl LlmRetrievalPlanner {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl PlannerModel for LlmRetrievalPlanner {
    async fn plan(&self, project_id: Uuid, query: &str) -> Result<String, PlannerModelError> {
        // Phase 3B role chain（任务书 §6.3）：检索规划 = general 用途。
        let cfg = match select_model_for_role(&self.pool, project_id, "general").await {
            ModelSelection::Provider(cfg) => cfg,
            ModelSelection::ConfigurationFailed { .. } => {
                return Err(PlannerModelError::NoProvider);
            }
            ModelSelection::ResolutionFailed(reason) => {
                return Err(PlannerModelError::ProviderUnavailable(reason));
            }
        };

        let input: String = query.chars().take(MAX_PLANNER_INPUT_CHARS).collect();
        let messages = [
            ConversationTurn::new("system", PLANNER_SYSTEM_PROMPT),
            ConversationTurn::new("user", format!("<query>\n{input}\n</query>")),
        ];
        let timeout = chrono::Duration::seconds(
            i64::from(cfg.timeout_secs).clamp(1, MAX_PLANNER_TIMEOUT_SECS),
        );
        let started = std::time::Instant::now();
        let reply = conversation_model::complete(
            &cfg.endpoint,
            &cfg.api_key,
            &cfg.model,
            timeout,
            &messages,
        )
        .await;
        let latency = started.elapsed().as_millis() as i64;

        match reply {
            Ok(completion) => {
                record_call(
                    &self.pool,
                    project_id,
                    cfg.provider_id,
                    &cfg.model,
                    PURPOSE_RETRIEVAL_PLANNER,
                    "ok",
                    None,
                    None,
                    latency,
                    None,
                    completion.usage,
                )
                .await;
                let raw = completion.content.trim().to_string();
                if raw.is_empty() {
                    return Err(PlannerModelError::InvalidOutput(
                        "planner returned empty content".into(),
                    ));
                }
                Ok(raw)
            }
            Err(err) => {
                let (kind, message) = match &err {
                    ModelError::Timeout => ("timeout", Some(err.to_string())),
                    ModelError::Unavailable(_) => ("provider_unavailable", Some(err.to_string())),
                    ModelError::BadOutput(_) => ("bad_output", Some(err.to_string())),
                    ModelError::Configuration(_) => ("configuration", Some(err.to_string())),
                };
                record_call(
                    &self.pool,
                    project_id,
                    cfg.provider_id,
                    &cfg.model,
                    PURPOSE_RETRIEVAL_PLANNER,
                    "failed",
                    Some(kind),
                    message.as_deref(),
                    latency,
                    None,
                    None,
                )
                .await;
                Err(match err {
                    ModelError::Timeout => PlannerModelError::Timeout,
                    ModelError::Unavailable(reason) => {
                        PlannerModelError::ProviderUnavailable(reason)
                    }
                    ModelError::BadOutput(reason) => PlannerModelError::InvalidOutput(reason),
                    ModelError::Configuration(_) => PlannerModelError::NoProvider,
                })
            }
        }
    }
}
