// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import TimelineView from "./TimelineView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type { DependencyGraph } from "@/api/task-plans";

/**
 * 时间线（S04-UI-06，接真）行为测试：
 * 任务条几何来自 delivery.task 的**生效排期**（planned_start_at /
 * planned_end_at / estimate_hours）；**未排期任务必须进未排期托盘**，
 * 不得静默丢弃。
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
        title: "已排期任务",
        status: "draft",
        priority: "normal",
        planned_start_at: "2026-11-20T09:00:00Z",
        planned_end_at: "2026-11-22T18:00:00Z",
        estimate_hours: 16,
        repository_name: "core-api"
      },
      {
        id: "bbbbbbbb-2222-2222-2222-222222222222",
        title: "未排期任务",
        status: "draft",
        priority: "low",
        planned_start_at: null,
        planned_end_at: null,
        estimate_hours: null,
        repository_name: null
      }
    ],
    edges: [],
    layers: [
      [
        "aaaaaaaa-1111-1111-1111-111111111111",
        "bbbbbbbb-2222-2222-2222-222222222222"
      ]
    ],
    critical_path: [],
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
  await router.push("/project/tasks/planning/timeline");
  await router.isReady();
  const { wrapper } = mountShell(
    TimelineView,
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

describe("TimelineView", () => {
  it("已排期任务出任务条，未排期任务进托盘", async () => {
    const { wrapper } = await mountView();

    expect(getDependencyGraph).toHaveBeenCalledWith("proj-1");
    expect(wrapper.find('[data-testid="tl-board"]').exists()).toBe(true);
    const tray = wrapper.find('[data-testid="tl-tray"]');
    expect(tray.exists()).toBe(true);
    expect(tray.text()).toContain("未排期任务");
  });

  it("排期校验报出未排期任务", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="tl-validate"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-testid="tl-validation"]').exists()).toBe(true);
  });
});
