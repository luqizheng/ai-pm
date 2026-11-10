// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import DesignAnalysisView from "./DesignAnalysisView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type { DesignAnalysisRow } from "@/api/designs";

/**
 * S03 分析页行为测试：
 * 需求覆盖与「过期依据」标记来自 /v1/pm/design-analysis 真实读模型——
 * 过期绑定（is_current=false）必须显式标出，不得当成已覆盖。
 */

const listDesignAnalysis = vi.fn();
const getDesignWorkspace = vi.fn();

vi.mock("@/api/designs", () => ({
  listDesignAnalysis: (...args: unknown[]) => listDesignAnalysis(...args),
  getDesignWorkspace: (...args: unknown[]) => getDesignWorkspace(...args)
}));

function analysisRow(
  overrides: Partial<DesignAnalysisRow> = {}
): DesignAnalysisRow {
  return {
    design_id: "design-1",
    code: "TD-001",
    title: "登录设计",
    lifecycle_status: "active",
    current_revision_id: "rev-1",
    current_version: 2,
    current_state: "approved",
    requirement_count: 1,
    stale_requirement_count: 0,
    open_finding_count: 0,
    requirements: [
      {
        design_id: "design-1",
        design_revision_id: "rev-1",
        requirement_id: "req-1",
        requirement_revision_id: "req-rev-1",
        requirement_code: "REQ-001",
        requirement_title: "登录需求",
        requirement_version: 3,
        requirement_state: "approved",
        is_current: true
      }
    ],
    adrs: [],
    ...overrides
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/design");
  await router.isReady();
  const { wrapper } = mountShell(
    DesignAnalysisView,
    { attachTo: document.body },
    router
  );
  const shell = useShellStore();
  shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
  shell.setCurrentProject("proj-1");
  await flushPromises();
  return { wrapper, router };
}

beforeEach(() => {
  listDesignAnalysis.mockReset();
  getDesignWorkspace.mockReset();
  getDesignWorkspace.mockResolvedValue({
    design_count: 1,
    adr_count: 0,
    baseline_count: 0,
    pending_approval_count: 0,
    open_finding_count: 0,
    linked_requirement_count: 1,
    stale_requirement_count: 0,
    design_states: {},
    adr_states: {}
  });
});

describe("DesignAnalysisView", () => {
  it("按项目读需求覆盖与影响关系，并标出过期依据", async () => {
    listDesignAnalysis.mockResolvedValue([
      analysisRow(),
      analysisRow({
        design_id: "design-2",
        code: "TD-002",
        title: "过期设计",
        stale_requirement_count: 1,
        requirements: [
          {
            ...analysisRow().requirements[0],
            design_id: "design-2",
            is_current: false
          }
        ]
      })
    ]);

    const { wrapper } = await mountView();

    expect(listDesignAnalysis).toHaveBeenCalledWith("proj-1");
    const text = wrapper.text();
    expect(text).toContain("REQ-001");
    expect(text).toContain("TD-002");
    // 过期绑定必须显式提示，不可静默当作已覆盖
    expect(
      wrapper.find('[data-testid="analysis-row-TD-002"]').text()
    ).toContain("过期");
  });

  it("无项目上下文时给空态，不发项目级请求", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });
    await router.push("/project/design");
    await router.isReady();
    const { wrapper } = mountShell(
      DesignAnalysisView,
      { attachTo: document.body },
      router
    );
    await flushPromises();

    expect(listDesignAnalysis).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("请先选择项目");
  });
});
