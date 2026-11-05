// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import type { Component } from "vue";
import { flushPromises } from "@vue/test-utils";
import type { ComponentMountingOptions, VueWrapper } from "@vue/test-utils";

/**
 * vditor 在 happy-dom 下无法初始化，mock 成 md2html / preview（同既有 MarkdownViewer 测试）。
 * 本文件的 4 张卡不直接渲染 Markdown，但共享组件链上可能引入 MarkdownViewer，统一兜底。
 */
const md2html = vi.hoisted(() => vi.fn().mockResolvedValue(""));

vi.mock("vditor", () => ({
  default: { md2html, preview: vi.fn(), version: "test" }
}));

import { mountShell } from "@/lib/test-utils";
import ActionRequestResponse from "./ActionRequestResponse.vue";
import DecisionResponse from "./DecisionResponse.vue";
import ExecutionResultResponse from "./ExecutionResultResponse.vue";
import QuestionResponse from "./QuestionResponse.vue";
import {
  actionRequestFixture,
  aiIdentity,
  decisionFixture,
  executionFailureFixture,
  executionResultFixture,
  questionFixture
} from "../fixtures";
import type { SubmitResult } from "../types";

const mounted: Array<VueWrapper> = [];

function mountCard<T extends Component>(
  component: T,
  props: Record<string, unknown>
): VueWrapper {
  const wrapper = mountShell(component, {
    props
  } as unknown as ComponentMountingOptions<T>);
  mounted.push(wrapper as VueWrapper);
  return wrapper as VueWrapper;
}

function findButton(
  wrapper: VueWrapper,
  label: string
): ReturnType<VueWrapper["find"]> | undefined {
  return wrapper
    .findAll("button")
    .find(button => button.text().includes(label));
}

async function settleUi(): Promise<void> {
  await flushPromises();
  await nextTick();
}

afterEach(() => {
  while (mounted.length) mounted.pop()?.unmount();
  document.body.innerHTML = "";
});

describe("QuestionResponse", () => {
  it("必填未答时提交被拦截并显示校验错误；答齐后一次性提交并转为只读汇总", async () => {
    const submit = vi.fn(
      async (): Promise<SubmitResult> => ({
        recorded: true,
        reference: "Q-2041"
      })
    );
    const wrapper = mountCard(QuestionResponse, {
      payload: questionFixture,
      submit,
      identity: aiIdentity
    });

    const submitAll = findButton(wrapper, "提交全部答案");
    expect(submitAll).toBeTruthy();
    await submitAll!.trigger("click");
    await settleUi();

    // 拦截：不发起提交、出现必填校验错误
    expect(submit).not.toHaveBeenCalled();
    expect(wrapper.findAll(".ai-question__item.is-invalid").length).toBe(3);
    expect(wrapper.findAll(".ai-question__error").length).toBe(3);
    expect(wrapper.text()).toContain("请填写");

    // 逐题作答：单选 / 多选 / 日期
    const scopeRadio = wrapper.find(
      '[data-question-id="q-scope"] input[type="radio"]'
    );
    (scopeRadio.element as HTMLInputElement).checked = true;
    await scopeRadio.trigger("change");

    await wrapper
      .find('[data-question-id="q-biomes"] .ui-checkbox__root')
      .trigger("click");

    const deadline = wrapper.find(
      '[data-question-id="q-deadline"] input[type="date"]'
    );
    await deadline.setValue("2026-11-20");
    await nextTick();

    expect(wrapper.findAll(".ai-question__error").length).toBe(0);

    await findButton(wrapper, "提交全部答案")!.trigger("click");
    await settleUi();

    expect(submit).toHaveBeenCalledTimes(1);
    const payload = submit.mock.calls[0][0] as {
      answers: Record<string, string | string[]>;
    };
    expect(payload.answers["q-scope"]).toBe("full");
    expect(payload.answers["q-biomes"]).toEqual(["forest"]);
    expect(payload.answers["q-deadline"]).toBe("2026-11-20");

    // 只读答案汇总，不再有输入控件
    expect(wrapper.find(".ai-question__summary").exists()).toBe(true);
    expect(wrapper.find(".ai-question__item").exists()).toBe(false);
    expect(wrapper.text()).toContain("已提交答案");
    expect(wrapper.text()).toContain("完整分布算法");
    expect(wrapper.text()).toContain("Q-2041");
  });

  it("保存草稿给出提示且不清空已答内容", async () => {
    const submit = vi.fn();
    const wrapper = mountCard(QuestionResponse, {
      payload: questionFixture,
      submit
    });

    const scopeRadio = wrapper.find(
      '[data-question-id="q-scope"] input[type="radio"]'
    );
    (scopeRadio.element as HTMLInputElement).checked = true;
    await scopeRadio.trigger("change");
    await nextTick();
    expect(wrapper.text()).toContain("已答 1 / 5");

    await findButton(wrapper, "保存草稿")!.trigger("click");
    await nextTick();

    expect(wrapper.text()).toContain("草稿已保存到本地");
    expect(
      (
        wrapper.find('[data-question-id="q-scope"] input[type="radio"]')
          .element as HTMLInputElement
      ).checked
    ).toBe(true);
    expect(wrapper.text()).toContain("已答 1 / 5");
    expect(submit).not.toHaveBeenCalled();
  });
});

describe("DecisionResponse", () => {
  it("未勾选本人确认不能提交；二次确认后显示记录号与人类决策人", async () => {
    const submit = vi.fn(
      async (): Promise<SubmitResult> => ({
        recorded: true,
        reference: "DEC-12"
      })
    );
    const wrapper = mountCard(DecisionResponse, {
      payload: decisionFixture,
      submit,
      identity: aiIdentity
    });

    const options = wrapper.findAll(".ai-decision__option");
    expect(options.length).toBe(3);
    // AI 推荐与用户选择是两个独立通道
    expect(options[0].text()).toContain("AI 推荐");

    await options[0].trigger("click");
    expect(wrapper.find(".ai-decision__option.is-selected").exists()).toBe(
      true
    );

    // 未勾选「以本人身份提交」→ 不打开二次确认、不提交
    await findButton(wrapper, "提交正式决策")!.trigger("click");
    await nextTick();
    expect(document.body.querySelector(".ui-confirm")).toBeNull();
    expect(submit).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("提交前请先勾选确认声明");

    // 勾选确认声明 → 打开 ConfirmationDialog
    await wrapper
      .find(".ai-decision__confirm-checkbox .ui-checkbox__root")
      .trigger("click");
    await findButton(wrapper, "提交正式决策")!.trigger("click");
    await nextTick();

    const dialog = document.body.querySelector(".ui-confirm");
    expect(dialog).not.toBeNull();
    expect(submit).not.toHaveBeenCalled();

    // 二次确认后才正式提交
    (
      document.body.querySelector(".ui-confirm__btn--primary") as HTMLElement
    ).click();
    await settleUi();

    expect(submit).toHaveBeenCalledTimes(1);
    const record = wrapper.find(".ai-decision__record");
    expect(record.exists()).toBe(true);
    expect(record.text()).toContain("决策人");
    expect(record.text()).toContain("你（人类）");
    expect(record.text()).toContain("DEC-12");
    expect(wrapper.text()).not.toContain("AI 决定");
    expect(wrapper.find(".ai-card").attributes("data-status")).toBe("resolved");
  });
});

describe("ActionRequestResponse", () => {
  it("Confirm 先打开 ConfirmationDialog；后端 resolve 前不显示成功", async () => {
    let resolveSubmit: (result: SubmitResult) => void = () => {};
    const submit = vi.fn(
      () =>
        new Promise<SubmitResult>(resolve => {
          resolveSubmit = resolve;
        })
    );

    const wrapper = mountCard(ActionRequestResponse, {
      payload: actionRequestFixture,
      submit,
      identity: aiIdentity
    });

    await findButton(wrapper, "确认")!.trigger("click");
    await nextTick();

    // 高后果操作：先弹二次确认，未确认前不提交
    expect(document.body.querySelector(".ui-confirm")).not.toBeNull();
    expect(submit).not.toHaveBeenCalled();
    expect(wrapper.text()).not.toContain("操作已确认并提交执行");

    (
      document.body.querySelector(".ui-confirm__btn--primary") as HTMLElement
    ).click();
    await settleUi();

    // 已发起提交，但后端未 resolve → 只能看到提交中
    expect(submit).toHaveBeenCalledTimes(1);
    expect(wrapper.find(".ai-submission.is-submitting").exists()).toBe(true);
    expect(wrapper.text()).not.toContain("操作已确认并提交执行");

    resolveSubmit({ recorded: true, reference: "ACT-9" });
    await settleUi();

    expect(wrapper.find(".ai-submission.is-success").exists()).toBe(true);
    expect(wrapper.text()).toContain("操作已确认并提交执行");
    expect(wrapper.text()).toContain("ACT-9");
    expect(wrapper.text()).toContain("以执行记录为准");
    expect(wrapper.text()).not.toContain("已执行完成");
  });

  it("查看详情展开影响与权限详情", async () => {
    const wrapper = mountCard(ActionRequestResponse, {
      payload: actionRequestFixture,
      submit: vi.fn()
    });

    expect(wrapper.find(".ai-action__details").exists()).toBe(false);
    expect(wrapper.text()).toContain("仓库写权限 + 合并审批");
    expect(wrapper.text()).toContain("3 项预期影响");

    await findButton(wrapper, "查看详情")!.trigger("click");
    await nextTick();

    expect(wrapper.find(".ai-action__details").exists()).toBe(true);
    expect(wrapper.text()).toContain("触发 CI 全量回归");
    expect(wrapper.findAll(".ai-action__impact li").length).toBe(3);
  });
});

describe("ExecutionResultResponse", () => {
  it("失败且 retryable 时 Retry 可用，重试成功后交互状态变化", async () => {
    const submit = vi.fn(
      async (): Promise<SubmitResult> => ({
        recorded: true,
        reference: "RUN-78"
      })
    );
    const wrapper = mountCard(ExecutionResultResponse, {
      payload: executionFailureFixture,
      submit,
      identity: aiIdentity
    });

    expect(wrapper.find(".ai-execution__state").classes()).toContain(
      "is-failed"
    );
    expect(wrapper.text()).toContain("失败");

    const retryButton = findButton(wrapper, "重试");
    expect(retryButton).toBeTruthy();

    await retryButton!.trigger("click");
    await settleUi();

    expect(submit).toHaveBeenCalledTimes(1);
    expect(wrapper.find(".ai-card").attributes("data-status")).toBe(
      "submitted"
    );
    expect(wrapper.find(".ai-submission.is-success").exists()).toBe(true);
    expect(wrapper.text()).toContain("重试请求已提交");
    expect(wrapper.text()).toContain("RUN-78");
  });

  it("不可重试时没有 Retry；partial 与 failed 状态视觉可辨", async () => {
    const nonRetryable = {
      ...executionFailureFixture,
      failure: {
        code: "FATAL",
        message: "Irrecoverable",
        retryable: false
      }
    };
    const failedWrapper = mountCard(ExecutionResultResponse, {
      payload: nonRetryable,
      submit: vi.fn()
    });

    expect(findButton(failedWrapper, "重试")).toBeUndefined();
    expect(failedWrapper.find(".ai-execution__state").classes()).toContain(
      "is-failed"
    );
    expect(failedWrapper.find(".ai-execution__failure").exists()).toBe(true);
    expect(failedWrapper.text()).toContain("不可重试");

    const partialWrapper = mountCard(ExecutionResultResponse, {
      payload: executionResultFixture,
      submit: vi.fn()
    });

    expect(partialWrapper.find(".ai-execution__state").classes()).toContain(
      "is-partial"
    );
    expect(partialWrapper.text()).toContain("部分成功");
    expect(
      partialWrapper.findAll(".ai-execution__warnings li").length
    ).toBeGreaterThan(0);
    expect(partialWrapper.findAll("tbody tr").length).toBe(3);
  });

  it("查看执行详情展开日志与 diff；请求修改需填写原因后才提交", async () => {
    const submit = vi.fn(
      async (): Promise<SubmitResult> => ({
        recorded: true,
        reference: "RUN-79"
      })
    );
    const wrapper = mountCard(ExecutionResultResponse, {
      payload: executionResultFixture,
      submit
    });

    await findButton(wrapper, "查看执行详情")!.trigger("click");
    await nextTick();

    expect(wrapper.find(".ai-execution__details").exists()).toBe(true);
    expect(wrapper.findAll(".ai-execution__log-line").length).toBe(5);
    expect(wrapper.findAll(".ai-execution__diff-line").length).toBeGreaterThan(
      0
    );

    // 请求修改：未填写原因不提交
    await findButton(wrapper, "请求修改")!.trigger("click");
    await nextTick();
    expect(wrapper.find(".ai-execution__changes").exists()).toBe(true);

    await findButton(wrapper, "提交修改请求")!.trigger("click");
    await nextTick();
    expect(wrapper.text()).toContain("请填写需要修改的原因");
    expect(submit).not.toHaveBeenCalled();

    // 填写原因后提交
    await wrapper
      .find(".ai-execution__changes textarea")
      .setValue("请补充空网格与单点网格用例");
    await findButton(wrapper, "提交修改请求")!.trigger("click");
    await settleUi();

    expect(submit).toHaveBeenCalledTimes(1);
    expect((submit.mock.calls[0][0] as { kind: string }).kind).toBe(
      "request-changes"
    );
    expect(wrapper.text()).toContain("修改请求已提交");
    expect(wrapper.find(".ai-execution__changes").exists()).toBe(false);
  });
});
