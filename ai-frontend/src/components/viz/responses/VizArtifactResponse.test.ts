// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";

import { mountShell, settle } from "@/lib/test-utils";

import VizArtifactResponse from "./VizArtifactResponse.vue";

function payload(overrides: Record<string, unknown> = {}) {
  return {
    artifactId: "artifact-1",
    slotKey: "req-trace:R-042",
    previewUrl: "blob:mock",
    title: "追踪图",
    state: "draft" as const,
    nature: "business" as const,
    versionNo: 3,
    nodeCount: 12,
    edgeCount: 20,
    ...overrides
  };
}

afterEach(() => {
  document.body.innerHTML = "";
});

describe("VizArtifactResponse（对话卡）", () => {
  it("渲染预览与计数", async () => {
    const wrapper = mountShell(VizArtifactResponse, {
      props: { payload: payload() }
    });
    await settle();
    expect(wrapper.find('[data-testid="viz-response-image"]').exists()).toBe(
      true
    );
    expect(wrapper.text()).toContain("12");
    expect(wrapper.text()).toContain("20");
  });

  it("accept / reject 回传审批 id", async () => {
    const wrapper = mountShell(VizArtifactResponse, {
      props: { payload: payload({ approvalId: "approval-1" }) }
    });
    await settle();
    await wrapper.get('[data-testid="viz-accept"]').trigger("click");
    await wrapper.get('[data-testid="viz-reject"]').trigger("click");
    expect(wrapper.emitted("accept")?.[0][0]).toBe("approval-1");
    expect(wrapper.emitted("reject")?.[0][0]).toBe("approval-1");
  });

  it("explanatory 产物不提供接受入口（不得转为正式业务事实）", async () => {
    const wrapper = mountShell(VizArtifactResponse, {
      props: { payload: payload({ nature: "explanatory" }) }
    });
    await settle();
    expect(wrapper.find('[data-testid="viz-nature-badge"]').exists()).toBe(
      true
    );
    expect(wrapper.find('[data-testid="viz-accept"]').exists()).toBe(false);
  });

  it("有解释载荷时渲染 AI 解释面板（Direct / Indirect 分区）", async () => {
    const wrapper = mountShell(VizArtifactResponse, {
      props: {
        payload: payload({
          explanation: {
            summary: "追踪图概览",
            directRelations: ["Design 覆盖 Requirement"],
            indirectRelations: ["Task 经 Requirement 关联 Design"]
          }
        })
      }
    });
    await settle();
    expect(wrapper.find('[data-testid="viz-explanation"]').exists()).toBe(true);
    expect(wrapper.text()).toContain("间接推导");
  });

  it("反馈区回传自然语言反馈（不修改业务对象）", async () => {
    const wrapper = mountShell(VizArtifactResponse, {
      props: { payload: payload() }
    });
    await settle();
    const input = wrapper.get('[data-testid="viz-feedback-input"]');
    await input.setValue("只显示已确认的需求");
    await wrapper.get('[data-testid="viz-feedback-submit"]').trigger("click");
    expect(wrapper.emitted("feedback")?.[0][0]).toBe("只显示已确认的需求");
  });
});
