import { describe, expect, it } from "vitest";
import { buildFocusPreset, FOCUS_PRESETS, resolveFocus } from "./presets";

describe("focus presets", () => {
  it("P0 只有 requirement / task 两个预设", () => {
    expect(Object.keys(FOCUS_PRESETS).sort()).toEqual(["requirement", "task"]);
    expect(FOCUS_PRESETS.requirement.layout).toBe("split");
    expect(FOCUS_PRESETS.requirement.panels).toEqual([
      "FocusSummary",
      "FocusTimeline"
    ]);
  });

  it("buildFocusPreset 产出合法 spec（subject/dataRef/config 对齐）", () => {
    const spec = buildFocusPreset("task", "T-7", "proj-1");
    expect(spec.schemaVersion).toBe("1.0");
    expect(spec.workspaceId).toBe("focus-task-T-7");
    expect(spec.subject).toEqual({ type: "task", id: "T-7" });
    expect(spec.panels).toHaveLength(2);
    expect(spec.panels[0].config).toEqual({ objectType: "task", objectId: "T-7" });
  });

  it("未知 objectType 返回降级原因", () => {
    expect(resolveFocus("artifact")).toEqual({
      ok: false,
      reason: "unknown-type"
    });
    expect(resolveFocus("requirement")).toEqual({
      ok: true,
      type: "requirement"
    });
  });
});