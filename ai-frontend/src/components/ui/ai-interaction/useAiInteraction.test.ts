// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";

import { createMockSubmit } from "./submitAdapter";
import { useAiInteraction } from "./useAiInteraction";

describe("useAiInteraction", () => {
  it("accept 成功后进入 accepted，并返回记录号（不代表业务生效）", async () => {
    const submit = createMockSubmit({ latencyMs: 0 });
    const state = useAiInteraction({ submit });

    expect(state.status.value).toBe("presented");

    const result = await state.accept({ choice: "split" });

    expect(result?.recorded).toBe(true);
    expect(result?.reference).toMatch(/^INT-/);
    expect(state.status.value).toBe("accepted");
    expect(state.phase.value).toBe("success");
    expect(state.isTerminal.value).toBe(true);
  });

  it("reject 记录原因并进入 rejected", async () => {
    const state = useAiInteraction({
      submit: createMockSubmit({ latencyMs: 0 })
    });

    await state.reject({ reason: "scope" });

    expect(state.status.value).toBe("rejected");
  });

  it("提交失败进入 failed，retry 复用上一次动作并成功", async () => {
    const submit = createMockSubmit({ latencyMs: 0, failTimes: 1 });
    const state = useAiInteraction({ submit });

    await state.submit({ answer: 1 });
    expect(state.phase.value).toBe("error");
    expect(state.status.value).toBe("failed");
    expect(state.error.value).toContain("SUBMIT_FAILED");

    await state.retry();
    expect(state.phase.value).toBe("success");
    expect(state.status.value).toBe("submitted");
  });

  it("defer 与草稿保留不改写提交阶段", () => {
    const state = useAiInteraction();

    state.begin();
    expect(state.status.value).toBe("in-progress");

    state.saveDraft({ q1: "basic" });
    expect(state.draft.value).toEqual({ q1: "basic" });

    state.defer();
    expect(state.status.value).toBe("deferred");
    expect(state.phase.value).toBe("idle");
  });

  it("resolve 用于正式决策提交，进入 resolved", async () => {
    const state = useAiInteraction({
      submit: createMockSubmit({ latencyMs: 0 })
    });

    await state.resolve({ optionId: "opt-gpu" });

    expect(state.status.value).toBe("resolved");
  });
});
