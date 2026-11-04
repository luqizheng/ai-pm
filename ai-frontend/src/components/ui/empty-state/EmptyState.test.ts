// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import EmptyState from "./EmptyState.vue";

describe("EmptyState", () => {
  it("渲染标题与描述", () => {
    const w = mount(EmptyState, {
      props: {
        title: "No repositories yet",
        description: "Connect a repository to begin discovery."
      }
    });
    expect(w.text()).toContain("No repositories yet");
    expect(w.text()).toContain("Connect a repository to begin discovery.");
  });

  it("无描述时不渲染描述节点", () => {
    const w = mount(EmptyState, { props: { title: "Empty" } });
    expect(w.find(".ui-empty__description").exists()).toBe(false);
  });

  it("actionLabel 存在时渲染主按钮并发出 action", async () => {
    const w = mount(EmptyState, {
      props: { title: "E", actionLabel: "Create item" }
    });
    await w.find(".ui-empty__btn--primary").trigger("click");
    expect(w.emitted("action")).toHaveLength(1);
  });

  it("secondaryActionLabel 渲染次按钮并发出事件", async () => {
    const w = mount(EmptyState, {
      props: { title: "E", secondaryActionLabel: "Import" }
    });
    await w.find(".ui-empty__btn--secondary").trigger("click");
    expect(w.emitted("secondaryAction")).toHaveLength(1);
  });

  it("默认图标渲染且 role=status", () => {
    const w = mount(EmptyState, { props: { title: "E" } });
    expect(w.find("svg").exists()).toBe(true);
    expect(w.attributes("role")).toBe("status");
  });
});
