// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import AttentionPanel from "./AttentionPanel.vue";
import type { AttentionItem } from "./attention";

const columnLabels = {
  item: "事项",
  category: "类别",
  object: "业务对象",
  status: "状态",
  owner: "责任",
  goto: "直达"
};

/** 行样例为按列头规格的示意（设计稿未提供行样例） */
const items: AttentionItem[] = [
  {
    id: "attn-1",
    title: "灰度发布策略修订待确认",
    category: "变更审阅",
    object: "REQ-118",
    statusLabel: "待人工确认",
    statusTone: "warning",
    owner: "林悦"
  },
  {
    id: "attn-2",
    title: "支付回调签名校验被依赖阻塞",
    category: "执行阻塞",
    object: "T-241",
    statusLabel: "阻塞",
    statusTone: "danger",
    owner: "陈昊"
  }
];

function mountPanel(overrides: Record<string, unknown> = {}) {
  return mount(AttentionPanel, {
    props: {
      title: "需关注事项",
      pendingLabel: "3 项待处理",
      counts: [
        { label: "紧急", count: 1, tone: "danger" },
        { label: "告知", count: 2, tone: "warning" }
      ],
      actionLabel: "前往 Inbox 处理全部",
      columnLabels,
      items,
      ...overrides
    }
  });
}

describe("AttentionPanel", () => {
  it("渲染头部：警示图标、标题、待处理 chip、计数与 Inbox 入口", async () => {
    const wrapper = mountPanel();

    expect(wrapper.find("h3").text()).toBe("需关注事项");
    expect(wrapper.find(".pm-attention__icon").exists()).toBe(true);
    expect(wrapper.find(".pm-attention__title-group .ui-badge--warning").text()).toContain(
      "3 项待处理"
    );
    expect(wrapper.text()).toContain("紧急");
    expect(wrapper.text()).toContain("1");
    expect(wrapper.text()).toContain("2");

    await wrapper.find(".pm-attention__inbox").trigger("click");
    expect(wrapper.emitted("inbox")).toHaveLength(1);
  });

  it("渲染六列列头带", () => {
    const wrapper = mountPanel();

    const heads = wrapper.findAll(".pm-attention__col-head");
    expect(heads).toHaveLength(6);
    expect(wrapper.text()).toContain("事项");
    expect(wrapper.text()).toContain("直达");
  });

  it("渲染事项行：各单元格 + 语义点，直达按钮带出整条事项", async () => {
    const wrapper = mountPanel();

    const rows = wrapper.findAll("ul.pm-attention__items .pm-attention__row");
    expect(rows).toHaveLength(2);
    expect(wrapper.text()).toContain("灰度发布策略修订待确认");
    expect(wrapper.text()).toContain("REQ-118");
    expect(wrapper.text()).toContain("林悦");
    expect(wrapper.findAll('[role="status"]')).toHaveLength(2);
    expect(wrapper.find(".ui-status--danger").exists()).toBe(true);

    await rows[1].find(".pm-attention__open").trigger("click");
    expect(wrapper.emitted("open")?.[0]?.[0]).toMatchObject({ id: "attn-2" });
  });

  it("items 为空时面板只剩壳与列头（设计稿默认态）", () => {
    const wrapper = mountPanel({ items: [] });

    expect(wrapper.findAll("ul.pm-attention__items .pm-attention__row")).toHaveLength(0);
    expect(wrapper.findAll('[role="status"]')).toHaveLength(0);
  });
});
