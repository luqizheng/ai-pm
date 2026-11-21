// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import PlanningView from "./PlanningView.vue";

/**
 * PA-06 屏 E（Task planning & dependencies）契约：
 *   - 依赖分层图=dependency-graph 真实 layers（层行×节点卡，waiting 节点
 *     accent 底）；头部统计（节点/并行组/关键路径）；
 *   - Start authorized execution=markTasksReady（E5 真人闸批量 ready 化），
 *     无 draft 任务时禁用；计划调整活动=项目 feed 真实条目（版本史无读面，
 *     不编造 vN）。
 */
const mountedWrappers: ReturnType<typeof mountShell>[] = [];

const listTasks = vi.fn();
const markTasksReady = vi.fn();
const getDependencyGraph = vi.fn();
const fetchProjectFeed = vi.fn();
const fetchConsultations = vi.fn();
const fetchPendingDecisions = vi.fn();

vi.mock("@/api/pm", () => ({
  listTasks: (...args: unknown[]) => listTasks(...args),
  decideApproval: vi.fn()
}));

vi.mock("@/api/tasks", () => ({
  markTasksReady: (...args: unknown[]) => markTasksReady(...args)
}));

vi.mock("@/api/task-plans", () => ({
  getDependencyGraph: (...args: unknown[]) => getDependencyGraph(...args)
}));

vi.mock("@/api/feeds", () => ({
  fetchProjectFeed: (...args: unknown[]) => fetchProjectFeed(...args)
}));

vi.mock("@/api/collaboration", () => ({
  fetchConsultations: (...args: unknown[]) => fetchConsultations(...args),
  fetchPendingDecisions: (...args: unknown[]) => fetchPendingDecisions(...args),
  fetchDecisionCenter: vi.fn(),
  fetchProjectRisks: vi.fn(),
  fetchWaitingDecisionCount: vi.fn().mockResolvedValue(0),
  decideConsultation: vi.fn(),
  cancelConsultation: vi.fn()
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

function node(overrides: Record<string, unknown> = {}) {
  return {
    id: "task-1",
    title: "PM-101 Schema mapping",
    status: "completed",
    priority: "normal",
    planned_start_at: null,
    planned_end_at: null,
    estimate_hours: null,
    repository_name: null,
    ...overrides
  };
}

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/collaboration/plan?project_id=p-1");
  await router.isReady();
  const wrapper = mountShell(PlanningView, {
    global: { plugins: [router] }
  });
  (mountedWrappers as unknown as VueWrapper[]).push(wrapper as VueWrapper);
  await flushPromises();
  await flushPromises();
  return wrapper;
}

afterEach(() => {
  for (const wrapper of mountedWrappers.splice(0)) {
    wrapper.unmount();
  }
  document.body.innerHTML = "";
});

beforeEach(() => {
  vi.clearAllMocks();
  i18n.global.locale.value = "en";
  getDependencyGraph.mockResolvedValue({
    nodes: [
      node(),
      node({
        id: "task-2",
        title: "PM-102 Sync engine",
        status: "running"
      }),
      node({
        id: "task-3",
        title: "PM-103 Audit export",
        status: "blocked"
      })
    ],
    edges: [],
    layers: [["task-1"], ["task-2", "task-3"]],
    critical_path: ["task-1", "task-2"],
    parallel_groups: [["task-2", "task-3"]],
    cycles: []
  });
  listTasks.mockResolvedValue({
    items: [
      taskRowOf("task-4", "draft"),
      taskRowOf("task-5", "draft"),
      taskRowOf("task-1", "completed")
    ],
    total: 3
  });
  fetchProjectFeed.mockResolvedValue([
    {
      id: "act-1",
      summary: "AI-PM reordered integration checks",
      created_at: "2026-11-21T10:24:00Z"
    }
  ]);
  fetchConsultations.mockResolvedValue([]);
  fetchPendingDecisions.mockResolvedValue([]);
});

function taskRowOf(id: string, status: string) {
  return {
    id,
    code: `PM-${id.slice(-3)}`,
    project_id: "p-1",
    milestone_id: null,
    title: `Task ${id}`,
    description: null,
    status,
    priority: "normal",
    assignee_id: null,
    revision: 1,
    planned_end_at: null,
    products: [],
    latest_run: null,
    deps_total: 0,
    deps_met: 0,
    product_blocked: false,
    created_at: "2026-11-21T09:00:00Z",
    updated_at: "2026-11-21T10:00:00Z"
  };
}

describe("PlanningView", () => {
  it("renders dependency layers with waiting nodes accented", async () => {
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="pl-layer-0"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="pl-layer-1"]').exists()).toBe(true);
    const waiting = wrapper.find('[data-testid="pl-node-task-3"]');
    expect(waiting.classes()).toContain("is-waiting");
    expect(wrapper.text()).toContain("3 nodes · 1 parallel groups");
    expect(wrapper.text()).toContain("critical path 2 steps");
  });

  it("starts authorized execution by batching draft tasks to ready", async () => {
    markTasksReady.mockResolvedValue({ updated: 2 });
    const wrapper = await mountAt();
    const start = wrapper.find('[data-testid="pl-start"]');
    expect((start.element as HTMLButtonElement).disabled).toBe(false);
    expect(start.text()).toContain("(2 draft → ready)");
    await start.trigger("click");
    await flushPromises();
    await flushPromises();
    expect(markTasksReady).toHaveBeenCalledWith("p-1", ["task-4", "task-5"]);
  });

  it("disables the start action when no draft tasks exist", async () => {
    listTasks.mockResolvedValue({
      items: [taskRowOf("task-1", "completed")],
      total: 1
    });
    const wrapper = await mountAt();
    const start = wrapper.find('[data-testid="pl-start"]');
    expect((start.element as HTMLButtonElement).disabled).toBe(true);
    expect(start.text()).toContain("no tasks awaiting confirmation");
  });

  it("renders real activity lines instead of a fabricated plan version history", async () => {
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="pl-history"]').text()).toContain(
      "AI-PM reordered integration checks"
    );
  });
});
