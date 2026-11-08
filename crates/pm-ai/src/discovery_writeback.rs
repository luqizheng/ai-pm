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
    /// 依赖落库失败（`task_dependency::add_dependency`；环/跨项目/状态不允许）。
    #[error("task dependency error: {0}")]
    Dependency(String),
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

/// 模型调用失败 + 记账信息（DEC-LLM-004.7：失败调用同样入账，不只记成功）。
/// `provider` 为 `None` = 失败发生在 provider 解析之前（无 FK 可锚）。
#[derive(Debug, Clone)]
pub struct ModelCallFailure {
    pub error: ModelError,
    pub provider: Option<ProviderStamp>,
    pub latency_ms: i64,
}

impl From<ModelError> for ModelCallFailure {
    fn from(error: ModelError) -> Self {
        Self {
            error,
            provider: None,
            latency_ms: 0,
        }
    }
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
    ) -> Result<ModelReply<RequirementDraft>, ModelCallFailure>;

    async fn decompose_tasks(
        &self,
        ctx: &TaskDecomposeContext,
    ) -> Result<ModelReply<RawTaskBatch>, ModelCallFailure>;
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

// ─────────────────────────────────────────────────────────── 实现

/// 本 session 起草出的需求（按 `content.basis_finding_ids` 与 session 的
/// CONFIRMED finding 求交认定——D-DK-4 不建信封，这个交集就是"归属"事实）。
///
/// 同时带**最新修订**（幂等判定与"人还在复核"的读数）与**当前正式修订**
/// （`current_revision_id`，只可能是 approved；任务钉定只认它，§3.4）。
#[derive(Debug, Clone)]
struct SessionRequirement {
    requirement_id: Uuid,
    requirement_code: String,
    requirement_title: String,
    latest_revision_id: Uuid,
    latest_revision_version: i32,
    latest_state: String,
    /// 当前正式修订（`NULL` = 尚无定稿版本 → B5 的拒绝判据）。
    approved_revision_id: Option<Uuid>,
    approved_revision_version: Option<i32>,
    approved_markdown: String,
}

impl SessionRequirement {
    fn approved(&self) -> bool {
        self.approved_revision_id.is_some()
    }
}

/// [`session_requirements`] 的原始行（列多，独立结构体避免超长元组——
/// 也避免 clippy `type_complexity`）。
#[derive(Debug, sqlx::FromRow)]
struct SessionRequirementRow {
    id: Uuid,
    code: String,
    title: String,
    latest_revision_id: Uuid,
    latest_revision_version: i32,
    latest_state: String,
    approved_revision_id: Option<Uuid>,
    approved_revision_version: Option<i32>,
    approved_markdown: Option<String>,
}

/// 真实写回通道（DK-B 合并时用它替换 [`NoWriteBack`]）。
///
/// 依赖全部是既有单点服务与端口：`pg` = 既有服务的事务入口，`model` = 模型
/// 回填，`capability` = DK-C 只读投影，`knowledge` = `submit_revision` 的
/// Markdown 绑定点（S02-I2）。
pub struct DiscoveryWriteBack {
    pub pool: sqlx::PgPool,
    pub model: std::sync::Arc<dyn WriteBackModel>,
    pub capability: std::sync::Arc<dyn CapabilityProjection>,
    pub knowledge: pm_application::knowledge::KnowledgeRoot,
    pub mode: WriteBackMode,
}

impl DiscoveryWriteBack {
    /// 本 session 内已 CONFIRMED 的 finding（S6 的裁剪投影：只给模型看事实，
    /// 不给 verification 决策面）。
    async fn confirmed_findings(
        &self,
        tenant_id: Uuid,
        session_id: Uuid,
    ) -> Result<Vec<FindingFact>, WriteBackError> {
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            "SELECT id, kind::text, content FROM discovery.discovery_finding \
              WHERE tenant_id = $1 AND session_id = $2 AND verification_status = 'CONFIRMED' \
              ORDER BY created_at, id",
        )
        .bind(tenant_id)
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, kind, content)| FindingFact { id, kind, content })
            .collect())
    }

    /// AI 主体（与主链同一个 `ai.project_ai_config.ai_principal_id`）。
    ///
    /// 写命令的 actor 必须是 **ai_pm** principal：真人闸（`require_human`）正是
    /// 靠它区分主体，用 human 冒充 AI 会让 B1/B2 的负向断言失去意义。
    async fn ai_principal(&self, project_id: Uuid) -> Result<Uuid, WriteBackError> {
        let principal: Option<Uuid> = sqlx::query_scalar(
            "SELECT ai_principal_id FROM ai.project_ai_config WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        principal.ok_or_else(|| {
            WriteBackError::Discovery("project has no ai_principal_id configured".to_string())
        })
    }

    /// 由 basis 交集认定的「本 session 的需求」。
    ///
    /// 交集扫**全部修订**（不只最新）：起草标记可能落在被后续修订取代的历史
    /// 修订上，只扫最新会让归属在同一需求被人类续改后消失，从而重复建需求。
    /// 同时返回最新修订与当前正式修订（任务钉定只认后者，§3.4）。
    async fn session_requirements(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        basis_ids: &[Uuid],
    ) -> Result<Vec<SessionRequirement>, WriteBackError> {
        if basis_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<SessionRequirementRow> = sqlx::query_as(
            "SELECT r.id, r.code, r.title, lat.id AS latest_revision_id, \
                    lat.version AS latest_revision_version, lat.state AS latest_state, \
                    cur.id AS approved_revision_id, cur.version AS approved_revision_version, \
                    cur.content->>'markdown' AS approved_markdown \
               FROM delivery.requirement r \
               JOIN LATERAL ( \
                   SELECT id, version, state \
                     FROM delivery.requirement_revision v \
                    WHERE v.requirement_id = r.id \
                    ORDER BY v.version DESC LIMIT 1 \
               ) lat ON TRUE \
               LEFT JOIN delivery.requirement_revision cur ON cur.id = r.current_revision_id \
              WHERE r.tenant_id = $1 AND r.project_id = $2 \
                AND EXISTS ( \
                    SELECT 1 FROM delivery.requirement_revision rv2 \
                     WHERE rv2.requirement_id = r.id \
                       AND EXISTS ( \
                           SELECT 1 FROM jsonb_array_elements_text( \
                                            coalesce(rv2.content->'basis_finding_ids', \
                                                     '[]'::jsonb)) e \
                            WHERE e.value::uuid = ANY($3))) \
              ORDER BY r.created_at, r.id",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(basis_ids)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| SessionRequirement {
                requirement_id: row.id,
                requirement_code: row.code,
                requirement_title: row.title,
                latest_revision_id: row.latest_revision_id,
                latest_revision_version: row.latest_revision_version,
                latest_state: row.latest_state,
                approved_revision_id: row.approved_revision_id,
                approved_revision_version: row.approved_revision_version,
                approved_markdown: row.approved_markdown.unwrap_or_default(),
            })
            .collect())
    }

    /// 需求级验收口径（`requirement_acceptance_criterion`，0056；§7.1 源一）。
    async fn acceptance_statements(
        &self,
        project_id: Uuid,
        requirement_id: Uuid,
    ) -> Result<Vec<String>, WriteBackError> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT statement FROM delivery.requirement_acceptance_criterion \
              WHERE project_id = $1 AND requirement_id = $2 \
              ORDER BY position, id",
        )
        .bind(project_id)
        .bind(requirement_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// 角色候选（§7.1 源三）：本项目**已启用**的 AI Role 的最新版本。
    /// LLM 只能从这些 `role_key` 里挑（越界丢弃，不编造角色）。
    async fn role_candidates(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<RoleCandidate>, WriteBackError> {
        let rows: Vec<(Uuid, Uuid, String, String)> = sqlx::query_as(
            "SELECT DISTINCT ON (pr.role_id) v.id, pr.role_id, t.role_key, v.version \
               FROM ai.ai_role_version v \
               JOIN ai.project_ai_role pr ON pr.id = v.project_role_id \
               JOIN ai.ai_role_template t ON t.id = pr.role_id \
              WHERE pr.tenant_id = $1 AND pr.project_id = $2 AND pr.enabled \
              ORDER BY pr.role_id, v.created_at DESC",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let mut candidates: Vec<RoleCandidate> = rows
            .into_iter()
            .map(|(version_id, role_id, role_key, version)| RoleCandidate {
                version_id,
                role_id,
                role_key,
                version,
            })
            .collect();
        candidates.sort_by(|a, b| a.role_key.cmp(&b.role_key));
        Ok(candidates)
    }

    /// 该需求修订是否已挂任务（`task_requirement`）——扫描谓词的排他面。
    async fn revision_has_tasks(
        &self,
        requirement_id: Uuid,
        revision_id: Uuid,
    ) -> Result<bool, WriteBackError> {
        let has: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM delivery.task_requirement \
                             WHERE requirement_id = $1 AND revision_id = $2)",
        )
        .bind(requirement_id)
        .bind(revision_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(has)
    }

    /// 需求编号：模型建议 → 归一化 → 同项目重号顺延（`-2`/`-3`…）。
    async fn unique_code(
        &self,
        project_id: Uuid,
        suggestion: Option<&str>,
        session_id: Uuid,
    ) -> Result<String, WriteBackError> {
        let base = sanitize_requirement_code(suggestion, session_id);
        for attempt in 1..=30 {
            let candidate = if attempt == 1 {
                base.clone()
            } else {
                format!("{base}-{attempt}")
            };
            let taken: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM delivery.requirement \
                                 WHERE project_id = $1 AND code = $2)",
            )
            .bind(project_id)
            .bind(&candidate)
            .fetch_one(&self.pool)
            .await?;
            if !taken {
                return Ok(candidate);
            }
        }
        // 30 个候选都被占：退化为 session 派生的确定值（几乎不可能撞）。
        Ok(format!(
            "DISC-{}",
            session_id.simple().to_string()[..8].to_ascii_uppercase()
        ))
    }

    /// 一次模型调用入账（成功与失败都记，DEC-LLM-004.7）。
    async fn record_model_call(
        &self,
        project_id: Uuid,
        status: &str,
        error_kind: Option<&str>,
        latency_ms: i64,
        provider: Option<&ProviderStamp>,
    ) {
        let Some(provider) = provider else {
            // provider 解析前就失败：没有 FK 可锚，只能结构化 WARN。
            tracing::warn!(
                project_id = %project_id,
                error_kind = error_kind.unwrap_or("none"),
                "discovery_orchestration model call failed before provider resolution"
            );
            return;
        };
        crate::call_record::record_call(
            &self.pool,
            project_id,
            provider.provider_id,
            &provider.model,
            crate::call_record::PURPOSE_DISCOVERY_ORCHESTRATION,
            status,
            error_kind,
            None,
            latency_ms,
            None,
            None,
        )
        .await;
    }
}

/// 需求编号归一化（大写 + 只留 `[A-Z0-9-]` + 折叠连字符 + 截断 32）。
/// 模型给空/全是标点时退化为 `DISC-<session 前 8 位>`——编号永远有确定值，
/// 不因模型偷懒而卡住起草。
fn sanitize_requirement_code(suggestion: Option<&str>, session_id: Uuid) -> String {
    let mut out = String::new();
    for ch in suggestion.unwrap_or_default().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else if (ch == '-' || ch == '_' || ch.is_whitespace()) && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() > 32 {
        out.truncate(32);
        while out.ends_with('-') {
            out.pop();
        }
    }
    if out.is_empty() {
        out = format!(
            "DISC-{}",
            session_id.simple().to_string()[..8].to_ascii_uppercase()
        );
    }
    out
}

/// 通过系统骨架校验的一条任务（依赖已解析成批内下标，天然无环）。
#[derive(Debug, Clone)]
struct ValidatedTask {
    kind: BatchTaskKind,
    title: String,
    description: Option<String>,
    acceptance: Vec<String>,
    /// 批内更早任务的下标（`depends_on` 的键 → 位置）。
    prerequisites: Vec<usize>,
    role_version_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
struct ValidatedBatch {
    tasks: Vec<ValidatedTask>,
    /// 超上限被截断丢弃的条数（§7.3：系统定数量，超了截断而不是整批拒绝）。
    truncated: usize,
    /// 角色建议越界被丢弃的条数（§7.3：角色只从候选选，越界即丢弃）。
    dropped_role_suggestions: usize,
}

/// 任务分解的系统骨架校验（§7.2/§7.3；纯函数，可单测）。
///
/// 系统定、模型不可协商的四件事：
/// 1. **数量**：前置 ≤ [`MAX_PREREQUISITE_TASKS`]、主任务 ≤ [`MAX_MAIN_TASKS`]，
///    超出**截断**（不是整批拒绝——数量溢出属可裁剪的噪声）；
/// 2. **必填**：title 非空；**每条**任务（含前置任务）至少一条非空验收口径，
///    否则**整批拒绝**（E4：无验收口径的任务永远完不成，会把主任务锁死）；
/// 3. **依赖**：`depends_on` 必须指向本批内**更早出现**的任务键——未知键/前向
///    引用/自引用一律整批拒绝；这是"禁止标题字符串语义"（E3）的编译期等价物，
///    同时使依赖图天然无环；
/// 4. **角色**：`role_ref` 只能命中候选 `role_key`，否则丢弃该建议（不报错、
///    不编造角色）并计数。
fn validate_task_batch(
    raw: RawTaskBatch,
    roles: &[RoleCandidate],
) -> Result<ValidatedBatch, WriteBackError> {
    let mut truncated = 0usize;
    let mut prerequisite = raw.prerequisite;
    if prerequisite.len() > MAX_PREREQUISITE_TASKS {
        truncated += prerequisite.len() - MAX_PREREQUISITE_TASKS;
        tracing::warn!(
            given = prerequisite.len(),
            limit = MAX_PREREQUISITE_TASKS,
            "task decomposition: prerequisite tasks truncated to the system limit"
        );
        prerequisite.truncate(MAX_PREREQUISITE_TASKS);
    }
    let mut main = raw.main;
    if main.len() > MAX_MAIN_TASKS {
        truncated += main.len() - MAX_MAIN_TASKS;
        tracing::warn!(
            given = main.len(),
            limit = MAX_MAIN_TASKS,
            "task decomposition: main tasks truncated to the system limit"
        );
        main.truncate(MAX_MAIN_TASKS);
    }

    let mut dropped_role_suggestions = 0usize;
    let mut tasks: Vec<ValidatedTask> = Vec::with_capacity(prerequisite.len() + main.len());
    let mut keys: Vec<String> = Vec::new();

    for (kind, group) in [
        (BatchTaskKind::Prerequisite, prerequisite),
        (BatchTaskKind::Main, main),
    ] {
        for task in group {
            let title = task.title.trim().to_string();
            if title.is_empty() {
                return Err(WriteBackError::BatchRejected(
                    "LLM returned a task without a title".to_string(),
                ));
            }
            let key = task.key.trim().to_string();
            if key.is_empty() {
                return Err(WriteBackError::BatchRejected(format!(
                    "task {title:?} has no batch key — dependency edges cannot be expressed"
                )));
            }
            if keys.iter().any(|existing| existing == &key) {
                return Err(WriteBackError::BatchRejected(format!(
                    "duplicate task key {key:?} in the batch"
                )));
            }

            // E4：无验收口径即整批拒绝。
            let acceptance: Vec<String> = task
                .acceptance_criteria
                .iter()
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty())
                .collect();
            if acceptance.is_empty() {
                return Err(WriteBackError::BatchRejected(format!(
                    "task {title:?} has no acceptance criteria — a task that cannot be verified is not generated"
                )));
            }

            // 依赖只允许指向"更早出现的键"（无未知键、无前向引用、无自引用、无环）。
            let mut prerequisites = Vec::with_capacity(task.depends_on.len());
            for reference in &task.depends_on {
                let reference = reference.trim();
                let index = keys
                    .iter()
                    .position(|existing| existing == reference)
                    .ok_or_else(|| {
                        WriteBackError::BatchRejected(format!(
                            "task {title:?} depends on unknown or later key {reference:?}"
                        ))
                    })?;
                if !prerequisites.contains(&index) {
                    prerequisites.push(index);
                }
            }

            let role_version_id = match task.role_ref.as_deref().map(str::trim) {
                Some(reference) if !reference.is_empty() => {
                    match roles.iter().find(|c| c.role_key == reference) {
                        Some(candidate) => Some(candidate.version_id),
                        None => {
                            dropped_role_suggestions += 1;
                            tracing::warn!(
                                role_ref = reference,
                                "task decomposition: role suggestion outside the candidate set dropped"
                            );
                            None
                        }
                    }
                }
                _ => None,
            };

            keys.push(key);
            tasks.push(ValidatedTask {
                kind,
                title,
                description: task
                    .description
                    .as_deref()
                    .map(str::trim)
                    .filter(|d| !d.is_empty())
                    .map(str::to_string),
                acceptance,
                prerequisites,
                role_version_id,
            });
        }
    }

    Ok(ValidatedBatch {
        tasks,
        truncated,
        dropped_role_suggestions,
    })
}

#[async_trait]
impl WriteBackHook for DiscoveryWriteBack {
    /// `create_requirement → update_draft（含 basis）→ submit_revision`，
    /// **到此止步**（停在 `pending_confirmation`，等人类 Approver 确认）。
    async fn on_review_required(
        &self,
        tenant_id: Uuid,
        session: &DiscoverySessionView,
    ) -> Result<DraftOutcome, WriteBackError> {
        if !self.mode.enabled() {
            return Ok(DraftOutcome::skipped("orchestrator_off"));
        }
        if session.status != pm_application::discovery::SESSION_STATUS_REVIEW_REQUIRED {
            return Ok(DraftOutcome::skipped("session_not_review_required"));
        }

        let findings = self.confirmed_findings(tenant_id, session.id).await?;
        if findings.is_empty() {
            // 没有已确认事实 → 没有可立的草案依据（不写"带病提案"）。
            return Ok(DraftOutcome::skipped("no_confirmed_finding"));
        }
        let basis_ids: Vec<Uuid> = findings.iter().map(|f| f.id).collect();

        // 幂等：本 session 已起草过（含人类已确认的终态）→ 不重复建需求。
        let existing = self
            .session_requirements(tenant_id, session.project_id, &basis_ids)
            .await?;
        if let Some(current) = existing.first() {
            return Ok(DraftOutcome {
                kind: DraftOutcomeKind::AlreadyDrafted,
                requirement_id: Some(current.requirement_id),
                revision_id: Some(current.latest_revision_id),
                revision_state: Some(current.latest_state.clone()),
                basis_finding_ids: basis_ids,
                reason: Some("session_already_drafted"),
            });
        }

        let capability = self.capability.project(session.project_id).await?;
        let ctx = RequirementDraftContext {
            project_id: session.project_id,
            session_id: session.id,
            session_kind: session.kind.clone(),
            findings,
            capability,
        };
        let reply = match self.model.draft_requirement(&ctx).await {
            Ok(reply) => {
                self.record_model_call(
                    session.project_id,
                    "ok",
                    None,
                    reply.latency_ms,
                    Some(&reply.provider),
                )
                .await;
                reply
            }
            Err(failure) => {
                self.record_model_call(
                    session.project_id,
                    "failed",
                    Some(crate::call_record::analysis_error_kind(&failure.error)),
                    failure.latency_ms,
                    failure.provider.as_ref(),
                )
                .await;
                return Err(WriteBackError::Model(failure.error));
            }
        };
        let draft = reply.value;

        let actor = pm_application::actor::AuditActor::principal(
            self.ai_principal(session.project_id).await?,
        );
        let code = self
            .unique_code(session.project_id, draft.code.as_deref(), session.id)
            .await?;
        let title = draft.title.trim().to_string();
        if title.is_empty() || draft.markdown.trim().is_empty() {
            return Err(WriteBackError::BatchRejected(
                "LLM returned an empty requirement title/markdown".to_string(),
            ));
        }
        let markdown = draft.markdown.trim().to_string();
        let summary = draft
            .summary
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        // ① create_requirement（AI 主体；无真人闸 —— §3.3 ✅）
        let entry = pm_application::requirement::create_requirement(
            &self.pool,
            tenant_id,
            session.project_id,
            pm_application::requirement::CreateRequirementRequest {
                code,
                title: title.clone(),
                description: summary.clone(),
                requirement_type: None,
                priority: None,
                markdown: Some(markdown.clone()),
                basis_finding_ids: Some(basis_ids.clone()),
            },
            &actor,
        )
        .await?;
        let revision_id = entry.latest_revision_id.ok_or_else(|| {
            WriteBackError::Discovery("created requirement has no draft revision".to_string())
        })?;

        // ② update_draft（含 basis）：与 ① 内容等价的重写，保留是为对齐 §8.1 时序
        //    并让「basis 随草稿更新」这条路本身有覆盖（不是空转：它是唯一能改
        //    draft 正文的合法面，人类/AI 后续修订都走它）。
        pm_application::requirement::update_draft(
            &self.pool,
            tenant_id,
            entry.id,
            revision_id,
            pm_application::requirement::DraftRevisionRequest {
                markdown,
                summary,
                basis_finding_ids: Some(basis_ids.clone()),
            },
            &actor,
        )
        .await?;

        // ③ submit_revision：draft → pending_confirmation（等价"提交待确认项"）。
        //    AI 到此为止：confirm/reject/lifecycle 均不在白名单，且服务层真人闸兜底。
        let view = pm_application::requirement::submit_revision(
            &self.pool,
            tenant_id,
            entry.id,
            revision_id,
            &actor,
            &self.knowledge,
        )
        .await?;

        tracing::info!(
            project_id = %session.project_id,
            session_id = %session.id,
            requirement_id = %entry.id,
            revision_id = %view.id,
            basis = basis_ids.len(),
            "discovery write-back: requirement draft submitted for human confirmation"
        );

        Ok(DraftOutcome {
            kind: DraftOutcomeKind::Submitted,
            requirement_id: Some(entry.id),
            revision_id: Some(view.id),
            revision_state: Some(view.state),
            basis_finding_ids: basis_ids,
            reason: None,
        })
    }

    /// 任务分解（§7）：触发 = 本 session 起草的需求已有 approved 修订，且该修订
    /// 尚无 `task_requirement` 行；前置门 = 未定稿直接拒绝（B5）。
    async fn decompose_if_approved(
        &self,
        tenant_id: Uuid,
        session: &DiscoverySessionView,
    ) -> Result<DecomposeOutcome, WriteBackError> {
        if !self.mode.enabled() {
            return Ok(DecomposeOutcome::NothingToDo {
                reason: "orchestrator_off",
            });
        }

        let findings = self.confirmed_findings(tenant_id, session.id).await?;
        if findings.is_empty() {
            return Ok(DecomposeOutcome::NothingToDo {
                reason: "no_confirmed_finding",
            });
        }
        let basis_ids: Vec<Uuid> = findings.iter().map(|f| f.id).collect();
        let candidates = self
            .session_requirements(tenant_id, session.project_id, &basis_ids)
            .await?;
        if candidates.is_empty() {
            return Ok(DecomposeOutcome::NothingToDo {
                reason: "no_requirement_drafted",
            });
        }

        // B5 前置门：本 session 的需求还没定稿 → 拒绝生成，文案指向需求页。
        // fail-closed：任何一条候选未 approved 即整轮拒绝（不挑"能拆的那条"）。
        for candidate in &candidates {
            if !candidate.approved() {
                tracing::warn!(
                    requirement_id = %candidate.requirement_id,
                    latest_revision = candidate.latest_revision_version,
                    latest_state = %candidate.latest_state,
                    "task decomposition blocked: requirement has no approved revision (B5)"
                );
                return Err(WriteBackError::RequirementNotApproved {
                    requirement_id: candidate.requirement_id,
                });
            }
        }

        // 扫描谓词的排他面：approved 修订已挂任务 → 本轮无事可做（幂等的来源）。
        let mut target = None;
        for candidate in candidates {
            let revision_id = candidate.approved_revision_id.ok_or_else(|| {
                WriteBackError::Discovery("approved requirement without approved revision".into())
            })?;
            if !self
                .revision_has_tasks(candidate.requirement_id, revision_id)
                .await?
            {
                target = Some((candidate, revision_id));
                break;
            }
        }
        let Some((target, revision_id)) = target else {
            return Ok(DecomposeOutcome::NothingToDo {
                reason: "tasks_already_generated",
            });
        };

        // 三源齐备：approved 正文 + 需求级验收口径（0056）+ 能力面（DK-C）+ 角色候选。
        let acceptance_criteria = self
            .acceptance_statements(session.project_id, target.requirement_id)
            .await?;
        let capability = self.capability.project(session.project_id).await?;
        let roles = self.role_candidates(tenant_id, session.project_id).await?;
        let ctx = TaskDecomposeContext {
            project_id: session.project_id,
            requirement_id: target.requirement_id,
            requirement_code: target.requirement_code.clone(),
            requirement_title: target.requirement_title.clone(),
            markdown: target.approved_markdown.clone(),
            acceptance_criteria,
            capability,
            roles: roles.clone(),
        };

        let reply = match self.model.decompose_tasks(&ctx).await {
            Ok(reply) => {
                self.record_model_call(
                    session.project_id,
                    "ok",
                    None,
                    reply.latency_ms,
                    Some(&reply.provider),
                )
                .await;
                reply
            }
            Err(failure) => {
                self.record_model_call(
                    session.project_id,
                    "failed",
                    Some(crate::call_record::analysis_error_kind(&failure.error)),
                    failure.latency_ms,
                    failure.provider.as_ref(),
                )
                .await;
                return Err(WriteBackError::Model(failure.error));
            }
        };

        // 系统骨架校验（§7.3）：上限截断 / 必填验收口径 / 依赖键 / 角色越界。
        let batch = validate_task_batch(reply.value, &roles)?;
        if batch.tasks.is_empty() {
            return Ok(DecomposeOutcome::NothingToDo {
                reason: "empty_batch",
            });
        }

        let actor = pm_application::actor::AuditActor::principal(
            self.ai_principal(session.project_id).await?,
        );
        let mut generated: Vec<GeneratedTask> = Vec::with_capacity(batch.tasks.len());
        for (index, task) in batch.tasks.iter().enumerate() {
            // ① create_task（status 恒 draft，F16/E2）+ 验收口径同事务
            let row = pm_application::pm_write::create_task(
                &self.pool,
                pm_application::pm_write::CreateTaskRequest {
                    project_id: Some(session.project_id),
                    milestone_id: None,
                    title: Some(task.title.clone()),
                    description: task.description.clone(),
                    priority: Some("normal".to_string()),
                    assignee_id: None,
                    ai_role_version_id: task.role_version_id,
                    acceptance_criteria: Some(
                        task.acceptance
                            .iter()
                            .map(|text| pm_application::pm_write::AcceptanceCriterionInput {
                                ordinal: None,
                                text: text.clone(),
                                verification_kind: Some("manual".to_string()),
                            })
                            .collect(),
                    ),
                },
                &actor,
            )
            .await?;

            // ② task_requirement 钉定（E1：revision 必为 approved，服务层执法）
            pm_application::pm_write::link_task_requirement(
                &self.pool,
                row.id,
                target.requirement_id,
                revision_id,
                &actor,
            )
            .await?;

            // ③ 前置依赖落边表（E3：不是标题字符串语义；D6 允许 draft 下游建边）
            let mut depends_on = Vec::with_capacity(task.prerequisites.len());
            for upstream_index in &task.prerequisites {
                let upstream = generated.get(*upstream_index).ok_or_else(|| {
                    WriteBackError::BatchRejected(format!(
                        "task #{index} references a prerequisite that was not created"
                    ))
                })?;
                pm_application::task_dependency::add_dependency(
                    &self.pool,
                    row.id,
                    upstream.task_id,
                    &actor,
                )
                .await
                .map_err(|error| WriteBackError::Dependency(format!("{error:?}")))?;
                depends_on.push(upstream.task_id);
            }

            generated.push(GeneratedTask {
                task_id: row.id,
                title: task.title.clone(),
                kind: task.kind,
                depends_on,
                acceptance_criteria: task.acceptance.len(),
                ai_role_version_id: task.role_version_id,
            });
        }

        let prerequisite_count = generated
            .iter()
            .filter(|t| t.kind == BatchTaskKind::Prerequisite)
            .count();
        let main_count = generated.len() - prerequisite_count;
        tracing::info!(
            project_id = %session.project_id,
            session_id = %session.id,
            requirement_id = %target.requirement_id,
            revision_id = %revision_id,
            revision_version = target.approved_revision_version.unwrap_or_default(),
            tasks = generated.len(),
            prerequisites = prerequisite_count,
            truncated = batch.truncated,
            dropped_roles = batch.dropped_role_suggestions,
            "discovery write-back: task batch drafted (status=draft, awaiting human batch approval)"
        );

        Ok(DecomposeOutcome::Generated(TaskBatchOutcome {
            requirement_id: target.requirement_id,
            revision_id,
            tasks: generated,
            prerequisite_count,
            main_count,
            truncated: batch.truncated,
            dropped_role_suggestions: batch.dropped_role_suggestions,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(key: &str, deps: &[&str], acceptance: &[&str]) -> RawTask {
        RawTask {
            key: key.to_string(),
            title: format!("任务 {key}"),
            description: None,
            acceptance_criteria: acceptance.iter().map(|s| s.to_string()).collect(),
            depends_on: deps.iter().map(|s| s.to_string()).collect(),
            role_ref: None,
        }
    }

    fn role(key: &str) -> RoleCandidate {
        RoleCandidate {
            version_id: Uuid::nil(),
            role_id: Uuid::nil(),
            role_key: key.to_string(),
            version: "1.0".to_string(),
        }
    }

    /// 依赖只允许指向"更早出现的键"：前向引用与自引用都在这里被挡下（E3 的
    /// 结构化保证——不靠标题字符串，也就不存在"改标题即断边"）。
    #[test]
    fn dependency_must_point_at_an_earlier_key() {
        let forward = RawTaskBatch {
            prerequisite: vec![],
            main: vec![raw("a", &["b"], &["验收"]), raw("b", &[], &["验收"])],
        };
        let error = validate_task_batch(forward, &[]).unwrap_err();
        assert!(
            matches!(error, WriteBackError::BatchRejected(_)),
            "{error:?}"
        );

        let self_ref = RawTaskBatch {
            prerequisite: vec![],
            main: vec![raw("a", &["a"], &["验收"])],
        };
        let error = validate_task_batch(self_ref, &[]).unwrap_err();
        assert!(
            matches!(error, WriteBackError::BatchRejected(_)),
            "{error:?}"
        );

        let ok = RawTaskBatch {
            prerequisite: vec![raw("p", &[], &["验收"])],
            main: vec![raw("m", &["p"], &["验收"])],
        };
        let batch = validate_task_batch(ok, &[]).unwrap();
        assert_eq!(batch.tasks[1].prerequisites, vec![0]);
    }

    /// 重复键会让依赖边指向错误的任务——整批拒绝。
    #[test]
    fn duplicate_keys_reject_the_batch() {
        let batch = RawTaskBatch {
            prerequisite: vec![raw("dup", &[], &["验收"])],
            main: vec![raw("dup", &[], &["验收"])],
        };
        let error = validate_task_batch(batch, &[]).unwrap_err();
        assert!(
            matches!(error, WriteBackError::BatchRejected(_)),
            "{error:?}"
        );
    }

    /// 空键 = 依赖无法表达，拒绝（不接受"标题即键"的退化写法）。
    #[test]
    fn missing_key_rejects_the_batch() {
        let batch = RawTaskBatch {
            prerequisite: vec![],
            main: vec![raw("", &[], &["验收"])],
        };
        let error = validate_task_batch(batch, &[]).unwrap_err();
        assert!(
            matches!(error, WriteBackError::BatchRejected(_)),
            "{error:?}"
        );
    }

    /// 角色建议命中候选才采纳；越界丢弃并计数（不报错、不编造）。
    #[test]
    fn role_suggestion_is_bounded_by_candidates() {
        let batch = RawTaskBatch {
            prerequisite: vec![],
            main: vec![
                raw("m1", &[], &["验收"]),
                RawTask {
                    role_ref: Some("developer".to_string()),
                    ..raw("m2", &[], &["验收"])
                },
            ],
        };
        let validated = validate_task_batch(batch, &[role("developer")]).unwrap();
        assert!(validated.tasks[0].role_version_id.is_none());
        assert_eq!(validated.tasks[1].role_version_id, Some(Uuid::nil()));
        assert_eq!(validated.dropped_role_suggestions, 0);

        let batch = RawTaskBatch {
            prerequisite: vec![],
            main: vec![RawTask {
                role_ref: Some("cto".to_string()),
                ..raw("m", &[], &["验收"])
            }],
        };
        let validated = validate_task_batch(batch, &[role("developer")]).unwrap();
        assert!(validated.tasks[0].role_version_id.is_none());
        assert_eq!(validated.dropped_role_suggestions, 1);
    }

    /// 验收口径只有空白 = 没有验收口径（E4）；前置任务同样必填（§7.2）。
    #[test]
    fn blank_acceptance_rejects_even_prerequisites() {
        let batch = RawTaskBatch {
            prerequisite: vec![raw("p", &[], &["   "])],
            main: vec![],
        };
        let error = validate_task_batch(batch, &[]).unwrap_err();
        assert!(
            matches!(error, WriteBackError::BatchRejected(_)),
            "{error:?}"
        );
    }

    /// 需求编号归一化：大写、只留 [A-Z0-9-]、折叠、空则退化为 session 派生值。
    #[test]
    fn requirement_code_is_normalized() {
        let session = Uuid::nil();
        assert_eq!(
            sanitize_requirement_code(Some("fr phone-login"), session),
            "FR-PHONE-LOGIN"
        );
        assert_eq!(
            sanitize_requirement_code(Some("  --  "), session),
            "DISC-00000000"
        );
        assert_eq!(sanitize_requirement_code(None, session), "DISC-00000000");
        assert_eq!(
            sanitize_requirement_code(Some("A".repeat(50).as_str()), session).len(),
            32
        );
    }
}
