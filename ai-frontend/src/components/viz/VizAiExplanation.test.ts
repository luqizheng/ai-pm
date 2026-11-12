// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";

import { mountShell } from "@/lib/test-utils";

import VizAiExplanation from "./VizAiExplanation.vue";

afterEach(() => {
  document.body.innerHTML = "";
});

describe("VizAiExplanation", () => {
  it("Direct 与 Indirect 分区呈现（不得混为一谈）", () => {
    const wrapper = mountShell(VizAiExplanation, {
      props: {
        explanation: {
          summary: "追踪图概览",
          directRelations: ["Design 覆盖 Requirement"],
          indirectRelations: ["Task 经 Requirement 关联 Design"]
        }
      }
    });

    const text = wrapper.text();
    expect(text).toContain("直接关系");
    expect(text).toContain("间接推导");
    expect(text).toContain("Task 经 Requirement 关联 Design");
  });

  it("无内容时对应分组显示占位而非空白", () => {
    const wrapper = mountShell(VizAiExplanation, {
      props: { explanation: { directRelations: [], indirectRelations: [] } }
    });
    expect(wrapper.text()).toContain("（无）");
  });

  it("渲染关键发现与待确认事项", () => {
    const wrapper = mountShell(VizAiExplanation, {
      props: {
        explanation: {
          keyFindings: ["存在未覆盖需求"],
          pendingConfirmations: ["需求 R-042 待确认"]
        }
      }
    });
    const text = wrapper.text();
    expect(text).toContain("存在未覆盖需求");
    expect(text).toContain("需求 R-042 待确认");
  });
});
