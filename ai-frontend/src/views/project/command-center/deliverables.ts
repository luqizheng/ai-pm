/**
 * 需求与交付物（Project Command Center）展示数据契约。
 *
 * 关键语义（设计稿脚注）：人工验收与 AI 执行状态**独立记录**，
 * AI 完成不代表验收通过 —— 两列状态各自建模，互不推导。
 */

import type { TaskStatTone } from "./task-overview";

/** 独立记录的一列状态（人工验收 / AI 执行共用） */
export interface DeliverableState {
  /** 状态文案 */
  label: string;
  /** 语义档 → StatusIndicator */
  tone: TaskStatTone;
}

/** 单条需求行（Pixso 3:7648 系） */
export interface DeliverableRow {
  /** 稳定键 */
  id: string;
  /** 需求编号（如「REQ-118」） */
  code: string;
  /** 需求标题 */
  title: string;
  /** 人工验收状态；undefined/null 显示占位 */
  acceptance?: DeliverableState | null;
  /** AI 执行状态；undefined/null 显示占位 */
  ai?: DeliverableState | null;
  /** 交付物覆盖率 0~100（进度条）；undefined/null 不渲染 */
  coverage?: number | null;
  /** 覆盖率文案（如「覆盖 72%」）；不传不渲染 */
  coverageLabel?: string;
}

/** 列头文案（调用方翻译） */
export interface DeliverableColumnLabels {
  requirement: string;
  acceptance: string;
  ai: string;
  deliverable: string;
}
