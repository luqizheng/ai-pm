// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { mountShell } from "@/shell/test-utils";
import FocusView from "./FocusView.vue";

const { pushMock } = vi.hoisted(() => ({ pushMock: vi.fn() }));

vi.mock("vue-router", () => ({
  useRoute: () => ({
    params: { projectId: "proj-1", objectType: "requirement", id: "R-101" },
    query: { from: "/workspace/proj-1/requirements" }
  }),
  useRouter: () => ({ push: pushMock })
}));

describe("FocusView", () => {
  it("合法对象渲染预设画布与对象摘要", () => {
    const w = mountShell(FocusView);
    expect(w.find("[data-test='focus-canvas']").exists()).toBe(true);
    expect(w.find("[data-test='focus-summary']").exists()).toBe(true);
    expect(w.text()).toContain("requirement · R-101");
  });

  it("返回按钮回到来源页", async () => {
    pushMock.mockClear();
    const w = mountShell(FocusView);
    await w.find("[data-test='focus-back']").trigger("click");
    expect(pushMock).toHaveBeenCalledWith("/workspace/proj-1/requirements");
  });
});