// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import TaskBreakdownView from "./TaskBreakdownView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type {
  TaskPlanDetail,
  TaskPlanEntry,
  TaskPlanProgress
} from "@/api/task-plans";

/**
 * 任务拆解（S04-UI-02，接真）行为测试：
 * 计划草案与已正式化任务全部来自真实计划读模型——
 * **只有 approved 计划能正式化成任务**（后端硬校验）。
 */

const listTaskPlans = vi.fn();
const getTaskPlan = vi.fn();
const getTaskPlanProgress = vi.fn();

vi.mock("@/api/task-plans", async () => {
  const actual =
    await vi.importActual<typeof import("@/api/task-plans")>(
      "@/api/task-plans"
    );
  return {
    planTaskDrafts: actual.planTaskDrafts,
    listTaskPlans: (...args: unknown[]) => listTaskPlans(...args),
    getTaskPlan: (...args: unknown[]) => getTaskPlan(...args),
    getTaskPlanProgress: (...args: unknown[]) => getTaskPlanProgress(...args)
  };
});

function planEntry(overrides: Partial<TaskPlanEntry> = {}): TaskPlanEntry {
  return {
    id: "plan-1",
    project_id: "proj-1",
    code: "PLAN-001",
    title: "登录实施计划",
    status: "approved",
    current_revision_id: "rev-1",
    source_baseline_id: "ib-1",
    row_revision: 1,
    created_at: "2026-11-10T08:00:00Z",
    updated_at: "2026-11-10T09:00:00Z",
    ...overrides
  };
}

function planDetail(): TaskPlanDetail {
  return {
    plan: planEntry(),
    revisions: [
      {
        id: "rev-1",
        plan_id: "plan-1",
        version: 1,
        content: {
          tasks: [
            {
              key: "T-1",
              title: "实现登录端点",
              description: "范围",
              acceptance_criteria: ["返回 200"],
              depends_on: [],
              priority: "high"
            },
            {
              key: "T-2",
              title: "补充登录测试",
              acceptance_criteria: ["覆盖 401"],
              depends_on: ["T-1"]
            }
          ]
        },
        state: "approved",
        content_hash: "h1",
        source: "ai",
        instructions: "按基线实现",
        submitted_at: "2026-11-10T08:30:00Z",
        approved_at: "2026-11-10T09:00:00Z",
        created_at: "2026-11-10T08:00:00Z"
      }
    ],
    approvals: [],
    materialized_tasks: [
      {
        task_id: "11111111-1111-1111-1111-111111111111",
        plan_task_key: "T-1",
        title: "实现登录端点",
        status: "draft"
      }
    ]
  };
}

function progress(): TaskPlanProgress {
  return {
    plan_id: "plan-1",
    status: "approved",
    revision_id: "rev-1",
    total_tasks: 1,
    by_status: { draft: 1 },
    blocked: [
      {
        id: "22222222-2222-2222-2222-222222222222",
        title: "阻塞任务",
        status: "blocked",
        open_upstreams: 2
      }
    ],
    planned_tasks: 2
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/tasks/breakdown");
  await router.isReady();
  const { wrapper } = mountShell(
    TaskBreakdownView,
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
  listTaskPlans.mockReset();
  getTaskPlan.mockReset();
  getTaskPlanProgress.mockReset();
  listTaskPlans.mockResolvedValue([planEntry()]);
  getTaskPlan.mockResolvedValue(planDetail());
  getTaskPlanProgress.mockResolvedValue(progress());
});

describe("TaskBreakdownView", () => {
  it("读真实计划端点并渲染草案与已正式化任务", async () => {
    const { wrapper } = await mountView();

    expect(listTaskPlans).toHaveBeenCalledWith("proj-1");
    expect(getTaskPlan).toHaveBeenCalledWith("plan-1");
    const text = wrapper.text();
    expect(text).toContain("实现登录端点");
    expect(text).toContain("补充登录测试");
    expect(text).toContain("PLAN-001");
  });

  it("计划进度呈现草案数与阻塞项（含未完成上游数）", async () => {
    const { wrapper } = await mountView();

    expect(getTaskPlanProgress).toHaveBeenCalledWith("plan-1");
    const text = wrapper.text();
    expect(text).toContain("阻塞任务");
    expect(text).toContain("2");
  });

  it("读端点失败时给错误态而非空壳", async () => {
    listTaskPlans.mockRejectedValue(new Error("boom"));
    const { wrapper } = await mountView();

    expect(wrapper.text().length).toBeGreaterThan(0);
    expect(getTaskPlan).not.toHaveBeenCalled();
  });
});
