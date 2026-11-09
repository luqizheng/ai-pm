//! PC / KB 统一检索契约（检索架构 V1.0 任务书 WP-1 §五）。
//!
//! **架构纪律（本 crate 的存在理由）**：LLM 负责提出检索计划，确定性代码负责授权、
//! 校验和执行——**LLM 不拥有检索权限，也不直接控制数据访问**。
//!
//! 为让「未经校验的计划在类型和模块边界上无法进入执行层」，本模块把**校验做成
//! 纯函数** [`validate_plan`]：
//!
//! - 应用层（`pm-application::retrieval`）负责 IO——取项目成员可见性、关系类型
//!   注册表、历史版本策略等事实，组装成 [`ValidationInputs`]；
//! - 契约层用**纯函数**完成六项校验与夹紧，产出 [`ValidatedPlan`]；
//! - [`ValidatedPlan`] 的字段**私有**，唯一构造点是 [`validate_plan`]——外部既不能
//!   手造，也不能绕过校验拿到执行层需要的计划。
//!
//! 端口 trait [`RetrievalPort`] 与既有 `KbServicePort` / `PcServicePort` 同模式：
//! V1 进程内实现（ADR-10 方案 A），Phase 7 换 HTTP 时代码不变。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ports::PortError;

// ── 调用方 ────────────────────────────────────────────────────────────────

/// 检索的授权主体（授权事实的来源）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalCaller {
    pub principal_id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    /// `interactive` / `api_key` / `unknown`（三分类既有口径）。
    pub auth_mode: String,
}

// ── 计划 ──────────────────────────────────────────────────────────────────

/// 检索意图（LLM 识别产物之一）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalIntent {
    FactualLookup,
    ImpactAnalysis,
    CompletenessCheck,
    Summary,
    Comparison,
}

/// 检索模式（路由结果）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalMode {
    PcOnly,
    KbOnly,
    PcKb,
    CrossDomain,
}

/// 检索通道。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalChannel {
    /// PC 结构化实体（四级匹配：精确 Key/编号 → 精确名称 → ILIKE → 上下文排序）。
    PcEntity,
    /// PC Revision 全文检索（PostgreSQL FTS）。
    PcFts,
    /// KB Section 索引（章节树 + 复合打分）。
    KbSection,
    /// KB 向量通道（WP-6 后启用）。
    KbVector,
    /// PC 向量通道（WP-6 后启用）。
    PcVector,
    /// 关系扩展（遍历；不参与 RRF）。
    Relationship,
}

/// 锚点实体（计划里指向要检索的知识）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityAnchor {
    /// `pc_object` / `kb_document`。
    pub kind: String,
    /// 领域内类型提示（可选）。
    #[serde(default)]
    pub type_hint: Option<String>,
    /// 对象 key / 名称 / 编号。
    pub key_or_name: String,
}

/// 遍历规格。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TraversalSpec {
    pub enabled: bool,
    /// 上限 5（默认 3）。
    pub max_hops: u8,
}

/// 分析规格。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSpec {
    /// `coverage_gap` / `impact` / `none`。
    pub kind: String,
    #[serde(default)]
    pub require_evidence: bool,
}

/// 检索预算（硬上限，校验器夹紧）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RetrievalBudget {
    pub max_candidates: usize,
    pub max_hops: u8,
    pub timeout_ms: u64,
}

/// 检索计划（LLM 产物 / 确定性降级产物）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalPlan {
    pub intent: RetrievalIntent,
    pub mode: RetrievalMode,
    /// 域标签（`PC` / `KB`）。
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub retrieval_modes: Vec<RetrievalChannel>,
    #[serde(default)]
    pub entities: Vec<EntityAnchor>,
    /// 查询改写（1~3 条，多通道共用）。
    #[serde(default)]
    pub rewritten_queries: Vec<String>,
    #[serde(default)]
    pub traversal: Option<TraversalSpec>,
    #[serde(default)]
    pub analysis: Option<AnalysisSpec>,
    /// 显式要求读取历史正式版本（受校验器第 ③ 项门控）。
    #[serde(default)]
    pub include_history: bool,
    #[serde(default)]
    pub budget: Option<RetrievalBudget>,
}

/// 计划来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalPlanSource {
    LlmPlanner,
    DeterministicFallback,
}

/// 降级原因码（机器可读；解释由前端/LLM 生成）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetrievalDegradationReason {
    PlannerDisabled,
    ProjectModelPolicyMissing,
    ProviderUnavailable,
    PlannerTimeout,
    InvalidPlannerOutput,
    PlannerExecutionError,
}

impl RetrievalDegradationReason {
    /// 稳定原因码字符串（响应契约用）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PlannerDisabled => "planner_disabled",
            Self::ProjectModelPolicyMissing => "project_model_policy_missing",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::PlannerTimeout => "planner_timeout",
            Self::InvalidPlannerOutput => "invalid_planner_output",
            Self::PlannerExecutionError => "planner_execution_error",
        }
    }
}

// ── 校验 ──────────────────────────────────────────────────────────────────

/// 校验输入（IO 事实；由应用层收集）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationInputs {
    /// ① 当前用户是该项目成员且项目可见（应用层查库得出）。
    pub project_visible: bool,
    /// ② 允许跨域关联检索（V1 = 同项目恒真；跨项目恒假——与调用方 project 绑定）。
    pub cross_domain_allowed: bool,
    /// ③ 允许读取已发布历史版本。
    pub allow_historical: bool,
    /// ④ 关系类型白名单（`pc.relationship_type` 注册表 active 集）。
    #[serde(default)]
    pub allowed_relationship_types: Vec<String>,
    /// ⑥ 命中敏感/不可访问知识（密级钩子；V1 无密级元数据 = false）。
    #[serde(default)]
    pub sensitive_blocked: bool,
}

/// 单项校验结论。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheck {
    /// 校验项标识（`project_membership` / `cross_domain` / `history` /
    /// `relationship_whitelist` / `depth_budget` / `sensitive` / `query_safety`）。
    pub id: String,
    pub passed: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

/// 校验报告（逐项 pass/deny + 原因；随响应返回并进审计）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub checks: Vec<ValidationCheck>,
}

impl ValidationReport {
    /// 全部通过？
    pub fn all_passed(&self) -> bool {
        self.checks.iter().all(|c| c.passed)
    }
}

/// **已校验计划**——字段私有，唯一构造点 [`validate_plan`]。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedPlan {
    plan: RetrievalPlan,
    report: ValidationReport,
    /// 校验器核对过的租户/项目（执行层的唯一数据边界——执行只查这两个上下文）。
    tenant_id: Uuid,
    project_id: Uuid,
}

impl ValidatedPlan {
    /// 计划本体（只读）。
    pub fn plan(&self) -> &RetrievalPlan {
        &self.plan
    }

    /// 校验报告。
    pub fn report(&self) -> &ValidationReport {
        &self.report
    }

    /// 已核租户（执行层数据边界）。
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    /// 已核项目（执行层数据边界）。
    pub fn project_id(&self) -> Uuid {
        self.project_id
    }
}

/// 预算硬上限（裁决 §五）。
pub const MAX_HOPS_LIMIT: u8 = 5;
pub const MAX_CANDIDATES_LIMIT: usize = 60;
pub const MAX_TIMEOUT_MS: u64 = 8_000;

/// 确定性降级计划（单一执行路径：PC+KB 双域直查，不允许 LLM 指定数据源或 SQL）。
pub fn deterministic_fallback_plan(query: &str) -> RetrievalPlan {
    RetrievalPlan {
        intent: RetrievalIntent::FactualLookup,
        mode: RetrievalMode::PcKb,
        domains: vec!["PC".to_string(), "KB".to_string()],
        retrieval_modes: vec![
            RetrievalChannel::PcEntity,
            RetrievalChannel::PcFts,
            RetrievalChannel::KbSection,
        ],
        entities: Vec::new(),
        rewritten_queries: vec![query.to_string()],
        traversal: None,
        analysis: None,
        include_history: false,
        budget: Some(RetrievalBudget {
            max_candidates: MAX_CANDIDATES_LIMIT,
            max_hops: 0,
            timeout_ms: MAX_TIMEOUT_MS,
        }),
    }
}

/// 计划校验（**纯函数**）。
///
/// 六项校验 + 预算夹紧 + 查询安全检查；任一 deny 即整体拒绝（返回报告）。
/// 通过时产出 [`ValidatedPlan`]——执行层的唯一入口类型。
pub fn validate_plan(
    mut plan: RetrievalPlan,
    _caller: &RetrievalCaller,
    inputs: &ValidationInputs,
) -> Result<ValidatedPlan, ValidationReport> {
    let mut checks: Vec<ValidationCheck> = Vec::new();

    // ① 项目成员可见性。
    checks.push(if inputs.project_visible {
        ValidationCheck {
            id: "project_membership".into(),
            passed: true,
            reason: None,
        }
    } else {
        ValidationCheck {
            id: "project_membership".into(),
            passed: false,
            reason: Some("caller is not a visible member of this project".into()),
        }
    });

    // ② 跨域访问（V1 = 同项目内允许；跨项目永久禁止——与调用方 project 绑定）。
    let wants_cross_domain = plan.mode == RetrievalMode::CrossDomain
        || plan
            .retrieval_modes
            .contains(&RetrievalChannel::Relationship);
    checks.push(if wants_cross_domain && !inputs.cross_domain_allowed {
        ValidationCheck {
            id: "cross_domain".into(),
            passed: false,
            reason: Some("cross-domain retrieval is not allowed for this caller".into()),
        }
    } else {
        ValidationCheck {
            id: "cross_domain".into(),
            passed: true,
            reason: None,
        }
    });

    // ③ 历史版本读取。
    checks.push(if plan.include_history && !inputs.allow_historical {
        ValidationCheck {
            id: "history".into(),
            passed: false,
            reason: Some("reading historical versions is not permitted".into()),
        }
    } else {
        ValidationCheck {
            id: "history".into(),
            passed: true,
            reason: None,
        }
    });

    // ④ 关系类型白名单（关系通道开启时必须有非空白名单）。
    let wants_relationship = plan
        .retrieval_modes
        .contains(&RetrievalChannel::Relationship);
    checks.push(
        if wants_relationship && inputs.allowed_relationship_types.is_empty() {
            ValidationCheck {
                id: "relationship_whitelist".into(),
                passed: false,
                reason: Some("relationship traversal requires a non-empty type whitelist".into()),
            }
        } else {
            ValidationCheck {
                id: "relationship_whitelist".into(),
                passed: true,
                reason: None,
            }
        },
    );

    // ⑤ 深度与预算夹紧（不 deny——夹回合法值并记录）。
    let budget = plan.budget.get_or_insert(RetrievalBudget {
        max_candidates: MAX_CANDIDATES_LIMIT,
        max_hops: MAX_HOPS_LIMIT.min(3),
        timeout_ms: MAX_TIMEOUT_MS,
    });
    let clamped_candidates = budget.max_candidates.min(MAX_CANDIDATES_LIMIT);
    let clamped_hops = budget.max_hops.min(MAX_HOPS_LIMIT);
    let clamped_timeout = budget.timeout_ms.min(MAX_TIMEOUT_MS);
    let clamped = clamped_candidates != budget.max_candidates
        || clamped_hops != budget.max_hops
        || clamped_timeout != budget.timeout_ms;
    budget.max_candidates = clamped_candidates;
    budget.max_hops = clamped_hops;
    budget.timeout_ms = clamped_timeout;
    checks.push(ValidationCheck {
        id: "depth_budget".into(),
        passed: true,
        reason: clamped.then(|| {
            format!(
                "budget clamped to candidates<={MAX_CANDIDATES_LIMIT}, hops<={MAX_HOPS_LIMIT}, timeout<={MAX_TIMEOUT_MS}ms"
            )
        }),
    });

    // ⑥ 敏感知识（密级钩子）。
    checks.push(if inputs.sensitive_blocked {
        ValidationCheck {
            id: "sensitive".into(),
            passed: false,
            reason: Some("query touches sensitive or inaccessible knowledge".into()),
        }
    } else {
        ValidationCheck {
            id: "sensitive".into(),
            passed: true,
            reason: None,
        }
    });

    // 查询安全：拒绝疑似 SQL / 内部表名的改写串（恶意计划输出防御）。
    let unsafe_query = plan
        .rewritten_queries
        .iter()
        .map(String::as_str)
        .chain(plan.entities.iter().map(|e| e.key_or_name.as_str()))
        .find(|q| looks_like_sql(q));
    checks.push(match unsafe_query {
        Some(q) => ValidationCheck {
            id: "query_safety".into(),
            passed: false,
            reason: Some(format!(
                "query looks like SQL or an internal table reference: {q}"
            )),
        },
        None => ValidationCheck {
            id: "query_safety".into(),
            passed: true,
            reason: None,
        },
    });

    let report = ValidationReport { checks };
    if report.all_passed() {
        Ok(ValidatedPlan {
            plan,
            report,
            tenant_id: _caller.tenant_id,
            project_id: _caller.project_id,
        })
    } else {
        Err(report)
    }
}

/// 疑似 SQL / 内部表名的启发式判定（纯字符串检查，无解析器依赖）。
fn looks_like_sql(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    const SQL_MARKERS: [&str; 8] = [
        "select ", " from ", " where ", " join ", "delete ", "update ", "drop ", ";",
    ];
    if SQL_MARKERS.iter().any(|m| lower.contains(m)) {
        return true;
    }
    // 内部表名/域前缀（schema.table 形态）。
    lower.contains("integration.") || lower.contains("governance.") || lower.contains("core.")
}

// ── 证据与遍历（WP-4 定版形状；wire 契约层）──────────────────────────────

/// 知识域。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KnowledgeDomain {
    #[serde(rename = "PC")]
    Pc,
    #[serde(rename = "KB")]
    Kb,
    #[serde(rename = "REQUIREMENT")]
    Requirement,
}

impl KnowledgeDomain {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pc => "PC",
            Self::Kb => "KB",
            Self::Requirement => "REQUIREMENT",
        }
    }
}

/// 权威状态（五值词表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityStatus {
    CurrentPublished,
    HistoricalPublished,
    DraftCandidate,
    Superseded,
    Speculative,
}

impl AuthorityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CurrentPublished => "current_published",
            Self::HistoricalPublished => "historical_published",
            Self::DraftCandidate => "draft_candidate",
            Self::Superseded => "superseded",
            Self::Speculative => "speculative",
        }
    }
}

/// 时效性分桶（三档 + 缺失值 Unknown）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessBucket {
    Recent,
    WithinYear,
    Older,
    Unknown,
}

/// 遍历方向（反向遍历不改关系语义，只改路径方向）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraversalDirection {
    Forward,
    Reverse,
}

/// 遍历边。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalEdge {
    pub relationship_id: String,
    pub relationship_type: String,
    pub source_id: String,
    pub target_id: String,
    pub traversal_direction: TraversalDirection,
}

/// 遍历缺口（六分类）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalGap {
    /// `no_registered_relationship` / `no_active_edge` / `endpoint_not_allowed` /
    /// `traversal_depth_exceeded` / `candidate_budget_exceeded` / `traversal_timeout`。
    pub kind: String,
    pub detail: String,
}

/// 遍历结果。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraversalResult {
    pub nodes: Vec<String>,
    pub edges: Vec<TraversalEdge>,
    pub gaps: Vec<TraversalGap>,
    pub truncated: bool,
    #[serde(default)]
    pub truncation_reason: Option<String>,
}

/// 单条检索证据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalEvidence {
    /// 结构化证据身份（与 `[E-n]` 临时引用编号分离）。
    pub evidence_id: String,
    pub source_domain: KnowledgeDomain,
    pub source_id: String,
    #[serde(default)]
    pub revision_id: Option<String>,
    pub authority_status: AuthorityStatus,
    pub project_id: Uuid,
    /// 仅 Relevance 维；五维分列，禁止折算单一总分。
    #[serde(default)]
    pub relevance_score: Option<f32>,
    pub retrieval_channel: RetrievalChannel,
    #[serde(default)]
    pub title: Option<String>,
    pub content_excerpt: String,
    #[serde(default)]
    pub locator: Option<String>,
    #[serde(default)]
    pub freshness: Option<FreshnessBucket>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub relationship_path: Vec<TraversalEdge>,
    #[serde(default)]
    pub speculative: bool,
}

/// 预算回执。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BudgetReport {
    pub text_evidence_count: usize,
    pub text_evidence_limit: usize,
    pub text_chars_used: usize,
    pub text_chars_limit: usize,
    pub traversal_edges_count: usize,
    pub traversal_edges_limit: usize,
    pub truncated_excerpt_count: usize,
    pub dropped_evidence_count: usize,
    pub dropped_traversal_path_count: usize,
    pub budget_exhausted: bool,
}

/// 证据包（LLM 消费面）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvidenceBundle {
    pub evidence: Vec<RetrievalEvidence>,
    pub traversal: TraversalResult,
    pub gaps: Vec<TraversalGap>,
    /// 机器可读原因码（WP-4 §6.4③）。
    pub degradations: Vec<String>,
    pub budget_report: BudgetReport,
}

/// 通道命中集合（融合前）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelHits {
    pub hits: Vec<RetrievalEvidence>,
    /// 不可用通道的原因码（如 `kb_section_index_unavailable`）——进 degradations。
    #[serde(default)]
    pub unavailable_codes: Vec<String>,
}

// ── 端口 ──────────────────────────────────────────────────────────────────

/// 规划模型调用失败（映射到降级原因码）。
#[derive(Debug, Clone, thiserror::Error)]
pub enum PlannerModelError {
    /// 项目未指定模型策略且无可用 Team Provider（→ `project_model_policy_missing`）。
    #[error("no project model policy and no enabled team provider")]
    NoProvider,
    /// 指定的 Provider 存在但不可用（→ `provider_unavailable`）。
    #[error("model provider is unavailable: {0}")]
    ProviderUnavailable(String),
    /// 超时（→ `planner_timeout`）。
    #[error("planner model call timed out")]
    Timeout,
    /// 模型输出不合规（→ `invalid_planner_output`）。
    #[error("planner model returned unusable output: {0}")]
    InvalidOutput(String),
    /// 其他执行错误（→ `planner_execution_error`）。
    #[error("planner model call failed: {0}")]
    Execution(String),
}

/// 规划模型端口（LLM 产计划；实现在 pm-ai，复用既有模型供给线）。
///
/// **纪律**：模型只返回**计划**，不返回数据、不决定访问权限——权限由
/// [`validate_plan`] 裁决。
#[async_trait]
pub trait PlannerModel: Send + Sync {
    /// 产出检索计划（JSON 文本，由调用方解析）。
    ///
    /// `project_id` 必传——**模型策略是项目级的**（`ai.project_ai_model_policy`
    /// + Team Provider 三级解析）。
    async fn plan(&self, project_id: Uuid, query: &str) -> Result<String, PlannerModelError>;
}

/// 统一检索端口（V1 进程内；Phase 7 换 HTTP）。
///
/// **纪律**：`execute` 只接受 [`ValidatedPlan`]——未经校验的计划在类型上无法进入
/// 执行层；`expand` 走同一套授权与校验。
#[async_trait]
pub trait RetrievalPort: Send + Sync {
    /// 生成检索计划（LLM 产计划；失败走确定性降级）。
    async fn plan(&self, query: &str, caller: &RetrievalCaller)
        -> Result<RetrievalPlan, PortError>;
    /// 校验计划（确定性）。
    async fn validate(
        &self,
        plan: RetrievalPlan,
        caller: RetrievalCaller,
    ) -> Result<ValidatedPlan, ValidationReport>;
    /// 执行多通道检索。
    async fn execute(&self, plan: ValidatedPlan) -> Result<ChannelHits, PortError>;
    /// 受控跨域关系扩展（WP-2 接真）。
    ///
    /// **入参是 `ValidatedPlan`（不是裸 seed）**——扩展必须走同一套授权与校验：
    /// 已校验计划天然携带**已核租户/项目**（遍历的数据边界）与跳数预算。
    async fn expand(&self, plan: &ValidatedPlan) -> Result<TraversalResult, PortError>;
    /// 组装可引用证据。
    async fn evidence(
        &self,
        hits: &ChannelHits,
        traversal: &TraversalResult,
    ) -> Result<EvidenceBundle, PortError>;
}
