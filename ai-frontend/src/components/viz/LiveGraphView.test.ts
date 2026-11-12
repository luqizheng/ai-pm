// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { mountShell, settle } from "@/lib/test-utils";
import type { VizLiveResponse } from "@/lib/viz/types";

import LiveGraphView from "./LiveGraphView.vue";

const { getLiveGraph } = vi.hoisted(() => ({
  getLiveGraph: vi.fn()
}));

vi.mock("@/api/visualizations", () => ({
  getLiveGraph
}));

const { createG6Renderer } = vi.hoisted(() => ({
  createG6Renderer: vi.fn()
}));

vi.mock("@/lib/viz/renderers/g6Adapter", () => ({
  createG6Renderer
}));

function liveResponse(
  overrides: Partial<VizLiveResponse["meta"]> = {}
): VizLiveResponse {
  return {
    data: {
      spec_version: "1.0",
      diagram_type: "trace-graph",
      nodes: [
        { id: "n1", label: "需求 A", kind: "requirement_revision" },
        { id: "n2", label: "设计 B", kind: "design_revision" },
        { id: "n3", label: "任务 C", kind: "task" }
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
          to: "n3",
          relation: "task.implements_requirement"
        }
      ]
    },
    meta: {
      generated_at: "2026-11-12T00:00:00Z",
      consistency: "current",
      truncated: false,
      ...overrides
    }
  };
}

function rendererMock() {
  return {
    capabilities: {
      rendererId: "g6",
      hoverTrace: true,
      focusPanel: true,
      routeJourney: true,
      lens: true
    },
    destroy: vi.fn(),
    applyLens: vi.fn().mockResolvedValue(undefined),
    clearLens: vi.fn().mockResolvedValue(undefined),
    highlightPath: vi.fn().mockResolvedValue(undefined),
    clearPath: vi.fn().mockResolvedValue(undefined),
    findPath: vi.fn()
  };
}

async function mountLive(props = { projectId: "p1", viewKind: "trace-graph" }) {
  const wrapper = mountShell(LiveGraphView, {
    props: props as never,
    attachTo: document.body
  });
  await settle();
  await settle();
  return wrapper;
}

beforeEach(() => {
  getLiveGraph.mockReset();
  createG6Renderer.mockReset();
});

afterEach(() => {
  document.body.innerHTML = "";
});

describe("LiveGraphView", () => {
  it("加载成功：创建渲染器并传入 Live spec（交互载体就位）", async () => {
    const renderer = rendererMock();
    createG6Renderer.mockResolvedValue(renderer);
    getLiveGraph.mockResolvedValue(liveResponse());

    await mountLive();

    expect(getLiveGraph).toHaveBeenCalledWith("p1", "trace-graph");
    expect(createG6Renderer).toHaveBeenCalledTimes(1);
    const options = createG6Renderer.mock.calls[0][0];
    expect(options.spec.nodes).toHaveLength(3);
    expect(options.spec.edges).toHaveLength(2);
    expect(options.container).toBeInstanceOf(HTMLElement);
  });

  it("点击节点 → emit selectNode(node, spec)（focus panel 通道）", async () => {
    const renderer = rendererMock();
    createG6Renderer.mockImplementation(async ({ onNodeClick }) => {
      onNodeClick?.("n2");
      return renderer;
    });
    getLiveGraph.mockResolvedValue(liveResponse());

    const wrapper = await mountLive();
    const emitted = wrapper.emitted("selectNode");
    expect(emitted).toHaveLength(1);
    const [node, spec] = emitted![0] as [never, never];
    expect(node).toMatchObject({ id: "n2", label: "设计 B" });
    expect(spec).toMatchObject({ view: { kind: "trace-graph" } });
  });

  it("渲染器抛错 → 降级表格（AC-P1-5，页面不报错）", async () => {
    createG6Renderer.mockRejectedValue(new Error("g6 boom"));
    getLiveGraph.mockResolvedValue(liveResponse());

    const wrapper = await mountLive();

    expect(wrapper.find('[data-testid="live-graph-degraded"]').exists()).toBe(
      true
    );
    // 降级表格渲染出节点行（复用 VizSpecTable）
    expect(
      wrapper.findAll('[data-testid="viz-node-row"]').length
    ).toBeGreaterThan(0);
  });

  it("加载失败 → 降级提示", async () => {
    getLiveGraph.mockRejectedValue(new Error("api down"));
    const wrapper = await mountLive();
    expect(
      wrapper.find('[data-testid="live-graph-load-failed"]').exists()
    ).toBe(true);
  });

  it("mixed-revision 必须提示（不静默展示成同一时点）", async () => {
    createG6Renderer.mockResolvedValue(rendererMock());
    getLiveGraph.mockResolvedValue(
      liveResponse({ consistency: "mixed-revision" })
    );
    const wrapper = await mountLive();
    expect(wrapper.find('[data-testid="live-graph-mixed"]').exists()).toBe(
      true
    );
  });

  it("truncated 提示", async () => {
    createG6Renderer.mockResolvedValue(rendererMock());
    getLiveGraph.mockResolvedValue(liveResponse({ truncated: true }));
    const wrapper = await mountLive();
    expect(wrapper.find('[data-testid="live-graph-truncated"]').exists()).toBe(
      true
    );
  });

  it("lens 选择变化 → applyLens(kind)（AC-P1-3）", async () => {
    const renderer = rendererMock();
    createG6Renderer.mockResolvedValue(renderer);
    getLiveGraph.mockResolvedValue(liveResponse());

    const wrapper = await mountLive();
    await wrapper.find('[data-testid="live-graph-lens-kind"]').setValue("task");
    await settle();

    expect(renderer.applyLens).toHaveBeenCalledWith({ kind: "task" });
  });

  it("lens 全部 → clearLens", async () => {
    const renderer = rendererMock();
    createG6Renderer.mockResolvedValue(renderer);
    getLiveGraph.mockResolvedValue(liveResponse());

    const wrapper = await mountLive();
    await wrapper.find('[data-testid="live-graph-lens-kind"]').setValue("task");
    await settle();
    await wrapper.find('[data-testid="live-graph-lens-kind"]').setValue("");
    await settle();

    expect(renderer.clearLens).toHaveBeenCalled();
  });

  it("route journey：BFS 命中 → highlightPath + 步骤列表（AC-P1-4 UI）", async () => {
    const renderer = rendererMock();
    renderer.findPath.mockReturnValue(["n1", "n2", "n3"]);
    createG6Renderer.mockResolvedValue(renderer);
    getLiveGraph.mockResolvedValue(liveResponse());

    const wrapper = await mountLive();
    await wrapper
      .find('[data-testid="live-graph-journey-from"]')
      .setValue("n1");
    await wrapper.find('[data-testid="live-graph-journey-to"]').setValue("n3");
    await wrapper
      .find('[data-testid="live-graph-journey-run"]')
      .trigger("click");
    await settle();

    expect(renderer.findPath).toHaveBeenCalledWith("n1", "n3");
    expect(renderer.highlightPath).toHaveBeenCalledWith(["n1", "n2", "n3"]);
    const path = wrapper.find('[data-testid="live-graph-journey-path"]');
    expect(path.exists()).toBe(true);
    expect(path.text()).toContain("需求 A");
    expect(path.text()).toContain("任务 C");
  });

  it("route journey 不可达 → 提示（不高亮）", async () => {
    const renderer = rendererMock();
    renderer.findPath.mockReturnValue(null);
    createG6Renderer.mockResolvedValue(renderer);
    getLiveGraph.mockResolvedValue(liveResponse());

    const wrapper = await mountLive();
    await wrapper
      .find('[data-testid="live-graph-journey-from"]')
      .setValue("n3");
    await wrapper.find('[data-testid="live-graph-journey-to"]').setValue("n1");
    await wrapper
      .find('[data-testid="live-graph-journey-run"]')
      .trigger("click");
    await settle();

    expect(
      wrapper.find('[data-testid="live-graph-journey-empty"]').exists()
    ).toBe(true);
    expect(renderer.highlightPath).not.toHaveBeenCalled();
  });

  it("卸载时释放渲染器", async () => {
    const renderer = rendererMock();
    createG6Renderer.mockResolvedValue(renderer);
    getLiveGraph.mockResolvedValue(liveResponse());

    const wrapper = await mountLive();
    wrapper.unmount();
    await settle();
    expect(renderer.destroy).toHaveBeenCalled();
  });
});
