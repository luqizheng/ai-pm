/**
 * 最近运行记录（Project Command Center）展示数据契约。
 *
 * 组件为纯展示件：文案由调用方翻译后经 props 传入（AGENTS.md §三.11 组件策略），
 * 语义色只收 `tone` 档位，不收裸色值。
 */

import type { TaskStatTone } from "./task-overview";

/** 单条 Agent 运行记录（Pixso 3:7599 系） */
export interface RunEntry {
  /** 稳定键 */
  id: string;
  /** 运行号（如「RUN-8842」） */
  runId: string;
  /** 执行 Agent 名（如「Coder-03」） */
  agent: string;
  /** 触发说明（如「T-241 代码生成」） */
  trigger: string;
  /** 结果文案（如「运行中」「成功」） */
  statusLabel: string;
  /** 结果语义档 → StatusIndicator */
  statusTone: TaskStatTone;
  /** 耗时（如「26m」） */
  duration: string;
}

/** 列头文案（调用方翻译） */
export interface RunColumnLabels {
  run: string;
  agent: string;
  trigger: string;
  result: string;
  duration: string;
}
