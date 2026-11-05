import type { Component } from "vue";

/**
 * AI Interaction Component Library — 数据契约（V1.0）
 *
 * 统一 Interactive Response Card 的公共模型：
 *   - 响应类型（8 类）
 *   - 交互生命周期（11 态，仅表现层状态，不是业务实体状态机）
 *   - 身份 / 证据 / 提交结果
 *   - 8 类响应各自的 payload 形状
 *
 * 约定：payload 只描述「呈现内容」，不含业务授权与审批语义；
 * 正式业务变更由业务接口负责，组件提交只记录「用户选择」。
 */

/** 8 类响应 */
export type AiResponseType =
  | "informational"
  | "recommendation"
  | "proposal"
  | "question"
  | "decision"
  | "artifact"
  | "action-request"
  | "execution-result";

/**
 * 交互生命周期（表现态）：
 * created → presented → in-progress → submitted → validating → processing
 *        → accepted / rejected / deferred / resolved / failed
 */
export type InteractionStatus =
  | "created"
  | "presented"
  | "in-progress"
  | "submitted"
  | "validating"
  | "processing"
  | "accepted"
  | "rejected"
  | "deferred"
  | "resolved"
  | "failed";

/** 提交阶段（控制按钮 loading / 结果条） */
export type SubmissionPhase = "idle" | "submitting" | "success" | "error";

/** AI-PM 身份 */
export interface AiIdentity {
  /** 展示名，例如「AI-PM」 */
  name: string;
  /** 角色副标，例如「研发负责人」 */
  role?: string;
  /** 头像字符（可选，缺省用 sparkles 图标） */
  avatarLabel?: string;
}

/** 证据来源类别 */
export type EvidenceSourceKind =
  | "doc"
  | "requirement"
  | "code"
  | "issue"
  | "decision"
  | "knowledge"
  | "execution";

export interface EvidenceSource {
  id: string;
  label: string;
  kind: EvidenceSourceKind;
  /** 版本 / 路径 / 时间等副信息 */
  meta?: string;
}

/** 关联业务对象 */
export interface EvidenceRelatedObject {
  id: string;
  label: string;
  /** 业务对象类型，例如「需求」「任务」「决策」「PR」 */
  kind: string;
}

/** 证据区 */
export interface AiEvidence {
  /** 分析摘要（一两句话） */
  summary?: string;
  sources?: EvidenceSource[];
  related?: EvidenceRelatedObject[];
  /** 默认展开证据区 */
  defaultOpen?: boolean;
}

/** 提交结果（成功时） */
export interface SubmitResult {
  /** 是否已记录（不代表业务已生效） */
  recorded: boolean;
  /** 记录号 / 引用，例如「INT-2041」 */
  reference?: string;
  /** 结果文案的 i18n key（由卡片映射到 ai.card.submission.*） */
  messageKey?: string;
}

/** 统一的交互信封（会话消息内联卡片时使用） */
export interface AiInteractionEnvelope<P = unknown> {
  id: string;
  type: AiResponseType;
  status: InteractionStatus;
  identity: AiIdentity;
  /** ISO 时间戳 */
  createdAt: string;
  title: string;
  description?: string;
  conclusion?: string;
  evidence?: AiEvidence;
  payload: P;
}

/* ---------------------------------------------------------------------------
 * 8 类响应 payload
 * ------------------------------------------------------------------------- */

/** 02 Informational：纯信息与分析 */
export interface InformationalPayload {
  /** Markdown 正文 */
  contentMarkdown: string;
  /** 来源引用（可缺省，回落到 evidence.sources） */
  sources?: EvidenceSource[];
}

/** 03/04/05 Recommendation */
export interface RecommendationBenefit {
  label: string;
  detail?: string;
}

export interface RecommendationRisk {
  label: string;
  /** low / medium / high */
  severity: "low" | "medium" | "high";
  detail?: string;
}

export interface RecommendationPayload {
  /** 推荐摘要 */
  summary: string;
  /** 详细解释（Markdown） */
  explanationMarkdown: string;
  benefits: RecommendationBenefit[];
  risks: RecommendationRisk[];
  /** 被推荐的动作标签，例如「拆分任务」 */
  recommendedAction?: string;
}

/** 06/07 Proposal */
export interface ProposalChange {
  label: string;
  /** 变更前 */
  before: string;
  /** 变更后 */
  after: string;
}

export interface ProposalPayload {
  /** 提案类型，例如「架构变更」「范围变更」 */
  proposalType: string;
  /** 拟变更项 */
  changes: ProposalChange[];
  /** 影响摘要 */
  impact: string[];
  /** 关联业务对象 */
  related: EvidenceRelatedObject[];
  /** 版本信息，例如「v1.2 → v1.3」 */
  version: string;
  /** 是否允许提交正式审批（accept ≠ approve） */
  canSubmitForApproval?: boolean;
}

/** 08/09/10 Question */
export type QuestionKind =
  | "single"
  | "multi"
  | "text"
  | "textarea"
  | "number"
  | "date";

export interface QuestionOption {
  value: string;
  label: string;
  description?: string;
}

export interface QuestionItem {
  id: string;
  kind: QuestionKind;
  label: string;
  help?: string;
  required?: boolean;
  options?: QuestionOption[];
  placeholder?: string;
  min?: number;
  max?: number;
  defaultValue?: string | string[];
}

export interface QuestionPayload {
  /** 问题组标题 */
  groupTitle: string;
  questions: QuestionItem[];
}

/** 11/12 Decision */
export interface DecisionOption {
  id: string;
  label: string;
  summary?: string;
  pros?: string[];
  cons?: string[];
  /** AI 推荐项 */
  recommended?: boolean;
}

export interface DecisionPayload {
  /** 决策背景 */
  background: string;
  /** 影响分析 */
  impact: string[];
  options: DecisionOption[];
  /** 是否允许补充意见 */
  allowComment?: boolean;
  /** 决策记录号占位（提交成功后写入） */
  decisionRecord?: string;
}

/** 13 Artifact */
export type ArtifactKind =
  | "requirement-doc"
  | "technical-design"
  | "project-plan"
  | "test-plan"
  | "knowledge-doc"
  | "code-change-summary";

export interface ArtifactVersion {
  label: string;
  /** ISO */
  createdAt?: string;
  author?: string;
}

export interface ArtifactPayload {
  artifactType: ArtifactKind;
  /** Markdown 正文（预览） */
  contentMarkdown: string;
  version: ArtifactVersion;
  /** 与上一版本的差异（可选） */
  diff?: DiffPayload;
  /** 来源引用 */
  sources?: EvidenceSource[];
  /** 可提交到的既有业务流程名，例如「需求评审」 */
  workflowLabel?: string;
}

/** Diff（14 Artifact 与 15 Execution 共用） */
export interface DiffLine {
  kind: "context" | "add" | "remove";
  text: string;
}

export interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

export interface DiffPayload {
  /** 文件或文档路径 */
  path: string;
  hunks: DiffHunk[];
}

/** 14 Action Request */
export interface ActionRequestPayload {
  /** 动作描述 */
  action: string;
  /** 目标对象 */
  target: string;
  /** 预期影响 */
  impact: string[];
  /** 所需权限 */
  permission: string;
  /** 风险等级 */
  riskLevel: "low" | "medium" | "high";
  /** 确认声明（用户需明确同意的原句） */
  statement: string;
  /** 属高后果操作 → 弹确认对话框 */
  consequential?: boolean;
}

/** 15 Execution Result */
export type ExecutionState = "succeeded" | "partial" | "failed" | "cancelled";

export interface ChangedFile {
  path: string;
  status: "modified" | "added" | "deleted";
  additions: number;
  deletions: number;
}

export interface ExecutionLogEntry {
  time: string;
  level: "info" | "warn" | "error";
  text: string;
}

export interface ExecutionResultPayload {
  agentName: string;
  taskRef: string;
  executionState: ExecutionState;
  summary: string;
  changedFiles: ChangedFile[];
  logs: ExecutionLogEntry[];
  warnings?: string[];
  failure?: {
    code: string;
    message: string;
    /** 是否允许重试 */
    retryable: boolean;
  };
  diff?: DiffPayload;
}

/* ---------------------------------------------------------------------------
 * 卡片通用 props（8 类卡片共享）
 * ------------------------------------------------------------------------- */

export type CardDensity = "default" | "conversation" | "compact";

/** 页脚动作（顺序即渲染顺序） */
export type FooterActionVariant = "primary" | "secondary" | "ghost" | "danger";

export interface FooterAction {
  key: string;
  label: string;
  variant?: FooterActionVariant;
  disabled?: boolean;
  loading?: boolean;
  /** lucide 图标组件（可选） */
  icon?: Component;
}

/** 会话消息（§7 Conversation Integration） */
export type ConversationRole = "user" | "ai";

export interface ConversationMessage {
  id: string;
  role: ConversationRole;
  /** ISO */
  createdAt: string;
  /** 轻量普通消息（Markdown，无卡片） */
  text?: string;
  /** 内联交互卡（AI 消息） */
  interaction?: AiInteractionEnvelope;
}
