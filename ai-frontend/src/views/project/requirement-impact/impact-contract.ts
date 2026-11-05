/**
 * 需求变更影响分析域契约（S02-UI-07，mock 先行）。
 *
 * 分析对象 = 一条**待确认的需求修订**（proposed revision，如 REQ-001 v2）
 * 及其下游影响。红线（§1/§4/§6/§7）：
 *   - 依赖不得虚构：影响对象必须来自需求域的真实关系（related 任务、
 *     sources 引用的知识文档、基线含入记录、验收标准快照），fixtures 逐条可溯；
 *   - AI 产出只是分析，不是确认事实（页面上必须显式标注）；
 *   - 人类决策强制：接受/拒绝变更必须经显式确认弹窗。
 */

import type { RequirementPriority } from "../requirements/requirement-entry";

/** 影响分析状态：analyzed = 已生成分析；pending = 等待（重新）分析 */
export type ImpactAnalysisStatus = "analyzed" | "pending";

/** 影响级别（§4 四级，全链路可辨） */
export type ImpactLevel = "direct" | "indirect" | "potential" | "uncertain";

/** 受影响对象类型（§4 七类；fixtures 只建真实存在的对象） */
export type ImpactTargetKind =
  | "acceptance_criteria"
  | "task"
  | "knowledge_doc"
  | "baseline"
  | "milestone";

/** 受影响对象（依赖图节点） */
export interface ImpactTarget {
  id: string;
  kind: ImpactTargetKind;
  /** 展示编码（AC-001 / TASK-210 / KB-AUTH-012 / BL-002 / M-01） */
  code: string;
  title: string;
  level: ImpactLevel;
  /** 为什么受影响（必须可溯到真实关系） */
  reason: string;
}

/** 影响发现类别（§5 A-D） */
export type ImpactFindingCategory =
  | "requirement"
  | "implementation"
  | "documentation"
  | "baseline";

/** 影响发现 */
export interface ImpactFinding {
  id: string;
  category: ImpactFindingCategory;
  level: ImpactLevel;
  body: string;
  /** 关联对象编码 */
  related?: string;
}

/** 变更来源 */
export type ChangeSource = "ai_assisted" | "human_edit" | "review_meeting";

/** 变更摘要（§3） */
export interface ChangeSummaryInfo {
  original_summary: string;
  proposed_summary: string;
  reason: string;
  source: ChangeSource;
  proposed_by: string;
  submitted_at: string;
}

/** AI 分析（§6——呈现为分析而非事实） */
export interface ImpactAiAnalysis {
  summary: string;
  risk: "low" | "medium" | "high";
  evidence: string[];
  confidence: "high" | "medium" | "low";
  recommendations: string[];
}

/** 变更影响分析主对象 */
export interface ChangeImpactAnalysis {
  id: string;
  requirement_id: string;
  requirement_code: string;
  title: string;
  type: "functional" | "non_functional";
  priority: RequirementPriority;
  /** 该需求在当前基线中的修订版本；未含入当前基线为 null */
  baseline_id: string | null;
  baseline_version: number | null;
  /** 拟议修订版本 */
  proposed_revision: number;
  status: ImpactAnalysisStatus;
  change: ChangeSummaryInfo;
  targets: ImpactTarget[];
  findings: ImpactFinding[];
  ai: ImpactAiAnalysis;
}

/** 人类决策类型（§7） */
export type ImpactDecisionKind = "accepted" | "more_analysis" | "rejected";
