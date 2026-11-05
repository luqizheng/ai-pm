import type { SubmitResult } from "./types";

/**
 * 提交适配器（目录页 / 测试用）。
 *
 * 真实业务页应注入自己的 submit 实现（业务接口），本模块只提供：
 *   - 可控延迟，用于展示「提交中」；
 *   - 可控失败次数，用于展示「失败 + 重试」；
 *   - 成功结果固定带记录号，语义是「已记录」，不是「业务已生效」。
 */

export interface MockSubmitOptions {
  /** 每次提交的模拟耗时，毫秒 */
  latencyMs?: number;
  /** 前 N 次提交失败（用于重试演示），默认 0 */
  failTimes?: number;
  /** 记录号前缀，例如 INT-2041 */
  referencePrefix?: string;
}

export interface MockSubmit {
  (payload: unknown): Promise<SubmitResult>;
  /** 重置失败计数 */
  reset: () => void;
  /** 设置剩余失败次数（页面上演示失败/成功切换） */
  setFailTimes: (times: number) => void;
}

export function createMockSubmit(options: MockSubmitOptions = {}): MockSubmit {
  const latencyMs = options.latencyMs ?? 600;
  const prefix = options.referencePrefix ?? "INT";
  let remainingFailures = options.failTimes ?? 0;
  let sequence = 0;

  const submit = async (_payload: unknown): Promise<SubmitResult> => {
    await new Promise(resolve => setTimeout(resolve, latencyMs));

    if (remainingFailures > 0) {
      remainingFailures -= 1;
      throw new Error("SUBMIT_FAILED");
    }

    sequence += 1;
    return {
      recorded: true,
      reference: `${prefix}-${2040 + sequence}`,
      messageKey: "ai.card.submission.recorded"
    };
  };

  const withHelpers = submit as MockSubmit;
  withHelpers.reset = () => {
    remainingFailures = options.failTimes ?? 0;
    sequence = 0;
  };
  withHelpers.setFailTimes = (times: number) => {
    remainingFailures = times;
  };

  return withHelpers;
}

/** 目录页统一使用的提交适配器：600ms，前 1 次成功（错误态由用例单独注入） */
export const demoSubmit = createMockSubmit({ latencyMs: 600 });
