// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import MembersCard from "./MembersCard.vue";

const members = [
  { id: "m-1", name: "林悦" },
  { id: "m-2", name: "陈屿" },
  { id: "m-3", name: "沈之衡" }
];

describe("MembersCard", () => {
  it("渲染标题、人数构成 chip 与成员 pill（Avatar 首字母回退）", () => {
    const wrapper = mount(MembersCard, {
      props: { title: "项目成员", meta: "8 人 · 4 Agent", members }
    });

    expect(wrapper.find("h3").text()).toBe("项目成员");
    expect(wrapper.text()).toContain("8 人 · 4 Agent");

    const pills = wrapper.findAll(".pm-members__pill");
    expect(pills).toHaveLength(3);
    expect(wrapper.text()).toContain("林悦");
    // Avatar 首字母回退：中文取姓名首字
    expect(wrapper.find(".ui-avatar__initials").text()).toBe("林");
  });
});
