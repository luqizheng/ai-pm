// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";

import { mountShell } from "@/lib/test-utils";
import type { VizArtifact } from "@/lib/viz/types";

import VizVersionBar from "./VizVersionBar.vue";

function artifact(id: string, version: number): VizArtifact {
  return {
    id,
    project_id: "project-1",
    slot_key: "req-trace:R-042",
    version_no: version,
    state: "published",
    nature: "business",
    spec: {
      spec_version: "1.0",
      slot_key: "req-trace:R-042",
      view: { kind: "trace-graph", focus: [], lenses: [] },
      nodes: [],
      edges: [],
      meta: { budget: "balanced" }
    },
    spec_hash: "h",
    source_hash: "s",
    created_at: "2026-11-12T00:00:00Z"
  };
}

afterEach(() => {
  document.body.innerHTML = "";
});

describe("VizVersionBar", () => {
  it("列出历史版本（发布版不可变，只做选择）", () => {
    const current = artifact("a3", 3);
    const wrapper = mountShell(VizVersionBar, {
      props: {
        artifact: current,
        history: [artifact("a2", 2), artifact("a1", 1)]
      }
    });

    const options = wrapper.findAll("option");
    expect(options).toHaveLength(3);
    expect(options[0].text()).toContain("v3");
  });

  it("切换版本时回传目标版本 id", async () => {
    const wrapper = mountShell(VizVersionBar, {
      props: { artifact: artifact("a3", 3), history: [artifact("a2", 2)] }
    });

    const select = wrapper.get('[data-testid="viz-version-select"]');
    await select.setValue("a2");
    const emitted = wrapper.emitted("selectVersion");
    expect(emitted?.[0][0]).toBe("a2");
  });

  it("只有一个版本时禁用对比", () => {
    const wrapper = mountShell(VizVersionBar, {
      props: { artifact: artifact("a1", 1) }
    });
    expect(
      wrapper.get('[data-testid="viz-compare"]').attributes("disabled")
    ).toBeDefined();
  });

  it("导出 SVG 恒可用；explanatory 产物不提供 HTML 导出", async () => {
    const wrapper = mountShell(VizVersionBar, {
      props: { artifact: artifact("a1", 1), explanatory: true }
    });
    expect(wrapper.find('[data-testid="viz-export-svg"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="viz-export-html"]').exists()).toBe(
      false
    );

    await wrapper.get('[data-testid="viz-export-svg"]').trigger("click");
    expect(wrapper.emitted("exportArtifact")?.[0][0]).toBe("svg");
  });
});
