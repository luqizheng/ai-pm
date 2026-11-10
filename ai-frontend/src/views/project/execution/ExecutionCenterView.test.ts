// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import ExecutionCenterView from "./ExecutionCenterView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type {
  HumanAttentionQueue,
  RunEvent,
  TaskRun
} from "@/api/execution-plans";

/**
 * 执行中心（S05-01，接真）行为测试：
 * 需要人工处理队列按 **waiting reason 五值**原样呈现（不合并成「等待中」），
 * 待答交互带问题原文；活跃 Run 与排队 Run 分列。
 */

const getHumanAttentionQueue = vi.fn();
const listTaskRuns = vi.fn();
const listProjectRunEvents = vi.fn();
const getExecutionOverview = vi.fn();

vi.mock("@/api/execution-plans", () => ({
  getHumanAttentionQueue: (...args: unknown[]) =>
    getHumanAttentionQueue(...args),
  listTaskRuns: (...args: unknown[]) => listTaskRuns(...args),
  listProjectRunEvents: (...args: unknown[]) => listProjectRunEvents(...args),
  getExecutionOverview: (...args: unknown[]) => getExecutionOverview(...args)
}));

function run(overrides: Partial<TaskRun> = {}): TaskRun {
  return {
    id: "run-1",
    task_id: "task-1",
    task_title: "实现登录端点",
    attempt_no: 2,
    run_kind: "delivery",
    status: "running",
    outcome: null,
    agent_id: "agent-1",
    executor_kind: "spawn",
    executor_session: null,
    lease_expires_at: null,
    created_at: "2026-11-10T08:00:00Z",
    started_at: "2026-11-10T08:01:00Z",
    finished_at: null,
    review_verdict: null,
    failure_category: null,
    failure_reason: null,
    ...overrides
  };
}

function queue(): HumanAttentionQueue {
  return {
    waiting: [
      {
        id: "w-1",
        task_id: "task-1",
        task_title: "实现登录端点",
        task_status: "awaiting_decision",
        task_priority: "high",
        reason: "needs_info",
        questions: [],
        requested_from: null,
        since: "2026-11-10T08:00:00Z",
        deadline: null
      }
    ],
    interactions: [
      {
        id: "i-1",
        task_id: "task-1",
        task_title: "实现登录端点",
        run_id: "run-1",
        reason: "needs_info",
        title: "澄清口令策略",
        question: "使用哪种口令策略？",
        options: ["A", "B"],
        required_response: true,
        deadline_at: "2026-11-10T12:00:00Z",
        created_at: "2026-11-10T08:00:00Z"
      }
    ],
    approvals: []
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/execution");
  await router.isReady();
  const { wrapper } = mountShell(
    ExecutionCenterView,
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
  getHumanAttentionQueue.mockReset();
  listTaskRuns.mockReset();
  listProjectRunEvents.mockReset();
  getExecutionOverview.mockReset();
  getHumanAttentionQueue.mockResolvedValue(queue());
  listTaskRuns.mockResolvedValue({ items: [run()], total: 1 });
  listProjectRunEvents.mockResolvedValue([
    {
      id: "e-1",
      run_id: "run-1",
      task_id: "task-1",
      event_type: "run_started",
      lease_generation: 0,
      client_id: null,
      occurred_at: "2026-11-10T08:01:00Z",
      received_at: "2026-11-10T08:01:00Z",
      detail: null
    } satisfies RunEvent
  ]);
  getExecutionOverview.mockResolvedValue({
    active_run_count: 1,
    waiting_count: 1,
    pending_interaction_count: 1,
    pending_approval_count: 0,
    failed_run_count: 0,
    awaiting_review_count: 0,
    total_run_count: 1,
    succeeded_run_count: 0,
    success_rate: 0,
    average_duration_seconds: null,
    device_ready_count: 1,
    device_total_count: 1,
    waiting_by_reason: [["needs_info", 1]]
  });
});

describe("ExecutionCenterView", () => {
  it("按等待原因五值呈现人工队列，待答交互带问题原文", async () => {
    const { wrapper } = await mountView();

    expect(getHumanAttentionQueue).toHaveBeenCalledWith("proj-1");
    // 原因是具体值（待澄清），不是笼统的「等待中」
    const attention = wrapper.find('[data-testid="ec-attention"]').text();
    expect(attention).toContain("待澄清");
    expect(attention).toContain("使用哪种口令策略？");
  });

  it("活跃 Run 与排队 Run 分列，Run 状态与 attempt 可辨", async () => {
    listTaskRuns.mockResolvedValue({
      items: [run(), run({ id: "run-2", status: "queued", attempt_no: 3 })],
      total: 2
    });
    const { wrapper } = await mountView();

    expect(wrapper.find('[data-testid="ec-active"]').text()).toContain(
      "执行中"
    );
    expect(wrapper.find('[data-testid="ec-queue"]').text()).toContain("排队");
    // attempt 号带 kind
    expect(wrapper.find('[data-testid="ec-active"]').text()).toContain("#2");
  });

  it("活动流呈现真实事件类型", async () => {
    const { wrapper } = await mountView();

    expect(wrapper.find('[data-testid="ec-events"]').text()).toContain("启动");
  });
});
