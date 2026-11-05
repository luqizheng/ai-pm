// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import KnowledgeSnapshotCard from "./KnowledgeSnapshotCard.vue";

const stats = [
  { label: "规范文档", count: 14 },
  { label: "架构决策", count: 9 },
  { label: "历史需求", count: 38 },
  { label: "代码索引", count: 6 }
];

describe("KnowledgeSnapshotCard", () => {
  it("渲染头部、入口与四类知识计数 pill", async () => {
    const wrapper = mount(KnowledgeSnapshotCard, {
      props: {
        title: "知识快照",
        meta: "只读摘要",
        actionLabel: "打开 Knowledge",
        stats,
        footnote: "知识库最近一次同步于今日 08:20，覆盖支付与订单两个域。"
      }
    });

    expect(wrapper.find("h3").text()).toBe("知识快照");
    expect(wrapper.text()).toContain("只读摘要");
    expect(wrapper.findAll(".pm-knowledge__pill")).toHaveLength(4);
    expect(wrapper.text()).toContain("38");

    await wrapper.find(".pm-section-head__action").trigger("click");
    expect(wrapper.emitted("action")).toHaveLength(1);
  });

  it("footnote 缺省不渲染脚注", () => {
    const wrapper = mount(KnowledgeSnapshotCard, {
      props: { title: "知识快照", actionLabel: "打开 Knowledge", stats }
    });

    expect(wrapper.find(".pm-knowledge__footnote").exists()).toBe(false);
  });
});
