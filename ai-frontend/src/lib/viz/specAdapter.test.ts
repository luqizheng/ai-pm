// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";

import {
  toLiveSpec,
  toVizArtifact,
  toVizEdge,
  toVizNode,
  toVizSpec,
  VizContractError
} from "./specAdapter";

function specDto() {
  return {
    spec_version: "1.0",
    slot_key: "req-trace:R-042",
    view: {
      kind: "trace-graph",
      focus: ["n1"],
      lenses: [{ name: "coverage", match: { edge_type: "covers" } }],
      layout_hint: "layered-lr"
    },
    nodes: [
      {
        id: "n1",
        label: "需求修订",
        kind: "requirement_revision",
        status: "confirmed",
        nature: "business",
        source_ref: {
          domain: "delivery",
          type: "requirement",
          id: "R-042",
          revision_id: "rev-12",
          content_hash: "abc"
        }
      },
      { id: "n2", label: "无溯源", kind: "design_revision" }
    ],
    edges: [
      {
        id: "e1",
        from: "n1",
        to: "n2",
        relation: "design.covers_requirement",
        derived: {
          direct: false,
          via: ["R-042"],
          semantic_confidence: "structurally_supported"
        }
      }
    ],
    meta: {
      title: "追踪图",
      lang: "zh-CN",
      budget: "balanced",
      nature: "business",
      registry_version: "1"
    }
  };
}

describe("specAdapter（唯一转换点）", () => {
  it("转换 Spec 核心结构", () => {
    const spec = toVizSpec(specDto());
    expect(spec.spec_version).toBe("1.0");
    expect(spec.slot_key).toBe("req-trace:R-042");
    expect(spec.nodes).toHaveLength(2);
    expect(spec.edges).toHaveLength(1);
    expect(spec.meta.budget).toBe("balanced");
    expect(spec.meta.registry_version).toBe("1");
  });

  it("保留推导标记与来源引用", () => {
    const spec = toVizSpec(specDto());
    expect(spec.edges[0].derived?.direct).toBe(false);
    expect(spec.edges[0].derived?.via).toEqual(["R-042"]);
    expect(spec.nodes[0].source_ref?.revision_id).toBe("rev-12");
    expect(spec.nodes[1].source_ref).toBeUndefined();
  });

  it("字段缺失即抛契约错误（不做静默默认）", () => {
    const dto = specDto() as Record<string, unknown>;
    delete dto.spec_version;
    expect(() => toVizSpec(dto)).toThrow(VizContractError);
  });

  it("节点 label 缺失即报错", () => {
    expect(() => toVizNode({ id: "n1", kind: "x" })).toThrow(VizContractError);
  });

  it("非法 nature 取值即报错", () => {
    expect(() =>
      toVizNode({ id: "n1", label: "l", kind: "x", nature: "hypothetical" })
    ).toThrow(VizContractError);
  });

  it("非法 budget 取值即报错", () => {
    const dto = specDto() as Record<string, Record<string, unknown>>;
    dto.meta.budget = "extreme";
    expect(() => toVizSpec(dto)).toThrow(VizContractError);
  });

  it("边端点缺失即报错", () => {
    expect(() => toVizEdge({ id: "e1", from: "a", relation: "r" })).toThrow(
      VizContractError
    );
  });

  it("转换 Artifact 并校验状态取值", () => {
    const artifact = toVizArtifact({
      id: "a1",
      project_id: "p1",
      slot_key: "slot",
      version_no: 3,
      state: "published",
      nature: "business",
      spec: specDto(),
      spec_hash: "sh",
      source_hash: "sou",
      created_at: "2026-11-12T00:00:00Z"
    });
    expect(artifact.state).toBe("published");
    expect(artifact.spec.nodes).toHaveLength(2);

    expect(() =>
      toVizArtifact({
        id: "a1",
        project_id: "p1",
        slot_key: "slot",
        state: "archived",
        spec: specDto(),
        spec_hash: "sh",
        source_hash: "sou",
        created_at: "x"
      })
    ).toThrow(VizContractError);
  });

  it("Live 响应拆出核心并保留一致性元信息", () => {
    const spec = toLiveSpec({
      data: {
        spec_version: "1.0",
        diagram_type: "trace-graph",
        nodes: [{ id: "n1", label: "l", kind: "requirement_revision" }],
        edges: []
      },
      meta: {
        generated_at: "2026-11-12T00:00:00Z",
        consistency: "mixed-revision",
        truncated: false
      }
    });
    expect(spec.view.kind).toBe("trace-graph");
    expect(spec.nodes).toHaveLength(1);
  });
});
