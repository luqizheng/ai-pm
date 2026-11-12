// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";

import { mountShell } from "@/lib/test-utils";

import VizFeedbackArea from "./VizFeedbackArea.vue";

afterEach(() => {
  document.body.innerHTML = "";
});

describe("VizFeedbackArea", () => {
  it("空输入时禁止提交", () => {
    const wrapper = mountShell(VizFeedbackArea, { props: {} });
    expect(
      wrapper.get('[data-testid="viz-feedback-submit"]').attributes("disabled")
    ).toBeDefined();
  });

  it("提交反馈并清空输入（反馈不修改业务对象，只触发新一轮生成）", async () => {
    const wrapper = mountShell(VizFeedbackArea, { props: {} });
    const input = wrapper.get('[data-testid="viz-feedback-input"]');
    await input.setValue("只显示已确认的需求");
    await wrapper.get('[data-testid="viz-feedback-submit"]').trigger("click");

    const emitted = wrapper.emitted("feedback");
    expect(emitted?.[0][0]).toBe("只显示已确认的需求");
    expect((input.element as HTMLTextAreaElement).value).toBe("");
  });

  it("提交中禁止重复提交", () => {
    const wrapper = mountShell(VizFeedbackArea, {
      props: { submitting: true }
    });
    expect(
      wrapper.get('[data-testid="viz-feedback-submit"]').attributes("disabled")
    ).toBeDefined();
  });

  it("示例建议可填入输入框", async () => {
    const wrapper = mountShell(VizFeedbackArea, {
      props: { suggestions: ["突出显示阻塞任务"] }
    });
    await wrapper.get("button.viz-feedback__suggestion").trigger("click");
    expect(
      (
        wrapper.get('[data-testid="viz-feedback-input"]')
          .element as HTMLTextAreaElement
      ).value
    ).toBe("突出显示阻塞任务");
  });
});
