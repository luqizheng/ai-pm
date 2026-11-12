// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

import { mountShell, settle } from "@/lib/test-utils";
import type { VizArtifact } from "@/lib/viz/types";

import VizArtifactFrame from "./VizArtifactFrame.vue";

const api = vi.hoisted(() => ({
  fetchArtifactSvgUrl: vi.fn(async () => "blob:mock-svg")
}));

vi.mock("@/api/visualizations", () => api);

function artifact(overrides: Partial<VizArtifact> = {}): VizArtifact {
  return {
    id: "artifact-1",
    project_id: "project-1",
    slot_key: "req-trace:R-042",
    version_no: 3,
    state: "published",
    nature: "business",
    spec: {
      spec_version: "1.0",
      slot_key: "req-trace:R-042",
      view: { kind: "trace-graph", focus: [], lenses: [] },
      nodes: [
        {
          id: "n1",
          label: "节点",
          kind: "requirement_revision",
          source_ref: { domain: "delivery", type: "requirement", id: "R-042" }
        }
      ],
      edges: [],
      meta: { title: "追踪图", budget: "balanced" }
    },
    spec_hash: "spec-hash",
    source_hash: "source-hash",
    created_at: "2026-11-12T00:00:00Z",
    ...overrides
  };
}

afterEach(() => {
  document.body.innerHTML = "";
  vi.clearAllMocks();
});

describe("VizArtifactFrame", () => {
  it("以 <img> 引用产物（不引入 iframe/object —— 产物零脚本执行）", async () => {
    const wrapper = mountShell(VizArtifactFrame, {
      props: { artifact: artifact() }
    });
    await settle();

    const image = wrapper.get('[data-testid="viz-artifact-image"]');
    expect(image.attributes("src")).toBe("blob:mock-svg");
    expect(wrapper.find("iframe").exists()).toBe(false);
    expect(wrapper.find("object").exists()).toBe(false);
  });

  it("卸载时释放 object URL（防 Blob 泄漏）", async () => {
    const revoke = vi.spyOn(URL, "revokeObjectURL");
    const wrapper = mountShell(VizArtifactFrame, {
      props: { artifact: artifact() }
    });
    await settle();
    wrapper.unmount();
    expect(revoke).toHaveBeenCalledWith("blob:mock-svg");
    revoke.mockRestore();
  });

  it("加载失败时降级（不抛错、不阻断页面）", async () => {
    api.fetchArtifactSvgUrl.mockRejectedValueOnce(new Error("boom"));
    const wrapper = mountShell(VizArtifactFrame, {
      props: { artifact: artifact() }
    });
    await settle();

    expect(wrapper.find('[data-testid="viz-artifact-error"]').exists()).toBe(
      true
    );
    expect(wrapper.find('[data-testid="viz-artifact-image"]').exists()).toBe(
      false
    );
  });

  it("stale 状态显示过期提示", async () => {
    const wrapper = mountShell(VizArtifactFrame, {
      props: { artifact: artifact({ state: "stale" }) }
    });
    await settle();
    expect(wrapper.find('[data-testid="viz-stale-hint"]').exists()).toBe(true);
  });

  it("explanatory 产物显示「非正式认知」标识", async () => {
    const wrapper = mountShell(VizArtifactFrame, {
      props: { artifact: artifact({ nature: "explanatory" }) }
    });
    await settle();
    expect(wrapper.find('[data-testid="viz-nature-badge"]').exists()).toBe(
      true
    );
  });

  it("始终明示 P0 无交互的缺口（避免被当作功能缺失）", async () => {
    const wrapper = mountShell(VizArtifactFrame, {
      props: { artifact: artifact() }
    });
    await settle();
    expect(wrapper.find('[data-testid="viz-interaction-hint"]').exists()).toBe(
      true
    );
  });

  it("explanatory 产物不提供接受入口（不得转为正式业务事实）", async () => {
    const wrapper = mountShell(VizArtifactFrame, {
      props: { artifact: artifact({ nature: "explanatory" }), actionable: true }
    });
    await settle();
    expect(
      wrapper.get('[data-testid="viz-accept"]').attributes("disabled")
    ).toBeDefined();
  });

  it("业务产物提供接受/拒绝入口", async () => {
    const wrapper = mountShell(VizArtifactFrame, {
      props: { artifact: artifact(), actionable: true }
    });
    await settle();
    await wrapper.get('[data-testid="viz-accept"]').trigger("click");
    await wrapper.get('[data-testid="viz-reject"]').trigger("click");
    expect(wrapper.emitted("accept")).toBeTruthy();
    expect(wrapper.emitted("reject")).toBeTruthy();
  });
});
