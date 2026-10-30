// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import Topbar from "./Topbar.vue";
import { mountShell } from "../test-utils";

describe("Topbar（传统壳顶栏）", () => {
  it("模式切换按钮发出 workspace 事件", async () => {
    const w = mountShell(Topbar, {
      props: {
        user: { name: "管理员", roles: ["admin"] }
      }
    });
    const toggle = w.find("[data-test='mode-toggle-ai']");
    expect(toggle.exists()).toBe(true);
    expect(toggle.attributes("aria-label")).toContain("AI");
    await toggle.trigger("click");
    expect(w.emitted("workspace")).toHaveLength(1);
  });

  it("模式切换按钮带可见文字（用户反馈：图标不可发现）", () => {
    const w = mountShell(Topbar, {
      props: {
        user: { name: "管理员", roles: ["admin"] }
      }
    });
    expect(w.find("[data-test='mode-toggle-ai']").text()).toContain(
      "AI 工作空间"
    );
  });
});
