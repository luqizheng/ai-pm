// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import TasksOverviewView from "./TasksOverviewView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type { Task } from "@/api/types";

/**
 * 任务总览（S04-UI-01）行为测试：
 * 统计聚合来自 /v1/pm/tasks 真实读模型；统计卡点击跳转任务池预过滤；
 * 待人工处理事项由列表派生。
 */

const listTasks = vi.fn();

vi.mock("@/api/tasks", () => ({
  listTasks: (...args: unknown[]) => listTasks(...args)
}));

function task(overrides: Partial<Task>): Task {
  return {
    id: "11111111-1111-1111-1111-111111111111",
    project_id: "proj-1",
    milestone_id: null,
    title: "实现登录端点",
    description: null,
    status: "draft",
    priority: "high",
    assignee_id: null,
    revision: 1,
    created_at: "2026-11-10T08:00:00Z",
    updated_at: "2026-11-10T08:00:00Z",
    ...overrides
  };
}

function buildTestRouter() {
  return createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
}

async function mountView() {
  const router = buildTestRouter();
  await router.push("/project/tasks");
  await router.isReady();
  const wrapper = mountShell(
    TasksOverviewView,
    { attachTo: document.body },
    router
  ).wrapper;
  // 项目上下文（视图经 shell store 取 currentProjectId），须在 mount 后取
  const shell = useShellStore();
  shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
  shell.setCurrentProject("proj-1");
  await flushPromises();
  return { wrapper, router };
}

beforeEach(() => {
  listTasks.mockReset();
});

describe("TasksOverviewView", () => {
  it("按 /v1/pm/tasks 列表聚合统计卡，并携带项目与分页参数请求", async () => {
    listTasks.mockResolvedValue({
      items: [
        task({ id: "a1", status: "draft" }),
        task({ id: "a2", status: "draft" }),
        task({ id: "a3", status: "ready", assignee_id: "p-1" }),
        task({ id: "a4", status: "blocked" })
      ],
      total: 4,
      page: 1,
      size: 200
    });

    const { wrapper } = await mountView();

    expect(listTasks).toHaveBeenCalledWith({
      project_id: "proj-1",
      page: 1,
      size: 200
    });
    expect(wrapper.find('[data-testid="ov-stat-total"]').text()).toContain("4");
    expect(wrapper.find('[data-testid="ov-stat-draft"]').text()).toContain("2");
    expect(wrapper.find('[data-testid="ov-stat-ready"]').text()).toContain("1");
    expect(wrapper.find('[data-testid="ov-stat-blocked"]').text()).toContain(
      "1"
    );
    expect(wrapper.find('[data-testid="ov-stat-unassigned"]').text()).toContain(
      "3"
    );
  });

  it("点击统计卡跳转任务池并携带预过滤 query", async () => {
    listTasks.mockResolvedValue({ items: [task({ id: "a1" })], total: 1 });
    const { wrapper, router } = await mountView();

    await wrapper.find('[data-testid="ov-stat-blocked"]').trigger("click");
    await flushPromises();

    expect(router.currentRoute.value.path).toBe("/project/tasks/backlog");
    expect(router.currentRoute.value.query.status).toBe("blocked");
  });

  it("待人工处理事项从列表派生（草稿待确认 / 无负责人）", async () => {
    listTasks.mockResolvedValue({
      items: [
        task({ id: "a1", status: "draft" }),
        task({ id: "a2", status: "ready" })
      ],
      total: 2
    });

    const { wrapper } = await mountView();
    const text = wrapper.find(".ov-attention").text();

    expect(text).toContain("1");
    expect(wrapper.findAll(".ov-attention__item").length).toBe(2);
  });

  it("加载失败渲染错误告警并提供刷新", async () => {
    listTasks.mockRejectedValue(new Error("network"));
    const { wrapper } = await mountView();

    expect(wrapper.findComponent({ name: "Alert" }).exists()).toBe(true);
  });
});
