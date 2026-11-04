// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";

import PasswordInput from "./PasswordInput.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

/**
 * 口令输入：明文切换 + 可访问名称（Figma DS/Forms/PasswordInput）。
 */
describe("PasswordInput", () => {
  it("默认以掩码呈现，切换后为明文", async () => {
    const { wrapper } = mountShell(PasswordInput, {
      props: { modelValue: "Orbit-Research!26" }
    });
    await nextTick();

    expect(wrapper.find("input").attributes("type")).toBe("password");

    await wrapper.find("button").trigger("click");
    await nextTick();

    expect(wrapper.find("input").attributes("type")).toBe("text");
    expect(wrapper.find("input").element.value).toBe("Orbit-Research!26");
  });

  it("切换按钮带显示 / 隐藏口令的可访问名称", async () => {
    const { wrapper } = mountShell(PasswordInput, {
      props: { modelValue: "Orbit-Research!26" }
    });
    await nextTick();

    const toggle = wrapper.find("button");
    expect(toggle.attributes("aria-label")).toBe(
      i18n.global.t("common.password.show")
    );

    await toggle.trigger("click");
    await nextTick();

    expect(wrapper.find("button").attributes("aria-label")).toBe(
      i18n.global.t("common.password.hide")
    );
  });

  it("可从初始可见态开始（状态参照板用）", async () => {
    const { wrapper } = mountShell(PasswordInput, {
      props: { modelValue: "Orbit-Research!26", defaultVisible: true }
    });
    await nextTick();

    expect(wrapper.find("input").attributes("type")).toBe("text");
  });
});
