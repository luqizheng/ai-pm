// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import AiAgentsOverviewCard from "./AiAgentsOverviewCard.vue";
import type { AgentMetric, AgentRow, AiAgentsColumnLabels } from "./ai-agents";

const metrics: AgentMetric[] = [
  { label: "运行中的 Agent", primary: "3 / 4", secondary: "在线" },
  { label: "今日运行次数", primary: "18", secondary: "次 · 成功 17" },
  { label: "平均单次执行时长", primary: "9.4", secondary: "分钟" }
];

const columnLabels: AiAgentsColumnLabels = {
  agent: "Agent / 角色",
  work: "当前工作",
  status: "状态",
  progress: "进度",
  duration: "时长"
};

/** 数据样本 = Pixso「Project Command Center」3:7503 的四行 */
const rows: AgentRow[] = [
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
    work: "PR #1286 订单导出分页重构",
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

function mountCard(overrides: Record<string, unknown> = {}) {
  return mount(AiAgentsOverviewCard, {
    props: {
      title: "AI Agents 概览",
      meta: "独立于任务状态",
      actionLabel: "AI R&D",
      metrics,
      columnLabels,
      rows,
      ...overrides
    }
  });
}

describe("AiAgentsOverviewCard", () => {
  it("渲染标题、元信息 chip 与入口，点击发出 action", async () => {
    const wrapper = mountCard();

    expect(wrapper.find("h3").text()).toBe("AI Agents 概览");
    expect(wrapper.text()).toContain("独立于任务状态");
    expect(wrapper.text()).toContain("AI R&D");

    await wrapper.find(".pm-section-head__action").trigger("click");
    expect(wrapper.emitted("action")).toHaveLength(1);
  });

  it("渲染三张紧凑指标 tile（主值 + 后缀）", () => {
    const wrapper = mountCard();

    const tiles = wrapper.findAll(".pm-agents__metric");
    expect(tiles).toHaveLength(3);
    expect(wrapper.text()).toContain("3 / 4");
    expect(wrapper.text()).toContain("在线");
    expect(wrapper.text()).toContain("次 · 成功 17");
    expect(wrapper.text()).toContain("9.4");
  });

  it("渲染列头带与四行 Agent 运行态", () => {
    const wrapper = mountCard();

    expect(wrapper.findAll(".pm-agents__col-head")).toHaveLength(5);
    expect(wrapper.text()).toContain("Agent / 角色");
    expect(wrapper.text()).toContain("当前工作");

    const agentRows = wrapper.findAll("ul.pm-agents__rows .pm-agents__row");
    expect(agentRows).toHaveLength(4);
    expect(wrapper.text()).toContain("Planner-01");
    expect(wrapper.text()).toContain("需求拆解");
    expect(wrapper.text()).toContain("T-241 支付回调签名校验升级");
    expect(wrapper.text()).toContain("41m");
  });

  it("每行一个语义点与一条进度条；完成的行走 success 变体", () => {
    const wrapper = mountCard();

    expect(wrapper.findAll('[role="status"]')).toHaveLength(4);
    expect(wrapper.findAll('[role="progressbar"]')).toHaveLength(4);
    expect(wrapper.find(".ui-progress--success").exists()).toBe(true);
    expect(wrapper.findAll(".ui-progress--accent")).toHaveLength(3);
    expect(wrapper.find(".ui-status--info").exists()).toBe(true);
  });

  it("progress 缺省的行不渲染进度条；meta 缺省不渲染 chip", () => {
    const wrapper = mountCard({
      meta: "",
      rows: [rows[0], { ...rows[1], progress: null }]
    });

    expect(wrapper.find(".ui-badge").exists()).toBe(false);
    expect(wrapper.findAll('[role="progressbar"]')).toHaveLength(1);
  });
});
