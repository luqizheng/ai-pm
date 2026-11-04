// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { Archive } from "lucide-vue-next";
import { describe, expect, it } from "vitest";

import ConfirmationDialog from "./ConfirmationDialog.vue";

describe("ConfirmationDialog", () => {
  function mountDialog(extra = {}) {
    return mount(ConfirmationDialog, {
      attachTo: document.body,
      props: {
        open: true,
        title: "Archive this item?",
        description:
          "The item will be hidden from active lists. You can restore it later.",
        confirmLabel: "Archive item",
        cancelLabel: "Cancel",
        ...extra
      }
    });
  }

  it("open=true 渲染标题 / 说明 / 双按钮", () => {
    const w = mountDialog();
    expect(document.body.textContent).toContain("Archive this item?");
    expect(document.body.textContent).toContain("Archive item");
    expect(document.body.textContent).toContain("Cancel");
    w.unmount();
  });

  it("open=false 不渲染面板", () => {
    const w = mount(ConfirmationDialog, {
      attachTo: document.body,
      props: {
        open: false,
        title: "X",
        confirmLabel: "OK",
        cancelLabel: "Cancel"
      }
    });
    expect(document.querySelector(".ui-confirm__panel")).toBeNull();
    w.unmount();
  });

  it("点击确认 / 取消发出对应事件", async () => {
    const w = mountDialog();
    const confirm = document.querySelector(
      ".ui-confirm__btn--primary"
    ) as HTMLButtonElement;
    confirm.click();
    expect(w.emitted("confirm")).toHaveLength(1);

    const cancel = document.querySelector(
      ".ui-confirm__btn--cancel"
    ) as HTMLButtonElement;
    cancel.click();
    expect(w.emitted("cancel")).toHaveLength(1);
    expect(w.emitted("update:open")?.[0]).toEqual([false]);
    w.unmount();
  });

  it("variant=danger 时确认按钮走 danger 面", () => {
    const w = mountDialog({ variant: "danger" });
    expect(document.querySelector(".ui-confirm__btn--danger")).not.toBeNull();
    w.unmount();
  });

  it("checkboxLabel 存在时渲染确认勾选行", () => {
    mountDialog({ checkboxLabel: "I have reviewed this change." });
    expect(document.body.textContent).toContain("I have reviewed this change.");
    document.body.innerHTML = "";
  });

  it("icon prop 渲染图标", () => {
    mountDialog({ icon: Archive });
    expect(document.querySelector(".ui-confirm__icon")).not.toBeNull();
    document.body.innerHTML = "";
  });
});
