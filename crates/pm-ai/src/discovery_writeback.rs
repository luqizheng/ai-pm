//! DK-D · 对话编排写回通道（设计稿 `docs/product/ai-pm-dialog-orchestration-design-v1.md`
//! §3（需求落点）/§6（写回通道）/§7（任务分解）；裁决 D-DK-4/5/8/11）。
//!
//! 本模块是 **hook 接口定义方**（DK-D 批 §范围 1）：DK-B 的第八条 worker 循环在
//! 会话跃迁到 `REVIEW_REQUIRED` 后调用 [`WriteBackHook::on_review_required`]，
//! 扫描到「approved 修订且该修订无 `task_requirement` 行」时调用
//! [`WriteBackHook::decompose_if_approved`]。DK-B 合并时把 [`NoWriteBack`] 换成
//! [`DiscoveryWriteBack`]（后合者适配），不需要改本模块的签名。
//!
//! 三条硬边界（本模块的存在理由）：
//! 1. **AI 永不可达 `approved`**：白名单 [`AI_WRITABLE_COMMANDS`] 是常量不是模型
//!    输出（§6.2 第 4 道闸）；`confirm`/`reject`/`lifecycle`/`link_finding`/
//!    `register_source_input` 一律不在白名单里，且服务层 `require_human` 是第二道。
//! 2. **AI 永不置 `ready`**：任务只 `create_task(status='draft')`；整批
//!    `draft → ready` 是人类动作（§7.4，E2/E5）。
//! 3. **一切写经既有单点服务**（V-6）：本模块零直写业务表，只作参数组装、系统侧
//!    骨架校验与记账。
//!
//! 开关：`AI_PM_DISCOVERY_ORCHESTRATOR=off|on`，**默认 off**，拼错值 WARN + off
//! （与 `AI_PM_CONVERSATION_REPLY` / `AI_PM_RUN_DISPATCH` 同传统：新的计费循环
//! 不得在升级时静默启动）。本模块导出该开关的唯一解析入口，DK-B 的循环复用
//! [`WriteBackMode::from_env`]，避免两处定义漂移。
//!
//! 记账（D-DK-11）：每次真模型调用落一条 `ai.llm_call_record`，
//! `purpose='discovery_orchestration'`（[`crate::call_record::PURPOSE_DISCOVERY_ORCHESTRATION`]），
//! 使「编排成本」与「闲聊成本」在账本上可分。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use pm_application::discovery::DiscoverySessionView;
use pm_application::pm_write::PmWriteError;

use crate::model::ModelError;

/// 编排器开关（D-DK-9：默认 off）。
pub const SWITCH_ENV: &str = "AI_PM_DISCOVERY_ORCHESTRATOR";

/// 任务分解的系统上限（§7.3：系统定数量，LLM 只回填内容）。
pub const MAX_PREREQUISITE_TASKS: usize = 3;
pub const MAX_MAIN_TASKS: usize = 8;

/// 编排器（AI 主体）可触发的写命令白名单（§3.3 表 ✅ 行）。
///
/// 这是 §6.2 的**第 4 道闸**：白名单是常量，不是模型输出——模型在回复里写
/// "我要确认" 也不会改变本表。新增项必须同时改设计稿裁决，否则视为越权。
pub const AI_WRITABLE_COMMANDS: [&str; 4] = [
    "create_requirement",
    "create_draft",
    "update_draft",
    "submit_revision",
];

/// 编排器**禁止**触发的写命令（§3.3 表 ❌ 行；负向断言用）。
///
/// `mark_task_ready` 不是既有服务函数名，而是「任务 `draft → ready`」这个人类
/// 动作的语义标签（§7.4）——列在这里是为了让"AI 不能置 ready" 有可断言的白名单
/// 事实，而不是只写在注释里。
pub const AI_FORBIDDEN_COMMANDS: [&str; 6] = [
    "confirm_revision",
    "reject_revision",
    "transition_lifecycle",
    "link_finding",
    "register_source_input",
    "mark_task_ready",
];

// ─────────────────────────────────────────────────────────────── 开关

/// `AI_PM_DISCOVERY_ORCHESTRATOR` 的取值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteBackMode {
    Off,
    On,
}

impl WriteBackMode {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("on") => Self::On,
            Some("off") | None => Self::Off,
            // 拼错值绝不静默开启计费循环。
            Some(other) => {
                tracing::warn!(
                    value = other,
                    "unknown AI_PM_DISCOVERY_ORCHESTRATOR value (expected off|on); staying off"
                );
                Self::Off
            }
        }
    }

    pub fn from_env() -> Self {
        Self::parse(std::env::var(SWITCH_ENV).ok().as_deref())
    }

    pub fn enabled(self) -> bool {
        matches!(self, Self::On)
    }
}

// ─────────────────────────────────────────────────────────────── 结果

/// 需求起草 hook 的三种归宿（§8.1 前半：AI 到此止步于 `pending_confirmation`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftOutcomeKind {
    /// 起草完成：`create_requirement → update_draft → submit_revision`，停在
    /// `pending_confirmation`（**不是** `approved`）。
    Submitted,
    /// 该 session 已经起草过需求（幂等重跑 / 人类已确认或正在复核）。
    AlreadyDrafted,
    /// 前置不满足（开关 off / 会话状态不对 / 无 CONFIRMED finding）——不写任何行。
    Skipped,
}

#[derive(Debug, Clone)]
pub struct DraftOutcome {
    pub kind: DraftOutcomeKind,
    pub requirement_id: Option<Uuid>,
    pub revision_id: Option<Uuid>,
    pub revision_state: Option<String>,
    /// 写进 `content.basis_finding_ids` 的提案级来源标记（非权威，§3.3）。
    pub basis_finding_ids: Vec<Uuid>,
    /// Skipped/AlreadyDrafted 的原因（结构化 WARN 用，不是给用户的文案）。
    pub reason: Option<&'static str>,
}

impl DraftOutcome {
    fn skipped(reason: &'static str) -> Self {
        Self {
            kind: DraftOutcomeKind::Skipped,
            requirement_id: None,
            revision_id: None,
            revision_state: None,
            basis_finding_ids: Vec::new(),
            reason: Some(reason),
        }
    }
}

/// 生成出来的一条任务及其在批次里的角色（§7.2：前置任务 vs 主任务）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchTaskKind {
    /// 使主任务可执行的条件（例："接入短信服务商并配置模板"）。
    Prerequisite,
    /// 直接交付验收项的任务。
    Main,
}

#[derive(Debug, Clone)]
pub struct GeneratedTask {
    pub task_id: Uuid,
    pub title: String,
    pub kind: BatchTaskKind,
    /// 落进 `delivery.task_dependency` 的上游任务 id（E3：不是标题字符串语义）。
    pub depends_on: Vec<Uuid>,
    pub acceptance_criteria: usize,
    pub ai_role_version_id: Option<Uuid>,
}

/// 一批任务分解的结果（E1/E2/E5 的读数面）。
#[derive(Debug, Clone)]
pub struct TaskBatchOutcome {
    pub requirement_id: Uuid,
    /// `task_requirement.revision_id` 指向的修订（必为 approved，E1）。
    pub revision_id: Uuid,
    /// 前置任务在前、主任务按依赖序在后。
    pub tasks: Vec<GeneratedTask>,
    pub prerequisite_count: usize,
    pub main_count: usize,
    /// 超出 §7.3 系统上限被截断丢弃的条数（WARN 已记）。
    pub truncated: usize,
    /// 角色建议越界（不在本项目 `ai_role_version` 候选里）被丢弃的条数。
    pub dropped_role_suggestions: usize,
}

impl TaskBatchOutcome {
    /// 本批落库的任务总数（E2：全部 `status='draft'`）。
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

/// 任务分解 hook 的归宿。
#[derive(Debug, Clone)]
pub enum DecomposeOutcome {
    /// 已生成一批任务（或本轮无候选——见 `tasks` 是否为空与 `reason`）。
    Generated(TaskBatchOutcome),
    /// 本轮无需生成：本 session 尚无需求，或候选需求都已挂 `task_requirement`。
    NothingToDo { reason: &'static str },
}

// ─────────────────────────────────────────────────────────────── 错误

/// 写回通道错误。
///
/// B5（`RequirementNotApproved`）是唯一面向用户的拒绝：它的文案必须能指向
/// "请先在需求页确认"，因为「AI 怎么停下来了」是 §3.4 明说的体验风险点。
#[derive(Debug, thiserror::Error)]
pub enum WriteBackError {
    #[error("discovery write-back is disabled ({SWITCH_ENV}=off)")]
    Disabled,
    #[error(
        "requirement {requirement_id} is not approved — 请先在需求页确认该需求（approved）后再生成任务"
    )]
    RequirementNotApproved { requirement_id: Uuid },
    /// LLM 输出结构性不合规（缺验收口径 / 依赖引用不存在 / 超出角色候选）——
    /// 整批拒绝，不落半批（§4.5 同款纪律：丢弃本轮产出）。
    #[error("task batch rejected: {0}")]
    BatchRejected(String),
    #[error("model failure: {0}")]
    Model(#[from] ModelError),
    #[error("discovery write-back db error: {0}")]
    Db(#[from] sqlx::Error),
    /// 既有单点服务的错误（`Forbidden` 即 §6.2 第 1 道闸的真实生效证据）。
    /// `PmWriteError` 未实现 `Display`/`Error`（pm-application 既有边界），故手写
    /// `From` 并以 `Debug` 呈现——错误归类靠 [`WriteBackError::is_forbidden`]。
    #[error("requirement service error: {0:?}")]
    Requirement(PmWriteError),
    #[error("discovery error: {0}")]
    Discovery(String),
}

impl From<PmWriteError> for WriteBackError {
    fn from(error: PmWriteError) -> Self {
        WriteBackError::Requirement(error)
    }
}

impl WriteBackError {
    /// 是否被服务层真人闸拒绝（B1/B2/B4 的 `Forbidden` 判据）。
    pub fn is_forbidden(&self) -> bool {
        matches!(
            self,
            WriteBackError::Requirement(PmWriteError::Forbidden(_))
        )
    }
}

// ─────────────────────────────────────────────────────────────── 端口

/// 一次模型调用的 provider 标识（记账用；`purpose='discovery_orchestration'`）。
#[derive(Debug, Clone)]
pub struct ProviderStamp {
    pub provider_id: Uuid,
    pub model: String,
}

/// 模型回包 + 记账信息。`latency_ms` 由适配器侧墙钟测得（与既有分析链同口径）。
#[derive(Debug, Clone)]
pub struct ModelReply<T> {
    pub value: T,
    pub provider: ProviderStamp,
    pub latency_ms: i64,
}

/// 注入给模型的一条访谈事实（只读投影；S6 的裁剪版）。
#[derive(Debug, Clone, Serialize)]
pub struct FindingFact {
    pub id: Uuid,
    pub kind: String,
    pub content: String,
}

/// 项目能力面（§7.1 三源之二；DK-C 注入投影的**只读**投影面）。
///
/// 本批只定义形状与空实现——DK-C 的能力投影模块并行在跑，本模块不碰它；
/// 合并后由 DK-C 提供具体实现（后合者适配）。
#[derive(Debug, Clone, Default, Serialize)]
pub struct CapabilityFacts {
    /// PC 认知对象（`pc.cognition_object`，0053）。
    pub objects: Vec<CapabilityObject>,
    /// KB digest（`kb.document`，`doc_key='repo-{id}'`）。
    pub digests: Vec<CapabilityDigest>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityObject {
    pub object_key: String,
    pub object_type: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityDigest {
    pub doc_key: String,
    pub summary: String,
}

/// 只读能力面端口（DK-C 实现；本批 [`EmptyCapabilityProjection`]）。
#[async_trait]
pub trait CapabilityProjection: Send + Sync {
    async fn project(&self, project_id: Uuid) -> Result<CapabilityFacts, WriteBackError>;
}

/// 空投影：分解仍然可跑（三源之二的贡献退化为空），用于 DK-C 未合并时。
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyCapabilityProjection;

#[async_trait]
impl CapabilityProjection for EmptyCapabilityProjection {
    async fn project(&self, _project_id: Uuid) -> Result<CapabilityFacts, WriteBackError> {
        Ok(CapabilityFacts::default())
    }
}

/// 项目角色候选（§7.1 三源之三：`ai.ai_role_version`，角色只从候选选）。
#[derive(Debug, Clone, Serialize)]
pub struct RoleCandidate {
    pub version_id: Uuid,
    pub role_id: Uuid,
    /// 角色键（模板 `role_key`，如 `"developer"`）——LLM 用它提建议。
    pub role_key: String,
    pub version: String,
}

/// 需求起草的模型输入。
#[derive(Debug, Clone)]
pub struct RequirementDraftContext {
    pub project_id: Uuid,
    pub session_id: Uuid,
    pub session_kind: String,
    pub findings: Vec<FindingFact>,
    pub capability: CapabilityFacts,
}

/// 模型起草出的需求草案（系统侧只补 `basis_finding_ids`，不让模型碰）。
#[derive(Debug, Clone, Deserialize)]
pub struct RequirementDraft {
    /// 需求编号建议（系统会归一化并处理同项目重号）。
    #[serde(default)]
    pub code: Option<String>,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub markdown: String,
}

/// 模型回填的一条任务（§7.3：系统定骨架，LLM 只回填 title/描述/验收/依赖建议）。
#[derive(Debug, Clone, Deserialize)]
pub struct RawTask {
    /// 批内引用键（依赖建议用键而不是标题字符串，避免标题改字即断边）。
    #[serde(default)]
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    /// 必填：无验收口径的任务一律不生成（E4）。
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    /// 依赖建议（本批内更早出现的任务键）。
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// 角色建议（`RoleCandidate::role_key`）；越界即丢弃（不报错、不编造）。
    #[serde(default)]
    pub role_ref: Option<String>,
}

/// 模型回填的整批任务骨架（系统已按 §7.2 分成两类）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawTaskBatch {
    #[serde(default)]
    pub prerequisite: Vec<RawTask>,
    #[serde(default)]
    pub main: Vec<RawTask>,
}

/// 任务分解的模型输入（三源齐备）。
#[derive(Debug, Clone)]
pub struct TaskDecomposeContext {
    pub project_id: Uuid,
    pub requirement_id: Uuid,
    pub requirement_code: String,
    pub requirement_title: String,
    /// approved 修订正文（§7.1 源一）。
    pub markdown: String,
    /// `requirement_acceptance_criterion`（0056）的验收口径文本。
    pub acceptance_criteria: Vec<String>,
    /// 只读能力面（源二）。
    pub capability: CapabilityFacts,
    /// 角色候选（源三）。
    pub roles: Vec<RoleCandidate>,
}

/// 写回通道所需的模型能力（真适配器 = `WriteBackModelAdapter`；测试 =
/// `FakeWriteBackModel`）。与既有 `AnalysisModel` 同款拆分：本模块只定义端口，
/// provider 解析/HTTP 细节归适配器。
#[async_trait]
pub trait WriteBackModel: Send + Sync {
    async fn draft_requirement(
        &self,
        ctx: &RequirementDraftContext,
    ) -> Result<ModelReply<RequirementDraft>, ModelError>;

    async fn decompose_tasks(
        &self,
        ctx: &TaskDecomposeContext,
    ) -> Result<ModelReply<RawTaskBatch>, ModelError>;
}

// ─────────────────────────────────────────────────────────────── Hook

/// 写回通道接口（DK-B 调用方唯一需要知道的形状）。
///
/// `tenant_id` 显式传入：`DiscoverySessionView` 不携带租户（会话行有该列，
/// 视图没有），而所有既有单点服务都以 `tenant_id` 为第一边界参数——把边界
/// 显式摆出来比让 hook 自己反查更安全。
#[async_trait]
pub trait WriteBackHook: Send + Sync {
    /// `REVIEW_REQUIRED` 跃迁后调用：起草需求草案并停在 `pending_confirmation`。
    async fn on_review_required(
        &self,
        tenant_id: Uuid,
        session: &DiscoverySessionView,
    ) -> Result<DraftOutcome, WriteBackError>;

    /// 扫描到「approved 修订且无 `task_requirement` 行」时调用：生成 draft 任务批。
    ///
    /// 前置门（B5）：需求未 approved → [`WriteBackError::RequirementNotApproved`]，
    /// 文案指向「请先在需求页确认」。
    async fn decompose_if_approved(
        &self,
        tenant_id: Uuid,
        session: &DiscoverySessionView,
    ) -> Result<DecomposeOutcome, WriteBackError>;
}

/// DK-B 合并前的空实现：接口先落地，行为后接（本批已提供
/// [`DiscoveryWriteBack`]，DK-B 只需换类型）。
#[derive(Debug, Clone, Copy, Default)]
pub struct NoWriteBack;

#[async_trait]
impl WriteBackHook for NoWriteBack {
    async fn on_review_required(
        &self,
        _tenant_id: Uuid,
        _session: &DiscoverySessionView,
    ) -> Result<DraftOutcome, WriteBackError> {
        Ok(DraftOutcome::skipped("no_write_back_hook"))
    }

    async fn decompose_if_approved(
        &self,
        _tenant_id: Uuid,
        _session: &DiscoverySessionView,
    ) -> Result<DecomposeOutcome, WriteBackError> {
        Ok(DecomposeOutcome::NothingToDo {
            reason: "no_write_back_hook",
        })
    }
}
