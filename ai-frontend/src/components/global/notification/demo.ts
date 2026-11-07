/**
 * Global/Notification 演示数据（骨架阶段）。
 *
 * 与 Figma「04 — Notification Center」Selected 样本一致：6 条记录 · 3 条未读，
 * 覆盖六类事件（审批请求 / 任务指派 / AI 执行动态 / 项目 / 仓库 / 系统警报）。
 * 标题描述是业务文案，接真实数据源时由后端下发，此处随 demo 静态提供
 * （i18n 门禁扫描 src/**，demo 字符串不在扫描面）。
 */
import type { NotificationRecord } from "./types";

export const demoNotificationRecords: NotificationRecord[] = [
  {
    id: "ntf-req-142",
    category: "approvalRequests",
    requiresAttention: true,
    title: "Decision requested on REQ-142",
    description: "Jonah asks you to review the proposed timeout policy.",
    timestamp: "2m ago",
    object: "Atlas Platform · REQ-142",
    openLabel: "REQ-142",
    target: "/project/requirements",
    read: false
  },
  {
    id: "ntf-task-318",
    category: "taskAssignment",
    requiresAttention: true,
    title: "TASK-318 assigned to you",
    description: "Add token expiry coverage for REQ-142.",
    timestamp: "8m ago",
    object: "Atlas Platform · TASK-318",
    openLabel: "TASK-318",
    target: "/project/execution",
    read: false
  },
  {
    id: "ntf-run-089",
    category: "aiUpdates",
    requiresAttention: false,
    title: "RUN-089 analysis completed",
    description: "Read-only analysis is ready. No changes were made.",
    timestamp: "24m ago",
    object: "Atlas Platform · RUN-089",
    openLabel: "RUN-089",
    target: "/project/execution",
    read: false
  },
  {
    id: "ntf-prj-008-milestone",
    category: "projectEvents",
    requiresAttention: false,
    title: "Milestone scope updated",
    description: "Jonah updated the Q4 reliability milestone.",
    timestamp: "1h ago",
    object: "Atlas Platform · PRJ-008",
    openLabel: "PRJ-008",
    target: "/project/settings/repositories",
    read: true
  },
  {
    id: "ntf-sdk-branch",
    category: "repositoryEvents",
    requiresAttention: false,
    title: "Default branch updated",
    description: "The project repository default branch is now main.",
    timestamp: "2h ago",
    object: "SDK Toolkit · sdk-client",
    openLabel: "sdk-client",
    target: "/project/settings/repositories",
    read: true
  },
  {
    id: "ntf-atlas-connection",
    category: "systemAlerts",
    requiresAttention: false,
    title: "Repository connection needs attention",
    description: "The atlas-api connection is unavailable. Review access.",
    timestamp: "3h ago",
    object: "Atlas Platform · atlas-api",
    openLabel: "atlas-api",
    target: "/project/settings/repositories",
    read: true
  }
];

/** 通知加载器契约（store.load 注入；rejected → Error 态） */
export type NotificationsLoader = () => Promise<NotificationRecord[]>;

/** 演示加载器：短延时跑出 Loading 态 */
export const demoNotificationsLoader: NotificationsLoader = () =>
  new Promise(resolve => {
    setTimeout(() => resolve(demoNotificationRecords), 250);
  });
