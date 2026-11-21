// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import CollaborationExecutionView from "./CollaborationExecutionView.vue";

/**
 * PA-06 屏 B（AI execution detail）契约：
 *   - 无进行中执行 → 诚实空态；有任务 → 自动选首个并渲染
 *     眉行/状态徽标/状态机链/依赖分解/产物与验证/活动记录；
 *   - 状态链=PA-06 执行状态模型（running→Executing 高亮，不编造步骤）；
 *   - 右栏 Why AI is doing this + Pause 能力缺口如实标注（不渲染假按钮）。
 */
const mountedWrappers: ReturnType<typeof mountShell>[] = [];

const listTasks = vi.fn();
const getTaskDetail = vi.fn();
const listAgents = vi.fn();
const fetchConsultations = vi.fn();
const fetchPendingDecisions = vi.fn();

vi.mock("@/api/pm", () => ({
  listTasks: (...args: unknown[]) => listTasks(...args),
  decideApproval: vi.fn()
}));

vi.mock("@/api/task-plans", () => ({
  getTaskDetail: (...args: unknown[]) => getTaskDetail(...args)
}));

vi.mock("@/api/execution-plans", () => ({
  listAgents: (...args: unknown[]) => listAgents(...args)
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

function taskRow(overrides: Record<string, unknown> = {}) {
  return {
    id: "task-1",
    code: "PM-101",
    project_id: "p-1",
    milestone_id: null,
    title: "Incremental sync engine",
    description: null,
    status: "running",
    priority: "normal",
    assignee_id: "agent-1",
    revision: 1,
    planned_end_at: null,
    products: [],
    latest_run: null,
    deps_total: 0,
    deps_met: 0,
    product_blocked: false,
    created_at: "2026-11-21T09:12:00Z",
    updated_at: "2026-11-21T10:42:00Z",
    ...overrides
  };
}

const taskDetail = {
  task: {
    id: "task-1",
    code: "PM-101",
    project_id: "p-1",
    title: "Incremental sync engine",
    description: "Sync changed records without data loss",
    status: "running",
    priority: "normal",
    assignee_id: "agent-1",
    revision: 1,
    goal: "Synchronize changed records without data loss",
    constraints: {},
    deliverables: [{ path: "src/sync.ts" }],
    git_branch: "feat/sync-engine",
    planned_start_at: null,
    planned_end_at: null,
    estimate_hours: null,
    implementation_baseline_id: null,
    plan_revision_id: null,
    created_at: "2026-11-21T09:12:00Z",
    updated_at: "2026-11-21T10:42:00Z"
  },
  traces: [
    {
      kind: "requirement",
      target_id: "req-12",
      code: "REQ-12",
      title: "Replay-safe sync",
      version: "v3",
      state: "approved"
    }
  ],
  dependencies: [
    {
      id: "task-0",
      title: "PM-100 · Schema mapping",
      status: "completed",
      priority: "normal",
      direction: "upstream"
    },
    {
      id: "task-9",
      title: "PM-102 · Integration validation",
      status: "planned",
      priority: "normal",
      direction: "downstream"
    }
  ],
  repositories: [{ repository_id: "repo-1" }],
  criteria: [
    { id: "ac-1", ordinal: 1, text: "Replay-safe", verification_kind: "manual" }
  ],
  approvals: [],
  history: [
    {
      id: "h-1",
      actor_id: "agent-1",
      actor_name: "Dev Agent 1",
      action: "Executing checkpoint replay",
      created_at: "2026-11-21T10:42:00Z"
    }
  ],
  baseline: null
};

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/collaboration/execution?project_id=p-1");
  await router.isReady();
  const wrapper = mountShell(CollaborationExecutionView, {
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
  listTasks.mockResolvedValue({ items: [taskRow()], total: 1 });
  getTaskDetail.mockResolvedValue(taskDetail);
  listAgents.mockResolvedValue([
    { kind: "principal", id: "agent-1", name: "Dev Agent 1" }
  ]);
  fetchConsultations.mockResolvedValue([]);
  fetchPendingDecisions.mockResolvedValue([]);
});

describe("CollaborationExecutionView", () => {
  it("auto-selects the first open task and renders detail sections", async () => {
    const wrapper = await mountAt();
    expect(getTaskDetail).toHaveBeenCalledWith("task-1");
    expect(wrapper.text()).toContain("PM-101 · Incremental sync engine");
    expect(wrapper.text()).toContain(
      "Synchronize changed records without data loss"
    );
    expect(wrapper.text()).toContain("Dev Agent 1 · coordinated by AI-PM");
    expect(wrapper.find('[data-testid="ex-plan"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="ex-decomposition"]').exists()).toBe(
      true
    );
    expect(wrapper.find('[data-testid="ex-dep-upstream"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="ex-dep-downstream"]').exists()).toBe(
      true
    );
    expect(wrapper.find('[data-testid="ex-artifacts"]').text()).toContain(
      "Acceptance criteria × 1"
    );
    expect(wrapper.find('[data-testid="ex-activity"]').text()).toContain(
      "Executing checkpoint replay"
    );
  });

  it("highlights the current state on the PA-06 state chain", async () => {
    const wrapper = await mountAt();
    const current = wrapper.find(".ex-chain__state.is-current");
    expect(current.exists()).toBe(true);
    expect(current.text()).toBe("Executing");
  });

  it("renders the context panel with honest pause-capability gap note", async () => {
    const wrapper = await mountAt();
    expect(wrapper.text()).toContain("Why AI is doing this");
    expect(wrapper.text()).toContain("Within authorization");
    expect(wrapper.text()).toContain("not shipped yet");
    // 不得伪装能力：无 Pause task 按钮
    expect(wrapper.text()).not.toContain("Pause task");
  });

  it("shows the honest empty state when no open execution exists", async () => {
    listTasks.mockResolvedValue({ items: [], total: 0 });
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="screen-feedback-empty"]').exists()).toBe(
      true
    );
    expect(getTaskDetail).not.toHaveBeenCalled();
  });
});
