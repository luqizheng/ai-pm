// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import ProjectHealthSection from "./ProjectHealthSection.vue";
import type { HealthMetric } from "./project-health";

/** 数据样本 = Pixso「Project Command Center」3:7293 的五张指标卡 */
const metrics: HealthMetric[] = [
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

describe("ProjectHealthSection", () => {
  it("渲染标题、更新时间 chip 与「查看指标定义」入口，点击发出 action", async () => {
    const wrapper = mount(ProjectHealthSection, {
      props: { title: "项目健康度", meta: "更新于 09:42", actionLabel: "查看指标定义", metrics }
    });

    expect(wrapper.find("h3").text()).toBe("项目健康度");
    expect(wrapper.text()).toContain("更新于 09:42");
    expect(wrapper.text()).toContain("查看指标定义");

    await wrapper.find(".pm-section-head__action").trigger("click");
    expect(wrapper.emitted("action")).toHaveLength(1);
  });

  it("渲染五张指标卡，含主值/后缀/脚注", () => {
    const wrapper = mount(ProjectHealthSection, {
      props: { title: "项目健康度", actionLabel: "查看指标定义", metrics }
    });

    const tiles = wrapper.findAll(".pm-health-metric");
    expect(tiles).toHaveLength(5);
    expect(wrapper.text()).toContain("63");
    expect(wrapper.text()).toContain("项已验收");
    expect(wrapper.text()).toContain("被阻塞任务与依赖未满足的风险项合计。");
    expect(wrapper.text()).toContain("需要人确认后才可继续推进的变更与取舍。");
  });

  it("仅带 progress 的卡渲染进度条（accent 变体）", () => {
    const wrapper = mount(ProjectHealthSection, {
      props: { title: "项目健康度", actionLabel: "查看指标定义", metrics }
    });

    expect(wrapper.findAll('[role="progressbar"]')).toHaveLength(3);
    expect(wrapper.findAll(".ui-progress--accent")).toHaveLength(3);
    expect(wrapper.find(".ui-progress--success").exists()).toBe(false);
  });

  it("警示卡渲染 danger/warning Badge，非警示卡无 chip", () => {
    const wrapper = mount(ProjectHealthSection, {
      props: { title: "项目健康度", actionLabel: "查看指标定义", metrics }
    });

    const badges = wrapper.findAll(".pm-health-metric .ui-badge");
    expect(badges).toHaveLength(2);
    expect(wrapper.find(".ui-badge--danger").exists()).toBe(true);
    expect(wrapper.find(".ui-badge--warning").exists()).toBe(true);
    expect(badges[0].text()).toContain("1 阻塞");
    expect(badges[1].text()).toContain("需处理");
  });

  it("meta 缺省不渲染 chip；progress 缺省不渲染进度条", () => {
    const wrapper = mount(ProjectHealthSection, {
      props: {
        title: "项目健康度",
        actionLabel: "查看指标定义",
        metrics: [metrics[0], metrics[3]]
      }
    });

    expect(wrapper.find(".pm-section-head .ui-badge").exists()).toBe(false);
    expect(wrapper.findAll('[role="progressbar"]')).toHaveLength(1);
  });
});
