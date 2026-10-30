// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { mountShell } from "@/shell/test-utils";
import DashboardView from "./DashboardView.vue";

const push = vi.fn();
vi.mock("vue-router", () => ({
  useRouter: () => ({ push })
}));

describe("概览", () => {
  beforeEach(() => {
    push.mockClear();
  });

  it("渲染指标卡与标题", () => {
    const w = mountShell(DashboardView);
    expect(w.find(".ui-dashboard__title").exists()).toBe(true);
    expect(w.findAll(".ui-dashboard__metric").length).toBeGreaterThan(0);
  });

  it("指标值使用等宽数字", () => {
    const w = mountShell(DashboardView);
    expect(w.find(".ui-dashboard__metric-value").exists()).toBe(true);
  });

  it("渲染 AI 工作空间预览入口", () => {
    const w = mountShell(DashboardView);
    expect(w.find("[data-test='workspace-entry']").exists()).toBe(true);
    expect(w.find("[data-test='workspace-entry']").text()).toContain(
      "AI 工作空间"
    );
  });

  it("点击入口跳转总控台演示项目", async () => {
    const w = mountShell(DashboardView);
    await w.find("[data-test='workspace-entry']").trigger("click");
    expect(push).toHaveBeenCalledWith("/workspace/proj-worldmap/console");
  });
});
