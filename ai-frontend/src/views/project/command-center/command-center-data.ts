import type { HealthMetric } from "./project-health";
import type { AttentionItem } from "./attention";
import type { TaskStat, TaskStatTone } from "./task-overview";
import type { AgentMetric, AgentRow } from "./ai-agents";
import type { RunEntry } from "./run-history";
import type { DeliverableRow } from "./deliverables";
import type { ActivityItem } from "./activity";
import type { KnowledgeStat } from "./knowledge";
import type { MemberChip } from "./members";

/** 与 vue-i18n 的全局 t 同签名（由页面注入，便于纯函数测试） */
export type Translate = (
  key: string,
  params?: Record<string, unknown>
) => string;

/**
 * 项目总控台视图数据（页面组装层）。
 *
 * 两种来源都收敛到这一份契约：
 * - 发布构建：`real-data.ts` 读 `/command-center` 六块（不可用区块空态，不补 0）；
 * - 本地 dev / 单测：`fixtures.ts` 的设计稿样本（VITE_USE_MOCK=1）。
 */
export interface CommandCenterHeaderData {
  name: string;
  description: string;
  statusLabel: string;
  statusTone: TaskStatTone;
  visibilityLabel: string;
  metaEntries: Array<{ label: string; value: string }>;
}

export interface CommandCenterAttentionData {
  pendingCount: number;
  urgentCount: number;
  notifyCount: number;
  items: AttentionItem[];
}

/** 初始化单事项（展示层已翻译） */
export interface InitItemView {
  key: string;
  title: string;
  /** 展示层状态已翻译 */
  statusLabel: string;
  tone: TaskStatTone;
  required: boolean;
  detail: string;
  /** 该步对应已有页面的深链（冷启动引导：每步可单独进入）；无则不可点 */
  link?: { path: string; query?: Record<string, string> };
}

/**
 * 项目初始化进度（S01-01 Initialization Overview）。
 *
 * 后端 `/command-center` 早已返回 `initialization_progress` 块，但前端一直
 * 没映射也没渲染——S01-01 的「初始化总览」因此缺页。这里把它接进总控台。
 *
 * `completion = not_determinable` 表示无项目类型/初始化要求配置，**不得
 * 宣称已完成**（契约 §4.5）。
 */
export interface CommandCenterInitData {
  items: InitItemView[];
  /** 完成度判定：`complete` / `incomplete` / `not_determinable`（原值透出）。 */
  completion: string;
  /** 必需事项已完成数 / 必需事项总数。 */
  requiredDone: number;
  requiredTotal: number;
  /** 后端收缩判定（§4.6：无进行中/未开始的必需事项且无阻塞 → false=收起）。
   *  初始化完成后自动折叠进度区域；用户可手动展开（不隐藏人工待办）。 */
  expanded: boolean;
}

export interface CommandCenterData {
  header: CommandCenterHeaderData;
  health: HealthMetric[];
  /** 健康度区右上角新鲜度（如「09:42」）；空串不渲染 */
  healthMetaTime: string;
  attention: CommandCenterAttentionData;
  taskTotal: number;
  taskStats: TaskStat[];
  agentMetrics: AgentMetric[];
  agentRows: AgentRow[];
  runs: RunEntry[];
  deliverables: DeliverableRow[];
  activity: ActivityItem[];
  knowledge: KnowledgeStat[];
  members: MemberChip[];
  memberHumanCount: number;
  memberAgentCount: number;
  /** 初始化进度（无该块时为 null，不补空对象）。 */
  initialization: CommandCenterInitData | null;
}
