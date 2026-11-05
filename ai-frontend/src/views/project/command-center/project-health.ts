/**
 * 项目健康度区（Project Command Center）展示数据契约。
 *
 * 组件为纯展示件：文案由调用方翻译后经 props 传入（AGENTS.md §三.11 组件策略），
 * 语义色只收 `tone` 档位，不收裸色值。
 */

import type { TaskStatTone } from "./task-overview";

/** 单张健康度指标卡（Pixso 3:7294 系） */
export interface HealthMetric {
  /** 指标名（如「总体进度」） */
  label: string;
  /** 主值（数字或「—」占位） */
  value: string | number;
  /** 值后缀（如「%」「项已验收」「项」）；不传不渲染 */
  suffix?: string;
  /** 进度 0~100；undefined/null 不渲染进度条（风险/决策卡无进度） */
  progress?: number | null;
  /** 标签行右侧警示 chip（如「1 阻塞」「需处理」）；不传不渲染 */
  chip?: { label: string; tone: TaskStatTone };
  /** 口径脚注 */
  hint?: string;
}
