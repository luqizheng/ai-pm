// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import AvatarGroup from "./AvatarGroup.vue";

describe("AvatarGroup", () => {
  it("渲染全部成员首字母", () => {
    const w = mount(AvatarGroup, {
      props: { avatars: ["Alex Morgan", "Rin Sato"] }
    });
    expect(w.text()).toContain("AM");
    expect(w.text()).toContain("RS");
  });

  it("超出 max 的收进计数徽标", () => {
    const w = mount(AvatarGroup, {
      props: { avatars: ["A B", "C D", "E F", "G H", "I J", "K L"], max: 4 }
    });
    expect(w.text()).toContain("+2");
    expect(w.text()).not.toContain("IJ");
  });

  it("未超出时不渲染计数", () => {
    const w = mount(AvatarGroup, { props: { avatars: ["A B"], max: 4 } });
    expect(w.text()).not.toMatch(/\+\d/);
  });

  it("size 落到类名", () => {
    const w = mount(AvatarGroup, {
      props: { avatars: ["A B"], size: "lg" }
    });
    expect(w.html()).toContain("ui-avatar-group--lg");
  });
});
