import { isFocusObjectType } from "../spec/schema";
import type { FocusObjectType, WorkspaceSpec } from "../spec/types";

/**
 * /focus 预设（附录 C.2）：P0 只有 requirement / task 两行。
 * 布局与 panel 顺序是协议的一部分，测试按此处断言。
 */
export const FOCUS_PRESETS: Record<
  FocusObjectType,
  { layout: WorkspaceSpec["layout"]; panels: string[] }
> = {
  requirement: { layout: "split", panels: ["FocusSummary", "FocusTimeline"] },
  task: { layout: "split", panels: ["FocusSummary", "FocusTimeline"] }
};

export type FocusResolution =
  | { ok: true; type: FocusObjectType }
  | { ok: false; reason: "unknown-type" };

export function resolveFocus(objectType: string): FocusResolution {
  if (isFocusObjectType(objectType)) {
    return { ok: true, type: objectType as FocusObjectType };
  }
  return { ok: false, reason: "unknown-type" };
}

export function buildFocusPreset(
  objectType: FocusObjectType,
  id: string,
  projectId: string
): WorkspaceSpec {
  const preset = FOCUS_PRESETS[objectType];
  return {
    schemaVersion: "1.0",
    workspaceId: `focus-${objectType}-${id}`,
    projectId,
    subject: { type: objectType, id },
    layout: preset.layout,
    panels: preset.panels.map(component => ({
      component,
      dataRef: { type: objectType, id },
      config: { objectType, objectId: id }
    }))
  };
}