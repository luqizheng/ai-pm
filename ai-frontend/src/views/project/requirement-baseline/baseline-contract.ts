/**
 * 需求基线管理域契约（S02-UI-06，mock 先行）。
 *
 * R3 裁决：第一阶段不建独立 Project Baseline 实体——基线由各**正式
 * Requirement Revision** 的集合表达，`BaselineRecord` 仅为 UI 读模型。
 * 红线（S01-01 / S02-UI-06 §6）：
 *   - 历史基线保留原引用（版本不可覆盖）；
 *   - 基线只能含入 confirmed 需求的正式修订；draft / pending_confirmation /
 *     conflict / 验收标准不完整的需求一律不得静默含入；
 *   - 建立基线必须经人类显式确认，不得自动建立。
 */

import type {
  RequirementPriority,
  RequirementType
} from "../requirements/requirement-entry";

/** 基线状态：active = 当前生效；superseded = 已被新基线替代（历史保留） */
export type BaselineStatus = "active" | "superseded";

/** 基线含入条目：引用需求在确立时点的正式修订 */
export interface BaselineEntryItem {
  requirement_id: string;
  requirement_code: string;
  title: string;
  type: RequirementType;
  priority: RequirementPriority;
  /** 含入的正式修订版本号 */
  revision_version: number;
}

/** 基线快照记录（UI 读模型） */
export interface BaselineRecord {
  /** 基线标识（BL-001） */
  id: string;
  version: number;
  description: string;
  established_at: string;
  established_by: string;
  status: BaselineStatus;
  entries: BaselineEntryItem[];
}

/** 基线比较中的需求修订变化（§7 Modified requirements） */
export interface BaselineComparisonModified {
  requirement_code: string;
  title: string;
  old_revision: number;
  new_revision: number;
  /** 两版验收标准的差异陈述 */
  criteria_removed: string[];
  criteria_added: string[];
}

/** 两基线比较结果（§7） */
export interface BaselineComparisonResult {
  added: BaselineEntryItem[];
  removed: BaselineEntryItem[];
  modified: BaselineComparisonModified[];
}

/** 准备向导的问题需求类别（§6 明确标识，一律锁定不得含入） */
export type BaselineIssueKind =
  | "draft"
  | "pending_confirmation"
  | "conflict"
  | "incomplete_criteria";
