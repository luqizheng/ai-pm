// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";

import KeyboardShortcutsDialog from "./KeyboardShortcutsDialog.vue";
import { i18n } from "@/lib/i18n";

/**
 * 键盘快捷键对话框：五组齐备；已确认键位 vs 「未分配 / 拟议」显式区分。
 */
describe("KeyboardShortcutsDialog", () => {
  function mountDialog(open = true) {
    return mount(KeyboardShortcutsDialog, {
      props: { open },
      attachTo: document.body,
      global: { plugins: [i18n] }
    });
  }

  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("打开：五组 + 搜索组四个已确认键位", async () => {
    const wrapper = mountDialog();
    await nextTick();

    const groups = document.querySelectorAll(".shortcuts-dialog__group");
    expect(groups.length).toBe(5);

    const keys = Array.from(
      document.querySelectorAll(".shortcuts-dialog__row-keys")
    ).map(el => el.textContent?.trim());
    expect(keys).toContain("Cmd/Ctrl+K");
    expect(keys).toContain("↑ / ↓");
    expect(keys).toContain("Enter");
    expect(keys).toContain("Esc");
    expect(document.querySelector(".shortcuts-dialog")?.textContent).toContain(
      "仅搜索快捷键已确认"
    );
  });

  it("未分配 / 拟议键位显式标注（不谎报为已确认）", async () => {
    mountDialog();
    await nextTick();

    const keys = Array.from(
      document.querySelectorAll(".shortcuts-dialog__row-keys")
    ).map(el => el.textContent?.trim());
    expect(keys).toContain("未分配");
    expect(keys).toContain("拟议 · 未分配");
  });

  it("完成按钮发出 update:open=false", async () => {
    const wrapper = mountDialog();
    await nextTick();

    Array.from(
      document.querySelectorAll<HTMLButtonElement>(".shortcuts-dialog button")
    ).find(b => b.textContent?.trim() === "完成")!.click();
    await nextTick();

    expect(wrapper.emitted("update:open")?.at(-1)?.[0]).toBe(false);
  });
});
