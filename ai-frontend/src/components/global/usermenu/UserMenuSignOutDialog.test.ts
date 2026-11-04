// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";

import UserMenuSignOutDialog from "./UserMenuSignOutDialog.vue";
import { i18n } from "@/lib/i18n";

/**
 * 退出登录确认（ConfirmationDialog）：确认按钮初始禁用，
 * 人工勾选后可用——不做推断退出。
 * 对话框经 Portal 挂到 body，用 document 查询（仓库约定）。
 */
describe("UserMenuSignOutDialog", () => {
  function mountDialog(open = true) {
    return mount(UserMenuSignOutDialog, {
      props: { open, email: "alex.morgan@acme.dev" },
      attachTo: document.body,
      global: { plugins: [i18n] }
    });
  }

  function dialog() {
    return document.querySelector<HTMLElement>(".user-signout");
  }

  function confirmButton() {
    return Array.from(
      dialog()?.querySelectorAll<HTMLButtonElement>("button") ?? []
    ).find(b => b.textContent?.trim() === "退出登录")!;
  }

  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("打开：标题/说明/确认勾选渲染，确认按钮禁用", async () => {
    mountDialog();
    await nextTick();

    expect(dialog()?.textContent).toContain("退出当前账户？");
    expect(dialog()?.textContent).toContain("alex.morgan@acme.dev");
    expect(confirmButton().disabled).toBe(true);
  });

  it("勾选人工确认后确认按钮可用，点击发出 confirm 并关闭", async () => {
    const wrapper = mountDialog();
    await nextTick();

    dialog()!.querySelector<HTMLElement>("[role='checkbox']")!.click();
    await nextTick();
    expect(confirmButton().disabled).toBe(false);

    confirmButton().click();
    await nextTick();
    expect(wrapper.emitted("confirm")).toBeTruthy();
    expect(wrapper.emitted("update:open")?.at(-1)?.[0]).toBe(false);
  });

  it("关闭后重新打开：确认勾选复位（确认按钮再次禁用）", async () => {
    const wrapper = mountDialog();
    await nextTick();

    dialog()!.querySelector<HTMLElement>("[role='checkbox']")!.click();
    await nextTick();
    await wrapper.setProps({ open: false });
    await new Promise(resolve => setTimeout(resolve, 0));
    await wrapper.setProps({ open: true });
    await nextTick();

    expect(confirmButton().disabled).toBe(true);
  });
});
