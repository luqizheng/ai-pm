// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import IconButton from "./IconButton.vue";

describe("IconButton", () => {
  it("label 落到 aria-label 与 title（无障碍名必填）", () => {
    const w = mount(IconButton, { props: { label: "More actions" } });
    expect(w.attributes("aria-label")).toBe("More actions");
    expect(w.attributes("title")).toBe("More actions");
  });

  it("loading 时禁用并标记 aria-busy，slot 不渲染", () => {
    const w = mount(IconButton, {
      props: { label: "Delete", loading: true },
      slots: { default: { template: "<i data-test='icon' />" } }
    });
    expect(w.attributes("disabled")).toBeDefined();
    expect(w.attributes("aria-busy")).toBe("true");
    expect(w.find("[data-test='icon']").exists()).toBe(false);
  });

  it("disabled 时禁用", () => {
    const w = mount(IconButton, { props: { label: "X", disabled: true } });
    expect(w.attributes("disabled")).toBeDefined();
  });

  it("variant/size 落到类名", () => {
    const w = mount(IconButton, {
      props: { label: "Trash", variant: "danger", size: "lg" }
    });
    expect(w.html()).toContain("ui-icon-button--danger");
    expect(w.html()).toContain("ui-icon-button--lg");
  });
});
