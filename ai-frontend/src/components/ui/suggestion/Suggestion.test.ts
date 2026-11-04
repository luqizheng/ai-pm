// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import Suggestion from "./Suggestion.vue";

describe("Suggestion", () => {
  it("渲染标题与正文，标题节点带 AI 标注 aria-label", () => {
    const w = mount(Suggestion, {
      props: {
        title: "Split this requirement",
        body: "Two acceptance criteria are mixed; consider splitting."
      }
    });
    expect(w.text()).toContain("Split this requirement");
    expect(w.text()).toContain("Two acceptance criteria are mixed");
    expect(w.attributes("aria-label")).toBe(
      "AI suggestion: Split this requirement"
    );
  });

  it("state 落到类名（accepted/rejected）", () => {
    const ok = mount(Suggestion, { props: { title: "A", state: "accepted" } });
    expect(ok.html()).toContain("is-accepted");
    const no = mount(Suggestion, { props: { title: "A", state: "rejected" } });
    expect(no.html()).toContain("is-rejected");
  });

  it("actions 插槽渲染", () => {
    const w = mount(Suggestion, {
      props: { title: "A" },
      slots: {
        actions: { template: "<button data-test='act'>Accept</button>" }
      }
    });
    expect(w.find("[data-test='act']").exists()).toBe(true);
  });

  it("默认 sparkles 图标渲染", () => {
    const w = mount(Suggestion, { props: { title: "A" } });
    expect(w.find(".ui-suggestion__icon").exists()).toBe(true);
  });
});
