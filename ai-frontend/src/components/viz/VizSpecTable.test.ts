// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";

import { mountShell } from "@/lib/test-utils";
import type { VizSpec } from "@/lib/viz/types";

import VizSpecTable from "./VizSpecTable.vue";

function spec(): VizSpec {
  return {
    spec_version: "1.0",
    slot_key: "golden",
    view: { kind: "trace-graph", focus: [], lenses: [] },
    nodes: [
      {
        id: "n1",
        label: "需求修订",
        kind: "requirement_revision",
        status: "confirmed",
        source_ref: { domain: "delivery", type: "requirement", id: "R-042" }
      },
      {
        id: "n2",
        label: "无溯源节点",
        kind: "design_revision"
      }
    ],
    edges: [
      {
        id: "e1",
        from: "n1",
        to: "n2",
        relation: "design.covers_requirement"
      },
      {
        id: "e2",
        from: "n2",
        to: "n1",
        relation: "derived_trace",
        derived: {
          direct: false,
          via: ["R-042"],
          semantic_confidence: "structurally_supported"
        }
      }
    ],
    meta: { budget: "balanced" }
  };
}

afterEach(() => {
  document.body.innerHTML = "";
});

describe("VizSpecTable", () => {
  it("渲染节点与关系（兼作 P0 节点导航通道）", () => {
    const wrapper = mountShell(VizSpecTable, { props: { spec: spec() } });

    expect(wrapper.findAll('[data-testid="viz-node-row"]')).toHaveLength(2);
    expect(wrapper.findAll('[data-testid="viz-edge-row"]')).toHaveLength(2);
  });

  it("无溯源节点显式标注（不得静默冒充已核实知识）", () => {
    const wrapper = mountShell(VizSpecTable, { props: { spec: spec() } });
    const rows = wrapper.findAll('[data-testid="viz-node-row"]');
    expect(rows[0].text()).toContain("delivery");
    expect(rows[1].text()).toContain("无溯源");
  });

  it("推导关系与直接关系分别标注", () => {
    const wrapper = mountShell(VizSpecTable, { props: { spec: spec() } });
    const rows = wrapper.findAll('[data-testid="viz-edge-row"]');
    expect(rows[0].text()).toContain("直接关系");
    expect(rows[1].text()).toContain("间接推导");
  });

  it("点击节点回传（打开溯源面板）", async () => {
    const wrapper = mountShell(VizSpecTable, { props: { spec: spec() } });
    await wrapper.findAll('[data-testid="viz-node-row"]')[0].trigger("click");
    const emitted = wrapper.emitted("selectNode");
    expect(emitted).toBeTruthy();
    expect((emitted?.[0][0] as { id: string }).id).toBe("n1");
  });
});
