// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import TasksBacklogView from "./TasksBacklogView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import { HttpError } from "@digua/http";
import type { Task } from "@/api/types";

/**
 * 任务池（S04-UI-04）行为测试：接真 /v1/pm/tasks 系端点——
 * 列表渲染、新建、批量 mark-ready（仅草稿计数）、编辑 409 冲突提示。
 * Dialog/ConfirmationDialog 走 Teleport，断言经 document.body。
 */

const listTasks = vi.fn();
const createTask = vi.fn();
const updateTask = vi.fn();
const markTasksReady = vi.fn();
const listTaskDependencies = vi.fn();
const addTaskDependency = vi.fn();
const removeTaskDependency = vi.fn();

vi.mock("@/api/tasks", () => ({
  listTasks: (...args: unknown[]) => listTasks(...args),
  createTask: (...args: unknown[]) => createTask(...args),
  updateTask: (...args: unknown[]) => updateTask(...args),
  markTasksReady: (...args: unknown[]) => markTasksReady(...args),
  listTaskDependencies: (...args: unknown[]) => listTaskDependencies(...args),
  addTaskDependency: (...args: unknown[]) => addTaskDependency(...args),
  removeTaskDependency: (...args: unknown[]) => removeTaskDependency(...args)
}));

const DRAFT_ID = "11111111-1111-1111-1111-111111111111";
const READY_ID = "22222222-2222-2222-2222-222222222222";

function task(overrides: Partial<Task>): Task {
  return {
    id: DRAFT_ID,
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

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/tasks/backlog");
  await router.isReady();
  const wrapper = mountShell(
    TasksBacklogView,
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

function bodyButton(label: string): HTMLButtonElement | null {
  const buttons = Array.from(document.body.querySelectorAll("button"));
  return buttons.find(node => node.textContent?.trim() === label) ?? null;
}

/** ui/Input 的 attrs 落在外壳 div 上，穿透到内部原生 input 再派发输入事件 */
function setBodyInput(testid: string, value: string) {
  const host = document.body.querySelector(`[data-testid="${testid}"]`);
  const input = (
    host?.matches("input") ? host : host?.querySelector("input")
  ) as HTMLInputElement | null;
  if (!input) throw new Error(`input not found: ${testid}`);
  input.value = value;
  input.dispatchEvent(new Event("input"));
}

beforeEach(() => {
  listTasks.mockReset().mockResolvedValue({
    items: [
      task({ id: DRAFT_ID, title: "实现登录端点" }),
      task({
        id: READY_ID,
        title: "构建登录表单",
        status: "ready",
        assignee_id: "p-1"
      })
    ],
    total: 2,
    page: 1,
    size: 50
  });
  createTask.mockReset();
  updateTask.mockReset();
  markTasksReady.mockReset();
});

describe("TasksBacklogView", () => {
  it("渲染任务行并携带项目参数请求列表", async () => {
    const { wrapper } = await mountView();

    expect(listTasks).toHaveBeenCalledWith({
      project_id: "proj-1",
      page: 1,
      size: 50
    });
    expect(wrapper.text()).toContain("实现登录端点");
    expect(wrapper.text()).toContain("构建登录表单");
    expect(wrapper.find('[data-status="ready"]').exists()).toBe(true);
  });

  it("新建任务走 createTask 并回刷列表", async () => {
    createTask.mockResolvedValue(task({}));
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="backlog-create"]').trigger("click");
    await flushPromises();
    setBodyInput("backlog-form-title", "实现令牌校验");
    await flushPromises();

    await bodyButton("创建")?.click();
    await flushPromises();

    expect(createTask).toHaveBeenCalledWith(
      expect.objectContaining({ project_id: "proj-1", title: "实现令牌校验" })
    );
    expect(listTasks.mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it("批量置待执行仅统计草稿任务，并调用 mark-ready 端点", async () => {
    markTasksReady.mockResolvedValue({
      result: { ready: [DRAFT_ID], already_ready: [], skipped: [] }
    });
    const { wrapper } = await mountView();

    expect(wrapper.find('[data-testid="backlog-bulk"]').exists()).toBe(false);

    await wrapper.find(`[data-testid="bl-check-11111111"]`).setValue(true);
    await wrapper.find(`[data-testid="bl-check-22222222"]`).setValue(true);
    await flushPromises();

    expect(wrapper.find('[data-testid="backlog-bulk"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="backlog-mark-ready"]').text()).toContain(
      "1"
    );

    await wrapper.find('[data-testid="backlog-mark-ready"]').trigger("click");
    await flushPromises();
    const confirmButton = bodyButton("置为待执行");
    expect(confirmButton).not.toBeNull();
    await confirmButton?.click();
    await flushPromises();

    expect(markTasksReady).toHaveBeenCalledWith("proj-1", [DRAFT_ID]);
  });

  it("编辑版本冲突（409）在表单内显式提示", async () => {
    updateTask.mockRejectedValue(new HttpError("conflict", { status: 409 }));
    const { wrapper } = await mountView();

    const editButton = wrapper
      .findAll("button")
      .find(node => node.text().includes("编辑"));
    await editButton?.trigger("click");
    await flushPromises();

    setBodyInput("backlog-form-title", "实现登录端点 v2");
    await flushPromises();

    await bodyButton("保存修改")?.click();
    await flushPromises();

    expect(
      document.body.querySelector('[data-testid="backlog-form-error"]')
        ?.textContent
    ).toContain("冲突");
    expect(updateTask).toHaveBeenCalledWith(
      DRAFT_ID,
      expect.objectContaining({ revision: 1 })
    );
  });
});
