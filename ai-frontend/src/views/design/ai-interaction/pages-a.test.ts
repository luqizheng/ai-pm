// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, type VueWrapper } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { nextTick, type Component } from "vue";

import { mountShell } from "@/lib/test-utils";

/** MarkdownViewer 依赖 Vditor：同 responses-a.test.ts，整体替换成 md2html 桩 */
const md2html = vi.hoisted(() => vi.fn());

vi.mock("vditor", () => ({
  default: {
    md2html,
    preview: vi.fn(),
    version: "test"
  }
}));

import AiInteractionBaseView from "./AiInteractionBaseView.vue";
import AiInteractionInformationalView from "./AiInteractionInformationalView.vue";
import AiInteractionProposalView from "./AiInteractionProposalView.vue";
import AiInteractionRecommendationView from "./AiInteractionRecommendationView.vue";

/**
 * 目录页 A 组契约（交付帧 01–07）：
 *   - 每页可挂载、真实渲染响应卡（不是静态贴图）；
 *   - Base：全段位卡 + 折叠切换生效；
 *   - Informational：无任何审批按钮，Copy / Ask follow-up 有可见反馈；
 *   - Recommendation：三态齐备，Pending 真实接收后出现提交结果条；
 *   - Proposal：接收提案与提交正式审批是两条独立结果。
 */
const mounted: Array<{ unmount: () => void }> = [];

function mountPage(view: Component): VueWrapper {
  const router = createRouter({
    history: createMemoryHistory(),
    // 目录页只依赖 RouterLink 解析，不渲染 RouterView，用空路由降低挂载成本
    routes: [{ path: "/:pathMatch(.*)*", component: { render: () => null } }]
  });
  const wrapper = mountShell(view, {
    global: { plugins: [router] }
  }) as unknown as VueWrapper;
  mounted.push(wrapper);
  return wrapper;
}

async function flush() {
  await flushPromises();
  for (let index = 0; index < 3; index += 1) await nextTick();
  await flushPromises();
}

/** 轮询等待异步提交结果（页面适配器有 300ms 延迟） */
async function waitFor(
  wrapper: VueWrapper,
  predicate: () => boolean,
  message: string,
  timeout = 5000
) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    await flush();
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 25));
  }
  throw new Error(`waitFor timeout: ${message}`);
}

function buttons(wrapper: VueWrapper) {
  return wrapper.findAll("button");
}

function buttonByText(wrapper: VueWrapper, label: string) {
  return buttons(wrapper).find(button => button.text().trim() === label);
}

function footerButton(wrapper: VueWrapper, label: string) {
  return wrapper
    .findAll(".ai-card__footer button")
    .find(button => button.text().trim() === label);
}

beforeEach(() => {
  md2html.mockResolvedValue("<p>正文</p>");
});

afterEach(() => {
  while (mounted.length) mounted.pop()?.unmount();
  md2html.mockReset();
  document.body.innerHTML = "";
});

describe("帧 01 — Base 页（卡片基座）", () => {
  it("渲染全段位卡、可选段卡、三档密度与折叠卡，折叠可展开/收起", async () => {
    const wrapper = mountPage(AiInteractionBaseView);
    await flush();

    // 全段位卡 + 可选段卡 + 3 张密度卡 + 折叠卡
    expect(wrapper.findAll(".ai-card").length).toBeGreaterThanOrEqual(6);
    // 六段位标注齐备
    expect(wrapper.findAll(".base-page__segments > li")).toHaveLength(6);
    expect(wrapper.find(".base-page__segments").text()).toContain("Evidence");
    // 证据段默认展开（全段位卡包含证据内容）
    expect(wrapper.find(".ai-evidence__body").exists()).toBe(true);

    // 折叠卡：默认折叠 → 内容被裁剪；点击后展开，再点收起
    const toggle = wrapper.find(".ai-card__collapse");
    expect(toggle.exists()).toBe(true);
    expect(wrapper.findAll(".ai-card__content-body.is-limited").length).toBe(1);
    expect(toggle.attributes("aria-expanded")).toBe("false");

    await toggle.trigger("click");
    await flush();
    expect(wrapper.find(".ai-card__collapse").attributes("aria-expanded")).toBe(
      "true"
    );
    expect(wrapper.findAll(".ai-card__content-body.is-limited").length).toBe(0);

    await wrapper.find(".ai-card__collapse").trigger("click");
    await flush();
    expect(wrapper.findAll(".ai-card__content-body.is-limited").length).toBe(1);
  });
});

describe("帧 02 — Informational 页（信息卡）", () => {
  it("无任何审批按钮；Copy 出现已复制，Ask follow-up 出现页面反馈", async () => {
    const wrapper = mountPage(AiInteractionInformationalView);
    await flush();

    expect(wrapper.findAll(".ai-card").length).toBe(2);

    // 信息卡不出现接收 / 拒绝 / 审批类动作
    for (const forbidden of ["接收", "拒绝", "审批"]) {
      const hit = buttons(wrapper).find(button =>
        button.text().includes(forbidden)
      );
      expect(hit, `unexpected action: ${forbidden}`).toBeFalsy();
    }

    // Copy → 卡片内「已复制」反馈
    const copy = buttonByText(wrapper, "复制内容");
    expect(copy).toBeTruthy();
    await copy!.trigger("click");
    await flush();
    expect(buttonByText(wrapper, "已复制")).toBeTruthy();

    // Ask follow-up → 页面级可见反馈
    expect(wrapper.find(".info-page__feedback").exists()).toBe(false);
    const followUp = buttonByText(wrapper, "继续追问");
    expect(followUp).toBeTruthy();
    await followUp!.trigger("click");
    await flush();

    const feedback = wrapper.find(".info-page__feedback");
    expect(feedback.exists()).toBe(true);
    expect(feedback.text()).toContain("ask-follow-up");

    // 有来源的卡渲染来源段；无来源样例不渲染（证据区按需缺省）
    expect(wrapper.findAll(".ai-info__sources")).toHaveLength(1);
  });
});

describe("帧 03/04/05 — Recommendation 页（建议三态）", () => {
  it("三态齐备，Pending 真实接收后出现提交结果条与记录号", async () => {
    const wrapper = mountPage(AiInteractionRecommendationView);
    await flush();

    expect(wrapper.findAll(".ai-card")).toHaveLength(3);
    const text = wrapper.text();
    expect(text).toContain("待处理");
    expect(text).toContain("已接收");
    expect(text).toContain("已拒绝");
    // 只读记录条与拒因
    expect(text).toContain("REC-2041");

    // 接收前没有提交结果条
    expect(wrapper.findAll(".ai-submission").length).toBe(0);

    const accept = footerButton(wrapper, "接收");
    expect(accept).toBeTruthy();
    await accept!.trigger("click");

    await waitFor(
      wrapper,
      () => /INT-\d+/.test(wrapper.text()),
      "accept submission result"
    );

    expect(wrapper.findAll(".ai-submission.is-success").length).toBe(1);
    expect(wrapper.text()).toContain("已记录你的选择");
    // 已接收即终态：动作按钮消失（只读摘要）
    expect(wrapper.findAll(".ai-card__footer").length).toBe(0);
  });

  it("Pending 的拒绝流程展开原因输入并保留原因", async () => {
    const wrapper = mountPage(AiInteractionRecommendationView);
    await flush();

    const reject = footerButton(wrapper, "拒绝");
    expect(reject).toBeTruthy();
    await reject!.trigger("click");
    await flush();

    const textarea = wrapper.find(".ai-reco__reject textarea");
    expect(textarea.exists()).toBe(true);
    await textarea.setValue("范围评估不足，先解冻接口");

    const confirmReject = buttonByText(wrapper, "确认拒绝");
    expect(confirmReject).toBeTruthy();
    await confirmReject!.trigger("click");

    // Pending 卡提交后出现结果条。Rejected 帧本身是 initialStatus 种下的只读态，
    // 所以用「提交结果条」而不是已拒绝文案作为真实提交完成的判据。
    await waitFor(
      wrapper,
      () => wrapper.findAll(".ai-submission.is-success").length === 1,
      "reject submission result"
    );

    const text = wrapper.text();
    expect(text).toContain("拒绝原因");
    expect(text).toContain("范围评估不足，先解冻接口");
    expect(wrapper.find(".ai-reco__summary.is-rejected").exists()).toBe(true);
  });
});

describe("帧 06/07 — Proposal 页（提案接收与正式审批分离）", () => {
  it("Review 卡可接收；Edit & Compare 展示对比与两条独立结果", async () => {
    const wrapper = mountPage(AiInteractionProposalView);
    await flush();

    expect(wrapper.find(".ai-card").exists()).toBe(true);
    const text = wrapper.text();
    expect(text).toContain("变更前");
    expect(text).toContain("变更后");
    expect(text).toContain("terrain-render-pipeline.md");
    // 两条独立结果各自带记录号
    expect(text).toContain("PROP-2041");
    expect(text).toContain("APPR-2042");
    // 帧 07 的两条结果条初始就存在，卡片内的审批结果条尚未出现
    expect(wrapper.findAll(".ai-submission").length).toBe(2);
    expect(wrapper.find(".ai-proposal__approval").exists()).toBe(false);

    // 1) 接收提案
    const accept = footerButton(wrapper, "接收提案");
    expect(accept).toBeTruthy();
    await accept!.trigger("click");
    await waitFor(
      wrapper,
      () => /INT-\d+/.test(wrapper.text()),
      "proposal accepted result"
    );
    expect(wrapper.find(".ai-proposal__result.is-accepted").exists()).toBe(
      true
    );

    // 2) 提交正式审批（独立结果，需二次确认）
    const submitApproval = footerButton(wrapper, "提交正式审批");
    expect(submitApproval).toBeTruthy();
    await submitApproval!.trigger("click");
    await flush();

    const confirm = document.querySelector(
      ".ui-confirm__btn--primary"
    ) as HTMLElement | null;
    expect(confirm).toBeTruthy();
    confirm!.click();

    await waitFor(
      wrapper,
      () => wrapper.text().includes("INT-2042"),
      "approval submitted result"
    );
    await waitFor(
      wrapper,
      () =>
        wrapper
          .find(".ai-proposal__approval .ai-submission.is-success")
          .exists(),
      "approval status strip"
    );

    const finalText = wrapper.text();
    expect(finalText).toContain("已接收提案");
    expect(finalText).toContain("已提交至正式审批流程");
    expect(finalText).not.toContain("已批准");
  });
});
