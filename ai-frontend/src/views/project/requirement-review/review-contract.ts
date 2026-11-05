/**
 * 需求评审域契约（S02-UI-03，mock 先行）。
 *
 * 评审队列的对象是**提案（Proposal）**，不是需求本身：
 *   - `ReviewStatus` 是提案的评审生命周期（待评审 / 已要求修改 / 已确认 / 已拒绝），
 *     与需求的 Revision 生命周期（draft / pending_confirmation / approved /
 *     rejected / superseded，见 requirement-entry.ts）**相互独立**，不得混用同一字段。
 *   - 提案 Confirm（人类正式决策）的落地语义：新需求提案定稿为正式需求；
 *     修订提案在目标需求上生成新的正式修订（原定稿转 superseded）。
 *   - AI 只能产生提案与分析，`source: "ai"` 的提案必须经人类决策才能变 confirmed；
 *     `decision` 记录只能由人类产生。
 */

import type {
  AcceptanceCriterion,
  AnalysisFinding,
  RequirementPriority,
  RequirementType,
  SourceItem
} from "../requirements/requirement-entry";

/** 提案来源：AI 生成 / 人工提交 / 批量导入 */
export type ProposalSource = "ai" | "human" | "import";

/**
 * 提案评审状态（独立于需求 Revision 生命周期）。
 * pending_review → changes_requested（退回修改，可重新提交）→ confirmed | rejected；
 * rejected 为终态（保留留档），confirmed 为终态（决策已落地）。
 */
export type ReviewStatus =
  | "pending_review"
  | "changes_requested"
  | "confirmed"
  | "rejected";

/** 人类决策类型（与 ReviewStatus 的三个终态/中间态对应） */
export type DecisionKind = "confirmed" | "changes_requested" | "rejected";

/** 人类决策记录（AI 不得产生） */
export interface DecisionRecord {
  kind: DecisionKind;
  reviewer: string;
  decided_at: string;
  /** 修改要求 / 拒绝理由 / 确认备注 */
  comment: string;
}

/** 并排比较差异块类型（S02-UI-03 §4D） */
export type ComparisonBlockKind =
  | "added"
  | "modified"
  | "removed"
  | "duplication";

/** 与既有需求的并排比较块 */
export interface ComparisonBlock {
  id: string;
  kind: ComparisonBlockKind;
  /** 提案侧内容（removed 块为空） */
  proposalText?: string;
  /** 既有需求侧内容（added 块为空） */
  existingText?: string;
  /** 说明（如重叠范围、引用规范） */
  note?: string;
}

/** 评审提案读模型 */
export interface ReviewProposal {
  id: string;
  /** 提案编号（PRP-001） */
  code: string;
  /** 提案标题（新需求提案 = 需求标题；修订提案 = 修订说明标题） */
  title: string;
  description: string;
  type: RequirementType;
  priority: RequirementPriority;
  acceptance_criteria: AcceptanceCriterion[];
  source: ProposalSource;
  /** 来源描述（「AI 分析助手 · 时间」/「林悦 · 时间」） */
  source_detail: string;
  submitted_at: string;
  status: ReviewStatus;
  /** 冲突指示（AI 分析检出未裁决冲突） */
  has_conflict: boolean;
  /** 修订提案的目标需求；新建需求提案为 null */
  requirement_code: string | null;
  requirement_id: string | null;
  /** 来源证据（B 节，复用需求域 SourceItem） */
  sources: SourceItem[];
  /** AI 分析（C 节，复用需求域 AnalysisFinding） */
  analysis: AnalysisFinding[];
  /** 与既有需求的并排比较（D 节）；无相似需求时 existing_code 为 null 且 blocks 为空 */
  comparison: {
    existing_code: string | null;
    blocks: ComparisonBlock[];
  };
  /** 人类决策记录；pending_review 无此字段 */
  decision?: DecisionRecord;
}

/** 评审过滤器键（§3 五档） */
export type ReviewFilterKey =
  | "all"
  | "pending_review"
  | "changes_requested"
  | "confirmed"
  | "rejected";
