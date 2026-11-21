// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import RecoveryView from "./RecoveryView.vue";

/**
 * PA-06 屏 G（Exceptions & recovery）契约：
 *   - 重大风险升级卡=open 风险（PA-05 Risk 域）→ 决策中心入口；
 *   - 常规恢复段=waiting 队列非决策行 + blocked 任务（真实原因，不编造
 *     「Attempt 2 of 3」计数）；无异常 → 诚实空态；
 *   - 恢复上限语义（PA-06-06 §8.2）常驻。
 */
const mountedWrappers: ReturnType<typeof mountShell>[] = [];

const fetchProjectRisks = vi.fn();
const fetchWaitingQueue = vi.fn();
const listTasks = vi.fn();
const fetchConsultations = vi.fn();
const fetchPendingDecisions = vi.fn();

vi.mock("@/api/collaboration", () => ({
  fetchProjectRisks: (...args: unknown[]) => fetchProjectRisks(...args),
  fetchWaitingQueue: (...args: unknown[]) => fetchWaitingQueue(...args),
  fetchDecisionCenter: vi.fn(),
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

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/collaboration/recovery?project_id=p-1");
  await router.isReady();
  const wrapper = mountShell(RecoveryView, {
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
  fetchProjectRisks.mockResolvedValue([
    {
      id: "r-1",
      project_id: "p-1",
      title: "External verification crosses the data boundary",
      description:
        "The proposed provider is outside the approved vendor list. No customer data has been transferred.",
      impact: "high",
      probability: "medium",
      status: "open",
      created_at: "2026-11-21T10:28:00Z"
    }
  ]);
  fetchWaitingQueue.mockResolvedValue([
    {
      task_id: "task-9",
      task_title: "PM-103 External verification",
      reason: "external_dependency",
      since: "2026-11-21T10:40:00Z",
      deadline: null
    }
  ]);
  listTasks.mockResolvedValue({
    items: [
      {
        id: "task-2",
        code: "PM-102",
        title: "Conflict checks",
        status: "blocked",
        assignee_id: null,
        deps_total: 0,
        deps_met: 0
      }
    ],
    total: 1
  });
  fetchConsultations.mockResolvedValue([]);
  fetchPendingDecisions.mockResolvedValue([]);
});

describe("RecoveryView", () => {
  it("renders the major-risk escalation card with the decision entry", async () => {
    const wrapper = await mountAt();
    const major = wrapper.find('[data-testid="rc-major"]');
    expect(major.exists()).toBe(true);
    expect(major.text()).toContain(
      "External verification crosses the data boundary"
    );
    expect(major.text()).toContain("Review authorization request");
    expect(wrapper.find('[data-testid="rc-head"], .rc-head').exists()).toBe(
      true
    );
  });

  it("renders routine recovery rows from real waiting reasons and blocked tasks", async () => {
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="rc-routine-task-9"]').exists()).toBe(
      true
    );
    expect(wrapper.find('[data-testid="rc-routine-task-9"]').text()).toContain(
      "external_dependency"
    );
    expect(wrapper.find('[data-testid="rc-routine-task-2"]').exists()).toBe(
      true
    );
    // 不编造重试计数
    expect(wrapper.text()).not.toContain("Attempt 2 of 3");
  });

  it("shows the honest empty state when there are no exceptions", async () => {
    fetchProjectRisks.mockResolvedValue([]);
    fetchWaitingQueue.mockResolvedValue([]);
    listTasks.mockResolvedValue({ items: [], total: 0 });
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="screen-feedback-empty"]').exists()).toBe(
      true
    );
    expect(wrapper.text()).toContain("Ordinary failures recover autonomously");
  });
});
