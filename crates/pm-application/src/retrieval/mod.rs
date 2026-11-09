//! PC / KB 统一检索面（检索架构 V1.0 任务书 WP-1）。
//!
//! **架构纪律**：LLM 负责提出检索计划，确定性代码负责授权、校验和执行——LLM 不拥有
//! 检索权限，也不直接控制数据访问。落地方式：
//!
//! - [`validator`]：收集 IO 事实（成员可见性、关系白名单、历史策略）→ 调契约层
//!   **纯函数** [`kb_pc_contracts::retrieval::validate_plan`] 产出
//!   `ValidatedPlan`（字段私有，唯一构造点在契约层）——**未经校验的计划在类型上
//!   无法进入执行层**；
//! - [`channels`]：多通道召回（V1：`pc_entity` 真实现；`pc_fts` / `kb_section` 待
//!   WP-3 索引设施落地）；
//! - [`service`]：编排（plan → validate → execute → evidence）+ 降级链。
//!
//! **降级链**（裁决 §5.3）：Planner 关闭 / 超时 / 坏 JSON / Provider 不可用 /
//! 项目无模型策略 → 统一走**确定性降级计划**（PC+KB 双域直查），但各自带
//! `degradation_reason`；降级**不得**让 Retrieval API 不可用。
//!
//! **Planner OFF ≠ Cross-domain OFF**（裁决 Q-B 补充）：D-7 是数据访问边界，
//! Planner 是计划生成能力——两者不绑定。Planner 关闭时执行固定双域策略。

pub mod channels;
pub mod service;
pub mod traversal;
pub mod validator;

use serde::Serialize;

/// 检索响应（唯一 REST 端点的返回体）。
#[derive(Debug, Clone, Serialize)]
pub struct RetrievalResponse {
    /// 计划来源（`llm_planner` / `deterministic_fallback`）。
    pub plan_source: String,
    /// 是否处于降级状态。
    pub degraded: bool,
    /// 降级原因码（机器可读；无降级时为空数组）。
    pub degradation_reason: Option<String>,
    /// 校验报告（逐项 pass/deny）。
    pub validation: kb_pc_contracts::retrieval::ValidationReport,
    /// 证据包。
    pub evidence: kb_pc_contracts::retrieval::EvidenceBundle,
}
