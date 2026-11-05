/**
 * AI Agents 概览（Project Command Center）展示数据契约。
 *
 * 组件为纯展示件：文案由调用方翻译后经 props 传入（AGENTS.md §三.11 组件策略），
 * 语义色只收 `tone` 档位，不收裸色值。
 */

import type { TaskStatTone } from "./task-overview";

/** 头部三张紧凑指标（设计稿：标签 + 混排值） */
export interface AgentMetric {
  /** 指标标签（如「运行中的 Agent」） */
  label: string;
  /** 主值（如「3 / 4」「18」「9.4」） */
  primary: string;
  /** 值后缀（如「在线」「次 · 成功 17」「分钟」）；不传不渲染 */
  secondary?: string;
}

/** Agent 运行行 */
export interface AgentRow {
  /** Agent 名称（如「Planner-01」） */
  name: string;
  /** 角色名（如「需求拆解」） */
  role: string;
  /** 当前工作描述 */
  work: string;
  /** 状态文案（如「运行中」） */
  statusLabel: string;
  /** 状态语义档 → StatusIndicator */
  statusTone: TaskStatTone;
  /** 进度 0~100；undefined/null 不渲染进度条 */
  progress?: number | null;
  /** 已用时长文案（如「12m」） */
  duration: string;
}

/** 列头文案（调用方翻译） */
export interface AiAgentsColumnLabels {
  agent: string;
  work: string;
  status: string;
  progress: string;
  duration: string;
}
