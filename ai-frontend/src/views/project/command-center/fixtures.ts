/**
 * 项目总控台演示数据（mock 数据先行，AGENTS.md §三.11 同 Personal Home 先例）。
 *
 * 本文件为 fixtures（i18n 硬编码门禁豁免）：人名/项目名/指标口径等**领域数据**
 * 走这里；界面文案（区块标题/列头/动作）走 `locales/<locale>/s01/command-center.yaml`。
 * 样本值 = Pixso「Project Command Center」Default State（3:7089）。
 */

import type { HealthMetric } from "./project-health";
import type { AttentionItem } from "./attention";
import type { TaskStat } from "./task-overview";
import type { AgentMetric, AgentRow } from "./ai-agents";
import type { RunEntry } from "./run-history";
import type { DeliverableRow } from "./deliverables";
import type { ActivityItem } from "./activity";
import type { KnowledgeStat } from "./knowledge";
import type { MemberChip } from "./members";

/** 页头项目信息 */
export const projectHeaderFixture = {
  name: "Atlas Platform",
  description:
    "面向支付与订单中台的企业级 AI 研发协作平台，覆盖需求拆解、代码执行与人工验收的闭环。",
  ownerValue: "林悦 · 项目负责人",
  createdValue: "2025-03-04 · 林悦"
};

/** 项目健康度（3:7293 五卡） */
export const healthFixture: HealthMetric[] = [
  {
    label: "总体进度",
    value: 63,
    suffix: "%",
    progress: 63,
    hint: "已完成任务与已验收需求加权占比，不含未启动工作。"
  },
  {
    label: "需求进度",
    value: 12,
    suffix: "项已验收",
    progress: 46,
    hint: "共 26 条需求，另有 8 条正在执行。"
  },
  {
    label: "任务进度",
    value: 57,
    suffix: "%",
    progress: 54,
    hint: "24 个任务中 13 个已完成，其余按计划推进。"
  },
  {
    label: "风险与阻塞",
    value: 2,
    suffix: "项",
    chip: { label: "1 阻塞", tone: "danger" },
    hint: "被阻塞任务与依赖未满足的风险项合计。"
  },
  {
    label: "待人工决策",
    value: 2,
    suffix: "项",
    chip: { label: "需处理", tone: "warning" },
    hint: "需要人确认后才可继续推进的变更与取舍。"
  }
];

/** 需关注事项计数与事项行（3:7341，行样例按列头规格的示意） */
export const attentionFixture = {
  pendingCount: 3,
  urgentCount: 1,
  notifyCount: 2,
  items: [
    {
      id: "attn-1",
      title: "灰度发布策略修订待确认",
      category: "变更审阅",
      object: "REQ-118",
      statusLabel: "待人工确认",
      statusTone: "warning",
      owner: "林悦"
    },
    {
      id: "attn-2",
      title: "支付回调签名校验被依赖阻塞",
      category: "执行阻塞",
      object: "T-241",
      statusLabel: "阻塞",
      statusTone: "danger",
      owner: "陈昊"
    }
  ] satisfies AttentionItem[]
};

/** 任务概览（3:7366 三卡） */
export const taskStatsFixture: TaskStat[] = [
  {
    label: "进行中",
    unit: "个任务",
    count: 5,
    breakdown: [
      { label: "AI 执行", count: 3, tone: "success" },
      { label: "人工", count: 2, tone: "neutral" }
    ],
    hint: "已开始且未结束的任务"
  },
  {
    label: "待人工确认",
    unit: "个任务",
    count: 3,
    breakdown: [
      { label: "变更审阅", count: 2, tone: "warning" },
      { label: "验收", count: 1, tone: "warning" }
    ],
    hint: "AI 完成但需人确认"
  },
  {
    label: "已完成",
    unit: "个任务",
    count: 13,
    breakdown: [
      { label: "本周完成", count: 4, tone: "accent" },
      { label: "累计", count: 13, tone: "neutral" }
    ],
    hint: "评审通过并合入主干"
  }
];

/** AI Agents 概览（3:7475） */
export const agentMetricsFixture: AgentMetric[] = [
  { label: "运行中的 Agent", primary: "3 / 4", secondary: "在线" },
  { label: "今日运行次数", primary: "18", secondary: "次 · 成功 17" },
  { label: "平均单次执行时长", primary: "9.4", secondary: "分钟" }
];

export const agentRowsFixture: AgentRow[] = [
  {
    name: "Planner-01",
    role: "需求拆解",
    work: "拆分 REQ-118 灰度发布策略",
    statusLabel: "运行中",
    statusTone: "success",
    progress: 71,
    duration: "12m"
  },
  {
    name: "Coder-03",
    role: "代码执行",
    work: "T-241 支付回调签名校验升级",
    statusLabel: "运行中",
    statusTone: "success",
    progress: 59,
    duration: "26m"
  },
  {
    name: "Reviewer-02",
    role: "代码审阅",
    work: "PR 1286 订单导出分页重构",
    statusLabel: "审阅中",
    statusTone: "info",
    progress: 34,
    duration: "6m"
  },
  {
    name: "Tester-01",
    role: "测试执行",
    work: "T-233 审计日志查询性能优化",
    statusLabel: "已完成",
    statusTone: "success",
    progress: 100,
    duration: "41m"
  }
];

/** 最近运行记录（3:7581） */
export const runEntriesFixture: RunEntry[] = [
  {
    id: "run-8842",
    runId: "RUN-8842",
    agent: "Coder-03",
    trigger: "T-241 代码生成",
    statusLabel: "运行中",
    statusTone: "success",
    duration: "26m"
  },
  {
    id: "run-8840",
    runId: "RUN-8840",
    agent: "Planner-01",
    trigger: "REQ-118 方案拆解",
    statusLabel: "运行中",
    statusTone: "success",
    duration: "12m"
  },
  {
    id: "run-8836",
    runId: "RUN-8836",
    agent: "Reviewer-02",
    trigger: "PR 1286 审阅",
    statusLabel: "通过",
    statusTone: "info",
    duration: "6m"
  },
  {
    id: "run-8831",
    runId: "RUN-8831",
    agent: "Tester-01",
    trigger: "T-233 回归测试",
    statusLabel: "成功",
    statusTone: "success",
    duration: "41m"
  }
];

/** 需求与交付物（3:7631；人工验收与 AI 执行独立记录） */
export const deliverableRowsFixture: DeliverableRow[] = [
  {
    id: "req-118",
    code: "REQ-118",
    title: "灰度发布策略",
    acceptance: { label: "待人工验收", tone: "warning" },
    ai: { label: "执行中", tone: "success" },
    coverage: 72,
    coverageLabel: "覆盖 72%"
  },
  {
    id: "req-121",
    code: "REQ-121",
    title: "审计日志留存 180 天",
    acceptance: { label: "未开始验收", tone: "neutral" },
    ai: { label: "已完成执行", tone: "success" },
    coverage: 100,
    coverageLabel: "覆盖 100%"
  },
  {
    id: "req-114",
    code: "REQ-114",
    title: "支付渠道对账文件校验",
    acceptance: { label: "已验收", tone: "success" },
    ai: { label: "已完成执行", tone: "success" },
    coverage: 100,
    coverageLabel: "覆盖 100%"
  },
  {
    id: "req-109",
    code: "REQ-109",
    title: "订单状态机幂等改造",
    acceptance: { label: "已验收", tone: "success" },
    ai: { label: "已完成执行", tone: "success" },
    coverage: 100,
    coverageLabel: "覆盖 100%"
  }
];

/** 最近项目动态（3:7722，仅高价值事件） */
export const activityFixture: ActivityItem[] = [
  {
    id: "act-1",
    time: "09:42",
    categoryLabel: "AI 执行",
    tone: "success",
    text: "Coder-03 完成 T-241 改动并提交 PR 1289",
    linkLabel: "T-241 支付回调签名校验升级",
    linkIcon: "git-pull-request"
  },
  {
    id: "act-2",
    time: "09:18",
    categoryLabel: "需确认",
    tone: "warning",
    text: "Planner-01 提交 REQ-118 灰度发布两套方案，等待负责人选择",
    linkLabel: "REQ-118 灰度发布策略",
    linkIcon: "file-check"
  },
  {
    id: "act-3",
    time: "08:55",
    categoryLabel: "验收",
    tone: "info",
    text: "周倩 验收通过 REQ-114 支付渠道对账文件校验",
    linkLabel: "REQ-114",
    linkIcon: "copy-check"
  },
  {
    id: "act-4",
    time: "昨天",
    categoryLabel: "阻塞",
    tone: "danger",
    text: "T-229 因测试环境网关不可用被标记为阻塞，等待运维恢复",
    linkLabel: "T-229 结算对账联调",
    linkIcon: "octagon-alert"
  },
  {
    id: "act-5",
    time: "昨天",
    categoryLabel: "合并",
    tone: "neutral",
    text: "陈屿 合并 PR 1284 统一审批人解析至 main",
    linkLabel: "PR 1284",
    linkIcon: "git-merge"
  }
];

/** 知识快照（3:7814，只读摘要） */
export const knowledgeStatsFixture: KnowledgeStat[] = [
  { label: "规范文档", count: 14 },
  { label: "架构决策", count: 9 },
  { label: "历史需求", count: 38 },
  { label: "代码索引", count: 6 }
];

/** 项目成员（3:7862，只读名单） */
export const membersFixture: MemberChip[] = [
  { id: "m-linyue", name: "林悦" },
  { id: "m-chenyu", name: "陈屿" },
  { id: "m-zhouqian", name: "周倩" },
  { id: "m-shenzhiheng", name: "沈之衡" },
  { id: "m-gunan", name: "顾南" },
  { id: "m-hexiao", name: "何晓" }
];
