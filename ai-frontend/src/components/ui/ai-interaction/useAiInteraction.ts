import { computed, ref } from "vue";

import type { InteractionStatus, SubmissionPhase, SubmitResult } from "./types";

/**
 * 交互生命周期状态机（表现层）。
 *
 * 只负责「卡片当前如何呈现」，不承载业务授权 / 审批语义：
 *   - accept 成功只代表「用户选择已记录」，不代表正式业务数据已变更；
 *   - 正式决策 / 审批由业务接口完成，卡片展示其结果与记录号。
 *
 * 状态：created → presented → in-progress → submitted → validating / processing
 *       → accepted / rejected / deferred / resolved / failed
 */

export interface UseAiInteractionOptions {
  initialStatus?: InteractionStatus;
  /** 提交适配器；缺省时只做本地态迁移（目录页用 mock 注入） */
  /**
   * 提交适配器。第二参 = 触发动作（Batch 3C 真端点路由用；目录页 mock
   * 忽略即可——向后兼容的扩展参，2026-11-09）。
   */
  submit?: (
    payload: unknown,
    action?: InteractionAction
  ) => Promise<SubmitResult>;
}

export type InteractionAction = "accept" | "reject" | "submit" | "resolve";

/** 动作成功后的目标状态 */
const SUCCESS_STATUS: Record<InteractionAction, InteractionStatus> = {
  accept: "accepted",
  reject: "rejected",
  submit: "submitted",
  resolve: "resolved"
};

export function useAiInteraction(options: UseAiInteractionOptions = {}) {
  const status = ref<InteractionStatus>(options.initialStatus ?? "presented");
  const phase = ref<SubmissionPhase>("idle");
  const result = ref<SubmitResult | null>(null);
  const error = ref<string | null>(null);
  const draft = ref<unknown>(null);

  let lastAction: InteractionAction | null = null;
  let lastPayload: unknown = null;

  const submitting = computed(() => phase.value === "submitting");
  const submitted = computed(
    () => phase.value === "success" && result.value !== null
  );

  /** 是否已进入不可编辑的终态（只读结果摘要） */
  const isTerminal = computed(() =>
    ["accepted", "rejected", "resolved"].includes(status.value)
  );

  /** 用户开始操作（展开编辑 / 选择选项） */
  function begin() {
    if (status.value === "created" || status.value === "presented") {
      status.value = "in-progress";
    }
  }

  function saveDraft(value: unknown) {
    draft.value = value;
  }

  function defer() {
    status.value = "deferred";
  }

  async function run(
    action: InteractionAction,
    payload: unknown,
    nextStatus: InteractionStatus
  ) {
    lastAction = action;
    lastPayload = payload;
    error.value = null;
    phase.value = "submitting";

    try {
      const adapter = options.submit;
      const res = adapter
        ? await adapter(payload, action)
        : ({ recorded: true } satisfies SubmitResult);
      result.value = res;
      status.value = nextStatus;
      phase.value = "success";
      return res;
    } catch (cause) {
      const message =
        cause instanceof Error ? cause.message : String(cause ?? "unknown");
      error.value = message;
      status.value = "failed";
      phase.value = "error";
      return null;
    }
  }

  /** 接收推荐 / 草稿（记录选择，不触发正式业务变更） */
  function accept(payload?: unknown) {
    return run("accept", payload, SUCCESS_STATUS.accept);
  }

  /** 拒绝（可带原因） */
  function reject(payload?: unknown) {
    return run("reject", payload, SUCCESS_STATUS.reject);
  }

  /** 提交结构化答案 / 提案 / 动作确认 */
  function submit(payload?: unknown) {
    return run("submit", payload, SUCCESS_STATUS.submit);
  }

  /** 正式决策提交（结果 = decision recorded） */
  function resolve(payload?: unknown) {
    return run("resolve", payload, SUCCESS_STATUS.resolve);
  }

  /** 失败后重试上一次动作 */
  function retry() {
    if (!lastAction) return Promise.resolve(null);
    return run(lastAction, lastPayload, SUCCESS_STATUS[lastAction]);
  }

  function reset() {
    status.value = "presented";
    phase.value = "idle";
    result.value = null;
    error.value = null;
  }

  return {
    status,
    phase,
    result,
    error,
    draft,
    submitting,
    submitted,
    isTerminal,
    begin,
    saveDraft,
    defer,
    accept,
    reject,
    submit,
    resolve,
    retry,
    reset
  };
}

export type AiInteractionState = ReturnType<typeof useAiInteraction>;
