// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, type VueWrapper } from "@vue/test-utils";
import { nextTick } from "vue";

import { mountShell } from "@/lib/test-utils";

/** MarkdownViewer 依赖 Vditor：按 MarkdownViewer.test.ts 的做法整体替换 */
const md2html = vi.hoisted(() => vi.fn());

vi.mock("vditor", () => ({
  default: {
    md2html,
    preview: vi.fn(),
    version: "test"
  }
}));

import ArtifactResponse from "./ArtifactResponse.vue";
import InformationalResponse from "./InformationalResponse.vue";
import ProposalResponse from "./ProposalResponse.vue";
import RecommendationResponse from "./RecommendationResponse.vue";
import {
  aiIdentity,
  artifactFixture,
  informationalFixture,
  proposalFixture,
  recommendationFixture
} from "../fixtures";
import type { SubmitResult } from "../types";

const submitResult: SubmitResult = {
  recorded: true,
  reference: "INT-2041",
  messageKey: "ai.card.submission.recorded"
};

const submit = () => Promise.resolve(submitResult);

const wrappers: Array<{ unmount: () => void }> = [];

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  md2html.mockReset();
  document.body.innerHTML = "";
});

beforeEach(() => {
  md2html.mockResolvedValue("<p>正文</p>");
});

async function flush() {
  await flushPromises();
  for (let index = 0; index < 3; index += 1) await nextTick();
  await flushPromises();
}

function mountA(component: unknown, payload: unknown) {
  const wrapper = mountShell(component as never, {
    props: {
      payload,
      identity: aiIdentity,
      timestamp: "10:01",
      submit
    }
  });
  wrappers.push(wrapper);
  return wrapper as VueWrapper;
}

function footerButtons(wrapper: VueWrapper) {
  return wrapper.findAll(".ai-card__footer button");
}

function footerButton(wrapper: VueWrapper, label: string) {
  return footerButtons(wrapper).find(button => button.text().includes(label));
}

function anyButton(wrapper: VueWrapper, label: string) {
  return wrapper
    .findAll("button")
    .find(button => button.text().includes(label));
}

describe("InformationalResponse", () => {
  it("只读展示正文与来源，Copy 触发复制反馈，不含审批类按钮", async () => {
    md2html.mockResolvedValue("<p>迭代风险摘要</p>");
    const wrapper = mountA(InformationalResponse, informationalFixture);
    await flush();

    expect(wrapper.find(".md-viewer__body").exists()).toBe(true);
    expect(wrapper.text()).toContain("需求文档：世界地图渲染 v2.0");

    const labels = footerButtons(wrapper).map(button => button.text());
    expect(labels.some(label => label.includes("接收"))).toBe(false);
    expect(labels.some(label => label.includes("拒绝"))).toBe(false);
    expect(labels.some(label => label.includes("审批"))).toBe(false);

    const copy = footerButton(wrapper, "复制内容");
    expect(copy).toBeTruthy();
    await copy!.trigger("click");
    await flush();

    expect(wrapper.emitted("action")?.[0]).toEqual(["copy"]);
    expect(footerButton(wrapper, "已复制")).toBeTruthy();
  });

  it("Ask follow-up 只上报 action，不在卡片内改变状态", async () => {
    const wrapper = mountA(InformationalResponse, informationalFixture);
    await flush();

    await footerButton(wrapper, "继续追问")!.trigger("click");
    await flush();

    expect(wrapper.emitted("action")?.[0]).toEqual(["ask-follow-up"]);
  });
});

describe("RecommendationResponse", () => {
  it("Accept 后出现结果条与记录号，动作按钮消失，文案不含「生效」", async () => {
    const wrapper = mountA(RecommendationResponse, recommendationFixture);
    await flush();

    await footerButton(wrapper, "接收")!.trigger("click");
    await flush();

    const text = wrapper.text();
    expect(text).toContain("INT-2041");
    expect(text).toContain("已记录你的选择");
    expect(text).toContain("已接收");
    expect(text).toContain(recommendationFixture.summary);
    expect(text).not.toContain("生效");
    expect(wrapper.find(".ai-card__footer").exists()).toBe(false);
    expect(wrapper.emitted("submitted")).toHaveLength(1);
  });

  it("Reject 展开原因输入，确认后进入 rejected 并保留原因", async () => {
    const wrapper = mountA(RecommendationResponse, recommendationFixture);
    await flush();

    await footerButton(wrapper, "拒绝")!.trigger("click");
    await flush();

    const textarea = wrapper.find("textarea");
    expect(textarea.exists()).toBe(true);
    await textarea.setValue("范围评估不足");
    await anyButton(wrapper, "确认拒绝")!.trigger("click");
    await flush();

    const text = wrapper.text();
    expect(text).toContain("已拒绝");
    expect(text).toContain("范围评估不足");
    expect(wrapper.find(".ai-card__footer").exists()).toBe(false);
    expect(wrapper.emitted("submitted")).toHaveLength(1);
  });

  it("Modify / Discuss 只上报 action", async () => {
    const wrapper = mountA(RecommendationResponse, recommendationFixture);
    await flush();

    await footerButton(wrapper, "调整")!.trigger("click");
    await footerButton(wrapper, "继续讨论")!.trigger("click");
    await flush();

    expect(wrapper.emitted("action")).toEqual([["modify"], ["discuss"]]);
  });
});

describe("ProposalResponse", () => {
  it("接收提案与提交正式审批是两条独立结果，且不出现「已批准」", async () => {
    const wrapper = mountA(ProposalResponse, proposalFixture);
    await flush();

    expect(wrapper.text()).toContain("变更前");
    expect(wrapper.text()).toContain("变更后");
    expect(wrapper.text()).toContain("影响");

    await footerButton(wrapper, "接收提案")!.trigger("click");
    await flush();

    expect(wrapper.text()).toContain("已接收提案");
    expect(wrapper.text()).toContain("INT-2041");

    // 接收 ≠ 审批：接收后仍可独立提交正式审批
    const submitApproval = footerButton(wrapper, "提交正式审批");
    expect(submitApproval).toBeTruthy();
    await submitApproval!.trigger("click");
    await flush();

    const dialogConfirm = document.querySelector(
      ".ui-confirm__btn--primary"
    ) as HTMLElement | null;
    expect(dialogConfirm).toBeTruthy();
    dialogConfirm!.click();
    await flush();

    const text = wrapper.text();
    expect(text).toContain("已提交至正式审批流程");
    expect(text).toContain("已接收提案");
    expect(text).not.toContain("已批准");
    expect(wrapper.emitted("submitted")).toHaveLength(2);
  });

  it("Review 折叠详情，Edit 用本地草稿保存", async () => {
    const wrapper = mountA(ProposalResponse, proposalFixture);
    await flush();

    await footerButton(wrapper, "收起详情")!.trigger("click");
    await flush();
    expect(wrapper.text()).not.toContain("地形数据来源");

    await footerButton(wrapper, "展开详情")!.trigger("click");
    await flush();
    expect(wrapper.text()).toContain("地形数据来源");

    await footerButton(wrapper, "编辑")!.trigger("click");
    await flush();
    const textareas = wrapper.findAll("textarea");
    expect(textareas.length).toBeGreaterThan(0);
    await textareas[0].setValue("任务内直接调用 TerrainSnapshot.sample()");
    await anyButton(wrapper, "保存草稿")!.trigger("click");
    await flush();

    expect(wrapper.text()).toContain("任务内直接调用 TerrainSnapshot.sample()");
    expect(wrapper.text()).toContain("已编辑");
  });
});

describe("ArtifactResponse", () => {
  it("diff 切换展示增删行，编辑保存后更新本地正文并标记已编辑", async () => {
    const wrapper = mountA(ArtifactResponse, artifactFixture);
    await flush();

    const diffButton = wrapper
      .findAll(".ai-artifact__view-btn")
      .find(button => button.text().includes("版本差异"));
    expect(diffButton).toBeTruthy();
    await diffButton!.trigger("click");

    expect(wrapper.find(".ai-artifact__diff").exists()).toBe(true);
    expect(
      wrapper.findAll(".ai-artifact__diff-line.is-add").length
    ).toBeGreaterThan(0);
    expect(
      wrapper.findAll(".ai-artifact__diff-line.is-remove").length
    ).toBeGreaterThan(0);

    const previewButton = wrapper
      .findAll(".ai-artifact__view-btn")
      .find(button => button.text().includes("预览"));
    await previewButton!.trigger("click");
    expect(wrapper.find(".md-viewer").exists()).toBe(true);

    await footerButton(wrapper, "编辑")!.trigger("click");
    await flush();

    const textarea = wrapper.find(".ai-artifact__editor textarea");
    expect(textarea.exists()).toBe(true);
    await textarea.setValue("# 更新后的正文");
    await anyButton(wrapper, "保存")!.trigger("click");
    await flush();

    expect(wrapper.text()).toContain("已编辑");
    expect(md2html).toHaveBeenCalledWith("# 更新后的正文", expect.anything());
  });

  it("提交至既有业务流程需二次确认，结果只写已提交至该流程", async () => {
    const wrapper = mountA(ArtifactResponse, artifactFixture);
    await flush();

    await footerButton(wrapper, "需求评审")!.trigger("click");
    await flush();

    const dialogConfirm = document.querySelector(
      ".ui-confirm__btn--primary"
    ) as HTMLElement | null;
    expect(dialogConfirm).toBeTruthy();
    dialogConfirm!.click();
    await flush();

    const text = wrapper.text();
    expect(text).toContain("已提交至需求评审");
    expect(text).toContain("INT-2041");
    expect(wrapper.find(".ai-card__footer").exists()).toBe(false);
  });
});
