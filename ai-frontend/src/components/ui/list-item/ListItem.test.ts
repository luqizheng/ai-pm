// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { GitBranch } from "lucide-vue-next";
import { describe, expect, it } from "vitest";

import ListItem from "./ListItem.vue";

describe("ListItem", () => {
  it("渲染标题与描述", () => {
    const w = mount(ListItem, {
      props: { title: "Atlas", description: "Main development repository" }
    });
    expect(w.text()).toContain("Atlas");
    expect(w.text()).toContain("Main development repository");
  });

  it("icon 渲染", () => {
    const w = mount(ListItem, { props: { title: "A", icon: GitBranch } });
    expect(w.find("svg").exists()).toBe(true);
  });

  it("selected 标记 aria-selected", () => {
    const w = mount(ListItem, { props: { title: "A", selected: true } });
    expect(w.attributes("aria-selected")).toBe("true");
  });

  it("点击发出 click", async () => {
    const w = mount(ListItem, { props: { title: "A" } });
    await w.trigger("click");
    expect(w.emitted("click")).toHaveLength(1);
  });

  it("disabled 禁用", () => {
    const w = mount(ListItem, { props: { title: "A", disabled: true } });
    expect(w.attributes("disabled")).toBeDefined();
  });

  it("trailing 插槽渲染", () => {
    const w = mount(ListItem, {
      props: { title: "A" },
      slots: { trailing: { template: "<em data-test='t'>2h</em>" } }
    });
    expect(w.find("[data-test='t']").text()).toBe("2h");
  });
});
