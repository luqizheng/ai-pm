/**
 * 需关注事项面板（Project Command Center）展示数据契约。
 *
 * 组件为纯展示件：文案由调用方翻译后经 props 传入（AGENTS.md §三.11 组件策略）。
 * 设计稿四个状态（default/needs/blocked/empty）都只给到面板壳
 * （头 + 计数 + 列头），未提供行样例 —— 行结构按列头规格落地，默认为空。
 */

import type { TaskStatTone } from "./task-overview";

/** 头部右侧的计数对（如「紧急 1」「告知 2」） */
export interface AttentionCount {
  /** 计数名（如「紧急」「告知」） */
  label: string;
  /** 数量 */
  count: number;
  /** 数字语义色档 */
  tone: "danger" | "warning";
}

/** 单条待关注事项 */
export interface AttentionItem {
  /** 稳定键（emit open 时原样带回） */
  id: string;
  /** 事项标题 */
  title: string;
  /** 类别（如「变更审阅」「执行阻塞」） */
  category: string;
  /** 业务对象（如「REQ-118」「T-241」） */
  object: string;
  /** 状态文案 */
  statusLabel: string;
  /** 状态语义档 → StatusIndicator */
  statusTone: TaskStatTone;
  /** 责任人显示名 */
  owner: string;
}

/** 列头文案（调用方翻译） */
export interface AttentionColumnLabels {
  item: string;
  category: string;
  object: string;
  status: string;
  owner: string;
  goto: string;
}
