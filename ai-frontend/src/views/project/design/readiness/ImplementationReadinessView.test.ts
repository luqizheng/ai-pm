// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import ImplementationReadinessView from "./ImplementationReadinessView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type { ImplementationBaseline } from "@/api/designs";

/**
 * S03 实施准备页行为测试：
 * 三个层级必须可区分——**门禁判定 ≠ 已冻结基线 ≠ 已交接 S04**。
 * 门禁未过时不得显示「就绪」；交接区只列真正以该基线为依据的任务。
 */

const evaluateReadiness = vi.fn();
const listImplementationBaselines = vi.fn();
const listBaselineTasks = vi.fn();
const getDesignWorkspace = vi.fn();

vi.mock("@/api/designs", () => ({
  evaluateReadiness: (...args: unknown[]) => evaluateReadiness(...args),
  listImplementationBaselines: (...args: unknown[]) =>
    listImplementationBaselines(...args),
  listBaselineTasks: (...args: unknown[]) => listBaselineTasks(...args),
  getDesignWorkspace: (...args: unknown[]) => getDesignWorkspace(...args)
}));

function baseline(
  overrides: Partial<ImplementationBaseline> = {}
): ImplementationBaseline {
  return {
    id: "ib-1",
    project_id: "proj-1",
    code: "IB-001",
    status: "frozen",
    content: {},
    content_hash: "hash-1",
    readiness: {},
    frozen_by: "p-1",
    frozen_at: "2026-11-10T09:00:00Z",
    superseded_by_baseline_id: null,
    created_at: "2026-11-10T09:00:00Z",
    ...overrides
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/design/readiness");
  await router.isReady();
  const { wrapper } = mountShell(
    ImplementationReadinessView,
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
  evaluateReadiness.mockReset();
  listImplementationBaselines.mockReset();
  listBaselineTasks.mockReset();
  getDesignWorkspace.mockReset();
  getDesignWorkspace.mockResolvedValue({
    design_count: 1,
    adr_count: 0,
    baseline_count: 1,
    pending_approval_count: 0,
    open_finding_count: 0,
    linked_requirement_count: 1,
    stale_requirement_count: 0,
    design_states: {},
    adr_states: {}
  });
  listImplementationBaselines.mockResolvedValue([baseline()]);
  listBaselineTasks.mockResolvedValue([]);
});

describe("ImplementationReadinessView", () => {
  it("门禁未过时逐条呈现未通过条件，不显示就绪", async () => {
    evaluateReadiness.mockResolvedValue({
      ready: false,
      design_revision_ids: ["rev-1"],
      conditions: [
        { condition: "design_approved", passed: true, details: [] },
        {
          condition: "requirements_confirmed",
          passed: false,
          details: ["requirement REQ-001 moved past linked revision"]
        }
      ]
    });

    const { wrapper } = await mountView();

    expect(evaluateReadiness).toHaveBeenCalledWith("proj-1");
    expect(
      wrapper.find('[data-testid="readiness-gate-verdict"]').text()
    ).toContain("未就绪");
    expect(
      wrapper
        .find('[data-testid="gate-condition-requirements_confirmed"]')
        .text()
    ).toContain("REQ-001");
  });

  it("门禁通过且已冻结基线时呈现基线与交接任务", async () => {
    evaluateReadiness.mockResolvedValue({
      ready: true,
      design_revision_ids: ["rev-1"],
      conditions: [{ condition: "design_approved", passed: true, details: [] }]
    });
    listBaselineTasks.mockResolvedValue([
      {
        id: "task-1",
        title: "实现登录端点",
        status: "draft",
        priority: "normal",
        created_at: "2026-11-10T10:00:00Z"
      }
    ]);

    const { wrapper } = await mountView();

    expect(
      wrapper.find('[data-testid="readiness-gate-verdict"]').text()
    ).toContain("已就绪");
    expect(wrapper.find('[data-testid="baseline-IB-001"]').exists()).toBe(true);
    expect(
      wrapper.find('[data-testid="handoff-task-task-1"]').text()
    ).toContain("实现登录端点");
  });
});
