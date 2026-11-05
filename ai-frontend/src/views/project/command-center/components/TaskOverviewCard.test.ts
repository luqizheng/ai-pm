// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import TaskOverviewCard from "./TaskOverviewCard.vue";
import TaskStatCard from "./TaskStatCard.vue";
import type { TaskStat } from "./task-overview";

/** 数据样本 = Pixso「Project Command Center」3:7366 的三张迷你卡 */
const stats: TaskStat[] = [
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

describe("TaskOverviewCard", () => {
  it("渲染标题、总数徽标与「全部任务」入口，点击发出 view-all", async () => {
    const wrapper = mount(TaskOverviewCard, {
      props: {
        title: "任务概览",
        totalLabel: "24 个任务",
        viewAllLabel: "全部任务",
        stats
      }
    });

    expect(wrapper.find("h3").text()).toBe("任务概览");
    expect(wrapper.text()).toContain("24 个任务");
    expect(wrapper.text()).toContain("全部任务");

    await wrapper.find(".pm-section-head__action").trigger("click");
    expect(wrapper.emitted("view-all")).toHaveLength(1);
  });

  it("为每个 stat 渲染一张迷你卡，含计数/单位/脚注", () => {
    const wrapper = mount(TaskOverviewCard, {
      props: { title: "任务概览", viewAllLabel: "全部任务", stats }
    });

    const tiles = wrapper.findAll(".pm-task-stat");
    expect(tiles).toHaveLength(3);
    expect(wrapper.text()).toContain("13");
    expect(wrapper.text()).toContain("已开始且未结束的任务");
    expect(wrapper.text()).toContain("评审通过并合入主干");
    // 总数徽标未传时不渲染
    expect(wrapper.find(".ui-badge").exists()).toBe(false);
  });

  it("明细行用 StatusIndicator 语义点并右对齐计数", () => {
    const wrapper = mount(TaskOverviewCard, {
      props: { title: "任务概览", viewAllLabel: "全部任务", stats }
    });

    const rows = wrapper.findAll(".pm-task-stat__row");
    expect(rows).toHaveLength(6);
    // 每行一个语义点（StatusIndicator role=status）+ 右对齐计数
    expect(wrapper.findAll('[role="status"]')).toHaveLength(6);
    expect(wrapper.find(".ui-status--success").exists()).toBe(true);
    expect(wrapper.find(".ui-status--warning").exists()).toBe(true);
    expect(wrapper.find(".ui-status--accent").exists()).toBe(true);
    expect(wrapper.find(".ui-status--neutral").exists()).toBe(true);
    expect(wrapper.text()).toContain("AI 执行");
    expect(wrapper.text()).toContain("变更审阅");
  });

  it("缺省 breakdown/hint/unit 时不渲染对应块", () => {
    const wrapper = mount(TaskStatCard, {
      props: { label: "进行中", count: 5 }
    });

    expect(wrapper.find(".pm-task-stat__rows").exists()).toBe(false);
    expect(wrapper.find(".pm-task-stat__hint").exists()).toBe(false);
    expect(wrapper.find(".pm-task-stat__unit").exists()).toBe(false);
    expect(wrapper.find(".pm-task-stat__count").text()).toBe("5");
  });
});
