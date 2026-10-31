import { describe, expect, it } from "vitest";
import { isFocusObjectType, parseWorkspaceSpec } from "./schema";

const valid = {
  schemaVersion: "1.0",
  workspaceId: "ws-1",
  projectId: "proj-1",
  subject: { type: "requirement", id: "R-101" },
  layout: "overview",
  panels: [{ component: "BriefingCard" }, { component: "GoalStrip", config: { compact: true } }]
};

describe("parseWorkspaceSpec", () => {
  it("接受合法 spec（未知字段忽略）", () => {
    const result = parseWorkspaceSpec({ ...valid, futureField: 1 });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.spec.layout).toBe("overview");
      expect(result.spec.panels).toHaveLength(2);
      expect(result.spec.subject).toEqual({ type: "requirement", id: "R-101" });
    }
  });

  it("拒绝越界 layout", () => {
    const result = parseWorkspaceSpec({ ...valid, layout: "grid" });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.errors.join()).toMatch(/layout/);
  });

  it("拒绝空 panels", () => {
    const result = parseWorkspaceSpec({ ...valid, panels: [] });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.errors.join()).toMatch(/panels/);
  });

  it("拒绝非字符串 component", () => {
    const result = parseWorkspaceSpec({
      ...valid,
      panels: [{ component: 42 }]
    });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.errors.join()).toMatch(/component/);
  });

  it("拒绝缺 id 的 dataRef", () => {
    const result = parseWorkspaceSpec({
      ...valid,
      panels: [{ component: "GoalStrip", dataRef: { type: "task" } }]
    });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.errors.join()).toMatch(/dataRef\.id/);
  });

  it("拒绝旧 schemaVersion", () => {
    const result = parseWorkspaceSpec({ ...valid, schemaVersion: "0.9" });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.errors.join()).toMatch(/schemaVersion/);
  });

  it("focus 类型白名单", () => {
    expect(isFocusObjectType("requirement")).toBe(true);
    expect(isFocusObjectType("task")).toBe(true);
    expect(isFocusObjectType("artifact")).toBe(false);
  });
});