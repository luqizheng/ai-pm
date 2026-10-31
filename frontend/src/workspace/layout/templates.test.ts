// @vitest-environment happy-dom
import { defineComponent } from "vue";
import { beforeEach, describe, expect, it } from "vitest";
import { mountShell } from "@/shell/test-utils";
import { registerWorkspaceComponent, resetWorkspaceRegistry } from "../registry/registry";
import type { WorkspaceSpec } from "../spec/types";
import { LAYOUT_SLOT_COUNT, resolveSlots } from "./templates";
import WorkspaceCanvas from "./WorkspaceCanvas.vue";

const PanelStub = defineComponent({
  props: { title: String },
  template: "<div data-test='stub-panel'>{{ title }}</div>"
});

function spec(overrides: Partial<WorkspaceSpec> = {}): WorkspaceSpec {
  return {
    schemaVersion: "1.0",
    workspaceId: "ws-1",
    projectId: "proj-1",
    layout: "overview",
    panels: [{ component: "A" }, { component: "B" }, { component: "C" }],
    ...overrides
  };
}

describe("layout templates", () => {
  it("槽位数表与模板一致", () => {
    expect(LAYOUT_SLOT_COUNT).toEqual({
      overview: 2,
      split: 2,
      timeline: 1,
      explorer: 2
    });
  });

  it("前 N 个 panel 入槽，其余进入溢出折叠区", () => {
    const resolved = resolveSlots(spec());
    expect(resolved.slots).toHaveLength(2);
    expect(resolved.slots[0].map(p => p.component)).toEqual(["A"]);
    expect(resolved.slots[1].map(p => p.component)).toEqual(["B"]);
    expect(resolved.overflow.map(p => p.component)).toEqual(["C"]);
  });

  it("timeline 只有一个槽位", () => {
    const resolved = resolveSlots(spec({ layout: "timeline" }));
    expect(resolved.slots).toHaveLength(1);
    expect(resolved.overflow).toHaveLength(2);
  });
});

describe("WorkspaceCanvas", () => {
  beforeEach(() => resetWorkspaceRegistry());

  it("注册组件按 config 渲染到槽位", () => {
    registerWorkspaceComponent({ id: "A", component: PanelStub });
    registerWorkspaceComponent({ id: "B", component: PanelStub });

    const w = mountShell(WorkspaceCanvas, { props: { spec: spec() } });
    expect(w.findAll("[data-test='stub-panel']")).toHaveLength(2);
    expect(w.find("[data-test='canvas-overflow']").exists()).toBe(true);
  });

  it("未注册组件降级为 FallbackPanel（不丢弃、不抛错）", () => {
    const w = mountShell(WorkspaceCanvas, { props: { spec: spec() } });
    expect(w.findAll("[data-test='fallback-panel']")).toHaveLength(3);
    expect(w.text()).toContain("unknown-component:A");
  });

  it("config 作为 props 透传", () => {
    registerWorkspaceComponent({ id: "A", component: PanelStub });
    registerWorkspaceComponent({ id: "B", component: PanelStub });
    registerWorkspaceComponent({ id: "C", component: PanelStub });

    const w = mountShell(WorkspaceCanvas, {
      props: {
        spec: spec({
          panels: [{ component: "A", config: { title: "hello" } }]
        })
      }
    });
    expect(w.text()).toContain("hello");
  });
});