// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import CollaborationOverviewView from "./CollaborationOverviewView.vue";

/**
 * PA-06 屏 A（Project collaboration Overview）契约：
 *   - YOUR ATTENTION：待决徽标 + 摘要卡 + Review 入口；空态=「常规 AI 工作
 *     无需批准继续」（空态不抹历史）；
 *   - AUTONOMOUS EXECUTION：任务×agent×状态徽标×进度行；
 *   - IMPORTANT RISK（仅 open 风险）/ RECENT OUTCOMES（feed 摘要行）；
 *   - 右栏 AI-PM Project context（方向/边界/进展）+ 底部统一 AI-PM 输入条。
 */
const mountedWrappers: ReturnType<typeof mountShell>[] = [];

const fetchDecisionCenter = vi.fn();
const fetchProjectRisks = vi.fn();
const listTasks = vi.fn();
const fetchProjectFeed = vi.fn();
const listAgents = vi.fn();
const getProjectGoal = vi.fn();
const fetchConsultations = vi.fn();
const fetchPendingDecisions = vi.fn();

vi.mock("@/api/collaboration", () => ({
  fetchDecisionCenter: (...args: unknown[]) => fetchDecisionCenter(...args),
  fetchProjectRisks: (...args: unknown[]) => fetchProjectRisks(...args),
  fetchConsultations: (...args: unknown[]) => fetchConsultations(...args),
  fetchPendingDecisions: (...args: unknown[]) => fetchPendingDecisions(...args),
  fetchWaitingDecisionCount: vi.fn().mockResolvedValue(0),
  decideConsultation: vi.fn(),
  cancelConsultation: vi.fn()
}));

vi.mock("@/api/pm", () => ({
  listTasks: (...args: unknown[]) => listTasks(...args),
  decideApproval: vi.fn()
}));

vi.mock("@/api/feeds", () => ({
  fetchProjectFeed: (...args: unknown[]) => fetchProjectFeed(...args)
}));

vi.mock("@/api/execution-plans", () => ({
  listAgents: (...args: unknown[]) => listAgents(...args)
}));

vi.mock("@/api/goal", () => ({
  getProjectGoal: (...args: unknown[]) => getProjectGoal(...args)
}));

function decisionItem(overrides: Record<string, unknown> = {}) {
  return {
    kind: "consultation",
    id: "c-1",
    kindLabel: "Human consultation · Task",
    title: "Extend audit retention to 90 days",
    meta: "T-ab12 waiting · Raised 10:12",
    createdAt: "2026-11-21T10:12:00Z",
    why: "Changes the meaning of confirmed requirement REQ-18.",
    recommendation: "Retain audit events for 90 days.",
    options: [],
    taskId: null,
    status: "pending",
    decisionContent: null,
    cancelledReason: null,
    ...overrides
  };
}

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
    deps_total: 4,
    deps_met: 2,
    product_blocked: false,
    created_at: "2026-11-20T08:00:00Z",
    updated_at: "2026-11-21T09:00:00Z",
    ...overrides
  };
}

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/collaboration?project_id=p-1");
  await router.isReady();
  const wrapper = mountShell(CollaborationOverviewView, {
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
  fetchDecisionCenter.mockResolvedValue({
    items: [
      decisionItem(),
      decisionItem({
        kind: "approval",
        id: "a-1",
        kindLabel: "AI proposal · create_tasks",
        title: "Approve external EU verification provider",
        meta: "high · Raised 10:28",
        taskId: null
      })
    ],
    pendingCount: 2,
    waitingDecisionCount: 1
  });
  fetchProjectRisks.mockResolvedValue([
    {
      id: "r-1",
      project_id: "p-1",
      title: "External data transfer is blocked by authorization",
      description: null,
      impact: "high",
      probability: "medium",
      status: "open",
      created_at: "2026-11-21T10:00:00Z"
    }
  ]);
  listTasks.mockResolvedValue({
    items: [
      taskRow(),
      taskRow({
        id: "task-2",
        code: "PM-102",
        title: "Conflict-resolution checks",
        status: "reviewing",
        assignee_id: null,
        deps_total: 0
      }),
      taskRow({
        id: "task-3",
        code: "PM-103",
        title: "Migration runbook",
        status: "completed"
      })
    ],
    total: 3
  });
  fetchProjectFeed.mockResolvedValue([
    {
      id: "act-1",
      summary: "T-1425 schema mapping completed",
      created_at: "2026-11-21T10:38:00Z"
    }
  ]);
  listAgents.mockResolvedValue([
    { kind: "principal", id: "agent-1", name: "Dev Agent 1" }
  ]);
  getProjectGoal.mockResolvedValue({
    state: "ready",
    metadata: { revision_no: 3 },
    markdown: "Ship reliable incremental sync for Atlas 2.4 by 27 Nov.\n",
    bodyError: ""
  });
  // 壳页签计数（CollaborationFrame）也走同一 mock 模块
  fetchConsultations.mockResolvedValue([]);
  fetchPendingDecisions.mockResolvedValue([]);
});

describe("CollaborationOverviewView", () => {
  it("renders attention badge, decision rows and review entry", async () => {
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="collab-attention"]').exists()).toBe(
      true
    );
    const badge = wrapper.find('[data-testid="attention-badge"]');
    expect(badge.text()).toContain("2 human decisions");
    const rows = wrapper.findAll('[data-testid="ov-decision-consultation"]');
    expect(rows).toHaveLength(1);
    expect(wrapper.text()).toContain("Extend audit retention to 90 days");
    expect(wrapper.text()).toContain(
      "Human judgment needed. Unaffected work continues."
    );
  });

  it("shows the empty state instead of fake data when nothing is pending", async () => {
    fetchDecisionCenter.mockResolvedValue({
      items: [],
      pendingCount: 0,
      waitingDecisionCount: 0
    });
    const wrapper = await mountAt();
    const empty = wrapper.find('[data-testid="screen-feedback-empty"]');
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain("No human decisions pending");
    expect(
      wrapper.find('[data-testid="ov-decision-consultation"]').exists()
    ).toBe(false);
  });

  it("renders execution rows with agent name, state badge and dependency progress", async () => {
    const wrapper = await mountAt();
    const row = wrapper.find('[data-testid="ov-task-PM-101"]');
    expect(row.exists()).toBe(true);
    expect(row.text()).toContain("Dev Agent 1 · coordinated by AI-PM");
    expect(row.text()).toContain("Executing");
    expect(row.text()).toContain("Deps 2/4");
    // 仅进行中任务入表（completed 的 PM-103 不出现）
    expect(wrapper.find('[data-testid="ov-task-PM-103"]').exists()).toBe(false);
  });

  it("renders important risk and recent outcomes sections", async () => {
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="ov-risk"]').text()).toContain(
      "External data transfer is blocked by authorization"
    );
    const outcomes = wrapper.find('[data-testid="collab-outcomes"]');
    expect(outcomes.text()).toContain("T-1425 schema mapping completed");
  });

  it("renders the AI-PM context panel with direction, boundary and progress", async () => {
    const wrapper = await mountAt();
    const ctx = wrapper.find('[data-testid="collab-attention"]').element
      .parentElement as HTMLElement | null;
    const root = wrapper.element;
    expect(root.textContent).toContain("AI-PM · Project context");
    expect(root.textContent).toContain(
      "Ship reliable incremental sync for Atlas 2.4 by 27 Nov."
    );
    expect(root.textContent).toContain("Objective v3 · confirmed by a human");
    expect(root.textContent).toContain("Progress, not supervision");
    void ctx;
  });

  it("shows error feedback with retry when loading fails", async () => {
    fetchDecisionCenter.mockRejectedValue(new Error("boom"));
    const wrapper = await mountAt();
    const err = wrapper.find('[data-testid="screen-feedback-error"]');
    expect(err.exists()).toBe(true);
    expect(err.text()).toContain("Retry safely");
  });

  it("bottom AI-PM input is rendered on the tab shell", async () => {
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="collab-input"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="collab-input-field"]').exists()).toBe(
      true
    );
  });
});
