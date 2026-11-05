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

/** 关联项类型 */
export type RelatedKind = "requirement" | "objective" | "task";

/** 关联项 */
export interface RelatedItem {
  kind: RelatedKind;
  code: string;
  title: string;
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
  updated_at: string;
  acceptance_criteria: AcceptanceCriterion[];
  /** 修订记录，version 倒序 */
  revisions: RevisionRecord[];
  sources: SourceItem[];
  discussions: DiscussionEntry[];
  analysis: AnalysisFinding[];
  ai_suggestions: AiSuggestion[];
  related: RelatedItem[];
}

/** 需求基线快照（R3：基线 = 各正式 Requirement 的集合，无独立 Baseline 实体） */
export interface BaselineSnapshot {
  version: number;
  established_at: string;
  established_by: string;
  /** 含入基线的需求编码（仅 confirmed 需求） */
  requirement_codes: string[];
}

/** AI 协作建议动作（§10 六个建议动作） */
export type AiActionKind =
  | "completeness"
  | "missing"
  | "duplication"
  | "conflict"
  | "acceptance"
  | "summary";
