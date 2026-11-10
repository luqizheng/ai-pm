// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import DependencyGraphView from "./DependencyGraphView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type { DependencyGraph } from "@/api/task-plans";

/**
 * 依赖图（S04-UI-05，接真）行为测试：
 * 节点 / 边 / 分层 / 关键路径 / 并行组 / 环检测全部来自
 * GET /projects/{id}/dependency-graph 的真实投影——不在前端重算拓扑。
 */

const getDependencyGraph = vi.fn();

vi.mock("@/api/task-plans", async () => {
  const actual =
    await vi.importActual<typeof import("@/api/task-plans")>(
      "@/api/task-plans"
    );
  return {
    ...actual,
    getDependencyGraph: (...args: unknown[]) => getDependencyGraph(...args)
  };
});

function graph(overrides: Partial<DependencyGraph> = {}): DependencyGraph {
  return {
    nodes: [
      {
        id: "aaaaaaaa-1111-1111-1111-111111111111",
        title: "前置任务",
        status: "completed",
        priority: "normal",
        planned_start_at: null,
        planned_end_at: null,
        estimate_hours: null,
        repository_name: "core-api"
      },
      {
        id: "bbbbbbbb-2222-2222-2222-222222222222",
        title: "后续任务",
        status: "draft",
        priority: "high",
        planned_start_at: null,
        planned_end_at: null,
        estimate_hours: 8,
        repository_name: null
      }
    ],
    edges: [
      {
        from_task_id: "aaaaaaaa-1111-1111-1111-111111111111",
        to_task_id: "bbbbbbbb-2222-2222-2222-222222222222",
        dependency_type: "FS"
      }
    ],
    layers: [
      ["aaaaaaaa-1111-1111-1111-111111111111"],
      ["bbbbbbbb-2222-2222-2222-222222222222"]
    ],
    critical_path: [
      "aaaaaaaa-1111-1111-1111-111111111111",
      "bbbbbbbb-2222-2222-2222-222222222222"
    ],
    parallel_groups: [],
    cycles: [],
    ...overrides
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/tasks/planning/graph");
  await router.isReady();
  const { wrapper } = mountShell(
    DependencyGraphView,
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
  getDependencyGraph.mockReset();
  getDependencyGraph.mockResolvedValue(graph());
});

describe("DependencyGraphView", () => {
  it("读真实依赖图端点并渲染节点与依赖边", async () => {
    const { wrapper } = await mountView();

    expect(getDependencyGraph).toHaveBeenCalledWith("proj-1");
    const text = wrapper.text();
    expect(text).toContain("前置任务");
    expect(text).toContain("后续任务");
    expect(wrapper.find('[data-testid="pg-canvas"]').exists()).toBe(true);
  });

  it("有环依赖时校验面板报出，而不是静默渲染", async () => {
    getDependencyGraph.mockResolvedValue(
      graph({
        cycles: [
          [
            "aaaaaaaa-1111-1111-1111-111111111111",
            "bbbbbbbb-2222-2222-2222-222222222222"
          ]
        ]
      })
    );
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="pg-validate"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-testid="pg-validation"]').exists()).toBe(true);
  });
});
