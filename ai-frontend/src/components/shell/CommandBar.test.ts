// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";

import CommandBar from "./CommandBar.vue";
import { useShellStore } from "@/stores/shell";
import { mountShell } from "./test-utils";

/**
 * AI 指令栏两态（Application Shell §2E：收起 72px / 展开 120px）。
 */
describe("CommandBar", () => {
  it("收起态只显示输入行，不显示建议与作用域", async () => {
    const { wrapper } = mountShell(CommandBar);
    await nextTick();

    expect(wrapper.find(".shell-commandbar__scope").exists()).toBe(false);
    expect(wrapper.find(".shell-commandbar__hints").exists()).toBe(false);
    expect(wrapper.find(".shell-commandbar__input").exists()).toBe(true);
  });

  it("展开态显示作用域行、人工确认提示与建议芯片", async () => {
    const { wrapper } = mountShell(CommandBar);
    const shell = useShellStore();
    shell.setCommandbarExpanded(true);
    await nextTick();

    expect(wrapper.find(".shell-commandbar__scope").exists()).toBe(true);
    expect(wrapper.find(".shell-commandbar__confirm").text()).toBe(
      "任何操作前请先确认"
    );
    expect(wrapper.findAll(".shell-commandbar__suggestion").length).toBe(2);
  });

  it("人工确认提示与 AI 建议使用不同视觉（不同颜色变量）", async () => {
    const { wrapper } = mountShell(CommandBar);
    const shell = useShellStore();
    shell.setCommandbarExpanded(true);
    await nextTick();

    const confirm = wrapper.find(".shell-commandbar__confirm").element;
    const suggestion = wrapper.find(
      ".shell-commandbar__suggestions-label"
    ).element;
    expect(confirm.className).not.toBe(suggestion.className);
  });

  it("输入为空时发送按钮禁用，输入后启用", async () => {
    const { wrapper } = mountShell(CommandBar);
    await nextTick();

    const send = wrapper.find(".shell-commandbar__send");
    expect(send.attributes("disabled")).toBeDefined();

    await wrapper.find(".shell-commandbar__field").setValue("分析风险");
    await nextTick();

    expect(
      wrapper.find(".shell-commandbar__send").attributes("disabled")
    ).toBeUndefined();
  });

  it("收起态的占位符带当前上下文", async () => {
    const { wrapper } = mountShell(CommandBar);
    await nextTick();

    expect(
      wrapper.find(".shell-commandbar__field").attributes("placeholder")
    ).toBe("向 Atlas Platform / Repository / main 提问…");
  });
});
