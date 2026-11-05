// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import RunHistoryCard from "./RunHistoryCard.vue";
import type { RunEntry } from "./run-history";

/** 数据样本 = Pixso「Project Command Center」3:7598 的四行 */
const entries: RunEntry[] = [
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
    id: "run-8836",
    runId: "RUN-8836",
    agent: "Reviewer-02",
    trigger: "PR #1286 审阅",
    statusLabel: "通过",
    statusTone: "info",
    duration: "6m"
  }
];

const columnLabels = {
  run: "运行",
  agent: "Agent",
  trigger: "触发",
  result: "结果",
  duration: "耗时"
};

describe("RunHistoryCard", () => {
  it("渲染头部、入口与五列列头", async () => {
    const wrapper = mount(RunHistoryCard, {
      props: {
        title: "最近运行记录",
        meta: "近 2 小时",
        actionLabel: "运行历史",
        columnLabels,
        entries
      }
    });

    expect(wrapper.find("h3").text()).toBe("最近运行记录");
    expect(wrapper.text()).toContain("近 2 小时");
    expect(wrapper.findAll(".pm-runs__col-head")).toHaveLength(5);

    await wrapper.find(".pm-section-head__action").trigger("click");
    expect(wrapper.emitted("action")).toHaveLength(1);
  });

  it("渲染运行行：Agent 走 accent、语义点与右对齐耗时", () => {
    const wrapper = mount(RunHistoryCard, {
      props: {
        title: "最近运行记录",
        actionLabel: "运行历史",
        columnLabels,
        entries
      }
    });

    const rows = wrapper.findAll("ul.pm-runs__rows .pm-runs__row");
    expect(rows).toHaveLength(2);
    expect(wrapper.text()).toContain("RUN-8842");
    expect(wrapper.find(".pm-runs__agent").classes()).toContain(
      "pm-runs__agent"
    );
    expect(wrapper.findAll('[role="status"]')).toHaveLength(2);
    expect(wrapper.find(".ui-status--info").exists()).toBe(true);
  });
});
