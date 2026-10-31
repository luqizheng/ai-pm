import { describe, expect, it } from "vitest";
import { useResource } from "./useResource";

describe("useResource", () => {
  it("loading → ready 并保存数据", async () => {
    const { state, data, refresh } = useResource(async () => 42);
    expect(state.value).toBe("loading");
    await refresh();
    expect(state.value).toBe("ready");
    expect(data.value).toBe(42);
  });

  it("reject → error，数据保持旧值", async () => {
    let fail = false;
    const { state, data, refresh } = useResource(async () => {
      if (fail) throw new Error("boom");
      return "first";
    });
    await refresh();
    fail = true;
    await refresh();
    expect(state.value).toBe("error");
    expect(data.value).toBe("first");
  });

  it("markStale 只在 ready 时生效", async () => {
    const { state, refresh, markStale } = useResource(async () => "ok");
    markStale();
    expect(state.value).toBe("loading");
    await refresh();
    markStale();
    expect(state.value).toBe("stale");
  });
});