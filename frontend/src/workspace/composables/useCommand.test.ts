import { describe, expect, it, vi } from "vitest";
import { useCommand } from "./useCommand";

describe("useCommand", () => {
  it("idle → submitting → succeeded", async () => {
    const executor = vi.fn(async () => "ok");
    const { phase, submit } = useCommand(executor);
    expect(phase.value).toBe("idle");
    const pending = submit({ id: 1 });
    expect(phase.value).toBe("submitting");
    await pending;
    expect(phase.value).toBe("succeeded");
    expect(executor).toHaveBeenCalledWith({ id: 1 });
  });

  it("reject → failed 并记录错误", async () => {
    const { phase, error, submit } = useCommand(async () => {
      throw new Error("nope");
    });
    await submit(undefined);
    expect(phase.value).toBe("failed");
    expect(error.value).toBe("nope");
  });

  it("submitting 期间重复 submit 被忽略", async () => {
    let release: (() => void) | undefined;
    const executor = vi.fn(
      () => new Promise<void>(resolve => (release = resolve))
    );
    const { submit } = useCommand(executor);
    const first = submit(undefined);
    void submit(undefined);
    expect(executor).toHaveBeenCalledTimes(1);
    release?.();
    await first;
  });
});