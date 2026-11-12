// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";

import { mountShell, settle } from "@/lib/test-utils";
import type { VizNode, VizSpec } from "@/lib/viz/types";

import VizTracePanel from "./VizTracePanel.vue";

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
        source_ref: {
          domain: "delivery",
          type: "requirement",
          id: "R-042",
          revision_id: "rev-12"
        }
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

function node(id: string): VizNode | null {
  return spec().nodes.find(n => n.id === id) ?? null;
}

afterEach(() => {
  document.body.innerHTML = "";
});

describe("VizTracePanel", () => {
  it("展示来源对象与版本", async () => {
    mountShell(VizTracePanel, {
      props: { open: true, spec: spec(), node: node("n1") },
      attachTo: document.body
    });
    await settle();

    const text = document.body.textContent ?? "";
    expect(text).toContain("delivery");
    expect(text).toContain("R-042");
    expect(text).toContain("rev-12");
  });

  it("提供打开原文档入口", async () => {
    mountShell(VizTracePanel, {
      props: { open: true, spec: spec(), node: node("n1") },
      attachTo: document.body
    });
    await settle();
    expect(
      document.body.querySelector('[data-testid="viz-open-source"]')
    ).not.toBeNull();
  });

  it("无溯源节点给出明确提示", async () => {
    mountShell(VizTracePanel, {
      props: { open: true, spec: spec(), node: node("n2") },
      attachTo: document.body
    });
    await settle();
    expect(
      document.body.querySelector('[data-testid="viz-unlinked"]')
    ).not.toBeNull();
  });

  it("推导关系标注推导路径（不得伪装成直接关系）", async () => {
    mountShell(VizTracePanel, {
      props: { open: true, spec: spec(), node: node("n2") },
      attachTo: document.body
    });
    await settle();
    expect(document.body.textContent ?? "").toContain("经 R-042 推导");
  });
});
