// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import DeliverablesCard from "./DeliverablesCard.vue";
import type { DeliverableRow } from "./deliverables";

/** 数据样本 = Pixso「Project Command Center」3:7647 的前两行 */
const rows: DeliverableRow[] = [
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
  }
];

const columnLabels = {
  requirement: "需求",
  acceptance: "验收",
  ai: "AI",
  deliverable: "交付物"
};

describe("DeliverablesCard", () => {
  it("渲染头部、四列列头与独立性脚注", async () => {
    const wrapper = mount(DeliverablesCard, {
      props: {
        title: "需求与交付物",
        meta: "人工验收 / AI 执行分列",
        actionLabel: "查看全部",
        columnLabels,
        rows,
        footnote: "人工验收与 AI 执行状态独立记录，AI 完成不代表验收通过。"
      }
    });

    expect(wrapper.find("h3").text()).toBe("需求与交付物");
    expect(wrapper.findAll(".pm-deliv__col-head")).toHaveLength(4);
    expect(wrapper.text()).toContain("AI 完成不代表验收通过");

    await wrapper.find(".pm-section-head__action").trigger("click");
    expect(wrapper.emitted("action")).toHaveLength(1);
  });

  it("需求行渲染编号/标题/双列状态/覆盖率，满覆盖走 success 进度条", () => {
    const wrapper = mount(DeliverablesCard, {
      props: { title: "需求与交付物", actionLabel: "查看全部", columnLabels, rows }
    });

    const items = wrapper.findAll("ul.pm-deliv__rows .pm-deliv__row");
    expect(items).toHaveLength(2);
    expect(wrapper.text()).toContain("灰度发布策略");
    expect(wrapper.findAll('[role="status"]')).toHaveLength(4);
    expect(wrapper.findAll('[role="progressbar"]')).toHaveLength(2);
    expect(wrapper.find(".ui-progress--success").exists()).toBe(true);
    expect(wrapper.find(".ui-progress--accent").exists()).toBe(true);
  });

  it("acceptance/ai 缺省显示占位，coverage 缺省不渲染进度条与脚注", () => {
    const wrapper = mount(DeliverablesCard, {
      props: {
        title: "需求与交付物",
        actionLabel: "查看全部",
        columnLabels,
        rows: [{ id: "req-x", code: "REQ-999", title: "未排期需求" }]
      }
    });

    expect(wrapper.findAll(".pm-deliv__state-none")).toHaveLength(2);
    expect(wrapper.findAll('[role="progressbar"]')).toHaveLength(0);
    expect(wrapper.find(".pm-deliv__footnote").exists()).toBe(false);
  });
});
