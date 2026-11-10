// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import HumanInterventionView from "./HumanInterventionView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type { HumanAttentionQueue } from "@/api/execution-plans";

/**
 * 人工介入（S05-04，接真）行为测试：
 * 五个 waiting reason 各自成段——每值是不同的处置路径，**不得合并**。
 */

const getHumanAttentionQueue = vi.fn();
const getExecutionOverview = vi.fn();

vi.mock("@/api/execution-plans", () => ({
  getHumanAttentionQueue: (...args: unknown[]) =>
    getHumanAttentionQueue(...args),
  getExecutionOverview: (...args: unknown[]) => getExecutionOverview(...args)
}));

const REASONS = [
  "needs_info",
  "needs_decision",
  "needs_approval",
  "external_dependency",
  "no_executor_available"
] as const;

function queue(): HumanAttentionQueue {
  return {
    waiting: REASONS.map((reason, index) => ({
      id: `w-${index}`,
      task_id: `task-${index}`,
      task_title: `任务 ${reason}`,
      task_status: "awaiting_decision",
      task_priority: "normal",
      reason,
      questions: [],
      requested_from: null,
      since: "2026-11-10T08:00:00Z",
      deadline: null
    })),
    interactions: [],
    approvals: []
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/execution/intervention");
  await router.isReady();
  const { wrapper } = mountShell(
    HumanInterventionView,
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
  getExecutionOverview.mockReset();
  getHumanAttentionQueue.mockResolvedValue(queue());
  getExecutionOverview.mockResolvedValue({
    active_run_count: 0,
    waiting_count: 5,
    pending_interaction_count: 0,
    pending_approval_count: 0,
    failed_run_count: 0,
    awaiting_review_count: 0,
    total_run_count: 0,
    succeeded_run_count: 0,
    success_rate: null,
    average_duration_seconds: null,
    device_ready_count: 0,
    device_total_count: 0,
    waiting_by_reason: REASONS.map(reason => [reason, 1])
  });
});

describe("HumanInterventionView", () => {
  it("五个等待原因各自成段，逐一可辨", async () => {
    const { wrapper } = await mountView();

    for (const reason of REASONS) {
      const section = wrapper.find(`[data-testid="hi-section-${reason}"]`);
      expect(section.exists()).toBe(true);
      expect(section.text().length).toBeGreaterThan(0);
    }
  });

  it("无等待项时每段给空态而非隐藏", async () => {
    getHumanAttentionQueue.mockResolvedValue({
      waiting: [],
      interactions: [],
      approvals: []
    });
    const { wrapper } = await mountView();

    expect(wrapper.find('[data-testid="hi-section-needs_info"]').exists()).toBe(
      true
    );
  });
});
