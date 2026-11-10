// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import GoalCard from "./GoalCard.vue";
import { i18n } from "@/lib/i18n";
import type { ProjectGoalView } from "@/api/goal";

/**
 * 项目目标卡（S01-I4）行为测试：
 * 三态必须可辨——**「没有目标」与「有目标但正文读不出来」绝不可混为一谈**。
 */

const labels = {
  revision: "修订",
  publishedAt: "发布时间",
  commit: "提交"
};

const states = {
  missingTitle: "尚未建立项目目标",
  missingHint: "系统不会臆测目标。",
  bodyUnavailableTitle: "目标正文当前不可读",
  bodyUnavailableHint: "这不是「没有目标」。"
};

const badges = {
  ready: "可用",
  missing: "未建立",
  bodyUnavailable: "正文不可读"
};

function mountCard(data: ProjectGoalView | null) {
  return mount(GoalCard, {
    props: { title: "项目目标", labels, states, badges, data },
    global: { plugins: [i18n] }
  });
}

function goal(overrides: Partial<ProjectGoalView> = {}): ProjectGoalView {
  return {
    state: "ready",
    metadata: {
      document_id: "doc-1",
      revision_id: "rev-1",
      revision_no: 3,
      content_hash: "sha256-abcdef",
      git_commit_sha: "abcdef1234567890",
      change_summary: "记录了范围调整",
      published_at: "2026-11-10T09:00:00Z"
    },
    markdown: "# Project Goal\n\n## 1. 项目背景\n提升研发交付效率。",
    bodyError: "",
    ...overrides
  };
}

describe("GoalCard", () => {
  it("ready 时渲染元数据与正文", () => {
    const wrapper = mountCard(goal());

    expect(wrapper.find('[data-testid="cc-goal-badge"]').text()).toBe("可用");
    expect(wrapper.find('[data-testid="cc-goal-markdown"]').text()).toContain(
      "提升研发交付效率"
    );
    expect(wrapper.text()).toContain("3");
    expect(wrapper.text()).toContain("abcdef1234");
  });

  it("missing 时显示未建立，且不显示正文区", () => {
    const wrapper = mountCard(
      goal({ state: "missing", metadata: null, markdown: "" })
    );

    expect(wrapper.find('[data-testid="cc-goal-missing"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="cc-goal-markdown"]').exists()).toBe(
      false
    );
    expect(wrapper.find('[data-testid="cc-goal-badge"]').text()).toBe("未建立");
  });

  it("body_unavailable 与 missing 严格区分，并保留元数据", () => {
    const wrapper = mountCard(
      goal({
        state: "body_unavailable",
        markdown: "",
        bodyError: "Io: missing"
      })
    );

    expect(
      wrapper.find('[data-testid="cc-goal-body-unavailable"]').exists()
    ).toBe(true);
    expect(wrapper.find('[data-testid="cc-goal-missing"]').exists()).toBe(
      false
    );
    expect(wrapper.find('[data-testid="cc-goal-badge"]').text()).toBe(
      "正文不可读"
    );
    // 元数据仍可见（目标确实存在）
    expect(wrapper.text()).toContain("abcdef1234");
  });
});
