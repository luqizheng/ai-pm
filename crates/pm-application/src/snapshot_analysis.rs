//! 仓库快照分析 LLM 端口（P2-13 裁决 D-1=A：Cognition/KB 域引入 LLM 执行体，
//! 治理不变；2026-11-08 裁决包
//! `docs/project-ai-team-v2/ai-pm-p2-13-pc-i6-llm-analysis-adjudication-pack-v1.md`）。
//!
//! 分层纪律与 [`crate::repo_facts::RepoAnalysisModel`] 同款：实现放在 `pm-ai`
//! （路由 + OpenAI 兼容调用 + `ai.llm_call_record` 记账），本层只依赖 trait，
//! 不反向依赖 `pm-ai`。
//!
//! D-2=A 产物边界（起笔切片）：LLM 只做**受约束增强**——对象骨架（4 类
//! object_key/形态）仍由结构化派生给定（`plan_cognition_objects`），LLM 产出
//! 限定为骨架对象的 summary/content 与快照 digest 叙述摘要；三区分类
//! （事实/推断/不确定）与证据引用约束由实现侧强制。结论恒 PROPOSED，正式
//! 确认语义不在此端口（AC-015-02/AC-016-02）。
//!
//! D-3=A 降级语义：`Ok(None)` = 无可用 provider（上层走结构化派生，
//! analysis_mode='derived'）；`Err(_)` = 解析/调用失败（上层同样回退结构化
//! 派生，analysis_mode='derived_fallback'）。实现绝不返回 fixtures 假数据
//! ——假数据进治理面属污染（与主链 FakeModel 回退的关键差异）。

use crate::repo_facts::{pm_ai_port::ModelError, RepoFacts};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 交给 LLM 的对象骨架（结构化派生给定，LLM 不得增删 object_key）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotObjectSkeleton {
    pub object_key: String,
    pub object_type: String,
    pub name: String,
    pub summary: String,
    pub content: String,
}

/// KB 侧增强结果：快照 digest 的 LLM 叙述摘要（统计清单仍由系统结构化生成，
/// 上层负责拼装与 mode 尾注）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotDigestEnrichment {
    pub markdown: String,
    pub model_name: String,
}

/// PC 侧单对象增强（object_key 必须来自骨架；evidence_refs 已由实现侧对照
/// RepoFacts 白名单过滤，调用方可直接持久化）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotObjectEnrichment {
    pub object_key: String,
    pub summary: String,
    pub content: String,
    pub evidence_refs: Vec<String>,
}

/// PC 侧增强结果：骨架全集的增强内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotObjectsEnrichment {
    pub objects: Vec<SnapshotObjectEnrichment>,
    pub model_name: String,
}

/// 仓库快照分析 LLM 端口。实现为纯增强调用方：不触库表（记账除外，在实现
/// 内部 best-effort），失败语义见模块头注。
#[async_trait::async_trait]
pub trait SnapshotAnalysisModel: Send + Sync {
    /// KB 侧：仓库事实 → digest 叙述摘要。无 provider → `Ok(None)`。
    async fn enrich_digest(
        &self,
        project_id: Uuid,
        repo_name: &str,
        repo_url: &str,
        facts: &RepoFacts,
    ) -> Result<Option<SnapshotDigestEnrichment>, ModelError>;

    /// PC 侧：仓库事实 + 骨架 → 逐对象增强。骨架全覆盖是硬契约——任一
    /// object_key 缺失即 `Err(BadOutput)`（上层回退结构化派生）。
    async fn enrich_objects(
        &self,
        project_id: Uuid,
        facts: &RepoFacts,
        skeletons: &[SnapshotObjectSkeleton],
    ) -> Result<Option<SnapshotObjectsEnrichment>, ModelError>;
}
