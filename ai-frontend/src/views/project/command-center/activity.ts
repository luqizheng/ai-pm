/**
 * 最近项目动态（Project Command Center）展示数据契约。
 *
 * 时间线行：轨道点（语义色）+ 时间 + 类别 chip + 事件描述 + 可选关联对象链接。
 * 仅高价值事件（设计稿口径）；文案由调用方翻译后经 props 传入。
 */

import type { TaskStatTone } from "./task-overview";

/** 关联对象行的小图标（lucide canonical 名） */
export type ActivityLinkIcon =
  | "git-pull-request"
  | "file-check"
  | "copy-check"
  | "octagon-alert"
  | "git-merge";

/** 单条动态（Pixso 3:7733 系） */
export interface ActivityItem {
  /** 稳定键 */
  id: string;
  /** 时间文案（如「09:42」「昨天」） */
  time: string;
  /** 类别 chip 文案（如「AI 执行」「需确认」「阻塞」） */
  categoryLabel: string;
  /** 类别语义档（轨道点与 chip 同色） */
  tone: TaskStatTone;
  /** 事件描述 */
  text: string;
  /** 关联对象链接文案；不传不渲染第二行 */
  linkLabel?: string;
  /** 关联对象小图标；不传不渲染图标 */
  linkIcon?: ActivityLinkIcon;
}
