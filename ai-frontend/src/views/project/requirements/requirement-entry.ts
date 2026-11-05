/**
 * Requirements Workspace 展示契约（S02-UI-01，mock 先行）。
 *
 * 页面读模型：以 `src/api/types.ts` 的 `Requirement` / `RequirementRevision`
 * （delivery.requirement / delivery.requirement_revision）为锚，补齐 UI 呈现
 * 所需的来源/验收标准/上下文字段。后端落地时以本文件为读模型参照。
 *
 * 红线（S02-UI-01 §1 Important + S02-I1 裁决）：
 *   - 类型仅 functional / non_functional（Constraint 不是确认需求类型）；
 *   - `confirmed` 对应后端 revision state = approved；conflict 为 UI 派生
 *     状态（存在未解决冲突检测项），需人工裁决后回到修订流程；
 *   - AI 产出只能进入 `ai_suggestions` / `proposals`，不得自动改写状态。
 */

/** 需求类型：仅功能 / 非功能两类 */
export type RequirementType = "functional" | "non_functional";

/** 优先级 */
export type RequirementPriority = "high" | "medium" | "low";

/**
 * UI 生命周期状态。
 * draft / pending_confirmation / confirmed 与 S02-I1 修订生命周期对齐；
 * conflict 为 UI 派生状态（存在未解决的冲突检测项）。
 */
export type RequirementUiState =
  | "draft"
  | "pending_confirmation"
  | "confirmed"
  | "conflict";

/** 修订状态（对齐 S02-I1 生命周期：approved = 已确认为正式版本） */
export type RevisionState =
  | "draft"
  | "pending_confirmation"
  | "approved"
  | "rejected"
  | "superseded";

/** 验收标准条目（清晰、可测的陈述） */
export interface AcceptanceCriterion {
  id: string;
  statement: string;
}

/** 修订来源（S02-UI-05 §3 Change origin：人工编辑 / AI 辅助 / 导入） */
export type RevisionOrigin = "human_edit" | "ai_assisted" | "import";

/** 修订关联决策（独立于修订状态的人类决策记录；对齐 S02-I1 Confirmation 方向） */
export interface RevisionDecision {
  kind: "confirmed" | "rejected";
  actor: string;
  acted_at: string;
}

/** 修订审计信息（S02-UI-05 §6） */
export interface RevisionAudit {
  /** 谁发起该修订 */
  initiated_by: string;
  /** 谁评审过（可空） */
  reviewed_by?: string;
  /** 为什么改 */
  reason: string;
}

/** 修订记录（RequirementRevision 读模型扩展） */
export interface RevisionRecord {
  id: string;
  version: number;
  state: RevisionState;
  /** 变更描述（一句话） */
  summary: string;
  author: string;
  created_at: string;
  /** 该版正文（Knowledge Markdown 权威正文的快照展示） */
  markdown: string;
  /** 变更来源；缺省视为未记录（历史数据渐进补齐） */
  origin?: RevisionOrigin;
  /** 关联决策（confirmed = 定稿凭据；rejected = 否决留档） */
  decision?: RevisionDecision;
  /** 审计信息（发起/评审/理由） */
  audit?: RevisionAudit;
  /** 该版优先级快照（版本比较用） */
  priority?: RequirementPriority;
  /** 该版类型快照（版本比较用） */
  type?: RequirementType;
  /** 该版验收标准快照（版本比较用） */
  criteria?: AcceptanceCriterion[];
}

/** 来源与追溯条目类型 */
export type SourceKind =
  | "user_request"
  | "discovery"
  | "knowledge"
  | "discussion"
  | "objective";

/** 来源与追溯条目（原始材料，与结构化需求视觉区分） */
export interface SourceItem {
  id: string;
  kind: SourceKind;
  title: string;
  /** 原文摘录 / 证据说明 */
  detail: string;
  /** 外部引用（文档编号 / commit sha / 会议纪要编号） */
  ref?: string;
}

/** 讨论与评审评论（人工通道） */
export interface DiscussionEntry {
  id: string;
  author: string;
  body: string;
  created_at: string;
}

/** 分析发现类型（完备性 / 歧义 / 重复 / 冲突） */
export type AnalysisKind =
  | "completeness"
  | "ambiguity"
  | "duplication"
  | "conflict";

/** 分析发现（Analysis 面板） */
export interface AnalysisFinding {
  id: string;
  kind: AnalysisKind;
  severity: "high" | "medium" | "low";
  body: string;
  /** 关联对象（如另一条需求编码） */
  related?: string;
}

/** AI 建议（永远只是提案，待人工评审） */
export interface AiSuggestion {
  id: string;
  body: string;
  created_at: string;
}

/** 需求规格（S02-UI-02 §3B：业务理由 / 预期结果 / 范围 / 明确排除） */
export interface RequirementSpecification {
  business_rationale: string;
  expected_outcome: string;
  scope: string;
  explicit_exclusions: string;
}

/** 关联项类型 */
export type RelatedKind = "requirement" | "objective" | "task";

/** 关联项 */
export interface RelatedItem {
  kind: RelatedKind;
  code: string;
  title: string;
  /** 关联需求可导航时给出的目标需求 id（S02-UI-02 §5 关联需求导航） */
  target_id?: string;
}

/** 需求工作区行/详情读模型 */
export interface RequirementEntry {
  id: string;
  /** 展示编码（REQ-001） */
  code: string;
  title: string;
  description: string;
  type: RequirementType;
  priority: RequirementPriority;
  status: RequirementUiState;
  /** 来源指示（列表行短标签） */
  source_label: string;
  created_at: string;
  updated_at: string;
  /** 需求规格（S02-UI-02 §3B） */
  specification: RequirementSpecification;
  acceptance_criteria: AcceptanceCriterion[];
  /** 修订记录，version 倒序 */
  revisions: RevisionRecord[];
  sources: SourceItem[];
  discussions: DiscussionEntry[];
  analysis: AnalysisFinding[];
  ai_suggestions: AiSuggestion[];
  related: RelatedItem[];
}

/** AI 协作建议动作（§10 六个建议动作） */
export type AiActionKind =
  | "completeness"
  | "missing"
  | "duplication"
  | "conflict"
  | "acceptance"
  | "summary";
