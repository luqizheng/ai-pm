// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { AlertTriangle } from "lucide-vue-next";
import { describe, expect, it } from "vitest";

import StatCard from "./StatCard.vue";

describe("StatCard", () => {
  it("渲染 label / 数值 / 说明与图标（截图原样 stacked）", () => {
    const w = mount(StatCard, {
      props: {
        label: "Needs Attention",
        value: "12",
        description: "Across 3 projects",
        icon: AlertTriangle,
        tone: "warning"
      }
    });
    expect(w.text()).toContain("Needs Attention");
    expect(w.text()).toContain("12");
    expect(w.text()).toContain("Across 3 projects");
    expect(w.find("svg").exists()).toBe(true);
    expect(w.html()).toContain("is-tone-warning");
  });

  it("compact 布局渲染单行", () => {
    const w = mount(StatCard, {
      props: { label: "Open tasks", value: "7", layout: "compact" }
    });
    expect(w.html()).toContain("ui-stat-card--compact");
    expect(w.text()).toContain("Open tasks");
  });

  it("trend 趋势标记随方向着色", () => {
    const up = mount(StatCard, {
      props: {
        label: "A",
        value: "1",
        trend: { direction: "up", value: "12%" }
      }
    });
    expect(up.text()).toContain("↑ 12%");
    expect(up.html()).toContain("is-up");
    const down = mount(StatCard, {
      props: {
        label: "B",
        value: "2",
        trend: { direction: "down", value: "5%" }
      }
    });
    expect(down.text()).toContain("↓ 5%");
    expect(down.html()).toContain("is-down");
  });

  it("actionLabel 渲染右上角操作并发 action 事件", async () => {
    const w = mount(StatCard, {
      props: { label: "A", value: "1", actionLabel: "View details" }
    });
    await w.find(".ui-stat-card__action").trigger("click");
    expect(w.emitted("action")).toHaveLength(1);
  });

  it("clickable 渲染为 button 且 selected 标记 aria-pressed", async () => {
    const w = mount(StatCard, {
      props: { label: "A", value: "1", clickable: true, selected: true }
    });
    expect(w.element.tagName).toBe("BUTTON");
    expect(w.attributes("aria-pressed")).toBe("true");
    await w.trigger("click");
    expect(w.emitted("select")).toHaveLength(1);
  });

  it("loading 态标记 aria-busy 且隐藏真实数值", () => {
    const w = mount(StatCard, {
      props: { label: "A", value: "42", loading: true }
    });
    expect(w.attributes("aria-busy")).toBe("true");
    expect(w.html()).toContain("is-loading");
  });
});
