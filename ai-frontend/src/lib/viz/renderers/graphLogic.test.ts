// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";

import type { VizSpec } from "../types";
import {
  collectKinds,
  collectRelations,
  edgeMatchesSelection,
  findPathInSpec,
  nodeMatchesSelection,
  pathEdgeIds,
  toG6Data
} from "./graphLogic";

function spec(): VizSpec {
  return {
    spec_version: "1.0",
    slot_key: "live:trace-graph",
    view: { kind: "trace-graph", focus: [], lenses: [] },
    nodes: [
      { id: "n1", label: "需求 A", kind: "requirement_revision" },
      { id: "n2", label: "设计 B", kind: "design_revision" },
      { id: "n3", label: "任务 C", kind: "task" },
      { id: "n4", label: "孤岛 D", kind: "task" }
    ],
    edges: [
      { id: "e1", from: "n1", to: "n2", relation: "design.covers_requirement" },
      {
        id: "e2",
        from: "n2",
        to: "n3",
        relation: "task.implements_requirement"
      }
    ],
    meta: { budget: "balanced" }
  };
}

describe("toG6Data", () => {
  it("映射节点与边（from/to → source/target）", () => {
    const data = toG6Data(spec());
    expect(data.nodes).toHaveLength(4);
    expect(data.edges).toHaveLength(2);
    expect(data.edges[0]).toMatchObject({
      id: "e1",
      source: "n1",
      target: "n2"
    });
    expect(data.nodes[0].data).toMatchObject({
      label: "需求 A",
      kind: "requirement_revision"
    });
  });

  it("悬空边（端点不在节点集）不进渲染数据", () => {
    const broken = spec();
    broken.edges.push({ id: "e9", from: "n1", to: "ghost", relation: "x" });
    const data = toG6Data(broken);
    expect(data.edges.map(e => e.id)).toEqual(["e1", "e2"]);
  });
});

describe("findPathInSpec（AC-P1-4 route journey）", () => {
  it("单跳：n1 → n2", () => {
    expect(findPathInSpec(spec(), "n1", "n2")).toEqual(["n1", "n2"]);
  });

  it("多跳：n1 → n2 → n3", () => {
    expect(findPathInSpec(spec(), "n1", "n3")).toEqual(["n1", "n2", "n3"]);
  });

  it("不可达（方向相反）：n3 ↛ n1", () => {
    expect(findPathInSpec(spec(), "n3", "n1")).toBeNull();
  });

  it("起终点相同：返回单节点序列", () => {
    expect(findPathInSpec(spec(), "n1", "n1")).toEqual(["n1"]);
  });

  it("端点不在图中：返回 null", () => {
    expect(findPathInSpec(spec(), "ghost", "n1")).toBeNull();
  });

  it("环图不死循环", () => {
    const cyclic = spec();
    cyclic.edges.push({ id: "e3", from: "n3", to: "n1", relation: "cycle" });
    expect(findPathInSpec(cyclic, "n1", "n3")).toEqual(["n1", "n2", "n3"]);
  });
});

describe("lens 匹配", () => {
  it("relation 维度：命中与未命中", () => {
    const s = spec();
    const e1 = s.edges[0];
    expect(
      edgeMatchesSelection(e1, { relation: "design.covers_requirement" })
    ).toBe(true);
    expect(edgeMatchesSelection(e1, { relation: "task.depends_on_task" })).toBe(
      false
    );
  });

  it("kind 维度：节点按 kind 匹配", () => {
    const s = spec();
    expect(
      nodeMatchesSelection(s, s.nodes[0], { kind: "requirement_revision" })
    ).toBe(true);
    expect(nodeMatchesSelection(s, s.nodes[0], { kind: "task" })).toBe(false);
  });

  it("kind + relation 组合：kind 命中但无命中邻边 → 不保留", () => {
    const s = spec();
    // n4 无任何边，relation 过滤下不保留
    expect(
      nodeMatchesSelection(s, s.nodes[3], {
        relation: "design.covers_requirement"
      })
    ).toBe(false);
    // n1 有关联边命中
    expect(
      nodeMatchesSelection(s, s.nodes[0], {
        relation: "design.covers_requirement"
      })
    ).toBe(true);
  });
});

describe("lens 数据源与路径边", () => {
  it("collectKinds / collectRelations 去重排序", () => {
    expect(collectKinds(spec())).toEqual([
      "design_revision",
      "requirement_revision",
      "task"
    ]);
    expect(collectRelations(spec())).toEqual([
      "design.covers_requirement",
      "task.implements_requirement"
    ]);
  });

  it("pathEdgeIds：路径相邻对 → 边 id 序列", () => {
    expect(pathEdgeIds(spec(), ["n1", "n2", "n3"])).toEqual(["e1", "e2"]);
    // 断开的相邻对跳过（防御）
    expect(pathEdgeIds(spec(), ["n1", "n3"])).toEqual([]);
  });
});
