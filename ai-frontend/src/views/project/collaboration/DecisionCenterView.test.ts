// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import DecisionCenterView from "./DecisionCenterView.vue";

/**
 * PA-06 屏 C（Human decision center）契约：
 *   - 左队列（consultation + approval 两类，选中 accent 底）+ 右详情
 *     （proposal 徽标 / WHY / recommendation / OPTIONS）；
 *   - consultation 决策 = 决策说明必填 → decideConsultation；
 *   - approval 裁决 = 理由必填（未填禁用）→ decideApproval approved|rejected；
 *   - 空态（无待决）=「常规 AI 工作无需批准继续」；已决行只读呈现（不抹历史）；
 *   - 底注：已确认决策保留在项目决策登记；提案不自动生效。
 */
const mountedWrappers: ReturnType<typeof mountShell>[] = [];

const fetchDecisionCenter = vi.fn();
const fetchConsultations = vi.fn();
const fetchPendingDecisions = vi.fn();
const decideConsultation = vi.fn();
const cancelConsultation = vi.fn();
const decideApproval = vi.fn();

vi.mock("@/api/collaboration", () => ({
  fetchDecisionCenter: (...args: unknown[]) => fetchDecisionCenter(...args),
  fetchConsultations: (...args: unknown[]) => fetchConsultations(...args),
  fetchPendingDecisions: (...args: unknown[]) => fetchPendingDecisions(...args),
  fetchWaitingDecisionCount: vi.fn().mockResolvedValue(2),
  decideConsultation: (...args: unknown[]) => decideConsultation(...args),
  cancelConsultation: (...args: unknown[]) => cancelConsultation(...args),
  fetchProjectRisks: vi.fn()
}));

vi.mock("@/api/pm", () => ({
  decideApproval: (...args: unknown[]) => decideApproval(...args),
  listTasks: vi.fn()
}));

function consultationRow(overrides: Record<string, unknown> = {}) {
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
    taskId: "t-1427",
    status: "pending",
    decisionContent: null,
    cancelledReason: null,
    ...overrides
  };
}

function approvalRow(overrides: Record<string, unknown> = {}) {
  return {
    kind: "approval",
    id: "a-1",
    kindLabel: "AI proposal · create_tasks",
    title: "Approve external EU verification provider",
    meta: "high · Raised 10:28",
    createdAt: "2026-11-21T10:28:00Z",
    why: "Red-line gate: external commitment requires an authorized acceptor.",
    recommendation: "Approve the provider for EU verification only.",
    options: [
      { label: "Recommended · EU provider", impact: "+$120/month" },
      { label: "Keep internal verification", impact: "T-1430 blocked" }
    ],
    taskId: null,
    status: "pending",
    decisionContent: null,
    cancelledReason: null,
    ...overrides
  };
}

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/collaboration/decisions?project_id=p-1");
  await router.isReady();
  const wrapper = mountShell(DecisionCenterView, {
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
    items: [consultationRow(), approvalRow()],
    pendingCount: 2,
    waitingDecisionCount: 2
  });
  fetchConsultations.mockResolvedValue([]);
  fetchPendingDecisions.mockResolvedValue([]);
});

describe("DecisionCenterView", () => {
  it("renders the queue with both kinds and selects the first item", async () => {
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="dc-queue"]').exists()).toBe(true);
    expect(
      wrapper.find('[data-testid="dc-queue-item-consultation"]').exists()
    ).toBe(true);
    expect(
      wrapper.find('[data-testid="dc-queue-item-approval"]').exists()
    ).toBe(true);
    const detail = wrapper.find('[data-testid="dc-detail"]');
    expect(detail.text()).toContain("Extend audit retention to 90 days");
    expect(detail.text()).toContain(
      "Changes the meaning of confirmed requirement REQ-18."
    );
  });

  it("shows proposal badge, options and approve/reject actions for approval rows", async () => {
    const wrapper = await mountAt();
    await wrapper
      .find('[data-testid="dc-queue-item-approval"]')
      .trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-testid="dc-proposal-badge"]').exists()).toBe(
      true
    );
    expect(wrapper.find('[data-testid="dc-detail"]').text()).toContain(
      "Recommended · EU provider"
    );
    // 理由未填 → 裁决按钮禁用（后端对空理由 400，前端提前拦）
    const approve = wrapper.find('[data-testid="dc-approve"]');
    expect((approve.element as HTMLButtonElement).disabled).toBe(true);
    const reject = wrapper.find('[data-testid="dc-reject"]');
    expect((reject.element as HTMLButtonElement).disabled).toBe(true);
  });

  it("submits a consultation decision with the required note", async () => {
    decideConsultation.mockResolvedValue(undefined);
    fetchDecisionCenter.mockResolvedValue({
      items: [consultationRow()],
      pendingCount: 1,
      waitingDecisionCount: 0
    });
    const wrapper = await mountAt();
    await wrapper
      .find('[data-testid="dc-decision-input"]')
      .setValue("Approved 90 days; keep encryption controls.");
    await wrapper.find('[data-testid="dc-decide"]').trigger("click");
    await flushPromises();
    await flushPromises();
    expect(decideConsultation).toHaveBeenCalledWith(
      "c-1",
      "Approved 90 days; keep encryption controls."
    );
  });

  it("submits an approval verdict through the red-line decide endpoint", async () => {
    decideApproval.mockResolvedValue({ id: "a-1", status: "approved" });
    fetchDecisionCenter.mockResolvedValue({
      items: [approvalRow()],
      pendingCount: 1,
      waitingDecisionCount: 0
    });
    const wrapper = await mountAt();
    await wrapper
      .find('[data-testid="dc-queue-item-approval"]')
      .trigger("click");
    await flushPromises();
    await wrapper
      .find('[data-testid="dc-decision-input"]')
      .setValue("Approved for EU verification only.");
    await wrapper.find('[data-testid="dc-approve"]').trigger("click");
    await flushPromises();
    await flushPromises();
    expect(decideApproval).toHaveBeenCalledWith("a-1", {
      decision: "approved",
      reason: "Approved for EU verification only."
    });
  });

  it("shows the empty state with the register note when nothing is pending", async () => {
    fetchDecisionCenter.mockResolvedValue({
      items: [],
      pendingCount: 0,
      waitingDecisionCount: 0
    });
    const wrapper = await mountAt();
    const empty = wrapper.find('[data-testid="screen-feedback-empty"]');
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain(
      "Ordinary AI work continues without approval"
    );
    expect(wrapper.text()).toContain(
      "Confirmed decisions remain in the project decision register"
    );
  });

  it("renders decided rows as read-only history (empty state never erases history)", async () => {
    fetchDecisionCenter.mockResolvedValue({
      items: [
        consultationRow({
          status: "decided",
          decisionContent: "Approved 90 days."
        })
      ],
      pendingCount: 0,
      waitingDecisionCount: 0
    });
    const wrapper = await mountAt();
    // 队列仍展示该行（空态不抹历史），但无输入与动作区
    expect(wrapper.find('[data-testid="dc-decision-input"]').exists()).toBe(
      false
    );
    expect(wrapper.find('[data-testid="dc-decided"]').text()).toContain(
      "Approved 90 days."
    );
  });

  it("shows error feedback with retry on load failure", async () => {
    fetchDecisionCenter.mockRejectedValue(new Error("boom"));
    const wrapper = await mountAt();
    const err = wrapper.find('[data-testid="screen-feedback-error"]');
    expect(err.exists()).toBe(true);
    expect(err.text()).toContain("Retry safely");
  });
});
