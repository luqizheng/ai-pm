// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";

import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";

import AiResponseCard from "./AiResponseCard.vue";
import type { FooterAction } from "../types";
import { aiIdentity, projectEvidence } from "../fixtures";

const i18n = createI18n({
  legacy: false,
  locale: "zh-CN",
  messages: {
    "zh-CN": {
      ai: {
        card: {
          type: {
            informational: "信息",
            recommendation: "建议",
            proposal: "提案",
            question: "信息补充",
            decision: "待决策",
            artifact: "产物",
            actionRequest: "操作确认",
            executionResult: "执行结果"
          },
          status: {
            created: "已创建",
            presented: "待处理",
            inProgress: "处理中",
            submitted: "已提交",
            validating: "校验中",
            processing: "执行中",
            accepted: "已接收",
            rejected: "已拒绝",
            deferred: "稍后处理",
            resolved: "已记录",
            failed: "提交失败"
          },
          conclusion: "结论",
          expand: "展开",
          collapse: "收起",
          more: "更多操作",
          evidence: {
            title: "证据与来源",
            analysis: "分析",
            sourcesCount: "{count} 个来源",
            relatedCount: "{count} 个关联对象"
          },
          submission: {
            submitting: "正在提交…",
            recorded: "已记录你的选择",
            reference: "· 记录号 {reference}",
            notEffective: "正式业务变更以审批结果为准",
            failed: "提交失败，请重试",
            retry: "重试"
          }
        }
      }
    }
  }
});

function mountCard(props: Record<string, unknown> = {}, slots = {}) {
  return mount(AiResponseCard, {
    props: {
      type: "recommendation",
      identity: aiIdentity,
      title: "建议拆分森林生成任务",
      description: "先解冻地形网格接口。",
      ...props
    },
    slots,
    global: { plugins: [i18n] }
  });
}

describe("AiResponseCard", () => {
  it("渲染身份、类型、状态、标题与结论", () => {
    const wrapper = mountCard({ conclusion: "拆分后本迭代可完成。" });

    expect(wrapper.text()).toContain("AI-PM");
    expect(wrapper.text()).toContain("建议");
    expect(wrapper.text()).toContain("待处理");
    expect(wrapper.text()).toContain("建议拆分森林生成任务");
    expect(wrapper.text()).toContain("拆分后本迭代可完成。");
  });

  it("缺省段落不渲染：无证据、无插槽时证据区与交互区不出现", () => {
    const wrapper = mountCard();

    expect(wrapper.find(".ai-evidence").exists()).toBe(false);
    expect(wrapper.find(".ai-card__interaction").exists()).toBe(false);
    expect(wrapper.find(".ai-card__footer").exists()).toBe(false);
  });

  it("证据区按 evidence 渲染并可展开来源", async () => {
    const wrapper = mountCard({ evidence: projectEvidence });
    const toggle = wrapper.find(".ai-evidence__toggle");
    expect(toggle.exists()).toBe(true);

    await toggle.trigger("click");
    expect(wrapper.findAll(".ai-evidence__source").length).toBeGreaterThan(0);
  });

  it("页脚动作点击 emit action key；提交中禁用", async () => {
    const actions: FooterAction[] = [
      { key: "accept", label: "接收", variant: "primary" },
      { key: "reject", label: "拒绝" }
    ];
    const wrapper = mountCard({ actions });

    const buttons = wrapper.findAll(".ai-card__footer button");
    expect(buttons.length).toBe(2);

    await buttons[0].trigger("click");
    expect(wrapper.emitted("action")?.[0]).toEqual(["accept"]);

    await wrapper.setProps({ submitting: true });
    expect(
      wrapper.findAll(".ai-card__footer button")[0].attributes("disabled")
    ).toBeDefined();
  });

  it("提交结果条成功时展示记录号与「不代替审批」提示", () => {
    const wrapper = mountCard({
      submissionPhase: "success",
      submissionResult: { recorded: true, reference: "INT-2041" }
    });

    const text = wrapper.find(".ai-submission").text();
    expect(text).toContain("INT-2041");
    expect(text).toContain("正式业务变更以审批结果为准");
  });
});
