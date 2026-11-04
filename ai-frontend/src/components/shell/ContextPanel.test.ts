// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";

import ContextPanel from "./ContextPanel.vue";
import { useShellStore } from "@/stores/shell";
import { mountShell } from "./test-utils";

/**
 * 右上下文栏行为（Application Shell §2D）。
 */
describe("ContextPanel", () => {
  it("渲染当前项目与仓库元数据", async () => {
    const { wrapper } = mountShell(ContextPanel);
    await nextTick();

    expect(wrapper.find(".shell-context__project-name").text()).toBe(
      "Atlas Platform"
    );

    const labels = wrapper.findAll(".shell-context__meta-label");
    expect(labels.map(el => el.text())).toEqual([
      "可见性",
      "默认分支",
      "我的角色",
      "归属团队"
    ]);
  });

  it("渲染相关信息链接与助手上下文提示", async () => {
    const { wrapper } = mountShell(ContextPanel);
    await nextTick();

    expect(wrapper.findAll(".shell-context__link").length).toBe(2);
    expect(wrapper.find(".shell-context__ai").exists()).toBe(true);
  });

  it("点击关闭按钮收起右上下文栏（翻转 store 的 contextOpen）", async () => {
    const { wrapper } = mountShell(ContextPanel);
    const shell = useShellStore();
    expect(shell.contextOpen).toBe(true);

    const close = wrapper
      .findAll(".shell-context__icon-button")
      .find(el => el.attributes("aria-label") === "关闭上下文栏");
    expect(close).toBeTruthy();

    await close!.trigger("click");
    await nextTick();

    expect(shell.contextOpen).toBe(false);
  });
});
