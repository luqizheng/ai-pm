// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import InitializationCard from "./InitializationCard.vue";
import { i18n } from "@/lib/i18n";
import type { CommandCenterInitData } from "../command-center-data";

/**
 * 项目初始化卡（S01-01）行为测试：
 * `completion = not_determinable` 时**不得显示已完成**——无项目类型/初始化
 * 要求配置就无法判定，证书 §4.5 明确禁止宣称完成。
 */

const labels = {
  complete: "已完成",
  incomplete: "未完成",
  notDeterminable: "无法判定"
};

function data(
  overrides: Partial<CommandCenterInitData> = {}
): CommandCenterInitData {
  return {
    items: [
      {
        key: "repository",
        title: "代码仓库接入",
        statusLabel: "已完成",
        tone: "success",
        required: true,
        detail: ""
      },
      {
        key: "requirements",
        title: "需求录入",
        statusLabel: "待人工处理",
        tone: "warning",
        required: true,
        detail: ""
      }
    ],
    completion: "incomplete",
    requiredDone: 1,
    requiredTotal: 2,
    ...overrides
  };
}

function mountCard(payload: CommandCenterInitData | null) {
  return mount(InitializationCard, {
    props: {
      title: "项目初始化",
      progressLabel: `必需 ${payload?.requiredDone ?? 0} / ${payload?.requiredTotal ?? 0}`,
      completionLabels: labels,
      requiredLabel: "必需",
      data: payload
    },
    global: { plugins: [i18n] }
  });
}

describe("InitializationCard", () => {
  it("渲染必需事项完成数与逐项状态", () => {
    const wrapper = mountCard(data());

    expect(wrapper.find('[data-testid="cc-initialization"]').exists()).toBe(
      true
    );
    expect(wrapper.text()).toContain("必需 1 / 2");
    expect(wrapper.find('[data-testid="cc-init-repository"]').text()).toContain(
      "代码仓库接入"
    );
    expect(
      wrapper.find('[data-testid="cc-init-requirements"]').text()
    ).toContain("待人工处理");
  });

  it("not_determinable 时完成度显示「无法判定」，不误报已完成", () => {
    const wrapper = mountCard(
      data({
        completion: "not_determinable",
        requiredDone: 2,
        requiredTotal: 2
      })
    );

    // 完成度徽标（头部）必须是「无法判定」——逐项状态里的「已完成」不在此列。
    const head = wrapper.find(".init-card__head").text();
    expect(head).toContain("无法判定");
    expect(head).not.toContain("已完成");
  });

  it("无该块时整卡不渲染（不补空对象）", () => {
    const wrapper = mountCard(null);

    expect(wrapper.find('[data-testid="cc-initialization"]').exists()).toBe(
      false
    );
  });
});
