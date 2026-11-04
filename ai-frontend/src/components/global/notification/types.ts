/**
 * Global/Notification 领域类型（Figma「03 — Notification Dropdown」/
 * 「04 — Notification Center」）。
 *
 * 设计口径（Figma 帧注记）：
 *   - 定位是**锚定的事件流，不是聊天**（an anchored event feed, not chat）；
 *   - 需人工处理的记录带显式 Human attention 徽标，普通更新保持安静；
 *   - 未读计数是文本徽标（textual），不是只有色点。
 */

/** 六类通知事件（设计任务书 §4B） */
export type NotificationCategory =
  | "taskAssignment"
  | "aiUpdates"
  | "approvalRequests"
  | "projectEvents"
  | "repositoryEvents"
  | "systemAlerts";

/** 分类展示顺序（下拉与中心一致） */
export const NOTIFICATION_CATEGORIES: NotificationCategory[] = [
  "approvalRequests",
  "taskAssignment",
  "aiUpdates",
  "projectEvents",
  "repositoryEvents",
  "systemAlerts"
];

/** 一条通知记录（DS/Data/NotificationItem 13:34472 + Record 33:87784） */
export interface NotificationRecord {
  id: string;
  category: NotificationCategory;
  /** 需人工处理（Human attention 徽标；审批请求 / 任务指派） */
  requiresAttention: boolean;
  /** 标题（未读 13 Semi Bold / 已读 13 Regular） */
  title: string;
  /** 11px 短描述 */
  description: string;
  /** 相对时间（demo 静态串，如 "2m ago"） */
  timestamp: string;
  /** 关联对象串（如 "Atlas Platform · REQ-142"） */
  object: string;
  /** 打开对象的按钮文案专有名词（如 "REQ-142"；空 = 不显示打开按钮） */
  openLabel: string;
  /** 打开对象的导航目标 */
  target: string;
  read: boolean;
}

/** 审批详情的人工决策态（DS/AI/HumanApprovalIndicator 13:34834） */
export type ApprovalIndicatorState = "pending" | "approved" | "rejected";
