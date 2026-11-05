/**
 * 任务概览（Project Command Center）展示数据契约。
 *
 * 组件为纯展示件：文案由调用方翻译后经 props 传入（AGENTS.md §三.11 组件策略），
 * 语义色只收 `tone` 档位，不收裸色值。
 */

/** StatusIndicator 支持的语义档（与公共组件同枚举） */
export type TaskStatTone =
  | "neutral"
  | "info"
  | "success"
  | "warning"
  | "danger"
  | "accent";

/** 迷你统计卡内的明细行：语义点 + 标签 + 右对齐计数 */
export interface TaskStatBreakdown {
  /** 明细标签（调用方翻译） */
  label: string;
  /** 明细计数 */
  count: number;
  /** 语义档 → StatusIndicator 状态点颜色 */
  tone: TaskStatTone;
}

/** 单张迷你统计卡（Pixso「Project Command Center」3:7377 系）：
 *  标签行 → 大数字 → 明细行 → 口径脚注 */
export interface TaskStat {
  /** 卡片标签（如「进行中」） */
  label: string;
  /** 计数单位（如「个任务」），弱化右对齐 */
  unit?: string;
  /** 主计数 */
  count: number;
  /** 明细分解（0~2 行） */
  breakdown?: TaskStatBreakdown[];
  /** 口径脚注 */
  hint?: string;
}
