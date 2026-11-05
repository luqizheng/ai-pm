// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { nextTick, type Component } from "vue";

import { i18n } from "@/lib/i18n";
import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";

/** ArtifactResponse 的 MarkdownViewer 依赖 Vditor：整体替换成 md2html 桩 */
const md2html = vi.hoisted(() => vi.fn());

vi.mock("vditor", () => ({
  default: {
    md2html,
    preview: vi.fn(),
    version: "test"
  }
}));

import AiInteractionQuestionView from "./AiInteractionQuestionView.vue";
import AiInteractionDecisionView from "./AiInteractionDecisionView.vue";
import AiInteractionArtifactView from "./AiInteractionArtifactView.vue";
import AiInteractionActionView from "./AiInteractionActionView.vue";
import AiInteractionExecutionView from "./AiInteractionExecutionView.vue";

/**
 * 目录页 C 组契约（交付帧 08–16）：
 *   - Question：五题型同卡真实可填、空必填提交出现校验错误、已提交帧渲染只读汇总；
 *   - Decision：AI 推荐 / 你的选择两条通道可辨，正式提交走二次确认，帧 12 记录只读；
 *   - Artifact：版本 + 预览 / 差异切换 + 编辑保存后出现「已编辑」；
 *   - Action：确认弹层真实出现，确认后只写「已提交执行」；低风险卡可取消；
 *   - Execution：partial / failed 两态，failed 的 Retry 真实可用。
 */
const mounted: Array<{ unmount: () => void }> = [];

const t = i18n.global.t;

async function flush(): Promise<void> {
  await flushPromises();
  for (let index = 0; index < 3; index += 1) await nextTick();
  await flushPromises();
}

async function waitFor(
  predicate: () => boolean,
  message: string,
  timeout = 5000
): Promise<void> {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    await flush();
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 25));
  }
  throw new Error(`waitFor timeout: ${message}`);
}

async function mountPage(view: Component) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/design");
  await router.isReady();

  const wrapper = mountShell(view, { global: { plugins: [router] } });
  mounted.push(wrapper as unknown as { unmount: () => void });
  await flush();
  return wrapper;
}

function footerButton(
  card: { findAll: (selector: string) => any[] },
  label: string
) {
  return card
    .findAll(".ai-card__footer button")
    .find((button: { text: () => string }) => button.text().trim() === label);
}

function bodyButton(selector: string): HTMLElement | null {
  return document.querySelector<HTMLElement>(selector);
}

beforeEach(() => {
  md2html.mockResolvedValue("<p>body</p>");
});

afterEach(() => {
  while (mounted.length) mounted.pop()?.unmount();
  md2html.mockReset();
  document.body.innerHTML = "";
});

describe("帧 08/09/10 — Question 页", () => {
  it("多题一卡可交互；空必填提交出现校验错误；已提交帧渲染只读答案汇总", async () => {
    const wrapper = await mountPage(AiInteractionQuestionView);
    const cards = wrapper.findAll(".ai-card");
    expect(cards).toHaveLength(3);

    // 08：五种题型同卡（单选 / 多选 / 数字 / 日期 / 多行）
    const pending = cards[0];
    expect(pending.findAll(".ai-question__item")).toHaveLength(5);
    expect(
      pending.findAll('input[type="radio"]').length
    ).toBeGreaterThanOrEqual(3);
    expect(pending.findAll(".ui-checkbox__root").length).toBeGreaterThanOrEqual(
      4
    );
    expect(pending.text()).toContain("已答 0 / 5");

    // 单选真实可点：进度计数随答案更新
    await pending.find('input[type="radio"]').setValue(true);
    await flush();
    expect(pending.text()).toContain("已答 1 / 5");

    // 09：空必填点提交 → 三题必填全部标红，且不进入提交中
    const validating = cards[1];
    const submit = footerButton(validating, t("ai.question.actions.submit"));
    expect(submit).toBeTruthy();
    await submit!.trigger("click");
    await flush();
    expect(validating.findAll(".ai-question__item.is-invalid")).toHaveLength(3);
    expect(validating.findAll(".ai-question__error").length).toBeGreaterThan(0);
    expect(validating.find(".ai-submission").exists()).toBe(false);

    // 10：挂载后重放一次真实提交 → 只读汇总（无表单、无页脚动作）
    await waitFor(
      () =>
        wrapper.findAll(".ai-card")[2].find(".ai-question__summary").exists(),
      "submitted read-only summary"
    );
    const submitted = wrapper.findAll(".ai-card")[2];
    expect(submitted.text()).toContain(t("ai.question.summary.title"));
    expect(submitted.text()).toContain("基础分布");
    expect(submitted.text()).toContain("温带森林");
    expect(submitted.findAll(".ai-question__item")).toHaveLength(0);
    expect(submitted.find(".ai-card__footer").exists()).toBe(false);
    expect(submitted.text()).toMatch(/INT-\d+/);
  });
});

describe("帧 11/12 — Decision 页", () => {
  it("AI 推荐与你的选择可辨；提交经二次确认；已记录帧展示决策人 / 记录号", async () => {
    const wrapper = await mountPage(AiInteractionDecisionView);

    // 帧 12 挂载时重放一次真实决策，先等它落定，保证弹层断言不被抢
    await waitFor(
      () =>
        wrapper.findAll(".ai-card")[1].find(".ai-decision__record").exists(),
      "recorded decision"
    );

    const pending = wrapper.findAll(".ai-card")[0];
    expect(pending.findAll(".ai-decision__option")).toHaveLength(3);
    expect(pending.text()).toContain(t("ai.decision.recommended"));
    expect(pending.text()).not.toContain(t("ai.decision.selected"));

    // 选第三项 → 出现「你的选择」通道徽标
    await pending.findAll(".ai-decision__option")[2].trigger("click");
    await flush();
    expect(pending.text()).toContain(t("ai.decision.selected"));

    // 勾选以本人身份提交 → 点正式提交 → 二次确认弹层出现在 body
    await pending
      .find(".ai-decision__confirm .ui-checkbox__root")
      .trigger("click");
    await flush();
    const submit = footerButton(pending, t("ai.decision.actions.submit"));
    expect(submit).toBeTruthy();
    await submit!.trigger("click");
    await flush();
    expect(bodyButton(".ui-confirm")).not.toBeNull();

    // 取消弹层：卡片仍在待决策态
    bodyButton(".ui-confirm__btn--cancel")!.click();
    await flush();
    expect(bodyButton(".ui-confirm")).toBeNull();

    // 帧 12：只读记录（决策人 = 人类 + 决策时间 + 记录号）
    const recorded = wrapper.findAll(".ai-card")[1];
    const recordText = recorded.text();
    expect(recordText).toContain(t("ai.decision.record.title"));
    expect(recordText).toContain(t("ai.decision.record.decidedByHuman"));
    expect(recordText).toContain("决策时间");
    expect(recordText).toMatch(/INT-\d+/);
    expect(recorded.findAll(".ai-decision__option")).toHaveLength(0);
  });
});

describe("帧 13 — Artifact 页", () => {
  it("版本信息与差异切换可交互；编辑保存后卡片出现「已编辑」", async () => {
    const wrapper = await mountPage(AiInteractionArtifactView);
    const cards = wrapper.findAll(".ai-card");
    expect(cards).toHaveLength(2);

    const card = cards[0];
    expect(card.text()).toContain(t("ai.artifact.type.requirementDoc"));
    expect(card.text()).toContain("v1.4");
    expect(card.text()).toContain("AI-PM");

    // 预览 / 差异切换
    const viewButtons = card.findAll(".ai-artifact__view-btn");
    expect(viewButtons).toHaveLength(2);
    await viewButtons[1].trigger("click");
    await flush();
    expect(card.text()).toContain("requirements/REQ-128-world-map-render.md");
    await viewButtons[0].trigger("click");
    await flush();
    expect(card.find(".ai-artifact__preview").exists()).toBe(true);

    // 编辑入口 → 编辑器 → 保存 → 已编辑徽标
    const edit = footerButton(card, t("ai.artifact.actions.edit"));
    expect(edit).toBeTruthy();
    await edit!.trigger("click");
    await flush();
    const editor = card.find(".ai-artifact__editor textarea");
    expect(editor.exists()).toBe(true);
    await editor.setValue("# World map render v2.0");
    const save = card
      .findAll(".ai-artifact__edit-actions button")
      .find((button: { text: () => string }) =>
        button.text().includes(t("ai.artifact.save"))
      );
    expect(save).toBeTruthy();
    await save!.trigger("click");
    await flush();
    expect(card.text()).toContain(t("ai.artifact.edited"));
    expect(card.find(".ai-artifact__editor").exists()).toBe(false);
  });
});

describe("帧 14 — Action 页", () => {
  it("高后果卡的确认弹层真实出现并在确认后提交；低风险卡可取消且不产生变更", async () => {
    const wrapper = await mountPage(AiInteractionActionView);
    const cards = wrapper.findAll(".ai-card");
    expect(cards).toHaveLength(2);

    const high = cards[0];
    expect(high.text()).toContain(t("ai.actionRequest.risk.medium"));

    const confirm = footerButton(high, t("ai.actionRequest.actions.confirm"));
    expect(confirm).toBeTruthy();
    await confirm!.trigger("click");
    await flush();

    const dialog = bodyButton(".ui-confirm");
    expect(dialog).not.toBeNull();
    expect(dialog!.textContent).toContain(t("ai.actionRequest.dialog.title"));
    // 弹层打开时尚未提交
    expect(high.find(".ai-submission").exists()).toBe(false);

    bodyButton(".ui-confirm__btn--primary")!.click();
    await waitFor(
      () => high.text().includes(t("ai.actionRequest.successNote")),
      "action request submitted"
    );
    expect(high.text()).toContain(t("ai.actionRequest.success"));

    // 低风险变体：确认不弹层；取消只记录「请求保持待处理」
    const low = cards[1];
    expect(low.text()).toContain(t("ai.actionRequest.risk.low"));
    const cancel = footerButton(low, t("ai.actionRequest.actions.cancel"));
    expect(cancel).toBeTruthy();
    await cancel!.trigger("click");
    await flush();
    const notice = wrapper.find(".action-page__notice");
    expect(notice.exists()).toBe(true);
    expect(notice.text()).toContain("Cancel pressed");
    expect(document.body.querySelector(".ui-confirm")).toBeNull();
  });
});

describe("帧 15/16 — Execution 页", () => {
  it("partial / failed 两态齐备；failed 的 Retry 真实可用并出现重试记录", async () => {
    const wrapper = await mountPage(AiInteractionExecutionView);
    const cards = wrapper.findAll(".ai-card");
    expect(cards).toHaveLength(2);

    // 15：部分成功
    const partial = cards[0];
    expect(partial.attributes("data-type")).toBe("execution-result");
    expect(partial.text()).toContain(t("ai.executionResult.state.partial"));
    expect(partial.text()).toContain("src/forest/generator.ts");
    expect(partial.text()).toContain(t("ai.executionResult.warnings.title"));

    // 16：失败 + 可重试
    const failed = cards[1];
    expect(failed.text()).toContain(t("ai.executionResult.state.failed"));
    expect(failed.text()).toContain("SNAPSHOT_TIMEOUT");
    expect(failed.text()).toContain(t("ai.executionResult.failure.retryable"));

    const retry = footerButton(failed, t("ai.executionResult.actions.retry"));
    expect(retry).toBeTruthy();
    await retry!.trigger("click");
    await waitFor(
      () => failed.find(".ai-execution__retry-note").exists(),
      "retry note"
    );
    expect(failed.text()).toContain(t("ai.executionResult.retryNote"));
  });
});
