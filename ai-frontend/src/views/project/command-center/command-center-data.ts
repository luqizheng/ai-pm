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
export type Translate = (key: string, params?: Record<string, unknown>) => string;

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
}
